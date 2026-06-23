//! The dma-management M3 PL011 slave-channel golden (dma-management construct,
//! PLAN T26) in a DEDICATED clean process (SOLE test in its binary, like
//! `dma_e1000_region_e2e`): the emit's MIR byte-snapshot is reproducible only in a
//! dedicated process (the rustc interner is process-global).
//!
//! It is the FIRST channel golden that COMPILES + RUNS. It witnesses the full M3
//! channel path on the slave `DmaChannel` + `region.span` + completion-registration
//! surface:
//!   * the compiler-managed `request_channel` (after the region `alloc_coherent`),
//!     the completion `register_completion` (after request_channel), and the LIFO
//!     `release_channel` teardown — the channel lifecycle (T14/T18);
//!   * the 5-op transport in author order: configure → stage_transfer →
//!     start_transfer (each fallible op a real `Result`, T14 BLOCKING #2 fix);
//!   * the `region.span(0..32)` endpoint lowering to the REAL device address
//!     (`regseq_dma_region_dev_base("rx_buf") + 0*1`) and len `32` — M3/T26
//!     replaces the M2 0/0 placeholder;
//!   * a slave channel does NOT set `dma_present` (no `BUS set_master`, T05).
//!
//! `BLESS=1` writes `golden.c` + `expected.stdout` from a single clean
//! bind+emit+run, then returns without verifying (the keystone pattern).

#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::{Path, PathBuf};

use codegen_coverage_runner::{manifest, pipeline, toolchain};
use test_ir_fixtures::{try_bridge_fixture_path, BridgeOutcome};

/// The host `cc`, matching the runner's default.
fn cc() -> &'static Path {
    Path::new("cc")
}

fn dma_corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/_dma")
}

#[test]
fn dma_pl011_channel_transport_oracle() {
    let dir = dma_corpus().join("dma_pl011_channel");
    let fixtures = manifest::discover(&dma_corpus()).expect("discover dma corpus");
    let fixture = fixtures
        .iter()
        .find(|f| f.manifest.name == "dma_pl011_channel")
        .expect("dma_pl011_channel fixture discovered");

    assert!(
        fixture.platform.is_some(),
        "dma_pl011_channel must carry platform.toml (the [dma.rx_buf] join, C02)"
    );

    if std::env::var_os("BLESS").is_some() {
        let options = pipeline::backend_options_for(fixture).expect("backend options");
        let mut model = match try_bridge_fixture_path(&fixture.input) {
            BridgeOutcome::Ok(m) => m,
            other => panic!("bless bridge failed: {other:?}"),
        };
        let identity = pipeline::bind_l2_platform(&mut model, fixture).expect("L2 bind (bless)");
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

    // The runner compiles golden.c + diffs stdout against expected.stdout (the
    // six-stage pipeline). A green verdict IS the COMPILE + RUN proof.
    let result = pipeline::run_fixture(fixture, cc());
    assert!(
        result.is_green(),
        "dma_pl011_channel must pass all six stages (compile + run): {:?}",
        result.verdict
    );

    let golden = std::fs::read_to_string(dir.join("golden.c")).expect("read golden.c");

    // ----------------------------------------------------------------------
    // CODEGEN-SHAPE: the span endpoint lowered to the REAL device address + len
    // (M3/T26 — NOT the M2 0/0 placeholder). dev_base("rx_buf") + 0*1, len 32.
    // ----------------------------------------------------------------------
    assert!(
        golden.contains("regseq_dma_stage_transfer(\"rx\", (regseq_dma_region_dev_base(\"rx_buf\") + 0ull * 1ull), 32ull, 0)"),
        "stage_transfer must lower the span to the real dev_base+offset addr and byte len (M3/T26); got:\n{golden}"
    );
    // The completion registration is emitted (registration-shape only).
    assert!(
        golden.contains("regseq_dma_register_completion(\"rx\")"),
        "the completion registration must be emitted (T18/D06)"
    );

    // ----------------------------------------------------------------------
    // RUNTIME trace oracle: the full M3 channel path in order.
    // ----------------------------------------------------------------------
    let stdout =
        std::fs::read_to_string(dir.join("expected.stdout")).expect("read expected.stdout");
    let pos = |needle: &str| -> usize {
        stdout
            .find(needle)
            .unwrap_or_else(|| panic!("expected.stdout must contain {needle:?}; got:\n{stdout}"))
    };
    let alloc = pos("DMA alloc_coherent rx_buf");
    let request = pos("DMA request_channel rx dir=0");
    let register = pos("DMA register_completion rx");
    let configure = pos("DMA configure rx");
    let stage = pos("DMA stage_transfer rx addr=");
    let start = pos("DMA start_transfer rx");
    let release = pos("DMA release_channel rx");
    let free = pos("DMA free_coherent rx_buf");

    // Preamble order: alloc_coherent (region) → request_channel → register_completion.
    assert!(
        alloc < request && request < register,
        "preamble order: alloc_coherent ({alloc}) → request_channel ({request}) → \
         register_completion ({register})"
    );
    // Transport order: configure → stage_transfer → start_transfer.
    assert!(
        register < configure && configure < stage && stage < start,
        "transport order: register_completion ({register}) → configure ({configure}) → \
         stage_transfer ({stage}) → start_transfer ({start})"
    );
    // LIFO teardown: release_channel BEFORE free_coherent (channel requested after
    // the region alloc → released first).
    assert!(
        start < release && release < free,
        "LIFO teardown: start_transfer ({start}) → release_channel ({release}) → \
         free_coherent ({free})"
    );

    // The stage_transfer trace carries the REAL span addr/len (len=0x20 = 32).
    assert!(
        stdout.contains("DMA stage_transfer rx addr=") && stdout.contains("len=0x20"),
        "the stage_transfer trace must carry the real span addr + len=0x20 (32 bytes); got:\n{stdout}"
    );
    // A slave channel never sets the bus-master flag (T05).
    assert!(
        !stdout.contains("BUS set_master"),
        "a slave DmaChannel must NOT set the bus-master flag (T05 is region-bus-master only)"
    );
    // The completion callback is NEVER fired this cycle (registration only, D06).
    assert!(
        stdout.contains("DMA register_completion rx"),
        "the completion registration must appear in the trace (T18)"
    );
}
