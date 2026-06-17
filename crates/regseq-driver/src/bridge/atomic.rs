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
    AtomicElement, AtomicOp, AttrData, Body, CallCallee, Const, FetchOp, FieldIdx, FieldPath,
    GenericArg, Instance, InstanceKind, IntrinsicKind, ItemId, ItemKind, Operand, Place, PlaceElem,
    RegseqModel, Rvalue, StatementKind, SyntheticItemKind, TerminatorKind, Ty as ModelTy, TyId,
    UintKind, VariantIdx,
};
use rustc_ast::token::{Delimiter, LitKind as TokenLitKind, TokenKind};
use rustc_ast::tokenstream::TokenTree;
use rustc_hir::{AttrArgs, Attribute};
use rustc_middle::ty::{self, Ty, TyCtxt, TyKind};
use rustc_span::{def_id::DefId, Symbol as RustcSymbol};

use super::framework_type::recover_source_def_id;
use super::{AtomicDiagnostic, BridgeCtxt, BridgeError};

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

/// Whether `ty` is an `Atomic<T>` resource field (any `T`). Used by the
/// device-lifecycle field classifier to route `Atomic` fields away from the
/// user-data D3 rejection (mirroring the `Mmio`/`Interrupt` routing); the
/// element-type legality + the field-attr record are handled by
/// [`materialise_atomics`].
pub(crate) fn is_atomic_field<'tcx>(tcx: TyCtxt<'tcx>, ty: Ty<'tcx>) -> bool {
    atomic_element_ty(tcx, ty).is_some()
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
                // Element-type legality (T09/C07/D08): an `Atomic<T>` whose `T`
                // is outside `{u32,i32,u64,i64}` + permissive pointer is rejected
                // with `ATOMIC-ERR-ELEMENT-TYPE`. Folded into the field walk that
                // already classifies `T` (the recognition is the legality).
                let Some(element) = classify_element(elem_ty) else {
                    return Err(BridgeError::AtomicValidation(
                        AtomicDiagnostic::ElementType {
                            field: field.name.to_string(),
                            ty: elem_ty.to_string(),
                        },
                    ));
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

// ----------------------------------------------------------------------------
// M2 frontend legality (C01/C02): the ordering-legality matrix, the const-
// `Ordering` discipline (D14), the bit-test element/bounds check (C07/D08), and
// the facade-signature MISSING-ORDERING residual (D15).
// ----------------------------------------------------------------------------

/// The author-facing facade leaf for a recognized atomic op (diagnostics).
fn atomic_op_name(op: AtomicOp) -> &'static str {
    match op {
        AtomicOp::Load => "load",
        AtomicOp::Store => "store",
        AtomicOp::FetchModify(FetchOp::Add) => "fetch_add",
        AtomicOp::FetchModify(FetchOp::Sub) => "fetch_sub",
        AtomicOp::FetchModify(FetchOp::Or) => "fetch_or",
        AtomicOp::FetchModify(FetchOp::And) => "fetch_and",
        AtomicOp::FetchModify(FetchOp::Xor) => "fetch_xor",
        AtomicOp::FetchModify(FetchOp::Swap) => "swap",
        AtomicOp::CompareExchange => "compare_exchange",
        AtomicOp::BitTestSet => "bit_test_set",
        AtomicOp::BitTestClear => "bit_test_clear",
        AtomicOp::Fence => "fence",
    }
}

/// The `Ordering` enum-discriminant name (C05 ABI: Relaxed=0 .. SeqCst=4).
fn ordering_name(variant: u32) -> &'static str {
    match variant {
        0 => "Relaxed",
        1 => "Acquire",
        2 => "Release",
        3 => "AcqRel",
        4 => "SeqCst",
        _ => "<unknown>",
    }
}

/// `a ⊑ b` in the ordering strength lattice
/// `Relaxed ⊏ {Acquire, Release} ⊏ AcqRel ⊏ SeqCst` (Acquire/Release
/// incomparable): true iff every memory-ordering capability of `a` is also in
/// `b` (the `compare_exchange` `failure ⊑ success` rule, C01/C04).
fn ordering_le(a: u32, b: u32) -> bool {
    // (acquire, release, seqcst) capabilities per discriminant.
    fn caps(o: u32) -> (bool, bool, bool) {
        match o {
            0 => (false, false, false), // Relaxed
            1 => (true, false, false),  // Acquire
            2 => (false, true, false),  // Release
            3 => (true, true, false),   // AcqRel
            4 => (true, true, true),    // SeqCst
            _ => (false, false, false),
        }
    }
    let (aa, ar, asq) = caps(a);
    let (ba, br, bsq) = caps(b);
    (!aa || ba) && (!ar || br) && (!asq || bsq)
}

/// `ATOMIC-ERR-ORDERING-NONCONST` for `op` (D14).
fn nonconst(op: &str) -> BridgeError {
    BridgeError::AtomicValidation(AtomicDiagnostic::OrderingNonConst { op: op.to_string() })
}

/// `ATOMIC-ERR-DEGENERATE-ACQREL` (generalized, O3) for `op` with the rejected
/// `variant` and the human `valid` set.
fn degenerate(op: &str, variant: u32, valid: &str) -> BridgeError {
    BridgeError::AtomicValidation(AtomicDiagnostic::DegenerateAcqrel {
        op: op.to_string(),
        ordering: ordering_name(variant).to_string(),
        valid: valid.to_string(),
    })
}

/// A short element-type name for a bit-test diagnostic.
fn model_elem_name(model: &RegseqModel, ty: TyId) -> &'static str {
    match &model.types[ty] {
        ModelTy::Uint(UintKind::U32) => "u32",
        ModelTy::Uint(UintKind::U64) => "u64",
        ModelTy::Int(regseq_model::IntKind::I32) => "i32",
        ModelTy::Int(regseq_model::IntKind::I64) => "i64",
        ModelTy::Ptr(..) | ModelTy::Ref(..) => "<ptr>",
        _ => "<other>",
    }
}

/// The materialized body of a function instance, if any.
fn function_body(instance: &Instance) -> Option<&Body> {
    match &instance.kind {
        InstanceKind::Function(data) => data.body.as_ref(),
        _ => None,
    }
}

/// The place an atomic wrapper-method `&self` receiver operand borrows — the
/// driver-struct field place the op targets — for the S01 direct-field check.
/// Handles a directly-projected receiver, the single-`Ref`-temp shape
/// (`_t = &place; op(move _t, …)`), and a `Use`-of-temp chain — mirroring the
/// per-backend `resolve_receiver_src` (same depth bound + ambiguity rule) so the
/// frontend resolves EXACTLY the receiver shapes the generic-c oracle does. That
/// parity is load-bearing for S01: a shape the oracle resolves to a nested field
/// (and would emit) must also be one this check resolves and rejects — otherwise
/// the hole reappears. `None` when the borrowed place is not recoverable (the
/// oracle then cannot resolve it either, so it errors rather than silently
/// emitting a nested access).
fn receiver_borrowed_place<'b>(body: &'b Body, recv: &'b Operand) -> Option<&'b Place> {
    receiver_borrowed_place_at(body, recv, 0)
}

fn receiver_borrowed_place_at<'b>(
    body: &'b Body,
    recv: &'b Operand,
    depth: u32,
) -> Option<&'b Place> {
    if depth > 8 {
        return None; // reaching-def chase exceeded the bound (matches the backend)
    }
    let place = match recv {
        Operand::Copy(p) | Operand::Move(p) => p,
        Operand::Constant(_) => return None,
    };
    // A receiver that already names a field (a directly-projected place).
    if place
        .projection
        .iter()
        .any(|e| matches!(e, PlaceElem::Field(..)))
    {
        return Some(place);
    }
    // A bare temp: chase its UNIQUE whole-local definition — a borrow
    // (`Ref`/`RawPtr`) gives the place; a `Use` of another operand recurses (the
    // backend resolver follows `Use`, so the frontend must too).
    let local = place.local;
    let mut def: Option<&Rvalue> = None;
    for block in body.basic_blocks.iter() {
        for stmt in block.statements.iter() {
            if let StatementKind::Assign(dst, rhs) = &stmt.kind {
                if dst.local == local && dst.projection.is_empty() {
                    if def.is_some() {
                        return None; // ambiguous
                    }
                    def = Some(rhs);
                }
            }
        }
    }
    match def? {
        Rvalue::Ref(_, src) | Rvalue::RawPtr(_, src) => Some(src),
        Rvalue::Use(inner) => receiver_borrowed_place_at(body, inner, depth + 1),
        _ => None,
    }
}

/// The `Atomic<T>` element type named by a wrapper-method `&self` receiver
/// operand — the receiver's place type, peeled of its reference, with the
/// `Atomic` struct's first type argument. `None` when the operand is not a
/// recoverable receiver place (the bit-test check is then skipped — the runtime
/// hook still bounds-asserts).
fn receiver_element_ty(model: &RegseqModel, body: &Body, recv: &Operand) -> Option<TyId> {
    let place = match recv {
        Operand::Copy(p) | Operand::Move(p) => p,
        Operand::Constant(_) => return None,
    };
    // The borrow temp carries the full `&Atomic<T>` type with no projection.
    if !place.projection.is_empty() {
        return None;
    }
    let ref_ty = body.local_decls[place.local].ty;
    let atomic_ty = match &model.types[ref_ty] {
        ModelTy::Ref(p, _) | ModelTy::Ptr(p, _) => *p,
        _ => return None,
    };
    match &model.types[atomic_ty] {
        ModelTy::Struct { args, .. } => args.iter().find_map(|a| match a {
            GenericArg::Type(t) => Some(*t),
            _ => None,
        }),
        _ => None,
    }
}

/// Is `ty` the `Ordering` enum (by type name, matching the name-based resource
/// anchors)? A by-value `Ordering` parameter satisfies the facade-signature
/// obligation (D15).
fn is_ordering_ty(tcx: TyCtxt<'_>, ty: Ty<'_>) -> bool {
    ty.ty_adt_def()
        .is_some_and(|adt| tcx.item_name(adt.did()).as_str() == "Ordering")
}

/// The `Call.args` / signature-input position(s) the mandatory `Ordering`
/// argument(s) occupy for an op (`args[0]` is the `&self` receiver for methods;
/// `fence` is a free fn). This is the SINGLE source of truth shared by the
/// facade-signature obligation (`validate_atomic_signatures`, which requires an
/// `Ordering` at each position) and the per-op legality check
/// (`validate_atomic_call`, which reads the ordering at each position) — so a
/// mis-declared facade whose arity/position does not match cannot slip an
/// unvalidated ordering past the ops check.
fn ordering_positions(op: AtomicOp) -> &'static [usize] {
    match op {
        AtomicOp::Load => &[1],
        AtomicOp::Store => &[2],
        AtomicOp::FetchModify(_) => &[2],
        AtomicOp::BitTestSet | AtomicOp::BitTestClear => &[2],
        AtomicOp::CompareExchange => &[3, 4],
        AtomicOp::Fence => &[0],
    }
}

/// Phase-4 facade-signature obligation (D15/R13; C02): every recognized
/// `#[rseq::intrinsic("atomic_*")]` callee must declare its mandatory trailing
/// by-value `Ordering` parameter(s) — `ATOMIC-ERR-MISSING-ORDERING` otherwise.
///
/// The canonical typed facade's call-site omission is type-system-subsumed (a
/// rustc error before the bridge runs, which executes in `after_analysis`); the
/// residual reachable case is a *mis-declared* facade, validated here exactly
/// like `timed_delay::validate_delay_signatures` / the device-interrupt ISR
/// signature check. Recognition is call-driven, so an uncalled mis-declared
/// facade is never recognized and never validated — harmless (it has no call
/// site to lower).
pub(crate) fn validate_atomic_signatures(cx: &mut BridgeCtxt<'_>) -> Result<(), BridgeError> {
    let tcx = cx.input.tcx;
    for (idx, attrs) in cx.model.domain_attrs.item_attrs.iter().enumerate() {
        let Some(op) = attrs.iter().find_map(|a| match a {
            AttrData::Intrinsic(IntrinsicKind::Atomic(op)) => Some(*op),
            _ => None,
        }) else {
            continue;
        };
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
        let opname = atomic_op_name(op);
        let sig = tcx.fn_sig(def_id).instantiate_identity().skip_binder();
        let inputs = sig.inputs();
        // Every position the ops legality check reads MUST hold a by-value
        // `Ordering` — i.e. the facade declares its mandatory `Ordering`
        // argument(s) at the canonical position(s) (D15). This rejects BOTH a
        // missing trailing `Ordering` AND a wrong-arity facade (e.g. a `store`
        // missing its value parameter, which would otherwise shift the ordering
        // out from under the ops check). Arity: a call provides exactly one arg
        // per input, so an in-range `Ordering` input guarantees the ops check
        // finds the ordering operand at the same index.
        for &pos in ordering_positions(op) {
            let ok = inputs.get(pos).is_some_and(|param| is_ordering_ty(tcx, *param));
            if !ok {
                let detail = match inputs.get(pos) {
                    None => format!(
                        "found {} parameter(s); the mandatory Ordering argument at position {pos} \
                         is absent",
                        inputs.len()
                    ),
                    Some(param) => {
                        format!("parameter at position {pos} has type `{param}`, not `Ordering`")
                    }
                };
                return Err(BridgeError::AtomicValidation(
                    AtomicDiagnostic::MissingOrdering {
                        facade: opname.to_string(),
                        detail,
                    },
                ));
            }
            // The const `Ordering` is read by POSITIONAL discriminant (the C05
            // ABI: Relaxed=0 .. SeqCst=4), so EACH ordering param's enum MUST
            // declare exactly Relaxed, Acquire, Release, AcqRel, SeqCst in that
            // order — else a same-named reordered enum (e.g.
            // `core::sync::atomic::Ordering`, which swaps Release/Acquire) would
            // SILENTLY remap (O7). EVERY position is checked, not just the first:
            // `compare_exchange` declares two (success, failure), and a mixed
            // facade (canonical success + std-shaped failure) would otherwise
            // misread the failure ordering. The `is_ordering_ty` check above
            // already confirmed this param is an `Ordering`-named enum.
            if let Some(param) = inputs.get(pos) {
                if let Err(detail) = ordering_enum_canonical(*param) {
                    return Err(BridgeError::AtomicValidation(
                        AtomicDiagnostic::OrderingEnum {
                            facade: opname.to_string(),
                            detail,
                        },
                    ));
                }
            }
        }
    }
    Ok(())
}

/// Verify an `Ordering` parameter enum declares exactly the canonical variants
/// `Relaxed, Acquire, Release, AcqRel, SeqCst` in that order (so the positional
/// const-discriminant ABI read is sound). Returns the mismatch detail otherwise.
fn ordering_enum_canonical(ty: Ty<'_>) -> Result<(), String> {
    const CANON: [&str; 5] = ["Relaxed", "Acquire", "Release", "AcqRel", "SeqCst"];
    let adt = ty
        .ty_adt_def()
        .filter(|adt| adt.is_enum())
        .ok_or_else(|| format!("`{ty}` is not an enum"))?;
    let variants = adt.variants();
    if variants.len() != CANON.len() {
        return Err(format!(
            "found {} variant(s), expected {}",
            variants.len(),
            CANON.len()
        ));
    }
    for (variant, expected) in variants.iter().zip(CANON.iter()) {
        if variant.name.as_str() != *expected {
            return Err(format!(
                "variant `{}` where `{expected}` was expected",
                variant.name
            ));
        }
    }
    Ok(())
}

/// Phase-5 frontend op legality (C01/C02): walks every materialized body and,
/// for each recognized atomic-op call, enforces the const-`Ordering` discipline
/// (D14) + the per-op valid-ordering matrix (C01) + the bit-test element/bounds
/// rule (C07/D08). Runs after body materialization (mirrors
/// `timed_delay::validate_isr_no_sleep`); a rejection aborts the bridge, so no
/// IR for an offending op reaches a backend.
pub(crate) fn validate_atomic_ops(model: &RegseqModel) -> Result<(), BridgeError> {
    for instance in model.instances.iter() {
        let Some(body) = function_body(instance) else {
            continue;
        };
        for block in body.basic_blocks.iter() {
            let TerminatorKind::Call {
                callee: CallCallee::Direct(callee_id),
                args,
                ..
            } = &block.terminator.kind
            else {
                continue;
            };
            let Some(IntrinsicKind::Atomic(op)) = model.instance_intrinsic(*callee_id) else {
                continue;
            };
            validate_atomic_call(model, body, *op, args)?;
        }
    }
    Ok(())
}

/// The valid-ordering matrix (C01) + bit-test/CAS/fence checks for one call.
fn validate_atomic_call(
    model: &RegseqModel,
    body: &Body,
    op: AtomicOp,
    args: &[Operand],
) -> Result<(), BridgeError> {
    let name = atomic_op_name(op);
    // S01 (#2 fix): a receiver op must target a DIRECT driver-struct field. This
    // target-agnostic frontend reject keeps the generic-c oracle in lock-step
    // with the Linux/Zephyr backends (which only address a direct field) — an
    // oracle that accepted a nested receiver would predict acceptance the OS
    // backends then refuse. `fence` has no receiver.
    if !matches!(op, AtomicOp::Fence) {
        if let Some(recv) = args.first() {
            if let Some(place) = receiver_borrowed_place(body, recv) {
                let field_steps = place
                    .projection
                    .iter()
                    .filter(|e| matches!(e, PlaceElem::Field(..)))
                    .count();
                let indexed = place
                    .projection
                    .iter()
                    .any(|e| matches!(e, PlaceElem::Index(_) | PlaceElem::ConstantIndex { .. }));
                if field_steps != 1 || indexed {
                    let detail = if indexed {
                        "the receiver indexes an array of atomics".to_string()
                    } else {
                        format!("the Atomic field is reached through {field_steps} field step(s)")
                    };
                    return Err(BridgeError::AtomicValidation(
                        AtomicDiagnostic::NestedField {
                            op: name.to_string(),
                            detail,
                        },
                    ));
                }
            }
        }
    }
    match op {
        // `load`: Relaxed/Acquire/SeqCst (Release/AcqRel ⇒ degenerate). arg 1.
        AtomicOp::Load => check_ordering(model, body, args, 1, name, &[0, 1, 4], "Relaxed/Acquire/SeqCst"),
        // `store`: Relaxed/Release/SeqCst (Acquire/AcqRel ⇒ degenerate). arg 2.
        AtomicOp::Store => check_ordering(model, body, args, 2, name, &[0, 2, 4], "Relaxed/Release/SeqCst"),
        // `fetch_*`/`swap`: all five orderings legal. arg 2.
        AtomicOp::FetchModify(_) => check_ordering(model, body, args, 2, name, &[0, 1, 2, 3, 4], "all"),
        // `bit_test_*`: all five orderings legal (arg 2) + unsigned element +
        // const bit in range.
        AtomicOp::BitTestSet | AtomicOp::BitTestClear => {
            check_ordering(model, body, args, 2, name, &[0, 1, 2, 3, 4], "all")?;
            check_bit_test(model, body, name, args)
        }
        // `compare_exchange`: success (arg 3) all five; failure (arg 4)
        // Relaxed/Acquire/SeqCst and failure ⊑ success.
        AtomicOp::CompareExchange => check_cas(model, body, args),
        // `atomic::fence`: Acquire/Release/AcqRel/SeqCst (Relaxed ⇒
        // FENCE-RELAXED); free fn, order at arg 0.
        AtomicOp::Fence => check_fence(model, body, args),
    }
}

/// Read the const ordering at `args[pos]` (non-const ⇒ ORDERING-NONCONST) and
/// require it in `valid` (else DEGENERATE-ACQREL). A missing positional argument
/// is type-subsumed (the rustc signature guarantees arity), so it is skipped.
fn check_ordering(
    model: &RegseqModel,
    body: &Body,
    args: &[Operand],
    pos: usize,
    name: &str,
    valid: &[u32],
    valid_str: &str,
) -> Result<(), BridgeError> {
    let Some(op_arg) = args.get(pos) else {
        return Ok(());
    };
    let Some(variant) = model.resolve_operand_enum_variant(body, op_arg) else {
        return Err(nonconst(name));
    };
    if !valid.contains(&variant) {
        return Err(degenerate(name, variant, valid_str));
    }
    Ok(())
}

/// `atomic::fence(order)` legality: const, and not Relaxed (C01).
fn check_fence(model: &RegseqModel, body: &Body, args: &[Operand]) -> Result<(), BridgeError> {
    let Some(op_arg) = args.first() else {
        return Ok(());
    };
    let Some(variant) = model.resolve_operand_enum_variant(body, op_arg) else {
        return Err(nonconst("fence"));
    };
    if variant == 0 {
        return Err(BridgeError::AtomicValidation(AtomicDiagnostic::FenceRelaxed));
    }
    // Valid fence orderings are exactly {Acquire, Release, AcqRel, SeqCst}.
    // Reject any out-of-range discriminant too (a malformed `Ordering`), closing
    // the same unknown-variant hole the CAS success bound closes.
    if !matches!(variant, 1 | 2 | 3 | 4) {
        return Err(degenerate(
            "fence",
            variant,
            "Acquire/Release/AcqRel/SeqCst",
        ));
    }
    Ok(())
}

/// `compare_exchange` ordering legality (C01/C04): success ∈ all five; failure ∈
/// {Relaxed, Acquire, SeqCst} and failure ⊑ success; both compile-time const.
fn check_cas(model: &RegseqModel, body: &Body, args: &[Operand]) -> Result<(), BridgeError> {
    let (Some(s_arg), Some(f_arg)) = (args.get(3), args.get(4)) else {
        return Ok(());
    };
    let Some(succ) = model.resolve_operand_enum_variant(body, s_arg) else {
        return Err(nonconst("compare_exchange"));
    };
    let Some(fail) = model.resolve_operand_enum_variant(body, f_arg) else {
        return Err(nonconst("compare_exchange"));
    };
    // success: all five legal — but it MUST be one of the five known orderings.
    // (Validating the bound closes the `ordering_le(_, unknown)` hole, where an
    // out-of-range discriminant has empty capabilities and would otherwise
    // appear weaker than any failure.)
    if !matches!(succ, 0 | 1 | 2 | 3 | 4) {
        return Err(degenerate(
            "compare_exchange success",
            succ,
            "Relaxed/Acquire/Release/AcqRel/SeqCst",
        ));
    }
    // failure: Relaxed/Acquire/SeqCst only.
    if !matches!(fail, 0 | 1 | 4) {
        return Err(degenerate(
            "compare_exchange failure",
            fail,
            "Relaxed/Acquire/SeqCst",
        ));
    }
    // ...and no stronger than success.
    if !ordering_le(fail, succ) {
        return Err(degenerate(
            "compare_exchange failure",
            fail,
            &format!("an ordering no stronger than success ({})", ordering_name(succ)),
        ));
    }
    Ok(())
}

/// `bit_test_*` element + bounds legality (C07/D08): the atomic element must be
/// unsigned (`u32`/`u64`), and a compile-time-constant bit index must be in
/// range — both `ATOMIC-ERR-BIT-TEST-TYPE`. A non-const bit index is bounds-
/// asserted at the generic-c hook at runtime.
fn check_bit_test(
    model: &RegseqModel,
    body: &Body,
    name: &str,
    args: &[Operand],
) -> Result<(), BridgeError> {
    let Some(recv) = args.first() else {
        return Ok(());
    };
    let Some(elem) = receiver_element_ty(model, body, recv) else {
        return Ok(());
    };
    let width: u128 = match &model.types[elem] {
        ModelTy::Uint(UintKind::U32) => 32,
        ModelTy::Uint(UintKind::U64) => 64,
        _ => {
            return Err(BridgeError::AtomicValidation(AtomicDiagnostic::BitTestType {
                op: name.to_string(),
                detail: format!(
                    "requires a supported unsigned integer atomic element type (u32/u64); \
                     observed Atomic<{}>",
                    model_elem_name(model, elem)
                ),
            }));
        }
    };
    // A compile-time-constant out-of-range bit index (the runtime case is
    // hook-asserted). Integer literals reach the call as a direct constant
    // operand at every opt level, so a direct read suffices here.
    if let Some(Operand::Constant(id)) = args.get(1) {
        if let Const::Uint(_, value) = model.consts.resolve(*id) {
            if *value >= width {
                return Err(BridgeError::AtomicValidation(AtomicDiagnostic::BitTestType {
                    op: name.to_string(),
                    detail: format!("bit index {value} is out of range for a {width}-bit atomic"),
                }));
            }
        }
    }
    Ok(())
}
