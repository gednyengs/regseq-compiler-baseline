//! Peripheral-clock-management linux_c emission witness — MANAGED (clock
//! construct, M3/T14/T16). A BACKEND-EMISSION witness, distinct from the
//! generic-c behavioral oracle (`clock_pl011_managed_e2e`): it string-checks the
//! Linux clk-framework lowering of the `clock-pl011-managed` fixture (PL011's
//! real managed target):
//!
//!   * the MANAGED clock field morphs to a `struct clk *` member in the priv
//!     `inner` struct (field_override, the sync-lock-morph mirror);
//!   * the acquire preamble is `devm_clk_get(&adev->dev, "uartclk")` with
//!     `IS_ERR`/`PTR_ERR` propagation, slotted AFTER the region map and BEFORE
//!     `on_init` (C07/D08); a get failure is a hard init error (I02);
//!   * the author ops lower to `clk_prepare_enable` / `clk_get_rate` (raw, no
//!     fallback, D10/R10) / `clk_disable_unprepare`; release is automatic (devm).
//!
//! SOLE test in this binary (rustc interner isolation, like the oracle).

#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::{Path, PathBuf};

use codegen_coverage_runner::{manifest, pipeline};
use test_ir_fixtures::{try_bridge_fixture_path, BridgeOutcome};

fn clock_corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/_clock")
}

#[test]
fn clock_linux_pl011_managed_witness() {
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
    let emit = regseq_backends::emit_linux_c(&model).expect("emit_linux_c must succeed");
    let tu = emit.tu;
    let pos = |needle: &str| {
        tu.find(needle)
            .unwrap_or_else(|| panic!("TU must contain `{needle}`; got:\n{tu}"))
    };

    // The clk header is pulled in (the morphed type + the clk_* ops).
    pos("#include <linux/clk.h>");
    // Field morph: the ZST `Clock` becomes a `struct clk *` member in `inner`.
    pos("struct clk *clk;");

    // Acquire preamble: devm_clk_get(con_id = the bound provider) + PTR_ERR,
    // AFTER the region map, BEFORE on_init (C07/D08/I02).
    let map = pos("devm_ioremap_resource(&adev->dev, &adev->res)");
    let acquire = pos("priv->inner.clk = devm_clk_get(&adev->dev, \"uartclk\");");
    pos("if (IS_ERR(priv->inner.clk))");
    pos("return PTR_ERR(priv->inner.clk);");
    let on_init = pos("pl011clk_on_init(&priv->inner)");
    assert!(
        map < acquire && acquire < on_init,
        "probe must order map_region -> devm_clk_get -> on_init (C07/D08)"
    );

    // Author ops over the morphed field (managed lowering, C04/D08/D10).
    pos("clk_prepare_enable((*");
    pos("clk_get_rate((*");
    // disable rides on_deinit.
    pos("clk_disable_unprepare((*");

    // A `clk_prepare_enable` failure PROPAGATES as Err — not a silent Ok (the
    // enable is branched, not `(void)`-discarded).
    pos("if (clk_prepare_enable((*");
    // The `?`-Result discriminant is DECODED tag→discriminant (Ok→0, Err→1); a
    // niche-encoded `Result<(), E>` read raw would mis-route every branch.
    pos("? 0x0 : 0x1");

    // No const-fold of the rate on the managed path (the raw live query, D10/R10).
    assert!(
        !tu.contains("= (uint32_t)24000000u")
            && !tu.contains("= (u32)24000000u")
            && !tu.contains("= 24000000u"),
        "managed get_rate must be the raw clk_get_rate, never a const-fold (D10/R10); got:\n{tu}"
    );
    // Release is devm-automatic — no explicit clk_put/clk_disable in remove
    // beyond the author's on_deinit disable (no double-free, A-2).
    assert!(
        !tu.contains("clk_put") && !tu.contains("devm_clk_put"),
        "managed release is devm-automatic; no explicit clk_put is emitted (A-2)"
    );
}
