//! Pin-controller-and-GPIO-configuration COMPOSED-interaction emission witness
//! (pinctrl construct, PLAN T24): the static per-target placement of the
//! `pinctrl-pl011-composed` driver (region + REQUIRED pinctrl + MANAGED clock +
//! interrupt), the linux_c / zephyr_c counterpart to the generic-c runtime oracle
//! (`pinctrl_composed_e2e`). One bridge, two emissions, asserting the C07
//! pins-before-clock preamble order across all four constructs:
//!
//!   linux_c  : devm_ioremap_resource -> devm_pinctrl_get -> devm_clk_get -> request_irq
//!   zephyr_c : DEVICE_MMIO_MAP -> pinctrl_apply_state(default) -> device_is_ready(clk) -> IRQ_CONNECT
//!
//! Teardown is NOT over-asserted: pinctrl is acquired via `devm_pinctrl_get`
//! (devm-automatic release, no explicit put) on Linux, and the Zephyr DT pin
//! config has no runtime release — the C07 reverse is realized structurally by the
//! framework device lifetime (the clock-witness precedent). SOLE test in this
//! binary (rustc interner isolation).

#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::{Path, PathBuf};

use codegen_coverage_runner::{manifest, pipeline};
use test_ir_fixtures::{try_bridge_fixture_path, BridgeOutcome};

fn pinctrl_corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/_pinctrl")
}

#[test]
fn pinctrl_composed_linux_zephyr_witness() {
    let fixtures = manifest::discover(&pinctrl_corpus()).expect("discover pinctrl corpus");
    let fixture = fixtures
        .iter()
        .find(|f| f.manifest.name == "pinctrl-pl011-composed")
        .expect("pinctrl-pl011-composed fixture discovered");

    let mut model = match try_bridge_fixture_path(&fixture.input) {
        BridgeOutcome::Ok(m) => m,
        other => panic!("bridge failed: {other:?}"),
    };
    pipeline::bind_l2_platform(&mut model, fixture).expect("L2 bind");

    // -- linux_c: map -> pinctrl acquire -> clock acquire -> interrupt request
    //    (C07/D08, pins-before-clock).
    let linux = regseq_backends::emit_linux_c(&model).expect("emit_linux_c must succeed");
    let ltu = linux.tu;
    let lpos = |needle: &str| {
        ltu.find(needle)
            .unwrap_or_else(|| panic!("linux TU must contain `{needle}`; got:\n{ltu}"))
    };
    lpos("#include <linux/pinctrl/consumer.h>");
    // The REQUIRED ZST `PinCtrl` becomes a `struct pinctrl *` member.
    lpos("struct pinctrl *pins;");
    let l_map = lpos("devm_ioremap_resource(&adev->dev, &adev->res)");
    let l_pin = lpos("priv->inner.pins = devm_pinctrl_get(&adev->dev);");
    let l_clk = lpos("devm_clk_get(&adev->dev, \"uartclk\")");
    let l_irq = lpos("request_irq(adev->irq[0]");
    assert!(
        l_map < l_pin && l_pin < l_clk && l_clk < l_irq,
        "linux preamble must order map_region -> devm_pinctrl_get -> devm_clk_get -> \
         request_irq (C07/D08); got:\n{ltu}"
    );
    // Required pinctrl is devm-acquired — release is devm-automatic (no explicit
    // put), so the C07 reverse is structural on unbind (the managed-clock A-2
    // precedent).
    assert!(
        !ltu.contains("pinctrl_put"),
        "the devm pinctrl release is devm-automatic — no explicit pinctrl_put; got:\n{ltu}"
    );

    // -- zephyr_c: region map -> pinctrl apply(default) -> clock readiness gate ->
    //    IRQ_CONNECT (C07/D08).
    let zephyr = regseq_backends::emit_zephyr_c(&model).expect("emit_zephyr_c must succeed");
    let ztu = zephyr.tu;
    let zpos = |needle: &str| {
        ztu.find(needle)
            .unwrap_or_else(|| panic!("zephyr TU must contain `{needle}`; got:\n{ztu}"))
    };
    zpos("#include <zephyr/drivers/pinctrl.h>");
    zpos("PINCTRL_DT_INST_DEFINE(0);");
    let z_map = zpos("DEVICE_MMIO_MAP(dev, K_MEM_CACHE_NONE);");
    // The init-hub acquire is the rc-checked `int __ret = pinctrl_apply_state(…)`
    // form (required), distinct from the author's `if (pinctrl_apply_state(…) != 0)`
    // body (emitted in a separate function, textually before the init hub).
    let z_pin = zpos(
        "int __ret = pinctrl_apply_state(PINCTRL_DT_INST_DEV_CONFIG_GET(0), PINCTRL_STATE_DEFAULT);",
    );
    let z_clk = zpos("device_is_ready(DEVICE_DT_GET(DT_INST_CLOCKS_CTLR_BY_IDX(0, 0)))");
    let z_irq = zpos("IRQ_CONNECT(DT_INST_IRQN(0)");
    assert!(
        z_map < z_pin && z_pin < z_clk && z_clk < z_irq,
        "zephyr preamble must order region-map -> pinctrl-apply(default) -> \
         clock-ready-gate -> IRQ_CONNECT (C07/D08); got:\n{ztu}"
    );
    // Codex M4 #6: the binding declares pinctrl support for a real Zephyr build.
    assert!(
        zephyr.binding_yaml.contains("pinctrl-device.yaml"),
        "the binding must include pinctrl-device.yaml when pinctrl is resolved; got:\n{}",
        zephyr.binding_yaml
    );
}
