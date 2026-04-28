//! Rule 2 `?`-operator desugaring collapse for Pass 2.
//!
//! Implemented by task regseq-model-46.
//!
//! At Pass 2 setup, [`TryMarkers::resolve`] caches the `DefId`s of
//! `Try::branch` and `FromResidual::from_residual` via
//! `tcx.lang_items().branch_fn()` / `from_residual_fn()`. The lang
//! items come from `core::ops` which is always in the dependency
//! graph of driver specs; when the lang items are absent the markers
//! hold `None` and recognition simply does not fire (no folding, no
//! error) -- consistent with the spec's "fallback rule".
//!
//! During Pass 2 body construction, [`TryFoldAnalysis::analyze`]
//! walks the MIR once and locates every `?` instance. For each
//! originating `Call` terminator (the fallible call whose result
//! feeds into `Try::branch`), the analysis records the data needed
//! to emit a single folded `Call` / `IntrinsicCall` in Pass 2's
//! terminator translation:
//!
//! - `new_destination` -- the Continue extraction's `_val` MIR
//!   `Place` (absorbed into the folded terminator's `destination`).
//! - `continue_target` -- the `bb_continue` block (re-used as the
//!   folded terminator's `target`; its Continue extraction statement
//!   is dropped from the translated block).
//! - `error_target` -- the `bb_store` block that follows
//!   `FromResidual::from_residual` (the folded terminator's
//!   `error_target`; its `_0 = move _converted` statement is dropped
//!   from the translated block).
//! - Statement indices to skip on `bb_continue` / `bb_store` during
//!   statement translation.
//!
//! The intermediate `bb_branch`, `bb_switch`, `bb_break`, and
//! `bb_unreachable` blocks hold the `Try::branch` call, the
//! `discriminant(_cf)` + `switchInt`, the `(_cf as Break).0`
//! extraction + `FromResidual::from_residual` call, and the
//! `otherwise` slot respectively. All four become unreachable once
//! the originating Call's successor is rewritten to
//! `(continue_target, error_target)` in the Pass 2 block-map
//! reachability walk, so they are dropped from the Regseq IR along
//! with cleanup blocks.
//!
//! ### Recognition trigger
//!
//! A `Call` terminator is the head of a `?` pattern when:
//!
//! 1. It has a `target` (does not diverge).
//! 2. That `target` block's terminator is a `Call` whose callee
//!    `DefId` is either `Try::branch` directly or an impl method of
//!    the `Try` trait that overrides `branch` (matched via
//!    [`TyCtxt::trait_item_of`]).
//! 3. The `Try::branch` Call's first argument is the originating
//!    Call's destination local (i.e. `_result`).
//!
//! Once the trigger matches, the full pattern shape is validated
//! (see [`recognize`]). Any structural deviation raises
//! [`ConstructionError::MalformedTryPattern`] -- the hard-error
//! policy from the spec ensures we never silently mis-fold a
//! partially-recognized pattern.

use std::collections::{BTreeMap, BTreeSet};

use rustc_abi::VariantIdx;
use rustc_hir::def_id::DefId;
use rustc_middle::mir;
use rustc_middle::ty::{self, TyCtxt};

use super::{ConstructionError, ErrorProvenance};

// ---------------------------------------------------------------------------
// Marker DefIds
// ---------------------------------------------------------------------------

/// Cached `DefId`s for `Try::branch` and `FromResidual::from_residual`.
///
/// Resolved once at Pass 2 setup via [`TyCtxt::lang_items`]. When the
/// crate under compilation does not pull in the lang items (which is
/// vanishingly rare for driver specs, which always depend on
/// `core::ops`), both fields hold `None` and recognition simply does
/// not fire. This mirrors the Deref-collapse / ordered-region
/// handling of absent lang items.
#[derive(Debug, Default, Copy, Clone, Eq, PartialEq)]
pub struct TryMarkers {
    branch: Option<DefId>,
    from_residual: Option<DefId>,
}

impl TryMarkers {
    /// Resolve both marker `DefId`s via `tcx.lang_items()`. Missing
    /// lang items are recorded as `None` -- recognition will not
    /// fire, and no error is raised.
    pub fn resolve(tcx: TyCtxt<'_>) -> Self {
        Self {
            branch: tcx.lang_items().branch_fn(),
            from_residual: tcx.lang_items().from_residual_fn(),
        }
    }

    /// Build markers from explicit `DefId`s. Used by integration
    /// tests that want to provide pre-resolved markers directly.
    pub fn from_def_ids(branch: DefId, from_residual: DefId) -> Self {
        Self {
            branch: Some(branch),
            from_residual: Some(from_residual),
        }
    }

    /// Return `true` if both marker `DefId`s resolved. When either
    /// is missing, recognition cannot fire.
    pub fn is_resolved(&self) -> bool {
        self.branch.is_some() && self.from_residual.is_some()
    }

    /// The `Try::branch` trait-method `DefId`, if resolved.
    pub fn branch_def_id(&self) -> Option<DefId> {
        self.branch
    }

    /// The `FromResidual::from_residual` trait-method `DefId`, if
    /// resolved.
    pub fn from_residual_def_id(&self) -> Option<DefId> {
        self.from_residual
    }

    /// Return `true` if `def_id` is `Try::branch` itself or an impl
    /// method that overrides it. Monomorphized MIR may carry either
    /// form depending on how rustc lowered the call; both must
    /// match.
    fn is_branch(&self, tcx: TyCtxt<'_>, def_id: DefId) -> bool {
        let Some(branch) = self.branch else {
            return false;
        };
        if def_id == branch {
            return true;
        }
        tcx.trait_item_of(def_id) == Some(branch)
    }

    /// Return `true` if `def_id` is `FromResidual::from_residual`
    /// itself or an impl method that overrides it.
    fn is_from_residual(&self, tcx: TyCtxt<'_>, def_id: DefId) -> bool {
        let Some(fr) = self.from_residual else {
            return false;
        };
        if def_id == fr {
            return true;
        }
        tcx.trait_item_of(def_id) == Some(fr)
    }
}

// ---------------------------------------------------------------------------
// Fold info and analysis result
// ---------------------------------------------------------------------------

/// Per-`?` fold record, keyed on the originating `Call`'s MIR block.
///
/// Pass 2's terminator translation consumes this to emit a folded
/// Regseq `Call` / `IntrinsicCall` whose `destination` is the
/// Continue extraction's place, `target` is the `continue_target`
/// block (remapped through the Pass 2 block-map), and `error_target`
/// is the `error_target` block.
#[derive(Debug, Clone)]
pub(crate) struct TryFoldInfo<'tcx> {
    /// MIR `Place` the folded terminator writes into (the `_val`
    /// in `_val = move ((_cf as Continue).0)`).
    pub new_destination: mir::Place<'tcx>,
    /// MIR block reached on the Continue path (typically
    /// `bb_continue`). The Continue extraction statement inside this
    /// block is dropped during statement translation.
    pub continue_target: mir::BasicBlock,
    /// Statement index in `continue_target` that carries the
    /// Continue extraction; dropped during statement translation.
    pub continue_skip_stmt: usize,
    /// MIR block reached on the Break path after
    /// `FromResidual::from_residual` returns (typically `bb_store`).
    /// Its `_0 = move _converted` statement is dropped during
    /// statement translation.
    pub error_target: mir::BasicBlock,
    /// Statement index in `error_target` that carries the
    /// `_0 = move _converted` assignment; `None` if the assignment
    /// is not present (uncommon -- happens only when rustc lowered
    /// the store in another shape). When `None`, no statement is
    /// dropped from `error_target`.
    pub error_skip_stmt: Option<usize>,
    /// MIR locals that the fold eliminates from the Regseq IR --
    /// the `_cf: ControlFlow<...>` temporary (whose type has no
    /// Regseq representation) plus the `_residual` and `_converted`
    /// slots and any stepping-stone locals rustc inserts between
    /// them. Build-time local-decl translation substitutes a
    /// placeholder type for these so map_ty's unknown-ADT rejection
    /// does not fire against the `ControlFlow` definition from
    /// `core::ops`.
    pub dropped_locals: Vec<mir::Local>,
}

/// Body-wide `?`-fold analysis result.
///
/// A map from originating Call MIR block to per-`?` fold info.
/// `is_empty()` is the common case (function has no `?`).
#[derive(Debug, Default, Clone)]
pub(crate) struct TryFoldAnalysis<'tcx> {
    folds: BTreeMap<mir::BasicBlock, TryFoldInfo<'tcx>>,
    dropped_statements: BTreeMap<mir::BasicBlock, BTreeSet<usize>>,
    dropped_locals: BTreeSet<mir::Local>,
}

impl<'tcx> TryFoldAnalysis<'tcx> {
    /// Walk the MIR body once and collect every `?`-fold instance.
    ///
    /// When `markers.is_resolved()` is `false`, returns an empty
    /// analysis without visiting MIR. Otherwise every `Call`
    /// terminator with a `target` is scanned for the Try::branch
    /// trigger; a match kicks off the full
    /// [`recognize`]-based pattern validation, which may raise
    /// [`ConstructionError::MalformedTryPattern`].
    pub(crate) fn analyze(
        tcx: TyCtxt<'tcx>,
        owner_def_id: DefId,
        markers: &TryMarkers,
        mir_body: &mir::Body<'tcx>,
    ) -> Result<Self, ConstructionError> {
        let mut folds: BTreeMap<mir::BasicBlock, TryFoldInfo<'tcx>> = BTreeMap::new();
        let mut dropped: BTreeMap<mir::BasicBlock, BTreeSet<usize>> = BTreeMap::new();
        let mut dropped_locals: BTreeSet<mir::Local> = BTreeSet::new();
        if !markers.is_resolved() {
            return Ok(Self {
                folds,
                dropped_statements: dropped,
                dropped_locals,
            });
        }
        for (bb, data) in mir_body.basic_blocks.iter_enumerated() {
            if data.is_cleanup {
                continue;
            }
            let term = data.terminator();
            let mir::TerminatorKind::Call {
                target: Some(return_bb),
                destination,
                ..
            } = &term.kind
            else {
                continue;
            };
            let Some(info) = recognize(
                tcx,
                owner_def_id,
                markers,
                mir_body,
                bb,
                *return_bb,
                destination,
                term.source_info.span,
            )?
            else {
                continue;
            };
            dropped
                .entry(info.continue_target)
                .or_default()
                .insert(info.continue_skip_stmt);
            if let Some(idx) = info.error_skip_stmt {
                dropped.entry(info.error_target).or_default().insert(idx);
            }
            for local in &info.dropped_locals {
                dropped_locals.insert(*local);
            }
            folds.insert(bb, info);
        }
        Ok(Self {
            folds,
            dropped_statements: dropped,
            dropped_locals,
        })
    }

    /// Look up the fold record attached to `bb` (if `bb` is the
    /// originating Call of a `?` instance).
    pub(crate) fn get(&self, bb: mir::BasicBlock) -> Option<&TryFoldInfo<'tcx>> {
        self.folds.get(&bb)
    }

    /// Return `true` if `(bb, stmt_idx)` names a Continue-extraction
    /// or `_0 = move _converted` statement that is dropped during
    /// statement translation.
    pub(crate) fn is_dropped_statement(&self, bb: mir::BasicBlock, stmt_idx: usize) -> bool {
        self.dropped_statements
            .get(&bb)
            .is_some_and(|s| s.contains(&stmt_idx))
    }

    /// `true` when no `?` was recognized in the body.
    #[allow(dead_code)]
    pub(crate) fn is_empty(&self) -> bool {
        self.folds.is_empty()
    }

    /// Iterate over every recognized fold as `(originating_bb,
    /// info)` pairs in deterministic MIR-block order.
    #[allow(dead_code)]
    pub(crate) fn iter(&self) -> impl Iterator<Item = (mir::BasicBlock, &TryFoldInfo<'tcx>)> {
        self.folds.iter().map(|(bb, info)| (*bb, info))
    }

    /// Return `true` if `local` is a fold intermediate whose
    /// LocalDecl's type must be placeholder-substituted during Pass
    /// 2 build (because its original MIR type -- e.g.
    /// `core::ops::ControlFlow` -- has no Regseq representation).
    pub(crate) fn is_dropped_local(&self, local: mir::Local) -> bool {
        self.dropped_locals.contains(&local)
    }
}

// ---------------------------------------------------------------------------
// Recognition
// ---------------------------------------------------------------------------

/// Validate the `?`-pattern rooted at `bb_branch` (the return target
/// of the originating Call). Returns `Ok(Some(info))` on a full
/// match, `Ok(None)` when the pattern does not begin here (fallback
/// rule), and `Err(MalformedTryPattern)` when the Try::branch
/// trigger fires but a subsequent step fails (hard-error rule).
#[allow(clippy::too_many_arguments)]
fn recognize<'tcx>(
    tcx: TyCtxt<'tcx>,
    owner_def_id: DefId,
    markers: &TryMarkers,
    mir_body: &mir::Body<'tcx>,
    originating_call_bb: mir::BasicBlock,
    bb_branch: mir::BasicBlock,
    originating_destination: &mir::Place<'tcx>,
    span: rustc_span::Span,
) -> Result<Option<TryFoldInfo<'tcx>>, ConstructionError> {
    // Step 1: bb_branch's terminator must be a Call to Try::branch.
    let branch_data = &mir_body.basic_blocks[bb_branch];
    let branch_term = branch_data.terminator();
    let mir::TerminatorKind::Call {
        func,
        args,
        destination: cf_place,
        target: Some(bb_switch),
        ..
    } = &branch_term.kind
    else {
        return Ok(None);
    };
    let Some(branch_callee) = resolve_def_id(func) else {
        return Ok(None);
    };
    if !markers.is_branch(tcx, branch_callee) {
        return Ok(None);
    }

    // Trigger matched -- from here on, every failure is a hard
    // error per the spec. Exception: an argument that does not
    // trace back to the originating Call's destination means the
    // `?` pattern actually belongs to some other Call (e.g. an
    // intermediate `let x = some_fn(); x?` rebind), so we fall back
    // to treating the originating Call as a plain Call.
    if args.len() != 1 {
        return Err(malformed(
            tcx,
            owner_def_id,
            originating_call_bb,
            span,
            "try_branch_arg_count",
        ));
    }
    let branch_arg_place = match &args[0].node {
        mir::Operand::Move(p) | mir::Operand::Copy(p) => p,
        _ => {
            return Err(malformed(
                tcx,
                owner_def_id,
                originating_call_bb,
                span,
                "try_branch_arg_not_place",
            ));
        }
    };
    if branch_arg_place != originating_destination {
        return Ok(None);
    }

    // Step 2: bb_switch carries `_discr = discriminant(_cf)` + a
    // SwitchInt on values [0, 1] with `otherwise: unreachable`.
    let switch_data = &mir_body.basic_blocks[*bb_switch];
    let discr_stmt_idx = find_single_substantive_stmt(switch_data).ok_or_else(|| {
        malformed(
            tcx,
            owner_def_id,
            originating_call_bb,
            span,
            "switch_block_shape",
        )
    })?;
    let discr_stmt = &switch_data.statements[discr_stmt_idx];
    let mir::StatementKind::Assign(boxed) = &discr_stmt.kind else {
        return Err(malformed(
            tcx,
            owner_def_id,
            originating_call_bb,
            span,
            "switch_stmt_not_assign",
        ));
    };
    let (_discr_lhs, rvalue) = &**boxed;
    let discriminant_place = match rvalue {
        mir::Rvalue::Discriminant(p) => p,
        _ => {
            return Err(malformed(
                tcx,
                owner_def_id,
                originating_call_bb,
                span,
                "switch_rvalue_not_discriminant",
            ));
        }
    };
    if discriminant_place != cf_place {
        return Err(malformed(
            tcx,
            owner_def_id,
            originating_call_bb,
            span,
            "discriminant_place_mismatch",
        ));
    }
    let mir::TerminatorKind::SwitchInt {
        targets: switch_targets,
        ..
    } = &switch_data.terminator().kind
    else {
        return Err(malformed(
            tcx,
            owner_def_id,
            originating_call_bb,
            span,
            "switch_terminator_not_switch",
        ));
    };
    let (bb_continue, bb_break, bb_unreachable) = extract_switch_triple(switch_targets)
        .ok_or_else(|| {
            malformed(
                tcx,
                owner_def_id,
                originating_call_bb,
                span,
                "switch_targets_not_continue_break_unreachable",
            )
        })?;

    // Step 2b: otherwise target must be Unreachable.
    let unreachable_term = mir_body.basic_blocks[bb_unreachable].terminator();
    if !is_unreachable_terminator(&unreachable_term.kind) {
        return Err(malformed(
            tcx,
            owner_def_id,
            originating_call_bb,
            span,
            "otherwise_not_unreachable",
        ));
    }

    // Step 3: bb_continue's first substantive statement extracts
    // `_val = move ((_cf as Continue).0)`.
    let continue_data = &mir_body.basic_blocks[bb_continue];
    let (continue_stmt_idx, val_place) =
        find_variant_extraction(continue_data, cf_place, VariantIdx::from_u32(0)).ok_or_else(
            || {
                malformed(
                    tcx,
                    owner_def_id,
                    originating_call_bb,
                    span,
                    "continue_extraction_missing",
                )
            },
        )?;

    // Step 4: bb_break's first substantive statement extracts
    // `_residual = move ((_cf as Break).0)`, followed by a Call to
    // `FromResidual::from_residual(move _residual) -> bb_store`.
    // The real MIR may insert one or more bare-local copy/move
    // stepping-stones between the Break extraction and the
    // `from_residual` Call (rustc generates
    // `_7 = move _5; from_residual(move _7)` at
    // `mir_drops_elaborated_and_const_checked`). The residual
    // identity check therefore walks transitive
    // `Rvalue::Use(Move|Copy(local))` assignments within bb_break
    // and accepts any local in the resulting derivation set.
    let break_data = &mir_body.basic_blocks[bb_break];
    let (break_stmt_idx, residual_place) =
        find_variant_extraction(break_data, cf_place, VariantIdx::from_u32(1)).ok_or_else(
            || {
                malformed(
                    tcx,
                    owner_def_id,
                    originating_call_bb,
                    span,
                    "break_extraction_missing",
                )
            },
        )?;
    let derived_residual_locals =
        collect_move_chain(break_data, residual_place.local, break_stmt_idx);
    let mir::TerminatorKind::Call {
        func: fr_func,
        args: fr_args,
        destination: converted_place,
        target: Some(bb_store),
        ..
    } = &break_data.terminator().kind
    else {
        return Err(malformed(
            tcx,
            owner_def_id,
            originating_call_bb,
            span,
            "break_terminator_not_call",
        ));
    };
    let Some(fr_callee) = resolve_def_id(fr_func) else {
        return Err(malformed(
            tcx,
            owner_def_id,
            originating_call_bb,
            span,
            "from_residual_callee_unresolved",
        ));
    };
    if !markers.is_from_residual(tcx, fr_callee) {
        return Err(malformed(
            tcx,
            owner_def_id,
            originating_call_bb,
            span,
            "break_callee_not_from_residual",
        ));
    }
    if fr_args.len() != 1 {
        return Err(malformed(
            tcx,
            owner_def_id,
            originating_call_bb,
            span,
            "from_residual_arg_count",
        ));
    }
    if let Err(tag) = validate_from_residual_arg(&fr_args[0].node, &derived_residual_locals) {
        return Err(malformed(tcx, owner_def_id, originating_call_bb, span, tag));
    }

    // Step 5 (optional): bb_store carries `_0 = move _converted`
    // somewhere in its statement list. Locate it so statement
    // translation can drop it. If the shape is absent we tolerate
    // that -- the algorithm's invariant is "residual temporaries
    // do not survive folding," and a missing write means the
    // frontend already fused the store away.
    let store_data = &mir_body.basic_blocks[*bb_store];
    let error_skip_stmt = find_return_place_store(store_data, converted_place);

    // Collect every intermediate MIR local the fold eliminates.
    // The ir-reference.md algorithm and the task spec both require
    // these temporaries to be dropped from the Regseq IR (so the
    // resulting body carries no `ControlFlow<...>` LocalDecl, no
    // residual temporary, and no Try-wrapper temp). Per-fold set:
    //   - `originating_destination.local` -- `_result`, the MIR
    //     destination of the originating Call before folding.
    //     Only dropped when the original Place is a bare local;
    //     otherwise the base local may be live (e.g. when the
    //     destination projects into `_0` -- the return place --
    //     directly, which rustc does for a trailing `fn() -> T?`).
    //   - `cf_place.local`   -- `_cf`, Try::branch destination.
    //   - `residual_place.local` -- `_residual`, Break extraction.
    //   - `converted_place.local` -- `_converted`, FromResidual dest.
    //   - every move-chain stepping-stone between `_residual` and
    //     the `FromResidual` Call's argument (e.g. rustc inserts
    //     `_7 = move _5` at mir_drops_elaborated).
    // Helper closure: only drop a local if it is a bare local
    // (no projection) AND it is not the MIR return place `_0`.
    // The return place is always live -- rustc often writes
    // FromResidual's result directly into `_0` rather than into a
    // synthetic `_converted` temp, so dropping it would remove the
    // function's return slot.
    let mut dropped_locals: Vec<mir::Local> = Vec::new();
    let push_if_droppable = |place: &mir::Place<'tcx>, out: &mut Vec<mir::Local>| {
        if !place.projection.is_empty() {
            return;
        }
        if place.local == mir::RETURN_PLACE {
            return;
        }
        if !out.contains(&place.local) {
            out.push(place.local);
        }
    };
    push_if_droppable(originating_destination, &mut dropped_locals);
    push_if_droppable(cf_place, &mut dropped_locals);
    push_if_droppable(&residual_place, &mut dropped_locals);
    push_if_droppable(converted_place, &mut dropped_locals);
    for local in derived_residual_locals.iter() {
        if *local != mir::RETURN_PLACE && !dropped_locals.contains(local) {
            dropped_locals.push(*local);
        }
    }

    Ok(Some(TryFoldInfo {
        new_destination: val_place,
        continue_target: bb_continue,
        continue_skip_stmt: continue_stmt_idx,
        error_target: *bb_store,
        error_skip_stmt,
        dropped_locals,
    }))
}

/// Construct a `MalformedTryPattern` error for `owner_def_id`'s Call
/// at `originating_call_bb` with a short `step` tag naming which
/// sub-check failed. The tag is plumbed through the error's
/// `rendered` field.
fn malformed(
    tcx: TyCtxt<'_>,
    owner_def_id: DefId,
    originating_call_bb: mir::BasicBlock,
    span: rustc_span::Span,
    step: &'static str,
) -> ConstructionError {
    ConstructionError::MalformedTryPattern {
        provenance: ErrorProvenance {
            item_path: tcx.def_path_str(owner_def_id),
            span,
        },
        call_mir_block: originating_call_bb.as_u32(),
        step,
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Resolve a MIR `Call`'s `func` operand to a `DefId`, if the
/// operand is a direct `FnDef` constant.
fn resolve_def_id<'tcx>(func: &mir::Operand<'tcx>) -> Option<DefId> {
    let mir::Operand::Constant(const_op) = func else {
        return None;
    };
    let mir::Const::Val(_, ty) = const_op.const_ else {
        return None;
    };
    let ty::TyKind::FnDef(def_id, _args) = ty.kind() else {
        return None;
    };
    Some(*def_id)
}

/// Find the single "substantive" statement in a block (ignoring
/// `StorageLive` / `StorageDead` markers). Returns `None` when the
/// block has zero or multiple substantive statements.
fn find_single_substantive_stmt<'tcx>(data: &mir::BasicBlockData<'tcx>) -> Option<usize> {
    let mut found: Option<usize> = None;
    for (idx, stmt) in data.statements.iter().enumerate() {
        if is_storage_marker(&stmt.kind) {
            continue;
        }
        if found.is_some() {
            return None;
        }
        found = Some(idx);
    }
    found
}

/// Scan for an `Assign` statement whose rvalue is
/// `Rvalue::Use(Operand::Move|Copy(Place { local: cf_place.local,
/// projection: [Downcast(_, variant), Field(0, _), ..] }))`, where
/// the leading projection chain extends `cf_place`. Returns the
/// statement index and the LHS's `Place`.
fn find_variant_extraction<'tcx>(
    data: &mir::BasicBlockData<'tcx>,
    cf_place: &mir::Place<'tcx>,
    variant: VariantIdx,
) -> Option<(usize, mir::Place<'tcx>)> {
    for (idx, stmt) in data.statements.iter().enumerate() {
        if is_storage_marker(&stmt.kind) {
            continue;
        }
        let mir::StatementKind::Assign(boxed) = &stmt.kind else {
            return None;
        };
        let (lhs, rvalue) = &**boxed;
        let mir::Rvalue::Use(operand) = rvalue else {
            return None;
        };
        let source_place = match operand {
            mir::Operand::Move(p) | mir::Operand::Copy(p) => p,
            _ => return None,
        };
        if source_place.local != cf_place.local {
            return None;
        }
        // The first two projection steps must be
        // `Downcast(variant)` then `Field(0, _)`. Additional
        // projections are tolerated so packed tuples inside the
        // variant still parse.
        let proj = source_place.projection.as_ref();
        let mut elems = proj.iter();
        let Some(mir::ProjectionElem::Downcast(_, got_variant)) = elems.next() else {
            return None;
        };
        if got_variant.as_u32() != variant.as_u32() {
            return None;
        }
        let Some(mir::ProjectionElem::Field(field_idx, _)) = elems.next() else {
            return None;
        };
        if field_idx.as_u32() != 0 {
            return None;
        }
        return Some((idx, *lhs));
    }
    None
}

/// Look for `_0 = move _converted` somewhere in the block's
/// statements. `_0` is MIR's `RETURN_PLACE` with no projection.
/// Returns the statement index when the shape matches.
fn find_return_place_store<'tcx>(
    data: &mir::BasicBlockData<'tcx>,
    converted_place: &mir::Place<'tcx>,
) -> Option<usize> {
    for (idx, stmt) in data.statements.iter().enumerate() {
        let mir::StatementKind::Assign(boxed) = &stmt.kind else {
            continue;
        };
        let (lhs, rvalue) = &**boxed;
        if lhs.local != mir::RETURN_PLACE || !lhs.projection.is_empty() {
            continue;
        }
        let mir::Rvalue::Use(operand) = rvalue else {
            continue;
        };
        let source_place = match operand {
            mir::Operand::Move(p) | mir::Operand::Copy(p) => p,
            _ => continue,
        };
        if source_place == converted_place {
            return Some(idx);
        }
    }
    None
}

/// `SwitchInt` must have values `[0, 1]` with three targets --
/// `[bb_continue, bb_break, bb_unreachable]`. Returns that triple.
fn extract_switch_triple(
    targets: &mir::SwitchTargets,
) -> Option<(mir::BasicBlock, mir::BasicBlock, mir::BasicBlock)> {
    let pairs: Vec<(u128, mir::BasicBlock)> = targets.iter().collect();
    if pairs.len() != 2 {
        return None;
    }
    if pairs[0].0 != 0 || pairs[1].0 != 1 {
        return None;
    }
    Some((pairs[0].1, pairs[1].1, targets.otherwise()))
}

/// Whether a terminator kind is `Unreachable` -- the `otherwise`
/// slot of a Try-desugaring SwitchInt must carry this shape per the
/// algorithm in `ir-reference.md` Section 5. Extracted as a pure
/// function so the hard-error branch
/// `otherwise_not_unreachable` can be unit-tested without wiring
/// up a full `TyCtxt` / `mir::Body`.
fn is_unreachable_terminator<'tcx>(kind: &mir::TerminatorKind<'tcx>) -> bool {
    matches!(kind, mir::TerminatorKind::Unreachable)
}

/// Verify that the operand of a `FromResidual::from_residual` Call
/// is a `Move`/`Copy` Place with no projections and whose root
/// local is a member of the move-chain derivation set reachable
/// from the Break extraction. Extracted as a pure function so the
/// hard-error branches `from_residual_arg_not_place`,
/// `from_residual_arg_has_projection`, and
/// `from_residual_arg_not_residual` can be unit-tested without
/// constructing a full `TyCtxt`.
fn validate_from_residual_arg<'tcx>(
    operand: &mir::Operand<'tcx>,
    derived_locals: &BTreeSet<mir::Local>,
) -> Result<(), &'static str> {
    let place = match operand {
        mir::Operand::Move(p) | mir::Operand::Copy(p) => p,
        _ => return Err("from_residual_arg_not_place"),
    };
    check_residual_root(place.projection, place.local, derived_locals)
}

/// Inner validator: the arg Place must have no projections and its
/// root local must appear in `derived_locals`. Split from
/// [`validate_from_residual_arg`] so unit tests can reach the
/// `from_residual_arg_has_projection` / `_not_residual` branches
/// without building a `mir::Place` (which requires a live `TyCtxt`
/// to intern the `projection: &'tcx List<...>` slice). Generic over
/// the projection element type so tests can pass an ordinary slice.
fn check_residual_root<P>(
    projection: &[P],
    local: mir::Local,
    derived_locals: &BTreeSet<mir::Local>,
) -> Result<(), &'static str> {
    if !projection.is_empty() {
        return Err("from_residual_arg_has_projection");
    }
    if !derived_locals.contains(&local) {
        return Err("from_residual_arg_not_residual");
    }
    Ok(())
}

/// Whether a statement kind is a MIR storage marker (ignored during
/// pattern validation).
fn is_storage_marker<'tcx>(kind: &mir::StatementKind<'tcx>) -> bool {
    matches!(
        kind,
        mir::StatementKind::StorageLive(_) | mir::StatementKind::StorageDead(_)
    )
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // The SwitchInt-triple extractor is a pure function over a
    // `mir::SwitchTargets` value, so we can exercise every
    // hard-error branch without wiring up a full `TyCtxt` /
    // `mir::Body`. Negative tests 2 and 3 from the spec (malformed
    // SwitchInt shape, wrong `otherwise` terminator) depend on this
    // extractor; negative tests 4 and 5 (missing FromResidual,
    // wrong arg shape) live behind the full `recognize` function
    // and are covered by the integration tests that construct real
    // MIR through rustc_driver.

    #[test]
    fn switch_triple_accepts_canonical_continue_break_otherwise() {
        let targets = mir::SwitchTargets::new(
            [
                (0u128, mir::BasicBlock::from_u32(10)),
                (1u128, mir::BasicBlock::from_u32(11)),
            ]
            .into_iter(),
            mir::BasicBlock::from_u32(12),
        );
        let got = extract_switch_triple(&targets).expect("canonical shape must extract");
        assert_eq!(got.0.as_u32(), 10);
        assert_eq!(got.1.as_u32(), 11);
        assert_eq!(got.2.as_u32(), 12);
    }

    #[test]
    fn switch_triple_rejects_extra_value() {
        let targets = mir::SwitchTargets::new(
            [
                (0u128, mir::BasicBlock::from_u32(10)),
                (1u128, mir::BasicBlock::from_u32(11)),
                (2u128, mir::BasicBlock::from_u32(12)),
            ]
            .into_iter(),
            mir::BasicBlock::from_u32(13),
        );
        assert!(
            extract_switch_triple(&targets).is_none(),
            "3-value SwitchInt must not match the Continue/Break shape",
        );
    }

    #[test]
    fn switch_triple_rejects_non_zero_first_value() {
        let targets = mir::SwitchTargets::new(
            [
                (5u128, mir::BasicBlock::from_u32(10)),
                (1u128, mir::BasicBlock::from_u32(11)),
            ]
            .into_iter(),
            mir::BasicBlock::from_u32(12),
        );
        assert!(
            extract_switch_triple(&targets).is_none(),
            "values other than [0, 1] cannot be a Try-desugaring SwitchInt",
        );
    }

    #[test]
    fn switch_triple_rejects_single_value() {
        let targets = mir::SwitchTargets::new(
            [(0u128, mir::BasicBlock::from_u32(10))].into_iter(),
            mir::BasicBlock::from_u32(11),
        );
        assert!(
            extract_switch_triple(&targets).is_none(),
            "Continue/Break requires exactly two values, not one",
        );
    }

    #[test]
    fn try_markers_default_is_unresolved() {
        let m = TryMarkers::default();
        assert!(!m.is_resolved());
        assert!(m.branch_def_id().is_none());
        assert!(m.from_residual_def_id().is_none());
    }

    // ---- `otherwise_not_unreachable` hard-error isolation ----
    //
    // The algorithm's step 2b demands the SwitchInt's `otherwise`
    // target carry `TerminatorKind::Unreachable`. `recognize` gates
    // that through `is_unreachable_terminator`. The spec
    // Test Plan's negative test 3 asks for a fixture where the
    // otherwise slot is NOT `Unreachable`; that shape cannot arise
    // from valid Rust `?` desugaring (rustc always emits
    // `Unreachable`), so isolation is exercised here against
    // synthetic `TerminatorKind` values.

    #[test]
    fn is_unreachable_terminator_accepts_unreachable() {
        assert!(is_unreachable_terminator(&mir::TerminatorKind::Unreachable));
    }

    #[test]
    fn is_unreachable_terminator_rejects_return() {
        assert!(!is_unreachable_terminator(&mir::TerminatorKind::Return));
    }

    #[test]
    fn is_unreachable_terminator_rejects_goto() {
        assert!(!is_unreachable_terminator(&mir::TerminatorKind::Goto {
            target: mir::BasicBlock::from_u32(99),
        }));
    }

    // ---- `from_residual_arg_*` hard-error isolation ----
    //
    // Spec Test Plan negative test 5: a `FromResidual` Call whose
    // DefId matches but whose argument shape is wrong must raise
    // `MalformedTryPattern`. `validate_from_residual_arg` fans out
    // into the three documented sub-cases:
    //   - argument is not a Place operand (`from_residual_arg_not_place`).
    //   - argument has projections (`from_residual_arg_has_projection`).
    //   - argument's root local is not in the Break-extraction move
    //     chain (`from_residual_arg_not_residual`).
    // The canonical shape is a `Move(Place(local, []))` whose root
    // local is the Break extraction destination; `recognize`
    // already plumbs that through in the happy-path integration
    // tests.

    fn local(n: u32) -> mir::Local {
        mir::Local::from_u32(n)
    }

    fn set_of(locals: &[u32]) -> BTreeSet<mir::Local> {
        locals.iter().copied().map(local).collect()
    }

    fn bare_move(local: mir::Local) -> mir::Operand<'static> {
        mir::Operand::Move(mir::Place {
            local,
            projection: rustc_middle::ty::List::empty(),
        })
    }

    #[test]
    fn validate_from_residual_arg_accepts_derived_local() {
        let derived = set_of(&[5, 7]);
        let operand = bare_move(local(5));
        assert!(validate_from_residual_arg(&operand, &derived).is_ok());
    }

    #[test]
    fn validate_from_residual_arg_accepts_move_chain_stepping_stone() {
        let derived = set_of(&[5, 7]);
        let operand = bare_move(local(7));
        assert!(validate_from_residual_arg(&operand, &derived).is_ok());
    }

    // Skipped: `Operand::Constant` requires a live `TyCtxt` to
    // build a `Const`, so we cannot synthesize that variant in
    // a leaf unit test. The "not a Place" branch
    // (`from_residual_arg_not_place`) is covered by inspection of
    // `validate_from_residual_arg`: every variant other than
    // `Move` / `Copy` returns the `not_place` tag, and the
    // integration tests demonstrate the rustc-generated
    // canonical shape uses those two variants exclusively.

    #[test]
    fn validate_from_residual_arg_rejects_unrelated_local() {
        let derived = set_of(&[5]);
        let operand = bare_move(local(11));
        assert_eq!(
            validate_from_residual_arg(&operand, &derived),
            Err("from_residual_arg_not_residual"),
        );
    }

    #[test]
    fn validate_from_residual_arg_rejects_copy_of_unrelated_local() {
        let derived = set_of(&[5]);
        let operand = mir::Operand::Copy(mir::Place {
            local: local(99),
            projection: rustc_middle::ty::List::empty(),
        });
        assert_eq!(
            validate_from_residual_arg(&operand, &derived),
            Err("from_residual_arg_not_residual"),
        );
    }

    // `check_residual_root` is generic over the projection element
    // type, so we use `()` placeholders to synthesize a non-empty
    // projection slice without needing a live `TyCtxt` to intern
    // a real `List<ProjectionElem<..>>`. The structural check we
    // want to unit-test is `projection.is_empty()`; the element
    // type never participates.

    #[test]
    fn check_residual_root_accepts_bare_derived_local() {
        let derived = set_of(&[5]);
        let projection: [(); 0] = [];
        assert!(check_residual_root(&projection, local(5), &derived).is_ok());
    }

    #[test]
    fn check_residual_root_rejects_projected_place() {
        let derived = set_of(&[5]);
        let projection = [(), ()]; // any non-empty slice suffices
        assert_eq!(
            check_residual_root(&projection, local(5), &derived),
            Err("from_residual_arg_has_projection"),
        );
    }

    #[test]
    fn check_residual_root_rejects_empty_projection_with_unrelated_local() {
        let derived = set_of(&[5]);
        let projection: [(); 0] = [];
        assert_eq!(
            check_residual_root(&projection, local(99), &derived),
            Err("from_residual_arg_not_residual"),
        );
    }

    #[test]
    fn check_residual_root_reports_projection_before_local_check() {
        // When both deviations are present, the has-projection
        // branch must fire first so the diagnostic tag is stable.
        let derived = set_of(&[5]);
        let projection = [()];
        assert_eq!(
            check_residual_root(&projection, local(99), &derived),
            Err("from_residual_arg_has_projection"),
        );
    }
}

/// Walk the statements of `data` past `after_stmt_idx` and build the
/// set of locals transitively derived from `root` via simple
/// `Assign(lhs, Rvalue::Use(Operand::Move|Copy(rhs)))` statements
/// where both `lhs` and `rhs` are bare locals (no projection).
///
/// Rustc at `mir_drops_elaborated_and_const_checked` inserts
/// stepping-stone assignments between the Break extraction and the
/// `FromResidual` Call (for example
/// `_7 = move _5; from_residual(move _7)`). Any local reachable via
/// that chain is an acceptable `FromResidual` argument from the
/// fold's perspective.
fn collect_move_chain<'tcx>(
    data: &mir::BasicBlockData<'tcx>,
    root: mir::Local,
    after_stmt_idx: usize,
) -> BTreeSet<mir::Local> {
    let mut set: BTreeSet<mir::Local> = BTreeSet::new();
    set.insert(root);
    for stmt in data.statements.iter().skip(after_stmt_idx + 1) {
        let mir::StatementKind::Assign(boxed) = &stmt.kind else {
            continue;
        };
        let (lhs, rvalue) = &**boxed;
        if !lhs.projection.is_empty() {
            continue;
        }
        let mir::Rvalue::Use(operand) = rvalue else {
            continue;
        };
        let source_place = match operand {
            mir::Operand::Move(p) | mir::Operand::Copy(p) => p,
            _ => continue,
        };
        if !source_place.projection.is_empty() {
            continue;
        }
        if set.contains(&source_place.local) {
            set.insert(lhs.local);
        }
    }
    set
}
