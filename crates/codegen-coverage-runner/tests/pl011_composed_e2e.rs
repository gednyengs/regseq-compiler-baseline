//! The composed PL011 fixture gate (backend-linux-c unit, M1/C2).
//!
//! T02 (this revision): the bridge-acceptance smoke — the first time the five
//! refined construct surfaces (register-access, BRM-AMBA, device-lifecycle,
//! diagnostics-logging, RegseqError) are bridged + L2-bound TOGETHER. The test
//! asserts the bound model carries every construct's recognition facts before
//! any golden exists. T03 extends this binary with the full six-stage run and
//! the `BLESS=1` flow that commits `golden.c` + `expected.stdout` (= the
//! canonical PL011 oracle trace).
//!
//! SOLE test in this binary (like `bus_keystone_e2e` / `lifecycle_pl011_e2e`):
//! the emit's MIR is sensitive to rustc's process-global interner history, so
//! snapshots are reproducible only in a dedicated clean process (D17/D06).
//!
//! `#![feature(rustc_private)]` + `extern crate rustc_driver;` link the
//! rustc-backed bridge (stage 1) into the test binary.

#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::{Path, PathBuf};

use codegen_coverage_runner::{manifest, pipeline};
use regseq_model::{
    AttrData, BusType, FrameworkType, IntrinsicKind, LogLevel, MatchEntry, MmioOp, TargetOs,
};
use test_ir_fixtures::{try_bridge_fixture_path, BridgeOutcome};

fn composed_corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/_composed")
}

#[test]
fn pl011_composed_bridges_to_a_validated_bound_model() {
    let fixtures = manifest::discover(&composed_corpus()).expect("discover composed corpus");
    let fixture = fixtures
        .iter()
        .find(|f| f.manifest.name == "pl011-composed")
        .expect("pl011-composed fixture discovered");

    // The fixture MUST carry its sibling platform.toml: per the ratified A-10
    // policy the L2 literals are the resolution trigger.
    assert!(
        fixture.platform.is_some(),
        "pl011-composed must discover a sibling platform.toml (A-10 trigger)"
    );

    // Stage 1: bridge (the five-surface composition's first acceptance gate).
    let mut model = match try_bridge_fixture_path(&fixture.input) {
        BridgeOutcome::Ok(m) => m,
        other => panic!("pl011-composed must bridge cleanly: {other:?}"),
    };

    // Stage 1b: L2 bind + surface validation + post-bind model validation —
    // the same seam production and the keystone e2e use.
    let identity = pipeline::bind_l2_platform(&mut model, fixture).expect("L2 bind + validate");
    assert_eq!(
        identity.os,
        Some(TargetOs::Linux),
        "[platform] identity must resolve os=linux"
    );

    // -- device-lifecycle: one recognized driver, AMBA-matched, with on_init
    //    and on_deinit overridden (and only those).
    assert_eq!(
        model.device_lifecycle.drivers.len(),
        1,
        "exactly one recognized driver"
    );
    let driver = model.device_lifecycle.drivers.values().next().unwrap();
    assert_eq!(
        driver.descriptor.match_set,
        vec![MatchEntry::Amba {
            id: 0x0004_1011,
            mask: 0x000f_ffff
        }],
        "the amba-pl011 periphid match entry"
    );
    assert!(
        driver.ops.on_init.is_some() && driver.ops.on_deinit.is_some(),
        "on_init + on_deinit overridden"
    );
    assert!(
        driver.ops.on_surprise_remove.is_none()
            && driver.ops.on_init_failure.is_none()
            && driver.ops.on_system_shutdown.is_none(),
        "no other lifecycle callback overridden"
    );

    // -- BRM: the amba bus selector + the resolved (base, size) from
    //    platform.toml, with no BAR (PCI-only) facts.
    assert_eq!(
        driver.descriptor.bus_type,
        Some(BusType::Amba),
        "bus = \"amba\" recognized"
    );
    assert_eq!(
        driver.handle.resource_fields.len(),
        1,
        "exactly one Mmio resource field"
    );
    let region = &driver.handle.resource_fields[0];
    assert!(
        region.binding.is_resolved(),
        "the L2 join must resolve the binding (A-10 trigger)"
    );
    assert_eq!(region.binding.base, Some(0x0900_0000), "QEMU virt PL011 base");
    assert_eq!(region.binding.size, Some(0x1000), "QEMU virt PL011 size");
    assert_eq!(region.binding.effective_bar, None, "no BAR on an AMBA region");

    // -- register-access + diagnostics-logging + RegseqError: the domain-attr
    //    tables must carry all five MMIO op kinds, all four log levels, and
    //    the RegseqError framework-type marker.
    let mut mmio_ops = std::collections::BTreeSet::new();
    let mut log_levels = std::collections::BTreeSet::new();
    let mut regseq_error_marked = false;
    for attrs in model.domain_attrs.item_attrs.iter() {
        for attr in attrs {
            match attr {
                AttrData::Intrinsic(IntrinsicKind::Mmio(op)) => {
                    mmio_ops.insert(*op as u8);
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
    assert!(
        regseq_error_marked,
        "RegseqError framework type recognized"
    );
}
