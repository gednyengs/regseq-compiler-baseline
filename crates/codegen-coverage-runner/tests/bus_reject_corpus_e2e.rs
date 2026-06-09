//! Bus-resource-management (construct #3) REJECT corpus gate — the end-to-end
//! compile-fail oracle (Stream-B T5 over T4's `expect = "reject"` machinery): the
//! committed `_bus/*` reject fixtures are GREEN, i.e. the runner pipeline rejects
//! each at its `[expected].stage` with its `[expected].diagnostic`. This pairs
//! the unit-tested `regseq_l2::validate_l2_surface` diagnostics (BRM-002/005/006)
//! with the runner — a join-time BRM is a green test when it fails as declared,
//! red if it is accepted or fails differently.
//!
//! These fixtures fail at the Bind stage (no emit / golden / snapshot), so they
//! are interner-history-insensitive and may share one binary — but NOT the
//! keystone's (which snapshots), so they live here, not in `bus_keystone_e2e`.
//! The keystone (`expect = "pass"`) in the same `_bus` corpus is filtered out.
//!
//! `#![feature(rustc_private)]` + `extern crate rustc_driver;` link the
//! rustc-backed bridge (stage 1).

#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::{Path, PathBuf};

use codegen_coverage_runner::manifest::{self, Expect};
use codegen_coverage_runner::pipeline;

/// The host `cc` (never invoked — reject fixtures fail at Bind, before compile).
fn cc() -> &'static Path {
    Path::new("cc")
}

fn bus_corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/_bus")
}

#[test]
fn committed_brm_reject_fixtures_are_green() {
    let fixtures = manifest::discover(&bus_corpus()).expect("discover bus corpus");
    let rejects: Vec<_> = fixtures
        .iter()
        .filter(|f| f.manifest.expect == Expect::Reject)
        .collect();

    // Guard against the filter silently matching nothing (a rename / move would
    // otherwise make this test vacuously pass).
    assert!(
        rejects.len() >= 2,
        "expected at least the BRM-005 + BRM-002 reject fixtures, found {}",
        rejects.len()
    );

    for fixture in rejects {
        let result = pipeline::run_fixture(fixture, cc());
        assert!(
            result.is_green(),
            "reject fixture {} must be GREEN (rejected at its expected stage with its \
             expected diagnostic): {:?}",
            fixture.manifest.name,
            result.verdict
        );
    }
}
