//! Phase-5 device-lifecycle resolution pass (T09 override-check + T10 field
//! classification).
//!
//! Runs AFTER `materialise_drivers` (Phase 4, descriptors interned) and BEFORE
//! `run_phase5_body_passes` (the drain that materializes function bodies). For
//! every driver that a `lifecycle::bind::<D>()` site bound, it:
//!
//!   * **T09 — override-check + callback synthesis.** Resolves each of the five
//!     `Lifecycle` trait methods for `D` via `Instance::try_resolve`. A method
//!     the `impl Lifecycle for D` overrides resolves to a DISTINCT (impl) def-id
//!     from the trait-default; that overriding instance is FORCE-MATERIALIZED
//!     (so the Phase-5 drain gives it a body the backend can emit) and its
//!     stable `InstanceRef` is stored in `LifecycleOps`. A non-overridden method
//!     resolves back to the trait default and stays `None` (the four "glued"
//!     callbacks still emit their `LC call <cb>` marker at the backend — CR-3 —
//!     but synthesize no author call).
//!   * **T10 — field classification.** Splits `D`'s fields into compiler-bound
//!     resources (`Mmio<T>`) and non-resource user-data. The full Rule-1/Rule-2
//!     user-data classification is T11; this pass only collects the resource
//!     fields (the M0 `Fake` driver is fieldless, so the handle is empty).
//!
//! This is the bridge half of the callback-synthesis seam: the first construct
//! that synthesizes a CALL to an author callback (`on_init`, …) that appears
//! nowhere in the MIR. `InstanceRef` (content-addressed) is the stable handle
//! because the Phase-4 (`F4-2`) instance sort reorders `model.instances`.

use std::collections::HashMap;

use regseq_model::{
    DriverHandle, InstanceRef, ItemId, LifecycleOps, MmioBinding, ResourceField, ResourceKind,
    UserDataField, UserDataKind,
};
use rustc_hir::def::DefKind;
use rustc_middle::ty::{self, Ty, TyCtxt, TyKind, UintTy};
use rustc_span::{def_id::DefId, Symbol as RustcSymbol};

use super::super::const_::instance_ref_for_function_provenance;
use super::super::framework_type::recover_source_def_id;
use super::super::ty::translate_ty;
use super::super::{BridgeCtxt, BridgeError, DeviceLifecycleDiagnostic};

/// The author-facing `Lifecycle` trait is identified by name (RD-2), mirroring
/// the name-based `Mmio` wrapper detection in [`is_mmio_resource`]. The
/// inline-per-fixture surface declares `trait Lifecycle` with no marker
/// attribute, so name identity is the anchor — no `bind<D: Lifecycle>` bound is
/// needed.
const LIFECYCLE_TRAIT_NAME: &str = "Lifecycle";

/// Resolve every interned driver's field split + lifecycle callbacks (RD-2/RD-3).
///
/// **Item-driven** (not bind-driven): walks every `#[rseq::driver]` interned by
/// `materialise_drivers`, independent of any `lifecycle::bind` site. For each:
///
///   * **Field classification runs ALWAYS** (T10) — a driver with no
///     `impl Lifecycle` is still a valid driver whose register / user-data
///     fields are classified (construct-level optionality, B1/RD-3).
///   * **Callback resolution runs only when the driver has an `impl Lifecycle`**
///     (T09); otherwise its [`LifecycleOps`] stay all-`None`. The `Lifecycle`
///     trait is identified by name from the driver's trait impls
///     ([`lifecycle_impls_by_driver`]), so no `bind` bound is needed to anchor
///     it.
pub(crate) fn resolve_drivers(cx: &mut BridgeCtxt<'_>) -> Result<(), BridgeError> {
    let tcx = cx.input.tcx;

    // The `impl Lifecycle for <Driver>` map (driver-struct def-id -> the
    // `Lifecycle` trait def-id). A driver absent from the map has no
    // `impl Lifecycle`, so its callbacks stay `None` (B1/RD-3).
    let lifecycle_impls = lifecycle_impls_by_driver(tcx);

    // Every interned driver, as (descriptor key, driver-struct def-id).
    let targets: Vec<(ItemId, DefId)> = cx
        .model
        .device_lifecycle
        .drivers
        .keys()
        .copied()
        .filter_map(|item_id| {
            let def_id = recover_source_def_id(tcx, cx.model.item_def_paths[item_id])?;
            Some((item_id, def_id))
        })
        .collect();

    for (item_id, d_def_id) in targets {
        // T10/T11/B1: classify fields (resources + user-data) for EVERY driver,
        // impl or not. The driver's logical name (for a D3 diagnostic) is read
        // first to avoid aliasing the mutable `cx` borrow `classify_fields` takes.
        let name_id = cx.model.device_lifecycle.drivers[&item_id].descriptor.name;
        let driver_name = cx.model.strings.resolve(name_id).to_string();
        let handle = classify_fields(cx, d_def_id, &driver_name)?;
        // T09: resolve callbacks only when the driver has an `impl Lifecycle`.
        let ops = match lifecycle_impls.get(&d_def_id).copied() {
            Some(lifecycle_trait) => {
                let d_ty = tcx.type_of(d_def_id).instantiate_identity();
                resolve_lifecycle_ops(cx, lifecycle_trait, d_ty)?
            }
            None => LifecycleOps::default(),
        };
        if let Some(driver) = cx.model.device_lifecycle.drivers.get_mut(&item_id) {
            driver.ops = ops;
            driver.handle = handle;
        }
    }
    Ok(())
}

/// Build the `impl Lifecycle for <Driver>` map: every local trait impl of a
/// trait *named* `"Lifecycle"` ([`LIFECYCLE_TRAIT_NAME`]), keyed by the impl's
/// Self-type ADT def-id, valued by the `Lifecycle` trait def-id. The override
/// resolution ([`resolve_lifecycle_ops`]) needs only the trait def-id + the
/// driver type, so name identity is a sufficient anchor (no `bind` bound). A
/// driver with no such impl is simply absent → its callbacks stay `None`
/// (B1/RD-3).
fn lifecycle_impls_by_driver(tcx: TyCtxt<'_>) -> HashMap<DefId, DefId> {
    let mut by_driver: HashMap<DefId, DefId> = HashMap::new();
    for local_def_id in tcx.hir_crate_items(()).definitions() {
        let impl_def_id = local_def_id.to_def_id();
        // Only trait impls carry a `TraitRef` (the implemented trait `def_id` +
        // the implementing Self type); inherent impls (`of_trait: false`) do not.
        if !matches!(tcx.def_kind(impl_def_id), DefKind::Impl { of_trait: true }) {
            continue;
        }
        let trait_ref = tcx.impl_trait_ref(impl_def_id).skip_binder();
        if tcx.item_name(trait_ref.def_id).as_str() != LIFECYCLE_TRAIT_NAME {
            continue;
        }
        if let Some(adt) = trait_ref.self_ty().ty_adt_def() {
            by_driver.insert(adt.did(), trait_ref.def_id);
        }
    }
    by_driver
}

/// Override-check the five `Lifecycle` callbacks for `d_ty`, force-materializing
/// each overriding instance and storing its `InstanceRef`.
fn resolve_lifecycle_ops<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    lifecycle_trait: DefId,
    d_ty: Ty<'tcx>,
) -> Result<LifecycleOps, BridgeError> {
    let tcx = cx.input.tcx;
    // Collect (method name, trait-method def-id) before touching `cx` mutably.
    let methods: Vec<(rustc_span::Symbol, DefId)> = tcx
        .associated_items(lifecycle_trait)
        .in_definition_order()
        .filter(|item| item.is_fn())
        .map(|item| (item.name(), item.def_id))
        .collect();
    // The trait method's only generic arg is `Self = D` (the callbacks take
    // `&mut self` and add no generics of their own).
    let args = tcx.mk_args(&[d_ty.into()]);

    let mut ops = LifecycleOps::default();
    for (name, trait_method_def_id) in methods {
        let resolved = ty::Instance::try_resolve(
            tcx,
            ty::TypingEnv::fully_monomorphized(),
            trait_method_def_id,
            args,
        );
        let Ok(Some(instance)) = resolved else {
            continue;
        };
        // Not overridden: resolves back to the trait-default method => `None`.
        if instance.def_id() == trait_method_def_id {
            continue;
        }
        let iref = instance_ref_for_function_provenance(cx, instance)?;
        assign_callback_slot(&mut ops, name.as_str(), iref);
    }
    Ok(ops)
}

/// Route a resolved callback `InstanceRef` to its named `LifecycleOps` slot. An
/// out-of-set method name cannot occur (rustc E0407 rejects a sixth method).
fn assign_callback_slot(ops: &mut LifecycleOps, name: &str, iref: InstanceRef) {
    match name {
        "on_init" => ops.on_init = Some(iref),
        "on_deinit" => ops.on_deinit = Some(iref),
        "on_surprise_remove" => ops.on_surprise_remove = Some(iref),
        "on_init_failure" => ops.on_init_failure = Some(iref),
        "on_system_shutdown" => ops.on_system_shutdown = Some(iref),
        _ => {}
    }
}

/// T10 + T11: split the driver struct's fields into resources (`Mmio<T>`) and
/// user-data, in declaration order (the lifecycle glue runs preamble/postamble in
/// that order). Each non-resource field is classified into its user-data init
/// rule ([`classify_user_data`]); a field that is neither Rule-1 nor Rule-2 is
/// rejected as the D3 diagnostic — the rejection is decided BEFORE [`translate_ty`]
/// so a bad field is a clean D3, not a generic type-translator error. Build-now
/// admits the scalar rules plus recursively zero-initializable POD structs. Runs
/// for EVERY driver, impl or not (B1/RD-3); a fieldless driver yields an empty
/// handle.
fn classify_fields(
    cx: &mut BridgeCtxt<'_>,
    d_def_id: DefId,
    driver_name: &str,
) -> Result<DriverHandle, BridgeError> {
    let tcx = cx.input.tcx;
    let adt = tcx.adt_def(d_def_id);
    let mut resource_fields = Vec::new();
    let mut user_data_fields = Vec::new();
    for variant in adt.variants() {
        for field in &variant.fields {
            let field_ty = tcx.type_of(field.did).instantiate_identity();
            // Device-interrupt fields (construct #6) are compiler-bound
            // resources owned by `bridge::interrupt` (recognized in its
            // Phase-4 pass) — route them away from the user-data classifier,
            // whose D3 would otherwise reject the methodless `Interrupt`
            // shell. They are not `ResourceField`s either: the interrupt
            // preamble's position is derived from `model.interrupt`, ordered
            // after all other resource preambles (D14).
            if crate::bridge::interrupt::is_interrupt_field(tcx, field_ty) {
                continue;
            }
            // Atomicity fields (construct #7) are driver-local `Atomic<T>` state
            // (zero L2, no lifecycle — D11), recognized + recorded by
            // `bridge::atomic::materialise_atomics` (element-type legality +
            // the `AttrData::Atomic` field-attr the backends read for storage +
            // init). Synchronization lock fields are compiler-managed resources
            // owned by `bridge::synchronization` (recognized in its Phase-4 pass).
            // Both route away from the user-data D3 classifier (whose
            // `UserDataNotInitializable` rule would reject the `Atomic<T>` / lock
            // shell); the per-target backends override storage + emit init.
            if crate::bridge::atomic::is_atomic_field(tcx, field_ty)
                || crate::bridge::synchronization::is_sync_field(tcx, field_ty)
            {
                continue;
            }
            // Peripheral-clock-management fields (clock construct) are
            // compiler-bound resources owned by `bridge::clock` (recognized in
            // its Phase-4 pass) — route them away from the user-data classifier,
            // whose D3 would otherwise reject the methodless `Clock` shell. They
            // are not `ResourceField`s either: the `acquire_clock` preamble's
            // position is derived from `model.clock`, ordered after `map_region`
            // and before interrupt (GRILL D16/C07).
            if crate::bridge::clock::is_clock_field(tcx, field_ty) {
                continue;
            }
            // Pin-controller-and-GPIO-configuration fields (pinctrl construct)
            // are compiler-bound resources owned by `bridge::pinctrl` (recognized
            // in its Phase-4 pass) — route them away from the user-data
            // classifier, whose D3 would otherwise reject the methodless `PinCtrl`
            // shell. They are not `ResourceField`s either: the `acquire_pinctrl`
            // preamble's position is derived from `model.pinctrl`, ordered after
            // `map_region` and before `acquire_clock` (pins-before-clock, D08/C07).
            if crate::bridge::pinctrl::is_pinctrl_field(tcx, field_ty) {
                continue;
            }
            // Deferred-work-item-scheduling fields (deferred-work-item-scheduling
            // construct) are compiler-bound resources owned by
            // `bridge::deferred_work` (recognized in its Phase-4 pass) — route
            // them away from the user-data classifier, whose D3 would otherwise
            // reject the methodless `Work` shell. They are not `ResourceField`s
            // either: a `Work` has no address footprint; its `init_work`/
            // `cancel_work` lifecycle is synthesized from the recognized field
            // (GRILL D04). Recognized BY TYPE (the
            // `#[rseq::abstraction("deferred_work")]` marker), not a field attr.
            if crate::bridge::deferred_work::is_work_field(tcx, field_ty) {
                continue;
            }
            if is_mmio_resource(tcx, field_ty) {
                // The canonical register-access facade lives at
                // `regseq::Mmio<T>`. Its extraction contract requires `T` to be
                // an explicit `#[rseq::register_block]` type. Legacy inline
                // fixtures use a crate-root `Mmio<T>` and remain on their
                // existing compatibility path; the canonical path never falls
                // back to accepting an unmarked lookalike block.
                if let Some(block_ty) = canonical_mmio_block_ty(tcx, field_ty) {
                    if !is_register_block_type(tcx, block_ty) {
                        return Err(BridgeError::RegisterAccessValidation {
                            driver: driver_name.to_string(),
                            field: field.name.to_string(),
                            block_type: block_ty.to_string(),
                        });
                    }
                }
                // Recognize the bus-resource-management L1 facts on the shared
                // `#[rseq::mmio(...)]` field attribute (construct #3, T03): the
                // `id` join key + the optional `bar`/`type`. `(base, size)` /
                // `effective_bar` stay `None` until the platform-file join (T05).
                let parsed = super::recognition::recognise_mmio_field_attr(tcx, field.did);
                let id = parsed.id.map(|s| cx.model.strings.intern(&s));
                let name = cx.model.strings.intern(field.name.as_str());
                resource_fields.push(ResourceField {
                    name,
                    kind: ResourceKind::Mmio,
                    binding: MmioBinding {
                        id,
                        bar_index: parsed.bar_index,
                        bar_type: parsed.bar_type,
                        effective_bar: None,
                        base: None,
                        size: None,
                    },
                });
                continue;
            }
            // T11: a non-resource field is user-data. Classify (and reject, D3)
            // from the rustc type BEFORE translating it to a model `TyId`.
            let kind = classify_user_data(tcx, field_ty).ok_or_else(|| {
                BridgeError::DeviceLifecycleValidation(
                    DeviceLifecycleDiagnostic::UserDataNotInitializable {
                        driver: driver_name.to_string(),
                        field: field.name.to_string(),
                        rejected_type: field_ty.to_string(),
                    },
                )
            })?;
            let ty = translate_ty(cx, field_ty)?;
            let name = cx.model.strings.intern(field.name.as_str());
            user_data_fields.push(UserDataField { name, ty, kind });
        }
    }
    Ok(DriverHandle {
        resource_fields,
        user_data_fields,
    })
}

/// Classify a non-resource driver field's type into its user-data init rule, or
/// `None` if it is neither (→ the D3 diagnostic). Rule-2: `Option<T: Copy>` →
/// [`UserDataKind::OptionCopy`]. Rule-1 (zero-init):
/// `bool`/`u8`/`u16`/`u32`/`u64`, or a [`zero_initializable_pod_struct`], →
/// [`UserDataKind::ZeroInit`]. Arrays, signed/`u128`/`usize` ints, references,
/// resources, unions, and non-POD aggregates remain rejected.
fn classify_user_data<'tcx>(tcx: TyCtxt<'tcx>, ty: Ty<'tcx>) -> Option<UserDataKind> {
    // Rule-2: `Option<T: Copy>` — `None` is the unseeded state.
    if let TyKind::Adt(adt, args) = ty.kind() {
        if tcx.get_diagnostic_name(adt.did()) == Some(rustc_span::sym::Option) {
            let inner = args.types().next()?;
            let typing_env = ty::TypingEnv::fully_monomorphized();
            return tcx
                .type_is_copy_modulo_regions(typing_env, inner)
                .then_some(UserDataKind::OptionCopy);
        }
    }
    // Rule-1: the scalar zero-init whitelist (these are inherently `Copy`) or
    // the scoped recursively zero-initializable POD-struct extension.
    match ty.kind() {
        TyKind::Bool => Some(UserDataKind::ZeroInit),
        TyKind::Uint(UintTy::U8 | UintTy::U16 | UintTy::U32 | UintTy::U64) => {
            Some(UserDataKind::ZeroInit)
        }
        TyKind::Adt(adt, _) if adt.is_struct() && zero_initializable_pod_struct(tcx, ty) => {
            Some(UserDataKind::ZeroInit)
        }
        _ => None,
    }
}

/// Whether `ty` is a Copy, non-dropping struct whose all-zero image recursively
/// denotes a valid value. This is intentionally narrower than Rust's general
/// aggregate/type admissibility: only the existing Rule-1 scalars, nested POD
/// structs, and fieldless scalar enums with a zero discriminant are members.
/// Compiler-managed resources are rejected even if their Rust representation
/// happens to be composed from scalar storage.
fn zero_initializable_pod_struct<'tcx>(tcx: TyCtxt<'tcx>, ty: Ty<'tcx>) -> bool {
    let TyKind::Adt(adt, args) = ty.kind() else {
        return false;
    };
    if !adt.is_struct() || compiler_managed_resource(tcx, ty) {
        return false;
    }

    let typing_env = ty::TypingEnv::fully_monomorphized();
    if !tcx.type_is_copy_modulo_regions(typing_env, ty) || ty.needs_drop(tcx, typing_env) {
        return false;
    }

    let fields = &adt.non_enum_variant().fields;
    !fields.is_empty()
        && fields
            .iter()
            .all(|field| zero_initializable_pod_member(tcx, field.ty(tcx, args)))
}

fn zero_initializable_pod_member<'tcx>(tcx: TyCtxt<'tcx>, ty: Ty<'tcx>) -> bool {
    match ty.kind() {
        TyKind::Bool | TyKind::Uint(UintTy::U8 | UintTy::U16 | UintTy::U32 | UintTy::U64) => true,
        TyKind::Adt(adt, _) if adt.is_struct() => zero_initializable_pod_struct(tcx, ty),
        TyKind::Adt(adt, _) if adt.is_enum() => {
            let typing_env = ty::TypingEnv::fully_monomorphized();
            tcx.type_is_copy_modulo_regions(typing_env, ty)
                && !ty.needs_drop(tcx, typing_env)
                && adt
                    .variants()
                    .iter()
                    .all(|variant| variant.fields.is_empty())
                && adt
                    .discriminants(tcx)
                    .any(|(_, discriminant)| discriminant.val == 0)
        }
        _ => false,
    }
}

fn compiler_managed_resource<'tcx>(tcx: TyCtxt<'tcx>, ty: Ty<'tcx>) -> bool {
    is_mmio_resource(tcx, ty)
        || crate::bridge::interrupt::is_interrupt_field(tcx, ty)
        || crate::bridge::atomic::is_atomic_field(tcx, ty)
        || crate::bridge::synchronization::is_sync_field(tcx, ty)
        || crate::bridge::clock::is_clock_field(tcx, ty)
        || crate::bridge::pinctrl::is_pinctrl_field(tcx, ty)
        || crate::bridge::deferred_work::is_work_field(tcx, ty)
}

/// Is `ty` a register-access `Mmio<T>` resource? Detected by the wrapper struct
/// name, matching the inline-per-fixture surface. (T24 exercises this with a
/// real `Mmio` field; M0 has none.)
fn is_mmio_resource(tcx: TyCtxt<'_>, ty: Ty<'_>) -> bool {
    ty.ty_adt_def()
        .is_some_and(|adt| tcx.item_name(adt.did()).as_str() == "Mmio")
}

/// Return the `T` in the canonical `regseq::Mmio<T>` facade path. Name-only,
/// crate-root `Mmio<T>` shells predate the register-access facade and are kept
/// distinct so locked inline fixtures do not masquerade as canonical resources.
fn canonical_mmio_block_ty<'tcx>(tcx: TyCtxt<'tcx>, ty: Ty<'tcx>) -> Option<Ty<'tcx>> {
    let TyKind::Adt(adt, args) = ty.kind() else {
        return None;
    };
    if tcx.item_name(adt.did()).as_str() != "Mmio"
        || tcx.item_name(tcx.parent(adt.did())).as_str() != "regseq"
    {
        return None;
    }
    args.types().next()
}

/// The canonical L1 register-block marker consumed by extraction.
fn is_register_block_type(tcx: TyCtxt<'_>, ty: Ty<'_>) -> bool {
    let Some(adt) = ty.ty_adt_def() else {
        return false;
    };
    let path = [
        RustcSymbol::intern("rseq"),
        RustcSymbol::intern("register_block"),
    ];
    let present = tcx.get_attrs_by_path(adt.did(), &path).next().is_some();
    present
}
