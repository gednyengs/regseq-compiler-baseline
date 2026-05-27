//! Stage-2 emit-seam test (Change J): the `regseq_backends` emit seam is
//! connected and honest. On a real bridged model, `emit_generic_c`
//! returns `Err(NotImplemented)`, and `pipeline::run_fixture` surfaces
//! that as `Fail { stage: Emit, .. }` -- the Phase-1 baseline behavior
//! until tasks b-e implement the lowering.
//!
//! `#![feature(rustc_private)]` + `extern crate rustc_driver;` link the
//! rustc-backed bridge (stage 1) into the test binary.

#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::{Path, PathBuf};

use codegen_coverage_runner::manifest::{Expect, Fixture, FixtureKind, Manifest};
use codegen_coverage_runner::pipeline::{self, FixtureVerdict, Stage};
use regseq_backends::{emit_generic_c, EmitError};
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
fn emit_seam_returns_not_implemented_on_a_real_model() {
    let snippet = TempFixtureDir::new("direct", "#![no_std]\npub fn no_op() {}\n");
    let model = match try_bridge_fixture_path(&snippet.dir.join("input.rs")) {
        BridgeOutcome::Ok(model) => model,
        other => panic!("bridge must succeed, got {other:?}"),
    };
    let err = emit_generic_c(&model).expect_err("stubbed emit must return Err");
    assert!(matches!(err, EmitError::NotImplemented));
    assert_eq!(err.to_string(), "generic-C emission not implemented");
}

#[test]
fn pipeline_tags_the_stubbed_emit_as_stage_emit_failure() {
    let snippet = TempFixtureDir::new("pipeline", "#![no_std]\npub fn no_op() {}\n");
    let fixture = Fixture {
        manifest: Manifest {
            name: "seam_probe".to_string(),
            kind: FixtureKind::Rust,
            entry: "regseq_main".to_string(),
            covers: vec![],
            expect: Expect::Pass,
        },
        dir: snippet.dir.clone(),
        input: snippet.dir.join("input.rs"),
        golden_c: snippet.dir.join("golden.c"),
        expected_stdout: snippet.dir.join("expected.stdout"),
        expected_stderr: None,
    };

    let result = pipeline::run_fixture(&fixture, Path::new("cc"));
    assert_eq!(result.name, "seam_probe");
    match result.verdict {
        FixtureVerdict::Fail { stage, detail } => {
            assert_eq!(
                stage,
                Stage::Emit,
                "stage-1 must pass, stage-2 emit must fail"
            );
            assert!(
                detail.contains("not implemented"),
                "detail should carry the emit error: {detail:?}"
            );
        }
        other => panic!("expected Fail {{ Emit }}, got {other:?}"),
    }
}
