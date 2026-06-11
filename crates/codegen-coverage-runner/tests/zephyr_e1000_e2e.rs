//! The zephyr_c composed-E1000 snapshot gate (backend-zephyr-c-e1000 T06,
//! M1 close): the converged `e1000-composed` under the ZEPHYR binding,
//! lowered by `emit_zephyr_c` to the C05 artifact set — `golden.zephyr.c` +
//! `golden.zephyr.binding.yaml` + `conformance.zephyr.json` — byte-locked,
//! plus the L0 structure suite with NAMED assertions verifying the
//! C01/C02/C03/C06 landmarks so a regression is diagnosed, not just
//! byte-diffed.
//!
//! `BLESS=1` writes the three goldens from a single clean bridge+bind+emit,
//! then returns without also verifying (the keystone pattern). The bless
//! happened strictly AFTER the T05 R08 unblessed-first proof.
//!
//! SOLE test in this binary (like `zephyr_pl011_e2e`): the emit's MIR is
//! sensitive to rustc's process-global interner history, so the
//! byte-snapshot is reproducible only in a dedicated clean process.

#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::{Path, PathBuf};

use codegen_coverage_runner::{manifest, pipeline};
use test_ir_fixtures::{try_bridge_fixture_path, BridgeOutcome};

fn composed_corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/_composed")
}

#[test]
fn zephyr_e1000_snapshot_and_l0_structure() {
    let fixtures = manifest::discover(&composed_corpus()).expect("discover composed corpus");
    let fixture = fixtures
        .iter()
        .find(|f| f.manifest.name == "e1000-composed")
        .expect("e1000-composed fixture discovered")
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

    // The C05 zephyr row (stage-attributed), byte-locking all three
    // artifacts.
    let result = pipeline::run_zephyr_fixture(fixture);
    assert!(
        result.is_green(),
        "e1000-composed (zephyr binding) must pass bridge -> emit_zephyr_c -> snapshot: {:?}",
        result.verdict
    );

    let tu = std::fs::read_to_string(pipeline::zephyr_golden_path(fixture))
        .expect("read golden.zephyr.c");
    let pos = |needle: &str| {
        tu.find(needle)
            .unwrap_or_else(|| panic!("TU must contain `{needle}`"))
    };

    // -- match consumption (C04): the DT entry IS the compat pin; both Pci
    //    rows triple-render-audited adjacent to it, in match-set order.
    let p_compat = pos("#define DT_DRV_COMPAT regseq_e1000\n");
    let p_audit1 = pos("/* RSEQ-IGNORED-MATCH: pci vendor=0x8086 device=0x100e");
    let p_audit2 = pos("/* RSEQ-IGNORED-MATCH: pci vendor=0x8086 device=0x100f");
    assert!(
        p_audit1 < p_audit2 && p_audit2 < p_compat,
        "both Pci rows audited adjacent to the DT_DRV_COMPAT pin, set order"
    );
    pos("not consumed by the ratified static-devicetree registration form");

    // -- the PCIe scaffold (R01): include, data field, TU-static carrier,
    //    DECLARE/INIT in the instantiation block.
    pos("#include <zephyr/drivers/pcie/pcie.h>");
    pos("struct pcie_dev *pcie;");
    pos("static struct pcie_dev *__rseq_pcie;");
    pos("DEVICE_PCIE_INST_DECLARE(n);");
    pos("DEVICE_PCIE_INST_INIT(n, pcie),");

    // -- the C01 preamble, emission order: BDF rung -> carrier install ->
    //    hoisted probe (deviation audit) -> claim elision -> MEM-only
    //    enable -> map into the TU-static base; both rungs unwrapped (D11).
    let p_rung = pos("if (data->pcie->bdf == PCIE_BDF_NONE)");
    let p_carrier = pos("__rseq_pcie = data->pcie;");
    let p_dev = pos("RSEQ-DEVIATION: get_resource_handle hoisted before enable_device");
    let p_probe = pos("if (!pcie_get_mbar(data->pcie->bdf, 0, &mbar))");
    let p_claim = pos("claim_regions: zero-call elision");
    let p_enable = pos("pcie_set_cmd(data->pcie->bdf, PCIE_CONF_CMDSTAT_MEM, true);");
    let p_map = pos("device_map(&e1000_mmio_base, mbar.phys_addr, mbar.size, K_MEM_CACHE_NONE);");
    assert!(p_rung < p_carrier && p_carrier < p_dev && p_dev < p_probe);
    assert!(p_probe < p_claim && p_claim < p_enable && p_enable < p_map);
    assert_eq!(
        tu.matches("return -ENODEV; /* native channel, unwrapped (D11) */")
            .count(),
        2
    );
    pos("[license: backend-zephyr-c-e1000 GRILL D02]");
    // No DT-reg machinery on the PCIe arm: no DEVICE_MMIO capture, no A-1
    // cross-check (the binding requires no `reg`; base/size dead, A-10(a)).
    assert!(!tu.contains("DEVICE_MMIO_MAP"), "no DT map on the PCIe arm");
    assert!(!tu.contains("DEVICE_MMIO_ROM_INIT"), "no DT reg capture");
    assert!(!tu.contains("DT_REG_ADDR"), "no A-1 cross-check (dead literals)");
    pos("\"TU-static MMIO base and __rseq_pcie carrier require a single status-okay instance\"");

    // -- config-space (C02/C03): both word-0 reads hit the id cache (zero
    //    bus traffic), revision@0x08 / BAR0@0x10 issue live word reads, and
    //    the CLS byte write lowers to the audited containing-word RMW.
    assert_eq!(
        tu.matches("uint32_t __cw = __rseq_pcie->id;").count(),
        2,
        "vendor@0x00 AND device@0x02 served from the D06 cache"
    );
    pos("uint32_t __cw = pcie_conf_read(__rseq_pcie->bdf, 2u);");
    pos("uint32_t __cw = pcie_conf_read(__rseq_pcie->bdf, 4u);");
    pos("RSEQ-RMW: width-1 config write lowered to the containing-word RMW");
    pos("[license: backend-zephyr-c-e1000 GRILL D05]");
    pos("uint32_t __msk = 0xFFu << __sh;");
    assert_eq!(
        tu.matches("pcie_conf_write(__rseq_pcie->bdf, __w,").count(),
        1,
        "exactly ONE config write (the benign CLS byte)"
    );

    // -- the C06 class disposition (D14): the deferred-class audit comment
    //    immediately above the instantiation define; the scaffold stays the
    //    class-neutral NULL-api form.
    pos("RSEQ-DEFERRED-CLASS: device_class \"network\" declared by the spec");
    pos("[license: backend-zephyr-c-e1000 GRILL D14]");
    pos("NULL /* api: ClassTag::Generic only (A-16) */");

    // -- lifecycle: authored on_deinit swaps the macro (C04 precedent);
    //    authored on_surprise_remove renders the absent-slot block citing
    //    the wildcard row (R06).
    pos("static int e1000_deinit(const struct device *dev)");
    pos("DEVICE_DT_INST_DEINIT_DEFINE(n, e1000_init, e1000_deinit, NULL,");
    assert_eq!(tu.matches("RSEQ-ABSENT-SLOT:").count(), 1);
    pos("* disposition: AbsentDispatch row: CAP-ZEPHYR-DT-SURPRISE-1");
    assert!(
        !tu.contains("e1000_on_surprise_remove"),
        "auditable non-emission: no callback symbol"
    );

    // -- register access over the TU-static base: donor offsets recovered
    //    from the #[repr(C)] block layout.
    pos("#define E1000_CTRL 0x000");
    pos("#define E1000_TDBAH 0x3804");
    pos("#define E1000_TDT 0x3818");
    pos("static mm_reg_t e1000_mmio_base;");
    assert!(tu.contains("sys_read32(") && tu.contains("sys_write32("));

    // -- diag-log: all four authored levels, module block named e1000.
    pos("#define LOG_MODULE_NAME e1000");
    pos("LOG_DBG(\"e1000 probe start\");");
    pos("LOG_INF(");
    pos("LOG_WRN(\"e1000 deinit: masking interrupts\");");
    pos("LOG_ERR(\"e1000 tx enable readback failed\");");

    // -- no host/runner artifacts: the device model is the OS.
    assert!(!tu.contains("int main("), "no main in a Zephyr driver TU");

    // -- the binding yaml: the D10 PCIe form with the triple-render audit
    //    comments (R05 third render).
    let yaml = std::fs::read_to_string(pipeline::zephyr_binding_path(fixture))
        .expect("read golden.zephyr.binding.yaml");
    assert!(yaml.contains("compatible: \"regseq,e1000\""), "{yaml}");
    assert!(yaml.contains("include: [base.yaml, pcie-device.yaml]"), "{yaml}");
    assert!(yaml.contains("vendor-id:\n    required: true"), "{yaml}");
    assert!(yaml.contains("device-id:\n    required: true"), "{yaml}");
    assert!(!yaml.contains("reg:"), "no reg requirement (D10):\n{yaml}");
    assert!(
        yaml.contains("# RSEQ-IGNORED-MATCH: pci vendor=0x8086 device=0x100e")
            && yaml.contains("# RSEQ-IGNORED-MATCH: pci vendor=0x8086 device=0x100f"),
        "{yaml}"
    );

    // -- the conformance JSON (C05 verbatim): delivered =
    //    on_init/on_init_failure/on_deinit (declaration order), absent =
    //    the wildcard row, ignored = both Pci rows, the C06 note.
    let conf = std::fs::read_to_string(pipeline::zephyr_conformance_path(fixture))
        .expect("read conformance.zephyr.json");
    assert!(conf.contains("{\"slot\": \"on_init\", \"row\": null}"), "{conf}");
    assert!(
        conf.contains("{\"slot\": \"on_init_failure\", \"row\": null}"),
        "{conf}"
    );
    assert!(
        conf.contains("{\"slot\": \"on_deinit\", \"row\": \"CAP-ZEPHYR-DT-DEINIT-1\"}"),
        "{conf}"
    );
    assert!(
        conf.contains("{\"slot\": \"on_surprise_remove\", \"row\": \"CAP-ZEPHYR-DT-SURPRISE-1\""),
        "{conf}"
    );
    assert!(
        conf.contains("\"entry\": \"pci vendor=0x8086 device=0x100e\"")
            && conf.contains("\"entry\": \"pci vendor=0x8086 device=0x100f\""),
        "{conf}"
    );
    assert!(
        conf.contains(
            "\"deferred_class\": {\"class\": \"network\", \"license\": \"backend-zephyr-c-e1000 GRILL D14\"}"
        ),
        "{conf}"
    );
}
