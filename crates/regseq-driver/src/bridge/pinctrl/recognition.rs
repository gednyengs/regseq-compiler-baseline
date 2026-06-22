//! Phase-4 `#[rseq::pinctrl(id = "…")]` field recognition + `#[rseq::pin_state]`
//! marker enumeration (pin-controller-and-GPIO-configuration construct).
//!
//! Two item-driven passes mirroring `bridge::clock::recognition` (fields) and
//! `bridge::framework_type` (markers):
//!
//!   * [`materialise_pinctrls`] — walk the interned `#[rseq::driver]` structs,
//!     classify each field against the typed `PinCtrl` resource shell, parse the
//!     mandatory `id` join key off the field attribute (R01/D07 — no field-name
//!     fallback), and intern the per-driver [`PinctrlResource`] facts into
//!     `model.pinctrl.drivers` plus the [`AttrData::PinctrlResource`] marker into
//!     `domain_attrs.field_attrs`. BOTH the required `PinCtrl` and optional
//!     `Option<PinCtrl>` forms are built (build-wide, D02/D03); `[PinCtrl; N]` /
//!     `&PinCtrl` / `Vec<PinCtrl>` are recognized whole and rejected (C06).
//!
//!   * [`materialise_pin_state_markers`] — walk every struct Item, read its
//!     `#[rseq::pin_state(id = "…")]` attribute, and record
//!     [`AttrData::PinStateMarker`] onto the marker `ItemId` (D04). The marker's
//!     resolved name is read at lowering from the `apply::<S>()` call's generic
//!     arg `S`. The empty `PinState` trait is irrelevant to recognition — the
//!     `#[rseq::pin_state]` attribute is the authority (the framework_type idiom).
//!
//! `PinCtrl` is detected by type name (matching the `Clock`/`Mmio`/`Interrupt`
//! anchors). M1 builds only the EASY `apply` receiver shapes (the shared
//! intrinsic pass recognizes the op; receiver recovery is the backend's, T07);
//! the full C06 recognition negative set (non-ZST marker, invalid/duplicate state
//! markers, unresolved generic `S`, spoofed `apply` receiver) is T09/T10.

use regseq_model::{
    AttrData, DriverPinctrls, FieldIdx, FieldPath, IntrinsicKind, ItemId, ItemKind,
    PinctrlResource, StringId, SyntheticItemKind, VariantIdx,
};
use rustc_ast::token::{Delimiter, LitKind as TokenLitKind, TokenKind};
use rustc_ast::tokenstream::TokenTree;
use rustc_hir::{AttrArgs, Attribute};
use rustc_middle::ty::{Ty, TyCtxt, TyKind};
use rustc_span::{def_id::DefId, Symbol as RustcSymbol};

use super::super::framework_type::recover_source_def_id;
use super::super::{BridgeCtxt, BridgeError, PinctrlDiagnostic};

/// The recognized declaration form of a pinctrl field. Build-wide v1 builds
/// [`PinctrlForm::Required`] and [`PinctrlForm::Optional`]; the others are
/// recognized whole and rejected (C06 UNSUPPORTED-FORM).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PinctrlForm {
    /// `PinCtrl` — the required form (D03).
    Required,
    /// `Option<PinCtrl>` — the optional form (D02).
    Optional,
    /// `[PinCtrl; N]` — the array form (designed-in, not built).
    Array,
    /// `&PinCtrl` / `&mut PinCtrl` — a reference form (not a resource decl).
    Ref,
}

impl PinctrlForm {
    /// The author-facing spelling for the `PIN-ERR-UNSUPPORTED-FORM` diagnostic.
    fn label(self) -> &'static str {
        match self {
            PinctrlForm::Required => "PinCtrl",
            PinctrlForm::Optional => "Option<PinCtrl>",
            PinctrlForm::Array => "[PinCtrl; N]",
            PinctrlForm::Ref => "&PinCtrl",
        }
    }
}

/// The outcome of parsing a `#[rseq::pinctrl(...)]` / `#[rseq::pin_state(...)]`
/// attribute — the only admitted key is `id = "…"`.
enum IdAttr {
    /// No attribute, or the attribute carries no `id` key.
    MissingId,
    /// A valid `id = "…"` with a non-empty value.
    Id(String),
    /// The attribute is structurally bad: empty/whitespace id, a duplicate `id`
    /// key, a non-string `id`, or an unknown key.
    Malformed(String),
}

/// Is `ty` the typed `PinCtrl` resource shell? Name-based detection, matching
/// `is_clock_ty` / `is_mmio_resource`'s anchor discipline.
fn is_pinctrl_ty(tcx: TyCtxt<'_>, ty: Ty<'_>) -> bool {
    ty.ty_adt_def()
        .is_some_and(|adt| tcx.item_name(adt.did()).as_str() == "PinCtrl")
}

/// Classify a driver-struct field type into its pinctrl declaration form, or
/// `None` when the field is not a pinctrl resource at all. All recognized forms
/// are returned whole; the non-`Required`/`Optional` forms are rejected by
/// [`materialise_pinctrls`], never silently narrowed here.
fn classify_pinctrl_form<'tcx>(tcx: TyCtxt<'tcx>, ty: Ty<'tcx>) -> Option<PinctrlForm> {
    if is_pinctrl_ty(tcx, ty) {
        return Some(PinctrlForm::Required);
    }
    match ty.kind() {
        TyKind::Adt(adt, args)
            if tcx.get_diagnostic_name(adt.did()) == Some(rustc_span::sym::Option) =>
        {
            let inner = args.types().next()?;
            is_pinctrl_ty(tcx, inner).then_some(PinctrlForm::Optional)
        }
        TyKind::Array(elem, _len) if is_pinctrl_ty(tcx, *elem) => Some(PinctrlForm::Array),
        TyKind::Ref(_, inner, _) if is_pinctrl_ty(tcx, *inner) => Some(PinctrlForm::Ref),
        _ => None,
    }
}

/// Is `ty` any recognized pinctrl declaration form? Used by the device-lifecycle
/// field split to route pinctrl fields away from the user-data classifier (whose
/// D3 would otherwise reject the methodless `PinCtrl` shell). Both the supported
/// required/optional forms and the rejected array/ref forms return `true`, so the
/// rejection is a `PIN-ERR-*` diagnostic, not an opaque user-data D3 error.
pub(crate) fn is_pinctrl_field<'tcx>(tcx: TyCtxt<'tcx>, ty: Ty<'tcx>) -> bool {
    classify_pinctrl_form(tcx, ty).is_some()
}

/// Whether `def_id` carries a `#[rseq::<leaf>(...)]` attribute.
fn has_rseq_attr(tcx: TyCtxt<'_>, def_id: DefId, leaf: &str) -> bool {
    let path = [RustcSymbol::intern("rseq"), RustcSymbol::intern(leaf)];
    let present = tcx.get_attrs_by_path(def_id, &path).next().is_some();
    present
}

/// Parse + validate a `#[rseq::<leaf>(id = "…")]` attribute on `def_id`. The ONLY
/// admitted key is `id = "…"` (non-empty); unknown/duplicate keys are rejected
/// (the pinctrl/pin_state attributes admit only `id`, like `#[rseq::clock]`,
/// GRILL D07). Mirrors `clock::recognition::parse_clock_field_attr`.
fn parse_id_attr(tcx: TyCtxt<'_>, def_id: DefId, leaf: &str) -> IdAttr {
    let path = [RustcSymbol::intern("rseq"), RustcSymbol::intern(leaf)];
    let Some(attr) = tcx.get_attrs_by_path(def_id, &path).next() else {
        return IdAttr::MissingId;
    };
    let Attribute::Unparsed(item) = attr else {
        return IdAttr::MissingId;
    };
    let AttrArgs::Delimited(delim) = &item.args else {
        return IdAttr::MissingId;
    };
    if delim.delim != Delimiter::Parenthesis {
        return IdAttr::MissingId;
    }
    let mut id: Option<String> = None;
    let mut iter = delim.tokens.iter().peekable();
    while iter.peek().is_some() {
        let Some(TokenTree::Token(key_tok, _)) = iter.next() else {
            return IdAttr::Malformed("malformed attribute tokens".to_string());
        };
        let Some(key) = key_tok.ident().map(|(ident, _)| ident.name) else {
            return IdAttr::Malformed("expected a `key = \"…\"` pair".to_string());
        };
        match iter.next() {
            Some(TokenTree::Token(tok, _)) if matches!(tok.kind, TokenKind::Eq) => {}
            _ => return IdAttr::Malformed(format!("key `{key}` is missing its `= \"…\"` value")),
        }
        let value = iter.next();
        let key_str = key.as_str();
        if key_str == "id" {
            if id.is_some() {
                return IdAttr::Malformed("duplicate `id` key".to_string());
            }
            match expect_str_lit(value) {
                Some(s) => id = Some(s),
                None => return IdAttr::Malformed("`id` must be a string literal".to_string()),
            }
        } else {
            return IdAttr::Malformed(format!("unknown key `{key_str}`"));
        }
        match iter.peek() {
            None => break,
            Some(TokenTree::Token(tok, _)) if matches!(tok.kind, TokenKind::Comma) => {
                iter.next();
            }
            _ => return IdAttr::Malformed("expected `,` between attribute keys".to_string()),
        }
    }
    match id {
        None => IdAttr::MissingId,
        Some(s) if s.trim().is_empty() => IdAttr::Malformed("`id` is empty or whitespace".to_string()),
        Some(s) => IdAttr::Id(s),
    }
}

/// Extract a string literal from a token, or `None` (mirrors
/// `clock::recognition::expect_str_lit`).
fn expect_str_lit(tt: Option<&TokenTree>) -> Option<String> {
    let TokenTree::Token(tok, _) = tt? else {
        return None;
    };
    let TokenKind::Literal(lit) = &tok.kind else {
        return None;
    };
    if !matches!(lit.kind, TokenLitKind::Str | TokenLitKind::StrRaw(_)) {
        return None;
    }
    Some(lit.symbol.as_str().to_string())
}

/// Phase-4 materialisation: intern every driver's pinctrl-resource facts and run
/// the M1 recognition negative subset (missing-id, malformed-attr,
/// attr-on-non-pinctrl, unsupported-form, duplicate-id). Runs AFTER
/// `device_lifecycle::materialise_drivers` (the driver map is the walk root).
/// Fields are walked in declaration order (the order the `acquire_pinctrl`
/// preamble ops will run, GRILL D08).
pub(crate) fn materialise_pinctrls(cx: &mut BridgeCtxt<'_>) -> Result<(), BridgeError> {
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
        let mut field_markers = Vec::new();
        let mut seen_ids: Vec<StringId> = Vec::new();
        for variant in adt.variants() {
            for (field_idx, field) in variant.fields.iter().enumerate() {
                let field_ty = tcx.type_of(field.did).instantiate_identity();
                let Some(form) = classify_pinctrl_form(tcx, field_ty) else {
                    // A `#[rseq::pinctrl]` attr on a non-`PinCtrl` field (C06).
                    if has_rseq_attr(tcx, field.did, "pinctrl") {
                        return Err(BridgeError::PinctrlValidation(
                            PinctrlDiagnostic::AttrOnNonPinctrl {
                                driver: driver_name.clone(),
                                field: field.name.to_string(),
                            },
                        ));
                    }
                    continue;
                };
                // Build-wide: required `PinCtrl` + optional `Option<PinCtrl>`;
                // reject the array/ref forms (C06 UNSUPPORTED-FORM).
                let required = match form {
                    PinctrlForm::Required => true,
                    PinctrlForm::Optional => false,
                    PinctrlForm::Array | PinctrlForm::Ref => {
                        return Err(BridgeError::PinctrlValidation(
                            PinctrlDiagnostic::UnsupportedForm {
                                driver: driver_name.clone(),
                                field: field.name.to_string(),
                                form: form.label().to_string(),
                            },
                        ))
                    }
                };
                // Mandatory, non-empty id with only the `id` key (C06).
                let id_str = match parse_id_attr(tcx, field.did, "pinctrl") {
                    IdAttr::Id(s) => s,
                    IdAttr::MissingId => {
                        return Err(BridgeError::PinctrlValidation(
                            PinctrlDiagnostic::MissingId {
                                driver: driver_name.clone(),
                                field: field.name.to_string(),
                            },
                        ))
                    }
                    IdAttr::Malformed(detail) => {
                        return Err(BridgeError::PinctrlValidation(
                            PinctrlDiagnostic::MalformedAttr {
                                driver: driver_name.clone(),
                                field: field.name.to_string(),
                                detail,
                            },
                        ))
                    }
                };
                let pinctrl_id = cx.model.strings.intern(&id_str);
                // Duplicate L1 id within one driver (C06).
                if seen_ids.contains(&pinctrl_id) {
                    return Err(BridgeError::PinctrlValidation(
                        PinctrlDiagnostic::DuplicateId {
                            driver: driver_name.clone(),
                            id: id_str,
                        },
                    ));
                }
                seen_ids.push(pinctrl_id);
                let name = cx.model.strings.intern(field.name.as_str());
                resources.push(PinctrlResource {
                    name,
                    pinctrl_id,
                    required,
                    binding: Default::default(),
                });
                field_markers.push((
                    FieldPath {
                        item: item_id,
                        variant: VariantIdx::ZERO,
                        field: FieldIdx::from_u32(field_idx as u32),
                    },
                    AttrData::PinctrlResource {
                        pinctrl_id,
                        required,
                    },
                ));
            }
        }
        if resources.is_empty() {
            continue;
        }
        for (path, marker) in field_markers {
            cx.model
                .domain_attrs
                .field_attrs
                .entry(path)
                .or_default()
                .push(marker);
        }
        cx.model
            .pinctrl
            .drivers
            .insert(item_id, DriverPinctrls { resources });
    }
    Ok(())
}

/// Phase-4 marker enumeration: walk every struct Item, read its
/// `#[rseq::pin_state(id = "…")]` attribute, and record
/// [`AttrData::PinStateMarker`] onto its `ItemId` (D04). Item-driven, mirroring
/// `framework_type::materialise_framework_types`: a marked struct is recorded
/// regardless of how it is used (it appears in the model only because some
/// `apply::<S>()` monomorphizes with it as the generic arg). Synthetic and
/// non-source (dependency-crate) Items are skipped.
pub(crate) fn materialise_pin_state_markers(cx: &mut BridgeCtxt<'_>) -> Result<(), BridgeError> {
    let tcx = cx.input.tcx;
    let mut recognised: Vec<(ItemId, StringId)> = Vec::new();
    // Track each claimed state NAME -> the marker type that first claimed it, so a
    // second marker claiming the same name is PIN-ERR-DUPLICATE-STATE-MARKER (C06).
    // Names are compared by their pre-intern string (the join key the L2 inventory
    // and the `apply` resolution use).
    let mut claimed: std::collections::BTreeMap<String, String> = std::collections::BTreeMap::new();
    for (idx, item) in cx.model.items.iter().enumerate() {
        let item_id = ItemId::from_u32(idx as u32);
        // Pin-state markers are author-declared unit structs.
        if !matches!(item.kind, ItemKind::Struct) {
            continue;
        }
        if !matches!(
            cx.model.item_synthetic_kinds[item_id],
            SyntheticItemKind::None
        ) {
            continue;
        }
        let Some(def_id) = recover_source_def_id(tcx, cx.model.item_def_paths[item_id]) else {
            continue;
        };
        if !def_id.is_local() {
            continue;
        }
        if !has_rseq_attr(tcx, def_id, "pin_state") {
            continue;
        }
        let marker_name = tcx.item_name(def_id).to_string();
        // The marker carries the attribute, so a missing/malformed `id` is a
        // structural error (PIN-ERR-MALFORMED-STATE-ATTR, C06).
        let id_str = match parse_id_attr(tcx, def_id, "pin_state") {
            IdAttr::Id(s) => s,
            IdAttr::MissingId => {
                return Err(BridgeError::PinctrlValidation(
                    PinctrlDiagnostic::MalformedStateAttr {
                        marker: marker_name,
                        detail: "the `id` key is missing".to_string(),
                    },
                ))
            }
            IdAttr::Malformed(detail) => {
                return Err(BridgeError::PinctrlValidation(
                    PinctrlDiagnostic::MalformedStateAttr {
                        marker: marker_name,
                        detail,
                    },
                ))
            }
        };
        // A pin-state marker is an author-declared UNIT struct (D04): it carries
        // no data, only its type identity + the `id`. A payload-bearing marker is
        // PIN-ERR-NON-ZST-MARKER (C06). The fieldless check is stricter than
        // ZST-ness (a `PhantomData` field is also rejected), matching the
        // "unit struct" contract exactly.
        let field_count = tcx.adt_def(def_id).non_enum_variant().fields.len();
        if field_count != 0 {
            return Err(BridgeError::PinctrlValidation(
                PinctrlDiagnostic::NonZstMarker {
                    marker: marker_name,
                    fields: field_count,
                },
            ));
        }
        // Two markers claiming one state name is ambiguous (C06). Compared by the
        // pre-intern name so the diagnostic names both marker types.
        if let Some(first) = claimed.get(&id_str) {
            return Err(BridgeError::PinctrlValidation(
                PinctrlDiagnostic::DuplicateStateMarker {
                    id: id_str,
                    first: first.clone(),
                    second: marker_name,
                },
            ));
        }
        claimed.insert(id_str.clone(), marker_name);
        let id = cx.model.strings.intern(&id_str);
        recognised.push((item_id, id));
    }
    for (item_id, id) in recognised {
        let attrs = &mut cx.model.domain_attrs.item_attrs[item_id];
        if !attrs
            .iter()
            .any(|a| matches!(a, AttrData::PinStateMarker { .. }))
        {
            attrs.push(AttrData::PinStateMarker { id });
        }
    }
    Ok(())
}

// PIN-ERR-CONTEXT (atomic-context `apply`, C06) is design-in, NOT built (M2/T13):
// it depends on a cross-construct sleepable/atomic context-detection seam that is
// not yet landed (the same seam timed-cpu-delay's `TCD-ERR-01` awaits). When that
// frontend context check lands, `apply` in an atomic region is rejected here;
// until then the disposition is recorded, not dropped (mirrors clock D11/PRD).

/// Is `recv` (an `apply` method's `&self` receiver type) a legal pinctrl-apply
/// receiver? The op is legal on `&PinCtrl` (required + optional-unwrapped) and on
/// the `&Option<PinCtrl>` extension receiver (GRILL D14). Peels the outer `&`,
/// then admits only the `Required`/`Optional` pinctrl forms.
fn is_legal_apply_receiver<'tcx>(tcx: TyCtxt<'tcx>, recv: Ty<'tcx>) -> bool {
    let pointee = match recv.kind() {
        TyKind::Ref(_, inner, _) => *inner,
        _ => recv,
    };
    matches!(
        classify_pinctrl_form(tcx, pointee),
        Some(PinctrlForm::Required | PinctrlForm::Optional)
    )
}

/// Phase-4 legality (C06 / D14): every recognized `pinctrl_apply` intrinsic must
/// be a method on the `PinCtrl` resource shell or on `Option<PinCtrl>` (the
/// `OptionalPinCtrl` extension). A `#[rseq::intrinsic("pinctrl_apply")]` tag on a
/// method with any other receiver is a spoofed pinctrl op
/// (`PIN-ERR-SPOOFED-RECEIVER`). Mirrors `clock::validate_clock_signatures` —
/// walks the recorded intrinsic markers, recovers each method's def-id, and
/// checks its `&self` receiver type. Runs AFTER `materialise_intrinsics`.
pub(crate) fn validate_pinctrl_signatures(cx: &mut BridgeCtxt<'_>) -> Result<(), BridgeError> {
    let tcx = cx.input.tcx;
    for (idx, attrs) in cx.model.domain_attrs.item_attrs.iter().enumerate() {
        let is_pinctrl_intrinsic = attrs
            .iter()
            .any(|a| matches!(a, AttrData::Intrinsic(IntrinsicKind::Pinctrl(_))));
        if !is_pinctrl_intrinsic {
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
        let method_name = tcx.item_name(def_id).to_string();
        let sig = tcx.fn_sig(def_id).instantiate_identity().skip_binder();
        // The op must be a genuine METHOD (an associated fn with a `self`
        // receiver), not a free/associated fn that merely takes a `PinCtrl` first
        // arg — otherwise an author could mint an alternate `apply` facade. Then
        // the `&self` receiver (`args[0]`) must be `&PinCtrl`/`PinCtrl` or
        // `&Option<PinCtrl>`/`Option<PinCtrl>` (D14).
        let is_method = tcx
            .opt_associated_item(def_id)
            .is_some_and(|assoc| assoc.is_method());
        let recv_ok = sig
            .inputs()
            .first()
            .is_some_and(|recv| is_legal_apply_receiver(tcx, *recv));
        if !(is_method && recv_ok) {
            let receiver = if !is_method {
                "a non-method (a free or associated fn without `self`)".to_string()
            } else {
                sig.inputs()
                    .first()
                    .map(|t| t.to_string())
                    .unwrap_or_else(|| "<no receiver>".to_string())
            };
            return Err(BridgeError::PinctrlValidation(
                PinctrlDiagnostic::SpoofedReceiver {
                    method: method_name,
                    receiver,
                },
            ));
        }
    }
    Ok(())
}
