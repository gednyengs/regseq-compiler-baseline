//! Peripheral-clock-management zephyr_c emission witness — STATIC (clock
//! construct, M3/T15/T16). A BACKEND-EMISSION witness: it string-checks that the
//! Zephyr lowering of the `clock-pl011-static` fixture folds the static clock
//! away entirely (D07) — the real PL011-Zephyr-qemu target IS static:
//!
//!   * NO `clock_control_*` ops and NO `device_is_ready` acquire gate;
//!   * NO `<zephyr/drivers/clock_control.h>`;
//!   * `get_rate` const-folds to the resolved `default_frequency` (48 MHz).
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
fn clock_zephyr_pl011_static_witness() {
    let fixtures = manifest::discover(&clock_corpus()).expect("discover clock corpus");
    let fixture = fixtures
        .iter()
        .find(|f| f.manifest.name == "clock-pl011-static")
        .expect("clock-pl011-static fixture discovered");

    let mut model = match try_bridge_fixture_path(&fixture.input) {
        BridgeOutcome::Ok(m) => m,
        other => panic!("bridge failed: {other:?}"),
    };
    pipeline::bind_l2_platform(&mut model, fixture).expect("L2 bind");
    let emit = regseq_backends::emit_zephyr_c(&model).expect("emit_zephyr_c must succeed");
    let tu = emit.tu;

    // The region map still happens (the clock is orthogonal to MMIO).
    assert!(
        tu.contains("DEVICE_MMIO_MAP(dev, K_MEM_CACHE_NONE);"),
        "the MMIO region preamble is unaffected by a static clock; got:\n{tu}"
    );

    // A static clock leaves NO clock_control trace: no acquire gate, no ops.
    for needle in [
        "<zephyr/drivers/clock_control.h>",
        "clock_control_on",
        "clock_control_off",
        "clock_control_get_rate",
        "device_is_ready(DEVICE_DT_GET(DT_INST_CLOCKS_CTLR",
    ] {
        assert!(
            !tu.contains(needle),
            "static clock must emit no `{needle}` (D07 backend const-fold); got:\n{tu}"
        );
    }

    // get_rate const-folds to the resolved default_frequency (48 MHz).
    assert!(
        tu.contains("48000000u"),
        "static get_rate must const-fold to default_frequency 48000000 (D07/D10); got:\n{tu}"
    );
}
