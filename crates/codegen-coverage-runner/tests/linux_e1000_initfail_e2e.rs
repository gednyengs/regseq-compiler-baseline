//! The linux_c induced-failure E1000 variant gate (backend-linux-c-e1000
//! T08/T09).
//!
//! T08 state: bridge-acceptance smoke — the D07 variant bridges to a
//! validated bound model with the authored-Err chain's facts recognized
//! (on_init + on_init_failure overridden, PCI bus, resolved BAR0 binding).
//! The `golden.linux.c` snapshot + BLESS path land with T09; the behavioral
//! proof is the L2 dmesg errno (T14).
//!
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
fn e1000_initfail_bridges_and_binds() {
    let fixtures = manifest::discover(&composed_corpus()).expect("discover composed corpus");
    let fixture = fixtures
        .iter()
        .find(|f| f.manifest.name == "e1000-initfail")
        .expect("e1000-initfail fixture discovered");
    assert!(
        fixture.platform.is_some(),
        "e1000-initfail must discover a sibling platform.toml (A-10 trigger)"
    );

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
    assert_eq!(
        driver.descriptor.match_set,
        vec![MatchEntry::Pci {
            vendor: 0x8086,
            device: 0x100e,
            subsystem: None,
            class: None,
        }],
        "the campaign 82540EM id"
    );
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
