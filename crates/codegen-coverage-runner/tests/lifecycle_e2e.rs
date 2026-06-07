//! Device-lifecycle M0 gate: the `lifecycle-fake` walking skeleton passes the
//! six-stage dual oracle in a DEDICATED, clean process (the emit's MIR is
//! sensitive to rustc's process-global interner history, so the byte-snapshot
//! is reproducible only here — D17). Beyond the snapshot, this asserts the
//! callback-synthesis seam directly: the verified `golden.c` must contain
//! `Fake::on_init`'s DEFINITION and a SYNTHESIZED call to it on the bind-site
//! instance, despite no MIR call terminator referencing it.
//!
//! SOLE test in this binary (like `corpus_e2e`): two `run_fixture`/bridge runs
//! in one process would perturb each other's interner history. `BLESS=1` writes
//! `golden.c` + `expected.stdout` from a single clean emit, then returns without
//! also verifying (which would re-bridge into the now-perturbed interner).
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
fn lifecycle_fake_m0_gate() {
    let dir = lifecycle_corpus().join("lifecycle-fake");

    // BLESS path: a single clean bridge -> emit -> compile+run, written out as
    // the golden snapshot + behavioral expectation. Returns WITHOUT verifying
    // (a second bridge in this process would see the first in the interner).
    if std::env::var_os("BLESS").is_some() {
        // Read the fixture's manifest so the bless emit uses the SAME backend
        // options (the `lifecycle_scenario` selection) as the run path — else the
        // blessed golden would not match a subsequent verify.
        let fixtures = manifest::discover(&lifecycle_corpus()).expect("discover lifecycle corpus");
        let fake = fixtures
            .iter()
            .find(|f| f.manifest.name == "lifecycle-fake")
            .expect("lifecycle-fake fixture discovered");
        let options = pipeline::backend_options_for(fake).expect("backend options");
        let model = match try_bridge_fixture_path(&dir.join("input.rs")) {
            BridgeOutcome::Ok(m) => m,
            other => panic!("bless bridge failed: {other:?}"),
        };
        let emit =
            regseq_backends::emit_generic_c_for_entry_with(&model, &fake.manifest.entry, &options)
                .expect("emit must succeed");
        let run = toolchain::compile_and_run(&emit.tu, &emit.shim, cc())
            .expect("compile + run must succeed");
        std::fs::write(dir.join("golden.c"), &emit.tu).expect("write golden.c");
        std::fs::write(dir.join("expected.stdout"), &run.stdout).expect("write expected.stdout");
        return;
    }

    // The full six-stage dual oracle in this clean process: acquire -> emit ->
    // snapshot vs golden.c -> compile -> run -> stdout oracle.
    let fixtures = manifest::discover(&lifecycle_corpus()).expect("discover lifecycle corpus");
    let fake = fixtures
        .iter()
        .find(|f| f.manifest.name == "lifecycle-fake")
        .expect("lifecycle-fake fixture discovered");
    let result = pipeline::run_fixture(fake, cc());
    assert!(
        result.is_green(),
        "lifecycle-fake must pass all six stages: {:?}",
        result.verdict
    );

    // M0 GATE, asserted on the golden.c the oracle just byte-verified against the
    // fresh emit: the emitted C contains `Fake::on_init`'s DEFINITION and a
    // SYNTHESIZED call to it on the synthesizer's per-scenario instance.
    let golden = std::fs::read_to_string(dir.join("golden.c")).expect("read golden.c");
    assert!(
        golden.contains("on_init(struct input_Fake *"),
        "golden.c must DEFINE Fake::on_init (a fn over `&mut Fake`)"
    );
    assert!(
        golden.contains("on_init(&inst)"),
        "golden.c must SYNTHESIZE a call to Fake::on_init on the scenario instance \
         (no MIR call terminator references it)"
    );
}
