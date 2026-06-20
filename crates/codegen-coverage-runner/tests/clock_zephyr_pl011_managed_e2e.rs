//! Peripheral-clock-management zephyr_c emission witness — MANAGED, SYNTHETIC
//! (clock construct, M3/T15/T16). A BACKEND-EMISSION witness, distinct from the
//! generic-c behavioral oracle. The real PL011-Zephyr-qemu target is STATIC, so
//! the managed Zephyr path is witnessed SYNTHETICALLY (D12): it string-checks
//! that the `clock-pl011-managed` spec lowers to the Zephyr clock_control API:
//!
//!   * the init hub gates on `device_is_ready(DEVICE_DT_GET(DT_INST_CLOCKS_CTLR_BY_IDX(0, 0)))`
//!     (a hard init error -> -ENODEV, I02), AFTER the region map (C07);
//!   * the author ops lower to `clock_control_on` / `clock_control_off` /
//!     `clock_control_get_rate` over the build-time DT clock node (no fallback,
//!     D10/R10); `<zephyr/drivers/clock_control.h>` is pulled in.
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
fn clock_zephyr_pl011_managed_witness() {
    let fixtures = manifest::discover(&clock_corpus()).expect("discover clock corpus");
    let fixture = fixtures
        .iter()
        .find(|f| f.manifest.name == "clock-pl011-managed")
        .expect("clock-pl011-managed fixture discovered");

    let mut model = match try_bridge_fixture_path(&fixture.input) {
        BridgeOutcome::Ok(m) => m,
        other => panic!("bridge failed: {other:?}"),
    };
    pipeline::bind_l2_platform(&mut model, fixture).expect("L2 bind");
    let emit = regseq_backends::emit_zephyr_c(&model).expect("emit_zephyr_c must succeed");
    let tu = emit.tu;
    let pos = |needle: &str| {
        tu.find(needle)
            .unwrap_or_else(|| panic!("TU must contain `{needle}`; got:\n{tu}"))
    };

    pos("#include <zephyr/drivers/clock_control.h>");

    // Acquire gate: device_is_ready on the DT clock controller, AFTER the region
    // map (DEVICE_MMIO_MAP), BEFORE on_init (C07/I02).
    let map = pos("DEVICE_MMIO_MAP(dev, K_MEM_CACHE_NONE);");
    let ready = pos("if (!device_is_ready(DEVICE_DT_GET(DT_INST_CLOCKS_CTLR_BY_IDX(0, 0))))");
    pos("return -ENODEV;");
    assert!(
        map < ready,
        "the clock readiness gate must follow the region map (C07/D12)"
    );

    // Author ops over the DT clock node (managed clock_control_*).
    pos("clock_control_on(DEVICE_DT_GET(DT_INST_CLOCKS_CTLR_BY_IDX(0, 0))");
    pos("clock_control_get_rate(DEVICE_DT_GET(DT_INST_CLOCKS_CTLR_BY_IDX(0, 0))");
    pos("clock_control_off(DEVICE_DT_GET(DT_INST_CLOCKS_CTLR_BY_IDX(0, 0))");

    // A `clock_control_on` failure PROPAGATES as Err (branched, not discarded);
    // a `clock_control_get_rate` failure FAILS LOUD (no error channel on u32).
    pos("if (clock_control_on(DEVICE_DT_GET(DT_INST_CLOCKS_CTLR_BY_IDX(0, 0))");
    pos("k_panic()");
    // The `?`-Result discriminant is DECODED tag→discriminant (Ok→0, Err→1).
    pos("? 0x0 : 0x1");

    // Managed get_rate is the raw live query, never a const-fold (D10/R10).
    assert!(
        !tu.contains("= (uint32_t)24000000u") && !tu.contains("= 24000000u"),
        "managed get_rate must be the raw clock_control_get_rate, not a fold; got:\n{tu}"
    );
}
