//! Non-fail-fast lint channel for the post-phase verifiers.
//!
//! [`verify_phase_1`] and [`verify_phase_2`] return a [`VerifyReport`]
//! carrying zero or more [`LintFinding`]s alongside the fail-fast
//! [`VerifierError`] error channel. Lints are reviewer-facing
//! diagnostics: they surface model conditions that merit attention but
//! do not violate phase-legality invariants, so emitting a lint does
//! **not** trigger rollback under [`MutationGuard::commit`].
//!
//! Rules live under [`rules`] (one rule per file). A rule walks the
//! model, appending [`LintFinding`]s to a growing vector. [`run`]
//! invokes every rule in module-order and then sorts the collected
//! findings by `(level, property-DeclId, kind-discriminant)` so two
//! runs on identical input produce byte-identical output.
//!
//! See [`rules::property_required_with_default`] for the single
//! property-side rule that currently exists. New non-property lint
//! kinds can extend [`LintKind`] without reshaping the verifier
//! surface.
//!
//! [`verify_phase_1`]: crate::verify::verify_phase_1
//! [`verify_phase_2`]: crate::verify::verify_phase_2
//! [`VerifierError`]: crate::verify::VerifierError
//! [`MutationGuard::commit`]: crate::model::MutationGuard::commit

use serde::Serialize;

use crate::identity::DeclId;

use super::VerifierProvenance;

pub mod rules;

/// A reviewer-facing finding produced by the phase-level verifier.
///
/// Lints are **not** fail-fast errors: a [`LintFinding`] flows out via
/// the `Ok(VerifyReport)` arm of [`verify_phase_1`] /
/// [`verify_phase_2`] and does **not** trigger rollback under
/// [`MutationGuard::commit`]. Callers that only care about structural
/// validity may drop the report; diagnostic surfaces render the
/// findings.
///
/// [`verify_phase_1`]: crate::verify::verify_phase_1
/// [`verify_phase_2`]: crate::verify::verify_phase_2
/// [`MutationGuard::commit`]: crate::model::MutationGuard::commit
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LintFinding {
    /// Reviewer attention level.
    pub level: LintLevel,
    /// Which rule produced this finding.
    pub kind: LintKind,
    /// Origin metadata for diagnostics. Mirrors the shape of
    /// [`VerifierError::provenance`] so consumers can render error-
    /// and lint-channel output uniformly.
    ///
    /// [`VerifierError::provenance`]: crate::verify::VerifierError::provenance
    pub provenance: VerifierProvenance,
    /// Human-readable description of the finding.
    pub details: String,
}

/// Attention level of a [`LintFinding`].
///
/// Ordered from lowest to highest severity so deterministic sorting
/// groups advisories before warnings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub enum LintLevel {
    /// Reviewer attention recommended but not required.
    Advisory,
    /// Reviewer attention strongly recommended; likely a driver bug.
    Warning,
}

/// The specific lint rule that fired.
///
/// Extension policy: append-only. New kinds land at the end of the
/// enum; existing kinds never change discriminant or payload shape --
/// downstream consumers (diagnostic renderers, integration tests,
/// serialized snapshots) treat each variant as a stable wire-level
/// identifier.
///
/// Additional non-property lint kinds may land here in future tasks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", content = "data")]
pub enum LintKind {
    /// A firmware property is marked required yet also declares a
    /// default literal. The default can never be consumed because
    /// required properties must be present in the platform file, so
    /// the default is reviewer-facing noise.
    ///
    PropertyRequiredWithDefault {
        /// The [`DeclId`] of the offending property field.
        property: DeclId,
    },
}

impl LintKind {
    /// Discriminant used as the tertiary sort key for
    /// [`VerifyReport`] determinism. Adding a variant here is the
    /// only change needed to keep new kinds deterministic.
    pub(crate) fn discriminant(&self) -> u16 {
        match self {
            LintKind::PropertyRequiredWithDefault { .. } => 0,
        }
    }

    /// Primary identity carried by the lint, used as the secondary
    /// sort key. Every current kind targets a single [`DeclId`];
    /// future kinds without a natural subject should still return a
    /// stable id (e.g., the containing module's id) to keep sort
    /// output byte-identical across runs.
    pub(crate) fn subject_decl_id(&self) -> DeclId {
        match self {
            LintKind::PropertyRequiredWithDefault { property } => *property,
        }
    }
}

/// Output of [`verify_phase_1`] / [`verify_phase_2`] on the success
/// path.
///
/// Carries the (possibly empty) list of non-fatal [`LintFinding`]s
/// detected during the phase-level walk. A structurally valid model
/// with no rule triggers returns `VerifyReport::default()`; a
/// structurally invalid model short-circuits with `Err(VerifierError)`
/// before a `VerifyReport` is ever constructed.
///
/// The shape is canonical: downstream consumers destructure
/// `.lints` directly. Additional fields may be added behind the
/// `#[serde(default)]` convention without requiring a new task-spec
/// cycle.
///
/// [`verify_phase_1`]: crate::verify::verify_phase_1
/// [`verify_phase_2`]: crate::verify::verify_phase_2
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct VerifyReport {
    /// Reviewer-facing findings. Empty if no lint rules triggered.
    ///
    /// Sorted by `(LintLevel, subject DeclId, LintKind discriminant)`
    /// so two runs on the same model produce byte-identical output.
    pub lints: Vec<LintFinding>,
}

impl VerifyReport {
    /// Construct a report with no lints.
    pub fn empty() -> Self {
        Self::default()
    }

    /// Sort `self.lints` into canonical order. Used by [`run`]
    /// immediately after rule execution; exposed for tests that
    /// build a report by hand.
    pub(crate) fn sort_findings(&mut self) {
        self.lints.sort_by(|a, b| {
            a.level
                .cmp(&b.level)
                .then_with(|| a.kind.subject_decl_id().cmp(&b.kind.subject_decl_id()))
                .then_with(|| a.kind.discriminant().cmp(&b.kind.discriminant()))
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::SourceOrigin;

    fn mk(level: LintLevel, prop: u32) -> LintFinding {
        LintFinding {
            level,
            kind: LintKind::PropertyRequiredWithDefault {
                property: DeclId::new(prop),
            },
            provenance: VerifierProvenance::for_item(
                format!("decl#{prop}"),
                SourceOrigin::CompilerGenerated,
            ),
            details: format!("decl#{prop}"),
        }
    }

    #[test]
    fn sort_findings_orders_by_level_then_decl_id() {
        let mut report = VerifyReport {
            lints: vec![
                mk(LintLevel::Warning, 7),
                mk(LintLevel::Advisory, 9),
                mk(LintLevel::Advisory, 3),
            ],
        };
        report.sort_findings();
        let ids: Vec<u32> = report
            .lints
            .iter()
            .map(|f| f.kind.subject_decl_id().index())
            .collect();
        assert_eq!(ids, vec![3, 9, 7]);
    }

    #[test]
    fn empty_report_is_default() {
        let report = VerifyReport::empty();
        assert!(report.lints.is_empty());
    }

    #[test]
    fn lint_finding_serializes_with_tagged_kind() {
        let f = mk(LintLevel::Advisory, 5);
        let json = serde_json::to_string(&f).unwrap();
        assert!(json.contains("PropertyRequiredWithDefault"));
        assert!(json.contains("\"level\""));
    }

    #[test]
    fn lint_level_ordering_places_advisory_before_warning() {
        assert!(LintLevel::Advisory < LintLevel::Warning);
    }
}
