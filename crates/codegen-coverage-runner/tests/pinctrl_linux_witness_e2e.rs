//! Pin-controller-and-GPIO-configuration linux_c emission witnesses (pinctrl
//! construct, M4/T20/T22/T23). BACKEND-EMISSION witnesses, distinct from the
//! generic-c behavioral oracle (`pinctrl_*_e2e`): they string-check the Linux
//! pinctrl-consumer lowering of the `_pinctrl/*` fixtures.
//!
//!   * OPTIONAL present (trait path) — the handle-less PM helpers
//!     `pinctrl_pm_select_{default,sleep}_state(&adev->dev)` (dev recovered via
//!     `container_of`), rc ignored; no `struct pinctrl *` field; an auto-bind
//!     no-op acquire preamble (the kernel selects "default" pre-probe);
//!   * REQUIRED present — the `struct pinctrl *` field morph + the acquire
//!     preamble `devm_pinctrl_get`/`IS_ERR`/`PTR_ERR` + select default, slotted
//!     AFTER the region map (pins-before-clock, C07); the author `apply` lowers to
//!     `pinctrl_lookup_state` + `pinctrl_select_state`. The acquire hard-fails on
//!     `IS_ERR`/nonzero rc (T22, required-acquire-failure);
//!   * OPTIONAL absent — NO pinctrl API call, NO `<linux/pinctrl/consumer.h>`, the
//!     direct `Err(PinctrlAbsent)` fold -> `case ... return -ENODEV` (D06/C04/D12).
//!
//! Mirrors `clock_linux_pl011_managed_e2e`. The plain `if let Some` call path is
//! NOT lowered on the real backends (its switchInt/Downcast is the T15 generic-c
//! shape — design-in here); the optional witness uses the `OptionalPinCtrl` trait
//! path (`pinctrl-pl011-optional-bound-trait`).

#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::{Path, PathBuf};

use codegen_coverage_runner::{manifest, pipeline};
use test_ir_fixtures::{try_bridge_fixture_path, BridgeOutcome};

fn pinctrl_corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/_pinctrl")
}

fn linux_tu(fixture_name: &str) -> String {
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
    regseq_backends::emit_linux_c(&model)
        .expect("emit_linux_c must succeed")
        .tu
}

#[test]
fn linux_optional_present_pm_helpers() {
    let tu = linux_tu("pinctrl-pl011-optional-bound-trait");
    let pos = |needle: &str| {
        tu.find(needle)
            .unwrap_or_else(|| panic!("TU must contain `{needle}`; got:\n{tu}"))
    };

    pos("#include <linux/pinctrl/consumer.h>");
    // The OPTIONAL form uses the handle-less PM helpers; dev recovered per-site
    // via container_of (the config-space mirror). rc ignored (best-effort, C04).
    pos("struct pl011pin_priv *__rseq_priv = container_of(_1, struct pl011pin_priv, inner);");
    pos("(void)pinctrl_pm_select_sleep_state(&__rseq_priv->adev->dev);");
    pos("(void)pinctrl_pm_select_default_state(&__rseq_priv->adev->dev);");
    // The OPTIONAL acquire is an auto-bind no-op (the kernel selects "default"
    // pre-probe) — NO devm_pinctrl_get, NO struct pinctrl * field morph.
    assert!(
        !tu.contains("devm_pinctrl_get"),
        "optional form must not acquire a handle (auto-bind, C04/D10); got:\n{tu}"
    );
    assert!(
        !tu.contains("struct pinctrl *"),
        "optional form morphs no field (handle-less); got:\n{tu}"
    );
    // Pins-before-clock ordering (C07): the auto-bind no-op preamble follows the
    // region map (there is no clock in this fixture; the slot order is witnessed
    // by the required fixture's devm_pinctrl_get).
    let map = pos("devm_ioremap_resource(&adev->dev, &adev->res)");
    let acquire = pos("OPTIONAL pin controller");
    assert!(
        map < acquire,
        "pinctrl acquire must follow the region map (C07)"
    );
}

#[test]
fn linux_required_present_handle_path_and_acquire_failure() {
    let tu = linux_tu("pinctrl-pl011-required-bound");
    let pos = |needle: &str| {
        tu.find(needle)
            .unwrap_or_else(|| panic!("TU must contain `{needle}`; got:\n{tu}"))
    };

    pos("#include <linux/pinctrl/consumer.h>");
    // Field morph: the REQUIRED ZST `PinCtrl` becomes a `struct pinctrl *` member.
    pos("struct pinctrl *pins;");
    // Acquire preamble: devm_pinctrl_get + select default, AFTER the region map
    // (pins-before-clock, C07).
    let map = pos("devm_ioremap_resource(&adev->dev, &adev->res)");
    let acquire = pos("priv->inner.pins = devm_pinctrl_get(&adev->dev);");
    assert!(
        map < acquire,
        "pinctrl acquire must follow the region map (C07)"
    );
    // T22 — required-acquire-failure: the acquire hard-fails (a get ERR_PTR / a
    // lookup ERR_PTR / a nonzero select rc aborts probe before on_init, I02), the
    // native errno propagated (PTR_ERR preserves -EPROBE_DEFER; the select rc is
    // propagated, not flattened).
    pos("if (IS_ERR(priv->inner.pins))");
    pos("return PTR_ERR(priv->inner.pins);");
    pos("pinctrl_lookup_state(priv->inner.pins, \"default\")");
    pos("return __ret_pins;");
    // The author `apply::<S>()` lowers to explicit-handle lookup + select over the
    // morphed field.
    pos("pinctrl_lookup_state((*_1).pins, \"sleep\")");
    pos("pinctrl_select_state((*_1).pins, __ps)");
}

#[test]
fn linux_optional_absent_folds_to_enodev_no_api() {
    let tu = linux_tu("pinctrl-pl011-optional-absent");
    let pos = |needle: &str| {
        tu.find(needle)
            .unwrap_or_else(|| panic!("TU must contain `{needle}`; got:\n{tu}"))
    };

    // The absent fold: a direct Err(PinctrlAbsent) -> ENODEV, no API call, and an
    // absent-only TU is pinctrl-scaffold-free (I07/Codex M4 #5).
    pos("pinctrl optional-absent fold");
    pos("case 0x2:");
    pos("return -ENODEV;");
    assert!(
        !tu.contains("<linux/pinctrl/consumer.h>"),
        "absent-only TU must not pull the pinctrl consumer header; got:\n{tu}"
    );
    for forbidden in [
        "devm_pinctrl_get",
        "pinctrl_pm_select_default_state",
        "pinctrl_pm_select_sleep_state",
        "pinctrl_lookup_state",
        "pinctrl_select_state",
    ] {
        assert!(
            !tu.contains(forbidden),
            "absent fold must emit NO pinctrl API call (`{forbidden}`); got:\n{tu}"
        );
    }
}
