//! Task-e Test Plan B5 / Change G: the committed on-disk diag-log corpus passes
//! all six pipeline stages (acquire -> emit -> snapshot vs the blessed golden.c
//! -> compile -> run -> stdout oracle). A wrong entry symbol (the Change-F gap)
//! would be an undefined symbol at link, so a clean six-stage pass proves the
//! entry resolution + the section-8 lowering + the snapshot oracle together.
//!
//! This is the SOLE test in its binary on purpose. Stage 3 byte-compares the
//! freshly emitted C against the committed `golden.c`, and the emit's MIR is
//! sensitive to rustc's process-global interner history -- so the snapshot is
//! reproducible only in a process that has not accumulated unrelated bridge
//! compilations. A dedicated binary gives that clean process, which is exactly
//! the canonical Mode-A context the runner regenerates the ledger from. (The
//! bless / entry-resolution bridges live in `diag_harness.rs`; behavioral
//! per-facet compile-and-run coverage lives in `generic_c_bodies.rs`.)
//!
//! `#![feature(rustc_private)]` + `extern crate rustc_driver;` link the
//! rustc-backed bridge (stage 1) into the test binary.

#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::Path;

use codegen_coverage_runner::manifest;
use codegen_coverage_runner::pipeline;

/// The host `cc`, matching the runner's default.
fn cc() -> &'static Path {
    Path::new("cc")
}

#[test]
fn on_disk_corpus_passes_all_stages() {
    let corpus = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures");
    let fixtures = manifest::discover(&corpus).expect("discover the committed corpus");
    assert!(
        fixtures.len() >= 10,
        "the task-e corpus has at least 10 fixtures, found {}",
        fixtures.len()
    );
    for fixture in &fixtures {
        let result = pipeline::run_fixture(fixture, cc());
        assert!(
            result.is_green(),
            "fixture {} must pass all six stages: {:?}",
            fixture.manifest.name,
            result.verdict
        );
    }
}
