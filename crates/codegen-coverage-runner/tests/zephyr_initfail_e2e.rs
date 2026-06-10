//! The init-fail variant snapshot gate (backend-zephyr-c T10, GRILL D14):
//! `pl011-initfail-zephyr` lowered by `emit_zephyr_c`, its two artifacts
//! byte-locked via this sole binary's `BLESS=1` path, plus named spot
//! assertions on the authored-`Err` path — the R12 L0 leg of the witness
//! (T16 boots the image for the L2 leg).
//!
//! The variant is OUTSIDE the C02 identity invariant, gets NO generic_c
//! oracle bless, and has NO L3 leg (D14).
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
fn zephyr_initfail_snapshot_and_err_path() {
    let fixtures = manifest::discover(&composed_corpus()).expect("discover composed corpus");
    let fixture = fixtures
        .iter()
        .find(|f| f.manifest.name == "pl011-initfail-zephyr")
        .expect("pl011-initfail-zephyr fixture discovered");

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
        return;
    }

    // The C05 zephyr row, byte-locking both artifacts.
    let result = pipeline::run_zephyr_fixture(fixture);
    assert!(
        result.is_green(),
        "pl011-initfail-zephyr must pass bridge -> emit_zephyr_c -> snapshot: {:?}",
        result.verdict
    );

    let tu = std::fs::read_to_string(pipeline::zephyr_golden_path(fixture))
        .expect("read golden.zephyr.c");
    let pos = |needle: &str| {
        tu.find(needle)
            .unwrap_or_else(|| panic!("TU must contain `{needle}`"))
    };

    // The distinct driver name (D14) under the SAME DUT compatible.
    pos("#define DT_DRV_COMPAT regseq_pl011");
    pos("static int pl011_initfail_init(const struct device *dev)");
    pos("#define LOG_MODULE_NAME pl011_initfail");

    // The authored error path, observably: the LOG_ERR line precedes the
    // Err return; the DeviceBusy -> -EBUSY arm is live in the adapter (the
    // I10 witness the T16 leg confirms on hardware: a nonzero init return
    // leaves device_is_ready() == false).
    let p_log = pos("LOG_ERR(\"pl011 init-fail witness: refusing to start\");");
    let p_call = pos("pl011_initfail_on_init(&data->inner)");
    let p_ebusy = pos("return -EBUSY;");
    assert!(
        p_log < p_call,
        "the authored LOG_ERR lives in the on_init body"
    );
    assert!(p_call < p_ebusy, "the adapter follows the call");

    // Fieldless witness: no MMIO region — no map, no TU-static base, no
    // A-1 cross-check; the single-instance guard still stands.
    assert!(!tu.contains("DEVICE_MMIO_MAP("), "no region to map: {tu}");
    assert!(!tu.contains("mm_reg_t"), "no TU-static base carrier: {tu}");
    assert!(
        !tu.contains("DT_REG_ADDR"),
        "no A-1 cross-check without literals"
    );
    pos("BUILD_ASSERT(DT_NUM_INST_STATUS_OKAY(DT_DRV_COMPAT) == 1");
    assert!(!tu.contains("int main("), "no main in a Zephyr driver TU");

    // The binding yaml carries the shared DUT compatible.
    let yaml = std::fs::read_to_string(pipeline::zephyr_binding_path(fixture))
        .expect("read golden.zephyr.binding.yaml");
    assert!(yaml.contains("compatible: \"regseq,pl011\""), "{yaml}");
}
