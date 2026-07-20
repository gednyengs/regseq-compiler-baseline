//! Synchronization (locks) lowering for the zephyr_c backend (T07, C02).
//!
//! Mirror of the linux_c sync lowering (shares no code per the F-1
//! independent-lowering contract), with the Zephyr C02 primitives:
//!
//!  * the lock FIELD lowers to the per-OS lock object ([`lock_field_decl`]);
//!  * `Mutex` → `k_mutex_lock(K_FOREVER)` / `k_mutex_unlock`; `SpinLock` AND
//!    `IrqSpinLock` BOTH lower to the key-carrying `k_spin_lock`/`k_spin_unlock`
//!    (the G-SYN-14 collapse: Zephyr's spinlock is always IRQ-off — the surface
//!    keeps the two kinds distinct, anti-narrowing, I06); `Completion` →
//!    `k_sem_take(K_NSEC)`/`k_sem_give`/`k_sem_reset`;
//!  * the guard hidden state is `k_spinlock_key_t key`, ASSIGNED FROM the
//!    `k_spin_lock` return and consumed by `k_spin_unlock` (contrast Linux's
//!    `flags`, which `spin_lock_irqsave` writes).
//!
//! C02 spellings version-pin-rechecked against Zephyr v4.4.0 (see
//! `docs/discussions/synchronization/T07-version-pin-recheck.md`).

use std::collections::BTreeMap;

use regseq_model::{
    BlockId, CallCallee, CompletionOpKind, InstanceId, InstanceKind, LocalId, Operand, RegseqModel,
    SyncKind, TerminatorKind, Ty, TyId,
};

use super::body::{field_c_name, local_name, operand_expr_typed, LowerCx};
use super::naming::c_ident;
use super::EmitError;

/// The per-OS lock-FIELD C declaration for a sync resource of `kind` named `f`.
/// `SpinLock`/`IrqSpinLock` share `struct k_spinlock` (the collapse).
pub(crate) fn lock_field_decl(kind: SyncKind, f: &str) -> String {
    match kind {
        SyncKind::Mutex => format!("struct k_mutex {f}"),
        SyncKind::SpinLock | SyncKind::IrqSpinLock => format!("struct k_spinlock {f}"),
        SyncKind::Completion => format!("struct k_sem {f}"),
    }
}

/// The `init_primitives` C statements for the init hub (C05): one per declared
/// lock/completion, over `data->inner.<field>`. `k_spinlock` needs NO runtime
/// init (zero-initialized static is valid) — so SpinLock/IrqSpinLock contribute
/// no line. `destroy_primitives` is a structural NO-OP on Zephyr (static
/// primitives — C05 safety-by-no-destroy). Empty for a lock-less driver.
pub(crate) fn init_primitive_lines(model: &RegseqModel, driver_ty: TyId) -> Vec<String> {
    let mut lines = Vec::new();
    if let Ty::Struct { item, .. } = &model.types[driver_ty] {
        if let Some(s) = model.synchronization.drivers.get(item) {
            for res in &s.resources {
                let f = c_ident(model.strings.resolve(res.name));
                match res.kind {
                    SyncKind::Mutex => lines.push(format!("k_mutex_init(&data->inner.{f});")),
                    // k_spinlock is zero-init valid; no runtime init.
                    SyncKind::SpinLock | SyncKind::IrqSpinLock => {}
                    // Completion ≈ a binary semaphore: initial 0, limit 1 (SingleWaiter).
                    SyncKind::Completion => {
                        lines.push(format!("k_sem_init(&data->inner.{f}, 0, 1);"))
                    }
                }
            }
        }
    }
    lines
}

/// The includes a sync resource of `kind` pulls in. `k_mutex`/`k_sem` live in
/// `<zephyr/kernel.h>`; `k_spinlock` in `<zephyr/spinlock.h>`.
pub(crate) fn includes_for(kind: SyncKind) -> &'static [&'static str] {
    match kind {
        SyncKind::Mutex | SyncKind::Completion => &["<zephyr/kernel.h>"],
        SyncKind::SpinLock | SyncKind::IrqSpinLock => &["<zephyr/kernel.h>", "<zephyr/spinlock.h>"],
    }
}

/// The synchronization lowering for ONE body (see linux_c/sync.rs::SyncBody).
pub(crate) struct SyncBody {
    pub stmts: BTreeMap<BlockId, String>,
    pub state_locals: Vec<String>,
    pub elide_locals: Vec<LocalId>,
}

struct AcquireFacts {
    kind: SyncKind,
    field: String,
    acquire_block: BlockId,
    release_blocks: Vec<BlockId>,
    guard_local: LocalId,
}

struct CompletionFacts {
    op: CompletionOpKind,
    field: String,
    block: BlockId,
}

/// Build the synchronization lowering for `body` (instance `id`). See the
/// linux_c twin for the structure; only the C02 primitive strings + the
/// key-vs-flags guard-state shape differ.
pub(crate) fn build_for_body(
    cx: &mut LowerCx<'_>,
    body: &regseq_model::Body,
    id: InstanceId,
) -> Result<SyncBody, EmitError> {
    let body_ref = cx.model.instance_ref_of(id);

    let mut acquires: Vec<AcquireFacts> = Vec::new();
    let mut completions: Vec<CompletionFacts> = Vec::new();
    for driver in cx.model.synchronization.drivers.values() {
        for site in &driver.acquire_sites {
            if site.body != body_ref {
                continue;
            }
            let res = driver.resources.get(site.lock_id as usize).ok_or_else(|| {
                EmitError::Unsupported("acquire site references an unknown lock id".into())
            })?;
            acquires.push(AcquireFacts {
                kind: res.kind,
                field: c_ident(cx.model.strings.resolve(res.name)),
                acquire_block: site.acquire_block,
                release_blocks: site.release_blocks.clone(),
                guard_local: site.guard_local,
            });
        }
        for site in &driver.completion_sites {
            if site.body != body_ref {
                continue;
            }
            let res = driver.resources.get(site.lock_id as usize).ok_or_else(|| {
                EmitError::Unsupported("completion site references an unknown lock id".into())
            })?;
            completions.push(CompletionFacts {
                op: site.op,
                field: c_ident(cx.model.strings.resolve(res.name)),
                block: site.block,
            });
        }
    }

    let mut sb = SyncBody {
        stmts: BTreeMap::new(),
        state_locals: Vec::new(),
        elide_locals: Vec::new(),
    };
    if acquires.is_empty() && completions.is_empty() {
        return Ok(sb);
    }

    let recv = receiver_expr(cx, body)?;

    let insert = |sb: &mut SyncBody, block: BlockId, stmt: String| -> Result<(), EmitError> {
        if sb.stmts.insert(block, stmt).is_some() {
            return Err(EmitError::Unsupported(
                "two synchronization sites map to one block (recognizer overlap)".into(),
            ));
        }
        Ok(())
    };

    for a in &acquires {
        let obj = format!("&{recv}->{}", a.field);
        // Zephyr guard hidden state: `k_spinlock_key_t key`, assigned FROM the
        // acquire return — for BOTH SpinLock and IrqSpinLock (the collapse).
        // Mutex carries none. Disambiguated by the acquire block.
        let key = if matches!(a.kind, SyncKind::SpinLock | SyncKind::IrqSpinLock) {
            let v = format!("_sync_key_{}", a.acquire_block.as_u32());
            sb.state_locals.push(format!("k_spinlock_key_t {v};"));
            Some(v)
        } else {
            None
        };
        let acquire_stmt = match a.kind {
            SyncKind::Mutex => format!("k_mutex_lock({obj}, K_FOREVER);"),
            SyncKind::SpinLock | SyncKind::IrqSpinLock => {
                format!("{} = k_spin_lock({obj});", key.as_ref().unwrap())
            }
            SyncKind::Completion => {
                return Err(EmitError::Unsupported(
                    "a Completion resource recorded a lock acquire site (recognizer error)".into(),
                ))
            }
        };
        insert(&mut sb, a.acquire_block, acquire_stmt)?;
        let release_stmt = match a.kind {
            SyncKind::Mutex => format!("k_mutex_unlock({obj});"),
            SyncKind::SpinLock | SyncKind::IrqSpinLock => {
                format!("k_spin_unlock({obj}, {});", key.as_ref().unwrap())
            }
            SyncKind::Completion => unreachable!("guarded above"),
        };
        for rb in &a.release_blocks {
            insert(&mut sb, *rb, release_stmt.clone())?;
            elide_receiver(body, *rb, &mut sb.elide_locals);
        }
        sb.elide_locals.push(a.guard_local);
        elide_receiver(body, a.acquire_block, &mut sb.elide_locals);
    }

    for c in &completions {
        let obj = format!("&{recv}->{}", c.field);
        elide_receiver(body, c.block, &mut sb.elide_locals);
        let stmt = match c.op {
            // The wait `Result` is discarded, as the generic-c oracle discards it
            // (design T07-lowering §D4). `K_NSEC` takes nanoseconds directly.
            CompletionOpKind::Wait => {
                let ns = wait_timeout_ns(cx, body, c.block)?;
                format!("(void)k_sem_take({obj}, K_NSEC({ns}));")
            }
            CompletionOpKind::Signal => format!("k_sem_give({obj});"),
            CompletionOpKind::Reset => format!("k_sem_reset({obj});"),
        };
        insert(&mut sb, c.block, stmt)?;
    }

    Ok(sb)
}

/// Elide the suppressed sync `Call`'s receiver temp at `block` (its first bare
/// local arg) — dead, and typed to the facade. The `completion_wait` Duration
/// arg (a later arg) is NOT elided.
fn elide_receiver(body: &regseq_model::Body, block: BlockId, out: &mut Vec<LocalId>) {
    if let TerminatorKind::Call { args, .. } = &body.basic_blocks[block].terminator.kind {
        if let Some(Operand::Copy(p) | Operand::Move(p)) = args.first() {
            if p.projection.is_empty() {
                out.push(p.local);
            }
        }
    }
}

/// The driver `&self` receiver for a sync site's body: arg-1 must be a reference
/// to the driver struct (mirrors the config-recovery discipline).
fn receiver_expr(cx: &LowerCx<'_>, body: &regseq_model::Body) -> Result<String, EmitError> {
    let recv = body
        .arg_locals()
        .next()
        .filter(|l| {
            matches!(
                &cx.model.types[body.local_decls[*l].ty],
                Ty::Ref(pointee, _) if *pointee == cx.driver.driver_ty
            )
        })
        .ok_or_else(|| {
            EmitError::Unsupported(
                "synchronization op in a body whose receiver is not the driver type \
                 (first slice: sync sites are driver methods)"
                    .into(),
            )
        })?;
    Ok(local_name(cx.model, body, recv))
}

/// The u64-nanoseconds expression for a `Completion::wait(Duration)` at `block`
/// (the Duration is the call's last argument; its `ns` field name comes from the
/// layout, per the `emit_delay` pattern).
fn wait_timeout_ns(
    cx: &mut LowerCx<'_>,
    body: &regseq_model::Body,
    block: BlockId,
) -> Result<String, EmitError> {
    let TerminatorKind::Call {
        callee: CallCallee::Direct(callee),
        args,
        ..
    } = &body.basic_blocks[block].terminator.kind
    else {
        return Err(EmitError::Unsupported(
            "completion wait site terminator is not a direct Call".into(),
        ));
    };
    let arg = args.last().ok_or_else(|| {
        EmitError::Unsupported("completion wait call has no Duration argument".into())
    })?;
    let param_tys = match &cx.model.instances[*callee].kind {
        InstanceKind::Function(d) => cx.model.sigs.resolve(d.fn_sig).inputs.clone(),
        _ => Vec::new(),
    };
    let duration_ty = param_tys.last().copied().ok_or_else(|| {
        EmitError::Unsupported("completion wait callee has no Duration parameter type".into())
    })?;
    let field = field_c_name(cx.model, duration_ty, 0)?;
    let arg = arg.clone();
    let dur_expr = operand_expr_typed(cx, body, &arg, Some(duration_ty))?;
    Ok(format!("(uint64_t)({dur_expr}).{field}"))
}

/// Whether `(body_ref, block)` is a recognized sync site whose `Call` callee must
/// be excluded from the lowered closure. Used by `reachable_closure`.
pub(crate) fn is_sync_call_block(
    model: &RegseqModel,
    body_ref: regseq_model::InstanceRef,
    block: BlockId,
) -> bool {
    model.synchronization.drivers.values().any(|d| {
        d.acquire_sites.iter().any(|s| {
            s.body == body_ref && (s.acquire_block == block || s.release_blocks.contains(&block))
        }) || d
            .completion_sites
            .iter()
            .any(|s| s.body == body_ref && s.block == block)
    })
}
