//! The serial device-class M1 vertical-slice gate (construct #7, PLAN T06/T07):
//! the `serial-pl011` fixture passes the six-stage dual oracle in a DEDICATED
//! clean process (the emit's MIR is sensitive to rustc's process-global interner
//! history). The blessed `expected.stdout` is the serial PLACEMENT CONTRACT made
//! observable (R09): the bring-up order (UART disabled while line-formatting,
//! then enabled), each per-source mask arm via `irq_source_set` → IMSC as a
//! `regseq_mmio_write` DISTINCT from the line-level `regseq_irq_enable`, the
//! interrupt-driven TX (arm TxReady → write → fire → synthesized-ISR decode +
//! disarm) and RX (poke + fire → ISR decode + read) legs, and teardown.
//!
//! `BLESS=1` writes `golden.c` + `expected.stdout` from a single clean
//! bind+emit+run, then returns without verifying (the keystone pattern).
//!
//! SOLE test in this binary (like `irq_pl011_e2e` / `lifecycle_pl011_e2e`).

#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::{Path, PathBuf};

use codegen_coverage_runner::{manifest, pipeline, toolchain};
use regseq_model::TargetOs;
use test_ir_fixtures::{try_bridge_fixture_path, BridgeOutcome};

fn cc() -> &'static Path {
    Path::new("cc")
}

fn serial_corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/_serial")
}

#[test]
fn serial_pl011_six_stage_oracle() {
    let dir = serial_corpus().join("serial-pl011");
    let fixtures = manifest::discover(&serial_corpus()).expect("discover serial corpus");
    let fixture = fixtures
        .iter()
        .find(|f| f.manifest.name == "serial-pl011")
        .expect("serial-pl011 fixture discovered")
        .with_binding(manifest::OsBinding::Linux)
        .expect("serial-pl011 resolves the linux binding (C05 layout)");
    let fixture = &fixture;

    assert!(
        fixture.platform.is_some(),
        "serial-pl011 must resolve platform.linux.toml (the mmio + interrupt joins)"
    );

    if std::env::var_os("BLESS").is_some() {
        let options = pipeline::backend_options_for(fixture).expect("backend options");
        let mut model = match try_bridge_fixture_path(&fixture.input) {
            BridgeOutcome::Ok(m) => m,
            other => panic!("bless bridge failed: {other:?}"),
        };
        let identity = pipeline::bind_l2_platform(&mut model, fixture).expect("L2 bind (bless)");
        assert_eq!(identity.os, Some(TargetOs::Linux));
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

    // Stages 1-6 (the process's first, interner-clean, bridge).
    let result = pipeline::run_fixture(fixture, cc());
    assert!(
        result.is_green(),
        "serial-pl011 must pass all six stages: {:?}",
        result.verdict
    );

    // ---- STRUCTURAL gates on the committed golden (R09 / Codex F1) ----
    let golden = std::fs::read_to_string(dir.join("golden.c")).expect("read golden.c");
    assert!(
        golden.contains("static void rseq_serial_isr_serialpl011(void *__ctx)"),
        "golden.c must define the compiler-SYNTHESIZED serial ISR wrapper (no authored #[rseq::isr])"
    );
    assert!(
        golden.contains("input__impl_4__irq_cause_decode(&__t->inner)"),
        "the synthesized ISR must call irq_cause_decode"
    );
    assert!(
        golden.contains("if ((__c.bits & 6u) != 0u) { input__impl_2__data_unit_read(&__t->inner); }"),
        "the synthesized ISR must DISPATCH rx_data OR rx_timeout (bits 1|2) → data_unit_read"
    );
    assert!(
        golden.contains("if ((__c.bits & 1u) != 0u)"),
        "the synthesized ISR must DISPATCH tx_ready"
    );
    assert!(
        golden.contains("regseq_irq_register(37u, rseq_serial_isr_serialpl011, (void *)&inst);"),
        "the line must register the synthesized serial ISR (B3: serial-class handler is synthesized)"
    );

    // ---- BEHAVIORAL gates: the serial placement contract made observable ----
    let stdout =
        std::fs::read_to_string(dir.join("expected.stdout")).expect("read expected.stdout");
    let pos = |needle: &str| -> usize {
        stdout
            .find(needle)
            .unwrap_or_else(|| panic!("expected.stdout must contain {needle:?}; got:\n{stdout}"))
    };

    // (1) PL011 §3.3.8: the line format (LCR_H/IBRD/FBRD) is reprogrammed while
    //     the UART is DISABLED — CR-disable < LCR_H write < CR-enable.
    let cr_disable = pos("W32 @0x30 = 0x00000000");
    let lcr_h = pos("W32 @0x2c = 0x00000070"); // 8N1 + FEN
    let cr_enable = pos("W32 @0x30 = 0x00000301"); // UARTEN+TXE+RXE
    assert!(
        cr_disable < lcr_h && lcr_h < cr_enable,
        "line-format must be programmed while the UART is disabled (PL011 §3.3.8)"
    );

    // (1b) D17/C03: BEFORE reprogramming, the configure scaffold WAITS for the
    //      current transfer to drain (FR.BUSY clear, via the author tx_drain_query)
    //      — an FR read between the CR-disable and the divisor/LCR_H writes, so an
    //      in-flight character is not corrupted. (The FEN-clear/FIFO-flush edge is
    //      deferred: no register-role metadata, no in-scope FIFO op — see M2 record.)
    let drain_poll = pos("R32 @0x18 = 0x00000000");
    assert!(
        cr_disable < drain_poll && drain_poll < lcr_h,
        "D17: the FR.BUSY drain poll (tx_drain_query) precedes the line-format reprogram"
    );

    // (2) The per-source interrupt arming (irq_source_set → IMSC) is a
    //     regseq_mmio_write DISTINCT from the line-level regseq_irq_enable
    //     (Codex F1 / I01): the line enable precedes the per-source arms, and the
    //     TX-source arm is its own IMSC write (0x7f0), not the line enable.
    let line_enable = pos("IRQ enable line=37");
    let rx_arm = pos("W32 @0x38 = 0x00000050"); // RXIM|RTIM
    let tx_arm = pos("W32 @0x38 = 0x000007f0"); // +TXIM (per-source TX arm)
    assert!(
        line_enable < rx_arm && rx_arm < tx_arm,
        "the line enable must precede the per-source IMSC arms (distinct ops, I01)"
    );

    // (2b) C02 edge 3 (clear-pending-before-unmask, T13/G-E1): the W1C ICR clear
    //      (in on_init) precedes every per-source IMSC arm — a stale latched cause
    //      must not be able to fire the instant a source unmasks (the behavioral
    //      confirmation of the verify_c02 source-ordering gate).
    let icr_clear = pos("W32 @0x44 = 0x000007ff");
    assert!(
        icr_clear < rx_arm && icr_clear < tx_arm,
        "C02 edge 3: ICR-clear (on_init) must precede the per-source IMSC arms"
    );

    // (3) The synthesized ISR DECODES (reads MIS) on each fire and dispatches:
    //     TX fire → decode 0x20 → disarm TxReady; RX fire → decode 0x10 → read DR.
    let tx_poke = pos("IRQ poke @0x040 = 0x00000020");
    let tx_decode = pos("R32 @0x40 = 0x00000020");
    let rx_poke = pos("IRQ poke @0x040 = 0x00000010");
    let rx_decode = pos("R32 @0x40 = 0x00000010");
    let rx_read = pos("R32 @0x0 = 0x00000055");
    assert!(
        tx_arm < tx_poke && tx_poke < tx_decode,
        "TX: arm TxReady → fire → decode"
    );
    assert!(
        tx_decode < rx_poke && rx_poke < rx_decode && rx_decode < rx_read,
        "RX: (after TX serviced) poke → decode → data_unit_read the byte"
    );

    // (3b) RX-timeout couples into the RX source: an RTMIS fire decodes (0x40) and
    //      is drained on the SAME RX path (Codex M1 finding 1).
    let rt_poke = pos("IRQ poke @0x040 = 0x00000040");
    let rt_decode = pos("R32 @0x40 = 0x00000040");
    let rt_read = pos("R32 @0x0 = 0x00000056");
    assert!(
        rx_read < rt_poke && rt_poke < rt_decode && rt_decode < rt_read,
        "RX-timeout: poke RTMIS → decode → data_unit_read (the timeout is serviced, not dropped)"
    );

    // (4) Teardown: disarm the sources, then on_deinit, then the line
    //     disable→unregister (D09/R06).
    let rx_disarm = pos("W32 @0x38 = 0x00000780"); // RxData disarmed
    let on_deinit = pos("LC call on_deinit");
    let disable = pos("IRQ disable line=37");
    let unregister = pos("IRQ unregister line=37");
    assert!(
        rt_read < rx_disarm && rx_disarm < on_deinit,
        "sources are disarmed at teardown (after the last IO) before on_deinit"
    );
    assert!(
        on_deinit < disable && disable < unregister,
        "teardown runs on_deinit → line disable → unregister (D09)"
    );
}
