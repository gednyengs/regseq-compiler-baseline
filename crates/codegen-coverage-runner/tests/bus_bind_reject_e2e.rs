//! Bus-resource-management (construct #3) NEGATIVE bind gate. Two layers:
//!
//! 1. T2 error-attribution: a `platform.toml` that trips a BRM diagnostic fails
//!    the pipeline at the new `Stage::Bind` (1b), tagged with the BRM code, BEFORE
//!    emit (`half_specified_region…` BRM-005; `bus_managed_region_with_no_platform…`
//!    BRM-006). The positive `bus_keystone_e2e` only covers the happy path.
//! 2. T4 `apply_expectation` POLARITY: an `expect = "reject"` fixture is RED unless
//!    it fails EXACTLY as declared — RED when accepted through emit, when it fails
//!    at the wrong stage, or with the wrong diagnostic. (The committed reject
//!    fixtures' GREEN path is `bus_reject_corpus_e2e`.)
//!
//! These reuse the verified `_bus/*` `input.rs` files (real bus+mmio drivers); the
//! BRM-005 case points at a temp `platform.toml` (base without size). No golden /
//! emit is reached, so this is robust to interner history; it is nonetheless its
//! OWN binary (it bridges, and the keystone snapshot must not share a process).
//!
//! `#![feature(rustc_private)]` + `extern crate rustc_driver;` link the
//! rustc-backed bridge (stage 1).

#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::{Path, PathBuf};

use codegen_coverage_runner::manifest::{Expect, Expected, Fixture, FixtureKind, Manifest};
use codegen_coverage_runner::pipeline::{self, FixtureVerdict, Stage};

/// The host `cc`, matching the runner's default (never invoked — we fail at Bind).
fn cc() -> &'static Path {
    Path::new("cc")
}

/// A temp dir holding the BRM-tripping `platform.toml`, removed on drop.
struct TempPlatform {
    dir: PathBuf,
}

impl TempPlatform {
    fn new(body: &str) -> TempPlatform {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir =
            std::env::temp_dir().join(format!("ccr-busreject-{}-{nanos}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("create temp platform dir");
        std::fs::write(dir.join("platform.toml"), body).expect("write platform.toml");
        TempPlatform { dir }
    }

    fn platform_path(&self) -> PathBuf {
        self.dir.join("platform.toml")
    }
}

impl Drop for TempPlatform {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

#[test]
fn half_specified_region_fails_at_bind_with_brm_005() {
    // The keystone driver (`bus = "pci"`, `#[rseq::mmio(id = "regs", bar = 0)]`)
    // is a valid bus+mmio driver — it bridges cleanly. We swap in a platform.toml
    // that supplies `base` but not `size`, which is a half-specified region
    // (BRM-005): `validate_l2_surface` rejects it at Stage::Bind.
    let keystone =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/_bus/brm-e1000-keystone");
    let bad = TempPlatform::new("version = \"1\"\n[mmio.regs]\nbar = 0\nbase = 0xf0000000\n");

    let fixture = Fixture {
        manifest: Manifest {
            name: "brm-halfspec-reject".to_string(),
            kind: FixtureKind::Rust,
            entry: "regseq_main".to_string(),
            covers: vec![],
            expect: Expect::Pass,
            overflow_checks: None,
            expected: Default::default(),
            lifecycle_scenario: None,
        },
        dir: keystone.clone(),
        input: keystone.join("input.rs"),
        // Never read — we fail at Bind, before snapshot/compile.
        golden_c: keystone.join("golden.c"),
        expected_stdout: keystone.join("expected.stdout"),
        expected_stderr: None,
        platform: Some(bad.platform_path()),
    };

    let result = pipeline::run_fixture(&fixture, cc());
    match result.verdict {
        FixtureVerdict::Fail { stage, detail } => {
            assert_eq!(
                stage,
                Stage::Bind,
                "a BRM violation must be attributed to Stage::Bind, got {stage:?} ({detail})"
            );
            assert!(
                detail.contains("BRM-005"),
                "the Bind failure must carry the BRM-005 code; got: {detail}"
            );
        }
        other => panic!("expected a Stage::Bind failure, got {other:?}"),
    }
}

#[test]
fn bus_managed_region_with_no_platform_file_fails_at_bind_with_brm_006() {
    // Production runs `validate_l2_surface` for EVERY bus-declared driver, even
    // when no platform file is present (callbacks.rs:237). The harness must too:
    // a bus-managed region with no resolvable `(base, size)` — here the keystone
    // PCI driver with `platform = None` — is UNRESOLVABLE (BRM-006) and must fail
    // at Stage::Bind, NOT false-green by skipping validation. (Codex T1–T3
    // review finding #1.)
    let keystone =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/_bus/brm-e1000-keystone");

    let fixture = Fixture {
        manifest: Manifest {
            name: "brm-no-platform-reject".to_string(),
            kind: FixtureKind::Rust,
            entry: "regseq_main".to_string(),
            covers: vec![],
            expect: Expect::Pass,
            overflow_checks: None,
            expected: Default::default(),
            lifecycle_scenario: None,
        },
        dir: keystone.clone(),
        input: keystone.join("input.rs"),
        golden_c: keystone.join("golden.c"),
        expected_stdout: keystone.join("expected.stdout"),
        expected_stderr: None,
        // The crux: NO platform file. Production would still reject this
        // bus-managed region; the harness must not skip the L2-surface validation.
        platform: None,
    };

    let result = pipeline::run_fixture(&fixture, cc());
    match result.verdict {
        FixtureVerdict::Fail { stage, detail } => {
            assert_eq!(
                stage,
                Stage::Bind,
                "an unresolvable bus region must fail at Stage::Bind, got {stage:?} ({detail})"
            );
            assert!(
                detail.contains("BRM-006"),
                "the Bind failure must carry the BRM-006 (unresolvable region) code; got: {detail}"
            );
        }
        other => panic!("expected a Stage::Bind BRM-006 failure, got {other:?}"),
    }
}

// ---- T4 apply_expectation polarity: a `reject` fixture is RED unless it fails
//      EXACTLY as declared. Without these, a bug making reject vacuously green
//      would go undetected. -------------------------------------------------

/// Build an `expect = "reject"` fixture over an existing fixture dir's `input.rs`
/// + an optional platform file, with the given `[expected]` localization.
fn reject_fixture(
    name: &str,
    dir: &Path,
    platform: Option<PathBuf>,
    stage: Option<Stage>,
    diagnostic: Option<&str>,
) -> Fixture {
    Fixture {
        manifest: Manifest {
            name: name.to_string(),
            kind: FixtureKind::Rust,
            entry: "regseq_main".to_string(),
            covers: vec![],
            expect: Expect::Reject,
            overflow_checks: None,
            expected: Expected {
                exit: None,
                stage,
                diagnostic: diagnostic.map(str::to_string),
            },
            lifecycle_scenario: Some("orderly".to_string()),
        },
        dir: dir.to_path_buf(),
        input: dir.join("input.rs"),
        golden_c: dir.join("golden.c"),
        expected_stdout: dir.join("expected.stdout"),
        expected_stderr: None,
        platform,
    }
}

#[test]
fn reject_fixture_accepted_through_emit_is_red() {
    // The keystone binds successfully (resolved region) and emits — it is
    // ACCEPTED. Marked `expect = "reject"`, it must be RED, not vacuously green.
    let keystone =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/_bus/brm-e1000-keystone");
    let fixture = reject_fixture(
        "keystone-as-reject",
        &keystone,
        Some(keystone.join("platform.toml")),
        Some(Stage::Bind),
        Some("BRM-005"),
    );
    let result = pipeline::run_fixture(&fixture, cc());
    assert!(
        result.is_fail(),
        "an accepted fixture marked expect=reject must be RED; got {:?}",
        result.verdict
    );
}

#[test]
fn reject_fixture_with_wrong_diagnostic_is_red() {
    // The halfspec fixture genuinely fails at Bind with BRM-005, but the
    // expectation demands a different code → RED (failed for the wrong reason).
    let halfspec =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/_bus/brm-halfspec-reject");
    let fixture = reject_fixture(
        "halfspec-wrong-diag",
        &halfspec,
        Some(halfspec.join("platform.toml")),
        Some(Stage::Bind),
        Some("BRM-999"),
    );
    let result = pipeline::run_fixture(&fixture, cc());
    assert!(
        result.is_fail(),
        "a reject fixture that fails with the wrong diagnostic must be RED; got {:?}",
        result.verdict
    );
}

#[test]
fn reject_fixture_failing_at_wrong_stage_is_red() {
    // The halfspec fixture fails at Bind, but the expectation demands Acquire →
    // RED (failed, but not where it was meant to).
    let halfspec =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/_bus/brm-halfspec-reject");
    let fixture = reject_fixture(
        "halfspec-wrong-stage",
        &halfspec,
        Some(halfspec.join("platform.toml")),
        Some(Stage::Acquire),
        Some("BRM-005"),
    );
    let result = pipeline::run_fixture(&fixture, cc());
    assert!(
        result.is_fail(),
        "a reject fixture that fails at the wrong stage must be RED; got {:?}",
        result.verdict
    );
}
