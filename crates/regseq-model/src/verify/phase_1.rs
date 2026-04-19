//! Post-Phase-1 verifier entry point.
//!
//! [`verify_phase_1`] runs the module-level and cross-item rule buckets
//! against a model that just came out of Phase-1 normalization
//! (spec.md Section 13 lines 1398-1402). It is fail-fast: the first
//! violation is returned and no further rules are evaluated.
//!
//! Rules covered here:
//!
//! - Module-tree invariants: exactly one Driver module, no duplicate
//!   item names within any module (spec.md lines 1402, 1425).
//! - Declaration-side completeness: every [`Function`] has a
//!   non-placeholder body (spec.md line 1399).
//! - Phase-1 boundary leakage: every resource binding slot is `None`
//!   and every property `bound_value` is `None` (spec.md lines
//!   1400-1401).
//! - Cross-reference resolution: every [`TypeDefId`] / [`FunctionId`] /
//!   [`RegisterBlockId`] referenced by an item, body, or resource
//!   slot maps to a declared entity (spec.md lines 1431-1432).
//! - Source-location coverage: every assigned [`DeclId`] has an entry
//!   in [`RegseqModel::source_locations`] (spec.md line 1432).
//!
//! [`Function`]: crate::model::function::Function
//! [`TypeDefId`]: crate::identity::TypeDefId
//! [`FunctionId`]: crate::identity::FunctionId
//! [`RegisterBlockId`]: crate::identity::RegisterBlockId
//! [`DeclId`]: crate::identity::DeclId
//! [`RegseqModel::source_locations`]: crate::model::RegseqModel::source_locations

use super::rules;
use super::VerifierError;
use crate::model::RegseqModel;

/// Verify that `model` satisfies the post-Phase-1 invariants.
///
/// Fail-fast: returns the first violation in deterministic order
/// (module-level rules before cross-item rules; within a rule,
/// declarations are iterated in their canonical container order).
///
/// A batching `verify_phase_1_collect` variant can be added later if
/// downstream consumers need it; no speculation now.
pub fn verify_phase_1(model: &RegseqModel) -> Result<(), VerifierError> {
    rules::module::run(model)?;
    rules::cross_item::run(model)?;
    // Task 57 may route additional transformation-level checks here.
    Ok(())
}
