//! Pass 2 body construction driver (task regseq-model-43).
//!
//! [`build_body`] is the straight-line entry point. It consumes a live
//! [`TyCtxt`], a monomorphized [`DefId`], and the Pass 1 cross-reference
//! indices, and returns a fully-constructed [`Body`] whose
//! `BodySourceMap` is structurally parallel to the block/local/statement
//! layout. Cleanup (unwind) blocks are dropped; remaining blocks are
//! renumbered contiguously.
//!
//! Scope restrictions for this task:
//!
//! - `SwitchInt` translation is handled here via
//!   [`super::switch_and_loops::translate_switch_int`] (task 44). Loop
//!   recovery surfaces as a natural back-edge `Goto` with no dedicated
//!   IR construct.
//! - Regseq intrinsic recognition (task 45) consults an
//!   [`IntrinsicLookup`] built from Phase 4's `IntrinsicRegistry`.
//!   Registered callee `DefId`s produce
//!   [`TerminatorKind::IntrinsicCall`]; unregistered callees fall
//!   through to [`TerminatorKind::Call`]. Metadata population
//!   (tasks 50-52) is still out of scope -- `metadata` is `None`.
//!   `?`-pattern folding lives in task 46, described below, and
//!   drives the `error_target` field.
//! - Rule 4 deref collapse (task 48) consults a
//!   [`DerefCollapseLookup`](super::deref_collapse::DerefCollapseLookup)
//!   built from the Phase 4 `AbstractionTypeRegistry`. Transport
//!   `Deref::deref` Calls (currently `Mmio<T>`) are dropped, replaced
//!   with an unconditional `Goto` to the Call's return target, and
//!   their destination temporaries are rewritten downstream via
//!   [`rewrite_body_places`](super::deref_collapse::rewrite_body_places).
//!   Non-transport `Deref::deref` Calls are untouched.
//! - Rule 2 `?`-pattern folding (task 46) consults a
//!   [`TryFoldAnalysis`](super::try_folding::TryFoldAnalysis) built
//!   by scanning the MIR body up-front for the `Try::branch` +
//!   `SwitchInt(ControlFlow)` + `FromResidual::from_residual`
//!   pattern. Originating Call / IntrinsicCall terminators with a
//!   recognized fold are emitted with `destination` retargeted to
//!   the Continue extraction's place, `target` retargeted to the
//!   `bb_continue` block (whose extraction statement is dropped
//!   during statement translation), and `error_target` pointing at
//!   the `bb_store` block after `FromResidual` returns (whose
//!   `_0 = move _converted` store is also dropped). The intermediate
//!   `bb_branch`, `bb_switch`, `bb_break`, and `bb_unreachable`
//!   blocks become unreachable after the successor rewrite and fall
//!   out via the reachability walk in [`build_block_map`].
//! - Non-straight-line terminators (`TailCall`, `Yield`, `CoroutineDrop`,
//!   `InlineAsm`, `FalseEdge`, `FalseUnwind`, `Assert`,
//!   `UnwindResume`, `UnwindTerminate`) are rejected via
//!   [`ConstructionError::UnsupportedTerminator`].

use std::collections::HashMap;

use rustc_hir::def_id::DefId;
use rustc_middle::mir;
use rustc_middle::ty::{self, TyCtxt};

use regseq_model::identity::{
    BasicBlockId, FunctionId, LocalId, SourceOrigin, Symbol, SymbolInterner,
};
use regseq_model::ir::{
    BasicBlock, BasicBlockSourceMap, Body, BodySourceMap, LocalDecl, MirIntrinsic, Statement,
    StatementKind, Terminator, TerminatorKind,
};

use super::deref_collapse::{
    receiver_mir_place, rewrite_body_places, DerefCollapseLookup, DerefCollapseRewrites,
};
use super::intrinsics::IntrinsicLookup;
use super::log_metadata::{extract_log_metadata, is_log_intrinsic, LogArgUnwrap, LogFoldAnalysis};
use super::mir_mapping::{
    map_operand, map_place, map_rvalue, map_ty, ConstantIndex, FunctionIndex, LocalIdMap,
    MapOperandResult, TypeDefIndex,
};
use super::ordered_region::{pair_ordered_regions, OrderedMarkerSite, OrderedMarkers};
use super::register_metadata::{is_register_intrinsic, resolve_register_metadata, RegisterIndex};
use super::try_folding::{TryFoldAnalysis, TryMarkers};
use super::{resolve_origin, ConstructionError, ErrorProvenance};
use crate::extract::abstraction_types::AbstractionTypeRegistry;

/// Build a Regseq IR [`Body`] for the function identified by `def_id`
/// by translating its monomorphized MIR body.
///
/// The `function_id` parameter identifies the owning `Function` in the
/// model so the orchestrator can swap the returned body in for the Pass
/// 1 placeholder; the build itself is independent of that id.
///
/// Mapping rules follow the tables in `ir-reference.md` Sections 4
/// (statements), 5 (terminators, basic subset), and 6 (body layout).
/// Cleanup blocks (`is_cleanup == true`) are dropped wholesale and the
/// remaining block indices are renumbered contiguously.
#[allow(clippy::too_many_arguments)]
pub fn build_body<'tcx>(
    tcx: TyCtxt<'tcx>,
    def_id: DefId,
    function_id: FunctionId,
    type_def_index: &TypeDefIndex<'_>,
    constants: &ConstantIndex<'_>,
    functions: &FunctionIndex<'_>,
    intrinsics: &IntrinsicLookup,
    deref_lookup: &DerefCollapseLookup<'_>,
    ordered_markers: &OrderedMarkers,
    try_markers: &TryMarkers,
    log_arg: &LogArgUnwrap,
    register_index: &RegisterIndex,
    register_block_map: &HashMap<DefId, regseq_model::identity::RegisterBlockId>,
    abstraction_registry: &AbstractionTypeRegistry,
    interner: &mut SymbolInterner,
) -> Result<Body, ConstructionError> {
    // `function_id` is currently not consumed inside `build_body` -- the
    // orchestrator uses it to reattach the returned body. Keep it in the
    // signature so callers cannot accidentally misuse the output against
    // a different function slot.
    let _ = function_id;

    let mir_body = tcx.optimized_mir(def_id);

    // Rule 2 `?`-pattern analysis runs first so the block-map
    // reachability walk can route originating Call terminators to
    // their folded successors (`continue_target`, `error_target`)
    // instead of the intermediate `bb_branch` -- the intermediate
    // blocks drop out naturally once unreachable.
    let try_folds = TryFoldAnalysis::analyze(tcx, def_id, try_markers, mir_body)?;

    // Rule 6 `LogArg` carrier pre-fold: identify every MIR local
    // whose type is `LogArg<'_>` (or an array thereof) so the
    // local-decl translation can skip them before
    // [`super::mir_mapping::map_ty`] is asked to resolve the
    // cross-crate `LogArg` ADT. Statement and terminator translation
    // consult the same set to drop array-aggregate Assigns and
    // collapse `LogArg::from(...)` Calls to `Goto`. The actual log
    // metadata extraction still runs at the IntrinsicCall site (see
    // [`extract_log_metadata`]) using the live MIR -- LogFoldAnalysis
    // only handles the structural elision, not the metadata payload.
    let log_folds = LogFoldAnalysis::analyze(mir_body, log_arg);

    let block_map = build_block_map(mir_body, &try_folds);
    let local_name_map = build_local_name_map(mir_body, interner);
    let (local_decls, local_sources, local_id_map) = build_local_decls(
        tcx,
        mir_body,
        &local_name_map,
        type_def_index,
        &try_folds,
        &log_folds,
        interner,
    )?;

    let mut basic_blocks: Vec<BasicBlock> = Vec::new();
    let mut block_sources: Vec<BasicBlockSourceMap> = Vec::new();
    // Rule 4 deref-collapse rewrite table, populated during terminator
    // translation as transport `Deref::deref` Calls are dropped. After
    // every block is built, `rewrite_body_places` splices the collapsed
    // destinations back through the receiver Places recorded here.
    let mut deref_rewrites = DerefCollapseRewrites::default();
    // Recognition rule 5 side channel (see
    // `construction::ordered_region`): every dropped marker `Call`
    // pushes an entry here so the post-translation pairing pass can
    // fold them into `Body.ordered_regions`.
    let mut ordered_sites: Vec<OrderedMarkerSite> = Vec::new();
    for (mir_bb, data) in mir_body.basic_blocks.iter_enumerated() {
        let Some(current_id) = block_map[mir_bb.as_usize()] else {
            continue;
        };

        let mut statements: Vec<Statement> = Vec::new();
        let mut statement_sources: Vec<SourceOrigin> = Vec::new();
        for (stmt_idx, stmt) in data.statements.iter().enumerate() {
            // Rule 2 absorbs the Continue-extraction statement into
            // the folded terminator's `destination` (and drops the
            // `_0 = move _converted` store on the error path). The
            // pre-computed `TryFoldAnalysis` flags those statements
            // by `(mir_bb, stmt_idx)`.
            if try_folds.is_dropped_statement(mir_bb, stmt_idx) {
                continue;
            }
            // `StorageLive` / `StorageDead` markers for locals that
            // the fold eliminates have no Regseq referent -- drop
            // them silently so later translation steps do not look
            // up a dropped local.
            if let mir::StatementKind::StorageLive(l) | mir::StatementKind::StorageDead(l) =
                &stmt.kind
            {
                if try_folds.is_dropped_local(*l) || log_folds.is_dropped_local(*l) {
                    continue;
                }
            }
            // Rule 6: drop any Assign whose destination is a
            // `LogArg`-carrier local. This covers both the per-arg
            // rename moves (e.g. `_t = move _from_call_dest`) and
            // the `[LogArg<'_>; N]` Aggregate(Array) statement that
            // collects the per-arg LogArg values for the log
            // intrinsic call. The IntrinsicCall translation rebuilds
            // the inner-arg list directly from the MIR body (see
            // `extract_log_metadata`), so the dropped statements
            // carry no information that survives into the Regseq IR.
            if let mir::StatementKind::Assign(boxed) = &stmt.kind {
                let (place, _) = boxed.as_ref();
                if place.projection.is_empty() && log_folds.is_dropped_local(place.local) {
                    continue;
                }
            }
            let origin = resolve_origin(tcx, interner, stmt.source_info.span);
            if let Some(kind) = translate_statement(
                tcx,
                stmt,
                &local_id_map,
                type_def_index,
                constants,
                interner,
            )? {
                statements.push(Statement { kind });
                statement_sources.push(origin);
            }
        }

        let term_data =
            data.terminator
                .as_ref()
                .ok_or_else(|| ConstructionError::UnsupportedTerminator {
                    provenance: ErrorProvenance::for_def_id(tcx, def_id),
                    rendered: format!("bb{} has no terminator", mir_bb.as_u32()),
                })?;
        let term_origin = resolve_origin(tcx, interner, term_data.source_info.span);
        let term_kind = translate_terminator(
            tcx,
            def_id,
            mir_body,
            term_data,
            mir_bb,
            &block_map,
            &local_id_map,
            type_def_index,
            constants,
            functions,
            intrinsics,
            deref_lookup,
            &mut deref_rewrites,
            ordered_markers,
            &try_folds,
            log_arg,
            &log_folds,
            register_index,
            register_block_map,
            abstraction_registry,
            current_id,
            &mut ordered_sites,
            interner,
        )?;
        basic_blocks.push(BasicBlock {
            statements,
            terminator: Terminator { kind: term_kind },
        });
        block_sources.push(BasicBlockSourceMap {
            statements: statement_sources,
            terminator: term_origin,
        });
    }

    // Apply collected Rule 4 rewrites to every Place reference in the
    // constructed body. When `deref_rewrites` is empty (no transport
    // deref was collapsed), this is a cheap no-op.
    rewrite_body_places(&mut basic_blocks, &deref_rewrites);

    let body_span = resolve_origin(tcx, interner, mir_body.span);

    let ordered_regions = pair_ordered_regions(tcx, def_id, &ordered_sites)?;

    Ok(Body {
        local_decls,
        arg_count: mir_body.arg_count,
        basic_blocks,
        source_map: BodySourceMap {
            body_span,
            locals: local_sources,
            blocks: block_sources,
        },
        ordered_regions,
    })
}

// ---------------------------------------------------------------------------
// Block remapping
// ---------------------------------------------------------------------------

/// Build a dense `mir::BasicBlock` -> `Option<BasicBlockId>` table.
/// `None` entries correspond to blocks dropped by Pass 2 -- the
/// spec-literal cleanup resume blocks plus any other block left
/// unreachable from entry after `unwind` edges on Call/Drop are
/// stripped during terminator translation.
///
/// The algorithm matches the two rules from the task spec's
/// Cleanup-block dropping section (drop `is_cleanup && UnwindResume`)
/// and ir-reference.md Section 6 ("cleanup blocks ending in resume are
/// dropped"; Call/Drop unwind fields are dropped during translation)
/// together. Steps:
///
/// 1. Mark every block whose MIR data is `is_cleanup == true` and
///    whose terminator is `UnwindResume`. These are the leaves of the
///    MIR cleanup subtree -- dropping them per the spec literal rule.
/// 2. Walk reachability from `START_BLOCK` considering only the edges
///    Pass 2 preserves (Call::target, Drop::target, Goto, SwitchInt,
///    Assert::target, FalseEdge::real_target, etc.). `Call::unwind`
///    and `Drop::unwind` edges into the cleanup subtree are not
///    traversed because translation drops those fields.
/// 3. Any MIR block neither marked in step 1 nor reached in step 2 is
///    unreachable. In practice this catches the non-resume interior
///    cleanup blocks (`Drop { .. -> resume }`, cleanup `Call` chains)
///    so their translation never produces dangling `BasicBlockId`
///    references.
/// 4. Blocks that survive are assigned contiguous `BasicBlockId`s in
///    MIR order.
fn build_block_map<'tcx>(
    mir_body: &mir::Body<'tcx>,
    try_folds: &TryFoldAnalysis<'tcx>,
) -> Vec<Option<BasicBlockId>> {
    let count = mir_body.basic_blocks.len();
    let mut drop_by_rule = vec![false; count];
    for (mir_bb, data) in mir_body.basic_blocks.iter_enumerated() {
        if data.is_cleanup && matches!(data.terminator().kind, mir::TerminatorKind::UnwindResume) {
            drop_by_rule[mir_bb.as_usize()] = true;
        }
    }

    let mut reachable = vec![false; count];
    let mut stack = vec![mir::START_BLOCK];
    while let Some(bb) = stack.pop() {
        let idx = bb.as_usize();
        if drop_by_rule[idx] || reachable[idx] {
            continue;
        }
        reachable[idx] = true;
        let data = &mir_body.basic_blocks[bb];
        let successors = try_folds
            .get(bb)
            .map(|info| vec![info.continue_target, info.error_target])
            .unwrap_or_else(|| non_unwind_successors(data));
        for succ in successors {
            if !drop_by_rule[succ.as_usize()] && !reachable[succ.as_usize()] {
                stack.push(succ);
            }
        }
    }

    let mut map: Vec<Option<BasicBlockId>> = vec![None; count];
    let mut next: u32 = 0;
    for idx in 0..count {
        if reachable[idx] {
            map[idx] = Some(BasicBlockId(next));
            next += 1;
        }
    }
    map
}

/// MIR basic-block successors considering only the edges that survive
/// Pass 2 translation -- i.e., excluding every `UnwindAction::Cleanup(bb)`
/// or similar unwind-side field that the translation drops. Used by
/// [`build_block_map`] to compute reachability with the same edge set
/// the translated IR will expose.
fn non_unwind_successors<'tcx>(data: &mir::BasicBlockData<'tcx>) -> Vec<mir::BasicBlock> {
    let mut out: Vec<mir::BasicBlock> = Vec::new();
    match &data.terminator().kind {
        mir::TerminatorKind::Goto { target } => out.push(*target),
        mir::TerminatorKind::SwitchInt { targets, .. } => {
            for t in targets.all_targets() {
                out.push(*t);
            }
        }
        mir::TerminatorKind::Call { target, .. } => {
            if let Some(t) = target {
                out.push(*t);
            }
        }
        mir::TerminatorKind::Drop { target, .. } => out.push(*target),
        mir::TerminatorKind::Assert { target, .. } => out.push(*target),
        mir::TerminatorKind::FalseEdge { real_target, .. } => out.push(*real_target),
        mir::TerminatorKind::FalseUnwind { real_target, .. } => out.push(*real_target),
        mir::TerminatorKind::Yield { resume, drop, .. } => {
            out.push(*resume);
            if let Some(t) = drop {
                out.push(*t);
            }
        }
        mir::TerminatorKind::InlineAsm { targets, .. } => {
            for t in targets.iter() {
                out.push(*t);
            }
        }
        mir::TerminatorKind::TailCall { .. }
        | mir::TerminatorKind::Return
        | mir::TerminatorKind::Unreachable
        | mir::TerminatorKind::UnwindResume
        | mir::TerminatorKind::UnwindTerminate(_)
        | mir::TerminatorKind::CoroutineDrop => {}
    }
    out
}

// ---------------------------------------------------------------------------
// Local declarations
// ---------------------------------------------------------------------------

/// Collect user-authored variable names from `mir_body.var_debug_info`.
/// Only debug entries whose contents are a bare [`mir::Place`] (no
/// projection) are retained -- composite fragments and constant debug
/// entries do not correspond to a single MIR local. Each local keeps the
/// first name encountered.
fn build_local_name_map<'tcx>(
    mir_body: &mir::Body<'tcx>,
    interner: &mut SymbolInterner,
) -> HashMap<mir::Local, Symbol> {
    let mut out: HashMap<mir::Local, Symbol> = HashMap::new();
    for info in &mir_body.var_debug_info {
        if info.composite.is_some() {
            continue;
        }
        if let mir::VarDebugInfoContents::Place(place) = &info.value {
            if !place.projection.is_empty() {
                continue;
            }
            out.entry(place.local)
                .or_insert_with(|| interner.intern(info.name.as_str()));
        }
    }
    out
}

/// Build the Regseq IR [`LocalDecl`] vector, the parallel source-origin
/// vector, and the [`LocalIdMap`] used by statement/terminator
/// translation.
#[allow(clippy::too_many_arguments)]
fn build_local_decls<'a, 'tcx>(
    tcx: TyCtxt<'tcx>,
    mir_body: &'a mir::Body<'tcx>,
    name_map: &HashMap<mir::Local, Symbol>,
    type_def_index: &TypeDefIndex<'_>,
    try_folds: &TryFoldAnalysis<'tcx>,
    log_folds: &LogFoldAnalysis,
    interner: &mut SymbolInterner,
) -> Result<(Vec<LocalDecl>, Vec<SourceOrigin>, LocalIdMap<'a, 'tcx>), ConstructionError> {
    let mut local_id_map = LocalIdMap::new(&mir_body.local_decls);
    let mut decls = Vec::with_capacity(mir_body.local_decls.len());
    let mut sources = Vec::with_capacity(mir_body.local_decls.len());
    // Rule 2 eliminates the `_cf`, `_residual`, `_converted`, and
    // originating-Call Try-wrapper locals from the Regseq IR
    // entirely (ir-reference.md Section 5 "Eliminations"). Skipping
    // them here shifts every downstream local's `LocalId` down; the
    // `LocalIdMap` records only the surviving (MIR local ->
    // contiguous LocalId) pairs, so every place / operand
    // translation in `mir_mapping` resolves against the new numbering
    // without changes. All references to the dropped locals are
    // themselves in blocks / statements the fold also drops, so no
    // `LocalIdMap::lookup` against a dropped local ever succeeds.
    //
    // Rule 6 (logging) shares the same dropping discipline for the
    // `LogArg<'_>` carrier locals + the `[LogArg<'_>; N]` array
    // local; see [`LogFoldAnalysis`]. The cross-crate `LogArg` ADT
    // never reaches `map_ty` thanks to this skip.
    let mut next: u32 = 0;
    for (local, decl) in mir_body.local_decls.iter_enumerated() {
        if try_folds.is_dropped_local(local) || log_folds.is_dropped_local(local) {
            continue;
        }
        local_id_map.insert(local, LocalId::new(next));
        next += 1;
        let ty = map_ty(tcx, decl.ty, type_def_index)?;
        let mutable = matches!(decl.mutability, rustc_ast::Mutability::Mut);
        let name = name_map.get(&local).copied();
        decls.push(LocalDecl { ty, mutable, name });
        sources.push(resolve_origin(tcx, interner, decl.source_info.span));
    }
    Ok((decls, sources, local_id_map))
}

// ---------------------------------------------------------------------------
// Statement translation
// ---------------------------------------------------------------------------

/// Translate one MIR [`mir::Statement`]. Returns [`None`] for statement
/// kinds the Regseq IR drops (`FakeRead`, `AscribeUserType`, `Retag`,
/// `PlaceMention`, `Coverage`, `ConstEvalCounter`, `Nop`,
/// `BackwardIncompatibleDropHint`).
fn translate_statement<'tcx>(
    tcx: TyCtxt<'tcx>,
    stmt: &mir::Statement<'tcx>,
    locals: &LocalIdMap<'_, 'tcx>,
    type_def_index: &TypeDefIndex<'_>,
    constants: &ConstantIndex<'_>,
    interner: &mut SymbolInterner,
) -> Result<Option<StatementKind>, ConstructionError> {
    match &stmt.kind {
        mir::StatementKind::Assign(boxed) => {
            let (place, rvalue) = boxed.as_ref();
            let p = map_place(tcx, place, locals, type_def_index, interner)?;
            let r = map_rvalue(tcx, rvalue, locals, type_def_index, constants, interner)?;
            Ok(Some(StatementKind::Assign(p, r)))
        }
        mir::StatementKind::SetDiscriminant {
            place,
            variant_index,
        } => {
            let p = map_place(tcx, place.as_ref(), locals, type_def_index, interner)?;
            Ok(Some(StatementKind::SetDiscriminant {
                place: p,
                variant_index: variant_index.as_u32(),
            }))
        }
        mir::StatementKind::StorageLive(local) => {
            let id = lookup_local(locals, *local)?;
            Ok(Some(StatementKind::StorageLive(id)))
        }
        mir::StatementKind::StorageDead(local) => {
            let id = lookup_local(locals, *local)?;
            Ok(Some(StatementKind::StorageDead(id)))
        }
        mir::StatementKind::Intrinsic(boxed) => {
            let mapped = translate_non_diverging_intrinsic(
                tcx,
                boxed.as_ref(),
                locals,
                type_def_index,
                constants,
                interner,
            )?;
            Ok(Some(StatementKind::MirIntrinsic(mapped)))
        }
        mir::StatementKind::FakeRead(_)
        | mir::StatementKind::AscribeUserType(_, _)
        | mir::StatementKind::Retag(_, _)
        | mir::StatementKind::PlaceMention(_)
        | mir::StatementKind::Coverage(_)
        | mir::StatementKind::ConstEvalCounter
        | mir::StatementKind::Nop
        | mir::StatementKind::BackwardIncompatibleDropHint { .. } => Ok(None),
    }
}

fn translate_non_diverging_intrinsic<'tcx>(
    tcx: TyCtxt<'tcx>,
    intrinsic: &mir::NonDivergingIntrinsic<'tcx>,
    locals: &LocalIdMap<'_, 'tcx>,
    type_def_index: &TypeDefIndex<'_>,
    constants: &ConstantIndex<'_>,
    interner: &mut SymbolInterner,
) -> Result<MirIntrinsic, ConstructionError> {
    match intrinsic {
        mir::NonDivergingIntrinsic::Assume(op) => {
            let mapped = require_operand(tcx, op, locals, type_def_index, constants, interner)?;
            Ok(MirIntrinsic::Assume(mapped))
        }
        mir::NonDivergingIntrinsic::CopyNonOverlapping(copy) => {
            let src = require_operand(tcx, &copy.src, locals, type_def_index, constants, interner)?;
            let dst = require_operand(tcx, &copy.dst, locals, type_def_index, constants, interner)?;
            let count = require_operand(
                tcx,
                &copy.count,
                locals,
                type_def_index,
                constants,
                interner,
            )?;
            Ok(MirIntrinsic::CopyNonOverlapping { src, dst, count })
        }
    }
}

fn require_operand<'tcx>(
    tcx: TyCtxt<'tcx>,
    op: &mir::Operand<'tcx>,
    locals: &LocalIdMap<'_, 'tcx>,
    type_def_index: &TypeDefIndex<'_>,
    constants: &ConstantIndex<'_>,
    interner: &mut SymbolInterner,
) -> Result<regseq_model::ir::Operand, ConstructionError> {
    match map_operand(tcx, op, locals, constants, type_def_index, interner)? {
        MapOperandResult::Mapped(o) => Ok(o),
        MapOperandResult::Drop => Err(ConstructionError::UnsupportedRvalue {
            provenance: ErrorProvenance::for_span(rustc_span::DUMMY_SP),
            rendered: "dropped operand in required position".to_owned(),
        }),
    }
}

fn lookup_local<'tcx>(
    locals: &LocalIdMap<'_, 'tcx>,
    local: mir::Local,
) -> Result<LocalId, ConstructionError> {
    locals
        .lookup(local)
        .ok_or_else(|| ConstructionError::UnsupportedProjection {
            provenance: ErrorProvenance::for_span(rustc_span::DUMMY_SP),
            rendered: format!(
                "MIR local _{} not assigned a Regseq LocalId",
                local.as_u32()
            ),
        })
}

// ---------------------------------------------------------------------------
// Terminator translation
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn translate_terminator<'tcx>(
    tcx: TyCtxt<'tcx>,
    owner_def_id: DefId,
    mir_body: &mir::Body<'tcx>,
    term: &mir::Terminator<'tcx>,
    mir_bb: mir::BasicBlock,
    block_map: &[Option<BasicBlockId>],
    locals: &LocalIdMap<'_, 'tcx>,
    type_def_index: &TypeDefIndex<'_>,
    constants: &ConstantIndex<'_>,
    functions: &FunctionIndex<'_>,
    intrinsics: &IntrinsicLookup,
    deref_lookup: &DerefCollapseLookup<'_>,
    deref_rewrites: &mut DerefCollapseRewrites,
    ordered_markers: &OrderedMarkers,
    try_folds: &TryFoldAnalysis<'tcx>,
    log_arg: &LogArgUnwrap,
    log_folds: &LogFoldAnalysis,
    register_index: &RegisterIndex,
    register_block_map: &HashMap<DefId, regseq_model::identity::RegisterBlockId>,
    abstraction_registry: &AbstractionTypeRegistry,
    current_block: BasicBlockId,
    ordered_sites: &mut Vec<OrderedMarkerSite>,
    interner: &mut SymbolInterner,
) -> Result<TerminatorKind, ConstructionError> {
    match &term.kind {
        mir::TerminatorKind::Goto { target } => Ok(TerminatorKind::Goto(require_block(
            block_map,
            *target,
            term.source_info.span,
        )?)),
        mir::TerminatorKind::Return => Ok(TerminatorKind::Return),
        mir::TerminatorKind::Unreachable => Ok(TerminatorKind::Unreachable),
        mir::TerminatorKind::Drop { place, target, .. } => {
            let p = map_place(tcx, place, locals, type_def_index, interner)?;
            let t = require_block(block_map, *target, term.source_info.span)?;
            Ok(TerminatorKind::Drop {
                place: p,
                target: t,
            })
        }
        mir::TerminatorKind::Call {
            func,
            args,
            destination,
            target,
            ..
        } => {
            let callee_def_id = resolve_callee_def_id(func, term.source_info.span)?;

            // Rule 6: collapse `LogArg::from(value)` Calls (and any
            // other Call whose destination is a dropped `LogArg`
            // carrier local) into an unconditional `Goto` to the
            // call's return target. The IntrinsicCall translation
            // for the enclosing log call rebuilds the inner-arg list
            // by walking the MIR directly (see
            // `extract_log_metadata`), so the collapsed Call's
            // operands carry no information that survives into the
            // Regseq IR. Dropping them keeps the cross-crate
            // `LogArg` ADT out of [`map_ty`].
            if destination.projection.is_empty() && log_folds.is_dropped_local(destination.local) {
                let return_target =
                    target.ok_or_else(|| ConstructionError::UnsupportedTerminator {
                        provenance: ErrorProvenance {
                            item_path: tcx.def_path_str(callee_def_id),
                            span: term.source_info.span,
                        },
                        rendered: "LogArg-producing Call has no return target -- cannot collapse"
                            .to_string(),
                    })?;
                return Ok(TerminatorKind::Goto(require_block(
                    block_map,
                    return_target,
                    term.source_info.span,
                )?));
            }

            // Recognition rule 5 (ir-reference.md Section 8): paired
            // `regseq::internal::ordered_region_begin()` /
            // `ordered_region_end()` marker calls are dropped from the
            // CFG. The marker's basic block becomes a `Goto` to the
            // call's original return target, preserving preceding
            // statements. The `(block, kind, target)` triple is
            // recorded so the post-translation pairing pass can fold
            // matched markers into `Body.ordered_regions`. This rule
            // runs *before* intrinsic recognition -- the markers are
            // compiler markers, not `#[rseq::intrinsic]` intrinsics.
            if let Some(kind) = ordered_markers.classify(callee_def_id) {
                let marker_target = match target {
                    Some(t) => require_block(block_map, *t, term.source_info.span)?,
                    None => {
                        return Err(ConstructionError::UnsupportedTerminator {
                            provenance: ErrorProvenance {
                                item_path: tcx.def_path_str(callee_def_id),
                                span: term.source_info.span,
                            },
                            rendered:
                                "ordered-region marker called as a diverging `Call` (no return target)"
                                    .to_string(),
                        });
                    }
                };
                ordered_sites.push(OrderedMarkerSite {
                    owning_block: current_block,
                    kind,
                    target: marker_target,
                });
                return Ok(TerminatorKind::Goto(marker_target));
            }

            // Recognition rule 4 (ir-reference.md Section 8): collapse
            // `<Mmio<T> as Deref>::deref` (and any other transport
            // abstraction registered in `AbstractionTypeRegistry`)
            // before operand translation. The Call is replaced with an
            // unconditional `Goto` to the return target, and the
            // destination temporary is recorded in `deref_rewrites` so
            // downstream uses splice back through the receiver Place
            // (see `super::deref_collapse`). Non-transport derefs are
            // left for regular Call translation below.
            if let Some(collapse) = try_collapse_transport_deref(
                tcx,
                callee_def_id,
                args,
                destination,
                target,
                block_map,
                locals,
                type_def_index,
                term.source_info.span,
                deref_lookup,
                deref_rewrites,
                interner,
            )? {
                return Ok(collapse);
            }

            // For logging intrinsics, the original MIR args are
            // `(template: &'static str, [LogArg<'_>; N])` -- both
            // shapes that the regular per-arg mapping rejects (the
            // `Slice` constant is not a Regseq scalar, and the
            // `LogArg` array's element type is the cross-crate
            // `LogArg` ADT). Skip the regular mapping when Rule 6
            // applies and let `extract_log_metadata` rebuild the
            // arg list from the inner per-argument operands.
            let is_log_call = intrinsics
                .lookup(callee_def_id)
                .is_some_and(is_log_intrinsic);
            let mut mapped_args: Vec<regseq_model::ir::Operand> = Vec::new();
            if !is_log_call {
                mapped_args.reserve(args.len());
                for arg in args.iter() {
                    mapped_args.push(require_operand(
                        tcx,
                        &arg.node,
                        locals,
                        type_def_index,
                        constants,
                        interner,
                    )?);
                }
            }

            // Rule 2 (ir-reference.md Section 5): when `TryFoldAnalysis`
            // recognized a `?` pattern rooted at this Call, re-aim
            // the terminator at the Continue / Break path and absorb
            // the extraction into `destination`. The intermediate
            // `bb_branch`/`bb_switch`/`bb_break`/`bb_unreachable`
            // blocks are unreachable under the fold-adjusted
            // successor set and fall out of the block map.
            let fold = try_folds.get(mir_bb);
            let dest = match fold {
                Some(info) => {
                    map_place(tcx, &info.new_destination, locals, type_def_index, interner)?
                }
                None => map_place(tcx, destination, locals, type_def_index, interner)?,
            };

            // Recognition rule 1 (ir-reference.md Section 8): callees
            // whose `DefId` is registered as a Regseq intrinsic become
            // `IntrinsicCall`. Method intrinsics preserve `&self` as
            // `args[0]`; free-function intrinsics call directly -- the
            // lookup does not distinguish, and the translated arg
            // order matches MIR's verbatim (spec.md Section 10).
            if let Some(variant) = intrinsics.lookup(callee_def_id) {
                let _ = log_folds; // silence unused-binding warning when no log calls fire
                let (intrinsic_target, intrinsic_error_target) = match fold {
                    Some(info) => (
                        require_block(block_map, info.continue_target, term.source_info.span)?,
                        Some(require_block(
                            block_map,
                            info.error_target,
                            term.source_info.span,
                        )?),
                    ),
                    None => match target {
                        Some(t) => (require_block(block_map, *t, term.source_info.span)?, None),
                        None => {
                            return Err(ConstructionError::UnsupportedTerminator {
                                provenance: ErrorProvenance {
                                    item_path: tcx.def_path_str(callee_def_id),
                                    span: term.source_info.span,
                                },
                                rendered: format!(
                                    "intrinsic `{}` called as a diverging `Call` (no return target)",
                                    variant.canonical_name(),
                                ),
                            });
                        }
                    },
                };

                // Recognition rule 6 (ir-reference.md Section 8): for
                // logging intrinsics, extract the template from the
                // first MIR arg, parse the restricted format subset
                // into placeholders, and unwrap the `LogArg::from(...)`
                // carrier on every element of the second MIR arg
                // (the `[LogArg<'_>; N]` array). The `mapped_args`
                // computed above (template + array operand) is
                // discarded -- the rebuilt list contains only the
                // inner per-argument operands.
                //
                // Recognition rule 7 (ir-reference.md Section 8): for
                // register-access intrinsics (`RegisterRead`,
                // `RegisterReadField`, `RegisterWrite`,
                // `RegisterWriteField`), resolve the register-block /
                // offset / access-width / optional field metadata
                // from the call's receiver type + monomorphized
                // generic args + field operand, and attach it as
                // [`IntrinsicMetadata::Register`]. The argument list
                // is preserved as-is (the field operand stays in
                // `args[1]` so backends keep access to composed
                // `FieldValue` inputs, and the register-marker
                // `RegisterId` carrier stays in `args[1]` for
                // `RegisterRead` / `RegisterWrite`).
                let (final_args, metadata) = if is_log_intrinsic(variant) {
                    let extraction = extract_log_metadata(
                        tcx,
                        mir_body,
                        owner_def_id,
                        args,
                        locals,
                        type_def_index,
                        constants,
                        log_arg,
                        term.source_info.span,
                        interner,
                    )?;
                    (extraction.args, Some(extraction.metadata))
                } else if is_register_intrinsic(variant) {
                    let generic_args = callee_generic_args(func).ok_or_else(|| {
                        ConstructionError::UnresolvedCallTarget {
                            provenance: ErrorProvenance {
                                item_path: tcx.def_path_str(callee_def_id),
                                span: term.source_info.span,
                            },
                            rendered: format!(
                                "register intrinsic `{}` call site has no resolvable generic args",
                                variant.canonical_name(),
                            ),
                        }
                    })?;
                    let resolution = resolve_register_metadata(
                        tcx,
                        variant,
                        callee_def_id,
                        generic_args,
                        args,
                        locals,
                        register_index,
                        register_block_map,
                        abstraction_registry,
                        term.source_info.span,
                    )?;
                    match resolution {
                        Some(r) => (mapped_args, Some(r.metadata)),
                        None => (mapped_args, None),
                    }
                } else {
                    (mapped_args, None)
                };

                return Ok(TerminatorKind::IntrinsicCall {
                    intrinsic: variant,
                    args: final_args,
                    metadata,
                    destination: dest,
                    target: intrinsic_target,
                    error_target: intrinsic_error_target,
                });
            }

            let callee = functions.lookup(callee_def_id).ok_or_else(|| {
                ConstructionError::UnresolvedCallTarget {
                    provenance: ErrorProvenance {
                        item_path: tcx.def_path_str(callee_def_id),
                        span: term.source_info.span,
                    },
                    rendered: tcx.def_path_str(callee_def_id),
                }
            })?;
            let (call_target, call_error_target) = match fold {
                Some(info) => (
                    Some(require_block(
                        block_map,
                        info.continue_target,
                        term.source_info.span,
                    )?),
                    Some(require_block(
                        block_map,
                        info.error_target,
                        term.source_info.span,
                    )?),
                ),
                None => (
                    match target {
                        Some(t) => Some(require_block(block_map, *t, term.source_info.span)?),
                        None => None,
                    },
                    None,
                ),
            };
            Ok(TerminatorKind::Call {
                callee,
                args: mapped_args,
                destination: dest,
                target: call_target,
                error_target: call_error_target,
            })
        }
        mir::TerminatorKind::SwitchInt { discr, targets } => {
            super::switch_and_loops::translate_switch_int(
                tcx,
                owner_def_id,
                term,
                discr,
                targets,
                block_map,
                locals,
                type_def_index,
                constants,
                interner,
            )
        }
        other => {
            let tag = match other {
                mir::TerminatorKind::Assert { .. } => "Assert",
                mir::TerminatorKind::UnwindResume => "UnwindResume",
                mir::TerminatorKind::UnwindTerminate(_) => "UnwindTerminate",
                mir::TerminatorKind::TailCall { .. } => "TailCall",
                mir::TerminatorKind::Yield { .. } => "Yield",
                mir::TerminatorKind::CoroutineDrop => "CoroutineDrop",
                mir::TerminatorKind::InlineAsm { .. } => "InlineAsm",
                mir::TerminatorKind::FalseEdge { .. } => "FalseEdge",
                mir::TerminatorKind::FalseUnwind { .. } => "FalseUnwind",
                // Exhaustively covered above -- the supported cases
                // (Goto/Return/Unreachable/Drop/Call/SwitchInt) are
                // already matched in earlier arms, so this arm is
                // unreachable for them.
                _ => "Unknown",
            };
            Err(ConstructionError::UnsupportedTerminator {
                provenance: ErrorProvenance {
                    item_path: tcx.def_path_str(owner_def_id),
                    span: term.source_info.span,
                },
                rendered: format!("{tag} ({:?})", term.kind),
            })
        }
    }
}

/// Attempt to recognize a transport `<T as Deref>::deref` Call and
/// collapse it into a `Goto` to the Call's return target.
///
/// Returns:
///
/// - `Ok(Some(Goto))` when the Call's callee resolves to `Deref::deref`
///   (trait method or its impl) AND the receiver's type is an ADT
///   whose `DefId` is registered in
///   [`AbstractionTypeRegistry`](crate::extract::abstraction_types::AbstractionTypeRegistry)
///   with a transport abstraction identifier (currently `"mmio"`).
///   The caller drops its own Call translation; the return value
///   becomes the block's terminator. The destination local (the
///   `&Block` temporary) is recorded in `deref_rewrites` so downstream
///   uses splice through the receiver Place.
/// - `Ok(None)` when the Call is not a transport deref. The caller
///   translates it as a regular `Call` (or, via Rule 1, an
///   `IntrinsicCall`).
/// - `Err(_)` when a transport-deref Call has a shape the collapse
///   cannot handle (no return target, non-`Copy`/`Move` receiver
///   operand, or a destination whose Regseq `LocalId` has no
///   assignment).
#[allow(clippy::too_many_arguments)]
fn try_collapse_transport_deref<'tcx>(
    tcx: TyCtxt<'tcx>,
    callee_def_id: DefId,
    args: &[rustc_span::source_map::Spanned<mir::Operand<'tcx>>],
    destination: &mir::Place<'tcx>,
    target: &Option<mir::BasicBlock>,
    block_map: &[Option<BasicBlockId>],
    locals: &LocalIdMap<'_, 'tcx>,
    type_def_index: &TypeDefIndex<'_>,
    span: rustc_span::Span,
    deref_lookup: &DerefCollapseLookup<'_>,
    deref_rewrites: &mut DerefCollapseRewrites,
    interner: &mut SymbolInterner,
) -> Result<Option<TerminatorKind>, ConstructionError> {
    // `Deref::deref(&self)` takes exactly one argument. If the MIR
    // shape deviates, the caller handles it via its normal fallback.
    let Some(first_arg) = args.first() else {
        return Ok(None);
    };
    let receiver_ty = first_arg.node.ty(locals.local_decls(), tcx);
    let Some(_transport_adt) = deref_lookup.match_transport_deref(tcx, callee_def_id, receiver_ty)
    else {
        return Ok(None);
    };

    // The receiver must be a Place-backed operand so we can record its
    // Regseq Place as the rewrite root. A literal `Deref::deref`
    // receiver would indicate a malformed specimen.
    let receiver_mir = receiver_mir_place(&first_arg.node).ok_or_else(|| {
        ConstructionError::UnresolvedCallTarget {
            provenance: ErrorProvenance {
                item_path: tcx.def_path_str(callee_def_id),
                span,
            },
            rendered: format!(
                "transport Deref::deref receiver is not a Place operand: {:?}",
                first_arg.node,
            ),
        }
    })?;
    let receiver_place = map_place(tcx, receiver_mir, locals, type_def_index, interner)?;

    // `Deref::deref` is infallible and always returns, so `target` must
    // be Some in well-formed MIR. Treat a missing target as a hard
    // error -- the backend would otherwise silently lose the
    // collapse's exit edge.
    let return_target = match target {
        Some(t) => require_block(block_map, *t, span)?,
        None => {
            return Err(ConstructionError::UnsupportedTerminator {
                provenance: ErrorProvenance {
                    item_path: tcx.def_path_str(callee_def_id),
                    span,
                },
                rendered: "transport Deref::deref called as a diverging Call (no target)"
                    .to_string(),
            });
        }
    };

    // The destination is the `&Block` temporary; MIR assigns the call
    // result to a bare local before any projections. Reject any other
    // shape because the rewrite rule only makes sense when the result
    // is held in a single named local.
    if !destination.projection.is_empty() {
        return Err(ConstructionError::UnsupportedProjection {
            provenance: ErrorProvenance {
                item_path: tcx.def_path_str(callee_def_id),
                span,
            },
            rendered: format!(
                "transport Deref::deref destination has projections: {:?}",
                destination,
            ),
        });
    }
    let dest_local_id = locals.lookup(destination.local).ok_or_else(|| {
        ConstructionError::UnsupportedProjection {
            provenance: ErrorProvenance::for_span(span),
            rendered: format!(
                "MIR local _{} (deref destination) not assigned a Regseq LocalId",
                destination.local.as_u32(),
            ),
        }
    })?;

    deref_rewrites.record(dest_local_id, receiver_place);
    Ok(Some(TerminatorKind::Goto(return_target)))
}

/// Resolve a MIR `Call::func` operand to a rustc [`DefId`].
///
/// In monomorphized MIR at `mir_drops_elaborated_and_const_checked`
/// with `mir_opt_level = 0`, direct function calls appear as
/// `Operand::Constant(Const::Val(_, TyKind::FnDef(def_id, _args)))`.
/// Any other shape (indirect pointer call, unresolved constant) is
/// flagged as [`ConstructionError::UnresolvedCallTarget`]. Callers
/// choose between the [`IntrinsicLookup`] (Rule 1) and the
/// [`FunctionIndex`] to turn the [`DefId`] into either an
/// [`TerminatorKind::IntrinsicCall`] or a [`TerminatorKind::Call`].
fn resolve_callee_def_id<'tcx>(
    func: &mir::Operand<'tcx>,
    span: rustc_span::Span,
) -> Result<DefId, ConstructionError> {
    if let mir::Operand::Constant(const_op) = func {
        if let mir::Const::Val(_, ty) = const_op.const_ {
            if let ty::TyKind::FnDef(def_id, _args) = ty.kind() {
                return Ok(*def_id);
            }
        }
    }
    Err(ConstructionError::UnresolvedCallTarget {
        provenance: ErrorProvenance::for_span(span),
        rendered: format!("{func:?}"),
    })
}

/// Recover the monomorphized generic-argument list from a MIR
/// `Call::func` operand. Returns [`None`] if `func` is not a direct
/// `FnDef` constant -- Rule 7 requires the generic args to pull the
/// register-marker `R` type parameter out, so indirect calls have
/// no metadata to attach.
fn callee_generic_args<'tcx>(func: &mir::Operand<'tcx>) -> Option<ty::GenericArgsRef<'tcx>> {
    let mir::Operand::Constant(const_op) = func else {
        return None;
    };
    let mir::Const::Val(_, ty) = const_op.const_ else {
        return None;
    };
    let ty::TyKind::FnDef(_, args) = ty.kind() else {
        return None;
    };
    Some(*args)
}

fn require_block(
    block_map: &[Option<BasicBlockId>],
    target: mir::BasicBlock,
    span: rustc_span::Span,
) -> Result<BasicBlockId, ConstructionError> {
    let idx = target.as_usize();
    match block_map.get(idx).copied().flatten() {
        Some(bb) => Ok(bb),
        None => Err(ConstructionError::UnsupportedTerminator {
            provenance: ErrorProvenance::for_span(span),
            rendered: format!(
                "terminator targets dropped block bb{} (is_cleanup)",
                target.as_u32()
            ),
        }),
    }
}
