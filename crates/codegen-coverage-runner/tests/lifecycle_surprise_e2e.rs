//! Device-lifecycle M4 gate: the `lifecycle-surprise` fixture passes the
//! six-stage dual oracle in a DEDICATED, clean process (the emit's MIR is
//! sensitive to rustc's process-global interner history, so the byte-snapshot is
//! reproducible only here — D17). Beyond the snapshot, this asserts the
//! surprise-removal dispatch directly: the verified `golden.c` must SYNTHESIZE
//! the device-gone teardown (`regseq_lc_teardown(0)`) and a call to the overridden
//! `FakeSurprise::on_surprise_remove`, and must be BYTE-DISTINCT from orderly (no
//! `regseq_lc_teardown(1)`, no `on_deinit`).
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
fn lifecycle_surprise_m4_gate() {
    let dir = lifecycle_corpus().join("lifecycle-surprise");

    // BLESS path: a single clean bridge -> emit -> compile+run, written out as
    // the golden snapshot + behavioral expectation. Returns WITHOUT verifying.
    if std::env::var_os("BLESS").is_some() {
        let fixtures = manifest::discover(&lifecycle_corpus()).expect("discover lifecycle corpus");
        let fixture = fixtures
            .iter()
            .find(|f| f.manifest.name == "lifecycle-surprise")
            .expect("lifecycle-surprise fixture discovered");
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
        .find(|f| f.manifest.name == "lifecycle-surprise")
        .expect("lifecycle-surprise fixture discovered");
    let result = pipeline::run_fixture(fixture, cc());
    assert!(
        result.is_green(),
        "lifecycle-surprise must pass all six stages: {:?}",
        result.verdict
    );

    // M4 GATE, asserted on the golden.c the oracle just byte-verified: the emitted
    // C SYNTHESIZES the device-gone teardown dispatch (present=0) and a call to the
    // overridden on_surprise_remove, and is BYTE-DISTINCT from orderly. stdout (the
    // dual oracle) asserts the `LC teardown present=0` line.
    let golden = std::fs::read_to_string(dir.join("golden.c")).expect("read golden.c");
    assert!(
        golden.contains("regseq_lc_teardown(0)"),
        "golden.c must SYNTHESIZE the surprise dispatch (regseq_lc_teardown(0))"
    );
    assert!(
        golden.contains("on_surprise_remove(&inst.inner)"),
        "golden.c must SYNTHESIZE a call to FakeSurprise::on_surprise_remove \
         (over the embedded spec driver &inst.inner — the Policy-P container morph)"
    );
    assert!(
        !golden.contains("regseq_lc_teardown(1)") && !golden.contains("on_deinit"),
        "the surprise scenario must be byte-distinct from orderly (no present=1, no on_deinit)"
    );
}
