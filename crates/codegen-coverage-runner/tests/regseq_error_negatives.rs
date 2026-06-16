//! regseq-backends-31 harness integration negatives (PLAN T13/T14): prove the
//! two fail-loud RegseqError paths the design promises, driven through the real
//! runner pipeline -- the same path `cargo-regseq` exercises.
//!
//! 1. A `#[rseq::framework_type("regseq-error")]`-marked enum carrying a variant
//!    absent from the C02 table (`Frobnicate`) must fail at `Stage::Emit` naming
//!    the variant -- never a silent `EIO` (R08 / I05 / D05).
//! 2. A manifest-custom `entry` combined with an error-returning signature is
//!    deferred this phase (D09), so it must fail at `Stage::Emit` naming the shape
//!    (R09 / D08).
//!
//! Both fixtures BRIDGE cleanly (stage 1) -- an error-returning entry is the
//! proven `regseq-driver-90` shape -- but must fail loud at emit (stage 2). The
//! backend is the single enforcement point (contract C04), so the runner and
//! `cargo-regseq --emit bin` reject identically; the driver-side consumer's
//! parity is proven in `regseq-driver/tests/emit_bin_exit.rs`.
//!
//! These are NEGATIVE fixtures, so they live here rather than in the on-disk
//! corpus (which `corpus_e2e.rs` requires to pass all six stages). Each test
//! drives one inline snippet through `pipeline::run_fixture`, mirroring the
//! `emit_seam.rs` pattern.
//!
//! `#![feature(rustc_private)]` + `extern crate rustc_driver;` link the
//! rustc-backed bridge (stage 1) into the test binary.

#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::{Path, PathBuf};

use codegen_coverage_runner::manifest::{Expect, Fixture, FixtureKind, Manifest};
use codegen_coverage_runner::pipeline::{self, FixtureVerdict, Stage};

/// A temp fixture directory (with an `input.rs`) removed on drop.
struct TempFixtureDir {
    dir: PathBuf,
}

impl TempFixtureDir {
    fn new(label: &str, src: &str) -> TempFixtureDir {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!(
            "ccr-rerr-neg-{label}-{}-{nanos}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).expect("must create temp fixture dir");
        std::fs::write(dir.join("input.rs"), src).expect("must write input.rs");
        TempFixtureDir { dir }
    }
}

impl Drop for TempFixtureDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// The host `cc`. These negatives fail at stage 2 (emit), so the compiler is
/// never actually invoked; this only satisfies `run_fixture`'s signature.
fn cc() -> &'static Path {
    Path::new("cc")
}

/// A self-declaring `#[rseq::framework_type("regseq-error")]` enum over
/// `variants` plus an `entry_fn` returning `Err(RegseqError::<returned>)`.
/// Mirrors `regseq-driver/tests/emit_bin_exit.rs::err_entry_source` (the proven
/// error-returning bridge shape), parameterized so the entry name and the
/// variant set are under the test's control. `no_std` keeps the emitted TU
/// minimal; the bridge stops before codegen so no panic handler is needed.
fn err_source(entry_fn: &str, variants: &[&str], returned: &str) -> String {
    let decls = variants.join(",\n    ");
    format!(
        r#"#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

#[rseq::framework_type("regseq-error")]
pub enum RegseqError {{
    {decls},
}}

pub fn {entry_fn}() -> Result<(), RegseqError> {{
    Err(RegseqError::{returned})
}}
"#
    )
}

/// Build an inline `Fixture` over `dir` (kind=rust, expect=pass) whose manifest
/// `entry` is `entry`. With no `golden.c`, a fixture that PASSED emit would stop
/// at stage 3; these negatives stop earlier, at stage 2.
fn fixture(dir: &TempFixtureDir, entry: &str) -> Fixture {
    Fixture {
        manifest: Manifest {
            name: "regseq_error_negative".to_string(),
            kind: FixtureKind::Rust,
            entry: entry.to_string(),
            covers: vec![],
            expect: Expect::Pass,
            overflow_checks: None,
            expected: Default::default(),
            lifecycle_scenario: None,
            irq_fire: None,
        },
        dir: dir.dir.clone(),
        input: dir.dir.join("input.rs"),
        golden_c: dir.dir.join("golden.c"),
        expected_stdout: dir.dir.join("expected.stdout"),
        expected_stderr: None,
        platform: None,
    }
}

#[test]
fn unknown_marked_variant_fails_at_emit_naming_the_variant() {
    // T13 / R08 / I05 / D05: a marked RegseqError variant absent from the C02
    // table (`Frobnicate`) must fail loud at emit naming the variant, surfaced by
    // the runner as a `Stage::Emit` failure -- never a degraded path, never a
    // silent map to `EIO`.
    let src = err_source(
        "regseq_main",
        &["InvalidParameter", "Frobnicate"],
        "Frobnicate",
    );
    let td = TempFixtureDir::new("unknown-variant", &src);
    let fx = fixture(&td, "regseq_main");

    match pipeline::run_fixture(&fx, cc()).verdict {
        FixtureVerdict::Fail { stage, detail } => {
            assert_eq!(
                stage,
                Stage::Emit,
                "the unknown marked variant must fail at emit (stage 2): {detail}"
            );
            assert!(
                detail.contains("Frobnicate"),
                "the diagnostic must name the offending variant: {detail}"
            );
            assert!(
                detail.contains("no errno mapping"),
                "the diagnostic must be the fail-loud errno-mapping error: {detail}"
            );
            // I05: reaching a `Stage::Emit` Fail means no C was produced, and the
            // diagnostic states the contract explicitly -- the unmapped variant
            // fails emit rather than being mapped to a generic code (a silent EIO).
            assert!(
                detail.contains("fails emit rather than mapping to a generic code"),
                "the diagnostic must document the never-silent-EIO contract: {detail}"
            );
        }
        other => panic!("expected Fail {{ Emit }}, got {other:?}"),
    }
}

#[test]
fn custom_entry_with_error_return_fails_at_emit_naming_the_shape() {
    // T14 / R09 / D08 / D09: a manifest-custom entry (a non-default suffix)
    // combined with an error-returning signature is deferred this phase, so it
    // must fail loud at emit naming the shape -- surfaced by the runner as a
    // `Stage::Emit` failure. All variants are C02-valid so the entry-shape check
    // (which fires first, before the adapter is built) is the cause, not an
    // errno-mapping miss.
    let src = err_source(
        "my_custom_entry",
        &["InvalidParameter", "OutOfMemory"],
        "InvalidParameter",
    );
    let td = TempFixtureDir::new("custom-entry", &src);
    let fx = fixture(&td, "my_custom_entry");

    match pipeline::run_fixture(&fx, cc()).verdict {
        FixtureVerdict::Fail { stage, detail } => {
            assert_eq!(
                stage,
                Stage::Emit,
                "the custom error-returning entry must fail at emit (stage 2): {detail}"
            );
            assert!(
                detail.contains("manifest-custom entry") && detail.contains("error-returning"),
                "the diagnostic must name the custom-entry+error-return shape: {detail}"
            );
            assert!(
                detail.contains("my_custom_entry"),
                "the diagnostic must name the offending entry: {detail}"
            );
        }
        other => panic!("expected Fail {{ Emit }}, got {other:?}"),
    }
}
