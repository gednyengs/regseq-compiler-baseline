//! Reviewer-facing driver-readiness diagnostics computed directly
//! from [`RegseqModel`] + the [`VerifyReport`] emitted by
//! [`regseq_model_old::verify::verify_phase_1`] (or
//! [`regseq_model_old::verify::verify_phase_2`]).
//!
//! Phase-legality checks (did every required binding populate? did
//! every Phase-1 field fill in? is the bus identity consistent?) live
//! in the Phase-6 verifiers. This module does **not** re-implement
//! those; it only surfaces reviewer-oriented observations: callback
//! inventories, resource-field summaries, and the property-lint
//! findings already produced by the verifier.
//!
//! The caller is responsible for invoking the verifier; a Phase-6
//! pipeline running under [`regseq_model_old::model::MutationGuard`] does
//! this automatically on `commit`, so typical consumers simply pass
//! the resulting [`VerifyReport`] to [`check_readiness`].

use std::collections::BTreeSet;

use regseq_model_old::identity::{FunctionId, TypeDefId};
use regseq_model_old::model::{CallbackFamily, RegseqModel};
use regseq_model_old::query;
use regseq_model_old::verify::{LintFinding, LintKind, VerifyReport};

pub use regseq_model_old::query::ResourceKind;

/// Lightweight, owned projection of a [`query::CallbackImpl`].
///
/// The query helper returns a borrowed view tied to the
/// [`RegseqModel`]; the readiness report is a standalone value that
/// reviewers may retain after dropping the model, so we copy out the
/// ids. No symbol names are carried -- reviewers looking for method
/// names should resolve `function_id` against the model's function
/// table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CallbackImplSummary {
    /// Callback family the impl's trait belongs to.
    pub family: CallbackFamily,
    /// The method's function id within the model.
    pub function_id: FunctionId,
    /// The driver type the impl targets.
    pub driver_type: TypeDefId,
}

/// Reviewer-facing readiness report for a driver model.
///
/// Carries the inventory-style observations a human reviewer wants
/// when inspecting a compiled driver: which callback families are
/// implemented, which resource fields exist, and which property lints
/// the verifier flagged. The caller is expected to have already
/// invoked the phase verifier; the [`property_lints`] field is a
/// filtered copy of that verifier's [`VerifyReport::lints`].
///
/// Every field is deterministic: order derives from the
/// [`regseq_model_old::query`] helpers (sorted by canonical key) or from
/// [`LintFinding`] sort order emitted by the verifier.
///
/// [`property_lints`]: Self::property_lints
#[derive(Debug, Clone)]
pub struct ReadinessReport {
    /// Callback families actually implemented by the driver.
    pub callback_families_present: BTreeSet<CallbackFamily>,
    /// Every callback method impl on the driver, one entry per
    /// method.
    pub callback_impls: Vec<CallbackImplSummary>,
    /// Every resource field on the driver struct, paired with its
    /// family tag.
    pub resource_field_names: Vec<(ResourceKind, String)>,
    /// Property-lint findings surfaced by the verifier.
    ///
    /// Today this carries only [`LintKind::PropertyRequiredWithDefault`]
    /// entries (the only lint kind the Phase-1 verifier emits). As
    /// additional property-level [`LintKind`] variants land, they
    /// appear here automatically.
    pub property_lints: Vec<LintFinding>,
}

/// Build a [`ReadinessReport`] for `model`, incorporating the
/// verifier's lint findings from `verify_report`.
///
/// Pure: no mutation, no global state, no file I/O. Executes in time
/// proportional to the model's module-tree size plus the verify
/// report's lint count.
pub fn check_readiness(model: &RegseqModel, verify_report: &VerifyReport) -> ReadinessReport {
    let callback_families_present = query::callback_families_present(model);

    let callback_impls = query::callback_impls(model)
        .into_iter()
        .map(|ci| CallbackImplSummary {
            family: ci.family,
            function_id: ci.function_id,
            driver_type: ci.driver_type,
        })
        .collect();

    let resource_field_names = query::resource_field_names(model)
        .into_iter()
        .map(|(kind, name)| (kind, name.to_string()))
        .collect();

    let property_lints = verify_report
        .lints
        .iter()
        .filter(|l| is_property_lint(&l.kind))
        .cloned()
        .collect();

    ReadinessReport {
        callback_families_present,
        callback_impls,
        resource_field_names,
        property_lints,
    }
}

/// Classify a [`LintKind`] as property-scoped or not.
///
/// Every current `LintKind` variant is property-scoped; the matcher
/// is spelled out rather than defaulted so that any future non-
/// property lint kind (e.g., callback- or body-level) has to be
/// explicitly triaged here before it silently flows into
/// [`ReadinessReport::property_lints`].
fn is_property_lint(kind: &LintKind) -> bool {
    match kind {
        LintKind::PropertyRequiredWithDefault { .. } => true,
    }
}
