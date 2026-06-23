//! The dma-management M1 E1000 coherent-region golden (dma-management construct,
//! PLAN T23) in a DEDICATED clean process (SOLE test in its binary, like
//! `e1000_composed_e2e` / `clock-pl011-static`): the emit's MIR byte-snapshot is
//! reproducible only in a dedicated process (the rustc interner is process-global).
//!
//! It witnesses the FULL M1 coherent path on the REAL closure-borrow surface.
//! NOTE ON CLAIM STRENGTH (FIX #5): the descriptor-field assertions below are
//! CODEGEN-SHAPE checks — they assert the EMITTED C lowers the closure's typed
//! field stores into a typed temporary that is `memcpy`'d into the region's
//! coherent backing buffer (the FIX #3 sound shape) with the exact author byte
//! values. They are NOT a runtime byte-dump assertion: the synthesized harness
//! `main` is shared across fixtures and has no per-fixture hook to dump the
//! backing buffer, so a true backing-buffer byte oracle would require a
//! cross-cutting harness change (out of scope for this pass). The genuine RUNTIME
//! oracle here is the DMA trace ordering in `expected.stdout`
//! (`alloc_coherent → wmb → free_coherent`). The pieces:
//!   * the closure-borrow element-access (`region.write(0, |d: &mut V| { … })`,
//!     T13) lowers its typed field stores into a typed temp `memcpy`'d into the
//!     coherent backing buffer — the CODEGEN-SHAPE (bytes-in-emit) check;
//!   * the dma-management store→doorbell fence (`regseq_dma_wmb()`, T16/C07/D09)
//!     falls between the region stores and the TX-tail doorbell write — the
//!     CODEGEN-SHAPE (order) oracle: `…region stores… → regseq_dma_wmb() →
//!     regseq_mmio_w32(<TDT>)`, FAILS if the fence is absent or misordered;
//!   * the compiler-managed `alloc_coherent`/`free_coherent` lifecycle frames the
//!     device lifetime (T12), `set_bus_master`/`dma_present` rides the bus
//!     acquisition (T05), and `device_addr()` programs the ring base (A2).
//!
//! `BLESS=1` writes `golden.c` + `expected.stdout` from a single clean
//! bind+emit+run, then returns without also verifying (the keystone pattern).

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
fn dma_e1000_region_six_stage_oracle() {
    let dir = dma_corpus().join("dma_e1000_region");
    let fixtures = manifest::discover(&dma_corpus()).expect("discover dma corpus");
    let fixture = fixtures
        .iter()
        .find(|f| f.manifest.name == "dma_e1000_region")
        .expect("dma_e1000_region fixture discovered");

    assert!(
        fixture.platform.is_some(),
        "dma_e1000_region must carry platform.toml (the [dma.tx_ring] join, C02)"
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

    let result = pipeline::run_fixture(fixture, cc());
    assert!(
        result.is_green(),
        "dma_e1000_region must pass all six stages: {:?}",
        result.verdict
    );

    let golden = std::fs::read_to_string(dir.join("golden.c")).expect("read golden.c");

    // ----------------------------------------------------------------------
    // CODEGEN-SHAPE oracle (T16/C07/D09): the publishing region stores, then the
    // store→doorbell fence, then the TX-tail doorbell MMIO write — IN THAT ORDER.
    // register-access emits the plain doorbell; dma-management inserts ONLY the
    // fence between them. FAILS if the fence is absent or misordered.
    // ----------------------------------------------------------------------
    let store = golden
        .find("__dma_view->buffer_addr")
        .expect("the closure's descriptor field store must be inlined into golden.c (T13)");
    let fence = golden
        .find("regseq_dma_wmb()")
        .expect("the dma-management store→doorbell fence must be emitted (T16)");
    // The TX-tail doorbell is the whole-register write at offset 0x18 = 24 (TDT;
    // the backend renders offsets in decimal). The E1000 BAR0 is bus-resolved, so
    // the doorbell rides the BASED accessor. Search from the fence forward so a
    // doorbell write in an unrelated earlier block cannot satisfy the order check.
    let doorbell = golden[fence..]
        .find("regseq_bus_mmio_write(24,")
        .or_else(|| golden[fence..].find("regseq_mmio_write(24,"))
        .map(|p| fence + p)
        .expect("the TX-tail doorbell MMIO write (offset 24 = 0x18) must be emitted after the fence");
    assert!(
        store < fence && fence < doorbell,
        "ordering MUST be: region stores ({store}) → regseq_dma_wmb() ({fence}) → doorbell ({doorbell})"
    );

    // CODEGEN-SHAPE (bytes-in-emit) check: the closure's THREE field stores are
    // inlined as typed stores into a TYPED TEMPORARY (the FIX #3 sound shape — no
    // strict-aliasing/alignment UB from a `V*` cast through the byte buffer), each
    // carrying the EXACT descriptor byte value the author wrote. The constants:
    // buffer_addr = 0x100002000 (4294975488), lower = 0x0B000040 (184549440),
    // upper = 0. (This asserts the EMITTED C, not a runtime byte dump — see the
    // module note on claim strength, FIX #5.)
    assert!(
        golden.contains("__dma_view->buffer_addr = ((uint64_t)(4294975488ULL));"),
        "the buffer_addr descriptor store must be emitted against the typed temp with the exact byte value"
    );
    assert!(
        golden.contains("__dma_view->lower = ((uint32_t)(184549440U));"),
        "the lower descriptor word store must be emitted with the exact byte value"
    );
    assert!(
        golden.contains("__dma_view->upper = ((uint32_t)(0U));"),
        "the upper descriptor word store must be emitted with the exact byte value"
    );
    // FIX #3 soundness: the view is a pointer to a TYPED TEMPORARY (`V *__dma_view
    // = &__dma_tmp;`), NOT a `V*` cast of the byte buffer; the temp's bytes are
    // memcpy'd INTO the coherent backing buffer (a publishing write), so the
    // descriptor bytes land without strict-aliasing/alignment UB.
    assert!(
        golden.contains("*__dma_view = &__dma_tmp;"),
        "FIX #3: the element view must be a pointer to a typed temporary, not a V* cast of the byte buffer"
    );
    assert!(
        golden.contains("memcpy(__dma_slot, &__dma_tmp, sizeof(struct input_LegacyTxDesc));"),
        "FIX #3: a publishing write must memcpy the typed temp into the coherent backing buffer (the bytes land)"
    );
    // The byte slot reaches into the coherent backing buffer at base +
    // i*element_size (element_size = 16).
    assert!(
        golden.contains("regseq_dma_region_cpu(\"tx_ring\")"),
        "the element_access slot must point into the region's coherent backing buffer"
    );
    // FIX #3: the backing buffer is an ALIGNED heap allocation honoring the
    // declared alignment (NOT an inline `unsigned char buf[]` with only char
    // alignment), so the memcpy at the element stride is well-defined.
    assert!(
        golden.contains("aligned_alloc("),
        "FIX #3: the coherent backing buffer must be an aligned allocation (no char-array alignment UB)"
    );

    // The compiler-managed alloc_coherent/free_coherent lifecycle (T12) + the
    // device_addr ring-base program (A2) + the bus-master gate (T05).
    assert!(
        golden.contains("regseq_dma_alloc_coherent(\"tx_ring\""),
        "the compiler-managed alloc_coherent preamble must fire (T12)"
    );
    assert!(
        golden.contains("regseq_dma_free_coherent(\"tx_ring\")"),
        "the compiler-managed free_coherent postamble must fire (T12)"
    );
    assert!(
        golden.contains("regseq_dma_region_dev_base(\"tx_ring\")"),
        "device_addr() must program the ring base from the region's device address (A2)"
    );
    assert!(
        golden.contains("regseq_bus_set_master()"),
        "a bus-master coherent-region driver must set_bus_master (dma_present, T05)"
    );

    // ----------------------------------------------------------------------
    // RUNTIME (trace-ordering) oracle: the alloc/free lifecycle framing and the
    // publishing fence are observable at runtime. The DMA trace brackets the run:
    // `DMA alloc_coherent` before `DMA free_coherent`, with `DMA wmb` (the
    // publishing fence) in between. (This is a TRACE-ORDERING assertion, not a
    // backing-buffer byte dump — see the module note on claim strength, FIX #5.)
    // ----------------------------------------------------------------------
    let stdout =
        std::fs::read_to_string(dir.join("expected.stdout")).expect("read expected.stdout");
    let pos = |needle: &str| -> usize {
        stdout
            .find(needle)
            .unwrap_or_else(|| panic!("expected.stdout must contain {needle:?}; got:\n{stdout}"))
    };
    let alloc = pos("DMA alloc_coherent tx_ring");
    let wmb = pos("DMA wmb");
    let free = pos("DMA free_coherent tx_ring");
    assert!(
        alloc < wmb && wmb < free,
        "the DMA trace frames the publishing fence: alloc_coherent → wmb → free_coherent"
    );
    // The rmb is DESIGNED-NOT-EMITTED this cycle (D09): no completion-read fence.
    assert!(
        !stdout.contains("DMA rmb"),
        "regseq_dma_rmb must NOT fire this cycle (completion-read ordering is designed, not built — D09)"
    );
}
