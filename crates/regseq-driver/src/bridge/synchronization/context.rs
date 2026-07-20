//! The synchronization context-marking analysis (M3 — the thesis component).
//!
//! This is the construct's load-bearing pass: the first that is **both a
//! consumer and a producer of context markings**. It runs once, after
//! [`super::acquire::recognise_acquire_sites`] has recorded every `lock()`
//! acquire site and `Completion` op site, and:
//!
//! - **Produces** the spinlock-held atomic-section marking (PLAN T10): for each
//!   `SpinLock`/`IrqSpinLock` acquire site, a [`SpinlockHeldRegion`] over the
//!   guard's (approximated, may-held, path-insensitive) MIR live range. The
//!   region is a BFS from the acquire block's CFG successors bounded by the
//!   site's `release_blocks` — it ends at `guard_drop_early`, not the syntactic
//!   scope. `Mutex` acquires produce no region (sleeping while holding a
//!   sleeping mutex is legal).
//!
//! - **Consumes** device-interrupt's `IsrBinding` IRQ-context marking (PLAN T11,
//!   §7.1/§7.2): in a body whose function item is an `#[rseq::isr]` handler,
//!   reject a sleeping/non-IRQ-safe acquire (`Mutex`/`SpinLock::lock`) or a
//!   `Completion::wait` (`forbidden-isr-blocking-op`); only `IrqSpinLock::lock`,
//!   `Completion::signal`, and `Completion::reset` pass.
//!
//! - **Consumes** timed-delay's may-sleep predicate (PLAN T12,
//!   may-sleep-while-held): reject a may-sleep call — a `Sleep`-intrinsic callee
//!   (the same `AttrData::Intrinsic(IntrinsicKind::Sleep)` predicate timed-delay
//!   uses), a `Mutex::lock`, or a `Completion::wait` — whose `Call`-terminator
//!   block lies inside a produced spinlock region (`sleep-in-atomic-section`).
//!
//! - **Rejects recursive same-lock acquisition** (PLAN T12, the reachable half
//!   of `forbidden-try-lock-or-unlock`): `lock(&self)` takes a shared borrow, so
//!   `let g1 = self.m.lock(); let g2 = self.m.lock();` *compiles* yet
//!   double-acquires a non-recursive lock — and the OS backends lower it to a
//!   real deadlocking lock (only the generic-c oracle would catch it, at
//!   runtime). A clean consumer of the T10 region: an acquire of lock-id `L`
//!   whose acquire block is inside an existing held region of the **same** `L`.
//!
//! **All intraprocedural/lexical** (GRILL `I04`/`C04`). The §7.1 check is
//! sound-but-incomplete: a transitively-IRQ-reachable illegal acquisition one
//! helper-call deeper escapes until the Open-4 interprocedural hardening lands —
//! so the conformance/audit output documents the limitation and never claims
//! "ISR-safety verified" (the same caveat `validate_isr_no_sleep` already
//! carries). Pure IR — no `TyCtxt`.

use std::collections::BTreeSet;

use regseq_model::{
    AttrData, BlockId, Body, CallCallee, CompletionOpKind, Instance, InstanceKind, IntrinsicKind,
    ItemId, RegseqModel, SpinlockHeldRegion, SyncKind, TerminatorKind,
};

use super::super::{BridgeError, SyncDiagnostic};

/// Run the context-marking analysis: produce the spinlock-held regions and run
/// the §7.1/§7.2/may-sleep-while-held/recursive-acquire checks. Aborts the
/// bridge (returns `Err`) on the first violation, so no IR for an offending spec
/// reaches a backend.
pub(crate) fn analyse_sync_contexts(model: &mut RegseqModel) -> Result<(), BridgeError> {
    // Phase 1 — analyse with only shared borrows of `model`, collecting the
    // produced regions to apply afterwards (the checks return `Err` inline).
    let mut produced: Vec<(ItemId, SpinlockHeldRegion)> = Vec::new();
    for i in 0..model.instances.len() {
        let inst_id = regseq_model::InstanceId::from_u32(i as u32);
        let instance = &model.instances[inst_id];
        let Some(body) = function_body(instance) else {
            continue;
        };
        let body_ref = model.instance_ref_of(inst_id);
        let is_isr = item_has_isr_binding(model, instance.item);

        // A body's lock/completion sites all belong to one driver (its receiver
        // is the driver struct); scan the drivers for the one carrying this
        // body's sites.
        for (driver_item, driver) in model.synchronization.drivers.iter() {
            let acquires: Vec<&regseq_model::AcquireSite> = driver
                .acquire_sites
                .iter()
                .filter(|s| s.body == body_ref)
                .collect();
            let completions: Vec<&regseq_model::CompletionSite> = driver
                .completion_sites
                .iter()
                .filter(|s| s.body == body_ref)
                .collect();
            if acquires.is_empty() && completions.is_empty() {
                continue;
            }

            // Per-acquire held region (for EVERY kind — the recursive-acquire
            // check needs Mutex regions too; only the spinning subset is
            // persisted as the thesis `SpinlockHeldRegion` marking).
            let regions: Vec<HeldRegion> = acquires
                .iter()
                .map(|site| HeldRegion {
                    lock_id: site.lock_id,
                    kind: kind_of(driver, site.lock_id),
                    acquire_block: site.acquire_block,
                    blocks: held_region_blocks(body, site.acquire_block, &site.release_blocks),
                })
                .collect();

            // T11 — §7.1 ISR-legality + §7.2 wait-context (consume IsrBinding).
            if is_isr {
                check_isr_legality(model, driver, &acquires, &completions)?;
            }

            // T12 — recursive same-lock acquisition (reachable; uses the region).
            check_recursive_acquire(model, driver, &acquires, &regions)?;

            // T12 — may-sleep-while-held over each spinning-kind region.
            check_sleep_in_atomic(model, driver, body, &completions, &regions)?;

            // T10 — persist the spinning-kind regions (the produced marking).
            for r in &regions {
                if matches!(
                    r.kind,
                    Some(SyncKind::SpinLock) | Some(SyncKind::IrqSpinLock)
                ) {
                    produced.push((
                        *driver_item,
                        SpinlockHeldRegion {
                            lock_id: r.lock_id,
                            kind: r.kind.expect("spinning kind matched above"),
                            body: body_ref.clone(),
                            acquire_block: r.acquire_block,
                            blocks: r.blocks.clone(),
                        },
                    ));
                }
            }
        }
    }

    // Phase 2 — apply the produced regions.
    for (driver_item, region) in produced {
        if let Some(driver) = model.synchronization.drivers.get_mut(&driver_item) {
            driver.held_regions.push(region);
        }
    }
    Ok(())
}

/// A per-acquire-site held region computed during analysis (the kind drives
/// both the may-sleep filter and which regions are persisted).
struct HeldRegion {
    lock_id: u32,
    kind: Option<SyncKind>,
    acquire_block: BlockId,
    blocks: Vec<BlockId>,
}

/// The basic blocks where the guard acquired at `acquire_block` is **held at
/// entry** — a BFS from the acquire block's CFG successors, stopping at (and
/// including) the `release_blocks`. The guard is held through a release block's
/// statements and dropped at its terminator, so the release block is in the
/// region but its successors are not. `acquire_block` itself is excluded (the
/// guard is not yet held while the acquire `Call` executes) — unless a CFG
/// back-edge re-reaches it while still held, which is itself a genuine
/// re-acquire and is correctly included.
fn held_region_blocks(
    body: &Body,
    acquire_block: BlockId,
    release_blocks: &[BlockId],
) -> Vec<BlockId> {
    let releases: BTreeSet<BlockId> = release_blocks.iter().copied().collect();
    let mut region: BTreeSet<BlockId> = BTreeSet::new();
    let mut work: Vec<BlockId> = body.basic_blocks[acquire_block]
        .terminator
        .successors()
        .collect();
    while let Some(b) = work.pop() {
        if !region.insert(b) {
            continue;
        }
        if releases.contains(&b) {
            // Boundary: held through this block, released at its terminator.
            continue;
        }
        for s in body.basic_blocks[b].terminator.successors() {
            work.push(s);
        }
    }
    // BTreeSet iteration is block-id sorted → deterministic block order.
    region.into_iter().collect()
}

/// §7.1 ISR-legality + §7.2 wait-context: in an `#[rseq::isr]` body, only the
/// IRQ-safe ops may run. `Mutex::lock` (sleeps) / `SpinLock::lock` (thread-only,
/// no IRQ save) / `Completion::wait` (sleeps) are rejected
/// (`forbidden-isr-blocking-op`); `IrqSpinLock::lock` / `signal` / `reset` pass.
fn check_isr_legality(
    model: &RegseqModel,
    driver: &regseq_model::DriverSynchronization,
    acquires: &[&regseq_model::AcquireSite],
    completions: &[&regseq_model::CompletionSite],
) -> Result<(), BridgeError> {
    for site in acquires {
        let kind = kind_of(driver, site.lock_id);
        let op = match kind {
            Some(SyncKind::Mutex) => "Mutex::lock",
            Some(SyncKind::SpinLock) => "SpinLock::lock",
            // IrqSpinLock is the ISR-safe acquire — legal in IRQ context.
            _ => continue,
        };
        return Err(forbidden_isr_blocking_op(model, driver, site.lock_id, op));
    }
    for site in completions {
        if site.op == CompletionOpKind::Wait {
            return Err(forbidden_isr_blocking_op(
                model,
                driver,
                site.lock_id,
                "Completion::wait",
            ));
        }
        // signal/reset are ISR-safe (I07) — legal.
    }
    Ok(())
}

/// Recursive same-lock acquisition: an acquire of lock-id `L` whose acquire
/// block lies inside an existing held region of the same `L` (the lock is
/// re-acquired while already held — a non-recursive double-acquire / deadlock
/// the OS backends would lower to a real lock).
fn check_recursive_acquire(
    model: &RegseqModel,
    driver: &regseq_model::DriverSynchronization,
    acquires: &[&regseq_model::AcquireSite],
    regions: &[HeldRegion],
) -> Result<(), BridgeError> {
    for region in regions {
        let held_in: BTreeSet<BlockId> = region.blocks.iter().copied().collect();
        for site in acquires {
            // Same lock, acquired inside this lock's held region → re-entrant.
            if site.lock_id == region.lock_id && held_in.contains(&site.acquire_block) {
                return Err(BridgeError::SynchronizationValidation(
                    SyncDiagnostic::ForbiddenTryLockOrUnlock {
                        field: lock_field_name(model, driver, site.lock_id),
                        detail: "recursive acquire — the same lock is acquired while already held \
                                 (the first-slice locks are non-recursive; this self-deadlocks)"
                            .to_string(),
                    },
                ));
            }
        }
    }
    Ok(())
}

/// may-sleep-while-held: a may-sleep call inside a spinning-kind held region.
/// The may-sleep predicate is { a `Sleep`-intrinsic callee (the timed-delay
/// predicate), a `Mutex::lock` acquire, a `Completion::wait` } — the predicate
/// agrees with timed-delay's.
fn check_sleep_in_atomic(
    model: &RegseqModel,
    driver: &regseq_model::DriverSynchronization,
    body: &Body,
    completions: &[&regseq_model::CompletionSite],
    regions: &[HeldRegion],
) -> Result<(), BridgeError> {
    // Completion::wait blocks in this body (a may-sleep op).
    let wait_blocks: BTreeSet<BlockId> = completions
        .iter()
        .filter(|s| s.op == CompletionOpKind::Wait)
        .map(|s| s.block)
        .collect();
    // Mutex::lock acquire blocks in this body (a may-sleep op).
    let mutex_blocks: BTreeSet<BlockId> = regions
        .iter()
        .filter(|r| r.kind == Some(SyncKind::Mutex))
        .map(|r| r.acquire_block)
        .collect();

    for region in regions {
        // Only a spinning-kind held region is an atomic section.
        if !matches!(
            region.kind,
            Some(SyncKind::SpinLock) | Some(SyncKind::IrqSpinLock)
        ) {
            continue;
        }
        let held_field = lock_field_name(model, driver, region.lock_id);
        for &b in &region.blocks {
            let op = if wait_blocks.contains(&b) {
                Some("Completion::wait")
            } else if mutex_blocks.contains(&b) {
                Some("Mutex::lock")
            } else if block_calls_sleep_intrinsic(model, body, b) {
                Some("regseq::delay::sleep")
            } else {
                None
            };
            if let Some(op) = op {
                return Err(BridgeError::SynchronizationValidation(
                    SyncDiagnostic::SleepInAtomicSection {
                        op: op.to_string(),
                        lock_field: held_field,
                    },
                ));
            }
        }
    }
    Ok(())
}

/// Whether block `b`'s terminator is a direct call to a `Sleep`-intrinsic
/// facade (the same predicate `validate_isr_no_sleep` uses).
fn block_calls_sleep_intrinsic(model: &RegseqModel, body: &Body, b: BlockId) -> bool {
    let TerminatorKind::Call {
        callee: CallCallee::Direct(callee_id),
        ..
    } = &body.basic_blocks[b].terminator.kind
    else {
        return false;
    };
    let callee_item = model.instances[*callee_id].item;
    item_attr_matches(model, callee_item, |a| {
        matches!(a, AttrData::Intrinsic(IntrinsicKind::Sleep))
    })
}

/// Build the `forbidden-isr-blocking-op` diagnostic for `op` on the lock/
/// completion at `lock_id`.
fn forbidden_isr_blocking_op(
    model: &RegseqModel,
    driver: &regseq_model::DriverSynchronization,
    lock_id: u32,
    op: &str,
) -> BridgeError {
    BridgeError::SynchronizationValidation(SyncDiagnostic::ForbiddenIsrBlockingOp {
        field: lock_field_name(model, driver, lock_id),
        op: op.to_string(),
        context: "rseq::isr".to_string(),
    })
}

/// The kind of the resource at `lock_id`, if present.
fn kind_of(driver: &regseq_model::DriverSynchronization, lock_id: u32) -> Option<SyncKind> {
    driver.resources.get(lock_id as usize).map(|r| r.kind)
}

/// The C-visible field name of the lock/completion at `lock_id` (for diagnostics).
fn lock_field_name(
    model: &RegseqModel,
    driver: &regseq_model::DriverSynchronization,
    lock_id: u32,
) -> String {
    driver
        .resources
        .get(lock_id as usize)
        .map(|r| model.strings.resolve(r.name).to_string())
        .unwrap_or_else(|| "<lock>".to_string())
}

/// Whether `item` carries the `#[rseq::isr]` `IsrBinding` marker (bounds-checked
/// — a late callee item beyond the attr table is treated as unmarked). The exact
/// gate `validate_isr_no_sleep` uses.
fn item_has_isr_binding(model: &RegseqModel, item: ItemId) -> bool {
    item_attr_matches(model, item, |a| matches!(a, AttrData::IsrBinding { .. }))
}

/// Whether `item` carries an attribute `pred` selects (bounds-checked).
fn item_attr_matches(model: &RegseqModel, item: ItemId, pred: impl Fn(&AttrData) -> bool) -> bool {
    item.index() < model.domain_attrs.item_attrs.len()
        && model.domain_attrs.item_attrs[item].iter().any(pred)
}

/// The materialized body of a function instance, if any.
fn function_body(instance: &Instance) -> Option<&Body> {
    match &instance.kind {
        InstanceKind::Function(data) => data.body.as_ref(),
        _ => None,
    }
}
