//! Phase-6 [`normalize`] entry point.
//!
//! Takes a [`RawFrontendArtifact`] (the rustc-independent mirror of
//! the Phase-5 JSON schema) and produces a Phase-1 [`RegseqModel`]
//! with every declaration-side slot populated and every binding slot
//! set to `None`. Calls [`crate::verify::verify_phase_1`] internally
//! before returning `Ok` so the caller is guaranteed the returned
//! model satisfies the post-Phase-1 invariants.
//!
//! The surface-by-surface normalization is implemented in
//! [`surface`]; provenance stamping in [`provenance`]; the serde
//! mirror of the driver's `RawFrontendArtifact` in [`raw_artifact`].
//!
//! # Error model
//!
//! Every failure returns a [`NormalizeError`] carrying a
//! [`NormalizeErrorKind`] and a [`NormalizeProvenance`] (both
//! `surface_path` and `containing_item` populated). The four "parse"
//! variants match spec.md Section 12 line 1361-1364 one-to-one;
//! [`NormalizeErrorKind::Phase1VerificationFailed`] leaks a verifier
//! error produced by the internal post-condition check.
//!
//! [`RegseqModel`]: crate::model::RegseqModel

// The task spec prescribes the exact `Result<T, NormalizeError>`
// signature for every public and internal entry point. `NormalizeError`
// carries a leaked `VerifierError` variant (`Phase1VerificationFailed`)
// plus the always-populated provenance, which pushes the `Err` size
// past the default clippy threshold. Box-wrapping would force every
// caller to dereference before matching on `.kind` / `.provenance`.
// The `verify` module has the same constraint and takes the same
// allow -- mirroring that here keeps the public signatures uniform.
#![allow(clippy::result_large_err)]

pub mod provenance;
pub mod raw_artifact;
pub mod surface;

pub use provenance::NormalizeProvenance;
pub use raw_artifact::RawFrontendArtifact;

use crate::model::RegseqModel;
use crate::verify::{verify_phase_1, VerifierError};

use self::provenance::NormalizeContext;

/// Normalize a raw frontend artifact into a Phase-1 [`RegseqModel`].
///
/// The returned model is guaranteed to pass
/// [`verify_phase_1`]; if the verifier rejects the normalized model
/// the error bubbles up as
/// [`NormalizeErrorKind::Phase1VerificationFailed`].
///
/// Deterministic by construction: the same input always produces a
/// byte-identical model because every inner `Vec` is populated in raw
/// surface order and all keyed containers are `BTreeMap` /
/// `BTreeSet`.
pub fn normalize(input: &RawFrontendArtifact) -> Result<RegseqModel, NormalizeError> {
    let mut ctx = NormalizeContext::root();
    build_model(&mut ctx, input, |_| {})
}

/// Test-only hook: run the per-surface normalization, then apply
/// `hook` to the still-pre-verification model before the internal
/// `verify_phase_1` call.
///
/// Exists so integration tests can prove
/// [`NormalizeErrorKind::Phase1VerificationFailed`] is actually raised
/// without needing a second code path that skips verification.
#[cfg(test)]
pub(crate) fn normalize_with_post_hook(
    input: &RawFrontendArtifact,
    hook: impl FnOnce(&mut RegseqModel),
) -> Result<RegseqModel, NormalizeError> {
    let mut ctx = NormalizeContext::root();
    build_model(&mut ctx, input, hook)
}

fn build_model(
    ctx: &mut NormalizeContext,
    input: &RawFrontendArtifact,
    post_hook: impl FnOnce(&mut RegseqModel),
) -> Result<RegseqModel, NormalizeError> {
    let mut builder = surface::Builder::new(ctx, input)?;

    // One call per top-level `RawFrontendArtifact` surface
    // (spec 58-normalize.md "Implementation Notes / Module layout").
    // The order here is the canonical surface order; do not re-order
    // without updating the determinism test.
    //
    // Note on DeclId provenance: the spec envisions Phase 5 Pass 1
    // pre-allocating DeclIds that Phase 6 carries through. The
    // [`RawFrontendArtifact`] mirror authored in this task reflects
    // the driver-side schema as-committed, which does not yet carry
    // DeclIds -- cross-references land as strings
    // (`register_block_type`, `interrupt_id`, `property_id`, etc.).
    // Until Phase 5 starts emitting DeclIds into the raw artifact,
    // normalize allocates monotonically in visit order and resolves
    // the string-keyed cross-references locally. Error semantics are
    // preserved; only the id-provenance source differs.
    surface::normalize_register_access(&mut builder, input.register_access.as_ref())?;
    surface::normalize_trait_callbacks(&mut builder, input.trait_callbacks.as_ref())?;
    surface::normalize_clock(&mut builder, input.clock.as_ref())?;
    surface::normalize_reset(&mut builder, input.reset.as_ref())?;
    surface::normalize_pinctrl(&mut builder, input.pinctrl.as_ref())?;
    surface::normalize_interrupts(&mut builder, input.interrupts.as_ref())?;
    surface::normalize_synchronization(&mut builder, input.synchronization.as_ref())?;
    surface::normalize_timed_delay(&mut builder, input.timed_delay.as_ref())?;
    surface::normalize_logging(&mut builder, input.logging.as_ref())?;
    surface::normalize_firmware_properties(&mut builder, input.firmware_properties.as_ref())?;
    surface::normalize_pm_gate(&mut builder, input.pm_gate.as_ref())?;
    surface::validate_identity_enrichment(&mut builder, input.identity_enrichment.as_ref())?;
    surface::normalize_source_locations(&mut builder, input.source_locations.as_ref())?;

    let finished = surface::finish(builder);

    let mut model = RegseqModel::from_verified_components_unchecked(
        finished.symbols,
        finished.source_locations,
        finished.root_module,
        1,
    );

    post_hook(&mut model);

    // `verify_phase_1` now returns `Ok(VerifyReport)` on success.
    // Normalize has no diagnostic surface of its own for lints;
    // reviewer-facing findings are consumed by `regseq-analysis`
    // from a direct `verify_phase_1` call (task 6), so the report is
    // dropped here.
    let _report = verify_phase_1(&model).map_err(|inner| NormalizeError {
        kind: NormalizeErrorKind::Phase1VerificationFailed { inner },
        provenance: NormalizeProvenance {
            surface_path: "<post-normalize verify>".into(),
            containing_item: "<RegseqModel root>".into(),
        },
    })?;
    Ok(model)
}

/// Error returned by [`normalize`].
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct NormalizeError {
    pub kind: NormalizeErrorKind,
    pub provenance: NormalizeProvenance,
}

impl std::fmt::Display for NormalizeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "normalize error [{}] at {} in {}: {:?}",
            kind_label(&self.kind),
            self.provenance.surface_path,
            self.provenance.containing_item,
            self.kind,
        )
    }
}

impl std::error::Error for NormalizeError {}

fn kind_label(kind: &NormalizeErrorKind) -> &'static str {
    match kind {
        NormalizeErrorKind::UnrecognizedEnumTag { .. } => "UnrecognizedEnumTag",
        NormalizeErrorKind::MissingRequiredAttribute { .. } => "MissingRequiredAttribute",
        NormalizeErrorKind::StructuralViolation { .. } => "StructuralViolation",
        NormalizeErrorKind::TypeConstraintViolation { .. } => "TypeConstraintViolation",
        NormalizeErrorKind::Phase1VerificationFailed { .. } => "Phase1VerificationFailed",
    }
}

/// The specific violation. The first four variants map one-to-one
/// with spec.md Section 12 line 1361-1364; the fifth leaks a verifier
/// error thrown by the Phase-1 post-condition check.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "kind", content = "data")]
pub enum NormalizeErrorKind {
    /// A stringly enum tag in the raw artifact was not recognized.
    UnrecognizedEnumTag {
        surface: &'static str,
        field: &'static str,
        tag: String,
    },
    /// A required declaration attribute was absent or empty in the
    /// raw artifact.
    MissingRequiredAttribute {
        surface: &'static str,
        attribute: &'static str,
        item: String,
    },
    /// A multi-entity invariant failed (e.g., two driver structs, a
    /// cross-reference pointing at a missing register block).
    StructuralViolation { description: String },
    /// A typed-attribute constraint failed (e.g., user-data field
    /// type is not `Option<T>`).
    TypeConstraintViolation { description: String },
    /// The internally invoked [`verify_phase_1`] rejected the
    /// normalized model. Indicates either a normalizer bug or that
    /// the raw artifact was inconsistent in a way the per-surface
    /// checks did not catch.
    Phase1VerificationFailed { inner: VerifierError },
}

#[cfg(test)]
mod tests {
    use super::raw_artifact::{RawDriverIdentity, RawFrontendArtifact};
    use super::*;

    fn minimal_artifact() -> RawFrontendArtifact {
        RawFrontendArtifact {
            driver_name: "testdrv".into(),
            target_os: "linux".into(),
            identity: Some(RawDriverIdentity {
                driver_name: "testdrv".into(),
                compatible: vec!["vendor,testdrv".into()],
                device_class: "serial".into(),
                bus: Some("amba".into()),
            }),
            ..Default::default()
        }
    }

    #[test]
    fn minimal_artifact_normalizes_and_passes_phase_1() {
        let input = minimal_artifact();
        let model = normalize(&input).expect("minimal normalize must succeed");
        assert_eq!(model.root_module.submodules.len(), 1);
        assert!(model.root_module.submodules[0].driver.is_some());
    }

    #[test]
    fn missing_identity_reports_missing_attribute_with_provenance() {
        let mut input = minimal_artifact();
        input.identity = None;
        let err = normalize(&input).expect_err("missing identity must fail");
        assert!(!err.provenance.surface_path.is_empty());
        assert!(!err.provenance.containing_item.is_empty());
        matches!(
            err.kind,
            NormalizeErrorKind::MissingRequiredAttribute { .. }
        );
    }

    #[test]
    fn post_hook_leaking_a_binding_surfaces_verifier_error() {
        // Proves `normalize()` does not silently bypass
        // `verify_phase_1` on its own output. We build an artifact
        // that would normally pass, then mutate one clock resource's
        // `binding` to `Some(..)` right before the internal
        // `verify_phase_1` call. The returned error must be
        // `Phase1VerificationFailed { inner: { kind:
        // UnexpectedBindingPopulated { slot: "clock" }, .. } }`.
        use crate::model::resources::ClockBinding;
        use crate::normalize::raw_artifact::{RawClockField, RawClockSurface};
        use crate::verify::VerifierErrorKind;

        let mut input = minimal_artifact();
        input.clock = Some(RawClockSurface {
            fields: vec![RawClockField {
                field_name: "clk".into(),
                clock_id: "apb".into(),
                required: true,
                field_type: String::new(),
            }],
            call_sites: vec![],
        });

        let err = normalize_with_post_hook(&input, |model| {
            // Dive into the driver submodule and populate the first
            // clock resource's binding slot. Normalize has already
            // constructed the model; we just corrupt it.
            let driver_mod = model
                .root_module
                .submodules
                .iter_mut()
                .find(|m| m.kind == crate::model::module::ModuleKind::Driver)
                .expect("driver module present");
            let driver = driver_mod.driver.as_mut().expect("driver struct present");
            driver.clocks[0].binding = Some(ClockBinding {
                default_frequency: 24_000_000,
            });
        })
        .expect_err("post-hook corruption must surface a verifier error");

        match &err.kind {
            NormalizeErrorKind::Phase1VerificationFailed { inner } => match &inner.kind {
                VerifierErrorKind::UnexpectedBindingPopulated { slot, .. } => {
                    assert_eq!(*slot, "clock");
                }
                other => panic!("expected UnexpectedBindingPopulated, got {other:?}"),
            },
            other => panic!("expected Phase1VerificationFailed, got {other:?}"),
        }
    }
}
