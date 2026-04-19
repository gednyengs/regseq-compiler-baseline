//! Post-Phase-2 verifier entry point.
//!
//! [`verify_phase_2`] runs against a model that has been mutated in
//! place by `populate_bindings` (task 59). It re-runs the subset of
//! Phase-1 invariants that still apply (module tree, cross-item
//! reference resolution, source-location coverage), then layers on:
//!
//! - Driver-struct-level rules (resource attribute completeness, no
//!   duplicate ids, user-data Option<T:Copy>) -- spec.md Section 13
//!   line 1426-1427.
//! - Body-level rules (referenced-id resolution, operand-type match,
//!   ReadOnlyRegisterWrite + ISR-context plumbing) -- spec.md Section
//!   13 line 1428-1430.
//! - Phase-2 boundary rules (required bindings populated, required
//!   property values populated) -- spec.md Section 13 line 1404-1408.
//! - Phase-2 cross-item rules (interrupt ISR binding targets resolve
//!   and are ISR-classified) -- spec.md Section 13 line 1432.
//!
//! Phase-1 leakage checks (Section 13 lines 1400-1401: "every binding
//! slot is `None`") deliberately do NOT re-run -- the whole point of
//! Phase 2 is to populate those slots.

use super::rules;
use super::VerifierError;
use crate::model::RegseqModel;

/// Verify that `model` satisfies the post-Phase-2 invariants.
///
/// Fail-fast: returns the first violation in deterministic order,
/// running rule buckets in the sequence prescribed by the task spec
/// (`.harness/tasks/regseq-model/56-post-phase2-verifier.md`):
///
/// 1. [`rules::module`] -- module-tree invariants and Phase-1
///    declaration-side completeness still apply post-join.
/// 2. [`rules::cross_item`] -- cross-item id resolution and
///    source-location coverage. This bucket walks both declaration
///    sites AND function bodies for `FunctionId`/`TypeDefId`/
///    `RegisterBlockId` references, so a body-level reference to an
///    undeclared one of those id kinds surfaces here as
///    `UnresolvedFunctionId` / `UnresolvedTypeDefId` /
///    `UnresolvedRegisterBlockId` -- not as `UnresolvedBodyReference`.
///    The body rule below remains responsible for `ConstantId`
///    references (which cross-item does not check) and for
///    body-specific checks (RO-write, ISR-context legality, operand
///    types).
/// 3. [`rules::driver_struct`] -- task 56 driver-struct rules.
/// 4. [`rules::body`] -- task 56 body-level rules.
/// 5. [`rules::phase_2_boundary`] -- required-binding/property
///    populated checks.
/// 6. [`rules::phase_2_cross_item`] -- ISR-target resolution.
///
/// The Phase-1 leakage rule (`UnexpectedBindingPopulated` /
/// `UnexpectedPropertyValuePopulated`) is bundled inside
/// [`rules::module::run`] but is gated by Phase 1; for Phase 2 we
/// skip it intentionally since populated bindings are now expected.
/// We achieve the skip by running only the module-tree subset --
/// see [`rules::module::run_phase_2_subset`].
pub fn verify_phase_2(model: &RegseqModel) -> Result<(), VerifierError> {
    rules::module::run_phase_2_subset(model)?;
    rules::cross_item::run(model)?;
    rules::driver_struct::run(model)?;
    rules::body::run(model)?;
    rules::phase_2_boundary::run(model)?;
    rules::phase_2_cross_item::run(model)?;
    Ok(())
}
