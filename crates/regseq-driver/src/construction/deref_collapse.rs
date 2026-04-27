//! Rule 4 deref-chain collapse for Pass 2.
//!
//! Implemented by task regseq-model-48.
//!
//! `<Mmio<T> as Deref>::deref` (and any other transport type registered
//! with a transport abstraction via `#[rseq::abstraction("mmio")]` and
//! friends) is recognized during terminator translation and collapsed
//! into transparent access to the inner register block. The primary
//! register-access path does NOT go through `Deref::deref` -- register
//! intrinsics live on `Mmio<T>` directly (spec.md Section 10.2), so
//! this rule only triggers for rare explicit `Deref::deref` uses in
//! spec code.
//!
//! Detection:
//!
//! - At Pass 2 setup, [`DerefCollapseLookup::build`] resolves the
//!   `Deref` trait's `deref` method `DefId` via
//!   `tcx.lang_items().deref_trait()` and caches it alongside a
//!   reference to the [`AbstractionTypeRegistry`].
//! - During terminator translation, a [`mir::TerminatorKind::Call`]
//!   whose callee `DefId` is either `Deref::deref` directly or an impl
//!   of the `Deref` trait's `deref` method (via
//!   [`TyCtxt::trait_item_of`]) and whose receiver type (from the
//!   first-argument `mir::Operand` type, with references peeled) has
//!   its `DefId` registered with a transport abstraction is recognized
//!   as a transport deref.
//!
//! Collapse:
//!
//! - The Call terminator is dropped and replaced with an unconditional
//!   [`TerminatorKind::Goto`] to the original Call's return target.
//! - The destination local (the `&Block` temporary) is recorded in a
//!   [`DerefCollapseRewrites`] table alongside the receiver's Regseq
//!   [`Place`]. The receiver Place carries the transport identity
//!   through to downstream register-metadata resolution (task 51).
//! - After every block is translated, [`rewrite_body_places`] walks
//!   the constructed body and rewrites every Place whose root local is
//!   a recorded destination: the place is re-rooted at the receiver's
//!   base local, prefixed with the receiver's projection chain, and --
//!   if the original place projected off the deref result -- an extra
//!   [`Projection::Deref`] is inserted to bridge the transport
//!   abstraction (so that `(*deref_result).field` becomes
//!   `(*receiver).(deref-bridge).field`, preserving the two levels of
//!   indirection the backend needs to see). Bare uses of the
//!   destination local (no projections) are replaced by the receiver
//!   place verbatim -- no `Deref` prefix is inserted because the
//!   original use did not project off the deref result.
//!
//! Limited scope:
//!
//! - Collapse only applies when the receiver's ADT `DefId` is
//!   registered in the [`AbstractionTypeRegistry`] with one of the
//!   identifiers in [`TRANSPORT_ABSTRACTIONS`].
//!   `Deref::deref` on any other type translates as a regular `Call`.
//! - No path-based detection, no hardcoded type list: the transport
//!   set is exactly the registered-abstraction set filtered by the
//!   string identifiers in [`TRANSPORT_ABSTRACTIONS`].

use std::collections::HashMap;

use rustc_hir::def_id::DefId;
use rustc_middle::mir;
use rustc_middle::ty::{self, TyCtxt};

use regseq_model::identity::LocalId;
use regseq_model::ir::{
    BasicBlock, MirIntrinsic, Operand, Place, Projection, Rvalue, Statement, StatementKind,
    Terminator, TerminatorKind,
};

use crate::extract::abstraction_types::AbstractionTypeRegistry;

use super::mir_mapping::LocalIdMap;
use super::{ConstructionError, ErrorProvenance};

/// Abstraction identifiers (as registered via
/// `#[rseq::abstraction("...")]`) recognized as transports for the
/// purposes of Rule 4 collapse. Extending this list is the sole
/// mechanism for adding new transport types (e.g. `"i2c"`, `"spi"`)
/// to the collapse rule -- detection is keyed exclusively off the
/// [`AbstractionTypeRegistry`].
pub const TRANSPORT_ABSTRACTIONS: &[&str] = &["mmio"];

// ---------------------------------------------------------------------------
// Lookup
// ---------------------------------------------------------------------------

/// Pass 2 lookup for Rule 4 deref collapse.
///
/// Caches the `DefId` of the `Deref` trait's `deref` method via
/// [`TyCtxt::lang_items`] so every Call recognition check is an `Eq`
/// comparison against pre-resolved `DefId`s -- not a path string
/// match. Holds a borrow of the [`AbstractionTypeRegistry`] so the
/// receiver's transport classification is resolved through the same
/// live `HashMap<DefId, String>` Phase 4 produced.
#[derive(Debug, Clone, Copy)]
pub struct DerefCollapseLookup<'a> {
    deref_method: DefId,
    abstraction_types: &'a AbstractionTypeRegistry,
}

impl<'a> DerefCollapseLookup<'a> {
    /// Build the lookup from a live [`TyCtxt`] and the Phase 4
    /// abstraction-types registry.
    ///
    /// Resolves `core::ops::Deref::deref`'s `DefId` up-front by
    /// consulting `tcx.lang_items().deref_trait()` for the trait and
    /// then selecting the trait's `deref` associated item by name.
    /// The pinned toolchain always exposes this lang item on every
    /// Regseq-spec crate (driver specs use `#![no_std]` but depend on
    /// `core`), so any failure here indicates a broken toolchain and
    /// is surfaced as [`ConstructionError::NoDerefLangItem`] rather
    /// than silently disabling Rule 4.
    pub fn build(
        tcx: TyCtxt<'_>,
        abstraction_types: &'a AbstractionTypeRegistry,
    ) -> Result<Self, ConstructionError> {
        let deref_trait =
            tcx.lang_items()
                .deref_trait()
                .ok_or_else(|| ConstructionError::NoDerefLangItem {
                    crate_root: ErrorProvenance::for_def_id(
                        tcx,
                        rustc_hir::def_id::CRATE_DEF_ID.to_def_id(),
                    ),
                })?;
        let deref_method = tcx
            .associated_items(deref_trait)
            .in_definition_order()
            .find(|item| item.is_fn() && item.opt_name() == Some(rustc_span::sym::deref))
            .map(|item| item.def_id)
            .ok_or_else(|| ConstructionError::NoDerefLangItem {
                crate_root: ErrorProvenance::for_def_id(tcx, deref_trait),
            })?;
        Ok(Self {
            deref_method,
            abstraction_types,
        })
    }

    /// Return `true` if `callee_def_id` is the `Deref::deref` trait
    /// method itself, or an impl of that trait method. Otherwise
    /// `false`.
    fn is_deref_method(&self, tcx: TyCtxt<'_>, callee_def_id: DefId) -> bool {
        if callee_def_id == self.deref_method {
            return true;
        }
        tcx.trait_item_of(callee_def_id) == Some(self.deref_method)
    }

    /// If the Call is a transport `Deref::deref`, return the
    /// receiver's ADT `DefId`. Otherwise return `None`.
    ///
    /// `receiver_ty` is the rustc type of the first argument of the
    /// Call at its call site (i.e., `&Mmio<T>` or similar). References
    /// are peeled before the ADT check, so wrapped receivers
    /// (`&&Mmio<T>`) still resolve.
    pub fn match_transport_deref<'tcx>(
        &self,
        tcx: TyCtxt<'tcx>,
        callee_def_id: DefId,
        receiver_ty: ty::Ty<'tcx>,
    ) -> Option<DefId> {
        if !self.is_deref_method(tcx, callee_def_id) {
            return None;
        }
        let peeled = receiver_ty.peel_refs();
        let ty::TyKind::Adt(adt, _) = peeled.kind() else {
            return None;
        };
        let adt_did = adt.did();
        let abstraction = self.abstraction_types.lookup(adt_did)?;
        if TRANSPORT_ABSTRACTIONS.contains(&abstraction) {
            Some(adt_did)
        } else {
            None
        }
    }

    /// Number of recognized transport abstractions currently visible
    /// to this lookup -- i.e., the intersection of the
    /// [`AbstractionTypeRegistry`]'s entries and
    /// [`TRANSPORT_ABSTRACTIONS`]. Useful for test diagnostics.
    pub fn transport_abstraction_count(&self) -> usize {
        self.abstraction_types
            .iter()
            .filter(|(_, ident)| TRANSPORT_ABSTRACTIONS.contains(ident))
            .count()
    }

    /// The resolved `Deref::deref` `DefId` cached by [`Self::build`].
    /// Exposed so tests can verify the lookup actually points at the
    /// pinned-toolchain's Deref lang-item fn.
    pub fn deref_method(&self) -> DefId {
        self.deref_method
    }
}

// ---------------------------------------------------------------------------
// Rewrites
// ---------------------------------------------------------------------------

/// Deref-collapse rewrite table built during terminator translation.
///
/// Maps each collapsed Call's destination `LocalId` (the `&Block`
/// temporary) to the receiver's Regseq [`Place`] (the `&Mmio<T>`
/// argument, translated through [`super::mir_mapping::map_place`]).
/// Consumed post-hoc by [`rewrite_body_places`].
#[derive(Debug, Default, Clone)]
pub struct DerefCollapseRewrites {
    entries: HashMap<LocalId, Place>,
}

impl DerefCollapseRewrites {
    /// Record that `dest` (the Call's destination local) should be
    /// transparently rewritten to `receiver` wherever it appears
    /// downstream. Two collapses with the same destination would
    /// indicate aliasing of SSA temporaries by MIR and is not expected
    /// at `mir_opt_level = 0`; the later insert wins.
    pub fn record(&mut self, dest: LocalId, receiver: Place) {
        self.entries.insert(dest, receiver);
    }

    /// Number of recorded rewrites.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether no rewrites have been recorded.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Read-only access to the underlying table. Exposed for tests and
    /// downstream tasks that need to trace receiver identity.
    pub fn entries(&self) -> &HashMap<LocalId, Place> {
        &self.entries
    }

    /// Apply the rewrite rule to a single Regseq [`Place`].
    ///
    /// If `place.local` was recorded as a collapsed destination,
    /// re-root the place at the receiver:
    ///
    /// - Bare uses (`projection` empty) become the receiver place
    ///   verbatim.
    /// - Projected uses (`projection` non-empty) get the receiver's
    ///   own projection chain, then a synthetic [`Projection::Deref`],
    ///   then the original projections -- so `*dest.field` (projection
    ///   `[Deref, Field]` rooted at `dest`) becomes `*receiver ->
    ///   deref-bridge -> ... -> field` rooted at the receiver's base
    ///   local.
    ///
    /// Rewrites iterate through chained deref collapses: if the
    /// receiver itself is rooted at another collapsed destination
    /// (e.g., two consecutive deref calls each routed through its own
    /// temporary), resolution continues until a non-collapsed root is
    /// reached. The iteration is bounded by the number of recorded
    /// entries, so it always terminates.
    pub fn apply(&self, place: Place) -> Place {
        let mut current = place;
        let max_iters = self.entries.len() + 1;
        for _ in 0..max_iters {
            let Some(receiver) = self.entries.get(&current.local) else {
                return current;
            };
            let original_had_projections = !current.projection.is_empty();
            let mut new_projection: Vec<Projection> =
                Vec::with_capacity(receiver.projection.len() + 1 + current.projection.len());
            new_projection.extend(receiver.projection.iter().cloned());
            if original_had_projections {
                new_projection.push(Projection::Deref);
                new_projection.append(&mut current.projection);
            }
            current = Place {
                local: receiver.local,
                projection: new_projection,
            };
        }
        // If we exit the loop without returning, the rewrite table is
        // self-referential (a collapsed destination rewrites into a
        // place that still roots at another collapsed destination with
        // no base case). That is impossible by construction: receivers
        // captured at Call translation are rooted at MIR locals that
        // were not themselves collapse destinations yet. We return the
        // current value rather than looping forever.
        current
    }
}

// ---------------------------------------------------------------------------
// Receiver-Place extraction
// ---------------------------------------------------------------------------

/// Extract the `mir::Place` that backs the first argument of a
/// transport `Deref::deref` Call.
///
/// The `&self` receiver of `Deref::deref` is passed as an
/// [`mir::Operand::Copy`] or [`mir::Operand::Move`]. Any other operand
/// shape (e.g., a `&Mmio<T>` literal -- not possible in practice, but
/// defensively) returns `None` so the caller falls back to treating
/// the Call as a regular `Call`.
pub fn receiver_mir_place<'a, 'tcx>(arg: &'a mir::Operand<'tcx>) -> Option<&'a mir::Place<'tcx>> {
    match arg {
        mir::Operand::Copy(p) | mir::Operand::Move(p) => Some(p),
        mir::Operand::Constant(_) | mir::Operand::RuntimeChecks(_) => None,
    }
}

/// Look up the rustc type of an arbitrary [`mir::Operand`] against
/// the body's local decls and tcx context. Used at Call time to
/// classify the receiver.
pub fn operand_ty<'tcx>(
    op: &mir::Operand<'tcx>,
    locals: &LocalIdMap<'_, 'tcx>,
    tcx: TyCtxt<'tcx>,
) -> ty::Ty<'tcx> {
    op.ty(locals.local_decls(), tcx)
}

// ---------------------------------------------------------------------------
// Body-level rewriter
// ---------------------------------------------------------------------------

/// Apply Rule 4 rewrites to every Place reference in a list of basic
/// blocks.
///
/// Walks statements, statement rvalues, terminators, and operands --
/// every site that embeds a [`Place`]. Each Place is passed through
/// [`DerefCollapseRewrites::apply`]. A no-op if `rewrites.is_empty()`.
pub fn rewrite_body_places(basic_blocks: &mut [BasicBlock], rewrites: &DerefCollapseRewrites) {
    if rewrites.is_empty() {
        return;
    }
    for block in basic_blocks.iter_mut() {
        for stmt in block.statements.iter_mut() {
            rewrite_statement(stmt, rewrites);
        }
        rewrite_terminator(&mut block.terminator, rewrites);
    }
}

fn rewrite_statement(stmt: &mut Statement, rewrites: &DerefCollapseRewrites) {
    match &mut stmt.kind {
        StatementKind::Assign(place, rvalue) => {
            rewrite_place_in_place(place, rewrites);
            rewrite_rvalue(rvalue, rewrites);
        }
        StatementKind::SetDiscriminant { place, .. } => {
            rewrite_place_in_place(place, rewrites);
        }
        StatementKind::StorageLive(_) | StatementKind::StorageDead(_) => {
            // StorageLive / StorageDead reference a bare MIR local
            // with no projections -- but the collapsed destination's
            // lifetime markers are still emitted for the dropped
            // `&Block` temporary. The markers are harmless; dropping
            // them would require synthesizing terminator-local
            // bookkeeping. The rewrite leaves them pointing at the
            // original local, since there is no Place to rewrite.
        }
        StatementKind::MirIntrinsic(intr) => rewrite_mir_intrinsic(intr, rewrites),
    }
}

fn rewrite_mir_intrinsic(intr: &mut MirIntrinsic, rewrites: &DerefCollapseRewrites) {
    match intr {
        MirIntrinsic::Assume(op) => rewrite_operand(op, rewrites),
        MirIntrinsic::CopyNonOverlapping { src, dst, count } => {
            rewrite_operand(src, rewrites);
            rewrite_operand(dst, rewrites);
            rewrite_operand(count, rewrites);
        }
    }
}

fn rewrite_rvalue(rv: &mut Rvalue, rewrites: &DerefCollapseRewrites) {
    match rv {
        Rvalue::Use(op) => rewrite_operand(op, rewrites),
        Rvalue::Repeat(op, _) => rewrite_operand(op, rewrites),
        Rvalue::Ref(_, place) => rewrite_place_in_place(place, rewrites),
        Rvalue::Cast(_, op, _) => rewrite_operand(op, rewrites),
        Rvalue::BinaryOp(_, l, r) => {
            rewrite_operand(l, rewrites);
            rewrite_operand(r, rewrites);
        }
        Rvalue::UnaryOp(_, op) => rewrite_operand(op, rewrites),
        Rvalue::Discriminant(place) => rewrite_place_in_place(place, rewrites),
        Rvalue::Aggregate(_, ops) => {
            for op in ops.iter_mut() {
                rewrite_operand(op, rewrites);
            }
        }
    }
}

fn rewrite_operand(op: &mut Operand, rewrites: &DerefCollapseRewrites) {
    match op {
        Operand::Copy(p) | Operand::Move(p) => rewrite_place_in_place(p, rewrites),
        Operand::Constant(_) | Operand::Literal(_) => {}
    }
}

fn rewrite_terminator(term: &mut Terminator, rewrites: &DerefCollapseRewrites) {
    match &mut term.kind {
        TerminatorKind::SwitchInt { discriminant, .. } => rewrite_operand(discriminant, rewrites),
        TerminatorKind::Call {
            args, destination, ..
        } => {
            for a in args.iter_mut() {
                rewrite_operand(a, rewrites);
            }
            rewrite_place_in_place(destination, rewrites);
        }
        TerminatorKind::IntrinsicCall {
            args, destination, ..
        } => {
            for a in args.iter_mut() {
                rewrite_operand(a, rewrites);
            }
            rewrite_place_in_place(destination, rewrites);
        }
        TerminatorKind::Drop { place, .. } => rewrite_place_in_place(place, rewrites),
        TerminatorKind::Goto(_) | TerminatorKind::Return | TerminatorKind::Unreachable => {}
    }
}

fn rewrite_place_in_place(place: &mut Place, rewrites: &DerefCollapseRewrites) {
    let taken = std::mem::replace(
        place,
        Place {
            local: place.local,
            projection: Vec::new(),
        },
    );
    *place = rewrites.apply(taken);
}

// All tests in this module exercise post-HIR/MIR output (constructed model,
// JSON serialization, snapshots). Voided pending MonoRust + RegseqModel
// redesign. See docs/discussions/compiler-refactoring/test-audit.md.
#[cfg(all(test, feature = "post-monorust-tests"))]
mod tests {
    use super::*;
    use regseq_model::identity::{LocalId, Symbol, SymbolInterner};
    use regseq_model::ir::{RegseqType, ScalarType};

    fn make_place(local: u32, projection: Vec<Projection>) -> Place {
        Place {
            local: LocalId(local),
            projection,
        }
    }

    fn field(name: Option<Symbol>, index: u32, ty: RegseqType) -> Projection {
        Projection::Field { index, name, ty }
    }

    // -- Empty rewrites leave the place unchanged --

    #[test]
    fn empty_rewrites_is_noop() {
        let rewrites = DerefCollapseRewrites::default();
        let place = make_place(3, vec![Projection::Deref]);
        let out = rewrites.apply(place.clone());
        assert_eq!(out, place);
    }

    // -- Bare use of dest local replaced by receiver verbatim --

    #[test]
    fn bare_dest_use_becomes_receiver() {
        let mut rewrites = DerefCollapseRewrites::default();
        let receiver = make_place(
            5,
            vec![Projection::Field {
                index: 0,
                name: None,
                ty: RegseqType::Scalar(ScalarType::U32),
            }],
        );
        rewrites.record(LocalId(9), receiver.clone());

        // Bare use of the dest local
        let out = rewrites.apply(make_place(9, vec![]));
        assert_eq!(out, receiver);
    }

    // -- Projected use of dest local re-roots via receiver + Deref bridge --

    #[test]
    fn projected_dest_use_prepends_receiver_and_deref_bridge() {
        let mut interner = SymbolInterner::new();
        let field_name = interner.intern("status");

        let receiver = make_place(5, vec![]);
        let mut rewrites = DerefCollapseRewrites::default();
        rewrites.record(LocalId(9), receiver.clone());

        // `*dest.status` -> projection [Deref, Field(status)]
        let orig = make_place(
            9,
            vec![
                Projection::Deref,
                field(Some(field_name), 0, RegseqType::Scalar(ScalarType::U32)),
            ],
        );
        let out = rewrites.apply(orig);
        // Expected: receiver's local, projection [Deref /* bridge */, Deref, Field].
        assert_eq!(out.local, LocalId(5));
        assert_eq!(out.projection.len(), 3);
        assert!(matches!(out.projection[0], Projection::Deref));
        assert!(matches!(out.projection[1], Projection::Deref));
        assert!(matches!(out.projection[2], Projection::Field { .. }));
    }

    // -- Receiver with its own projection chain is prepended in full --

    #[test]
    fn projected_dest_preserves_receiver_projection_prefix() {
        let receiver = make_place(
            5,
            vec![
                Projection::Deref,
                Projection::Field {
                    index: 3,
                    name: None,
                    ty: RegseqType::Scalar(ScalarType::U32),
                },
            ],
        );
        let mut rewrites = DerefCollapseRewrites::default();
        rewrites.record(LocalId(9), receiver.clone());

        let orig = make_place(9, vec![Projection::Deref]);
        let out = rewrites.apply(orig);

        assert_eq!(out.local, LocalId(5));
        // receiver.projection (2 entries) + bridge Deref + orig's Deref
        assert_eq!(out.projection.len(), 4);
        assert!(matches!(out.projection[0], Projection::Deref));
        assert!(matches!(out.projection[1], Projection::Field { .. }));
        assert!(matches!(out.projection[2], Projection::Deref));
        assert!(matches!(out.projection[3], Projection::Deref));
    }

    // -- Place whose local is not in the rewrite table is untouched --

    #[test]
    fn unrelated_local_untouched() {
        let mut rewrites = DerefCollapseRewrites::default();
        rewrites.record(LocalId(9), make_place(5, vec![]));

        let orig = make_place(7, vec![Projection::Deref]);
        let out = rewrites.apply(orig.clone());
        assert_eq!(out, orig);
    }

    // -- Chained rewrites iterate through until stable --

    #[test]
    fn chained_rewrites_iterate() {
        let mut rewrites = DerefCollapseRewrites::default();
        // _9 rewrites to _5 (bare); _5 rewrites to _2 (bare).
        rewrites.record(LocalId(9), make_place(5, vec![]));
        rewrites.record(LocalId(5), make_place(2, vec![]));

        let out = rewrites.apply(make_place(9, vec![]));
        assert_eq!(out.local, LocalId(2));
        assert!(out.projection.is_empty());
    }

    // -- rewrite_body_places no-ops on empty rewrites --

    #[test]
    fn rewrite_body_places_noop_on_empty() {
        use regseq_model::identity::BasicBlockId;
        let mut blocks = vec![BasicBlock {
            statements: vec![Statement {
                kind: StatementKind::StorageLive(LocalId(7)),
            }],
            terminator: Terminator {
                kind: TerminatorKind::Goto(BasicBlockId(0)),
            },
        }];
        let rewrites = DerefCollapseRewrites::default();
        rewrite_body_places(&mut blocks, &rewrites);
        match &blocks[0].statements[0].kind {
            StatementKind::StorageLive(l) => assert_eq!(*l, LocalId(7)),
            _ => panic!("statement kind changed unexpectedly"),
        }
    }
}
