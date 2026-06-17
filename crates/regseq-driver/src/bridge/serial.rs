//! Phase-5 serial device-class recognition pass (construct #7, PLAN T03).
//!
//! Runs AFTER `device_lifecycle::resolve_drivers` (descriptors interned, device
//! class known) and BEFORE the Phase-5 body drain, so the force-materialized
//! serial op bodies get bodies the backend can emit — the same discipline as
//! the lifecycle callbacks and the ISR handlers.
//!
//! For every `#[rseq::device_class(serial)]` driver, it collects the
//! framework-pure v1 in-scope serial surface (GRILL D02/D03):
//!
//!   * base `Serial` (C04) — REQUIRED; a serial-class driver missing it is a
//!     frontend error (SER-ERR-MISSING-BASE);
//!   * `SerialRuntimeLineConfig` (C03) — optional plane (impl-presence);
//!   * `SerialInterruptDriven` (C01) — optional plane (impl-presence).
//!
//! Each trait's methods are resolved via `Instance::try_resolve` and
//! force-materialized to content-addressed `InstanceRef`s (`instance_ref_for_
//! function_provenance`). The support-enum variant maps (`SerialIrqSource`,
//! `IrqSourceState`, `ConfigResult`, `PortScope`, `ReadyDir`, `DataBits`,
//! `Parity`, `StopBits`) are captured from the recognized method signatures and
//! validated against the closed vocab (I02 normalization; the spine). This
//! replaces the deferred/NULL-api treatment of `ClassTag::Serial` ADDITIVELY —
//! a non-serial class is untouched.

use regseq_model::{
    ClassTag, DriverSerial, EnumBinding, InstanceRef, ItemId, SerialBaseOps, SerialCause,
    SerialInterruptOps, SerialLineConfigOps, SerialQueueControlOps, SerialRxErrorKind,
    SerialTypeBindings, VariantIdx,
};
use rustc_hir::def::DefKind;
use rustc_middle::mir;
use rustc_middle::mir::visit::Visitor as MirVisitor;
use rustc_middle::ty::{self, Ty, TyCtxt};
use rustc_span::def_id::DefId;

use super::const_::instance_ref_for_function_provenance;
use super::framework_type::recover_source_def_id;
use super::{BridgeCtxt, BridgeError, SerialDiagnostic};

const SERIAL_TRAIT: &str = "Serial";
const LINE_CONFIG_TRAIT: &str = "SerialRuntimeLineConfig";
const INTERRUPT_DRIVEN_TRAIT: &str = "SerialInterruptDriven";
const QUEUE_CONTROL_TRAIT: &str = "SerialQueueControl";

// T04 reject matrix (R07/R10). The deferred members carried INSIDE an in-scope
// trait (declared with default bodies on the author surface so the in-scope
// impl stays complete); any author OVERRIDE is rejected. Keep in step with the
// `_serial_surface.rs` trait declarations.
const LINE_CONFIG_DEFERRED_MEMBERS: &[&str] =
    &["line_format_readback", "break_set", "flow_auto_set"];
const QUEUE_CONTROL_DEFERRED_MEMBERS: &[&str] = &["fifo_purge_hw"];
// The deferred COMPANION traits — any impl on a serial-class driver is rejected.
const DEFERRED_TRAITS: &[&str] = &["SerialModemControl", "SerialConsole", "SerialBulkTransfer"];

// O2 hardening: the author bit-position convention consts and their REQUIRED
// values (`1 << <canonical discriminant>`). The backends decode the authored
// `CauseSet`/`RxError` bitsets against the pinned `CauseSetRepr`/`RxErrorRepr`
// (bit position = `SerialCause`/`SerialRxErrorKind` discriminant), so a const at
// the wrong position would make them silently misclassify. The expected values
// are derived from the model discriminants (single source of truth).
const CAUSE_CONSTS: &[(&str, u32)] = &[
    ("CAUSE_TX_READY", 1u32 << (SerialCause::TxReady as u32)),
    ("CAUSE_RX_DATA", 1u32 << (SerialCause::RxData as u32)),
    ("CAUSE_RX_TIMEOUT", 1u32 << (SerialCause::RxTimeout as u32)),
    ("CAUSE_LINE_ERROR", 1u32 << (SerialCause::LineError as u32)),
    ("CAUSE_MODEM_DELTA", 1u32 << (SerialCause::ModemDelta as u32)),
];
const RXERR_CONSTS: &[(&str, u32)] = &[
    ("RX_ERR_FRAMING", 1u32 << (SerialRxErrorKind::Framing as u32)),
    ("RX_ERR_PARITY", 1u32 << (SerialRxErrorKind::Parity as u32)),
    ("RX_ERR_HW_OVERRUN", 1u32 << (SerialRxErrorKind::HwOverrun as u32)),
    ("RX_ERR_SW_OVERRUN", 1u32 << (SerialRxErrorKind::SwOverrun as u32)),
    ("RX_ERR_BREAK", 1u32 << (SerialRxErrorKind::Break as u32)),
];

// The convention bit-domain masks (union of `1 << discriminant`) for the
// body-provenance analysis — every constant bit OR'd into the authored
// `CauseSet`/`RxError` must lie within these.
const CAUSE_MASK: u32 = (1 << (SerialCause::TxReady as u32))
    | (1 << (SerialCause::RxData as u32))
    | (1 << (SerialCause::RxTimeout as u32))
    | (1 << (SerialCause::LineError as u32))
    | (1 << (SerialCause::ModemDelta as u32));
const RXERR_MASK: u32 = (1 << (SerialRxErrorKind::Framing as u32))
    | (1 << (SerialRxErrorKind::Parity as u32))
    | (1 << (SerialRxErrorKind::HwOverrun as u32))
    | (1 << (SerialRxErrorKind::SwOverrun as u32))
    | (1 << (SerialRxErrorKind::Break as u32));

// Canonical variant vocab (canonical order = the `*Kind::ORDER` / declaration
// order the IR + backends index by).
const IRQ_SOURCE_VARIANTS: &[&str] = &["TxReady", "RxData", "LineError"];
const IRQ_STATE_VARIANTS: &[&str] = &["Armed", "Disarmed"];
const CONFIG_RESULT_VARIANTS: &[&str] = &["Ok", "Unsupported"];
const PORT_SCOPE_VARIANTS: &[&str] = &["Block", "Tx", "Rx"];
const READY_DIR_VARIANTS: &[&str] = &["TxSpace", "RxData"];
const DATA_BITS_VARIANTS: &[&str] = &["Five", "Six", "Seven", "Eight", "Nine"];
const PARITY_VARIANTS: &[&str] = &["None", "Even", "Odd"];
const STOP_BITS_VARIANTS: &[&str] = &["One", "Two"];

/// Recognize every serial-class driver's op-set + type bindings (T03).
pub(crate) fn resolve_serial(cx: &mut BridgeCtxt<'_>) -> Result<(), BridgeError> {
    let tcx = cx.input.tcx;

    // Trait-impl maps (driver-struct def-id -> trait def-id), one per serial
    // trait, keyed by name like `device_lifecycle::lifecycle_impls_by_driver`.
    let serial_impls = trait_impls_by_driver(tcx, SERIAL_TRAIT);
    let line_config_impls = trait_impls_by_driver(tcx, LINE_CONFIG_TRAIT);
    let interrupt_impls = trait_impls_by_driver(tcx, INTERRUPT_DRIVEN_TRAIT);
    let queue_control_impls = trait_impls_by_driver(tcx, QUEUE_CONTROL_TRAIT);

    // Serial-class drivers, as (descriptor key, driver-struct def-id).
    let targets: Vec<(ItemId, DefId)> = cx
        .model
        .device_lifecycle
        .drivers
        .keys()
        .copied()
        .filter(|item_id| {
            matches!(
                cx.model.device_lifecycle.drivers[item_id].descriptor.device_class,
                ClassTag::Serial
            )
        })
        .filter_map(|item_id| {
            let def_id = recover_source_def_id(tcx, cx.model.item_def_paths[item_id])?;
            Some((item_id, def_id))
        })
        .collect();

    // O2-hardening validation runs once per crate (the convention consts are
    // crate-global): RxError when ≥1 serial driver exists; CauseSet when ≥1 has
    // the interrupt-driven plane.
    let mut rxerr_consts_checked = false;
    let mut cause_consts_checked = false;

    for (item_id, d_def_id) in targets {
        let driver_name = {
            let name_id = cx.model.device_lifecycle.drivers[&item_id].descriptor.name;
            cx.model.strings.resolve(name_id).to_string()
        };

        // Gate: recognize a serial-class driver only when it implements the
        // base `Serial` surface (T03). Two sub-cases when base is absent:
        //   * a capability plane (`SerialRuntimeLineConfig` /
        //     `SerialInterruptDriven`) WITHOUT the base data path is an
        //     incoherent surface — reject (SER-ERR-MISSING-BASE);
        //   * NO serial trait impls at all is a bare/deferred serial tag — skip
        //     leniently (the backend keeps the deferred-class NULL-api fallback;
        //     additive, so a non-implementing serial tag still bridges).
        let serial_trait = match serial_impls.get(&d_def_id).copied() {
            Some(t) => t,
            None => {
                if line_config_impls.contains_key(&d_def_id)
                    || interrupt_impls.contains_key(&d_def_id)
                {
                    return Err(BridgeError::SerialValidation(
                        SerialDiagnostic::MissingBase {
                            driver: driver_name,
                        },
                    ));
                }
                continue;
            }
        };

        // --- T04 frontend enforcement (R07/R10): fail-loud BEFORE building the
        //     IR, so the backend never sees a rejected or partially-built serial
        //     surface. Three gates: (a) no dynamic author call to the
        //     backend-only `irq_source_set`; (b) no override of a v1-deferred
        //     member inside an in-scope trait; (c) no impl of a deferred
        //     companion trait.
        reject_deferred_traits(tcx, d_def_id, &driver_name)?;
        reject_deferred_members(tcx, d_def_id, &driver_name)?;
        reject_dynamic_irq_source_set(
            tcx,
            d_def_id,
            interrupt_impls.get(&d_def_id).copied(),
            &driver_name,
        )?;

        // O2 hardening: the inline-RxError convention consts back the base data
        // path (data_unit_read), so validate once any serial driver is seen.
        if !rxerr_consts_checked {
            validate_repr_consts(tcx, RXERR_CONSTS, "RxError")?;
            rxerr_consts_checked = true;
        }

        let d_ty = tcx.type_of(d_def_id).instantiate_identity();
        let mut types = SerialTypeBindings::default();

        // --- base Serial (C04) ---
        let base = resolve_base(cx, serial_trait, d_ty, &driver_name, &mut types)?;

        // O2 body-provenance: the inline RxError bitset built in data_unit_read
        // must be a pure OR of convention-domain bits (the backend decodes it
        // against the pinned RxErrorRepr).
        if let Some(m) = op_impl_method_def_id(tcx, serial_trait, d_ty, "data_unit_read") {
            verify_bitset_provenance(tcx, m, "RxError", "data_unit_read", RXERR_MASK)?;
        }

        // --- SerialRuntimeLineConfig (C03), optional ---
        let line_config = match line_config_impls.get(&d_def_id).copied() {
            Some(t) => Some(resolve_line_config(cx, t, d_ty, &driver_name, &mut types)?),
            None => None,
        };

        // --- SerialInterruptDriven (C01), optional ---
        let interrupt_driven = match interrupt_impls.get(&d_def_id).copied() {
            Some(t) => {
                // O2 hardening: the CauseSet convention consts back
                // irq_cause_decode — validate once the interrupt plane is present.
                if !cause_consts_checked {
                    validate_repr_consts(tcx, CAUSE_CONSTS, "CauseSet")?;
                    cause_consts_checked = true;
                }
                // O2 body-provenance: the CauseSet bitset built in
                // irq_cause_decode (per driver) must be a pure OR of convention
                // bits.
                if let Some(m) = op_impl_method_def_id(tcx, t, d_ty, "irq_cause_decode") {
                    verify_bitset_provenance(tcx, m, "CauseSet", "irq_cause_decode", CAUSE_MASK)?;
                }
                Some(resolve_interrupt_driven(cx, t, d_ty, &driver_name, &mut types)?)
            }
            None => None,
        };

        // --- SerialQueueControl (C05; T09 INCLUDE), optional plane ---
        let queue_control = match queue_control_impls.get(&d_def_id).copied() {
            Some(t) => Some(resolve_queue_control(cx, t, d_ty)?),
            None => None,
        };

        let driver = DriverSerial {
            base,
            line_config,
            interrupt_driven,
            queue_control,
            types,
        };
        cx.model.serial.drivers.insert(item_id, driver);
    }
    Ok(())
}

/// Build the `impl <trait_name> for <Driver>` map, keyed by the impl's Self ADT
/// def-id (mirrors `device_lifecycle::lifecycle_impls_by_driver`).
fn trait_impls_by_driver(tcx: TyCtxt<'_>, trait_name: &str) -> std::collections::HashMap<DefId, DefId> {
    let mut by_driver = std::collections::HashMap::new();
    for local_def_id in tcx.hir_crate_items(()).definitions() {
        let impl_def_id = local_def_id.to_def_id();
        if !matches!(tcx.def_kind(impl_def_id), DefKind::Impl { of_trait: true }) {
            continue;
        }
        let trait_ref = tcx.impl_trait_ref(impl_def_id).skip_binder();
        if tcx.item_name(trait_ref.def_id).as_str() != trait_name {
            continue;
        }
        if let Some(adt) = trait_ref.self_ty().ty_adt_def() {
            by_driver.insert(adt.did(), trait_ref.def_id);
        }
    }
    by_driver
}

/// Every impl block (trait + inherent) whose Self ADT is `driver_def_id`, as
/// `(trait def-id if a trait impl, impl block def-id)`. The single-scan basis
/// for the T04 enforcement gates.
fn driver_impl_blocks(tcx: TyCtxt<'_>, driver_def_id: DefId) -> Vec<(Option<DefId>, DefId)> {
    let mut blocks = Vec::new();
    for local_def_id in tcx.hir_crate_items(()).definitions() {
        let impl_def_id = local_def_id.to_def_id();
        let of_trait = match tcx.def_kind(impl_def_id) {
            DefKind::Impl { of_trait } => of_trait,
            _ => continue,
        };
        let self_ty = tcx.type_of(impl_def_id).instantiate_identity();
        let Some(adt) = self_ty.ty_adt_def() else {
            continue;
        };
        if adt.did() != driver_def_id {
            continue;
        }
        let trait_def_id = if of_trait {
            // Guarded by `of_trait`, so the impl has a trait ref (mirrors
            // `trait_impls_by_driver`).
            Some(tcx.impl_trait_ref(impl_def_id).skip_binder().def_id)
        } else {
            None
        };
        blocks.push((trait_def_id, impl_def_id));
    }
    blocks
}

/// T04(c) — reject any impl of a v1-deferred companion trait
/// (`SerialModemControl`/`SerialConsole`/`SerialBulkTransfer`) on a serial-class
/// driver (R10, SER-ERR-DEFERRED-TRAIT).
fn reject_deferred_traits(
    tcx: TyCtxt<'_>,
    driver_def_id: DefId,
    driver_name: &str,
) -> Result<(), BridgeError> {
    for (trait_def_id, _impl_def_id) in driver_impl_blocks(tcx, driver_def_id) {
        let Some(trait_def_id) = trait_def_id else {
            continue;
        };
        let trait_name = tcx.item_name(trait_def_id);
        if DEFERRED_TRAITS.contains(&trait_name.as_str()) {
            return Err(BridgeError::SerialValidation(
                SerialDiagnostic::DeferredTrait {
                    driver: driver_name.to_string(),
                    trait_name: trait_name.to_string(),
                },
            ));
        }
    }
    Ok(())
}

/// T04(b) — reject any author OVERRIDE of a v1-deferred member inside an
/// in-scope trait (R10, SER-ERR-DEFERRED-MEMBER). The deferred members are
/// declared with default bodies on the author surface; an impl that *defines*
/// one (i.e. it appears among the impl block's own associated items) is
/// overriding a not-yet-built capability.
fn reject_deferred_members(
    tcx: TyCtxt<'_>,
    driver_def_id: DefId,
    driver_name: &str,
) -> Result<(), BridgeError> {
    for (trait_def_id, impl_def_id) in driver_impl_blocks(tcx, driver_def_id) {
        let Some(trait_def_id) = trait_def_id else {
            continue;
        };
        let trait_name = tcx.item_name(trait_def_id);
        let deferred: &[&str] = match trait_name.as_str() {
            LINE_CONFIG_TRAIT => LINE_CONFIG_DEFERRED_MEMBERS,
            QUEUE_CONTROL_TRAIT => QUEUE_CONTROL_DEFERRED_MEMBERS,
            _ => continue,
        };
        for item in tcx.associated_items(impl_def_id).in_definition_order() {
            if !item.is_fn() {
                continue;
            }
            let member = item.name();
            if deferred.contains(&member.as_str()) {
                return Err(BridgeError::SerialValidation(
                    SerialDiagnostic::DeferredMember {
                        driver: driver_name.to_string(),
                        trait_name: trait_name.to_string(),
                        member: member.to_string(),
                    },
                ));
            }
        }
    }
    Ok(())
}

/// A read-only MIR visitor that flags any `irq_source_set` reference: an
/// `Operand::Constant` of `FnDef` type that (statically) resolves to the
/// `irq_source_set` trait method. This catches BOTH a direct call (the `Call`
/// terminator's `func` operand) AND a fn-pointer reification / pass-as-value
/// (`let f = <Self>::irq_source_set;` / `trampoline(<Self>::irq_source_set, …)`
/// — the `FnDef` ZST appears as an operand before the cast), closing the
/// FnDef-only Call-terminator hole (Codex T04 review).
struct IrqSourceSetRefFinder<'tcx> {
    tcx: TyCtxt<'tcx>,
    /// The `irq_source_set` trait-method def-id.
    iss_trait_method: DefId,
    /// Set once a reference is seen.
    found: bool,
    /// Closure bodies CONSTRUCTED in this body, collected for the BFS to recurse
    /// into (catches a closure coerced straight to a fn pointer with no
    /// `TyKind::Closure` local — Codex T04 round-2).
    closures: Vec<DefId>,
}

impl<'tcx> MirVisitor<'tcx> for IrqSourceSetRefFinder<'tcx> {
    fn visit_operand(&mut self, operand: &mir::Operand<'tcx>, location: mir::Location) {
        if let mir::Operand::Constant(constant) = operand {
            if let ty::TyKind::FnDef(callee, _) = constant.const_.ty().kind() {
                // Concrete-Self call/reference lands on the impl method (its
                // `trait_item_def_id` points back to the trait method); a direct
                // trait-method reference IS it.
                let is_iss = *callee == self.iss_trait_method
                    || self
                        .tcx
                        .opt_associated_item(*callee)
                        .and_then(|ai| ai.trait_item_def_id())
                        == Some(self.iss_trait_method);
                if is_iss {
                    self.found = true;
                }
            }
        }
        self.super_operand(operand, location);
    }

    fn visit_rvalue(&mut self, rvalue: &mir::Rvalue<'tcx>, location: mir::Location) {
        // A closure (capturing or not) is built via an `Aggregate(Closure(..))`;
        // record its body so the BFS recurses into it. (Coroutines/async are
        // rejected upstream at stage-3, so they cannot reach here.)
        if let mir::Rvalue::Aggregate(kind, _) = rvalue {
            if let mir::AggregateKind::Closure(closure_def_id, _) = **kind {
                self.closures.push(closure_def_id);
            }
        }
        self.super_rvalue(rvalue, location);
    }
}

/// Does the body tree rooted at `root` (the method body + every closure it
/// constructs, transitively) reference `iss_trait_method`? A bounded BFS:
/// closures are discovered as `TyKind::Closure` local-decl types (mirrors
/// `synth::closure`) and recursed into, so a closure/generic trampoline that
/// hides the call cannot evade the check (Codex T04 review). mir-opt-level is 0,
/// so no inlining perturbs the body.
fn body_tree_references_irq_source_set(
    tcx: TyCtxt<'_>,
    root: DefId,
    iss_trait_method: DefId,
) -> bool {
    let mut seen: std::collections::HashSet<DefId> = std::collections::HashSet::new();
    let mut work = vec![root];
    while let Some(def_id) = work.pop() {
        if !seen.insert(def_id) {
            continue;
        }
        if !tcx.is_mir_available(def_id) {
            continue;
        }
        let body = tcx.optimized_mir(def_id);
        let mut finder = IrqSourceSetRefFinder {
            tcx,
            iss_trait_method,
            found: false,
            closures: Vec::new(),
        };
        finder.visit_body(body);
        if finder.found {
            return true;
        }
        // Recurse into closures this body constructs — discovered both by their
        // `Aggregate(Closure(..))` construction (the robust path) and by any
        // `TyKind::Closure` local-decl (a backstop).
        work.extend(finder.closures);
        for local in body.local_decls.iter() {
            if let ty::TyKind::Closure(closure_def_id, _) = local.ty.kind() {
                work.push(*closure_def_id);
            }
        }
    }
    false
}

/// T04(a) — reject a dynamic author reference to `irq_source_set` (R07/D03/I01,
/// SER-ERR-DYNAMIC-IRQ-SET). `irq_source_set` is backend-called only: the
/// compiler emits the calls at the per-OS scatter sites, so any author body that
/// calls — or takes a fn-pointer to, or routes through a closure/generic
/// trampoline — breaks the portable placement contract. Scans the body tree of
/// every method the driver defines (trait + inherent impls) EXCEPT
/// `irq_source_set`'s own body.
///
/// Rule (Codex T04 review): ONLY `irq_source_set`'s own body may reference
/// `irq_source_set`; every other driver method (and any closure it builds) must
/// not. The scan covers direct calls, fn-pointer reifications, and closures
/// (capturing or coerced-to-fn-pointer), transitively.
///
/// This is a guard-rail against the obvious misuse, NOT a sandbox. R07 cannot be
/// airtight: an author owns the mask bits and could replicate the IMSC write
/// directly in a base op — a write the compiler cannot distinguish from any
/// other MMIO. The same boundary covers a `const FN_TABLE: fn(..) = …;` const-
/// promoted fn-pointer table (the original `FnDef` is hidden behind a `FnPtr`
/// const initializer this scan does not evaluate). Both are documented as the
/// guard-rail boundary; R07 catches *reaching for the named op*, the real
/// portability hazard.
///
/// (A closure COERCED to a fn pointer is independently rejected EARLIER by the
/// bridge's substantive-HRTB restriction — `extern "rust-call" fn(..)` is
/// unsupported DSL — so that evasion never reaches T04; a closure CALLED
/// directly is caught here via the `Aggregate(Closure)` discovery.)
fn reject_dynamic_irq_source_set(
    tcx: TyCtxt<'_>,
    driver_def_id: DefId,
    interrupt_trait: Option<DefId>,
    driver_name: &str,
) -> Result<(), BridgeError> {
    let Some(interrupt_trait) = interrupt_trait else {
        return Ok(());
    };
    let Some(iss_trait_method) = trait_method_def_id(tcx, interrupt_trait, "irq_source_set") else {
        return Ok(());
    };
    for (_trait_def_id, impl_def_id) in driver_impl_blocks(tcx, driver_def_id) {
        for item in tcx.associated_items(impl_def_id).in_definition_order() {
            if !item.is_fn() {
                continue;
            }
            // Skip the `irq_source_set` body itself (it legitimately commits the
            // mask; only OTHER methods may not reference it).
            if item.trait_item_def_id() == Some(iss_trait_method) {
                continue;
            }
            if body_tree_references_irq_source_set(tcx, item.def_id, iss_trait_method) {
                return Err(BridgeError::SerialValidation(
                    SerialDiagnostic::DynamicIrqSourceSet {
                        driver: driver_name.to_string(),
                        caller: tcx.item_name(item.def_id).to_string(),
                    },
                ));
            }
        }
    }
    Ok(())
}

/// Const-eval a module-level `const NAME: u32` to its value. `None` for a const
/// that is not declared `u32` (so a same-named const of another type is neither
/// truncated nor false-rejected — Codex hardening nit), or non-scalar /
/// un-evaluable.
fn eval_u32_const(tcx: TyCtxt<'_>, def_id: DefId) -> Option<u32> {
    if tcx.type_of(def_id).instantiate_identity() != tcx.types.u32 {
        return None;
    }
    let cv = tcx.const_eval_poly(def_id).ok()?;
    match cv {
        mir::ConstValue::Scalar(rustc_middle::mir::interpret::Scalar::Int(si)) => {
            u32::try_from(si.try_to_bits(si.size()).ok()?).ok()
        }
        _ => None,
    }
}

/// O2 hardening — validate the author bit-position convention consts against the
/// pinned representation (SER-ERR-REPR-CONST). Scans every crate-level `u32`
/// `const` whose name is in `expected`; a PRESENT const whose value ≠
/// `1 << discriminant` is rejected (the backend would misclassify).
///
/// Boundary (Codex review): this NARROWS the silent-misclassification hole to the
/// named-const convention; it does NOT close it. Absent consts are skipped (the
/// canonical surface declares them, so any driver built against it is covered,
/// but presence is not required); and it does not analyze the
/// `irq_cause_decode`/`data_unit_read` BODIES, so an author who builds the bitset
/// with raw literals or differently-named consts bypasses it — the same
/// guard-rail-vs-sandbox boundary as the T04 R07 check. Name matching is by
/// const name (a reserved convention, consistent with the construct's name-based
/// recognition throughout).
fn validate_repr_consts(
    tcx: TyCtxt<'_>,
    expected: &[(&str, u32)],
    repr: &str,
) -> Result<(), BridgeError> {
    for local_def_id in tcx.hir_crate_items(()).definitions() {
        let def_id = local_def_id.to_def_id();
        if !matches!(tcx.def_kind(def_id), DefKind::Const) {
            continue;
        }
        let name = tcx.item_name(def_id);
        let Some((_, want)) = expected.iter().find(|(n, _)| *n == name.as_str()) else {
            continue;
        };
        let Some(found) = eval_u32_const(tcx, def_id) else {
            continue;
        };
        if found != *want {
            return Err(BridgeError::SerialValidation(
                SerialDiagnostic::ReprConstMismatch {
                    repr: repr.to_string(),
                    const_name: name.to_string(),
                    found,
                    expected: *want,
                },
            ));
        }
    }
    Ok(())
}

/// Resolve the concrete impl-method `DefId` (the one carrying MIR) for a trait
/// method on `d_ty`, for the body-provenance analysis.
fn op_impl_method_def_id<'tcx>(
    tcx: TyCtxt<'tcx>,
    trait_def_id: DefId,
    d_ty: Ty<'tcx>,
    method_name: &str,
) -> Option<DefId> {
    let m = trait_method_def_id(tcx, trait_def_id, method_name)?;
    let args = tcx.mk_args(&[d_ty.into()]);
    let resolved =
        ty::Instance::try_resolve(tcx, ty::TypingEnv::fully_monomorphized(), m, args).ok()??;
    Some(resolved.def_id())
}

/// Evaluate a `u32` constant MIR operand to its value. `None` for a non-`u32` or
/// un-evaluable const (so a non-constant contribution is detected, not silently
/// treated as zero).
fn const_operand_u32<'tcx>(tcx: TyCtxt<'tcx>, konst: mir::Const<'tcx>) -> Option<u32> {
    if konst.ty() != tcx.types.u32 {
        return None;
    }
    let bits = konst.try_eval_bits(tcx, ty::TypingEnv::fully_monomorphized())?;
    u32::try_from(bits).ok()
}

/// Locals whose value can be mutated through an alias the const-bit slice cannot
/// follow: a `&mut` borrow, or ANY raw pointer of the local (a `*const` can be
/// cast to `*mut`, Codex round-2). A traced bits local in this set is foreign; a
/// repr local in this set is rejected outright.
fn aliased_locals(body: &mir::Body<'_>) -> std::collections::HashSet<mir::Local> {
    let mut set = std::collections::HashSet::new();
    for bb in body.basic_blocks.iter() {
        for stmt in &bb.statements {
            let mir::StatementKind::Assign(box (_, rvalue)) = &stmt.kind else {
                continue;
            };
            match rvalue {
                mir::Rvalue::Ref(_, mir::BorrowKind::Mut { .. }, place)
                | mir::Rvalue::RawPtr(_, place) => {
                    set.insert(place.local);
                }
                _ => {}
            }
        }
    }
    set
}

/// The type of a place (handles projections like `c.bits` / `*p`).
fn place_ty<'tcx>(tcx: TyCtxt<'tcx>, body: &mir::Body<'tcx>, place: &mir::Place<'tcx>) -> Ty<'tcx> {
    place.ty(&body.local_decls, tcx).ty
}

/// Whether `ty` is the repr ADT (CauseSet / RxError) by name.
fn is_repr_ty(tcx: TyCtxt<'_>, ty: Ty<'_>, repr: &str) -> bool {
    ty.ty_adt_def()
        .map(|a| tcx.item_name(a.did()).as_str() == repr)
        .unwrap_or(false)
}

/// The callee `DefId` of a `Call` whose func is a direct `FnDef` constant.
fn call_fndef(func: &mir::Operand<'_>) -> Option<DefId> {
    let mir::Operand::Constant(c) = func else {
        return None;
    };
    let ty::TyKind::FnDef(def_id, _) = c.const_.ty().kind() else {
        return None;
    };
    Some(*def_id)
}

/// Verify one bits-operand: a borrow-aware backward const-bit slice. Reject a
/// constant bit outside `mask`, or a foreign (non-constant / mutably-aliased /
/// unmodeled) contribution.
fn verify_bits_operand<'tcx>(
    tcx: TyCtxt<'tcx>,
    body: &'tcx mir::Body<'tcx>,
    arg: &mir::Operand<'tcx>,
    mut_borrowed: &std::collections::HashSet<mir::Local>,
    mask: u32,
    repr: &str,
    op: &str,
) -> Result<(), BridgeError> {
    let mut state = BitsetSlice {
        collected: 0,
        foreign: false,
        work: Vec::new(),
    };
    let mut seen: std::collections::HashSet<mir::Local> = std::collections::HashSet::new();
    fold_bitset_operand(tcx, arg, &mut state);
    while let Some(local) = state.work.pop() {
        if !seen.insert(local) {
            continue;
        }
        if mut_borrowed.contains(&local) {
            // Value can change through an alias the slice cannot follow.
            state.foreign = true;
            continue;
        }
        for bb in body.basic_blocks.iter() {
            for stmt in &bb.statements {
                let mir::StatementKind::Assign(box (place, rvalue)) = &stmt.kind else {
                    continue;
                };
                if place.as_local() != Some(local) {
                    continue;
                }
                match rvalue {
                    mir::Rvalue::Use(o) => fold_bitset_operand(tcx, o, &mut state),
                    mir::Rvalue::BinaryOp(mir::BinOp::BitOr, box (a, b)) => {
                        fold_bitset_operand(tcx, a, &mut state);
                        fold_bitset_operand(tcx, b, &mut state);
                    }
                    // Any other rvalue feeding the bitset (BitAnd of a device read,
                    // a call result, a cast, …) is a non-convention contribution.
                    _ => state.foreign = true,
                }
            }
        }
    }
    let bitset_err = |detail: String| {
        BridgeError::SerialValidation(SerialDiagnostic::BitsetProvenance {
            repr: repr.to_string(),
            op: op.to_string(),
            detail,
        })
    };
    if state.foreign {
        return Err(bitset_err(
            "a non-constant or computed value (e.g. a masked device read or a mutably-aliased \
             local) reaches the bitset — it must be a pure OR of convention constants"
                .to_string(),
        ));
    }
    let extra = state.collected & !mask;
    if extra != 0 {
        return Err(bitset_err(format!(
            "constant bit(s) {extra:#x} fall outside the pinned {repr} bit-domain {mask:#x}"
        )));
    }
    Ok(())
}

/// Accumulator for the backward const-bit slice (see [`verify_bitset_provenance`]).
struct BitsetSlice {
    /// The union of constant bits proven to flow into the bitset.
    collected: u32,
    /// Set when a contribution is not a recognizable convention constant
    /// (a non-`u32`/un-evaluable const, a projected place, or an unmodeled rvalue
    /// — e.g. a masked device read).
    foreign: bool,
    /// Locals still to trace.
    work: Vec<mir::Local>,
}

/// Fold one OR-operand into the slice: a `u32` constant contributes its bits; a
/// bare local is queued for tracing; anything else is a foreign contribution.
fn fold_bitset_operand<'tcx>(
    tcx: TyCtxt<'tcx>,
    op: &mir::Operand<'tcx>,
    state: &mut BitsetSlice,
) {
    match op {
        mir::Operand::Constant(c) => match const_operand_u32(tcx, c.const_) {
            Some(v) => state.collected |= v,
            None => state.foreign = true,
        },
        mir::Operand::Copy(p) | mir::Operand::Move(p) => match p.as_local() {
            Some(l) => state.work.push(l),
            None => state.foreign = true,
        },
        // Any other operand form (e.g. RuntimeChecks) is not a convention const.
        _ => state.foreign = true,
    }
}

/// O2 body-provenance hardening — verify the authored `CauseSet`/`RxError` value
/// (built in `irq_cause_decode`/`data_unit_read`) is constructed ONLY from pinned
/// convention-domain CONSTANT bits, so the backend (which decodes it against the
/// pinned `CauseSetRepr`/`RxErrorRepr`) cannot silently misclassify.
///
/// SOUND scan (Codex-hardened): EVERY site that produces a `repr` value is
/// verified or rejected —
///   * `<repr>::from_bits(arg)` call → the `arg` is sliced (must be a pure OR of
///     convention-domain constant bits, borrow-aware);
///   * `<repr>::empty()` call → 0, OK;
///   * a `repr` struct-literal `Aggregate` → its `bits` field is sliced (closes
///     the same-module `repr { bits: 1<<7 }` literal bypass — `include!` puts the
///     driver in the surface's module, so the private field IS reachable);
///   * a copy/move of a `repr` value from a bare local → OK (its source site is
///     itself scanned);
///   * ANY OTHER `repr`-producing call (a helper returning `repr`) or rvalue (a
///     field load, a cast) → REJECTED (closes the helper-delegation bypass).
///
/// The backward const-bit slice is borrow-aware (a mutably-aliased accumulator is
/// foreign) and covers every producing site (not just the first), closing the
/// false-accept holes of a single-`from_bits` slice. mir-opt-level is 0 (no
/// inlining), so the constructor calls survive.
///
/// Codex-converged over 4 adversarial rounds → **SOUND-WITHIN-DSL**: no
/// DSL-admissible body can return a `repr` carrying a bit outside `mask` while
/// passing. Every residual evasion requires a construct the bridge rejects
/// elsewhere (transmute, unions, FFI/extern, raw-pointer/metadata tricks). The
/// holes closed across the rounds: fn-only single-call slicing → all
/// constructors; bare-local keying → place-TYPE keying (projected dests);
/// projected interior writes at any nesting (`c.bits=` / `d.errors.bits=`);
/// `&mut`/raw aliasing of the bits accumulator AND of the repr value; helper
/// delegation; same-module struct literals.
///
/// Irreducible residual (documented): the device→semantic mapping — which device
/// register bit the author tests before OR-ing a given convention bit (does
/// `MIS_RX` really mean rx_data?) — is author knowledge no static check can
/// recover. This pass guarantees the OUTPUT bitset carries only convention-domain
/// bits, not that each bit was set for the right device reason. It is a WHITELIST
/// (accept only canonical construction; reject all else), so it also rejects
/// valid-but-noncanonical bodies (e.g. `from_bits(raw & 0x1f)` masked
/// construction) — acceptable incompleteness for a verifiable DSL subset.
fn verify_bitset_provenance(
    tcx: TyCtxt<'_>,
    method_def_id: DefId,
    repr: &str,
    op: &str,
    mask: u32,
) -> Result<(), BridgeError> {
    if !tcx.is_mir_available(method_def_id) {
        return Ok(());
    }
    let body = tcx.optimized_mir(method_def_id);
    let aliased = aliased_locals(body);
    let unverifiable = |detail: String| {
        BridgeError::SerialValidation(SerialDiagnostic::BitsetProvenance {
            repr: repr.to_string(),
            op: op.to_string(),
            detail,
        })
    };
    let is_repr = |ty: Ty<'_>| is_repr_ty(tcx, ty, repr);

    // Reject any `&mut`/raw alias of a `repr`-typed local: its value could be
    // mutated through the alias unseen, so it is unverifiable (Codex round-2).
    for &local in &aliased {
        if is_repr(body.local_decls[local].ty) {
            return Err(unverifiable(format!(
                "a `{repr}` value is mutably aliased (`&mut`/raw ptr) — its bits cannot be tracked"
            )));
        }
    }

    for bb in body.basic_blocks.iter() {
        // (A) Calls producing a `repr` value (destination place TYPE is `repr` —
        // catches a projected dest like `d.error = from_bits(..)`).
        if let mir::TerminatorKind::Call {
            func,
            args,
            destination,
            ..
        } = &bb.terminator().kind
        {
            if is_repr(place_ty(tcx, body, destination)) {
                let name = call_fndef(func).map(|d| tcx.item_name(d).to_string());
                match name.as_deref() {
                    Some("from_bits") => {
                        let arg = args.first().ok_or_else(|| {
                            unverifiable("from_bits called with no argument".to_string())
                        })?;
                        verify_bits_operand(tcx, body, &arg.node, &aliased, mask, repr, op)?;
                    }
                    Some("empty") => {}
                    other => {
                        return Err(unverifiable(format!(
                            "a `{}` value is produced by an unverifiable call `{}` (not \
                             from_bits/empty) — a helper must not build the bitset",
                            repr,
                            other.unwrap_or("<indirect>")
                        )));
                    }
                }
            }
        }
        // (B) Statements writing a `repr` value or its interior.
        for stmt in &bb.statements {
            let mir::StatementKind::Assign(box (place, rvalue)) = &stmt.kind else {
                continue;
            };
            // An interior write into a `repr` at ANY nesting depth — a projected
            // destination whose final type is a sub-component but SOME projection
            // prefix is `repr`-typed (e.g. `c.bits = …`, or `d.errors.bits = …`
            // where `d.errors` is the repr) — is a NON-CANONICAL mutation. Reject
            // (Codex round-3: check every prefix, not just the base local).
            if !is_repr(place_ty(tcx, body, place))
                && place
                    .iter_projections()
                    .any(|(base, _)| is_repr(base.ty(&body.local_decls, tcx).ty))
            {
                return Err(unverifiable(format!(
                    "a `{repr}` value's interior is mutated in place (only construction via \
                     from_bits/empty/a struct literal is verifiable)"
                )));
            }
            // A whole-`repr` assignment (bare local OR `*p` deref → place type repr).
            if !is_repr(place_ty(tcx, body, place)) {
                continue;
            }
            match rvalue {
                // A struct literal `repr { bits }` — verify the bits field(s).
                mir::Rvalue::Aggregate(kind, fields) => {
                    let mir::AggregateKind::Adt(adt_did, ..) = **kind else {
                        return Err(unverifiable(format!(
                            "a `{repr}` value is produced by a non-struct aggregate"
                        )));
                    };
                    if tcx.item_name(adt_did).as_str() != repr {
                        return Err(unverifiable(format!(
                            "a `{repr}`-typed place is built from a different aggregate type"
                        )));
                    }
                    for f in fields.iter() {
                        verify_bits_operand(tcx, body, f, &aliased, mask, repr, op)?;
                    }
                }
                // A copy/move of a `repr` value from a BARE local — its producing
                // site is itself scanned (the methods take no `repr` inputs and
                // interior mutation is rejected above, so a bare `repr` local's
                // source is always a scanned construction site).
                mir::Rvalue::Use(mir::Operand::Copy(p) | mir::Operand::Move(p))
                    if p.as_local().is_some() => {}
                // A `repr` value from any other source (a projected/field load, a
                // cast, a constant) is unverifiable.
                _ => {
                    return Err(unverifiable(format!(
                        "a `{repr}` value is produced by an unverifiable rvalue (not from_bits/\
                         empty/a checked struct literal/a copy of a checked local)"
                    )));
                }
            }
        }
    }
    Ok(())
}

/// Resolve one trait method by name → its force-materialized `InstanceRef`.
/// Required serial methods always resolve to the impl (no defaults).
fn resolve_method<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    trait_def_id: DefId,
    d_ty: Ty<'tcx>,
    method_name: &str,
) -> Result<Option<InstanceRef>, BridgeError> {
    let tcx = cx.input.tcx;
    let Some(method_def_id) = trait_method_def_id(tcx, trait_def_id, method_name) else {
        return Ok(None);
    };
    let args = tcx.mk_args(&[d_ty.into()]);
    let resolved = ty::Instance::try_resolve(
        tcx,
        ty::TypingEnv::fully_monomorphized(),
        method_def_id,
        args,
    );
    let Ok(Some(instance)) = resolved else {
        return Ok(None);
    };
    let iref = instance_ref_for_function_provenance(cx, instance)?;
    Ok(Some(iref))
}

/// The trait-method def-id for `method_name` on `trait_def_id`, if present.
fn trait_method_def_id(tcx: TyCtxt<'_>, trait_def_id: DefId, method_name: &str) -> Option<DefId> {
    tcx.associated_items(trait_def_id)
        .in_definition_order()
        .find(|item| item.is_fn() && item.name().as_str() == method_name)
        .map(|item| item.def_id)
}

fn resolve_base<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    serial_trait: DefId,
    d_ty: Ty<'tcx>,
    driver_name: &str,
    types: &mut SerialTypeBindings,
) -> Result<SerialBaseOps, BridgeError> {
    let tcx = cx.input.tcx;
    // Capture PortScope / ReadyDir from the method signatures BEFORE the
    // mutable-cx materialization calls.
    if let Some(m) = trait_method_def_id(tcx, serial_trait, "port_service_set") {
        let scope_ty = nth_input_ty(tcx, m, 1);
        types.port_scope = Some(capture_variants(
            scope_ty,
            PORT_SCOPE_VARIANTS,
            driver_name,
            "PortScope",
        )?);
    }
    if let Some(m) = trait_method_def_id(tcx, serial_trait, "data_ready_query") {
        let dir_ty = nth_input_ty(tcx, m, 1);
        types.ready_dir = Some(capture_variants(
            dir_ty,
            READY_DIR_VARIANTS,
            driver_name,
            "ReadyDir",
        )?);
    }
    let port_service_set = resolve_method(cx, serial_trait, d_ty, "port_service_set")?;
    let data_ready_query = resolve_method(cx, serial_trait, d_ty, "data_ready_query")?;
    let data_unit_write = resolve_method(cx, serial_trait, d_ty, "data_unit_write")?;
    let data_unit_read = resolve_method(cx, serial_trait, d_ty, "data_unit_read")?;
    Ok(SerialBaseOps {
        port_service_set,
        data_ready_query,
        data_unit_write,
        data_unit_read,
        // The pinned RxError representation (O2-analog for the data path): a
        // compiler convention the author `RX_ERR_*` consts match, mirroring how
        // `cause_set` is pinned in `resolve_interrupt_driven`.
        rx_error: regseq_model::RxErrorRepr::default(),
    })
}

fn resolve_line_config<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    line_config_trait: DefId,
    d_ty: Ty<'tcx>,
    driver_name: &str,
    types: &mut SerialTypeBindings,
) -> Result<SerialLineConfigOps, BridgeError> {
    let tcx = cx.input.tcx;
    if let Some(m) = trait_method_def_id(tcx, line_config_trait, "line_format_apply") {
        // param[1] = LineFormat (struct): recurse into its enum fields.
        let line_format_ty = nth_input_ty(tcx, m, 1);
        capture_line_format_fields(tcx, line_format_ty, driver_name, types)?;
        // return = ConfigResult.
        let result_ty = output_ty(tcx, m);
        types.config_result = Some(capture_variants(
            result_ty,
            CONFIG_RESULT_VARIANTS,
            driver_name,
            "ConfigResult",
        )?);
    }
    let line_format_apply = resolve_method(cx, line_config_trait, d_ty, "line_format_apply")?;
    Ok(SerialLineConfigOps { line_format_apply })
}

fn resolve_interrupt_driven<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    interrupt_trait: DefId,
    d_ty: Ty<'tcx>,
    driver_name: &str,
    types: &mut SerialTypeBindings,
) -> Result<SerialInterruptOps, BridgeError> {
    let tcx = cx.input.tcx;
    if let Some(m) = trait_method_def_id(tcx, interrupt_trait, "irq_source_set") {
        let source_ty = nth_input_ty(tcx, m, 1);
        types.irq_source = Some(capture_variants(
            source_ty,
            IRQ_SOURCE_VARIANTS,
            driver_name,
            "SerialIrqSource",
        )?);
        let state_ty = nth_input_ty(tcx, m, 2);
        types.irq_state = Some(capture_variants(
            state_ty,
            IRQ_STATE_VARIANTS,
            driver_name,
            "IrqSourceState",
        )?);
    }
    let irq_source_set = resolve_method(cx, interrupt_trait, d_ty, "irq_source_set")?;
    let irq_cause_decode = resolve_method(cx, interrupt_trait, d_ty, "irq_cause_decode")?;
    Ok(SerialInterruptOps {
        irq_source_set,
        irq_cause_decode,
        cause_set: regseq_model::CauseSetRepr::default(),
    })
}

fn resolve_queue_control<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    queue_control_trait: DefId,
    d_ty: Ty<'tcx>,
) -> Result<SerialQueueControlOps, BridgeError> {
    // `tx_drain_query(&self) -> bool` carries no support enum — nothing to
    // capture, just force-materialize the body.
    let tx_drain_query = resolve_method(cx, queue_control_trait, d_ty, "tx_drain_query")?;
    Ok(SerialQueueControlOps { tx_drain_query })
}

/// The `n`-th input type of a function (0 = receiver), under identity args.
fn nth_input_ty<'tcx>(tcx: TyCtxt<'tcx>, fn_def_id: DefId, n: usize) -> Ty<'tcx> {
    let sig = tcx.fn_sig(fn_def_id).instantiate_identity();
    sig.skip_binder().inputs()[n]
}

/// The output type of a function, under identity args.
fn output_ty<'tcx>(tcx: TyCtxt<'tcx>, fn_def_id: DefId) -> Ty<'tcx> {
    let sig = tcx.fn_sig(fn_def_id).instantiate_identity();
    sig.skip_binder().output()
}

/// Capture a support enum's variant map: for each canonical name (in order),
/// the authored variant index. Validates the variant set against the closed
/// vocab (T04: a missing/renamed variant is a clean diagnostic, not a backend
/// surprise).
fn capture_variants(
    enum_ty: Ty<'_>,
    expected: &[&str],
    driver: &str,
    type_name: &str,
) -> Result<EnumBinding, BridgeError> {
    let Some(adt) = enum_ty.ty_adt_def() else {
        return Err(BridgeError::SerialValidation(SerialDiagnostic::TypeMismatch {
            driver: driver.to_string(),
            type_name: type_name.to_string(),
            detail: "expected an enum type".to_string(),
        }));
    };
    if !adt.is_enum() {
        return Err(BridgeError::SerialValidation(SerialDiagnostic::TypeMismatch {
            driver: driver.to_string(),
            type_name: type_name.to_string(),
            detail: "expected an enum type".to_string(),
        }));
    }
    let mut variants = Vec::with_capacity(expected.len());
    for name in expected {
        let found = adt
            .variants()
            .iter_enumerated()
            .find(|(_, v)| v.name.as_str() == *name)
            .map(|(idx, _)| idx);
        match found {
            Some(idx) => variants.push(VariantIdx::from_u32(idx.as_u32())),
            None => {
                return Err(BridgeError::SerialValidation(
                    SerialDiagnostic::BadVariants {
                        driver: driver.to_string(),
                        type_name: type_name.to_string(),
                        detail: format!("missing required variant `{name}`"),
                    },
                ))
            }
        }
    }
    Ok(EnumBinding { variants })
}

/// Recurse into `LineFormat`'s enum-typed fields (`data_bits`, `parity`,
/// `stop_bits`) and capture each variant map. `baud: u32` is skipped.
fn capture_line_format_fields(
    tcx: TyCtxt<'_>,
    line_format_ty: Ty<'_>,
    driver: &str,
    types: &mut SerialTypeBindings,
) -> Result<(), BridgeError> {
    let Some(adt) = line_format_ty.ty_adt_def() else {
        return Err(BridgeError::SerialValidation(SerialDiagnostic::TypeMismatch {
            driver: driver.to_string(),
            type_name: "LineFormat".to_string(),
            detail: "expected a struct type".to_string(),
        }));
    };
    for field in adt.all_fields() {
        let fname = field.name.as_str();
        let fty = tcx.type_of(field.did).instantiate_identity();
        match fname {
            "data_bits" => {
                types.data_bits = Some(capture_variants(
                    fty, DATA_BITS_VARIANTS, driver, "DataBits",
                )?)
            }
            "parity" => {
                types.parity =
                    Some(capture_variants(fty, PARITY_VARIANTS, driver, "Parity")?)
            }
            "stop_bits" => {
                types.stop_bits = Some(capture_variants(
                    fty, STOP_BITS_VARIANTS, driver, "StopBits",
                )?)
            }
            _ => {}
        }
    }
    Ok(())
}
