//! The linux_c serial device-class gate (construct #7, PLAN T08 / M2).
//!
//! The single-file `serial-pl011` fixture (THE M1/M2 serial spec) through the
//! Linux row: bridge → L2 bind → `emit_linux_c` (serial lowering) → byte-exact
//! `golden.linux.c`, plus the M2 STRUCTURE suite asserting the T08 acceptance by
//! name (R08): the `uart_ops` vtable wiring, the per-source `irq_source_set`
//! scatter at its Linux WHEN sites DISTINCT from the line plumbing (request_irq),
//! `tx_empty` ← `tx_drain_query` (C05/T09), `set_termios` surfacing
//! `ConfigResult::Unsupported` (I03), and the synthesized serial ISR.
//!
//! **Validation boundary (T13):** snapshot (emission) validated. The live
//! boot+IO Tier-2 campaign (amba-pl011 QEMU, R05) is gated on the T13 toolchain
//! and is not run in-repo.
//!
//! `BLESS=1` writes `golden.linux.c` + `conformance.linux.json`. SOLE test in
//! this binary (interner isolation, the established sole-binary pattern).

#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::{Path, PathBuf};

use codegen_coverage_runner::{manifest, pipeline};
use test_ir_fixtures::{try_bridge_fixture_path, BridgeOutcome};

fn serial_corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/_serial")
}

#[test]
fn linux_serial_pl011_snapshot_and_m2_structure() {
    let fixtures = manifest::discover(&serial_corpus()).expect("discover serial corpus");
    let fixture = fixtures
        .iter()
        .find(|f| f.manifest.name == "serial-pl011")
        .expect("serial-pl011 fixture discovered")
        .with_binding(manifest::OsBinding::Linux)
        .expect("serial-pl011 resolves the linux binding (platform.linux.toml)");
    let fixture = &fixture;

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

    // The Linux row, byte-locking the snapshot.
    let result = pipeline::run_linux_fixture(fixture);
    assert!(
        result.is_green(),
        "serial-pl011 must pass bridge -> emit_linux_c -> snapshot: {:?}",
        result.verdict
    );

    let tu = std::fs::read_to_string(pipeline::linux_golden_path(fixture))
        .expect("read golden.linux.c");
    let pos = |needle: &str| {
        tu.find(needle)
            .unwrap_or_else(|| panic!("golden.linux.c must contain `{needle}`"))
    };

    // -- the uart_ops vtable wires the in-scope surface to the kernel slots.
    let vtable = pos("static const struct uart_ops serialpl011_uart_ops = {");
    pos(".tx_empty\t= serialpl011_tx_empty,");
    pos(".start_tx\t= serialpl011_start_tx,");
    pos(".stop_tx\t= serialpl011_stop_tx,");
    pos(".stop_rx\t= serialpl011_stop_rx,");
    pos(".startup\t= serialpl011_startup,");
    pos(".shutdown\t= serialpl011_shutdown,");
    pos(".set_termios\t= serialpl011_set_termios,");

    // -- per-source scatter: start_tx ARMS TxReady, stop_tx DISARMS it — the
    //    construct contribution (the mask hoisted out of authored bodies into the
    //    uart_ops WHEN sites, R04/I01), via the author irq_source_set.
    let start_tx = pos("static void serialpl011_start_tx(struct uart_port *port)");
    let stop_tx = pos("static void serialpl011_stop_tx(struct uart_port *port)");
    let stop_rx = pos("static void serialpl011_stop_rx(struct uart_port *port)");
    assert!(
        tu[start_tx..stop_tx].contains("serialpl011_irq_source_set(&priv->inner,"),
        "start_tx must scatter irq_source_set (arm TxReady)"
    );
    assert!(
        tu[stop_tx..stop_rx].contains("serialpl011_irq_source_set(&priv->inner,"),
        "stop_tx must scatter irq_source_set (disarm TxReady)"
    );

    // -- startup: the LINE plumbing (request_irq) is DISTINCT from the per-source
    //    arming (irq_source_set → IMSC); the line request precedes the source arms
    //    (Codex F1 / I01 — a line enable alone must NOT satisfy per-source arming).
    let startup = pos("static int serialpl011_startup(struct uart_port *port)");
    let request_irq = pos("request_irq(port->irq, serialpl011_isr, 0, \"serialpl011\", priv);");
    let startup_arm = tu[startup..]
        .find("serialpl011_irq_source_set(&priv->inner,")
        .map(|o| startup + o)
        .expect("startup must arm a source");
    assert!(
        startup < request_irq && request_irq < startup_arm,
        "startup: request_irq (the LINE) precedes the per-source IMSC arms (distinct ops)"
    );

    // -- tx_empty ← tx_drain_query (C05 / T09 INCLUDE): serial_core derefs this
    //    unconditionally; no constant stub.
    let tx_empty = pos("static unsigned int serialpl011_tx_empty(struct uart_port *port)");
    assert!(
        tu[tx_empty..].contains("serialpl011_tx_drain_query(&priv->inner) ? TIOCSER_TEMT : 0;"),
        "tx_empty must lower to the author tx_drain_query (FR.BUSY), not a constant"
    );

    // -- set_termios ← line_format_apply, surfacing ConfigResult::Unsupported
    //    (I03 — not silently defaulted) by reverting to the prior termios.
    let set_termios = pos("static void serialpl011_set_termios(struct uart_port *port,");
    let apply = tu[set_termios..]
        .find("serialpl011_line_format_apply(&priv->inner, lf);")
        .map(|o| set_termios + o)
        .expect("set_termios must call line_format_apply");
    assert!(
        tu[apply..].contains("*termios = *old;"),
        "set_termios must SURFACE ConfigResult::Unsupported (revert termios), not silently default"
    );

    // -- the compiler-synthesized serial ISR (B3): decode + dispatch.
    let isr = pos("static irqreturn_t serialpl011_isr(int irq, void *dev_id)");
    assert!(
        tu[isr..].contains("serialpl011_irq_cause_decode(&priv->inner)"),
        "the synthesized ISR must decode via irq_cause_decode"
    );
    assert!(
        tu[isr..].contains("serialpl011_data_unit_read(&priv->inner)"),
        "the ISR RX path must drain via data_unit_read"
    );
    assert!(
        tu[isr..].contains("serialpl011_tx_pump(priv)"),
        "the ISR TX path must refill via the shared tx_pump (Codex M1 F2: real-OS backend refills)"
    );

    // -- the TX pump refills via the author data path, and start_tx KICKS it
    //    (not arm-only — the level-triggered PL011 TX IRQ needs a primed FIFO;
    //    verified against a live QEMU run). The pump is emitted before the ISR.
    let pump = pos("static void serialpl011_tx_pump(struct serialpl011_priv *priv)");
    assert!(
        tu[pump..isr].contains("serialpl011_data_unit_write(&priv->inner,")
            && tu[pump..isr].contains("uart_xmit_advance(&priv->port, 1)"),
        "tx_pump must refill the FIFO via data_unit_write + uart_xmit_advance"
    );
    let start_tx2 = pos("static void serialpl011_start_tx(struct uart_port *port)");
    assert!(
        tu[start_tx2..].contains("serialpl011_tx_pump(priv);"),
        "start_tx must KICK the pump after arming TxReady (not arm-only)"
    );

    // -- serial-core registration: uart_register_driver before any add_one_port.
    let init = pos("static int __init serialpl011_init(void)");
    let reg = pos("uart_register_driver(&serialpl011_uart_driver);");
    let add = pos("uart_add_one_port(&serialpl011_uart_driver, &priv->port);");
    assert!(init < reg, "module init registers the uart_driver");
    assert!(vtable < add, "the port is added with the uart_ops vtable");

    // -- conformance lists the per-source arming sites (the construct delivery).
    let conformance = std::fs::read_to_string(pipeline::linux_conformance_path(fixture))
        .expect("read conformance.linux.json");
    assert!(
        conformance.contains("uart_ops.tx_empty")
            && conformance.contains("irq_source_set@start_tx"),
        "the conformance report must list the delivered uart_ops slots + arming sites"
    );

    // -- A-10 negative leg: stripping the L2 binding turns the SAME serial model
    //    into the hard unresolved-binding diagnostic.
    let unbound = match try_bridge_fixture_path(&fixture.input) {
        BridgeOutcome::Ok(m) => m,
        other => panic!("bridge failed: {other:?}"),
    };
    let err = regseq_backends::emit_linux_c(&unbound).expect_err("unresolved binding must fail");
    assert!(
        err.to_string().contains("A-10"),
        "the A-10 diagnostic must name the missing L2 literals: {err}"
    );
}
