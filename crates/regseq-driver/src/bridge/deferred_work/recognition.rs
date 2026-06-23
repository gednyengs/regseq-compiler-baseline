//! Phase-4 type-driven `Work`-field recognition + `deferred_work_submit`
//! intrinsic legality (deferred-work-item-scheduling construct).
//!
//! This is the genuinely-new recognition path in the compiler: unlike every
//! other typed resource (`Mmio`/`Clock`/`Interrupt`/`PinCtrl`), which keys on a
//! name-based type test (`item_name(adt.did()) == "Clock"`) **plus** a field
//! attribute (`#[rseq::clock(id = …)]`), the `Work` resource is recognized BY
//! TYPE: it carries **no** field attribute, and recognition reads the
//! `#[rseq::abstraction("deferred_work")]` marker on the field type's resolved
//! ADT item `DefId` (mirroring `framework_type::recognise_framework_type_attr`,
//! NOT the name test). So an alias / re-export of `Work` recognizes, while an
//! unmarked local `struct Work` does not (GRILL D04).
//!
//! v1 builds only the **required single** `Work` form; `Option<Work>` /
//! `[Work; N]` / `Vec<Work>` / `&Work` are recognized whole and rejected
//! (`DWS-ERR-UNSUPPORTED-FORM`; their support is design-in, their rejection is
//! built, GRILL D01/T04). The `Work` shell's `submit()` is an author intrinsic
//! (tagged `#[rseq::intrinsic("deferred_work_submit")]`, recognized by the
//! shared intrinsic pass); the compiler-managed `init_work`/`cancel_work` are
//! synthesized downstream from the recognized field, not author-called.

use regseq_model::{
    AttrData, DriverDeferredWork, FieldIdx, IntrinsicKind, ItemId, SyntheticItemKind, WorkResource,
};
use rustc_middle::ty::{Ty, TyCtxt, TyKind};
use rustc_span::{def_id::DefId, sym, Symbol as RustcSymbol};

use super::super::framework_type::recover_source_def_id;
use super::super::{BridgeCtxt, BridgeError, DeferredWorkDiagnostic};

/// The author-facing `#[rseq::abstraction("…")]` NAME of the deferred-work
/// resource shell. Recognition keys on this marker (on the field type's ADT
/// item DefId), not on the type name `Work` (GRILL D04). Byte-identical to the
/// facade's `#[rseq::abstraction("deferred_work")]` tag.
const WORK_ABSTRACTION: &str = "deferred_work";

/// Read the `#[rseq::abstraction("…")]` tool attribute on `def_id` and return
/// its NAME, or `None` when the attribute is absent, appears more than once, or
/// is not a single string literal. Mirrors
/// `framework_type::recognise_framework_type_attr` exactly, but on the
/// `[rseq, abstraction]` attribute path (the marker every resource-shell facade
/// carries — clock/pinctrl carry it as documentation; for deferred work it is
/// the recognition AUTHORITY).
fn recognise_abstraction_attr(tcx: TyCtxt<'_>, def_id: DefId) -> Option<RustcSymbol> {
    let path = [
        RustcSymbol::intern("rseq"),
        RustcSymbol::intern("abstraction"),
    ];
    let mut attrs = tcx.get_attrs_by_path(def_id, &path);
    let attr = attrs.next()?;
    if attrs.next().is_some() {
        return None;
    }
    let list = attr.meta_item_list()?;
    let [nested] = list.as_slice() else {
        return None;
    };
    let lit = nested.lit()?;
    let rustc_ast::LitKind::Str(name, _) = lit.kind else {
        return None;
    };
    Some(name)
}

/// Is `ty` the typed `Work` resource shell? Type-driven detection (GRILL D04):
/// the field type's ADT item must carry `#[rseq::abstraction("deferred_work")]`.
/// NOT a name test — an alias/re-export of `Work` resolves to the same ADT and
/// recognizes; an unmarked local `struct Work` does not.
fn is_work_ty(tcx: TyCtxt<'_>, ty: Ty<'_>) -> bool {
    ty.ty_adt_def().is_some_and(|adt| {
        recognise_abstraction_attr(tcx, adt.did()).is_some_and(|name| name.as_str() == WORK_ABSTRACTION)
    })
}

/// The recognized declaration form of a `Work` resource field. Only `Required`
/// is built; the rest are recognized-and-rejected (GRILL D01/T04).
#[derive(Clone, Copy, PartialEq, Eq)]
enum WorkForm {
    /// `Work` — the required single form (the only built form).
    Required,
    /// `Option<Work>` — design-in (the optional form).
    Optional,
    /// `[Work; N]` — design-in (the array form).
    Array,
    /// `Vec<Work>` — design-in (the dynamic-collection form). Unlike clock,
    /// this is an extra `Adt` case beyond `Option`/`Array`/`Ref`.
    Vec,
    /// `&Work` — design-in (a borrow, never an owned resource field).
    Ref,
}

impl WorkForm {
    /// The human-readable form label for the `DWS-ERR-UNSUPPORTED-FORM`
    /// diagnostic.
    fn label(self) -> &'static str {
        match self {
            WorkForm::Required => "Work",
            WorkForm::Optional => "Option<Work>",
            WorkForm::Array => "[Work; N]",
            WorkForm::Vec => "Vec<Work>",
            WorkForm::Ref => "&Work",
        }
    }
}

/// Classify a driver-struct field type into its `Work` declaration form, or
/// `None` when the field is not a deferred-work resource at all. All recognized
/// forms are returned whole; the non-`Required` forms are rejected by
/// [`materialise_deferred_work`], never silently narrowed here. Mirrors
/// `clock::classify_clock_form`, plus the extra `Vec<Work>` `Adt` case.
fn classify_work_form<'tcx>(tcx: TyCtxt<'tcx>, ty: Ty<'tcx>) -> Option<WorkForm> {
    if is_work_ty(tcx, ty) {
        return Some(WorkForm::Required);
    }
    match ty.kind() {
        TyKind::Adt(adt, args) if tcx.get_diagnostic_name(adt.did()) == Some(sym::Option) => {
            let inner = args.types().next()?;
            is_work_ty(tcx, inner).then_some(WorkForm::Optional)
        }
        TyKind::Adt(adt, args) if tcx.get_diagnostic_name(adt.did()) == Some(sym::Vec) => {
            let inner = args.types().next()?;
            is_work_ty(tcx, inner).then_some(WorkForm::Vec)
        }
        TyKind::Array(elem, _len) if is_work_ty(tcx, *elem) => Some(WorkForm::Array),
        TyKind::Ref(_, inner, _) if is_work_ty(tcx, *inner) => Some(WorkForm::Ref),
        _ => None,
    }
}

/// Is `ty` any recognized `Work` declaration form? Used by the device-lifecycle
/// field split to route `Work` fields away from the user-data classifier (whose
/// D3 would otherwise reject the methodless `Work` shell). Both the supported
/// required form and the rejected non-required forms return `true`, so the
/// rejection is a `DWS-ERR-*` diagnostic, not an opaque user-data D3 error.
pub(crate) fn is_work_field<'tcx>(tcx: TyCtxt<'tcx>, ty: Ty<'tcx>) -> bool {
    classify_work_form(tcx, ty).is_some()
}

/// Phase-4 materialisation: intern every driver's `Work`-resource facts and run
/// the recognition negative set (unsupported form). Runs AFTER
/// `device_lifecycle::materialise_drivers` (the driver map is the walk root) and
/// after the sibling resource passes. Fields are walked in declaration order
/// (the order the `init_work` preamble ops + the per-resource slot index follow,
/// GRILL T07). Records NO field-attr marker — the `Work` field carries no
/// attribute; recognition is purely type-driven (GRILL D04), and the
/// device-lifecycle routing keys on `is_work_field` (the type test).
pub(crate) fn materialise_deferred_work(cx: &mut BridgeCtxt<'_>) -> Result<(), BridgeError> {
    let tcx = cx.input.tcx;
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
        let driver_name = {
            let name_id = cx.model.device_lifecycle.drivers[&item_id].descriptor.name;
            cx.model.strings.resolve(name_id).to_string()
        };
        let adt = tcx.adt_def(d_def_id);
        let mut resources = Vec::new();
        for variant in adt.variants() {
            for (field_idx, field) in variant.fields.iter().enumerate() {
                let field_ty = tcx.type_of(field.did).instantiate_identity();
                let Some(form) = classify_work_form(tcx, field_ty) else {
                    continue;
                };
                // v1 builds only the required single form; reject the rest
                // (their *support* is design-in, their *rejection* is built).
                if form != WorkForm::Required {
                    return Err(BridgeError::DeferredWorkValidation(
                        DeferredWorkDiagnostic::UnsupportedForm {
                            driver: driver_name.clone(),
                            field: field.name.to_string(),
                            form: form.label().to_string(),
                        },
                    ));
                }
                let name = cx.model.strings.intern(field.name.as_str());
                resources.push(WorkResource {
                    name,
                    field: FieldIdx::from_u32(field_idx as u32),
                    required: true,
                    // The `Work` ADT is a ZST; the byte offset is preserved for
                    // the design-in container_of recovery (GRILL S14), unused in
                    // v1 (the backend keys on the per-resource slot index).
                    offset: 0,
                    handler: None,
                    submit_sites: Vec::new(),
                });
            }
        }
        if resources.is_empty() {
            continue;
        }
        cx.model
            .deferred_work
            .drivers
            .insert(item_id, DriverDeferredWork { resources });
    }
    Ok(())
}

/// Phase-4 legality: every recognized `deferred_work_submit` intrinsic must be a
/// method on the `Work` resource shell. A `#[rseq::intrinsic("deferred_work_submit")]`
/// tag on a method with any other receiver is a spoofed submit op (the `submit`
/// op exists only on `Work`). Mirrors `clock::validate_clock_signatures` — walks
/// the recorded intrinsic markers, recovers each method's def-id, and checks its
/// `&self` receiver type via the abstraction marker. Runs AFTER
/// `materialise_intrinsics`. (The direct-field-path requirement on the *call
/// site* — not the callee — is enforced by the post-body-materialization pass in
/// `submit::recognise_submit_sites`, GRILL T06.)
pub(crate) fn validate_deferred_work_signatures(
    cx: &mut BridgeCtxt<'_>,
) -> Result<(), BridgeError> {
    let tcx = cx.input.tcx;
    for (idx, attrs) in cx.model.domain_attrs.item_attrs.iter().enumerate() {
        let is_submit_intrinsic = attrs
            .iter()
            .any(|a| matches!(a, AttrData::Intrinsic(IntrinsicKind::DeferredWork(_))));
        if !is_submit_intrinsic {
            continue;
        }
        let item_id = ItemId::from_u32(idx as u32);
        if !matches!(
            cx.model.item_synthetic_kinds[item_id],
            SyntheticItemKind::None
        ) {
            continue;
        }
        let Some(def_id) = recover_source_def_id(tcx, cx.model.item_def_paths[item_id]) else {
            continue;
        };
        let sig = tcx.fn_sig(def_id).instantiate_identity().skip_binder();
        // The `&self` receiver is `args[0]`; peel the reference and check the
        // receiver type is the `Work` resource shell (the abstraction marker).
        let on_work = sig.inputs().first().is_some_and(|recv| match recv.kind() {
            TyKind::Ref(_, inner, _) => is_work_ty(tcx, *inner),
            _ => is_work_ty(tcx, *recv),
        });
        if !on_work {
            let receiver = sig
                .inputs()
                .first()
                .map(|t| t.to_string())
                .unwrap_or_else(|| "<no receiver>".to_string());
            return Err(BridgeError::DeferredWorkValidation(
                DeferredWorkDiagnostic::SpoofedReceiver { receiver },
            ));
        }
    }
    Ok(())
}
