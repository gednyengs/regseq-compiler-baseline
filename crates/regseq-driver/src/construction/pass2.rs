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
//! - Intrinsic recognition is out of scope (task 45).
//! - `?`-pattern folding is out of scope (task 46). `error_target` is
//!   always `None`.
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

use super::mir_mapping::{
    map_operand, map_place, map_rvalue, map_ty, ConstantIndex, FunctionIndex, LocalIdMap,
    MapOperandResult, TypeDefIndex,
};
use super::{resolve_origin, ConstructionError, ErrorProvenance};

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
pub fn build_body<'tcx>(
    tcx: TyCtxt<'tcx>,
    def_id: DefId,
    function_id: FunctionId,
    type_def_index: &TypeDefIndex<'_>,
    constants: &ConstantIndex<'_>,
    functions: &FunctionIndex<'_>,
    interner: &mut SymbolInterner,
) -> Result<Body, ConstructionError> {
    // `function_id` is currently not consumed inside `build_body` -- the
    // orchestrator uses it to reattach the returned body. Keep it in the
    // signature so callers cannot accidentally misuse the output against
    // a different function slot.
    let _ = function_id;

    let mir_body = tcx.optimized_mir(def_id);

    let block_map = build_block_map(mir_body);
    let local_name_map = build_local_name_map(mir_body, interner);
    let (local_decls, local_sources, local_id_map) =
        build_local_decls(tcx, mir_body, &local_name_map, type_def_index, interner)?;

    let mut basic_blocks: Vec<BasicBlock> = Vec::new();
    let mut block_sources: Vec<BasicBlockSourceMap> = Vec::new();
    for (mir_bb, data) in mir_body.basic_blocks.iter_enumerated() {
        if block_map[mir_bb.as_usize()].is_none() {
            continue;
        }

        let mut statements: Vec<Statement> = Vec::new();
        let mut statement_sources: Vec<SourceOrigin> = Vec::new();
        for stmt in &data.statements {
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
            term_data,
            &block_map,
            &local_id_map,
            type_def_index,
            constants,
            functions,
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

    let body_span = resolve_origin(tcx, interner, mir_body.span);

    Ok(Body {
        local_decls,
        arg_count: mir_body.arg_count,
        basic_blocks,
        source_map: BodySourceMap {
            body_span,
            locals: local_sources,
            blocks: block_sources,
        },
        ordered_regions: Vec::new(),
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
fn build_block_map<'tcx>(mir_body: &mir::Body<'tcx>) -> Vec<Option<BasicBlockId>> {
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
        for succ in non_unwind_successors(data) {
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
fn build_local_decls<'a, 'tcx>(
    tcx: TyCtxt<'tcx>,
    mir_body: &'a mir::Body<'tcx>,
    name_map: &HashMap<mir::Local, Symbol>,
    type_def_index: &TypeDefIndex<'_>,
    interner: &mut SymbolInterner,
) -> Result<(Vec<LocalDecl>, Vec<SourceOrigin>, LocalIdMap<'a, 'tcx>), ConstructionError> {
    let mut local_id_map = LocalIdMap::new(&mir_body.local_decls);
    let mut decls = Vec::with_capacity(mir_body.local_decls.len());
    let mut sources = Vec::with_capacity(mir_body.local_decls.len());
    for (local, decl) in mir_body.local_decls.iter_enumerated() {
        local_id_map.insert(local, LocalId::new(local.as_u32()));
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
    term: &mir::Terminator<'tcx>,
    block_map: &[Option<BasicBlockId>],
    locals: &LocalIdMap<'_, 'tcx>,
    type_def_index: &TypeDefIndex<'_>,
    constants: &ConstantIndex<'_>,
    functions: &FunctionIndex<'_>,
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
            let callee = resolve_callee(tcx, func, functions, term.source_info.span)?;
            let mut mapped_args: Vec<regseq_model::ir::Operand> = Vec::with_capacity(args.len());
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
            let dest = map_place(tcx, destination, locals, type_def_index, interner)?;
            let target = match target {
                Some(t) => Some(require_block(block_map, *t, term.source_info.span)?),
                None => None,
            };
            Ok(TerminatorKind::Call {
                callee,
                args: mapped_args,
                destination: dest,
                target,
                error_target: None,
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

/// Resolve a MIR `Call::func` operand to a Regseq [`FunctionId`].
///
/// In monomorphized MIR at `mir_drops_elaborated_and_const_checked`
/// with `mir_opt_level = 0`, direct function calls appear as
/// `Operand::Constant(Const::Val(_, TyKind::FnDef(def_id, _args)))`.
/// Any other shape (indirect pointer call, unresolved constant) is
/// flagged as [`ConstructionError::UnresolvedCallTarget`].
fn resolve_callee<'tcx>(
    tcx: TyCtxt<'tcx>,
    func: &mir::Operand<'tcx>,
    functions: &FunctionIndex<'_>,
    span: rustc_span::Span,
) -> Result<FunctionId, ConstructionError> {
    if let mir::Operand::Constant(const_op) = func {
        if let mir::Const::Val(_, ty) = const_op.const_ {
            if let ty::TyKind::FnDef(def_id, _args) = ty.kind() {
                return functions.lookup(*def_id).ok_or_else(|| {
                    ConstructionError::UnresolvedCallTarget {
                        provenance: ErrorProvenance {
                            item_path: tcx.def_path_str(*def_id),
                            span,
                        },
                        rendered: tcx.def_path_str(*def_id),
                    }
                });
            }
        }
    }
    Err(ConstructionError::UnresolvedCallTarget {
        provenance: ErrorProvenance::for_span(span),
        rendered: format!("{func:?}"),
    })
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
