//! Phase-5 `self.work.submit()` direct-field-path recognition
//! (deferred-work-item-scheduling construct, GRILL T06).
//!
//! The submit op's *callee* legality (it must be a method on the `Work` shell)
//! is checked by `recognition::validate_deferred_work_signatures` against the
//! callee signature. This pass is the *call-site* half: it walks the materialized
//! bodies and, per recognized `deferred_work_submit` call, recovers the
//! receiver's field path back to the bound `Work` resource and records a
//! [`SubmitSite`] on it. A receiver that is NOT a direct field path to a bound
//! `Work` — a spoof, a projection that lands elsewhere, or a local alias — is
//! rejected (`DWS-ERR-SPOOFED-RECEIVER`).
//!
//! Direct-field-path-only (GRILL D11, M2/B-3): v1 recognizes `submit` ONLY on a
//! direct field path (`self.work.submit()` / `driver.work.submit()`). A local
//! alias (`let w = &self.work; w.submit()`) or any projection that reaches the
//! `Work` indirectly is the design-in form (GRILL Out-of-Scope: "submit
//! alias/projection receiver recognition"), so it is REJECTED. The recovery in
//! [`chase_to_place`] is therefore a strict single-auto-ref-hop (no `Use`/copy
//! chasing) — the deliberate divergence from the sync `acquire` mirror, which
//! DOES chase alias chains for its own contract.
//!
//! The built direct form is **plain method-call syntax** on the field —
//! `self.work.submit()` / `driver.work.submit()` — which lowers to exactly the
//! single auto-ref this recovery accepts. Explicit-borrow / UFCS spellings of the
//! "same" call (`(&self.work).submit()`, `Work::submit(&self.work)`) insert an
//! extra reborrow temp and so REJECT; that is intentional — v1's direct form is
//! the plain method call, not every syntactically-direct borrowed expression
//! (the corpus witness, E1000 `schedule_work(&adapter->reset_task)`, is the
//! plain form).
//!
//! Pure IR — no `TyCtxt`. Mirrors `synchronization::acquire::recognise_acquire_sites`
//! (the receiver-source / projection recovery is duplicated, not extracted, per
//! the mirror-not-extract discipline GRILL D06): the clock signature-check phase
//! sees only the callee item / receiver type and cannot inspect a call's body
//! projections nor record per-`Work` call sites (Codex pre-finalize BLOCKING-2).

use regseq_model::{
    AttrData, BlockId, Body, CallCallee, FieldIdx, InstanceId, InstanceKind, IntrinsicKind, ItemId,
    Operand, Place, PlaceElem, RegseqModel, Rvalue, StatementKind, SubmitSite, SubmitVariant,
    TerminatorKind, Ty,
};

use super::super::{BridgeError, DeferredWorkDiagnostic};

/// Walk every materialized body and record a [`SubmitSite`] per recognized
/// `self.work.submit()` call on its bound `Work` resource. Rejects a submit
/// whose receiver is not a direct field path to a bound `Work`
/// (`DWS-ERR-SPOOFED-RECEIVER`). Runs in the same post-body-materialization /
/// post-sort window as `synchronization::recognise_acquire_sites`.
pub(crate) fn recognise_submit_sites(model: &mut RegseqModel) -> Result<(), BridgeError> {
    // Phase 1 — collect (driver item, field) with only shared borrows of `model`.
    let mut found: Vec<(ItemId, FieldIdx)> = Vec::new();
    for i in 0..model.instances.len() {
        let inst_id = InstanceId::from_u32(i as u32);
        let body = match &model.instances[inst_id].kind {
            InstanceKind::Function(data) => match &data.body {
                Some(body) => body,
                None => continue,
            },
            _ => continue,
        };
        for b in 0..body.basic_blocks.len() {
            let block_id = BlockId::from_u32(b as u32);
            let TerminatorKind::Call { callee, args, .. } =
                &body.basic_blocks[block_id].terminator.kind
            else {
                continue;
            };
            if !is_submit_callee(model, callee) {
                continue;
            }
            // The submit receiver is the first argument (`&self.work`); recover
            // its direct field path back to a bound `Work` resource.
            let resolved = args
                .first()
                .and_then(|recv| chase_to_place(body, recv))
                .and_then(|place| driver_field_of(model, body, &place))
                .filter(|(driver_item, field_idx)| is_bound_work(model, *driver_item, *field_idx));
            let Some((driver_item, field_idx)) = resolved else {
                // The receiver does not resolve to a DIRECT field path to a bound
                // `Work` — a spoof, a projection that lands elsewhere, or a local
                // alias (`let w = &self.work; w.submit()`). v1 recognizes `submit`
                // only on a direct field path (GRILL D11); the alias/projection
                // form is design-in, not built (Out-of-Scope), so it is rejected.
                return Err(BridgeError::DeferredWorkValidation(
                    DeferredWorkDiagnostic::SpoofedReceiver {
                        receiver: "a `submit` receiver that does not resolve to a bound `Work` field"
                            .to_string(),
                    },
                ));
            };
            found.push((driver_item, field_idx));
        }
    }

    // Phase 2 — apply. Every entry resolved to a bound `Work` above.
    for (driver_item, field_idx) in found {
        if let Some(driver) = model.deferred_work.drivers.get_mut(&driver_item) {
            if let Some(res) = driver.resources.iter_mut().find(|r| r.field == field_idx) {
                res.submit_sites.push(SubmitSite {
                    variant: SubmitVariant::Submit,
                });
            }
        }
    }
    Ok(())
}

/// Whether a `Direct` callee carries the `deferred_work_submit` intrinsic marker
/// (recorded on the callee item by `materialise_intrinsics`).
fn is_submit_callee(model: &RegseqModel, callee: &CallCallee) -> bool {
    let CallCallee::Direct(inst) = callee else {
        return false;
    };
    let item = model.instances[*inst].item;
    model.domain_attrs.item_attrs[item]
        .iter()
        .any(|a| matches!(a, AttrData::Intrinsic(IntrinsicKind::DeferredWork(_))))
}

/// Whether `(driver_item, field)` names a bound `Work` resource.
fn is_bound_work(model: &RegseqModel, driver_item: ItemId, field: FieldIdx) -> bool {
    model
        .deferred_work
        .drivers
        .get(&driver_item)
        .is_some_and(|d| d.resources.iter().any(|r| r.field == field))
}

/// Recover the place a submit-receiver operand DIRECTLY borrows — the SINGLE
/// auto-ref the method call inserts (`_t = &(*self).work`). v1 recognizes
/// `Work::submit` ONLY on a direct field path (GRILL D11): the receiver temp's
/// sole reaching def must be ONE `Ref`/`RawPtr` of the borrowed place; that place
/// is then resolved by `driver_field_of` and must name a bound `Work`.
///
/// This is the deliberate **B-3 divergence from the sync `acquire` mirror**: the
/// sync pass chases `Use`/reborrow temp CHAINS (its own aliasing contract follows
/// copies), but `submit` does NOT — a local alias lowers either to a reborrow
/// `&*w` (whose borrowed place is `(*w)`, a `&Work` deref that `driver_field_of`
/// cannot resolve to a field) or to a `Use` copy of the alias local (rejected
/// here, since only a `Ref`/`RawPtr` def is a direct borrow). Both are the
/// design-in alias form (GRILL Out-of-Scope: "submit alias/projection receiver
/// recognition"), so both reject. Empirically (the live `mir_opt_level`), the
/// direct `self.work.submit()` is exactly one `Ref` hop with no intervening
/// `Use`, so this strict single-hop recovery accepts it and rejects every alias.
fn chase_to_place(body: &Body, op: &Operand) -> Option<Place> {
    let place = match op {
        Operand::Copy(p) | Operand::Move(p) => p,
        Operand::Constant(_) => return None,
    };
    // A receiver passed with its own projection is already a place (not an
    // auto-ref temp) — hand it to `driver_field_of` as-is.
    if !place.projection.is_empty() {
        return Some(place.clone());
    }
    // Otherwise the receiver is a temp; its SOLE def must be the auto-ref
    // `Ref`/`RawPtr` of the borrowed place. A `Use`/copy/other def is an alias or
    // a non-direct-borrow ⇒ NOT a direct field path ⇒ reject (no chasing, D11).
    let local = place.local;
    let mut def: Option<&Rvalue> = None;
    for bb in body.basic_blocks.iter() {
        for stmt in bb.statements.iter() {
            let StatementKind::Assign(dst, rhs) = &stmt.kind else {
                continue;
            };
            if dst.local != local || !dst.projection.is_empty() {
                continue;
            }
            if def.is_some() {
                return None; // ambiguous
            }
            def = Some(rhs);
        }
    }
    match def {
        Some(Rvalue::Ref(_, src) | Rvalue::RawPtr(_, src)) => Some(src.clone()),
        _ => None,
    }
}

/// Walk a place's projection and recover `(driver struct item, field index)` of
/// the field it names — the `Work` field for a `self.work` receiver. Duplicated
/// from `synchronization::acquire::driver_field_of` per the mirror-not-extract
/// discipline.
fn driver_field_of(model: &RegseqModel, body: &Body, place: &Place) -> Option<(ItemId, FieldIdx)> {
    let mut ty = body.local_decls[place.local].ty;
    let mut result: Option<(ItemId, FieldIdx)> = None;
    for elem in place.projection.iter() {
        match elem {
            PlaceElem::Deref => {
                ty = match &model.types[ty] {
                    Ty::Ref(p, _) | Ty::Ptr(p, _) => *p,
                    _ => return None,
                };
            }
            PlaceElem::Field(idx, fty) => {
                let item = match &model.types[ty] {
                    Ty::Struct { item, .. } => *item,
                    _ => return None,
                };
                result = Some((item, *idx));
                ty = *fty;
            }
            _ => return None,
        }
    }
    result
}
