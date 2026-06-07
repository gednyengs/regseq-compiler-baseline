//! Device-lifecycle M5 gate (shutdown-PRESENT): the `lifecycle-e1000` fixture
//! passes the six-stage dual oracle in a DEDICATED, clean process (the emit's MIR
//! is sensitive to rustc's process-global interner history, so the byte-snapshot
//! is reproducible only here — D17). Beyond the snapshot, this asserts the
//! shutdown-present path: the verified `golden.c` must SYNTHESIZE a call to the
//! overridden `E1000::on_system_shutdown` and carry the PCI match in the
//! registration trace.
//!
//! SOLE test in this binary (like `lifecycle_e2e` / `corpus_e2e`): two bridge
//! runs in one process would perturb each other's interner history. `BLESS=1`
//! writes `golden.c` + `expected.stdout` from a single clean emit, then returns
//! without also verifying.

#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::{Path, PathBuf};

use codegen_coverage_runner::{manifest, pipeline, toolchain};
use test_ir_fixtures::{try_bridge_fixture_path, BridgeOutcome};

/// The host `cc`, matching the runner's default.
fn cc() -> &'static Path {
    Path::new("cc")
}

fn lifecycle_corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/_lifecycle")
}

#[test]
fn lifecycle_e1000_m5_gate() {
    let dir = lifecycle_corpus().join("lifecycle-e1000");

    if std::env::var_os("BLESS").is_some() {
        let fixtures = manifest::discover(&lifecycle_corpus()).expect("discover lifecycle corpus");
        let fixture = fixtures
            .iter()
            .find(|f| f.manifest.name == "lifecycle-e1000")
            .expect("lifecycle-e1000 fixture discovered");
        let options = pipeline::backend_options_for(fixture).expect("backend options");
        let model = match try_bridge_fixture_path(&dir.join("input.rs")) {
            BridgeOutcome::Ok(m) => m,
            other => panic!("bless bridge failed: {other:?}"),
        };
        let emit = regseq_backends::emit_generic_c_for_entry_with(
            &model,
            &fixture.manifest.entry,
            &options,
        )
        .expect("emit must succeed");
        let run = toolchain::compile_and_run(&emit.tu, &emit.shim, cc())
            .expect("compile + run must succeed");
        std::fs::write(dir.join("golden.c"), &emit.tu).expect("write golden.c");
        std::fs::write(dir.join("expected.stdout"), &run.stdout).expect("write expected.stdout");
        return;
    }

    let fixtures = manifest::discover(&lifecycle_corpus()).expect("discover lifecycle corpus");
    let fixture = fixtures
        .iter()
        .find(|f| f.manifest.name == "lifecycle-e1000")
        .expect("lifecycle-e1000 fixture discovered");
    let result = pipeline::run_fixture(fixture, cc());
    assert!(
        result.is_green(),
        "lifecycle-e1000 must pass all six stages: {:?}",
        result.verdict
    );

    // M5 GATE (shutdown-present): the golden must SYNTHESIZE a call to the
    // overridden on_system_shutdown (the slot is PRESENT, D10/I02), and carry the
    // PCI match in the registration trace (LC-3). stdout (dual oracle) asserts the
    // `LC call on_system_shutdown` line.
    let golden = std::fs::read_to_string(dir.join("golden.c")).expect("read golden.c");
    assert!(
        golden.contains("on_system_shutdown(&inst)"),
        "golden.c must SYNTHESIZE a call to E1000::on_system_shutdown (shutdown-present)"
    );
    assert!(
        golden.contains("pci:0x8086:0x100e"),
        "golden.c must carry the PCI match in the registration trace (LC-3)"
    );
}
