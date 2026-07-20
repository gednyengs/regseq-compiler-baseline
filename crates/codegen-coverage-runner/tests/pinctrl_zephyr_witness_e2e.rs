//! Pin-controller-and-GPIO-configuration zephyr_c emission witnesses (pinctrl
//! construct, M4/T21/T22/T23). BACKEND-EMISSION witnesses, SYNTHETIC like the
//! clock managed Zephyr witness (the real PL011-Zephyr-qemu target is static):
//! they string-check the Zephyr pinctrl lowering of the `_pinctrl/*` fixtures.
//!
//!   * OPTIONAL present (trait path) — `PINCTRL_DT_INST_DEFINE(0)` + the init-hub
//!     default apply + author `pinctrl_apply_state(PINCTRL_DT_INST_DEV_CONFIG_GET(0),
//!     PINCTRL_STATE_{DEFAULT,SLEEP})`, rc ignored; `pinctrl-device.yaml` in the
//!     binding;
//!   * REQUIRED present — same API, rc checked (T22 hard-fail, the native rc
//!     propagated);
//!   * OPTIONAL absent — NO pinctrl API call, NO `PINCTRL_DT_INST_DEFINE`, NO
//!     `<zephyr/drivers/pinctrl.h>`, NO `pinctrl-device.yaml`, the direct
//!     `Err(PinctrlAbsent)` fold -> `case ... return -ENODEV` (D06/C04/D12).
//!
//! Mirrors `clock_zephyr_pl011_managed_e2e`. The plain `if let Some` call path is
//! NOT lowered on the real backends (T15 generic-c shape — design-in); the
//! optional witness uses the `OptionalPinCtrl` trait path.

#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::{Path, PathBuf};

use codegen_coverage_runner::{manifest, pipeline};
use test_ir_fixtures::{try_bridge_fixture_path, BridgeOutcome};

fn pinctrl_corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/_pinctrl")
}

fn zephyr_emit(fixture_name: &str) -> regseq_backends::ZephyrEmit {
    let fixtures = manifest::discover(&pinctrl_corpus()).expect("discover pinctrl corpus");
    let fixture = fixtures
        .iter()
        .find(|f| f.manifest.name == fixture_name)
        .unwrap_or_else(|| panic!("{fixture_name} fixture discovered"));
    let mut model = match try_bridge_fixture_path(&fixture.input) {
        BridgeOutcome::Ok(m) => m,
        other => panic!("bridge failed: {other:?}"),
    };
    pipeline::bind_l2_platform(&mut model, fixture).expect("L2 bind");
    regseq_backends::emit_zephyr_c(&model).expect("emit_zephyr_c must succeed")
}

#[test]
fn zephyr_optional_present_apply_state() {
    let e = zephyr_emit("pinctrl-pl011-optional-bound-trait");
    let tu = e.tu;
    let pos = |needle: &str| {
        tu.find(needle)
            .unwrap_or_else(|| panic!("TU must contain `{needle}`; got:\n{tu}"))
    };

    pos("#include <zephyr/drivers/pinctrl.h>");
    pos("PINCTRL_DT_INST_DEFINE(0);");
    // Author applies via the DT pin config; the OPTIONAL form ignores the rc.
    pos("(void)pinctrl_apply_state(PINCTRL_DT_INST_DEV_CONFIG_GET(0), PINCTRL_STATE_SLEEP);");
    pos("(void)pinctrl_apply_state(PINCTRL_DT_INST_DEV_CONFIG_GET(0), PINCTRL_STATE_DEFAULT);");
    // Pins-before-clock ordering (C07): the init-hub default apply follows the
    // region map.
    let map = pos("DEVICE_MMIO_MAP(dev, K_MEM_CACHE_NONE);");
    let acquire = pos("apply the OPTIONAL default pin state");
    assert!(
        map < acquire,
        "the pinctrl apply must follow the region map (C07)"
    );
    // Codex M4 #6: the binding declares pinctrl support for a real Zephyr build.
    assert!(
        e.binding_yaml.contains("pinctrl-device.yaml"),
        "the binding must include pinctrl-device.yaml when pinctrl is resolved; got:\n{}",
        e.binding_yaml
    );
}

#[test]
fn zephyr_required_present_and_acquire_failure() {
    let e = zephyr_emit("pinctrl-pl011-required-bound");
    let tu = e.tu;
    let pos = |needle: &str| {
        tu.find(needle)
            .unwrap_or_else(|| panic!("TU must contain `{needle}`; got:\n{tu}"))
    };

    pos("PINCTRL_DT_INST_DEFINE(0);");
    // T22 — required-acquire-failure: the init-hub default apply checks the rc and
    // propagates it NATIVE (a hard init error, I02), BEFORE the clock gate /
    // on_init.
    let map = pos("DEVICE_MMIO_MAP(dev, K_MEM_CACHE_NONE);");
    let acquire =
        pos("int __ret = pinctrl_apply_state(PINCTRL_DT_INST_DEV_CONFIG_GET(0), PINCTRL_STATE_DEFAULT);");
    assert!(
        map < acquire,
        "the pinctrl apply must follow the region map (C07)"
    );
    pos("return __ret;");
    // The author `apply::<S>()` checks the rc (required hard-fails on failure).
    pos("if (pinctrl_apply_state(PINCTRL_DT_INST_DEV_CONFIG_GET(0), PINCTRL_STATE_SLEEP) != 0) {");
}

#[test]
fn zephyr_optional_absent_folds_to_enodev_no_api() {
    let e = zephyr_emit("pinctrl-pl011-optional-absent");
    let tu = e.tu;
    let pos = |needle: &str| {
        tu.find(needle)
            .unwrap_or_else(|| panic!("TU must contain `{needle}`; got:\n{tu}"))
    };

    pos("pinctrl optional-absent fold");
    pos("case 0x2:");
    pos("return -ENODEV;");
    assert!(
        !tu.contains("PINCTRL_DT_INST_DEFINE"),
        "absent-only TU must not define a DT pin config; got:\n{tu}"
    );
    assert!(
        !tu.contains("<zephyr/drivers/pinctrl.h>"),
        "absent-only TU must not pull the pinctrl header; got:\n{tu}"
    );
    assert!(
        !tu.contains("pinctrl_apply_state"),
        "absent fold must emit NO pinctrl API call; got:\n{tu}"
    );
    assert!(
        !e.binding_yaml.contains("pinctrl-device.yaml"),
        "an absent-only driver must not declare pinctrl in the binding; got:\n{}",
        e.binding_yaml
    );
}
