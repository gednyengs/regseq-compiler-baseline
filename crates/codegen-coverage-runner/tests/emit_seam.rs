//! Stage-2 emit-seam test (Change J): the `regseq_backends` emit seam is
//! connected and honest. As of regseq-backends-05 (Phase-1 task b) the seam
//! is live: on a real bridged model `emit_generic_c` returns a real `Ok(tu)`
//! (the section 15 bank-1 prelude; banks 2-5 are still empty hooks), and
//! `pipeline::run_fixture` therefore advances *past* stage 2 -- it no longer
//! fails at `Stage::Emit`.
//!
//! `#![feature(rustc_private)]` + `extern crate rustc_driver;` link the
//! rustc-backed bridge (stage 1) into the test binary.

#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::{Path, PathBuf};

use codegen_coverage_runner::manifest::{Expect, Fixture, FixtureKind, Manifest};
use codegen_coverage_runner::pipeline::{self, FixtureVerdict, Stage};
use regseq_backends::emit_generic_c;
use test_ir_fixtures::{try_bridge_fixture_path, BridgeOutcome};

/// A temp directory (with an `input.rs`) removed on drop.
struct TempFixtureDir {
    dir: PathBuf,
}

impl TempFixtureDir {
    fn new(label: &str, src: &str) -> TempFixtureDir {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir =
            std::env::temp_dir().join(format!("ccr-seam-{label}-{}-{nanos}", std::process::id()));
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

#[test]
fn emit_seam_returns_a_real_tu_on_a_real_model() {
    let snippet = TempFixtureDir::new("direct", "#![no_std]\npub fn no_op() {}\n");
    let model = match try_bridge_fixture_path(&snippet.dir.join("input.rs")) {
        BridgeOutcome::Ok(model) => model,
        other => panic!("bridge must succeed, got {other:?}"),
    };
    // Task b: the seam is live -- a real bridged model emits the bank-1
    // prelude (banks 2-5 are still empty hooks), never Err(NotImplemented).
    let tu = emit_generic_c(&model).expect("emit must return Ok on a real model");
    assert!(tu.contains("#include <stdint.h>"));
    assert!(tu.contains("_Noreturn static inline void regseq_panic(const char *msg)"));
}

#[test]
fn pipeline_advances_past_emit_now_that_the_seam_is_live() {
    // The emit stage is now entry-aware (regseq-backends-27): it resolves +
    // classifies the entry alongside the TU, so the probe fixture carries a
    // unit-void `regseq_main` for emit to resolve.
    let snippet = TempFixtureDir::new(
        "pipeline",
        "#![no_std]\n#[no_mangle]\npub extern \"C\" fn regseq_main() {}\n",
    );
    let fixture = Fixture {
        manifest: Manifest {
            name: "seam_probe".to_string(),
            kind: FixtureKind::Rust,
            entry: "regseq_main".to_string(),
            covers: vec![],
            expect: Expect::Pass,
            overflow_checks: None,
            expected: Default::default(),
            lifecycle_scenario: None,
        },
        dir: snippet.dir.clone(),
        input: snippet.dir.join("input.rs"),
        golden_c: snippet.dir.join("golden.c"),
        expected_stdout: snippet.dir.join("expected.stdout"),
        expected_stderr: None,
    };

    let result = pipeline::run_fixture(&fixture, Path::new("cc"));
    assert_eq!(result.name, "seam_probe");
    // Stage 1 (acquire) and stage 2 (emit + entry-shim classification) now both
    // pass; with no committed golden.c the fixture stops at stage 3 (snapshot)
    // -- proving emit no longer short-circuits the pipeline at Stage::Emit.
    match result.verdict {
        FixtureVerdict::Fail { stage, detail } => {
            assert_ne!(stage, Stage::Acquire, "stage-1 acquire must pass");
            assert_ne!(stage, Stage::Emit, "stage-2 emit must now pass");
            assert_eq!(
                stage,
                Stage::Snapshot,
                "with no golden.c the fixture stops at the snapshot stage"
            );
            assert!(
                detail.contains("cannot read golden"),
                "detail should carry the missing-golden error: {detail:?}"
            );
        }
        other => panic!("expected Fail {{ Snapshot }}, got {other:?}"),
    }
}
