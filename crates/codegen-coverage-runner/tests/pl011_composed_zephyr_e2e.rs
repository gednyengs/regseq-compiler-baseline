//! The composed PL011 Zephyr-sibling oracle gate (backend-zephyr-c unit, M1/C1).
//!
//! The sibling fixture (GRILL D01) carries the five refined construct surfaces
//! — register-access, BRM (`bus = "amba"` folding into the Platform path),
//! device-lifecycle, diagnostics-logging, RegseqError — with a Devicetree
//! match set (`compatible = ["regseq,pl011"]`, D04) and NO `on_deinit` (D06),
//! over the eval DUT binding (second PL011, base 0x09040000).
//!
//! T01 stage: bridge-acceptance smoke — the sibling must bridge to a validated
//! BOUND model with the expected recognition facts (the linux T02 pattern).
//! T03 extends this SAME binary with the certified six-stage generic_c
//! pipeline and the `BLESS=1` path writing `golden.c` + `expected.stdout`
//! (the committed Zephyr-leg canonical oracle trace).
//!
//! SOLE test in this binary (like `pl011_composed_e2e`): the emit's MIR is
//! sensitive to rustc's process-global interner history, so the byte-snapshot
//! T03 adds is reproducible only in a dedicated clean process.
//!
//! `#![feature(rustc_private)]` + `extern crate rustc_driver;` link the
//! rustc-backed bridge (stage 1) into the test binary.
//!
//! Bridging this fixture emits two benign rustc warnings (`unused_macros` /
//! `unused_imports` on the diag surface's `warn` macro): the only `warn!`
//! site lives in the `on_deinit` the sibling drops, and the macro block is
//! C02 shared-remainder text that cannot diverge from the linux file.

#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::{Path, PathBuf};

use codegen_coverage_runner::{manifest, pipeline};
use regseq_model::{
    AttrData, BusType, FrameworkType, IntrinsicKind, LogLevel, MatchEntry, MmioOp, RegseqModel,
    TargetOs,
};
use test_ir_fixtures::{try_bridge_fixture_path, BridgeOutcome};

fn composed_corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/_composed")
}

#[test]
fn pl011_composed_zephyr_bridge_acceptance() {
    let fixtures = manifest::discover(&composed_corpus()).expect("discover composed corpus");
    let fixture = fixtures
        .iter()
        .find(|f| f.manifest.name == "pl011-composed-zephyr")
        .expect("pl011-composed-zephyr fixture discovered");

    // The fixture MUST carry its sibling platform.toml: per the ratified A-10
    // policy the L2 literals are the resolution trigger (and the D11 emitted
    // cross-check consumes them).
    assert!(
        fixture.platform.is_some(),
        "pl011-composed-zephyr must discover a sibling platform.toml (A-10 trigger)"
    );

    // Bridge (the process's first — interner-clean — bridge, so the T03
    // snapshot path can grow around this call without reordering).
    let mut model = match try_bridge_fixture_path(&fixture.input) {
        BridgeOutcome::Ok(m) => m,
        other => panic!("pl011-composed-zephyr must bridge cleanly: {other:?}"),
    };
    let identity = pipeline::bind_l2_platform(&mut model, fixture).expect("L2 bind + validate");
    assert_eq!(
        identity.os,
        Some(TargetOs::Zephyr),
        "[platform] identity must resolve os=zephyr"
    );
    assert_recognition_facts(&model);
}

/// The five-construct recognition facts on the bound model (T01 acceptance —
/// the sibling deltas vs `pl011-composed`: Devicetree match, no `on_deinit`,
/// the DUT base).
fn assert_recognition_facts(model: &RegseqModel) {
    // -- device-lifecycle: one recognized driver, Devicetree-matched, with
    //    on_init overridden and ONLY on_init (no on_deinit: D01/D06).
    assert_eq!(
        model.device_lifecycle.drivers.len(),
        1,
        "exactly one recognized driver"
    );
    let driver = model.device_lifecycle.drivers.values().next().unwrap();
    assert_eq!(
        driver.descriptor.match_set.len(),
        1,
        "exactly one match entry (the DUT compatible)"
    );
    match &driver.descriptor.match_set[0] {
        MatchEntry::Devicetree { compatible } => {
            assert_eq!(
                model.strings.resolve(*compatible),
                "regseq,pl011",
                "the D04 reserved compatible string"
            );
        }
        other => panic!("expected a devicetree match entry, got {other:?}"),
    }
    assert!(driver.ops.on_init.is_some(), "on_init overridden");
    assert!(
        driver.ops.on_deinit.is_none(),
        "NO on_deinit authored (D01: the zephyr ledger emit-rejects it, B-6/D06)"
    );
    assert!(
        driver.ops.on_surprise_remove.is_none()
            && driver.ops.on_init_failure.is_none()
            && driver.ops.on_system_shutdown.is_none(),
        "no other lifecycle callback overridden"
    );

    // -- BRM: the amba bus selector (shared remainder per D02 — Amba folds
    //    into the Platform path) + the resolved DUT (base, size) from
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
    assert_eq!(
        region.binding.base,
        Some(0x0904_0000),
        "the eval DUT base (second PL011, D04)"
    );
    assert_eq!(region.binding.size, Some(0x1000), "the DUT region size");
    assert_eq!(
        region.binding.effective_bar, None,
        "no BAR on an AMBA region"
    );

    // -- register-access + diagnostics-logging + RegseqError: the domain-attr
    //    tables must carry all five MMIO op kinds, the three AUTHORED log
    //    levels, and the RegseqError framework-type marker. Warn is absent by
    //    construction: the linux fixture's only `warn!` site lives in the
    //    `on_deinit` this sibling drops (D01), and the shared `on_init` body
    //    cannot gain one without breaking the C02 identity remainder. (D09's
    //    "all four levels live" is the prj.conf VISIBILITY pin, not a claim
    //    about authored sites.)
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
    for level in [LogLevel::Debug, LogLevel::Info, LogLevel::Error] {
        assert!(
            log_levels.contains(&(level.clone() as u8)),
            "log level {level:?} recognized"
        );
    }
    assert!(regseq_error_marked, "RegseqError framework type recognized");
}
