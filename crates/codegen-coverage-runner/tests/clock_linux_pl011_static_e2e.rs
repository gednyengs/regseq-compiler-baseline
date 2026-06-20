//! Peripheral-clock-management linux_c emission witness — STATIC (clock
//! construct, M3/T14/T16). A BACKEND-EMISSION witness, distinct from the
//! generic-c behavioral oracle (`clock_pl011_static_e2e`): it string-checks that
//! the Linux lowering of the `clock-pl011-static` fixture folds the static clock
//! away entirely (D07):
//!
//!   * NO `struct clk *` field morph (the ZST `Clock` field stays dropped);
//!   * NO `devm_clk_get` acquire preamble and NO `<linux/clk.h>`;
//!   * `enable`/`disable` are no-ops (no `clk_prepare_enable`/`clk_disable_unprepare`);
//!   * `get_rate` const-folds to the resolved `default_frequency` (48 MHz),
//!     never a `clk_get_rate` (D07/D10).
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
fn clock_linux_pl011_static_witness() {
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
    let emit = regseq_backends::emit_linux_c(&model).expect("emit_linux_c must succeed");
    let tu = emit.tu;

    // The region map still happens (the clock is orthogonal to MMIO).
    assert!(
        tu.contains("devm_ioremap_resource(&adev->dev, &adev->res)"),
        "the MMIO region preamble is unaffected by a static clock; got:\n{tu}"
    );

    // A static clock leaves NO clk-framework trace: no morph, no acquire, no ops.
    for needle in [
        "<linux/clk.h>",
        "struct clk *",
        "devm_clk_get",
        "clk_prepare_enable",
        "clk_disable_unprepare",
        "clk_get_rate",
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
