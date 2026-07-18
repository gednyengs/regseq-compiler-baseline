//! The zephyr_c induced-failure E1000 variant gate (backend-zephyr-c-e1000
//! T06): `e1000-initfail` under the ZEPHYR binding, lowered by
//! `emit_zephyr_c`, its artifacts byte-locked via this sole binary's
//! `BLESS=1` path, plus named spot assertions on the authored-Err path.
//!
//! The variant gets NO generic_c oracle pair (C05: the behavioral proof is
//! the L2 `init_res=5` witness — EIO from the authored HardwareError,
//! distinct from the C01 rungs' unwrapped 19/ENODEV, I09).
//!
//! SOLE test in this binary (clean-process interner isolation, like the
//! other snapshot e2es).

#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::{Path, PathBuf};

use codegen_coverage_runner::{manifest, pipeline};
use test_ir_fixtures::{try_bridge_fixture_path, BridgeOutcome};

fn composed_corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/_composed")
}

#[test]
fn zephyr_e1000_initfail_snapshot_and_err_path() {
    let fixtures = manifest::discover(&composed_corpus()).expect("discover composed corpus");
    let fixture = fixtures
        .iter()
        .find(|f| f.manifest.name == "e1000-initfail")
        .expect("e1000-initfail fixture discovered")
        .with_binding(manifest::OsBinding::Zephyr)
        .expect("the converged fixture resolves the zephyr binding (C05)");
    let fixture = &fixture;

    if std::env::var_os("BLESS").is_some() {
        let mut model = match try_bridge_fixture_path(&fixture.input) {
            BridgeOutcome::Ok(m) => m,
            other => panic!("bless bridge failed: {other:?}"),
        };
        pipeline::bind_l2_platform(&mut model, fixture).expect("L2 bind (bless)");
        let emit = regseq_backends::emit_zephyr_c(&model).expect("emit_zephyr_c must succeed");
        std::fs::write(pipeline::zephyr_golden_path(fixture), &emit.tu)
            .expect("write golden.zephyr.c");
        std::fs::write(pipeline::zephyr_binding_path(fixture), &emit.binding_yaml)
            .expect("write golden.zephyr.binding.yaml");
        std::fs::write(
            pipeline::zephyr_conformance_path(fixture),
            codegen_coverage_runner::conformance::render_json(&emit.conformance),
        )
        .expect("write conformance.zephyr.json");
        return;
    }

    // The C05 zephyr row, byte-locking the artifacts.
    let result = pipeline::run_zephyr_fixture(fixture);
    assert!(
        result.is_green(),
        "e1000-initfail (zephyr binding) must pass bridge -> emit_zephyr_c -> snapshot: {:?}",
        result.verdict
    );

    let tu = std::fs::read_to_string(pipeline::zephyr_golden_path(fixture))
        .expect("read golden.zephyr.c");
    let pos = |needle: &str| {
        tu.find(needle)
            .unwrap_or_else(|| panic!("TU must contain `{needle}`"))
    };

    // The distinct DUT compatible + driver name (D01: regseq,e1000fail
    // mirrors the module name so both DUTs stay bindable side by side).
    pos("#define DT_DRV_COMPAT regseq_e1000fail");
    pos("static int e1000fail_init(const struct device *dev)");
    pos("#define LOG_MODULE_NAME e1000fail");
    pos("/* RSEQ-IGNORED-MATCH: pci vendor=0x8086 device=0x100e");

    // The full C01 preamble precedes the authored body (the variant has a
    // region): both rungs, carrier install, MEM-only enable, map.
    let p_rung = pos("if (data->pcie->bdf == PCIE_BDF_NONE)");
    let p_probe = pos("if (!pcie_get_mbar(data->pcie->bdf, 0, &mbar))");
    let p_call = pos("e1000fail_on_init(&data->inner)");
    assert!(p_rung < p_probe && p_probe < p_call);

    // The authored error path, observably: the STATUS read against the
    // impossible expectation feeds the LOG_ERR + Err return; the
    // HardwareError -> -EIO arm is live in the adapter, the on_init_failure
    // hook sequenced before it (the I09 wrapped channel: init_res=5 at L2,
    // distinct from the rungs' unwrapped 19).
    pos("E1000FAIL_STATUS");
    pos("0xdeadbeefu");
    let p_hook = pos("(void)e1000fail_on_init_failure(&data->inner);");
    let p_eio = pos("return -EIO;");
    assert!(p_call < p_hook && p_hook < p_eio);
    pos("LOG_ERR(\"e1000fail: status %#x fails the impossible expectation\"");
    pos("LOG_ERR(\"e1000fail: init failed, cleaning up partial state\");");

    // No on_deinit: the plain DEFINE, no deinit fn, no macro swap.
    assert!(!tu.contains("DEVICE_DT_INST_DEINIT_DEFINE"), "{tu}");
    pos("DEVICE_DT_INST_DEFINE(n, e1000fail_init, NULL,");
    // No on_surprise_remove: no absent-slot block.
    assert!(!tu.contains("RSEQ-ABSENT-SLOT"), "{tu}");

    // The C06 class disposition rides the variant too (declared network).
    pos("RSEQ-DEFERRED-CLASS: device_class \"network\" declared by the spec");

    // The conformance JSON: delivered = on_init/on_init_failure only;
    // ignored = the one Pci row; the C06 note.
    let conf = std::fs::read_to_string(pipeline::zephyr_conformance_path(fixture))
        .expect("read conformance.zephyr.json");
    assert!(
        conf.contains("{\"slot\": \"on_init\", \"row\": null}"),
        "{conf}"
    );
    assert!(
        conf.contains("{\"slot\": \"on_init_failure\", \"row\": null}"),
        "{conf}"
    );
    assert!(!conf.contains("on_deinit"), "{conf}");
    assert!(!conf.contains("on_surprise_remove"), "{conf}");
    assert!(
        conf.contains("\"entry\": \"pci vendor=0x8086 device=0x100e\""),
        "{conf}"
    );
    assert!(
        conf.contains(
            "\"deferred_class\": {\"class\": \"network\", \"license\": \"backend-zephyr-c-e1000 GRILL D14\"}"
        ),
        "{conf}"
    );
}
