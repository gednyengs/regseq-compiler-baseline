//! Pre-translation Rust-MIR normalizer for the same-type `Result`-`?`
//! operator (regseq-driver-92).
//!
//! At `mir_opt_level=0` (no inliner) the `?` operator desugars to a
//! fixed MIR cluster that the bridge cannot otherwise translate:
//!
//! ```text
//! BB_branch: dst = <Result<T,E> as Try>::branch(op) -> BB_disc   // CORE CALL #1 (#[lang="branch"])
//! BB_disc:   d = discriminant(dst); switchInt(d) -> [0: BB_cont, 1: BB_break, otherwise: unreachable]
//! BB_cont:   <bind (dst as Continue).0>; <original continue successor>     (no bind for a unit/ZST Ok)
//! BB_break:  r = (dst as Break).0;   // Result<Infallible, E>
//!            _0 = <Result<T,E> as FromResidual<_>>::from_residual(r) -> BB_fromres_succ  // CORE CALL #2 (#[lang="from_residual"])
//! ```
//!
//! Two blockers stop the bridge here: (1) the `branch` / `from_residual`
//! call signatures hit the `Try::Residual` / `Try::Output` associated-type
//! projections, which the type translator hard-rejects as `NonRigidTy`;
//! (2) the two `core` callees have no bridge-pulled body, so they would
//! emit as `extern` prototypes and fail to link.
//!
//! This pass recognizes the cluster by lang-item DefIds plus dataflow
//! (robust to the cleanup passes' local/block renumbering) and rewrites
//! it into a direct discriminant match on the `?`-operand, eliminating
//! both `core` calls, the `ControlFlow` / `Infallible` locals, and the
//! projection-typed locals. The rewrite preserves the original continue
//! and `from_residual` successors (and therefore any drops on the
//! early-return path), satisfying the spec's "no synthesized bare return"
//! requirement.
//!
//! Scope is deliberately narrow: same-error-type `?` on
//! `core::result::Result` only. Cross-type `?` (a real `From`
//! conversion), `Option`/other-carrier `?`, an unexpected cluster shape,
//! and a `needs_drop` `Ok` payload all fail loud with an explicit
//! [`BridgeError::UnsupportedQuestionMark`] rather than the vague
//! `NonRigidTy` the un-normalized projection would otherwise produce.

use std::collections::{HashMap, HashSet};

use rustc_abi::{FieldIdx, VariantIdx};
use rustc_hir::LangItem;
use rustc_index::IndexVec;
use rustc_middle::mir::visit::{MutVisitor, PlaceContext};
use rustc_middle::mir::{
    AggregateKind, BasicBlock, Body, Local, Location, Operand, Place, ProjectionElem, Rvalue,
    SourceInfo, Statement, StatementKind, Terminator, TerminatorKind, RETURN_PLACE,
};
use rustc_middle::ty::{self, GenericArgsRef, Ty, TyCtxt};
use rustc_span::sym;

use crate::bridge::BridgeError;

/// `Result` enum variant index for `Ok`. `enum Result<T, E> { Ok(T),
/// Err(E) }` fixes `Ok` at variant 0, so this matches the `0`
/// switch-case of the desugar's `Continue` arm.
const OK_VARIANT: VariantIdx = VariantIdx::from_u32(0);
/// `Result` enum variant index for `Err`. Fixed at variant 1, matching
/// the `1` switch-case of the desugar's `Break` arm.
const ERR_VARIANT: VariantIdx = VariantIdx::from_u32(1);
/// `ControlFlow::Continue` variant index -- the `0` `switchInt` case of
/// the `?` desugar (the continue arm).
const CONTINUE_VARIANT: VariantIdx = VariantIdx::from_u32(0);
/// `ControlFlow::Break` variant index -- the `1` `switchInt` case and the
/// variant the residual is extracted from (`(dst as Break).0`).
const BREAK_VARIANT: VariantIdx = VariantIdx::from_u32(1);

/// Is `def_id` one of the `core` lang-item methods the `?` normalizer
/// consumes (`Try::branch` / `FromResidual::from_residual`)?
///
/// The normalizer rewrites every `?` cluster away before translation, so
/// these callees never appear in a translated body and must not be
/// interned as model Items by Phase-1 mono-item discovery -- otherwise
/// they would surface as zero-instance bucket-1 orphans whose
/// `Try::Residual` / `Try::Output` projection signatures the orphan-
/// instantiation pass cannot translate. Phase-1 discovery calls this to
/// skip them.
pub(crate) fn is_question_mark_consumed_lang_item(
    tcx: TyCtxt<'_>,
    def_id: rustc_span::def_id::DefId,
) -> bool {
    // Call sites reference the trait method (the lang item) directly.
    if is_branch_or_from_residual(tcx, def_id) {
        return true;
    }
    // Phase-1 mono collection resolves the call to the concrete `impl`
    // method (e.g. `<Result<T, E> as Try>::branch`), whose own DefId is
    // not the lang item; map it back to the trait item it implements.
    if let Some(assoc) = tcx.opt_associated_item(def_id) {
        if let Some(trait_item) = assoc.trait_item_def_id() {
            return is_branch_or_from_residual(tcx, trait_item);
        }
    }
    false
}

/// Is `def_id` the `Try::branch` or `FromResidual::from_residual` trait
/// method lang item?
fn is_branch_or_from_residual(tcx: TyCtxt<'_>, def_id: rustc_span::def_id::DefId) -> bool {
    tcx.is_lang_item(def_id, LangItem::TryTraitBranch)
        || tcx.is_lang_item(def_id, LangItem::TryTraitFromResidual)
}

/// Normalize every same-type `Result`-`?` cluster in `body` in place,
/// before local-decl and basic-block translation. A no-op for bodies
/// that contain no `?` (the common case), so existing fixtures bridge
/// byte-identically.
///
/// Returns [`BridgeError::UnsupportedQuestionMark`] for any recognized
/// `?` cluster outside the same-type `Result` scope.
pub(crate) fn normalize_question_mark<'tcx>(
    tcx: TyCtxt<'tcx>,
    body: &mut Body<'tcx>,
) -> Result<(), BridgeError> {
    // Phase A: recognize every `branch`-lang-item call site and gate it.
    // Reading the whole body up front avoids any read-after-write hazard
    // across clusters during the mutation phases below.
    let mut plans = Vec::new();
    for (bb, data) in body.basic_blocks.iter_enumerated() {
        let Some(term) = &data.terminator else {
            continue;
        };
        if call_lang_item_def_id(tcx, term, LangItem::TryTraitBranch).is_none() {
            continue;
        }
        plans.push(recognize_cluster(tcx, body, bb)?);
    }

    if plans.is_empty() {
        return Ok(());
    }

    // Defensive: every `from_residual` call must be the break arm of a
    // recognized cluster. A stray `from_residual` would otherwise reach
    // translation as a body-less `core` call and fail to link.
    for (bb, data) in body.basic_blocks.iter_enumerated() {
        let Some(term) = &data.terminator else {
            continue;
        };
        if call_lang_item_def_id(tcx, term, LangItem::TryTraitFromResidual).is_some()
            && !plans.iter().any(|plan| plan.break_bb == bb)
        {
            return Err(unsupported(
                "found a `from_residual` call with no recognizable `?` cluster (unexpected MIR shape)",
            ));
        }
    }

    // Phase B: redirect every use of each cluster's `ControlFlow`
    // destination local to the `?`-operand local. This rewrites the
    // `discriminant(dst)` read and the `Continue`-arm payload extraction
    // (if any) to read the operand directly; `Continue` and `Ok` are
    // both variant 0, so the existing downcast index stays valid.
    for plan in &plans {
        let mut remapper = LocalRemapper {
            tcx,
            from: plan.dst_local,
            to: plan.op_local,
        };
        remapper.visit_body(body);
    }

    // Phase C: replace the two `core` calls with structural control flow.
    for plan in &plans {
        apply_plan(tcx, body, plan);
    }

    // Phase D: the `ControlFlow` destination and the `Result<Infallible,
    // E>` residual locals are now unreferenced. Retype them to unit so
    // no `ControlFlow` / `Infallible` / projection type survives into the
    // translated model.
    for plan in &plans {
        for &dead in std::iter::once(&plan.dst_local).chain(plan.dead_residual_locals.iter()) {
            body.local_decls[dead].ty = tcx.types.unit;
        }
    }

    Ok(())
}

/// A recognized same-type `Result`-`?` cluster plus everything the
/// rewrite needs. Block and local IDs are captured by value so the
/// mutation phases never re-borrow the body during recognition.
struct ClusterPlan<'tcx> {
    /// Block whose terminator is the `branch` call.
    branch_bb: BasicBlock,
    /// Block holding `discriminant(dst)` + `switchInt`.
    disc_bb: BasicBlock,
    /// Block whose terminator is the `from_residual` call.
    break_bb: BasicBlock,
    /// Original successor of the `from_residual` call (drops on the
    /// early-return path live here; preserved verbatim).
    fromres_succ: BasicBlock,
    /// The `?`-operand local (`Result<T, E>`).
    op_local: Local,
    /// The `ControlFlow` destination local of the `branch` call.
    dst_local: Local,
    /// `Err`-payload type of the operand (`E`).
    err_field_ty: Ty<'tcx>,
    /// `DefId` of `core::result::Result`.
    result_did: rustc_span::def_id::DefId,
    /// Generic args of the function's return `Result<T_dst, E_dst>`,
    /// used to build the `Err` aggregate in the return type's layout.
    ret_args: GenericArgsRef<'tcx>,
    /// Return place the `from_residual` call wrote (the `Err` aggregate
    /// target).
    ret_place: Place<'tcx>,
    /// `SourceInfo` of the `branch` call, reused for the synthesized
    /// goto.
    branch_si: SourceInfo,
    /// `SourceInfo` of the `from_residual` call, reused for the
    /// synthesized `Err` assignment + goto.
    break_si: SourceInfo,
    /// `Result<Infallible, E>` residual locals (the break arm's assign
    /// targets), dead after the rewrite.
    dead_residual_locals: Vec<Local>,
}

/// Source of a break-arm residual local: either the `(dst as Break).0`
/// extraction itself, or a copy of another residual local.
enum ResidualSrc {
    /// `lhs = (dst as Break).0` -- the residual extraction.
    Extract,
    /// `lhs = copy/move <local>` -- a copy of another residual local.
    CopyOf(Local),
}

/// Is `place` the `(dst as Break).0` residual extraction -- a `Break`
/// (variant 1) downcast followed by field 0? The base local is checked
/// by the caller; this only inspects the projection shape.
fn is_break_payload_place(place: &Place<'_>) -> bool {
    let elems = place.projection;
    elems.len() == 2
        && matches!(elems[0], ProjectionElem::Downcast(_, v) if v == BREAK_VARIANT)
        && matches!(elems[1], ProjectionElem::Field(f, _) if f == FieldIdx::from_u32(0))
}

/// Recognize the `?` cluster anchored at the `branch` call in
/// `branch_bb`, gating it against the same-type `Result` scope.
fn recognize_cluster<'tcx>(
    tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
    branch_bb: BasicBlock,
) -> Result<ClusterPlan<'tcx>, BridgeError> {
    let branch_data = &body.basic_blocks[branch_bb];
    let branch_term = branch_data
        .terminator
        .as_ref()
        .expect("branch block has a terminator");
    let branch_si = branch_term.source_info;
    let TerminatorKind::Call {
        args,
        destination,
        target,
        ..
    } = &branch_term.kind
    else {
        unreachable!("caller only passes Call terminators");
    };

    // The `?`-operand must be a bare-local Move/Copy and the `branch`
    // destination a bare local; anything else is an unexpected shape.
    if args.len() != 1 {
        return Err(unsupported(
            "`branch` call did not take exactly one argument",
        ));
    }
    let op_place = args[0]
        .node
        .place()
        .ok_or_else(|| unsupported("`?`-operand is not a place"))?;
    if !op_place.projection.is_empty() {
        return Err(unsupported(
            "`?`-operand is a projected place (unexpected shape)",
        ));
    }
    if !destination.projection.is_empty() {
        return Err(unsupported(
            "`branch` destination is projected (unexpected shape)",
        ));
    }
    let op_local = op_place.local;
    let dst_local = destination.local;
    let disc_bb = target.ok_or_else(|| unsupported("`branch` call has no return target"))?;

    // Same-type gate: the operand and the function return must both be
    // `core::result::Result` with the same error type.
    let result_did = tcx
        .get_diagnostic_item(sym::Result)
        .ok_or_else(|| unsupported("`core::result::Result` diagnostic item is unavailable"))?;

    let op_ty = body.local_decls[op_local].ty;
    let ty::TyKind::Adt(op_adt, op_args) = op_ty.kind() else {
        return Err(unsupported(&format!(
            "`?`-operand is not an enum carrier: {op_ty}"
        )));
    };
    if op_adt.did() != result_did {
        return Err(unsupported(&format!(
            "`?` on a non-`Result` carrier is unsupported (operand type `{op_ty}`); only same-type `Result`-`?` is normalized"
        )));
    }

    let ret_ty = body.local_decls[RETURN_PLACE].ty;
    let ty::TyKind::Adt(ret_adt, ret_args) = ret_ty.kind() else {
        return Err(unsupported(&format!(
            "function returns a non-`Result` type `{ret_ty}` while using `?`"
        )));
    };
    if ret_adt.did() != result_did {
        return Err(unsupported(&format!(
            "function returns a non-`Result` carrier `{ret_ty}` while using `?`"
        )));
    }

    let err_field_src = op_adt.variant(ERR_VARIANT).fields[FieldIdx::from_u32(0)].ty(tcx, op_args);
    let err_field_dst =
        ret_adt.variant(ERR_VARIANT).fields[FieldIdx::from_u32(0)].ty(tcx, ret_args);
    if tcx.erase_and_anonymize_regions(err_field_src)
        != tcx.erase_and_anonymize_regions(err_field_dst)
    {
        return Err(unsupported(&format!(
            "cross-type `?` (operand error `{err_field_src}` -> return error `{err_field_dst}`) requires a `From` conversion and is unsupported"
        )));
    }

    // A `needs_drop` `Ok` payload cannot be faithfully preserved by the
    // move-out rewrite (the operand is left partially moved with no drop
    // obligation the bridge can model), so fail loud.
    let ok_field_ty = op_adt.variant(OK_VARIANT).fields[FieldIdx::from_u32(0)].ty(tcx, op_args);
    if ok_field_ty.needs_drop(tcx, ty::TypingEnv::fully_monomorphized()) {
        return Err(unsupported(&format!(
            "`?` with a `needs_drop` `Ok` payload (`{ok_field_ty}`) cannot be preserved by the same-type normalizer"
        )));
    }

    // Discriminant block: a `discriminant(dst)` read into a temp that the
    // block's `switchInt` then switches on, selecting the continue
    // (value 0) and break (value 1) arms. Linking the switch operand to
    // the discriminant temp (rather than accepting any `switchInt`) ties
    // recognition to the fixed `?` dataflow shape.
    let disc_data = &body.basic_blocks[disc_bb];
    let mut disc_local = None;
    for stmt in disc_data.statements.iter() {
        if let StatementKind::Assign(boxed) = &stmt.kind {
            if let Rvalue::Discriminant(place) = &boxed.1 {
                if place.local == dst_local && place.projection.is_empty() {
                    if !boxed.0.projection.is_empty() {
                        return Err(unsupported(
                            "`?` discriminant is read into a projected place (unexpected shape)",
                        ));
                    }
                    disc_local = Some(boxed.0.local);
                }
            }
        }
    }
    let disc_local = disc_local.ok_or_else(|| {
        unsupported("`?` discriminant block does not read the `branch` result (unexpected shape)")
    })?;
    let Some(TerminatorKind::SwitchInt { discr, targets }) =
        disc_data.terminator.as_ref().map(|term| &term.kind)
    else {
        return Err(unsupported(
            "`?` discriminant block is not terminated by `switchInt` (unexpected shape)",
        ));
    };
    let switch_reads_disc = discr
        .place()
        .is_some_and(|place| place.local == disc_local && place.projection.is_empty());
    if !switch_reads_disc {
        return Err(unsupported(
            "`?` switchInt does not switch on the `branch` discriminant (unexpected shape)",
        ));
    }
    let mut cont_bb = None;
    let mut break_bb = None;
    for (value, target) in targets.iter() {
        if value == u128::from(CONTINUE_VARIANT.as_u32()) {
            cont_bb = Some(target);
        } else if value == u128::from(BREAK_VARIANT.as_u32()) {
            break_bb = Some(target);
        }
    }
    // The continue (value 0) arm must exist for the fixed shape; its
    // payload extraction is redirected generically by the Phase-B
    // dst->op remap rather than by this block id.
    cont_bb
        .ok_or_else(|| unsupported("`?` switchInt has no continue (Ok) arm (unexpected shape)"))?;
    let break_bb = break_bb
        .ok_or_else(|| unsupported("`?` switchInt has no break (Err) arm (unexpected shape)"))?;

    // Break block: a `from_residual` call into the bare return place whose
    // argument traces (through copies) back to the `(dst as Break).0`
    // residual extraction. Every statement in the block must be part of
    // that residual chain.
    let break_data = &body.basic_blocks[break_bb];
    let break_term = break_data
        .terminator
        .as_ref()
        .ok_or_else(|| unsupported("`?` break block has no terminator"))?;
    let break_si = break_term.source_info;
    let TerminatorKind::Call {
        func,
        args: fromres_args,
        destination: ret_place,
        target: fromres_target,
        ..
    } = &break_term.kind
    else {
        return Err(unsupported(
            "`?` break block is not terminated by a call (unexpected shape)",
        ));
    };
    if call_lang_item_def_id_of(tcx, func, LangItem::TryTraitFromResidual).is_none() {
        return Err(unsupported(
            "`?` break block terminator is not the `from_residual` lang item (unexpected shape)",
        ));
    }
    if ret_place.local != RETURN_PLACE || !ret_place.projection.is_empty() {
        return Err(unsupported(
            "`?` `from_residual` does not write the bare return place (unexpected shape)",
        ));
    }
    if fromres_args.len() != 1 {
        return Err(unsupported(
            "`?` `from_residual` call did not take exactly one argument (unexpected shape)",
        ));
    }
    let fromres_succ = fromres_target
        .ok_or_else(|| unsupported("`?` `from_residual` call has no return target"))?;

    // The break arm carries the residual one of two ways:
    //
    //   * non-ZST residual: `ext = (dst as Break).0; [copies]; from_residual(move resid)` --
    //     a chain of `Use` assignments the argument traces back to the
    //     `(dst as Break).0` extraction.
    //   * ZST residual (`RemoveZsts`): the extraction + copies are elided
    //     and the argument is the residual constant, leaving an empty
    //     break block. The discriminant-block check above already ties
    //     this `break_bb` to the cluster's `dst`, so the constant arm is
    //     genuinely this `?`'s break arm.
    //
    // Either way, the rewrite reads the error payload back out of the
    // `?`-operand (`(op as Err).0`), so the residual locals (if any)
    // become dead.
    let dead_residual_locals = match fromres_args[0].node.place() {
        Some(resid_arg) if resid_arg.projection.is_empty() => {
            // Every break-arm statement must be `lhs = use(src)` with
            // `src` the `(dst as Break).0` extraction or a copy of another
            // residual local. The chain map validates that shape.
            let mut residual_chain: HashMap<Local, ResidualSrc> = HashMap::new();
            for stmt in break_data.statements.iter() {
                let StatementKind::Assign(boxed) = &stmt.kind else {
                    return Err(unsupported(
                        "`?` break block has a non-assignment statement (unexpected shape)",
                    ));
                };
                if !boxed.0.projection.is_empty() {
                    return Err(unsupported(
                        "`?` break block assigns through a projection (unexpected shape)",
                    ));
                }
                let Rvalue::Use(operand) = &boxed.1 else {
                    return Err(unsupported(
                        "`?` break block has a non-`Use` rvalue (unexpected shape)",
                    ));
                };
                let src_place = operand.place().ok_or_else(|| {
                    unsupported("`?` break block copies a constant (unexpected shape)")
                })?;
                let src = if src_place.local == dst_local {
                    if !is_break_payload_place(&src_place) {
                        return Err(unsupported(
                            "`?` break block reads a non-`Break` projection of the `branch` result (unexpected shape)",
                        ));
                    }
                    ResidualSrc::Extract
                } else if src_place.projection.is_empty() {
                    ResidualSrc::CopyOf(src_place.local)
                } else {
                    return Err(unsupported(
                        "`?` break block copies a projected place (unexpected shape)",
                    ));
                };
                residual_chain.insert(boxed.0.local, src);
            }

            // Trace the `from_residual` argument back to the
            // `(dst as Break).0` extraction; the visited locals ARE the
            // residual chain (and the dead set). Anything off this chain
            // is rejected below, so we never rewrite a break arm whose
            // residual came from somewhere other than this cluster's `dst`.
            let mut dead = Vec::new();
            let mut on_chain = HashSet::new();
            let mut current = resid_arg.local;
            loop {
                if !on_chain.insert(current) {
                    return Err(unsupported(
                        "`?` break block has a cyclic residual chain (unexpected shape)",
                    ));
                }
                dead.push(current);
                match residual_chain.get(&current) {
                    Some(ResidualSrc::Extract) => break,
                    Some(ResidualSrc::CopyOf(next)) => current = *next,
                    None => {
                        return Err(unsupported(
                            "`?` `from_residual` argument does not trace to the `(dst as Break).0` extraction (unexpected shape)",
                        ));
                    }
                }
            }

            // Reject any break statement not on that traced chain. Each
            // chain local is defined by exactly one statement, so the
            // legitimate shape has `statements == chain length`; a strict
            // inequality means an off-chain statement OR a duplicate
            // assignment (which the LHS-keyed map would otherwise hide).
            // Either must fail loud rather than be silently removed by
            // `apply_plan`'s wholesale break-block replacement.
            if break_data.statements.len() != dead.len() {
                return Err(unsupported(
                    "`?` break block has a statement outside the residual chain (unexpected shape)",
                ));
            }
            dead
        }
        Some(_) => {
            return Err(unsupported(
                "`?` `from_residual` argument is a projected place (unexpected shape)",
            ));
        }
        None => {
            // ZST-residual case: the argument is the residual constant and
            // `RemoveZsts` elided the extraction, so the break block must
            // be empty. No residual locals to retype.
            if !break_data.statements.is_empty() {
                return Err(unsupported(
                    "`?` ZST-residual break block carries unexpected statements (unexpected shape)",
                ));
            }
            Vec::new()
        }
    };

    Ok(ClusterPlan {
        branch_bb,
        disc_bb,
        break_bb,
        fromres_succ,
        op_local,
        dst_local,
        err_field_ty: err_field_src,
        result_did,
        ret_args,
        ret_place: *ret_place,
        branch_si,
        break_si,
        dead_residual_locals,
    })
}

/// Rewrite the two `core` calls of a recognized cluster into structural
/// control flow. The operand's discriminant read and continue-arm
/// extraction were already redirected by the Phase-B local remap.
fn apply_plan<'tcx>(tcx: TyCtxt<'tcx>, body: &mut Body<'tcx>, plan: &ClusterPlan<'tcx>) {
    // Build the `Err` aggregate (`_0 = Result::<T_dst, E>::Err(move (op
    // as Err).0)`) before borrowing the blocks mutably.
    let err_payload = Place::from(plan.op_local).project_deeper(
        &[
            ProjectionElem::Downcast(None, ERR_VARIANT),
            ProjectionElem::Field(FieldIdx::from_u32(0), plan.err_field_ty),
        ],
        tcx,
    );
    let operands: IndexVec<FieldIdx, Operand<'tcx>> =
        IndexVec::from_raw(vec![Operand::Move(err_payload)]);
    let err_rvalue = Rvalue::Aggregate(
        Box::new(AggregateKind::Adt(
            plan.result_did,
            ERR_VARIANT,
            plan.ret_args,
            None,
            None,
        )),
        operands,
    );
    let err_assign = Statement::new(
        plan.break_si,
        StatementKind::Assign(Box::new((plan.ret_place, err_rvalue))),
    );

    let blocks = body.basic_blocks_mut();

    // `branch` call -> `goto` the discriminant block (same target the
    // call returned to).
    blocks[plan.branch_bb].terminator = Some(Terminator {
        source_info: plan.branch_si,
        kind: TerminatorKind::Goto {
            target: plan.disc_bb,
        },
    });

    // `from_residual` call -> assign `Err(e)` in the return type's layout
    // and `goto` the original `from_residual` successor (preserving any
    // drops on that path).
    let break_block = &mut blocks[plan.break_bb];
    break_block.statements = vec![err_assign];
    break_block.terminator = Some(Terminator {
        source_info: plan.break_si,
        kind: TerminatorKind::Goto {
            target: plan.fromres_succ,
        },
    });
}

/// Return the callee `DefId` if `term` is a `Call` whose callee is the
/// given lang item.
fn call_lang_item_def_id<'tcx>(
    tcx: TyCtxt<'tcx>,
    term: &Terminator<'tcx>,
    item: LangItem,
) -> Option<rustc_span::def_id::DefId> {
    let TerminatorKind::Call { func, .. } = &term.kind else {
        return None;
    };
    call_lang_item_def_id_of(tcx, func, item)
}

/// Return the callee `DefId` if `func` is a constant `FnDef` operand for
/// the given lang item.
fn call_lang_item_def_id_of<'tcx>(
    tcx: TyCtxt<'tcx>,
    func: &Operand<'tcx>,
    item: LangItem,
) -> Option<rustc_span::def_id::DefId> {
    let Operand::Constant(constant) = func else {
        return None;
    };
    let ty::TyKind::FnDef(def_id, _) = constant.const_.ty().kind() else {
        return None;
    };
    tcx.is_lang_item(*def_id, item).then_some(*def_id)
}

/// Construct an [`BridgeError::UnsupportedQuestionMark`] with the given
/// reason.
fn unsupported(reason: &str) -> BridgeError {
    BridgeError::UnsupportedQuestionMark {
        reason: reason.to_string(),
    }
}

/// Redirects every use of `from` to `to` across a MIR body. Used to
/// point the discriminant read and continue-arm extraction at the
/// `?`-operand once the `branch` call is removed.
struct LocalRemapper<'tcx> {
    tcx: TyCtxt<'tcx>,
    from: Local,
    to: Local,
}

impl<'tcx> MutVisitor<'tcx> for LocalRemapper<'tcx> {
    fn tcx(&self) -> TyCtxt<'tcx> {
        self.tcx
    }

    fn visit_local(&mut self, local: &mut Local, _context: PlaceContext, _location: Location) {
        if *local == self.from {
            *local = self.to;
        }
    }
}
