//! Phase-5 op / acquire-site recognition.
//!
//! Walks the materialized bodies and records, per recognized lock acquisition,
//! an [`AcquireSite`] into the `SynchronizationTables` sibling table: which lock
//! (`lock_id`, recovered from the receiver field), the enclosing body
//! (`InstanceRef`), the acquire block, the guard local the `lock()` binds, and
//! the block(s) where the guard's drop-glue `Call` releases it.
//!
//! Pure IR — no `TyCtxt`. An acquire is `lock()` whose receiver place resolves
//! to a driver field that Phase-4 recognized as a `SyncResource` (the recognizer
//! anchors on the marked field; the `lock` method name is a redundant gate). A
//! release is the guard local's drop-glue `Call`: rustc's `Drop` is lowered to a
//! synthetic `Call` to a `SyntheticItemKind::DropGlue` instance whose argument is
//! `&raw mut guard` — a synthetic `RawPtr` temp — so the dropped place is chased
//! back through that temp's unique definition to the guard local (it is NOT the
//! call's first argument directly).
//!
//! All recognition is **intraprocedural/lexical** (GRILL `C04`/`I04`): a guard
//! whose acquire and release live in one body. The interprocedural case (a guard
//! held across a call) is the deferred Open-4 hardening.

use regseq_model::{
    AcquireSite, AssertKind, Body, CallCallee, CompletionOpKind, CompletionSite, DefPathData,
    FieldIdx, InstanceId, InstanceKind, ItemId, NonDivergingIntrinsic, Operand, Place, PlaceElem,
    RegseqModel, Rvalue, StatementKind, SyncKind, SyncOp, SyntheticItemKind, TerminatorKind, Ty,
};

use super::super::{BridgeError, SyncDiagnostic};

/// Bound on the reaching-def chase (mirrors the backend's
/// `resolve_receiver_src` depth guard).
const CHASE_DEPTH: u32 = 8;

/// Recognize lock acquire/release sites across all materialized bodies and
/// record their facts into `model.synchronization.drivers[*].acquire_sites`.
pub(crate) fn recognise_acquire_sites(model: &mut RegseqModel) -> Result<(), BridgeError> {
    // Phase 1 — collect (driver item, site) with only shared borrows of `model`.
    let mut found: Vec<(ItemId, AcquireSite)> = Vec::new();
    let mut found_completions: Vec<(ItemId, CompletionSite)> = Vec::new();
    for i in 0..model.instances.len() {
        let inst_id = InstanceId::from_u32(i as u32);
        let body = match &model.instances[inst_id].kind {
            InstanceKind::Function(data) => match &data.body {
                Some(body) => body,
                None => continue,
            },
            _ => continue,
        };
        collect_sites_in_body(model, inst_id, body, &mut found, &mut found_completions)?;
    }

    // Phase 2 — apply. A site whose driver has no recognized synchronization
    // surface (e.g. the receiver field was not a lock) is silently dropped by
    // `collect_sites_in_body` already, so every entry here has a live driver.
    for (driver_item, site) in found {
        if let Some(driver) = model.synchronization.drivers.get_mut(&driver_item) {
            driver.acquire_sites.push(site);
        }
    }
    for (driver_item, site) in found_completions {
        if let Some(driver) = model.synchronization.drivers.get_mut(&driver_item) {
            driver.completion_sites.push(site);
        }
    }
    Ok(())
}

/// Scan one body's blocks for `lock()` acquires, recording an [`AcquireSite`]
/// per recognized one (with its paired release blocks).
fn collect_sites_in_body(
    model: &RegseqModel,
    inst_id: InstanceId,
    body: &Body,
    found: &mut Vec<(ItemId, AcquireSite)>,
    found_completions: &mut Vec<(ItemId, CompletionSite)>,
) -> Result<(), BridgeError> {
    for b in 0..body.basic_blocks.len() {
        let block_id = regseq_model::BlockId::from_u32(b as u32);
        let TerminatorKind::Call {
            callee,
            args,
            destination,
            ..
        } = &body.basic_blocks[block_id].terminator.kind
        else {
            continue;
        };
        let Some(leaf) = callee_leaf(model, callee) else {
            continue;
        };
        // The completion op kind for a `wait`/`signal`/`reset` leaf, else none.
        let completion_op = match leaf {
            "wait" => Some(CompletionOpKind::Wait),
            "signal" => Some(CompletionOpKind::Signal),
            "reset" => Some(CompletionOpKind::Reset),
            _ => None,
        };
        if leaf != "lock" && completion_op.is_none() {
            continue;
        }
        // Resolve the receiver (first arg) to the recognized sync field; anchor
        // on the field's resource (GRILL D01 — the authoritative gate).
        let Some(recv) = args.first() else {
            continue;
        };
        let Some(place) = chase_to_place(body, recv, 0) else {
            continue;
        };
        let Some((driver_item, field_idx)) = driver_field_of(model, body, &place) else {
            continue;
        };
        let Some(lock_id) = lock_id_of(model, driver_item, field_idx) else {
            continue;
        };
        let kind = resource_kind_of(model, driver_item, lock_id);
        match completion_op {
            // `wait`/`signal`/`reset` on a `Completion` resource → a point op.
            Some(op) => {
                if kind != Some(SyncKind::Completion) {
                    continue;
                }
                // `wait` discards its `Result` this slice (the timeout→Err is not
                // modeled); a body that CONSUMES the result would read an unset
                // destination on every backend — reject loud (I06) rather than
                // emit wrong code.
                if op == CompletionOpKind::Wait
                    && wait_result_consumed(model, body, destination.local)
                {
                    return Err(BridgeError::SynchronizationValidation(
                        SyncDiagnostic::CompletionWaitResultConsumed {
                            field: completion_field_name(model, driver_item, lock_id),
                        },
                    ));
                }
                found_completions.push((
                    driver_item,
                    CompletionSite {
                        lock_id,
                        op,
                        body: model.instance_ref_of(inst_id),
                        block: block_id,
                    },
                ));
            }
            // `lock()` on a lock resource → an acquire (with its guard release).
            None => {
                if kind == Some(SyncKind::Completion) {
                    continue; // a `Completion` has no `lock()`.
                }
                let guard_local = destination.local;
                let release_blocks = find_release_blocks(model, body, guard_local);
                found.push((
                    driver_item,
                    AcquireSite {
                        lock_id,
                        op: SyncOp::LockAcquire,
                        body: model.instance_ref_of(inst_id),
                        acquire_block: block_id,
                        guard_local,
                        release_blocks,
                    },
                ));
            }
        }
    }
    Ok(())
}

/// Whether a `Completion::wait`'s result local `dest` is CONSUMED (read as a
/// value, directly OR through a borrow/copy) rather than discarded. The first
/// slice discards the result, so a genuine read means the body branches on the
/// timeout outcome — out of slice.
///
/// Two passes. **Taint** (pass 1, fixpoint): `dest` plus every local derived
/// from a tainted local by `&`/`&raw`/move/copy/cast (so a borrowed `r` reached
/// via `let r = wait(); r.is_ok()` and any aliasing chain is tracked). **Use**
/// (pass 2): a tainted local read by a `Discriminant` / arithmetic / aggregate
/// rvalue, a `SwitchInt`, an `Assert`, or a NON-drop-glue `Call` argument is a
/// consume. The synthetic drop-glue `Call` (the `&raw mut dest` discard) is the
/// only permitted use — its callee is `SyntheticItemKind::DropGlue`.
fn wait_result_consumed(model: &RegseqModel, body: &Body, dest: regseq_model::LocalId) -> bool {
    use std::collections::BTreeSet;
    // Pass 1 — taint closure over alias/copy edges.
    let mut tainted: BTreeSet<regseq_model::LocalId> = BTreeSet::new();
    tainted.insert(dest);
    loop {
        let mut grew = false;
        for bb in body.basic_blocks.iter() {
            for stmt in bb.statements.iter() {
                let StatementKind::Assign(d, rv) = &stmt.kind else {
                    continue;
                };
                if !d.projection.is_empty() || tainted.contains(&d.local) {
                    continue;
                }
                let src = match rv {
                    Rvalue::Ref(_, p) | Rvalue::RawPtr(_, p) => Some(p.local),
                    Rvalue::Use(Operand::Copy(p) | Operand::Move(p))
                    | Rvalue::Cast(_, Operand::Copy(p) | Operand::Move(p), _) => Some(p.local),
                    _ => None,
                };
                if src.is_some_and(|s| tainted.contains(&s)) {
                    tainted.insert(d.local);
                    grew = true;
                }
            }
        }
        if !grew {
            break;
        }
    }
    // Pass 2 — a consuming use of any tainted local (alias/copy edges, handled
    // in pass 1, are NOT themselves consumes).
    let reads = |op: &Operand| matches!(op, Operand::Copy(p) | Operand::Move(p) if tainted.contains(&p.local));
    for bb in body.basic_blocks.iter() {
        for stmt in bb.statements.iter() {
            match &stmt.kind {
                StatementKind::Assign(d, rv) => {
                    // A move/borrow of a tainted local into a WHOLE local is
                    // taint-building (pass 1 propagated it); into a PROJECTED place
                    // (`pair.0 = move tmp`) the value escapes into an aggregate the
                    // taint set does not track — that is a consume.
                    let escapes_to_aggregate = !d.projection.is_empty();
                    let consumed = match rv {
                        Rvalue::Discriminant(p) => tainted.contains(&p.local),
                        Rvalue::BinaryOp(_, a, b) => reads(a) || reads(b),
                        Rvalue::UnaryOp(_, a) => reads(a),
                        Rvalue::Aggregate(_, ops) => ops.iter().any(&reads),
                        Rvalue::Repeat(a, _) => reads(a),
                        Rvalue::Use(o) | Rvalue::Cast(_, o, _) => reads(o) && escapes_to_aggregate,
                        Rvalue::Ref(_, p) | Rvalue::RawPtr(_, p) => {
                            tainted.contains(&p.local) && escapes_to_aggregate
                        }
                    };
                    if consumed {
                        return true;
                    }
                }
                // A non-diverging intrinsic reading a tainted local (e.g.
                // `copy_nonoverlapping(&r, …)` reading the result's bytes through
                // a tainted ref) is also a consume.
                StatementKind::Intrinsic(ndi) => {
                    let touched = match ndi {
                        NonDivergingIntrinsic::Assume(o) => reads(o),
                        NonDivergingIntrinsic::CopyNonOverlapping(c) => {
                            reads(&c.src) || reads(&c.dst) || reads(&c.count)
                        }
                    };
                    if touched {
                        return true;
                    }
                }
                _ => {}
            }
        }
        match &bb.terminator.kind {
            TerminatorKind::SwitchInt { discr, .. } => {
                if reads(discr) {
                    return true;
                }
            }
            TerminatorKind::Call { callee, args, .. } => {
                // The drop-glue Call IS the discard; any other call reading a
                // tainted local — as an argument OR the indirect callee pointer —
                // consumes the result.
                if !is_drop_glue_callee(model, callee) {
                    let callee_read = matches!(callee, CallCallee::Indirect(o) if reads(o));
                    if callee_read || args.iter().any(&reads) {
                        return true;
                    }
                }
            }
            TerminatorKind::Assert { cond, msg, .. } => {
                let msg_read = match msg {
                    AssertKind::BoundsCheck { len, index } => reads(len) || reads(index),
                    AssertKind::Overflow(_, a, b) => reads(a) || reads(b),
                    AssertKind::OverflowNeg(o)
                    | AssertKind::DivisionByZero(o)
                    | AssertKind::RemainderByZero(o)
                    | AssertKind::InvalidEnumConstruction(o) => reads(o),
                    AssertKind::MisalignedPointerDereference { required, found } => {
                        reads(required) || reads(found)
                    }
                    AssertKind::NullPointerDereference => false,
                };
                if reads(cond) || msg_read {
                    return true;
                }
            }
            _ => {}
        }
    }
    false
}

/// The C-visible name of the `Completion` field at `lock_id` (for diagnostics).
fn completion_field_name(model: &RegseqModel, driver_item: ItemId, lock_id: u32) -> String {
    model
        .synchronization
        .drivers
        .get(&driver_item)
        .and_then(|d| d.resources.get(lock_id as usize))
        .map(|r| model.strings.resolve(r.name).to_string())
        .unwrap_or_else(|| "<completion>".to_string())
}

/// The block(s) whose terminator releases `guard_local` — its `Drop` (a
/// synthetic drop-glue `Call` whose dropped place chases back to the guard) OR
/// an explicit `guard_drop_early(self)` call consuming the guard — in block
/// order.
fn find_release_blocks(
    model: &RegseqModel,
    body: &Body,
    guard_local: regseq_model::LocalId,
) -> Vec<regseq_model::BlockId> {
    let mut blocks = Vec::new();
    for b in 0..body.basic_blocks.len() {
        let block_id = regseq_model::BlockId::from_u32(b as u32);
        let TerminatorKind::Call { callee, args, .. } =
            &body.basic_blocks[block_id].terminator.kind
        else {
            continue;
        };
        // A drop-glue `Call` (scope-end drop) or an explicit `guard_drop_early`
        // consume — both release the guard.
        let is_release = is_drop_glue_callee(model, callee)
            || callee_leaf(model, callee) == Some("guard_drop_early");
        if !is_release {
            continue;
        }
        let Some(arg) = args.first() else {
            continue;
        };
        // The released place is `&raw mut guard` (drop glue) or `move guard`
        // (drop_early); chase back to the guard local.
        let Some(place) = chase_to_place(body, arg, 0) else {
            continue;
        };
        if place.local == guard_local && place.projection.is_empty() {
            blocks.push(block_id);
        }
    }
    blocks
}

/// The leaf (value-namespace) name of a `Direct` callee.
fn callee_leaf<'a>(model: &'a RegseqModel, callee: &CallCallee) -> Option<&'a str> {
    let CallCallee::Direct(inst) = callee else {
        return None;
    };
    let item = model.instances[*inst].item;
    let seg = model.items[item].def_path.segments.last()?;
    match seg.data {
        DefPathData::ValueNs(s) => Some(model.strings.resolve(s)),
        _ => None,
    }
}

/// The kind of the resource at `lock_id` on `driver_item`, if any.
fn resource_kind_of(model: &RegseqModel, driver_item: ItemId, lock_id: u32) -> Option<SyncKind> {
    model
        .synchronization
        .drivers
        .get(&driver_item)?
        .resources
        .get(lock_id as usize)
        .map(|r| r.kind)
}

/// Whether a `Direct` callee is a synthetic drop-glue instance.
fn is_drop_glue_callee(model: &RegseqModel, callee: &CallCallee) -> bool {
    let CallCallee::Direct(inst) = callee else {
        return false;
    };
    let item = model.instances[*inst].item;
    matches!(
        model.item_synthetic_kinds[item],
        SyntheticItemKind::DropGlue { .. }
    )
}

/// Chase a borrow operand back to the place it ultimately borrows, through
/// bounded `Ref`/`RawPtr`/`Use` temp chains (mirrors the backend's
/// `resolve_receiver_src`; at low `mir_opt_level` each temp is single-assignment
/// so the unique whole-body definition is the reaching def).
fn chase_to_place(body: &Body, op: &Operand, depth: u32) -> Option<Place> {
    if depth > CHASE_DEPTH {
        return None;
    }
    let place = match op {
        Operand::Copy(p) | Operand::Move(p) => p,
        Operand::Constant(_) => return None,
    };
    match place.projection.last() {
        // Already names a field/element directly.
        Some(PlaceElem::Field(..) | PlaceElem::Index(..) | PlaceElem::ConstantIndex { .. }) => {
            return Some(place.clone())
        }
        // A bare local (or a deref-only place): find its unique definition.
        None => {}
        Some(_) => return Some(place.clone()),
    }
    if !place.projection.is_empty() {
        // A deref-headed place with no trailing field — use it as-is.
        return Some(place.clone());
    }
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
        Some(Rvalue::Use(inner)) => chase_to_place(body, inner, depth + 1),
        Some(_) => None,
        // No `Assign` definition: the local is defined by a `Call` terminator's
        // destination (a `lock()` guard) or is itself a call argument — it names
        // the place directly, so return it as-is. This is the `guard_drop_early`
        // path: `Guard::guard_drop_early(move guard)` passes the guard local
        // directly (no intermediate `RawPtr` temp, unlike drop glue).
        None => Some(place.clone()),
    }
}

/// Walk a place's projection and recover `(driver struct item, field index)` of
/// the field it names — the lock field for a `self.lock` receiver.
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

/// The `lock_id` (resource index) of the lock declared at `field` on
/// `driver_item`, or `None` when that field is not a recognized lock.
fn lock_id_of(model: &RegseqModel, driver_item: ItemId, field: FieldIdx) -> Option<u32> {
    let driver = model.synchronization.drivers.get(&driver_item)?;
    driver
        .resources
        .iter()
        .position(|r| r.field == field)
        .map(|i| i as u32)
}
