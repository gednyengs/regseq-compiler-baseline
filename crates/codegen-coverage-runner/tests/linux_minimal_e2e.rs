//! The linux_c minimal snapshot gate (backend-linux-c T05).
//!
//! The smallest input through the C02 linux pipeline row: bridge →
//! `emit_linux_c` → byte-exact `golden.linux.c`, with stage-attributed
//! verdicts (`FixtureVerdict`). `BLESS=1` writes the snapshot from a single
//! clean bridge+emit — for THIS binary's fixture only (I04).
//!
//! SOLE test in this binary (the established sole-binary pattern): the
//! bridged MIR is sensitive to rustc's process-global interner history, so
//! the byte snapshot is reproducible only in a dedicated clean process.
//!
//! `#![feature(rustc_private)]` + `extern crate rustc_driver;` link the
//! rustc-backed bridge (stage 1) into the test binary.

#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::{Path, PathBuf};

use codegen_coverage_runner::{manifest, pipeline};
use test_ir_fixtures::{try_bridge_fixture_path, BridgeOutcome};

fn linux_corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/_linux")
}

#[test]
fn linux_minimal_snapshot_locks_the_scaffold() {
    let fixtures = manifest::discover(&linux_corpus()).expect("discover linux corpus");
    let fixture = fixtures
        .iter()
        .find(|f| f.manifest.name == "linux-minimal")
        .expect("linux-minimal fixture discovered");

    if std::env::var_os("BLESS").is_some() {
        let mut model = match try_bridge_fixture_path(&fixture.input) {
            BridgeOutcome::Ok(m) => m,
            other => panic!("bless bridge failed: {other:?}"),
        };
        pipeline::bind_l2_platform(&mut model, fixture).expect("L2 bind (bless)");
        let emit = regseq_backends::emit_linux_c(&model).expect("emit_linux_c must succeed");
        std::fs::write(pipeline::linux_golden_path(fixture), &emit.tu)
            .expect("write golden.linux.c");
        std::fs::write(
            pipeline::linux_conformance_path(fixture),
            codegen_coverage_runner::conformance::render_json(&emit.conformance),
        )
        .expect("write conformance.linux.json");
        return;
    }

    // The C02 linux row, stage-attributed.
    let result = pipeline::run_linux_fixture(fixture);
    assert!(
        result.is_green(),
        "linux-minimal must pass bridge -> emit_linux_c -> snapshot: {:?}",
        result.verdict
    );

    // Artifact-set gates beyond the byte snapshot: the Kbuild fragment and
    // module name (GRILL D04/S09), re-derived from a fresh emit over the
    // same bound model.
    let mut model = match try_bridge_fixture_path(&fixture.input) {
        BridgeOutcome::Ok(m) => m,
        other => panic!("bridge failed: {other:?}"),
    };
    pipeline::bind_l2_platform(&mut model, fixture).expect("L2 bind");
    let emit = regseq_backends::emit_linux_c(&model).expect("emit_linux_c");
    assert_eq!(emit.module_name, "linux_minimal");
    assert_eq!(emit.kbuild, "obj-m += linux_minimal.o\n");
}
