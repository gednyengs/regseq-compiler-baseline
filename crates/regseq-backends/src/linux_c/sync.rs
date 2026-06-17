//! Synchronization (locks) lowering for the linux_c backend (T07, C02).
//!
//! The author declares a typed lock field (`Mutex<T>`/`SpinLock<T>`/
//! `IrqSpinLock<T>`/`Completion`) on the `#[rseq::driver]` struct; the recognition
//! bridge records it in `model.synchronization` as a `SyncResource` plus per-site
//! acquire/completion facts keyed by `(body InstanceRef, BlockId)`. This module
//! turns those facts into kernel-C:
//!
//!  * the driver-struct lock FIELD lowers to the per-OS lock object
//!    ([`lock_field_decl`], spliced in by `emit_type_defs`'s `field_override`);
//!    the protected payload `T` is NOT stored this slice (opaque guards, no
//!    `Deref` — design T07-lowering §D1);
//!  * each acquire/release/completion CALL is suppressed and replaced by the C02
//!    primitive ([`build_for_body`] → a `block -> statement` map the body emitter
//!    consumes in place of the `Call` terminator), with the per-OS guard hidden
//!    state (Linux `unsigned long flags`, written by `spin_lock_irqsave` and
//!    consumed by `spin_unlock_irqrestore`) round-tripped through a function
//!    local.
//!
//! Mirrors the generic-c `SyncAction` interception (`generic_c/body_emit.rs`); the
//! C02 spellings are version-pin-rechecked against Linux 6.18 (see
//! `docs/discussions/synchronization/T07-version-pin-recheck.md`).

use std::collections::BTreeMap;

use regseq_model::{
    BlockId, CallCallee, CompletionOpKind, InstanceId, InstanceKind, LocalId, Operand, RegseqModel,
    SyncKind, TerminatorKind, Ty, TyId,
};

use super::body::{field_c_name, local_name, operand_expr_typed, LowerCx};
use super::naming::c_ident;
use super::EmitError;

/// The per-OS lock-FIELD C declaration for a sync resource of `kind` named `f`
/// (spliced into the driver struct by `emit_type_defs`'s `field_override`).
/// `SpinLock`/`IrqSpinLock` share `spinlock_t` (the IRQ save/restore is in the
/// acquire primitive, not the type — Linux keeps the kinds distinct only at the
/// op, I06).
pub(crate) fn lock_field_decl(kind: SyncKind, f: &str) -> String {
    match kind {
        SyncKind::Mutex => format!("struct mutex {f}"),
        SyncKind::SpinLock | SyncKind::IrqSpinLock => format!("spinlock_t {f}"),
        SyncKind::Completion => format!("struct completion {f}"),
    }
}

/// The `init_primitives` C statements for the probe preamble (C05): one per
/// declared lock/completion, over `priv->inner.<field>`. Emitted AFTER
/// `set_drvdata` and BEFORE the region map / IRQ register / `on_init`, so the
/// lock-live window ⊇ the line-live window (I05). Empty for a lock-less driver.
/// `destroy_primitives` is a no-op on Linux (a spinlock/mutex needs no teardown;
/// the devm-allocated `priv` is freed wholesale) — see `emit_scaffold`'s remove.
pub(crate) fn init_primitive_lines(model: &RegseqModel, driver_ty: TyId) -> Vec<String> {
    let mut lines = Vec::new();
    if let Ty::Struct { item, .. } = &model.types[driver_ty] {
        if let Some(s) = model.synchronization.drivers.get(item) {
            for res in &s.resources {
                let f = c_ident(model.strings.resolve(res.name));
                lines.push(match res.kind {
                    SyncKind::Mutex => format!("mutex_init(&priv->inner.{f});"),
                    SyncKind::SpinLock | SyncKind::IrqSpinLock => {
                        format!("spin_lock_init(&priv->inner.{f});")
                    }
                    SyncKind::Completion => format!("init_completion(&priv->inner.{f});"),
                });
            }
        }
    }
    lines
}

/// The includes a sync resource of `kind` pulls in (struct field type + the C02
/// primitives share the same header per kind).
pub(crate) fn includes_for(kind: SyncKind) -> &'static [&'static str] {
    match kind {
        SyncKind::Mutex => &["<linux/mutex.h>"],
        SyncKind::SpinLock | SyncKind::IrqSpinLock => &["<linux/spinlock.h>"],
        // `wait_for_completion_timeout` needs the jiffies conversion.
        SyncKind::Completion => &["<linux/completion.h>", "<linux/jiffies.h>"],
    }
}

/// The synchronization lowering for ONE body: the per-block primitive statements
/// that replace the recognized sync `Call`s, the guard hidden-state locals to
/// declare up front, and the guard locals to elide (dead — their producing
/// `lock()` and consuming drop are both suppressed).
pub(crate) struct SyncBody {
    /// `block -> primitive statement(s)` emitted in place of the block's `Call`
    /// terminator (the body emitter appends the suppressed call's `goto`).
    pub stmts: BTreeMap<BlockId, String>,
    /// Guard hidden-state local declarations (e.g. `unsigned long _sync_flags_3;`).
    pub state_locals: Vec<String>,
    /// Locals to elide (never declared): the guard locals, plus the suppressed
    /// sync calls' RECEIVER temps (`_t = &self.lock`) — dead, and crucially typed
    /// to the FACADE (`struct IrqSpinLock *`) which no longer matches the
    /// substituted field (`spinlock_t`), so leaving them would not compile.
    /// Their derived borrow temps are elided by the body emitter's taint loop.
    pub elide_locals: Vec<LocalId>,
}

/// Collected (owned) facts for one recognized acquire site in the body.
struct AcquireFacts {
    kind: SyncKind,
    field: String,
    acquire_block: BlockId,
    release_blocks: Vec<BlockId>,
    guard_local: LocalId,
}

/// Collected (owned) facts for one recognized completion op site in the body.
struct CompletionFacts {
    op: CompletionOpKind,
    field: String,
    block: BlockId,
}

/// Build the synchronization lowering for `body` (the function instance `id`).
/// Sites are matched to this body by content-addressed `InstanceRef` (stable
/// across the Phase-4 instance sort), exactly as generic-c.
pub(crate) fn build_for_body(
    cx: &mut LowerCx<'_>,
    body: &regseq_model::Body,
    id: InstanceId,
) -> Result<SyncBody, EmitError> {
    let body_ref = cx.model.instance_ref_of(id);

    // -- collect owned facts (immutable model borrow released before rendering)
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

    // The lock object is `&<receiver>-><field>`; the receiver is the body's
    // driver `&self` (arg-1), recovered once (sync sites are driver methods —
    // first-slice corpus; any other receiver shape is a loud diagnostic).
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
        // Guard hidden state: IrqSpinLock round-trips `unsigned long flags`
        // (written by irqsave, consumed by irqrestore); Mutex/SpinLock carry
        // none. Disambiguated by the acquire block (unique per site).
        let state = if a.kind == SyncKind::IrqSpinLock {
            let v = format!("_sync_flags_{}", a.acquire_block.as_u32());
            sb.state_locals.push(format!("unsigned long {v};"));
            Some(v)
        } else {
            None
        };
        let acquire_stmt = match a.kind {
            SyncKind::Mutex => format!("mutex_lock({obj});"),
            SyncKind::SpinLock => format!("spin_lock({obj});"),
            SyncKind::IrqSpinLock => {
                format!("spin_lock_irqsave({obj}, {});", state.as_ref().unwrap())
            }
            SyncKind::Completion => {
                return Err(EmitError::Unsupported(
                    "a Completion resource recorded a lock acquire site (recognizer error)".into(),
                ))
            }
        };
        insert(&mut sb, a.acquire_block, acquire_stmt)?;
        let release_stmt = match a.kind {
            SyncKind::Mutex => format!("mutex_unlock({obj});"),
            SyncKind::SpinLock => format!("spin_unlock({obj});"),
            SyncKind::IrqSpinLock => {
                format!("spin_unlock_irqrestore({obj}, {});", state.as_ref().unwrap())
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
            // The wait `Result` is discarded (`(void)`), exactly as the generic-c
            // oracle discards it — the timeout-expiry → `Err` mapping is out of
            // the first slice (design T07-lowering §D4). ns → ms rounds UP for the
            // minimum-wait guarantee, as timed-delay.
            CompletionOpKind::Wait => {
                let ns = wait_timeout_ns(cx, body, c.block)?;
                format!(
                    "(void)wait_for_completion_timeout({obj}, msecs_to_jiffies(({ns} == 0ULL ? 0ULL : 1ULL + ({ns} - 1ULL) / 1000000ULL)));"
                )
            }
            CompletionOpKind::Signal => format!("complete({obj});"),
            CompletionOpKind::Reset => format!("reinit_completion({obj});"),
        };
        insert(&mut sb, c.block, stmt)?;
    }

    Ok(sb)
}

/// Elide the suppressed sync `Call`'s RECEIVER temp at `block` (its first
/// argument, when a bare local): `_t = &self.lock` is dead once the call is
/// replaced by the C02 primitive, and is typed to the facade. The Duration
/// argument of `completion_wait` (a later arg) is deliberately NOT elided — it
/// feeds the timeout.
fn elide_receiver(body: &regseq_model::Body, block: BlockId, out: &mut Vec<LocalId>) {
    if let TerminatorKind::Call { args, .. } = &body.basic_blocks[block].terminator.kind {
        if let Some(Operand::Copy(p) | Operand::Move(p)) = args.first() {
            if p.projection.is_empty() {
                out.push(p.local);
            }
        }
    }
}

/// The driver `&self` receiver expression for a sync site's body: arg-1 must be
/// a reference to the driver struct (mirrors `config_recovery`). Any other shape
/// is out of the first slice (never wrong code, I06).
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

/// The u64-nanoseconds expression for a `Completion::wait(Duration)` at `block`:
/// the `Duration` is the call's last argument; its sole `ns` field name is
/// recovered from the layout (the `emit_delay` pattern).
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
    // Clone the operand so the immutable `args` borrow ends before `&mut cx`.
    let arg = arg.clone();
    let dur_expr = operand_expr_typed(cx, body, &arg, Some(duration_ty))?;
    Ok(format!("(uint64_t)({dur_expr}).{field}"))
}

/// Whether `(body_ref, block)` is a recognized sync site whose `Call` callee must
/// be excluded from the lowered closure (its body is never lowered — the call is
/// replaced by the C02 primitive). Used by `reachable_closure`.
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
