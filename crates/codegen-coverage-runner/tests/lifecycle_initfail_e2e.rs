//! Device-lifecycle M3 gate: the `lifecycle-initfail` fixture passes the
//! six-stage dual oracle in a DEDICATED, clean process (the emit's MIR is
//! sensitive to rustc's process-global interner history, so the byte-snapshot is
//! reproducible only here — D17). Beyond the snapshot, this asserts the
//! init-failure rollback seam directly: the verified `golden.c` must SYNTHESIZE
//! the on_init Result inspection (`regseq_lc_on_init_err()`) and a call to the
//! overridden `FakeFail::on_init_failure` on the failure path.
//!
//! SOLE test in this binary (like `lifecycle_e2e` / `corpus_e2e`): two bridge
//! runs in one process would perturb each other's interner history. `BLESS=1`
//! writes `golden.c` + `expected.stdout` from a single clean emit, then returns
//! without also verifying.
//!
//! The lifecycle fixtures live under `fixtures/_lifecycle/` (a subdir without a
//! manifest), so `corpus_e2e`'s top-level `fixtures/` discovery does not pick
//! them up and their interner history does not perturb the shared corpus.

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
fn lifecycle_initfail_m3_gate() {
    let dir = lifecycle_corpus().join("lifecycle-initfail");

    // BLESS path: a single clean bridge -> emit -> compile+run, written out as
    // the golden snapshot + behavioral expectation. Returns WITHOUT verifying.
    if std::env::var_os("BLESS").is_some() {
        let fixtures = manifest::discover(&lifecycle_corpus()).expect("discover lifecycle corpus");
        let fixture = fixtures
            .iter()
            .find(|f| f.manifest.name == "lifecycle-initfail")
            .expect("lifecycle-initfail fixture discovered");
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

    // The full six-stage dual oracle in this clean process.
    let fixtures = manifest::discover(&lifecycle_corpus()).expect("discover lifecycle corpus");
    let fixture = fixtures
        .iter()
        .find(|f| f.manifest.name == "lifecycle-initfail")
        .expect("lifecycle-initfail fixture discovered");
    let result = pipeline::run_fixture(fixture, cc());
    assert!(
        result.is_green(),
        "lifecycle-initfail must pass all six stages: {:?}",
        result.verdict
    );

    // M3 GATE, asserted on the golden.c the oracle just byte-verified: the emitted
    // C SYNTHESIZES the on_init Result inspection (the failure-trace emit) and a
    // call to the overridden on_init_failure on the failure path. stdout proves
    // the markers fire; golden.c proves the author failure body is actually
    // invoked (not merely the CR-3 default-no-op marker).
    let golden = std::fs::read_to_string(dir.join("golden.c")).expect("read golden.c");
    assert!(
        golden.contains("regseq_lc_on_init_err()"),
        "golden.c must SYNTHESIZE the init-failure trace (regseq_lc_on_init_err)"
    );
    assert!(
        golden.contains("on_init_failure(&inst)"),
        "golden.c must SYNTHESIZE a call to FakeFail::on_init_failure on the failure path"
    );
}
