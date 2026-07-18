//! Phase-4 `#[rseq::clock(id = "…")]` field recognition + `clock_*` intrinsic
//! legality (peripheral-clock-management construct).
//!
//! Mirrors `bridge::interrupt::recognition` (item-driven): walk the interned
//! `#[rseq::driver]` structs, classify each field's declaration form against the
//! typed `Clock` resource shell, parse the mandatory `id` join key off the field
//! attribute (R01/D13 — no field-name fallback), and intern the per-driver
//! [`ClockResource`] facts into `model.clock.drivers` plus the
//! [`AttrData::ClockResource`] marker into `domain_attrs.field_attrs`.
//!
//! v1 builds only the **required** `Clock` form; `Option<Clock>` / `[Clock; N]` /
//! `&Clock` are recognized whole and rejected (C06 N2/N14 — optional + arrays
//! designed-in, not built). The full recognition negative set (C06): N1 missing
//! id, N12 malformed attr (empty id / duplicate or unknown key), N13 attr on a
//! non-`Clock` field, N2/N14 unsupported form, N15 duplicate L1 id, N21 forbidden
//! L1 key, N22 spoofed `clock_*` receiver. `Clock` is detected by type name
//! (matching the `Mmio`/`Interrupt` anchors). The shell's `enable`/`disable`/
//! `get_rate` are author intrinsics; the compiler-managed `acquire_clock`/
//! `release` are synthesized downstream from the recognized field (D05).

use regseq_model::{
    AttrData, ClockResource, DriverClocks, FieldIdx, FieldPath, IntrinsicKind, ItemId, StringId,
    SyntheticItemKind, VariantIdx,
};
use rustc_ast::token::{Delimiter, LitKind as TokenLitKind, TokenKind};
use rustc_ast::tokenstream::TokenTree;
use rustc_hir::{AttrArgs, Attribute};
use rustc_middle::ty::{Ty, TyCtxt, TyKind};
use rustc_span::{def_id::DefId, Symbol as RustcSymbol};

use super::super::framework_type::recover_source_def_id;
use super::super::{BridgeCtxt, BridgeError, ClockDiagnostic};

/// Forbidden L1 `#[rseq::clock(...)]` attribute keys (C06 N21): `mode`/
/// `default_frequency`/`provider` belong in the `[clock.<id>]` PLATFORM section,
/// not the L1 attribute; `required` is inferred from the type form;
/// `set_rate`/`clock_auto_enable` are reserved/out-of-scope. `clk_bulk_*` is a
/// reserved family (matched by prefix). The ONLY admitted L1 key is `id`.
const FORBIDDEN_ATTR_KEYS: &[&str] = &[
    "required",
    "mode",
    "default_frequency",
    "provider",
    "set_rate",
    "clock_auto_enable",
];

/// The recognized declaration form of a `Clock` field. v1 builds only
/// [`ClockForm::Required`]; the others are recognized whole and rejected
/// (C06 N2/N14).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ClockForm {
    /// `Clock` — the required form (the only one v1 builds).
    Required,
    /// `Option<Clock>` — the optional form (designed-in, not built).
    Optional,
    /// `[Clock; N]` — the array form (designed-in, not built).
    Array,
    /// `&Clock` / `&mut Clock` — a reference form (not a resource declaration).
    Ref,
}

impl ClockForm {
    /// The author-facing spelling for the `CLK-ERR-UNSUPPORTED-FORM` diagnostic.
    fn label(self) -> &'static str {
        match self {
            ClockForm::Required => "Clock",
            ClockForm::Optional => "Option<Clock>",
            ClockForm::Array => "[Clock; N]",
            ClockForm::Ref => "&Clock",
        }
    }
}

/// The outcome of parsing a `#[rseq::clock(...)]` field attribute.
enum ClockAttr {
    /// No `#[rseq::clock]` attribute, or the attribute carries no `id` key
    /// (C06 N1 — mandatory id, no field-name fallback).
    MissingId,
    /// A valid `id = "…"` with a non-empty value.
    Id(String),
    /// The attribute is structurally bad (C06 N12): empty/whitespace id, a
    /// duplicate `id` key, a non-string `id`, or an unknown key.
    Malformed(String),
    /// A forbidden L1 key (C06 N21).
    Forbidden(String),
}

/// Is `ty` the typed `Clock` resource shell? Name-based detection, matching
/// `is_mmio_resource` / `is_interrupt_ty`'s anchor discipline.
fn is_clock_ty(tcx: TyCtxt<'_>, ty: Ty<'_>) -> bool {
    ty.ty_adt_def()
        .is_some_and(|adt| tcx.item_name(adt.did()).as_str() == "Clock")
}

/// Classify a driver-struct field type into its `Clock` declaration form, or
/// `None` when the field is not a clock resource at all. All recognized forms
/// are returned whole; the non-`Required` forms are rejected by
/// [`materialise_clocks`], never silently narrowed here.
fn classify_clock_form<'tcx>(tcx: TyCtxt<'tcx>, ty: Ty<'tcx>) -> Option<ClockForm> {
    if is_clock_ty(tcx, ty) {
        return Some(ClockForm::Required);
    }
    match ty.kind() {
        TyKind::Adt(adt, args)
            if tcx.get_diagnostic_name(adt.did()) == Some(rustc_span::sym::Option) =>
        {
            let inner = args.types().next()?;
            is_clock_ty(tcx, inner).then_some(ClockForm::Optional)
        }
        TyKind::Array(elem, _len) if is_clock_ty(tcx, *elem) => Some(ClockForm::Array),
        TyKind::Ref(_, inner, _) if is_clock_ty(tcx, *inner) => Some(ClockForm::Ref),
        _ => None,
    }
}

/// Is `ty` any recognized `Clock` declaration form? Used by the device-lifecycle
/// field split to route clock fields away from the user-data classifier (whose
/// D3 would otherwise reject the methodless `Clock` shell). Both the supported
/// required form and the rejected non-required forms return `true`, so the
/// rejection is a `CLK-ERR-*` diagnostic, not an opaque user-data D3 error.
pub(crate) fn is_clock_field<'tcx>(tcx: TyCtxt<'tcx>, ty: Ty<'tcx>) -> bool {
    classify_clock_form(tcx, ty).is_some()
}

/// Whether `def_id` carries a `#[rseq::clock(...)]` attribute (used for the C06
/// N13 attr-on-non-`Clock`-field check).
fn has_clock_attr(tcx: TyCtxt<'_>, def_id: DefId) -> bool {
    let path = [RustcSymbol::intern("rseq"), RustcSymbol::intern("clock")];
    // Bind before the block end so the iterator temporary (which borrows `path`)
    // drops before `path` does (Rust 2024 capture/drop-order rule).
    let present = tcx.get_attrs_by_path(def_id, &path).next().is_some();
    present
}

/// Parse + validate the `#[rseq::clock(...)]` attribute on `def_id`. The ONLY
/// admitted key is `id = "…"` (non-empty); unknown/duplicate keys (N12) and the
/// forbidden L1 keys (N21) are rejected here — unlike `#[rseq::mmio]`/
/// `#[rseq::interrupt]`, clock does NOT silently ignore unknown keys (GRILL D13).
fn parse_clock_field_attr(tcx: TyCtxt<'_>, def_id: DefId) -> ClockAttr {
    let path = [RustcSymbol::intern("rseq"), RustcSymbol::intern("clock")];
    let Some(attr) = tcx.get_attrs_by_path(def_id, &path).next() else {
        return ClockAttr::MissingId;
    };
    let Attribute::Unparsed(item) = attr else {
        return ClockAttr::MissingId;
    };
    let AttrArgs::Delimited(delim) = &item.args else {
        return ClockAttr::MissingId;
    };
    if delim.delim != Delimiter::Parenthesis {
        return ClockAttr::MissingId;
    }
    let mut id: Option<String> = None;
    let mut iter = delim.tokens.iter().peekable();
    while iter.peek().is_some() {
        let Some(TokenTree::Token(key_tok, _)) = iter.next() else {
            return ClockAttr::Malformed("malformed attribute tokens".to_string());
        };
        let Some(key) = key_tok.ident().map(|(ident, _)| ident.name) else {
            return ClockAttr::Malformed("expected a `key = \"…\"` pair".to_string());
        };
        match iter.next() {
            Some(TokenTree::Token(tok, _)) if matches!(tok.kind, TokenKind::Eq) => {}
            _ => {
                return ClockAttr::Malformed(format!("key `{key}` is missing its `= \"…\"` value"))
            }
        }
        let value = iter.next();
        let key_str = key.as_str();
        if key_str == "id" {
            if id.is_some() {
                return ClockAttr::Malformed("duplicate `id` key".to_string());
            }
            match expect_str_lit(value) {
                Some(s) => id = Some(s),
                None => return ClockAttr::Malformed("`id` must be a string literal".to_string()),
            }
        } else if FORBIDDEN_ATTR_KEYS.contains(&key_str) || key_str.starts_with("clk_bulk") {
            return ClockAttr::Forbidden(key_str.to_string());
        } else {
            return ClockAttr::Malformed(format!("unknown key `{key_str}`"));
        }
        match iter.peek() {
            None => break,
            Some(TokenTree::Token(tok, _)) if matches!(tok.kind, TokenKind::Comma) => {
                iter.next();
            }
            _ => return ClockAttr::Malformed("expected `,` between attribute keys".to_string()),
        }
    }
    match id {
        None => ClockAttr::MissingId,
        Some(s) if s.trim().is_empty() => {
            ClockAttr::Malformed("`id` is empty or whitespace".to_string())
        }
        Some(s) => ClockAttr::Id(s),
    }
}

/// Extract a string literal from a token, or `None` (mirrors
/// `interrupt::recognition::expect_str_lit`).
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

/// Phase-4 materialisation: intern every driver's clock-resource facts and run
/// the C06 recognition negative set (N1/N12/N13/N2/N14/N15/N21). Runs AFTER
/// `device_lifecycle::materialise_drivers` (the driver map is the walk root).
/// Fields are walked in declaration order (the order `acquire_clock` preamble
/// ops will run, GRILL D16).
pub(crate) fn materialise_clocks(cx: &mut BridgeCtxt<'_>) -> Result<(), BridgeError> {
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
                let Some(form) = classify_clock_form(tcx, field_ty) else {
                    // C06 N13: a `#[rseq::clock]` attr on a non-`Clock` field.
                    if has_clock_attr(tcx, field.did) {
                        return Err(BridgeError::ClockValidation(
                            ClockDiagnostic::AttrOnNonClock {
                                driver: driver_name.clone(),
                                field: field.name.to_string(),
                            },
                        ));
                    }
                    continue;
                };
                // v1 builds only the required form; reject the rest (C06 N2/N14).
                if form != ClockForm::Required {
                    return Err(BridgeError::ClockValidation(
                        ClockDiagnostic::UnsupportedForm {
                            driver: driver_name.clone(),
                            field: field.name.to_string(),
                            form: form.label().to_string(),
                        },
                    ));
                }
                // Mandatory, non-empty id with only the `id` key (C06 N1/N12/N21).
                let id_str = match parse_clock_field_attr(tcx, field.did) {
                    ClockAttr::Id(s) => s,
                    ClockAttr::MissingId => {
                        return Err(BridgeError::ClockValidation(ClockDiagnostic::MissingId {
                            driver: driver_name.clone(),
                            field: field.name.to_string(),
                        }))
                    }
                    ClockAttr::Malformed(detail) => {
                        return Err(BridgeError::ClockValidation(
                            ClockDiagnostic::MalformedAttr {
                                driver: driver_name.clone(),
                                field: field.name.to_string(),
                                detail,
                            },
                        ))
                    }
                    ClockAttr::Forbidden(key) => {
                        return Err(BridgeError::ClockValidation(
                            ClockDiagnostic::ForbiddenAttrKey {
                                driver: driver_name.clone(),
                                field: field.name.to_string(),
                                key,
                            },
                        ))
                    }
                };
                let id = cx.model.strings.intern(&id_str);
                // Duplicate L1 id within one driver (C06 N15).
                if seen_ids.contains(&id) {
                    return Err(BridgeError::ClockValidation(ClockDiagnostic::DuplicateId {
                        driver: driver_name.clone(),
                        id: id_str,
                    }));
                }
                seen_ids.push(id);
                let name = cx.model.strings.intern(field.name.as_str());
                resources.push(ClockResource {
                    name,
                    id,
                    required: true,
                    binding: Default::default(),
                });
                field_markers.push((
                    FieldPath {
                        item: item_id,
                        variant: VariantIdx::ZERO,
                        field: FieldIdx::from_u32(field_idx as u32),
                    },
                    AttrData::ClockResource { id, required: true },
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
            .clock
            .drivers
            .insert(item_id, DriverClocks { resources });
    }
    Ok(())
}

/// Phase-4 legality (C06 N22): every recognized `clock_*` intrinsic must be a
/// method on the `Clock` resource shell. A `#[rseq::intrinsic("clock_*")]` tag on
/// a method with any other receiver is a spoofed clock op (the `enable`/`disable`/
/// `get_rate` ops exist only on `Clock`). Mirrors `atomic::validate_atomic_signatures`
/// — walks the recorded intrinsic markers, recovers each method's def-id, and
/// checks its `&self` receiver type. Runs AFTER `materialise_intrinsics`.
pub(crate) fn validate_clock_signatures(cx: &mut BridgeCtxt<'_>) -> Result<(), BridgeError> {
    let tcx = cx.input.tcx;
    for (idx, attrs) in cx.model.domain_attrs.item_attrs.iter().enumerate() {
        let is_clock_intrinsic = attrs
            .iter()
            .any(|a| matches!(a, AttrData::Intrinsic(IntrinsicKind::Clock(_))));
        if !is_clock_intrinsic {
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
        // The `&self` receiver is `args[0]`; peel the reference and check the
        // receiver type is the `Clock` resource shell.
        let on_clock = sig.inputs().first().is_some_and(|recv| match recv.kind() {
            TyKind::Ref(_, inner, _) => is_clock_ty(tcx, *inner),
            _ => is_clock_ty(tcx, *recv),
        });
        if !on_clock {
            let receiver = sig
                .inputs()
                .first()
                .map(|t| t.to_string())
                .unwrap_or_else(|| "<no receiver>".to_string());
            return Err(BridgeError::ClockValidation(
                ClockDiagnostic::SpoofedReceiver {
                    method: method_name,
                    receiver,
                },
            ));
        }
    }
    Ok(())
}
