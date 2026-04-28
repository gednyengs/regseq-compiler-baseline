//! Lint rule bucket.
//!
//! One rule per file. [`run`] invokes every rule against the live
//! [`RegseqModel`] and returns the aggregated [`VerifyReport`]. The
//! returned report is deterministically sorted; rule authors may
//! append findings in any order.
//!
//! [`RegseqModel`]: crate::model::RegseqModel
//! [`VerifyReport`]: super::VerifyReport

use crate::model::regseq_model::RegseqModel;

use super::VerifyReport;

pub mod property_required_with_default;

/// Execute every lint rule against `model`, returning the aggregated
/// [`VerifyReport`] in deterministic order.
///
/// Rule execution order is irrelevant: the aggregated findings are
/// re-sorted by [`VerifyReport::sort_findings`] before the report is
/// returned, so adding a rule never changes the byte-level output of
/// an unrelated rule.
pub fn run(model: &RegseqModel) -> VerifyReport {
    let mut report = VerifyReport::default();
    property_required_with_default::check(model, &mut report.lints);
    report.sort_findings();
    report
}
