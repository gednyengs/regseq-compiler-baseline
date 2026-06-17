//! Atomic-memory-access-and-ordering (atomicity construct) frontend bridge.
//!
//! Two responsibilities, mirroring the register-access (`Mmio<T>`) +
//! device-interrupt (`Interrupt`) field-resource precedent and the timed-delay
//! signature-check precedent:
//!
//! 1. [`materialise_atomics`] (Phase 4): an **item-driven** pass that walks every
//!    interned struct, classifies each `Atomic<T>` field by element type, and
//!    records an [`AttrData::Atomic`] marker into `domain_attrs.field_attrs`
//!    keyed by `FieldPath` (the element type + the optional
//!    `#[rseq::atomic(initial = N)]` literal). The set of recorded markers, in
//!    `FieldPath` order, is what the generic-c backend enumerates to assign each
//!    `Atomic` field a deterministic cell id (C06/C07/OR-1).
//!
//! `Atomic` is detected by type name, matching the name-based `Mmio` /
//! `Interrupt` anchors (`device_lifecycle/resolve.rs`,
//! `interrupt/recognition.rs`). The standalone micro-unit (D05) needs no
//! `#[rseq::driver]` host: the scan covers every struct in the crate under
//! compilation, so a plain driver-state struct (zero L2, no lifecycle — D11) is
//! recognized.
//!
//! The const-`Ordering` discipline (D14) + the per-op legality matrix (C01),
//! element-type/bit-test legality (C07/D08), and the facade-signature
//! `MISSING-ORDERING` residual (D15) are M2 frontend passes; this module is the
//! M1 record half.

use regseq_model::{
    AtomicElement, AttrData, FieldIdx, FieldPath, ItemId, ItemKind, SyntheticItemKind, VariantIdx,
};
use rustc_ast::token::{Delimiter, LitKind as TokenLitKind, TokenKind};
use rustc_ast::tokenstream::TokenTree;
use rustc_hir::{AttrArgs, Attribute};
use rustc_middle::ty::{self, Ty, TyCtxt, TyKind};
use rustc_span::{def_id::DefId, Symbol as RustcSymbol};

use super::framework_type::recover_source_def_id;
use super::{BridgeCtxt, BridgeError};

/// If `ty` is the typed `Atomic<T>` resource shell, the element type `T`.
/// Name-based detection, matching `is_mmio_resource` / `is_interrupt_ty`.
fn atomic_element_ty<'tcx>(tcx: TyCtxt<'tcx>, ty: Ty<'tcx>) -> Option<Ty<'tcx>> {
    let TyKind::Adt(adt, args) = ty.kind() else {
        return None;
    };
    if tcx.item_name(adt.did()).as_str() != "Atomic" {
        return None;
    }
    args.types().next()
}

/// Classify the `Atomic<T>` element type `T` into the recognized
/// [`AtomicElement`] set (`{u32,i32,u64,i64}` + permissive pointer, C07/D08), or
/// `None` for an unsupported `T`. (The M2 element-type pass turns the `None`
/// case into `ATOMIC-ERR-ELEMENT-TYPE`; in M1 an unsupported element is simply
/// not recorded, so its ops fail loud at emit rather than mis-lowering.)
fn classify_element(elem: Ty<'_>) -> Option<AtomicElement> {
    match elem.kind() {
        TyKind::Uint(ty::UintTy::U32) => Some(AtomicElement::U32),
        TyKind::Int(ty::IntTy::I32) => Some(AtomicElement::I32),
        TyKind::Uint(ty::UintTy::U64) => Some(AtomicElement::U64),
        TyKind::Int(ty::IntTy::I64) => Some(AtomicElement::I64),
        // Permissive pointer-width integer atomic (G-ATM-07): both `*mut T` and
        // `*const T`.
        TyKind::RawPtr(..) => Some(AtomicElement::Ptr),
        _ => None,
    }
}

/// Parse the optional `initial = <int-literal>` key off `#[rseq::atomic(...)]` on
/// `def_id`. Returns `None` when the attribute is absent or carries no `initial`
/// key (default-init); a present-but-malformed `initial` also yields `None`
/// (treated as default-init for M1; the M2 attribute-validation can tighten
/// this). A leading `-` makes the literal negative (for `i32`/`i64`).
fn recognise_atomic_initial(tcx: TyCtxt<'_>, def_id: DefId) -> Option<i128> {
    let path = [RustcSymbol::intern("rseq"), RustcSymbol::intern("atomic")];
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
        // A leading `-` negates the following integer literal.
        let mut negative = false;
        if let Some(TokenTree::Token(tok, _)) = iter.peek() {
            if matches!(tok.kind, TokenKind::Minus) {
                negative = true;
                iter.next();
            }
        }
        let value = iter.next();
        if key.as_str() == "initial" {
            return expect_int_lit(negative, value);
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

/// Extract an integer literal from a token as an `i128` (decimal/hex/octal/
/// binary, underscores stripped, type suffix ignored), or `None`.
fn expect_int_lit(negative: bool, tt: Option<&TokenTree>) -> Option<i128> {
    let TokenTree::Token(tok, _) = tt? else {
        return None;
    };
    let TokenKind::Literal(lit) = &tok.kind else {
        return None;
    };
    if !matches!(lit.kind, TokenLitKind::Integer) {
        return None;
    }
    // `lit.symbol` is the digit body (the type suffix is carried separately in
    // `lit.suffix`); strip the `_` digit separators.
    let raw = lit.symbol.as_str().replace('_', "");
    let value = if let Some(h) = raw.strip_prefix("0x").or_else(|| raw.strip_prefix("0X")) {
        i128::from_str_radix(h, 16).ok()?
    } else if let Some(o) = raw.strip_prefix("0o").or_else(|| raw.strip_prefix("0O")) {
        i128::from_str_radix(o, 8).ok()?
    } else if let Some(b) = raw.strip_prefix("0b").or_else(|| raw.strip_prefix("0B")) {
        i128::from_str_radix(b, 2).ok()?
    } else {
        raw.parse::<i128>().ok()?
    };
    Some(if negative { -value } else { value })
}

/// Phase-4 materialisation: record the [`AttrData::Atomic`] field marker for
/// every `Atomic<T>` driver-struct field.
///
/// Item-driven (walks every interned struct), mirroring
/// `framework_type::materialise_framework_types`' struct scan + synthetic guard
/// + local-`DefId` filter. Runs in `assign_item_ids_and_populate_sidetables`
/// after the intrinsic/framework-type passes. The recorded markers, enumerated
/// in `FieldPath` (BTreeMap) order by the backend, fix each field's cell id.
pub(crate) fn materialise_atomics(cx: &mut BridgeCtxt<'_>) -> Result<(), BridgeError> {
    let tcx = cx.input.tcx;
    // Collect the (FieldPath, marker) pairs first so the immutable tcx-driven
    // walk does not overlap the mutable field_attrs insert.
    let mut markers: Vec<(FieldPath, AttrData)> = Vec::new();
    for (idx, item) in cx.model.items.iter().enumerate() {
        let item_id = ItemId::from_u32(idx as u32);
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
        let adt = tcx.adt_def(def_id);
        // A struct has exactly one variant; `variants()` keeps the field order.
        for variant in adt.variants() {
            for (field_idx, field) in variant.fields.iter().enumerate() {
                let field_ty = tcx.type_of(field.did).instantiate_identity();
                let Some(elem_ty) = atomic_element_ty(tcx, field_ty) else {
                    continue;
                };
                // An `Atomic<T>` with an unsupported `T`: not recorded in M1
                // (the M2 element-type pass rejects it with
                // `ATOMIC-ERR-ELEMENT-TYPE`). Skipping here means its ops fail
                // loud at the backend rather than lowering against a guessed
                // width.
                let Some(element) = classify_element(elem_ty) else {
                    continue;
                };
                let initial_value = recognise_atomic_initial(tcx, field.did);
                markers.push((
                    FieldPath {
                        item: item_id,
                        variant: VariantIdx::ZERO,
                        field: FieldIdx::from_u32(field_idx as u32),
                    },
                    AttrData::Atomic {
                        element,
                        initial_value,
                    },
                ));
            }
        }
    }
    for (path, marker) in markers {
        cx.model
            .domain_attrs
            .field_attrs
            .entry(path)
            .or_default()
            .push(marker);
    }
    Ok(())
}
