//! Pin-controller-and-GPIO-configuration TIER-2 VEHICLE — linux_c emission +
//! golden lock (eval QEMU behavioral ground truth). The `_pinctrl_tier2`
//! `pl011pin-optional` fixture (a plain-AMBA PL011 driver carrying an
//! optional-PRESENT pinctrl) lowered through `emit_linux_c` to a byte-locked
//! `golden.linux.c`, which the eval harness's `15-build-generated-module.sh`
//! compiles into a `.ko` and `41-smoke-gate-generated.sh` boots on QEMU.
//!
//! This test is the in-repo half (emission + the structural witness); the live
//! boot+bind+unbind is the eval-side Tier-2 campaign. `BLESS=1` (re)writes
//! `golden.linux.c`. SOLE test in this binary (interner isolation).

#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::{Path, PathBuf};

use codegen_coverage_runner::{manifest, pipeline};
use test_ir_fixtures::{try_bridge_fixture_path, BridgeOutcome};

fn tier2_corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/_pinctrl_tier2")
}

#[test]
fn linux_pinctrl_tier2_optional_snapshot() {
    let fixtures = manifest::discover(&tier2_corpus()).expect("discover the pinctrl-tier2 corpus");
    let fixture = fixtures
        .iter()
        .find(|f| f.manifest.name == "pl011pin-optional")
        .expect("pl011pin-optional fixture discovered");
    let golden = fixture
        .input
        .parent()
        .expect("fixture dir")
        .join("golden.linux.c");

    let mut model = match try_bridge_fixture_path(&fixture.input) {
        BridgeOutcome::Ok(m) => m,
        other => panic!("bridge failed: {other:?}"),
    };
    pipeline::bind_l2_platform(&mut model, fixture).expect("L2 bind");
    let tu = regseq_backends::emit_linux_c(&model)
        .expect("emit_linux_c must succeed")
        .tu;

    if std::env::var_os("BLESS").is_some() {
        std::fs::write(&golden, &tu).expect("write golden.linux.c");
        return;
    }

    let on_disk = std::fs::read_to_string(&golden)
        .expect("golden.linux.c present (run BLESS=1 once to emit it)");
    assert_eq!(
        on_disk, tu,
        "emit_linux_c drifted from the blessed golden.linux.c — re-bless if intended"
    );

    let pos = |needle: &str| {
        tu.find(needle)
            .unwrap_or_else(|| panic!("golden.linux.c must contain `{needle}`; got:\n{tu}"))
    };

    // -- it is an amba_driver that can bind the RESERVED test UART (periphid
    //    0x00abc123 / mask 0x00ffffff) — the harness reserves ttyAMA1 with that id.
    pos("static struct amba_driver");
    pos("0x00abc123");
    // -- the optional pinctrl lowers to the handle-less PM helper (best-effort,
    //    rc-ignored), with dev recovered via container_of — NO struct pinctrl *
    //    field morph, NO devm_pinctrl_get (the auto-bind path).
    pos("#include <linux/pinctrl/consumer.h>");
    pos("(void)pinctrl_pm_select_default_state(&__rseq_priv->adev->dev);");
    assert!(
        !tu.contains("devm_pinctrl_get"),
        "the optional form must not acquire a handle (auto-bind); got:\n{tu}"
    );
    assert!(
        !tu.contains("struct pinctrl *"),
        "the optional form morphs no field (handle-less); got:\n{tu}"
    );
    // -- the smoke-gate dmesg landmarks the harness greps (BRINGUP_LOG / DEINIT_LOG).
    pos("pl011pin up");
    pos("pl011pin deinit: masking interrupts");
}
