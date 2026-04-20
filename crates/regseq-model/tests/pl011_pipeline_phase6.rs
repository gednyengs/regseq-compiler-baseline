//! Phase-6 end-to-end pipeline test for the PL011 fixture.
//!
//! Exercises `RawFrontendArtifact -> normalize -> verify_phase_1 ->
//! populate_bindings -> verify_phase_2` and freezes the resulting
//! serialized [`RegseqModel`] as the Phase-6 golden. See task spec
//! `regseq-model-60`.
//!
//! This test is **configuration-only**: the pipeline logic lives in
//! tasks 55-59; this file is the final gate that locks their joint
//! behavior via a committed JSON golden.

use std::path::PathBuf;

use regseq_model::model::{ModuleKind, RegseqModel};
use regseq_model::normalize::raw_artifact::RawFrontendArtifact;
use regseq_model::platform::{ConstructSection, PlatformFile};
use regseq_model::platform_parser::parse_platform_file_path;
use regseq_model::serialize::serialize_regseq_model;
use regseq_model::verify::{verify_phase_1, verify_phase_2, VerifierErrorKind};
use regseq_model::{normalize, populate_bindings, JoinErrorKind, NormalizeErrorKind};

// ---------------------------------------------------------------------
// Fixture loading
// ---------------------------------------------------------------------

fn fixture_root() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("../../tests/fixtures/pl011-full-spec");
    p
}

fn raw_artifact_path() -> PathBuf {
    fixture_root().join("expected/raw-frontend.json")
}

fn platform_path() -> PathBuf {
    fixture_root().join("platform.toml")
}

fn golden_path() -> PathBuf {
    fixture_root().join("expected/regseq-model.json")
}

fn load_raw_artifact() -> RawFrontendArtifact {
    let raw = std::fs::read_to_string(raw_artifact_path()).expect("read raw-frontend.json fixture");
    serde_json::from_str(&raw).expect("deserialize RawFrontendArtifact")
}

fn load_platform() -> PlatformFile {
    parse_platform_file_path(&platform_path()).expect("parse platform.toml fixture")
}

/// Run the full Phase-6 pipeline against the PL011 fixture.
///
/// Every intermediate verifier call is asserted so a regression in any
/// one phase fails the test at that phase rather than at the golden
/// byte-comparison.
fn run_pipeline() -> RegseqModel {
    let raw = load_raw_artifact();
    let platform = load_platform();

    let mut model = normalize(&raw).expect("normalize must succeed on PL011");
    verify_phase_1(&model).expect("PL011 normalize must satisfy Phase-1 invariants");

    populate_bindings(&mut model, &platform)
        .expect("populate_bindings must succeed on PL011 platform");
    verify_phase_2(&model).expect("PL011 bound model must satisfy Phase-2 invariants");

    model
}

// ---------------------------------------------------------------------
// Happy path + golden byte-equality
// ---------------------------------------------------------------------

#[test]
fn pl011_phase6_pipeline_end_to_end() {
    let model = run_pipeline();

    // Sanity-check cross-cutting facts that would be easy to silently
    // regress if the golden itself drifted.
    let driver = model
        .root_module
        .submodules
        .iter()
        .find(|m| m.kind == ModuleKind::Driver)
        .and_then(|m| m.driver.as_ref())
        .expect("driver submodule must be present");

    for c in &driver.clocks {
        assert!(c.binding.is_some(), "every clock binding must be populated");
    }
    for r in &driver.resets {
        assert!(r.binding.is_some(), "every reset binding must be populated");
    }
    for p in &driver.pinctrl {
        assert!(
            p.binding.is_some(),
            "every pinctrl binding must be populated"
        );
    }
    for i in &driver.interrupts {
        assert!(
            i.binding.is_some(),
            "every interrupt binding must be populated"
        );
    }
    for m in &driver.mmio {
        assert!(m.binding.is_some(), "every mmio binding must be populated");
    }
    for p in &driver.properties {
        assert!(
            p.bound_value.is_some(),
            "every required property value must be populated: {}",
            model.symbols.resolve(p.property_id),
        );
    }
    assert!(
        driver.platform_metadata.is_some(),
        "platform metadata must be recorded after Phase 2",
    );

    let serialized = serialize_regseq_model(&model);

    let golden_path = golden_path();
    if std::env::var_os("REGEN_GOLDEN").is_some() {
        std::fs::write(&golden_path, &serialized).expect("write Phase-6 golden");
        return;
    }
    let expected = std::fs::read_to_string(&golden_path).unwrap_or_else(|e| {
        panic!(
            "expected Phase-6 golden at {} (error: {e}). \
             Regenerate with `REGEN_GOLDEN=1 cargo test -p regseq-model \
             --test pl011_pipeline_phase6 pl011_phase6_pipeline_end_to_end`.",
            golden_path.display(),
        )
    });
    assert!(
        serialized.as_bytes() == expected.as_bytes(),
        "Phase-6 golden drift: serialized RegseqModel does not match \
         {} byte-for-byte. Re-run with REGEN_GOLDEN=1 to regenerate if \
         the drift is intentional.",
        golden_path.display(),
    );
}

// ---------------------------------------------------------------------
// Determinism: two runs produce byte-identical serialization
// ---------------------------------------------------------------------

#[test]
fn pl011_phase6_pipeline_is_deterministic() {
    let out_a = serialize_regseq_model(&run_pipeline());
    let out_b = serialize_regseq_model(&run_pipeline());
    assert!(
        out_a.as_bytes() == out_b.as_bytes(),
        "Phase-6 pipeline must be deterministic across runs",
    );
}

// ---------------------------------------------------------------------
// Negative -- malformed raw artifact -> NormalizeError
// ---------------------------------------------------------------------

#[test]
fn malformed_raw_artifact_returns_normalize_error() {
    // Drop the `identity` surface -- normalize() raises
    // MissingRequiredAttribute on it before any structural work.
    let mut raw = load_raw_artifact();
    raw.identity = None;

    let err = normalize(&raw).expect_err("missing identity must fail to normalize");
    match err.kind {
        NormalizeErrorKind::MissingRequiredAttribute { .. } => {}
        other => panic!("expected MissingRequiredAttribute, got {other:?}"),
    }
    assert!(
        !err.provenance.surface_path.is_empty(),
        "NormalizeError provenance.surface_path must be populated",
    );
    assert!(
        !err.provenance.containing_item.is_empty(),
        "NormalizeError provenance.containing_item must be populated",
    );
}

// ---------------------------------------------------------------------
// Negative -- platform file with unknown section id
// ---------------------------------------------------------------------

#[test]
fn unknown_platform_section_returns_unknown_platform_section() {
    let mut model = normalize(&load_raw_artifact()).expect("normalize must succeed");
    let mut platform = load_platform();
    platform.construct_sections.push(ConstructSection {
        section_kind: "clock".into(),
        section_key: Some("bogus_clk".into()),
        values: Default::default(),
    });

    let err =
        populate_bindings(&mut model, &platform).expect_err("unknown clock id must fail join");
    match &err.kind {
        JoinErrorKind::UnknownPlatformSection { kind, id } => {
            assert_eq!(kind, "clock");
            assert_eq!(id, "bogus_clk");
        }
        other => panic!("expected UnknownPlatformSection, got {other:?}"),
    }
}

// ---------------------------------------------------------------------
// Negative -- required section missing -> Phase2VerificationFailed
// ---------------------------------------------------------------------

#[test]
fn missing_required_platform_section_returns_phase_2_verification_failed() {
    let mut model = normalize(&load_raw_artifact()).expect("normalize must succeed");
    let mut platform = load_platform();
    // Drop the required `[mmio.uart_regs]` section.
    platform
        .construct_sections
        .retain(|s| s.section_kind != "mmio");

    let err = populate_bindings(&mut model, &platform)
        .expect_err("missing required mmio section must fail join");
    match &err.kind {
        JoinErrorKind::Phase2VerificationFailed { inner } => match &inner.kind {
            VerifierErrorKind::RequiredBindingMissing { slot, .. } => {
                assert_eq!(
                    *slot, "mmio",
                    "missing required mmio must bubble up as RequiredBindingMissing",
                );
            }
            other => panic!("expected RequiredBindingMissing, got {other:?}"),
        },
        other => panic!("expected Phase2VerificationFailed, got {other:?}"),
    }
}
