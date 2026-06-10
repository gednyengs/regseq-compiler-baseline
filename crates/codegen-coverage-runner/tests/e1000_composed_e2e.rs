//! The composed E1000 oracle gate (backend-linux-c-e1000 unit, M1).
//!
//! The five refined construct surfaces (register-access, BRM-PCI,
//! device-lifecycle, diagnostics-logging, RegseqError) plus the config-space
//! ops flow the full certified six-stage path together: bridge → L2 bind →
//! emit → snapshot → cc → run → stdout oracle. The blessed `expected.stdout`
//! is the CANONICAL E1000 ORACLE TRACE (GRILL D01/D05): the L3 trace
//! differential compares the generated Linux PCI driver's QEMU MMIO capture
//! against it, so this file is doubly protected (I04 + the L3 artifact).
//!
//! T01 state: the fixture is authored and this binary carries the
//! bridge-acceptance + recognition smoke (T01 acceptance). The six-stage
//! snapshot run + BLESS path land with T02, which commits the oracle trace.
//!
//! SOLE test in this binary (like `pl011_composed_e2e` / `bus_keystone_e2e`):
//! the emit's MIR is sensitive to rustc's process-global interner history, so
//! the byte-snapshot is reproducible only in a dedicated clean process (D17).
//!
//! `#![feature(rustc_private)]` + `extern crate rustc_driver;` link the
//! rustc-backed bridge (stage 1) into the test binary.

#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::{Path, PathBuf};

use codegen_coverage_runner::{manifest, pipeline};
use regseq_model::{
    AttrData, BusType, ConfigOp, FrameworkType, IntrinsicKind, LogLevel, MatchEntry, MmioOp,
    RegseqModel, TargetOs,
};
use test_ir_fixtures::{try_bridge_fixture_path, BridgeOutcome};

fn composed_corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/_composed")
}

#[test]
fn e1000_composed_bridges_and_binds() {
    let fixtures = manifest::discover(&composed_corpus()).expect("discover composed corpus");
    let fixture = fixtures
        .iter()
        .find(|f| f.manifest.name == "e1000-composed")
        .expect("e1000-composed fixture discovered");

    // The fixture MUST carry its sibling platform.toml: per the ratified A-10
    // policy the L2 literals are the resolution trigger (dead on Linux, C04).
    assert!(
        fixture.platform.is_some(),
        "e1000-composed must discover a sibling platform.toml (A-10 trigger)"
    );

    // Bridge-acceptance smoke (T01): the composed spec bridges to a validated
    // bound model with all five construct surfaces + config ops recognized.
    let mut model = match try_bridge_fixture_path(&fixture.input) {
        BridgeOutcome::Ok(m) => m,
        other => panic!("e1000-composed must bridge cleanly: {other:?}"),
    };
    let identity = pipeline::bind_l2_platform(&mut model, fixture).expect("L2 bind + validate");
    assert_eq!(
        identity.os,
        Some(TargetOs::Linux),
        "[platform] identity must resolve os=linux"
    );
    assert_recognition_facts(&model);
}

/// The five-construct + config-ops recognition facts on the bound model (T01
/// acceptance; re-asserted by the T02 six-stage gate).
fn assert_recognition_facts(model: &RegseqModel) {
    // -- device-lifecycle: one recognized driver, PCI-matched (two rows),
    //    with on_init/on_deinit/on_surprise_remove/on_init_failure overridden
    //    and on_system_shutdown ABSENT (the omit-when-absent .shutdown slot).
    assert_eq!(
        model.device_lifecycle.drivers.len(),
        1,
        "exactly one recognized driver"
    );
    let driver = model.device_lifecycle.drivers.values().next().unwrap();
    assert_eq!(
        driver.descriptor.match_set,
        vec![
            MatchEntry::Pci {
                vendor: 0x8086,
                device: 0x100e,
                subsystem: None,
                class: None,
            },
            MatchEntry::Pci {
                vendor: 0x8086,
                device: 0x100f,
                subsystem: None,
                class: None,
            },
        ],
        "the campaign 82540EM id + the 82545EM sibling row (B-6 full match_set)"
    );
    assert!(
        driver.ops.on_init.is_some() && driver.ops.on_deinit.is_some(),
        "on_init + on_deinit overridden"
    );
    assert!(
        driver.ops.on_surprise_remove.is_some(),
        "on_surprise_remove overridden (the present-predicate .remove input)"
    );
    assert!(
        driver.ops.on_init_failure.is_some(),
        "on_init_failure overridden (the D07 error-machinery hook)"
    );
    assert!(
        driver.ops.on_system_shutdown.is_none(),
        "on_system_shutdown NOT overridden (omit-when-absent .shutdown)"
    );

    // -- BRM: the pci bus selector + the resolved (base, size) from
    //    platform.toml + the effective BAR.
    assert_eq!(
        driver.descriptor.bus_type,
        Some(BusType::Pci),
        "bus = \"pci\" recognized"
    );
    assert_eq!(
        driver.handle.resource_fields.len(),
        1,
        "exactly one Mmio resource field (single resolved region, D02)"
    );
    let region = &driver.handle.resource_fields[0];
    assert!(
        region.binding.is_resolved(),
        "the L2 join must resolve the binding (A-10 trigger)"
    );
    assert_eq!(region.binding.base, Some(0xf000_0000), "C04 dead-literal base");
    assert_eq!(region.binding.size, Some(0x2_0000), "C04 dead-literal size");
    assert_eq!(region.binding.effective_bar, Some(0), "BAR0 (L2.bar ?? L1.bar)");

    // -- register-access + config + diagnostics-logging + RegseqError: the
    //    domain-attr tables must carry all five MMIO op kinds, both config
    //    ops, all four log levels, and the RegseqError framework-type marker.
    let mut mmio_ops = std::collections::BTreeSet::new();
    let mut config_ops = std::collections::BTreeSet::new();
    let mut log_levels = std::collections::BTreeSet::new();
    let mut regseq_error_marked = false;
    for attrs in model.domain_attrs.item_attrs.iter() {
        for attr in attrs {
            match attr {
                AttrData::Intrinsic(IntrinsicKind::Mmio(op)) => {
                    mmio_ops.insert(*op as u8);
                }
                AttrData::Intrinsic(IntrinsicKind::Config(op)) => {
                    config_ops.insert(*op as u8);
                }
                AttrData::Intrinsic(IntrinsicKind::Log(level)) => {
                    log_levels.insert(level.clone() as u8);
                }
                AttrData::FrameworkType(FrameworkType::RegseqError) => {
                    regseq_error_marked = true;
                }
                _ => {}
            }
        }
    }
    for op in [
        MmioOp::Read,
        MmioOp::ReadField,
        MmioOp::Write,
        MmioOp::WriteField,
        MmioOp::Modify,
    ] {
        assert!(
            mmio_ops.contains(&(op as u8)),
            "register-access op {op:?} recognized"
        );
    }
    for op in [ConfigOp::Read, ConfigOp::Write] {
        assert!(
            config_ops.contains(&(op as u8)),
            "config-space op {op:?} recognized (A-11 surface)"
        );
    }
    for level in [
        LogLevel::Debug,
        LogLevel::Info,
        LogLevel::Warn,
        LogLevel::Error,
    ] {
        assert!(
            log_levels.contains(&(level.clone() as u8)),
            "log level {level:?} recognized"
        );
    }
    assert!(regseq_error_marked, "RegseqError framework type recognized");
}
