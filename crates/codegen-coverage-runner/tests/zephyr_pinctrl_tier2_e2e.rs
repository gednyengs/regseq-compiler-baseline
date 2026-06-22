//! Pin-controller-and-GPIO-configuration TIER-2 VEHICLE — zephyr_c emission +
//! golden lock (eval QEMU behavioral ground truth, the cross-OS Zephyr leg). The
//! SAME `_pinctrl_tier2` `pl011pin-optional` fixture the linux leg uses (a plain
//! `#[rseq::driver]` PL011 carrying an optional-PRESENT pinctrl — plain, so it
//! dodges the serial fast-path's resolved-pinctrl reject) lowered through
//! `emit_zephyr_c` to a byte-locked `golden.zephyr.c` + `golden.zephyr.binding.yaml`,
//! which the eval harness's `15-build-pinctrl-image.sh` compiles into a Zephyr image
//! and `40-smoke-gate-pinctrl.sh` boots on QEMU.
//!
//! Unlike the linux optional form (a handle-less `pinctrl_pm_select_default_state`
//! runtime no-op that boots inert on a target with NO pin controller), Zephyr pin
//! control is build-time DT-authoritative: `PINCTRL_DT_INST_DEFINE(0)` +
//! `pinctrl_apply_state(PINCTRL_DT_INST_DEV_CONFIG_GET(0), PINCTRL_STATE_DEFAULT)`
//! reference compile-time DT pin config, so the image only builds against a real DT
//! pin controller. The eval leg supplies Zephyr's own `vnd,pinctrl-test` controller,
//! so `pinctrl_apply_state` ACTUALLY EXECUTES at boot (a live witness, not inert).
//!
//! This test is the in-repo half (emission + the structural witness); the live
//! boot+apply is the eval-side Tier-2 campaign. `BLESS=1` (re)writes both goldens.
//! SOLE test in this binary (interner isolation).

#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::{Path, PathBuf};

use codegen_coverage_runner::{manifest, pipeline};
use test_ir_fixtures::{try_bridge_fixture_path, BridgeOutcome};

fn tier2_corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/_pinctrl_tier2")
}

#[test]
fn zephyr_pinctrl_tier2_optional_snapshot() {
    let fixtures = manifest::discover(&tier2_corpus()).expect("discover the pinctrl-tier2 corpus");
    let fixture = fixtures
        .iter()
        .find(|f| f.manifest.name == "pl011pin-optional")
        .expect("pl011pin-optional fixture discovered");
    let dir = fixture.input.parent().expect("fixture dir");
    let golden_c = dir.join("golden.zephyr.c");
    let golden_binding = dir.join("golden.zephyr.binding.yaml");

    let mut model = match try_bridge_fixture_path(&fixture.input) {
        BridgeOutcome::Ok(m) => m,
        other => panic!("bridge failed: {other:?}"),
    };
    pipeline::bind_l2_platform(&mut model, fixture).expect("L2 bind");
    let emitted = regseq_backends::emit_zephyr_c(&model).expect("emit_zephyr_c must succeed");
    let tu = emitted.tu;
    let binding = emitted.binding_yaml;

    if std::env::var_os("BLESS").is_some() {
        std::fs::write(&golden_c, &tu).expect("write golden.zephyr.c");
        std::fs::write(&golden_binding, &binding).expect("write golden.zephyr.binding.yaml");
        return;
    }

    let on_disk_c = std::fs::read_to_string(&golden_c)
        .expect("golden.zephyr.c present (run BLESS=1 once to emit it)");
    assert_eq!(
        on_disk_c, tu,
        "emit_zephyr_c drifted from the blessed golden.zephyr.c — re-bless if intended"
    );
    let on_disk_binding = std::fs::read_to_string(&golden_binding)
        .expect("golden.zephyr.binding.yaml present (run BLESS=1 once to emit it)");
    assert_eq!(
        on_disk_binding, binding,
        "emit_zephyr_c drifted from the blessed golden.zephyr.binding.yaml — re-bless if intended"
    );

    let pos = |needle: &str| {
        tu.find(needle)
            .unwrap_or_else(|| panic!("golden.zephyr.c must contain `{needle}`; got:\n{tu}"))
    };

    // -- the build-time DT pin control scaffolding: the include, the per-instance
    //    define, and the OPTIONAL (rc-ignored) default-state apply over the DT pin
    //    config. Zephyr's optional form shares the required API (differs only in rc
    //    handling) — distinct from linux's handle-less PM helper.
    pos("#include <zephyr/drivers/pinctrl.h>");
    pos("PINCTRL_DT_INST_DEFINE(0);");
    pos("(void)pinctrl_apply_state(PINCTRL_DT_INST_DEV_CONFIG_GET(0), PINCTRL_STATE_DEFAULT);");
    // The default state is applied TWICE: the framework init-hub preamble (C07,
    // pins-before-clock) + the authored on_init body (`self.pinctrl.apply::<DefaultState>()`).
    // The byte-lock already pins the TU; this count makes the double-apply invariant
    // explicit and surfaces an accidental re-bless that drops/duplicates an apply
    // (Codex diff NIT 4). The live campaign witnesses both applies (`RSEQ-PINCTRL` ×2).
    assert_eq!(
        tu.matches("pinctrl_apply_state(").count(),
        2,
        "expected exactly two pinctrl_apply_state applies (hub preamble + authored body); got:\n{tu}"
    );
    // -- the region is mapped (the vehicle writes imsc/icr); no IRQ in this vehicle.
    pos("DEVICE_MMIO_MAP(dev, K_MEM_CACHE_NONE);");
    assert!(
        !tu.contains("IRQ_CONNECT"),
        "the pinctrl tier-2 vehicle declares no interrupt; got:\n{tu}"
    );
    // -- the smoke-gate console landmarks the harness greps.
    pos("pl011pin up");
    pos("pl011pin deinit: masking interrupts");

    // -- the binding declares pinctrl support for a real Zephyr build (Codex M4 #6).
    assert!(
        binding.contains("pinctrl-device.yaml"),
        "the binding must include pinctrl-device.yaml when pinctrl is resolved; got:\n{binding}"
    );
}
