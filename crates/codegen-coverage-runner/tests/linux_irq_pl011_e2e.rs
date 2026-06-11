//! The device-interrupt linux_c gate (construct #6, PLAN T09 + the T12 linux
//! leg): the `irq-pl011` fixture's linux lowering is byte-locked as
//! `golden.linux.c` + `conformance.linux.json`, with the C01 placement made
//! STRUCTURAL in the emitted kernel module:
//!
//!   * probe: `request_irq(adev->irq[0], rseq_isr_<drv>, IRQF_NO_AUTOEN,
//!     "<name>", priv)` AFTER the region preamble (D14), failure returning
//!     the kernel status before on_init (R16);
//!   * `enable_irq` after the on_init Ok adapter (D02/R05) — the Err path
//!     has already returned, carrying the `free_irq` unwind (R16);
//!   * remove: `disable_irq` -> `free_irq` after on_deinit (D09/R06), the
//!     manual pair (no devm);
//!   * the `irqreturn_t` wrapper maps `Handled -> IRQ_HANDLED`, everything
//!     else `IRQ_NONE` (never suppressed, R08), acquiring no locks (R18);
//!   * NO `IRQF_TRIGGER_*` and (this board) no `IRQF_SHARED` (D10/R12, D05);
//!   * conformance reports `delivered` for the deregister slot (the D09
//!     contrast with zephyr's structurally-absent static-connect deregister).
//!
//! `BLESS=1` writes both artifacts from a single clean bridge+bind+emit.
//! SOLE test in this binary (interner isolation, D17).

#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::{Path, PathBuf};

use codegen_coverage_runner::{manifest, pipeline};
use test_ir_fixtures::{try_bridge_fixture_path, BridgeOutcome};

fn irq_corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/_irq")
}

#[test]
fn linux_irq_pl011_snapshot_and_l0_structure() {
    let fixtures = manifest::discover(&irq_corpus()).expect("discover irq corpus");
    let fixture = fixtures
        .iter()
        .find(|f| f.manifest.name == "irq-pl011")
        .expect("irq-pl011 fixture discovered")
        .with_binding(manifest::OsBinding::Linux)
        .expect("irq-pl011 resolves the linux binding");
    let fixture = &fixture;

    if std::env::var_os("BLESS").is_some() {
        let mut model = match try_bridge_fixture_path(&fixture.input) {
            BridgeOutcome::Ok(m) => m,
            other => panic!("bless bridge failed: {other:?}"),
        };
        pipeline::bind_l2_platform(&mut model, fixture).expect("L2 bind (bless)");
        let emit = regseq_backends::emit_linux_c(&model).expect("emit_linux_c must succeed");
        std::fs::write(pipeline::linux_golden_path(fixture), &emit.tu)
            .expect("write golden.linux.c");
        std::fs::write(
            pipeline::linux_conformance_path(fixture),
            codegen_coverage_runner::conformance::render_json(&emit.conformance),
        )
        .expect("write conformance.linux.json");
        return;
    }

    let result = pipeline::run_linux_fixture(fixture);
    assert!(
        result.is_green(),
        "irq-pl011 must pass bridge -> bind -> emit_linux_c -> snapshot: {:?}",
        result.verdict
    );

    let tu = std::fs::read_to_string(pipeline::linux_golden_path(fixture))
        .expect("read golden.linux.c");
    let pos = |needle: &str| {
        tu.find(needle)
            .unwrap_or_else(|| panic!("TU must contain `{needle}`; got:\n{tu}"))
    };

    // C01 probe order, structurally: region preamble -> request_irq(DISABLED)
    // -> on_init adapter -> enable_irq -> return 0.
    let map = pos("devm_ioremap_resource(&adev->dev, &adev->res)");
    let request = pos(
        "request_irq(adev->irq[0], rseq_isr_pl011irq,\n\t\t\t\t\t    IRQF_NO_AUTOEN, \"pl011irq\", priv)",
    );
    let on_init_call = pos("pl011irq_on_init(&priv->inner)");
    let enable = pos("enable_irq(adev->irq[0]);");
    assert!(
        map < request && request < on_init_call && on_init_call < enable,
        "probe must order map -> request_irq(NO_AUTOEN) -> on_init -> enable_irq (D14/D02/R05)"
    );
    // The Err path carries the manual unwind (R16) before its errno return.
    let unwind = pos("free_irq(adev->irq[0], priv);");
    assert!(
        unwind > request && unwind < enable,
        "the on_init-Err unwind's free_irq must sit inside the adapter, before the enable"
    );

    // Remove order: on_deinit -> disable_irq -> free_irq (D09/R06).
    let on_deinit_call = pos("pl011irq_on_deinit(&priv->inner)");
    let disable = pos("disable_irq(adev->irq[0]);");
    let last_free = tu
        .rfind("free_irq(adev->irq[0], priv);")
        .expect("remove must free the irq");
    assert!(
        on_deinit_call < disable && disable < last_free,
        "remove must order on_deinit -> disable_irq -> free_irq (D09)"
    );

    // The wrapper: irqreturn_t, dev_id recovery, IRQ_NONE never suppressed.
    pos("static irqreturn_t rseq_isr_pl011irq(int irq, void *dev_id)");
    pos("return IRQ_HANDLED;");
    pos("return IRQ_NONE;");

    // R12/D05: no trigger flags, and this board's line is not shared.
    assert!(
        !tu.contains("IRQF_TRIGGER"),
        "no IRQF_TRIGGER_* may be emitted (D10/R12: the DT is authoritative)"
    );
    assert!(
        !tu.contains("IRQF_SHARED"),
        "IRQF_SHARED only when the L2 board fact says shared (D05); this board is not"
    );
    assert!(
        !tu.contains("devm_request_irq"),
        "the manual request/free pair is the ratified lowering (D09, no devm)"
    );

    // The T12 linux leg: conformance reports the deregister slot DELIVERED.
    let conf = std::fs::read_to_string(pipeline::linux_conformance_path(fixture))
        .expect("read conformance.linux.json");
    assert!(
        conf.contains("\"slot\": \"interrupt.register_handler.deregister\""),
        "conformance must report the deregister slot delivered (D09 contrast with C02); got:\n{conf}"
    );
}
