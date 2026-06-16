//! Phase-4 `#[rseq::interrupt(id = "…")]` field recognition (construct #6).
//!
//! Mirrors `bridge::device_lifecycle::recognition` (item-driven): walk the
//! interned `#[rseq::driver]` structs, classify each field's declaration form
//! against the typed `Interrupt` resource shell (`Interrupt` /
//! `Option<Interrupt>` / `[Interrupt; N]` — all three recognized whole, R02),
//! parse the mandatory `id` join key off the field attribute (R01/D08 — no
//! field-name fallback, unlike `#[rseq::mmio]`), and intern the per-driver
//! [`InterruptResource`] facts into `model.interrupt.drivers` plus the
//! [`AttrData::InterruptResource`] marker into `domain_attrs.field_attrs`.
//!
//! `Interrupt` is detected by type name, matching the name-based `Mmio` /
//! `Lifecycle` anchors (the `#[rseq::abstraction("interrupt")]` tag on the
//! shell is documentation; recognition keys are the attribute strings — GRILL
//! O4). The shell is methodless from the driver's perspective: recognition
//! introduces no callable line-control surface (I02/I03).

use regseq_model::{
    AttrData, DriverInterrupts, FieldIdx, FieldPath, InterruptForm, InterruptResource, ItemId,
    VariantIdx,
};
use rustc_ast::token::{Delimiter, LitKind as TokenLitKind, TokenKind};
use rustc_ast::tokenstream::TokenTree;
use rustc_hir::{AttrArgs, Attribute};
use rustc_middle::ty::{Ty, TyCtxt, TyKind};
use rustc_span::{def_id::DefId, Symbol as RustcSymbol};

use super::super::framework_type::recover_source_def_id;
use super::super::{BridgeCtxt, BridgeError, InterruptDiagnostic};

/// Is `ty` the typed `Interrupt` resource shell? Name-based detection,
/// matching [`is_mmio_resource`]'s anchor discipline
/// (`device_lifecycle/resolve.rs`).
fn is_interrupt_ty(tcx: TyCtxt<'_>, ty: Ty<'_>) -> bool {
    ty.ty_adt_def()
        .is_some_and(|adt| tcx.item_name(adt.did()).as_str() == "Interrupt")
}

/// Classify a driver-struct field type into its `Interrupt` declaration form,
/// or `None` when the field is not an interrupt resource at all. All three
/// forms are recognized whole (R02/D06); the non-`Single` forms are emit-stage
/// rejected later, never silently narrowed here (I05).
pub(crate) fn classify_interrupt_form<'tcx>(
    tcx: TyCtxt<'tcx>,
    ty: Ty<'tcx>,
) -> Option<InterruptForm> {
    if is_interrupt_ty(tcx, ty) {
        return Some(InterruptForm::Single);
    }
    match ty.kind() {
        TyKind::Adt(adt, args)
            if tcx.get_diagnostic_name(adt.did()) == Some(rustc_span::sym::Option) =>
        {
            let inner = args.types().next()?;
            is_interrupt_ty(tcx, inner).then_some(InterruptForm::Optional)
        }
        TyKind::Array(elem, len) if is_interrupt_ty(tcx, *elem) => {
            let len = len.try_to_target_usize(tcx)?;
            Some(InterruptForm::Array {
                len: u32::try_from(len).ok()?,
            })
        }
        _ => None,
    }
}

/// Is `ty` any recognized `Interrupt` declaration form? Used by the
/// device-lifecycle field split to route interrupt fields away from the
/// user-data classifier (whose D3 would otherwise reject them).
pub(crate) fn is_interrupt_field<'tcx>(tcx: TyCtxt<'tcx>, ty: Ty<'tcx>) -> bool {
    classify_interrupt_form(tcx, ty).is_some()
}

/// Parse the `id = "…"` key off `#[rseq::interrupt(...)]` on `def_id`.
/// Returns `None` when the attribute is absent, carries no `id`, or the value
/// is not a string literal — the caller turns all three into the mandatory-id
/// frontend error (R01/D08; no field-name fallback). Unknown keys are skipped
/// (forward-compat, matching `recognise_mmio_field_attr`).
fn recognise_interrupt_field_attr(tcx: TyCtxt<'_>, def_id: DefId) -> Option<String> {
    let path = [RustcSymbol::intern("rseq"), RustcSymbol::intern("interrupt")];
    let attr = tcx.get_attrs_by_path(def_id, &path).next()?;
    let Attribute::Unparsed(item) = attr else {
        return None;
    };
    let AttrArgs::Delimited(delim) = &item.args else {
        return None;
    };
    if delim.delim != Delimiter::Parenthesis {
        return None;
    }
    let mut iter = delim.tokens.iter().peekable();
    while iter.peek().is_some() {
        let Some(TokenTree::Token(key_tok, _)) = iter.next() else {
            break;
        };
        let Some(key) = key_tok.ident().map(|(ident, _)| ident.name) else {
            break;
        };
        match iter.next() {
            Some(TokenTree::Token(tok, _)) if matches!(tok.kind, TokenKind::Eq) => {}
            _ => break,
        }
        let value = iter.next();
        if key.as_str() == "id" {
            return expect_str_lit(value);
        }
        match iter.peek() {
            None => break,
            Some(TokenTree::Token(tok, _)) if matches!(tok.kind, TokenKind::Comma) => {
                iter.next();
            }
            _ => break,
        }
    }
    None
}

/// Extract a string literal from a token, or `None` (mirrors
/// `device_lifecycle::recognition::expect_str_lit`).
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

/// Phase-4 materialisation: intern every driver's interrupt-resource facts.
///
/// Runs AFTER `device_lifecycle::materialise_drivers` (the driver map is the
/// walk root). For each driver struct, fields are walked in declaration order
/// (the order interrupt preambles will run, among themselves); each
/// `Interrupt`-form field must carry `#[rseq::interrupt(id = "…")]` — a
/// missing attribute or missing `id` is the mandatory-id frontend error
/// (R01/D08). Recognized facts land in `model.interrupt.drivers` and as
/// [`AttrData::InterruptResource`] entries in `domain_attrs.field_attrs`.
pub(crate) fn materialise_interrupts(cx: &mut BridgeCtxt<'_>) -> Result<(), BridgeError> {
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
        for variant in adt.variants() {
            for (field_idx, field) in variant.fields.iter().enumerate() {
                let field_ty = tcx.type_of(field.did).instantiate_identity();
                let Some(form) = classify_interrupt_form(tcx, field_ty) else {
                    continue;
                };
                let Some(id) = recognise_interrupt_field_attr(tcx, field.did) else {
                    return Err(BridgeError::InterruptValidation(
                        InterruptDiagnostic::MissingId {
                            driver: driver_name.clone(),
                            field: field.name.to_string(),
                        },
                    ));
                };
                let id = cx.model.strings.intern(&id);
                let name = cx.model.strings.intern(field.name.as_str());
                resources.push(InterruptResource {
                    name,
                    id,
                    form,
                    binding: Default::default(),
                });
                field_markers.push((
                    FieldPath {
                        item: item_id,
                        variant: VariantIdx::ZERO,
                        field: FieldIdx::from_u32(field_idx as u32),
                    },
                    AttrData::InterruptResource { id, form },
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
        cx.model.interrupt.drivers.insert(
            item_id,
            DriverInterrupts {
                resources,
                handlers: Vec::new(),
            },
        );
    }
    Ok(())
}
