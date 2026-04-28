//! Rule 5 `ordered!` region recognition for Pass 2.
//!
//! Implemented by task regseq-model-49.
//!
//! At Pass 2 setup, [`OrderedMarkers::resolve`] walks the facade crate
//! (`regseq`) for the free functions
//! [`regseq::internal::ordered_region_begin`] and
//! [`regseq::internal::ordered_region_end`] and caches their
//! [`DefId`]s. The lookup is keyed by walking the crate's module
//! children through a live `TyCtxt` query -- no hard-coded path
//! strings, no heuristics: if the facade crate is loaded but the
//! markers cannot be resolved, setup fails with
//! [`ConstructionError::OrderedMarkerLookupFailed`] so facade renames
//! do not silently disable region detection.
//!
//! During terminator translation, a `Call` whose callee `DefId` matches
//! one of the cached markers is dropped: the marker's basic block is
//! emitted with a `Goto(target)` terminator (preserving preceding
//! statements) and a side entry `(block_id, kind, target_id)` is
//! recorded. After every block is translated,
//! [`pair_ordered_regions`] folds the side entries into
//! `OrderedRegion { entry, exit }` records via a LIFO stack (supporting
//! nesting) and asserts that every `begin` has a matching `end` and
//! vice versa -- unpaired markers raise
//! [`ConstructionError::UnpairedOrderedMarker`].
//!
//! The resulting regions live on `Body.ordered_regions` (per
//! `ir-reference.md` Section 6). The SESE invariant is expressed as
//! reachability from `entry` without passing through `exit`; it is
//! satisfied structurally because `ordered!` is a source-level macro
//! whose expansion brackets a single block with a single entry and
//! single exit, and the translation preserves that bracketing.

use rustc_hir::def::DefKind;
use rustc_hir::def_id::{CrateNum, DefId, CRATE_DEF_INDEX};
use rustc_middle::ty::TyCtxt;

use regseq_model_old::identity::BasicBlockId;
use regseq_model_old::ir::OrderedRegion;

use super::{ConstructionError, ErrorProvenance};

/// Which end of an ordered region a recognized marker call belongs to.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum OrderedMarkerKind {
    /// A `regseq::internal::ordered_region_begin()` call.
    Begin,
    /// A `regseq::internal::ordered_region_end()` call.
    End,
}

/// An `(owning-block, marker-kind, call-target)` triple captured at
/// the call site when Pass 2 drops a marker `Call`.
///
/// `owning_block` is the translated `BasicBlockId` that held the
/// marker `Call` terminator (now a `Goto(target)`); `target` is the
/// block the marker's original return edge pointed at. For a `Begin`
/// marker `target` is the region's `entry`; for an `End` marker
/// `target` is the region's `exit`.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct OrderedMarkerSite {
    pub owning_block: BasicBlockId,
    pub kind: OrderedMarkerKind,
    pub target: BasicBlockId,
}

/// Cached [`DefId`]s of the two `regseq::internal::*` ordered-region
/// marker functions.
///
/// Resolved at Pass 2 setup via a live `TyCtxt` walk of the facade
/// crate's items (see [`Self::resolve`]). The markers are compiler
/// markers, not `#[rseq::intrinsic]` intrinsics, so they do not appear
/// in [`super::intrinsics::IntrinsicLookup`]; recognition happens
/// purely by `DefId` equality against the cached values.
#[derive(Debug, Default, Copy, Clone, Eq, PartialEq)]
pub struct OrderedMarkers {
    begin: Option<DefId>,
    end: Option<DefId>,
}

impl OrderedMarkers {
    /// Resolve both marker [`DefId`]s by walking the `regseq` facade
    /// crate's `internal` module.
    ///
    /// If the facade crate is not a dependency of the crate being
    /// compiled, both fields are `None` and [`Self::resolve`] returns
    /// successfully -- specimens that do not pull in `regseq` have no
    /// `ordered!` markers to detect, and a missing facade is not an
    /// error (for example, minimal inline test specimens).
    ///
    /// If the facade crate is loaded but either marker function cannot
    /// be resolved, [`ConstructionError::OrderedMarkerLookupFailed`]
    /// is returned. This defends against facade renames masquerading
    /// as silent "no regions present" outcomes.
    pub fn resolve(tcx: TyCtxt<'_>) -> Result<Self, ConstructionError> {
        let Some(regseq_crate) = find_crate_by_name(tcx, "regseq") else {
            return Ok(Self::default());
        };

        let root = DefId {
            krate: regseq_crate,
            index: CRATE_DEF_INDEX,
        };
        let internal = resolve_child_module(tcx, root, "internal").ok_or_else(|| {
            ConstructionError::OrderedMarkerLookupFailed {
                provenance: ErrorProvenance::for_def_id(tcx, root),
                rendered: "regseq::internal".to_string(),
            }
        })?;

        let begin = resolve_child_function(tcx, internal, "ordered_region_begin");
        let end = resolve_child_function(tcx, internal, "ordered_region_end");

        match (begin, end) {
            (Some(b), Some(e)) => Ok(Self {
                begin: Some(b),
                end: Some(e),
            }),
            _ => Err(ConstructionError::OrderedMarkerLookupFailed {
                provenance: ErrorProvenance::for_def_id(tcx, internal),
                rendered: format!(
                    "regseq::internal::ordered_region_begin={}, ordered_region_end={}",
                    begin.is_some(),
                    end.is_some(),
                ),
            }),
        }
    }

    /// Construct an [`OrderedMarkers`] from a pre-resolved pair of
    /// marker [`DefId`]s.
    ///
    /// Used by integration tests that define synthetic marker
    /// functions without depending on the `regseq` facade crate --
    /// the test harness picks the two `DefId`s through normal HIR
    /// lookup (`find_free_fn_def_id`) and wires them here. The
    /// production setup path is [`Self::resolve`].
    pub fn from_def_ids(begin: DefId, end: DefId) -> Self {
        Self {
            begin: Some(begin),
            end: Some(end),
        }
    }

    /// Classify a callee `DefId` as one of the two markers, or `None`
    /// if the callee is not a marker.
    pub fn classify(&self, def_id: DefId) -> Option<OrderedMarkerKind> {
        if Some(def_id) == self.begin {
            Some(OrderedMarkerKind::Begin)
        } else if Some(def_id) == self.end {
            Some(OrderedMarkerKind::End)
        } else {
            None
        }
    }

    /// Whether both markers were resolved. `false` when the facade
    /// crate is not a dependency (both [`DefId`]s are `None`).
    pub fn is_resolved(&self) -> bool {
        self.begin.is_some() && self.end.is_some()
    }
}

/// Fold a Pass-2-ordered sequence of marker sites into
/// [`OrderedRegion`] records via a LIFO stack.
///
/// Nesting is supported: each `Begin` pushes onto the stack, each
/// `End` pops the most recently pushed `Begin` and emits one
/// [`OrderedRegion`] whose `entry` is the `Begin`'s target and whose
/// `exit` is the `End`'s target. Strict containment is a consequence
/// of the stack discipline: an inner region's `Begin` is always
/// popped before its outer region's `Begin`, so the inner region's
/// block range is contained within the outer's.
///
/// An `End` with an empty stack, or a `Begin` with no matching `End`
/// after the last site, raises
/// [`ConstructionError::UnpairedOrderedMarker`].
pub(crate) fn pair_ordered_regions<'tcx>(
    tcx: TyCtxt<'tcx>,
    owner: DefId,
    sites: &[OrderedMarkerSite],
) -> Result<Vec<OrderedRegion>, ConstructionError> {
    let mut stack: Vec<OrderedMarkerSite> = Vec::new();
    let mut regions: Vec<OrderedRegion> = Vec::new();

    for site in sites {
        match site.kind {
            OrderedMarkerKind::Begin => stack.push(*site),
            OrderedMarkerKind::End => {
                let begin =
                    stack
                        .pop()
                        .ok_or_else(|| ConstructionError::UnpairedOrderedMarker {
                            provenance: ErrorProvenance::for_def_id(tcx, owner),
                            rendered: format!(
                                "ordered_region_end in bb{} has no matching begin",
                                site.owning_block.0,
                            ),
                        })?;
                regions.push(OrderedRegion {
                    entry: begin.target,
                    exit: site.target,
                });
            }
        }
    }

    if let Some(leftover) = stack.pop() {
        return Err(ConstructionError::UnpairedOrderedMarker {
            provenance: ErrorProvenance::for_def_id(tcx, owner),
            rendered: format!(
                "ordered_region_begin in bb{} has no matching end",
                leftover.owning_block.0,
            ),
        });
    }

    Ok(regions)
}

// ---------------------------------------------------------------------------
// Facade crate walking helpers
// ---------------------------------------------------------------------------

fn find_crate_by_name(tcx: TyCtxt<'_>, name: &str) -> Option<CrateNum> {
    tcx.crates(())
        .iter()
        .copied()
        .find(|&krate| tcx.crate_name(krate).as_str() == name)
}

fn resolve_child_module(tcx: TyCtxt<'_>, parent: DefId, name: &str) -> Option<DefId> {
    for child in tcx.module_children(parent) {
        let Some(def_id) = child.res.opt_def_id() else {
            continue;
        };
        if !matches!(tcx.def_kind(def_id), DefKind::Mod) {
            continue;
        }
        if child.ident.name.as_str() == name {
            return Some(def_id);
        }
    }
    None
}

fn resolve_child_function(tcx: TyCtxt<'_>, parent: DefId, name: &str) -> Option<DefId> {
    for child in tcx.module_children(parent) {
        let Some(def_id) = child.res.opt_def_id() else {
            continue;
        };
        if !matches!(tcx.def_kind(def_id), DefKind::Fn) {
            continue;
        }
        if child.ident.name.as_str() == name {
            return Some(def_id);
        }
    }
    None
}

// All tests in this module exercise post-HIR/MIR output (constructed model,
// JSON serialization, snapshots). Voided pending MonoRust + RegseqModel
// redesign. See docs/discussions/compiler-refactoring/test-audit.md.
#[cfg(all(test, feature = "post-monorust-tests"))]
mod tests {
    use super::*;

    #[test]
    fn default_markers_are_unresolved() {
        let markers = OrderedMarkers::default();
        assert!(!markers.is_resolved());
    }
}
