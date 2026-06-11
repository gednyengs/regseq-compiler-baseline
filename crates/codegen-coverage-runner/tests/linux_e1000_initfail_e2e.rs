//! The linux_c induced-failure E1000 variant gate (backend-linux-c-e1000
//! T08/T09).
//!
//! The D07 variant through the C02 linux row: bridge → L2 bind →
//! `emit_linux_c` → byte-exact `golden.linux.c`, plus the authored-Err
//! chain's structure facts (the deliberately-impossible STATUS expectation
//! feeding the on_init_failure hook and the negated -EIO return) and the
//! recognition smoke. The behavioral proof is the L2 dmesg errno (T14).
//!
//! `BLESS=1` writes `golden.linux.c` for THIS binary's fixture only (I04).
//! SOLE test in this binary (interner isolation, the established
//! sole-binary pattern).

#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::{Path, PathBuf};

use codegen_coverage_runner::{manifest, pipeline};
use regseq_model::{BusType, MatchEntry, TargetOs};
use test_ir_fixtures::{try_bridge_fixture_path, BridgeOutcome};

fn composed_corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/_composed")
}

#[test]
fn e1000_initfail_snapshot_and_structure() {
    let fixtures = manifest::discover(&composed_corpus()).expect("discover composed corpus");
    let fixture = fixtures
        .iter()
        .find(|f| f.manifest.name == "e1000-initfail")
        .expect("e1000-initfail fixture discovered")
        .with_binding(manifest::OsBinding::Linux)
        .expect("the converged fixture resolves the linux binding (C05)");
    let fixture = &fixture;
    assert!(
        fixture.platform.is_some(),
        "e1000-initfail must resolve platform.linux.toml (A-10 trigger, C05)"
    );

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

    // The C02 linux row (stage-attributed), byte-locking the snapshot.
    let result = pipeline::run_linux_fixture(fixture);
    assert!(
        result.is_green(),
        "e1000-initfail must pass bridge -> emit_linux_c -> snapshot: {:?}",
        result.verdict
    );

    let tu = std::fs::read_to_string(pipeline::linux_golden_path(fixture))
        .expect("read golden.linux.c");
    let pos = |needle: &str| {
        tu.find(needle)
            .unwrap_or_else(|| panic!("TU must contain `{needle}`"))
    };

    // The authored-Err chain (D07): STATUS read against the impossible
    // expectation, the on_init_failure hook inside the Err arm, then the
    // negated -EIO (HardwareError row); devres owns the unwind.
    pos("readl(_1->regs + E1000FAIL_STATUS)");
    pos("0xdeadbeefu");
    let probe = pos("static int e1000fail_probe(struct pci_dev *pdev, const struct pci_device_id *id)");
    let enable = pos("ret = pcim_enable_device(pdev);");
    let map = pos("priv->inner.regs = pcim_iomap_region(pdev, 0, \"e1000fail\");");
    let on_init_call = pos("e1000fail_on_init(&priv->inner)");
    let hook = pos("(void)e1000fail_on_init_failure(&priv->inner);");
    let eio = pos("return -EIO;");
    assert!(probe < enable && enable < map && map < on_init_call && on_init_call < hook);
    assert!(hook < eio, "the hook runs before the negated errno returns");
    // No on_deinit/surprise/shutdown: plain remove, no predicate, no slot.
    assert!(!tu.contains("pci_device_is_present"));
    assert!(!tu.contains(".shutdown"));
    pos("/* on_deinit not overridden: default no-op teardown */");
    // The dmesg witnesses for T14.
    pos("pr_err(\"e1000fail: status %#x fails the impossible expectation\\n\"");
    pos("pr_err(\"e1000fail: init failed, cleaning up partial state\\n\");");
    // Single-row table + sentinel + device table under the distinct name.
    pos(".device\t\t= 0x100e,");
    pos("MODULE_DEVICE_TABLE(pci, e1000fail_ids);");
    pos("module_pci_driver(e1000fail_driver);");

    // RECOGNITION SMOKE (T08 acceptance) — strictly after the snapshot
    // verdict (interner discipline).
    let mut model = match try_bridge_fixture_path(&fixture.input) {
        BridgeOutcome::Ok(m) => m,
        other => panic!("e1000-initfail must bridge cleanly: {other:?}"),
    };
    let identity = pipeline::bind_l2_platform(&mut model, fixture).expect("L2 bind + validate");
    assert_eq!(identity.os, Some(TargetOs::Linux));

    // The D07 chain's model facts: a PCI driver named e1000fail (distinct
    // module name, S17), campaign-id matched, with on_init + on_init_failure
    // overridden and a resolved BAR0 binding.
    assert_eq!(model.device_lifecycle.drivers.len(), 1);
    let driver = model.device_lifecycle.drivers.values().next().unwrap();
    assert_eq!(
        model.strings.resolve(driver.descriptor.name),
        "e1000fail",
        "distinct driver/module name so both generated modules can coexist"
    );
    assert_eq!(driver.descriptor.bus_type, Some(BusType::Pci));
    // The CONVERGED match set (backend-zephyr-c-e1000 D01): the Pci row
    // first, the zephyr DT entry LAST.
    assert_eq!(driver.descriptor.match_set.len(), 2, "Pci, DT");
    assert_eq!(
        driver.descriptor.match_set[0],
        MatchEntry::Pci {
            vendor: 0x8086,
            device: 0x100e,
            subsystem: None,
            class: None,
        },
        "the campaign 82540EM id"
    );
    let MatchEntry::Devicetree { compatible } = &driver.descriptor.match_set[1] else {
        panic!("the converged DT entry is appended LAST (D01)");
    };
    assert_eq!(model.strings.resolve(*compatible), "regseq,e1000fail");
    assert!(
        driver.ops.on_init.is_some(),
        "on_init overridden (the authored-Err site)"
    );
    assert!(
        driver.ops.on_init_failure.is_some(),
        "on_init_failure overridden (the D07 hook)"
    );
    assert!(
        driver.ops.on_deinit.is_none()
            && driver.ops.on_surprise_remove.is_none()
            && driver.ops.on_system_shutdown.is_none(),
        "the variant overrides exactly the failure-path callbacks"
    );
    let region = &driver.handle.resource_fields[0];
    assert!(region.binding.is_resolved(), "A-10 trigger");
    assert_eq!(region.binding.effective_bar, Some(0), "BAR0");
}
