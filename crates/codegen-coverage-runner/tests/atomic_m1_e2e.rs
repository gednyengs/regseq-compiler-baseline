//! Atomic-memory-access-and-ordering (atomicity construct) M1 vertical slice
//! (T06): the `atomic-m1` host-oracle fixture bridged through the certified
//! six-stage generic_c path (bridge → emit → snapshot → cc → run → stdout
//! oracle). The fixture is L2-free + driverless (D05/D11), so the bind stage is
//! a no-op.
//!
//! `BLESS=1` writes `golden.c` + `expected.stdout` from a single clean
//! bridge+emit+run (the established sole-binary bless exception); the committed
//! goldens are the behavioral oracle the unblessed run diffs against. The
//! gates below assert the load-bearing slice: the always-on `regseq_atomic_*`
//! host model, the cell-init constructor (R10/S10), the four M1 op lowerings
//! reading the const `Ordering` discriminant (C06/D14), and the trace proving
//! value correctness + ordering-tag preservation + the bit-test prior bit (C05).
//!
//! SOLE test in this binary (interner isolation, like the composed e2e tests).

#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::{Path, PathBuf};

use codegen_coverage_runner::{manifest, pipeline, toolchain};
use test_ir_fixtures::{try_bridge_fixture_path, BridgeOutcome};

/// The host `cc`, matching the runner's default.
fn cc() -> &'static Path {
    Path::new("cc")
}

fn atomic_corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/_atomic")
}

#[test]
fn atomic_m1_six_stage_oracle() {
    let fixtures = manifest::discover(&atomic_corpus()).expect("discover atomic corpus");
    let fixture = fixtures
        .iter()
        .find(|f| f.manifest.name == "atomic-m1")
        .expect("atomic-m1 fixture discovered");

    if std::env::var_os("BLESS").is_some() {
        // The bless path mirrors `run_fixture`'s bridge + (no-op) bind + emit so
        // the blessed goldens reflect exactly what the unblessed run compares.
        let options = pipeline::backend_options_for(fixture).expect("backend options");
        let mut model = match try_bridge_fixture_path(&fixture.input) {
            BridgeOutcome::Ok(m) => m,
            other => panic!("bless bridge failed: {other:?}"),
        };
        let identity = pipeline::bind_l2_platform(&mut model, fixture).expect("L2 bind (no-op)");
        let context = regseq_backends::BackendContext::new(options, identity);
        let emit = regseq_backends::emit_generic_c_for_entry_with_context(
            &model,
            &fixture.manifest.entry,
            &context,
        )
        .expect("emit must succeed");
        let run = toolchain::compile_and_run(&emit.tu, &emit.shim, cc())
            .expect("compile + run must succeed");
        std::fs::write(&fixture.golden_c, &emit.tu).expect("write golden.c");
        std::fs::write(&fixture.expected_stdout, &run.stdout).expect("write expected.stdout");
        return;
    }

    // Stages 1-6 (the process's first — interner-clean — bridge).
    let result = pipeline::run_fixture(fixture, cc());
    assert!(
        result.is_green(),
        "atomic-m1 must pass all six stages: {:?}",
        result.verdict
    );

    // SNAPSHOT GATES on the committed golden: the always-on host oracle, the
    // R10/S10 cell-init constructor seeding counter's cell (id 1) to 7, and the
    // four M1 op lowerings against cell ids 0 (flags) / 1 (counter).
    let golden = std::fs::read_to_string(&fixture.golden_c).expect("read golden.c");
    for needle in [
        "static uint64_t regseq_atomic_cells",
        "regseq_atomic_init(void)",
        "regseq_atomic_cells[1] = 0x7ull;",
        "regseq_atomic_store(0, 32, (uint64_t)(",
        "regseq_atomic_load(0, 32,",
        "regseq_atomic_load(1, 32,",
        "regseq_atomic_bittest(0, 32, 0,", // set
        "regseq_atomic_bittest(0, 32, 1,", // clear
    ] {
        assert!(
            golden.contains(needle),
            "golden must contain `{needle}`:\n{golden}"
        );
    }

    // BEHAVIORAL GATES on the canonical oracle trace: value correctness
    // (store->load round-trip), ordering-tag preservation (the Ordering name is
    // printed), the bit-test prior bit (0 on set, 1 on clear), and the
    // field-init (counter loads 7 before any store).
    let stdout = std::fs::read_to_string(&fixture.expected_stdout).expect("read expected.stdout");
    for line in [
        "ATM load cell=1 w=32 ord=Acquire => 0x7",
        "ATM store cell=0 w=32 ord=Release val=0x1",
        "ATM load cell=0 w=32 ord=Acquire => 0x1",
        "ATM bit_set cell=0 w=32 ord=SeqCst bit=3 prior=0",
        "ATM bit_clear cell=0 w=32 ord=SeqCst bit=3 prior=1",
    ] {
        assert!(
            stdout.contains(line),
            "the M1 oracle trace must carry `{line}`; got:\n{stdout}"
        );
    }
}
