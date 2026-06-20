//! The zephyr_c serial device-class gate (construct #7, PLAN T10/T11 / M3).
//!
//! The single-file `serial-pl011` fixture (THE M1/M2/M3 serial spec) through
//! the Zephyr row: bridge → L2 bind → `emit_zephyr_c` (serial lowering) →
//! byte-exact `golden.zephyr.c` + `golden.zephyr.binding.yaml`, plus the M3
//! STRUCTURE suite asserting the T10/T11 acceptance by name (R08): the
//! `uart_driver_api` vtable wiring, the per-source `irq_source_set` scatter at
//! its Zephyr WHEN sites (the irq_{tx,rx,err}_{enable,disable} slots) DISTINCT
//! from the line plumbing (IRQ_CONNECT), `irq_tx_complete` ← `tx_drain_query`
//! (C05/T09), `configure` surfacing `ConfigResult::Unsupported` as `-ENOTSUP`
//! (I03), the synthesized callback-dispatch ISR, and the `&<drv>_uart_api`
//! instantiation arg (the delivered class, not the deferred-class NULL).
//!
//! **Validation boundary (T13):** snapshot (emission) validated. The live
//! boot+IO Tier-2 campaign (qemu_cortex_a53, R05) rides the eval harness.
//!
//! `BLESS=1` writes `golden.zephyr.c` + `golden.zephyr.binding.yaml` +
//! `conformance.zephyr.json`. SOLE test in this binary (interner isolation,
//! the established sole-binary pattern).

#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::{Path, PathBuf};

use codegen_coverage_runner::{manifest, pipeline};
use test_ir_fixtures::{try_bridge_fixture_path, BridgeOutcome};

fn serial_corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/_serial")
}

#[test]
fn zephyr_serial_pl011_snapshot_and_m3_structure() {
    let fixtures = manifest::discover(&serial_corpus()).expect("discover serial corpus");
    let fixture = fixtures
        .iter()
        .find(|f| f.manifest.name == "serial-pl011")
        .expect("serial-pl011 fixture discovered")
        .with_binding(manifest::OsBinding::Zephyr)
        .expect("serial-pl011 resolves the zephyr binding (platform.zephyr.toml)");
    let fixture = &fixture;

    if std::env::var_os("BLESS").is_some() {
        let mut model = match try_bridge_fixture_path(&fixture.input) {
            BridgeOutcome::Ok(m) => m,
            other => panic!("bless bridge failed: {other:?}"),
        };
        pipeline::bind_l2_platform(&mut model, fixture).expect("L2 bind (bless)");
        let emit = regseq_backends::emit_zephyr_c(&model).expect("emit_zephyr_c must succeed");
        std::fs::write(pipeline::zephyr_golden_path(fixture), &emit.tu)
            .expect("write golden.zephyr.c");
        std::fs::write(pipeline::zephyr_binding_path(fixture), &emit.binding_yaml)
            .expect("write golden.zephyr.binding.yaml");
        std::fs::write(
            pipeline::zephyr_conformance_path(fixture),
            codegen_coverage_runner::conformance::render_json(&emit.conformance),
        )
        .expect("write conformance.zephyr.json");
        return;
    }

    // The Zephyr row, byte-locking BOTH artifacts.
    let result = pipeline::run_zephyr_fixture(fixture);
    assert!(
        result.is_green(),
        "serial-pl011 must pass bridge -> emit_zephyr_c -> snapshot: {:?}",
        result.verdict
    );

    let tu = std::fs::read_to_string(pipeline::zephyr_golden_path(fixture))
        .expect("read golden.zephyr.c");
    let pos = |needle: &str| {
        tu.find(needle)
            .unwrap_or_else(|| panic!("golden.zephyr.c must contain `{needle}`"))
    };

    // -- the uart subsystem is included and the api vtable wires the in-scope
    //    surface to the uart_driver_api slots.
    pos("#include <zephyr/drivers/uart.h>");
    let vtable = pos("static DEVICE_API(uart, serialpl011_uart_api) = {");
    pos(".poll_in\t= serialpl011_poll_in,");
    pos(".poll_out\t= serialpl011_poll_out,");
    pos(".configure\t= serialpl011_configure,");
    pos(".fifo_fill\t= serialpl011_fifo_fill,");
    pos(".fifo_read\t= serialpl011_fifo_read,");
    pos(".irq_tx_complete= serialpl011_irq_tx_complete,");
    pos(".irq_callback_set = serialpl011_irq_callback_set,");

    // -- per-source scatter: irq_tx_enable ARMS TxReady, irq_rx_enable ARMS
    //    RxData, irq_err_enable ARMS LineError — the construct contribution (the
    //    mask hoisted out of authored bodies into the uart_driver_api WHEN
    //    sites, R04/I01), via the author irq_source_set. Each slot DISTINCT.
    let irq_tx_enable = pos("static void serialpl011_irq_tx_enable(const struct device *dev)");
    let irq_tx_disable = pos("static void serialpl011_irq_tx_disable(const struct device *dev)");
    let irq_rx_enable = pos("static void serialpl011_irq_rx_enable(const struct device *dev)");
    let irq_err_enable = pos("static void serialpl011_irq_err_enable(const struct device *dev)");
    assert!(
        tu[irq_tx_enable..irq_tx_disable].contains("serialpl011_irq_source_set(&data->inner,"),
        "irq_tx_enable must scatter irq_source_set (arm TxReady)"
    );
    assert!(
        tu[irq_rx_enable..].contains("serialpl011_irq_source_set(&data->inner,"),
        "irq_rx_enable must scatter irq_source_set (arm RxData)"
    );
    assert!(
        tu[irq_err_enable..].contains("serialpl011_irq_source_set(&data->inner,"),
        "irq_err_enable must scatter irq_source_set (arm LineError)"
    );

    // -- the LINE plumbing (IRQ_CONNECT) is DISTINCT from the per-source arming
    //    (irq_source_set → IMSC): the init connects the synthesized serial ISR.
    let init = pos("static int serialpl011_init(const struct device *dev)");
    assert!(
        tu[init..].contains("IRQ_CONNECT(DT_INST_IRQN(0), DT_INST_IRQ(0, priority), serialpl011_isr,"),
        "init must IRQ_CONNECT the synthesized serial ISR (the LINE, distinct from the per-source IMSC)"
    );

    // -- irq_tx_complete ← tx_drain_query (C05 / T09 INCLUDE): reads FR.BUSY,
    //    not derivable from data_ready_query(TxSpace) = FR.TXFF.
    let tx_complete = pos("static int serialpl011_irq_tx_complete(const struct device *dev)");
    assert!(
        tu[tx_complete..].contains("serialpl011_tx_drain_query(&data->inner)"),
        "irq_tx_complete must lower to the author tx_drain_query (FR.BUSY), not a constant"
    );

    // -- irq_update ← irq_cause_decode (Zephyr's once-per-callback status read;
    //    the authored decode W1C-acks the line-error causes).
    let irq_update = pos("static int serialpl011_irq_update(const struct device *dev)");
    assert!(
        tu[irq_update..].contains("serialpl011_irq_cause_decode(&data->inner)"),
        "irq_update must read via the author irq_cause_decode (the error-ack home)"
    );

    // -- configure ← line_format_apply, surfacing ConfigResult::Unsupported as
    //    -ENOTSUP (I03 — not silently defaulted), in the §3.3.8 disable→drain→
    //    format→enable bracket (D17 adds the FR.BUSY drain-wait).
    let configure = pos("static int serialpl011_configure(const struct device *dev,");
    let apply = tu[configure..]
        .find("serialpl011_line_format_apply(&data->inner, lf);")
        .map(|o| configure + o)
        .expect("configure must call line_format_apply");
    assert!(
        tu[apply..].contains("return -ENOTSUP;"),
        "configure must SURFACE ConfigResult::Unsupported (-ENOTSUP), not silently default"
    );
    // D17/C03: disable → wait FR.BUSY clear (tx_drain_query) → line_format_apply,
    //    so an in-flight char is not corrupted (the FEN-clear edge is deferred —
    //    see the M2 record).
    let cfg_disable = tu[configure..apply]
        .find("serialpl011_port_service_set(&data->inner,")
        .map(|o| configure + o)
        .expect("configure must disable the port (§3.3.8 bracket)");
    let cfg_drain = tu[configure..apply]
        .find("while (!serialpl011_tx_drain_query(&data->inner))")
        .map(|o| configure + o)
        .expect("D17: configure must busy-wait FR.BUSY clear (tx_drain_query) before reprogram");
    assert!(
        cfg_disable < cfg_drain && cfg_drain < apply,
        "D17/C03: disable → wait FR.BUSY clear (tx_drain_query) → line_format_apply"
    );

    // -- the TX prime-kick: irq_tx_enable arms TxReady THEN calls the callback
    //    once (the level TX IRQ does not fire for an already-empty FIFO — the
    //    Linux M2 live-run construct finding, mirrored on Zephyr).
    assert!(
        tu[irq_tx_enable..irq_tx_disable].contains("data->cb(dev, data->cb_data);"),
        "irq_tx_enable must prime-kick the registered callback after arming TxReady"
    );
    // -- the prime is irq_lock-serialized against the now-armed TX IRQ (the live
    //    byte-IO double-send finding: without the lock the TX IRQ preempts the
    //    prime in thread context and re-enters the callback concurrently).
    assert!(
        tu[irq_tx_enable..irq_tx_disable].contains("irq_lock();")
            && tu[irq_tx_enable..irq_tx_disable].contains("irq_unlock("),
        "irq_tx_enable must serialize the prime under irq_lock (TX re-entrancy finding)"
    );

    // -- the compiler-synthesized serial ISR (B3): the upstream callback
    //    dispatch (no authored #[rseq::isr]).
    let isr = pos("static void serialpl011_isr(const struct device *dev)");
    assert!(
        tu[isr..].contains("data->cb(dev, data->cb_data);"),
        "the synthesized ISR must dispatch to the registered uart callback"
    );

    // -- the interrupt-driven plane is Kconfig-guarded (mirrors the upstream
    //    pl011 driver; the fifo/irq slots only exist under it).
    pos("#ifdef CONFIG_UART_INTERRUPT_DRIVEN");
    pos("#ifdef CONFIG_UART_USE_RUNTIME_CONFIGURE");
    // -- the data struct carries the registered-callback storage + the cached
    //    config (config_get has no HW readback, I03).
    let data_body = tu
        .split("struct serialpl011_data {")
        .nth(1)
        .unwrap()
        .split("};")
        .next()
        .unwrap();
    assert!(
        data_body.contains("uart_irq_callback_user_data_t cb;") && data_body.contains("void *cb_data;"),
        "the data struct must carry the registered uart callback"
    );
    assert!(
        data_body.contains("struct uart_config uart_cfg;"),
        "the data struct must cache uart_config for config_get"
    );

    // -- the instantiation: the api arg is the DELIVERED class (&uart_api), NOT
    //    the deferred-class NULL; the on_deinit form swaps to DEINIT_DEFINE.
    let define = pos("DEVICE_DT_INST_DEINIT_DEFINE(n, serialpl011_init, serialpl011_deinit, NULL,");
    assert!(
        tu[define..].contains("&serialpl011_uart_api);"),
        "the instantiation must install the delivered uart_driver_api (not NULL)"
    );
    assert!(
        !tu.contains("api: ClassTag::Generic only (A-16)"),
        "a delivered serial class must NOT carry the deferred-class NULL-api comment"
    );
    assert!(vtable < define, "the vtable is defined before the instantiation");

    // -- C02 edge 4 (T13): in the deinit scaffold the authored on_deinit quiesce
    //    (mask-all) is ORDERED BEFORE the port disable — the compiler-owned half of
    //    Zephyr's teardown disarm-before-disable (the authored mask content is the
    //    spec body). This is the structural backing for the schedule's TeardownQuiesce.
    let deinit = pos("static int serialpl011_deinit(const struct device *dev)");
    let deinit_quiesce = tu[deinit..]
        .find("serialpl011_on_deinit(&data->inner)")
        .map(|o| deinit + o)
        .expect("deinit must run the authored on_deinit quiesce");
    let deinit_disable = tu[deinit..]
        .find("serialpl011_port_service_set(&data->inner,")
        .map(|o| deinit + o)
        .expect("deinit must disable the port");
    assert!(
        deinit_quiesce < deinit_disable,
        "C02 edge 4: on_deinit (mask-all quiesce) must precede the port disable in deinit"
    );

    // -- the single-instance guard + the A-1/D11 cross-check (DUT at 0x9040000).
    pos("BUILD_ASSERT(DT_NUM_INST_STATUS_OKAY(DT_DRV_COMPAT) == 1");
    pos("BUILD_ASSERT(DT_REG_ADDR(DT_DRV_INST(0)) == 0x9040000 &&");

    // -- no host/runner artifacts (S16/R02): the device model is the OS.
    assert!(!tu.contains("int main("), "no main in a Zephyr driver TU");
    assert!(!tu.contains("printf("), "no host printf");

    // -- the binding yaml (D07): the same compatible the TU pins.
    let yaml = std::fs::read_to_string(pipeline::zephyr_binding_path(fixture))
        .expect("read golden.zephyr.binding.yaml");
    assert!(yaml.contains("compatible: \"regseq,serialpl011\""), "{yaml}");

    // -- conformance lists the delivered uart slots + the per-source arming sites.
    let conformance = std::fs::read_to_string(pipeline::zephyr_conformance_path(fixture))
        .expect("read conformance.zephyr.json");
    assert!(
        conformance.contains("uart_driver_api.irq_tx_complete")
            && conformance.contains("irq_source_set@irq_tx_enable"),
        "the conformance report must list the delivered uart slots + arming sites"
    );

    // -- A-10 negative leg: stripping the L2 binding turns the SAME serial model
    //    into the hard unresolved-binding diagnostic (the binding gate precedes
    //    the serial dispatch).
    let unbound = match try_bridge_fixture_path(&fixture.input) {
        BridgeOutcome::Ok(m) => m,
        other => panic!("bridge failed: {other:?}"),
    };
    let err = regseq_backends::emit_zephyr_c(&unbound).expect_err("unresolved binding must fail");
    assert!(
        err.to_string().contains("A-10"),
        "the A-10 diagnostic must name the missing L2 literals: {err}"
    );
}
