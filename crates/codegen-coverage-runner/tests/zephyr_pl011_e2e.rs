//! The zephyr_c first-light snapshot gate (backend-zephyr-c unit, M2/C2):
//! `pl011-composed-zephyr` lowered by `emit_zephyr_c` to the D07 artifact
//! pair — `golden.zephyr.c` + `golden.zephyr.binding.yaml` — byte-locked,
//! plus the L0 structure suite with NAMED assertions (C01.1) so a
//! regression is diagnosed, not just byte-diffed.
//!
//! `BLESS=1` writes both goldens from a single clean bridge+bind+emit, then
//! returns without also verifying (the keystone pattern).
//!
//! SOLE test in this binary (like `pl011_composed_zephyr_e2e`): the emit's
//! MIR is sensitive to rustc's process-global interner history, so the
//! byte-snapshot is reproducible only in a dedicated clean process.
//!
//! `#![feature(rustc_private)]` + `extern crate rustc_driver;` link the
//! rustc-backed bridge (stage 1) into the test binary.

#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::{Path, PathBuf};

use codegen_coverage_runner::{manifest, pipeline};
use test_ir_fixtures::{try_bridge_fixture_path, BridgeOutcome};

fn composed_corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/_composed")
}

#[test]
fn zephyr_pl011_snapshot_and_l0_structure() {
    let fixtures = manifest::discover(&composed_corpus()).expect("discover composed corpus");
    let fixture = fixtures
        .iter()
        .find(|f| f.manifest.name == "pl011-composed-zephyr")
        .expect("pl011-composed-zephyr fixture discovered");

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

    // The C05 zephyr row (stage-attributed), byte-locking BOTH artifacts.
    let result = pipeline::run_zephyr_fixture(fixture);
    assert!(
        result.is_green(),
        "pl011-composed-zephyr must pass bridge -> emit_zephyr_c -> snapshot: {:?}",
        result.verdict
    );

    let tu = std::fs::read_to_string(pipeline::zephyr_golden_path(fixture))
        .expect("read golden.zephyr.c");
    let pos = |needle: &str| {
        tu.find(needle)
            .unwrap_or_else(|| panic!("TU must contain `{needle}`"))
    };

    // -- scaffold §1 skeleton order (C01.1): compat pin -> log block ->
    //    includes -> state -> bodies -> init fn -> instantiation -> guards.
    let p_compat = pos("#define DT_DRV_COMPAT regseq_pl011");
    let p_log = pos("LOG_MODULE_REGISTER(LOG_MODULE_NAME);");
    let p_include = pos("#include <zephyr/device.h>");
    let p_config = pos("struct pl011_config {");
    let p_data = pos("struct pl011_data {");
    let p_init = pos("static int pl011_init(const struct device *dev)");
    let p_foreach = pos("DT_INST_FOREACH_STATUS_OKAY(RSEQ_INIT)");
    let p_guard = pos("BUILD_ASSERT(DT_NUM_INST_STATUS_OKAY(DT_DRV_COMPAT) == 1");
    assert!(
        p_compat < p_log && p_log < p_include,
        "compat pin, then the log-module block, then plain includes (§1)"
    );
    assert!(p_include < p_config && p_config < p_data && p_data < p_init);
    assert!(p_init < p_foreach && p_foreach < p_guard);

    // -- per-instance state: MMIO macros FIRST members; the spec driver
    //    embedded in data (A-17).
    let config_body = tu
        .split("struct pl011_config {")
        .nth(1)
        .unwrap()
        .split("};")
        .next()
        .unwrap();
    assert!(
        config_body.trim_start().starts_with("DEVICE_MMIO_ROM;"),
        "DEVICE_MMIO_ROM must be the FIRST config member"
    );
    let data_body = tu
        .split("struct pl011_data {")
        .nth(1)
        .unwrap()
        .split("};")
        .next()
        .unwrap();
    assert!(
        data_body.trim_start().starts_with("DEVICE_MMIO_RAM;"),
        "DEVICE_MMIO_RAM must be the FIRST data member"
    );
    assert!(data_body.contains("inner;"), "spec driver embedded in data");

    // -- the single-instance guard AND the A-1/D11 cross-check, both with
    //    named messages.
    pos("\"TU-static MMIO base requires a single status-okay instance\"");
    pos("BUILD_ASSERT(DT_REG_ADDR(DT_DRV_INST(0)) == 0x9040000 &&");
    pos("DT_REG_SIZE(DT_DRV_INST(0)) == 0x1000,");
    pos("\"DT reg disagrees with the resolved [mmio] platform literals (D11)\"");

    // -- claim elision (the ratified zero-call lowering): no claim call in
    //    the TU; the elision is documented in the preamble comment.
    assert!(
        !tu.contains("claim_regions("),
        "claim_regions must emit ZERO calls"
    );
    pos("claim_regions = zero calls");

    // -- the init preamble: map then the TU-static base install (A-2).
    let p_map = pos("DEVICE_MMIO_MAP(dev, K_MEM_CACHE_NONE);");
    let p_base = pos("pl011_mmio_base = DEVICE_MMIO_GET(dev);");
    let p_on_init_call = pos("pl011_on_init(&data->inner)");
    assert!(
        p_map < p_base && p_base < p_on_init_call,
        "map -> base install -> on_init (scaffold §3 order)"
    );

    // -- the errno adapter: negated C02 symbols (the composed RegseqError
    //    carries DeviceBusy -> -EBUSY and HardwareError -> -EIO), Ok -> 0.
    let p_adapter = pos("if (__tag != ");
    assert!(p_on_init_call < p_adapter);
    pos("return -EBUSY;");
    pos("return -EIO;");

    // -- register access (matrix §A): width-switched sys_io accessors over
    //    the TU-static base + the const bank from the recovered block
    //    layout (real PL011 offsets).
    pos("#define PL011_DR 0x000");
    pos("#define PL011_FR 0x018");
    pos("#define PL011_IBRD 0x024");
    pos("#define PL011_FBRD 0x028");
    pos("#define PL011_LCR_H 0x02c");
    pos("#define PL011_CR 0x030");
    pos("#define PL011_IMSC 0x038");
    pos("#define PL011_ICR 0x044");
    assert!(
        tu.contains("sys_read32(") && tu.contains("sys_write32("),
        "32-bit registers must use sys_read32/sys_write32"
    );
    assert!(
        !tu.contains("sys_read64(") && !tu.contains("sys_write64("),
        "no 64-bit accessor may appear in a 32-bit-only TU"
    );
    pos("static mm_reg_t pl011_mmio_base;");

    // -- diag-log (matrix §D): demand-driven module block named from the
    //    descriptor (A-14), one LOG_* per authored site, [LEVEL] tags
    //    dropped, no trailing newlines.
    pos("#define LOG_MODULE_NAME pl011");
    pos("LOG_DBG(\"pl011 probe start\");");
    pos("LOG_INF(\"pl011 baud divisors: %u %u\"");
    pos("LOG_INF(\"pl011 up\");");
    pos("LOG_ERR(\"pl011 tx busy past spin bound %u\"");
    assert!(
        !tu.contains("[DEBUG]") && !tu.contains("[INFO]") && !tu.contains("\\n\""),
        "no [LEVEL] tags, no trailing newlines in LOG_* formats"
    );

    // -- no host/runner artifacts (S16/R02): the device model is the OS.
    assert!(!tu.contains("int main("), "no main in a Zephyr driver TU");
    assert!(!tu.contains("printf("), "no host printf");

    // -- the binding yaml (D07): the same compatible the TU pins.
    let yaml = std::fs::read_to_string(pipeline::zephyr_binding_path(fixture))
        .expect("read golden.zephyr.binding.yaml");
    assert!(yaml.contains("compatible: \"regseq,pl011\""), "{yaml}");
    assert!(yaml.contains("include: base.yaml"), "{yaml}");
}
