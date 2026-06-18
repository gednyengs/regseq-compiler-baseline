//! Phase-4 synchronization lock-field recognition.
//!
//! Mirrors `bridge::interrupt::recognition` (item-driven): walk the interned
//! `#[rseq::driver]` structs, classify each field against the typed lock shells,
//! and intern the per-driver [`SyncResource`] facts into
//! `model.synchronization.drivers` plus the [`AttrData::SyncResource`] marker
//! into `domain_attrs.field_attrs`. The acquire-site facts are recognized later
//! by the Phase-5 op pass (`super::acquire`), once bodies are materialized.
//!
//! First slice (M1): the witness kind `IrqSpinLock<T>` only — detected by type
//! name, matching the name-based `Mmio`/`Interrupt` anchors. `Mutex`/`SpinLock`/
//! `Completion` are added in M2. The field's wrapper type *is* the kind (GRILL
//! `C01`/`D01`); there is no inference. The protected payload `T` is recorded as
//! `None` in M1 (the per-OS guard payload lowering — which needs `T` — lands in
//! M2/T07).

use regseq_model::{
    AttrData, Cardinality, DriverSynchronization, FieldIdx, FieldPath, ItemId, SyncKind,
    SyncResource, VariantIdx,
};
use rustc_ast::token::{Delimiter, TokenKind};
use rustc_ast::tokenstream::TokenTree;
use rustc_hir::{AttrArgs, Attribute};
use rustc_middle::ty::{Ty, TyCtxt};
use rustc_span::{def_id::DefId, Symbol as RustcSymbol};

use super::super::framework_type::recover_source_def_id;
use super::super::{BridgeCtxt, BridgeError, SyncDiagnostic};

/// Classify a driver-struct field type into its lock/signal kind, or `None`
/// when the field is not a synchronization resource. Name-based, matching the
/// `Mmio`/`Interrupt` anchors. The full first-slice surface (GRILL C01):
/// `Mutex`/`SpinLock`/`IrqSpinLock` (payload `T`) + `Completion` (no `T`).
pub(crate) fn classify_sync_kind<'tcx>(tcx: TyCtxt<'tcx>, ty: Ty<'tcx>) -> Option<SyncKind> {
    let name = ty.ty_adt_def().map(|adt| tcx.item_name(adt.did()))?;
    match name.as_str() {
        "Mutex" => Some(SyncKind::Mutex),
        "SpinLock" => Some(SyncKind::SpinLock),
        "IrqSpinLock" => Some(SyncKind::IrqSpinLock),
        "Completion" => Some(SyncKind::Completion),
        _ => None,
    }
}

/// Parse the decl-time `#[rseq::sync(cardinality = SingleWaiter)]` on a
/// `Completion` field. Absent → `SingleWaiter` (the default + only first-slice
/// cardinality). `AllWaiters` is rejected (deferred — GRILL C01/C02); any other
/// value — an unknown identifier OR a non-identifier (literal/malformed) token —
/// is also rejected (`UnknownCardinality`) rather than silently defaulting, so a
/// typo cannot mis-state the author's intent.
fn recognise_sync_cardinality(
    tcx: TyCtxt<'_>,
    def_id: DefId,
    field_name: &str,
) -> Result<Cardinality, BridgeError> {
    let path = [RustcSymbol::intern("rseq"), RustcSymbol::intern("sync")];
    let Some(attr) = tcx.get_attrs_by_path(def_id, &path).next() else {
        return Ok(Cardinality::SingleWaiter);
    };
    let Attribute::Unparsed(item) = attr else {
        return Ok(Cardinality::SingleWaiter);
    };
    let AttrArgs::Delimited(delim) = &item.args else {
        return Ok(Cardinality::SingleWaiter);
    };
    if delim.delim != Delimiter::Parenthesis {
        return Ok(Cardinality::SingleWaiter);
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
        if key.as_str() == "cardinality" {
            if let Some(TokenTree::Token(vtok, _)) = value {
                if let Some((vident, _)) = vtok.ident() {
                    return match vident.name.as_str() {
                        "SingleWaiter" => Ok(Cardinality::SingleWaiter),
                        "AllWaiters" => Err(BridgeError::SynchronizationValidation(
                            SyncDiagnostic::AllWaitersUnsupported {
                                field: field_name.to_string(),
                            },
                        )),
                        // A typo / unknown value must NOT silently bind the
                        // conservative default and mis-state intent — reject loud.
                        other => Err(BridgeError::SynchronizationValidation(
                            SyncDiagnostic::UnknownCardinality {
                                field: field_name.to_string(),
                                value: other.to_string(),
                            },
                        )),
                    };
                }
            }
            // `cardinality = <non-identifier>` (a literal / malformed token) is
            // not a recognized value — reject rather than silently default.
            return Err(BridgeError::SynchronizationValidation(
                SyncDiagnostic::UnknownCardinality {
                    field: field_name.to_string(),
                    value: "<non-identifier>".to_string(),
                },
            ));
        }
        match iter.peek() {
            Some(TokenTree::Token(tok, _)) if matches!(tok.kind, TokenKind::Comma) => {
                iter.next();
            }
            _ => break,
        }
    }
    Ok(Cardinality::SingleWaiter)
}

/// Is `ty` any recognized synchronization lock field? Used by the
/// device-lifecycle field split to route lock fields away from the user-data
/// classifier (whose `UserDataNotInitializable` rule would otherwise reject the
/// lock shell), exactly as `interrupt::is_interrupt_field` does.
pub(crate) fn is_sync_field<'tcx>(tcx: TyCtxt<'tcx>, ty: Ty<'tcx>) -> bool {
    classify_sync_kind(tcx, ty).is_some()
}

/// Phase-4 materialisation: intern every driver's lock-resource facts.
///
/// Runs AFTER `device_lifecycle::materialise_drivers` and
/// `interrupt::materialise_interrupts` (the driver map is the walk root). For
/// each driver struct, fields are walked in declaration order; each recognized
/// lock field interns a [`SyncResource`] (the index into the driver's
/// `resources` is its stable `lock_id`) and an [`AttrData::SyncResource`] field
/// marker. The acquire sites are left empty here — the Phase-5 op pass fills
/// them once bodies are materialized.
pub(crate) fn materialise_synchronization(cx: &mut BridgeCtxt<'_>) -> Result<(), BridgeError> {
    let tcx = cx.input.tcx;
    let targets: Vec<(ItemId, rustc_span::def_id::DefId)> = cx
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
        let adt = tcx.adt_def(d_def_id);
        let mut resources = Vec::new();
        let mut field_markers = Vec::new();
        for variant in adt.variants() {
            for (field_idx, field) in variant.fields.iter().enumerate() {
                let field_ty = tcx.type_of(field.did).instantiate_identity();
                let Some(kind) = classify_sync_kind(tcx, field_ty) else {
                    continue;
                };
                // `Completion` carries the decl-time cardinality (rejecting the
                // deferred `AllWaiters`); the lock kinds default `SingleWaiter`
                // (unused — cardinality is a `Completion`-only concern).
                let cardinality = if kind == SyncKind::Completion {
                    recognise_sync_cardinality(tcx, field.did, field.name.as_str())?
                } else {
                    Cardinality::SingleWaiter
                };
                let name = cx.model.strings.intern(field.name.as_str());
                let field_idx = FieldIdx::from_u32(field_idx as u32);
                resources.push(SyncResource {
                    name,
                    field: field_idx,
                    kind,
                    // The protected `T` is populated with the per-OS guard
                    // payload lowering; the first slice does not access it.
                    protected_type: None,
                    cardinality,
                });
                field_markers.push((
                    FieldPath {
                        item: item_id,
                        variant: VariantIdx::ZERO,
                        field: field_idx,
                    },
                    AttrData::SyncResource { kind },
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
        cx.model.synchronization.drivers.insert(
            item_id,
            DriverSynchronization {
                resources,
                acquire_sites: Vec::new(),
                completion_sites: Vec::new(),
                // Filled by the M3 context-marking pass (`super::context`).
                held_regions: Vec::new(),
            },
        );
    }
    Ok(())
}
