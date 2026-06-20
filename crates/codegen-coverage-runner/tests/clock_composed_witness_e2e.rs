//! Peripheral-clock-management COMPOSED-interaction emission witness (clock
//! construct, PLAN T17): the static per-target placement of the
//! `clock-pl011-composed` driver (region + MANAGED clock + interrupt), the
//! linux_c / zephyr_c counterpart to the generic-c runtime oracle
//! (`clock_composed_e2e`). One bridge, two emissions, asserting the C07 preamble
//! order across constructs:
//!
//!   linux_c  : devm_ioremap_resource  ->  devm_clk_get  ->  request_irq
//!   zephyr_c : DEVICE_MMIO_MAP  ->  device_is_ready(clock ctlr)  ->  IRQ_CONNECT
//!
//! SOLE test in this binary (rustc interner isolation).

#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::{Path, PathBuf};

use codegen_coverage_runner::{manifest, pipeline};
use test_ir_fixtures::{try_bridge_fixture_path, BridgeOutcome};

fn clock_corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/_clock")
}

#[test]
fn clock_composed_linux_zephyr_witness() {
    let fixtures = manifest::discover(&clock_corpus()).expect("discover clock corpus");
    let fixture = fixtures
        .iter()
        .find(|f| f.manifest.name == "clock-pl011-composed")
        .expect("clock-pl011-composed fixture discovered");

    let mut model = match try_bridge_fixture_path(&fixture.input) {
        BridgeOutcome::Ok(m) => m,
        other => panic!("bridge failed: {other:?}"),
    };
    pipeline::bind_l2_platform(&mut model, fixture).expect("L2 bind");

    // -- linux_c: map -> clock acquire -> interrupt request (C07/D08/D14).
    let linux = regseq_backends::emit_linux_c(&model).expect("emit_linux_c must succeed");
    let ltu = linux.tu;
    let lpos = |needle: &str| {
        ltu.find(needle)
            .unwrap_or_else(|| panic!("linux TU must contain `{needle}`; got:\n{ltu}"))
    };
    let l_map = lpos("devm_ioremap_resource(&adev->dev, &adev->res)");
    let l_clk = lpos("devm_clk_get(&adev->dev, \"uartclk\")");
    let l_irq = lpos("request_irq(adev->irq[0]");
    assert!(
        l_map < l_clk && l_clk < l_irq,
        "linux preamble must order map_region -> devm_clk_get -> request_irq (C07)"
    );
    // Teardown (target-specific): the interrupt deregisters EXPLICITLY in
    // `.remove` (`disable_irq` -> `free_irq`, the manual pair, no devm); the clock
    // release is devm-automatic (no explicit `clk_put`), so the C07 reverse is
    // realized structurally on unbind (A-2).
    let l_deinit = lpos("pl011composed_on_deinit(&priv->inner)");
    let l_disable = lpos("disable_irq(adev->irq[0]);");
    // `free_irq` also appears in the probe's on_init-Err unwind; the teardown one
    // is the LAST occurrence (in `.remove`), after the on_deinit call.
    let l_free = ltu
        .rfind("free_irq(adev->irq[0], priv);")
        .expect("the remove must free the irq");
    assert!(
        l_deinit < l_disable && l_disable < l_free,
        "linux teardown must order on_deinit -> disable_irq -> free_irq (D09)"
    );
    assert!(
        !ltu.contains("clk_put") && !ltu.contains("devm_clk_put"),
        "the managed clock release is devm-automatic — no explicit clk_put (A-2); got:\n{ltu}"
    );

    // -- zephyr_c: region map -> clock readiness gate -> IRQ_CONNECT (C07/D12).
    let zephyr = regseq_backends::emit_zephyr_c(&model).expect("emit_zephyr_c must succeed");
    let ztu = zephyr.tu;
    let zpos = |needle: &str| {
        ztu.find(needle)
            .unwrap_or_else(|| panic!("zephyr TU must contain `{needle}`; got:\n{ztu}"))
    };
    let z_map = zpos("DEVICE_MMIO_MAP(dev, K_MEM_CACHE_NONE);");
    let z_clk = zpos("device_is_ready(DEVICE_DT_GET(DT_INST_CLOCKS_CTLR_BY_IDX(0, 0)))");
    let z_irq = zpos("IRQ_CONNECT(DT_INST_IRQN(0)");
    assert!(
        z_map < z_clk && z_clk < z_irq,
        "zephyr preamble must order region-map -> clock-ready-gate -> IRQ_CONNECT (C07)"
    );
    // Teardown (structural): the interrupt masks at deinit (`irq_disable` — a
    // static IRQ_CONNECT has no runtime disconnect); the Zephyr clock_control API
    // has NO release primitive, so the clock release is STRUCTURALLY ABSENT — the
    // single `clock_control_off` is the author's `on_deinit` disable, never a
    // compiler-synthesized release (the C07 reverse is realized by the framework
    // device lifetime, not an explicit unwind).
    let z_deinit = zpos("pl011composed_on_deinit(&data->inner)");
    let z_irqoff = zpos("irq_disable(DT_INST_IRQN(0));");
    assert!(
        z_deinit < z_irqoff,
        "zephyr teardown must run on_deinit before the interrupt mask (irq_disable)"
    );
    assert_eq!(
        ztu.matches("clock_control_off").count(),
        1,
        "exactly one clock_control_off (the author on_deinit disable) — no compiler clock release on Zephyr; got:\n{ztu}"
    );
}
