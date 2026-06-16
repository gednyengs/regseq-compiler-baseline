//! The composed E1000 oracle gate (backend-linux-c-e1000 unit, M1/C2).
//!
//! The five refined construct surfaces (register-access, BRM-PCI,
//! device-lifecycle, diagnostics-logging, RegseqError) plus the config-space
//! ops flow the full certified six-stage path together: bridge → L2 bind →
//! emit → snapshot → cc → run → stdout oracle. The blessed `expected.stdout`
//! is the CANONICAL E1000 ORACLE TRACE (GRILL D01/D05): the L3 trace
//! differential compares the generated Linux PCI driver's QEMU MMIO capture
//! against it, so this file is doubly protected (I04 + the L3 artifact).
//!
//! `BLESS=1` writes `golden.c` + `expected.stdout` from a single clean
//! bind+emit+run, then returns without also verifying (the keystone pattern).
//!
//! SOLE test in this binary (like `pl011_composed_e2e` / `bus_keystone_e2e`):
//! the emit's MIR is sensitive to rustc's process-global interner history, so
//! the byte-snapshot is reproducible only in a dedicated clean process (D17).
//! Ordering inside the test preserves that: the verify path runs the pipeline
//! (its bridge is the process's FIRST bridge, matching the bless process)
//! before the recognition smoke re-bridges for model-fact assertions.
//!
//! The C1 review finding (smoke pinned recognition kinds, not body facts) is
//! discharged HERE: the behavioral gates below pin the config widths + the
//! single CLS write, the TDBAH-then-TDBAL order, the TDT=0 doorbell, the
//! runtime-index array offsets, and the donor-pattern offsets — on the
//! committed dual oracle, the same place the pl011 composition gates live.
//!
//! `#![feature(rustc_private)]` + `extern crate rustc_driver;` link the
//! rustc-backed bridge (stage 1) into the test binary.

#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::{Path, PathBuf};

use codegen_coverage_runner::{manifest, pipeline, toolchain};
use regseq_model::{
    AttrData, BusType, ConfigOp, FrameworkType, IntrinsicKind, LogLevel, MatchEntry, MmioOp,
    RegseqModel, TargetOs,
};
use test_ir_fixtures::{try_bridge_fixture_path, BridgeOutcome};

/// The host `cc`, matching the runner's default.
fn cc() -> &'static Path {
    Path::new("cc")
}

fn composed_corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/_composed")
}

#[test]
fn e1000_composed_six_stage_oracle() {
    let dir = composed_corpus().join("e1000-composed");
    let fixtures = manifest::discover(&composed_corpus()).expect("discover composed corpus");
    let fixture = fixtures
        .iter()
        .find(|f| f.manifest.name == "e1000-composed")
        .expect("e1000-composed fixture discovered")
        .with_binding(manifest::OsBinding::Linux)
        .expect("the converged fixture resolves the linux binding (C05)");
    let fixture = &fixture;

    // The bound fixture MUST carry its per-OS platform file: per the ratified
    // A-10 policy the L2 literals are the resolution trigger (dead on Linux).
    assert!(
        fixture.platform.is_some(),
        "e1000-composed must resolve platform.linux.toml (A-10 trigger, C05)"
    );

    if std::env::var_os("BLESS").is_some() {
        // The bless path applies the IDENTICAL L2 bind + identity resolution
        // run_fixture does, so the blessed golden reflects the BOUND model and
        // the traced `[platform]` identity.
        let options = pipeline::backend_options_for(fixture).expect("backend options");
        let mut model = match try_bridge_fixture_path(&fixture.input) {
            BridgeOutcome::Ok(m) => m,
            other => panic!("bless bridge failed: {other:?}"),
        };
        let identity = pipeline::bind_l2_platform(&mut model, fixture).expect("L2 bind (bless)");
        assert_eq!(identity.os, Some(TargetOs::Linux));
        assert_recognition_facts(&model);
        let context = regseq_backends::BackendContext::new(options, identity);
        let emit = regseq_backends::emit_generic_c_for_entry_with_context(
            &model,
            &fixture.manifest.entry,
            &context,
        )
        .expect("emit must succeed");
        let run = toolchain::compile_and_run(&emit.tu, &emit.shim, cc())
            .expect("compile + run must succeed");
        std::fs::write(dir.join("golden.c"), &emit.tu).expect("write golden.c");
        std::fs::write(dir.join("expected.stdout"), &run.stdout).expect("write expected.stdout");
        return;
    }

    // Stages 1-6 (the process's first — interner-clean — bridge).
    let result = pipeline::run_fixture(fixture, cc());
    assert!(
        result.is_green(),
        "e1000-composed must pass all six stages: {:?}",
        result.verdict
    );

    // COMPOSITION GATES on the committed golden: the PCI acquisition preamble
    // (enable_device + claim_regions(0) + get_handle) feeding map_region with
    // the real platform literals (A-10: L2 is the trigger; bar=0 on the PCI
    // region), under the traced qemu-pc/x86_64 Linux identity.
    let golden = std::fs::read_to_string(dir.join("golden.c")).expect("read golden.c");
    assert!(
        golden.contains("regseq_bus_map_region(0xf0000000ull, 0x20000ull, 0);"),
        "golden.c must map the real declared PCI (base, size, bar) from platform.toml"
    );
    assert!(
        golden.contains("regseq_bus_enable_device();"),
        "golden.c must CALL the PCI-only enable_device in the acquisition preamble"
    );
    assert!(
        golden.contains("regseq_bus_claim_regions(0);")
            && golden.contains("regseq_bus_get_handle();"),
        "golden.c must emit the every-bus acquisition preamble before map_region"
    );
    assert!(
        golden.contains("/* regseq target platform: os=linux board=qemu-pc arch=x86_64 */"),
        "golden.c must carry the [platform] identity trace from platform.toml"
    );

    // BEHAVIORAL GATES (dual oracle) on the canonical E1000 oracle trace —
    // these pin the body facts the C1 review called out:
    let stdout =
        std::fs::read_to_string(dir.join("expected.stdout")).expect("read expected.stdout");

    // Registration renders the FIRST match row (multi-row table proven at the
    // model level by assert_recognition_facts + at L0 by the linux_c table).
    assert!(
        stdout.contains("LC register e1000 match=pci:0x8086:0x100e"),
        "oracle trace must register with the campaign 82540EM id; got:\n{stdout}"
    );
    assert!(
        stdout.contains("BUS enable_device"),
        "the PCI acquisition must trace enable_device; got:\n{stdout}"
    );
    assert!(
        stdout.contains("BUS map base=0xf0000000 size=0x20000 bar=0"),
        "the acquisition must map BAR0 with the C04 literals; got:\n{stdout}"
    );

    // Config-space surface (A-11 both directions): header reads across
    // widths 2/2/1/4 at offsets 0x00/0x02/0x08/0x10, and EXACTLY ONE config
    // write — the benign CLS byte at 0x0C, value 0x10 (D10/O2).
    for line in [
        "BUS config_read off=0x0 width=2 = 0x0",
        "BUS config_read off=0x2 width=2 = 0x0",
        "BUS config_read off=0x8 width=1 = 0x0",
        "BUS config_read off=0x10 width=4 = 0x0",
        "BUS config_write off=0xc width=1 val=0x10",
    ] {
        assert!(
            stdout.contains(line),
            "oracle trace must carry the config op `{line}`; got:\n{stdout}"
        );
    }
    assert_eq!(
        stdout.matches("BUS config_write").count(),
        1,
        "exactly ONE benign config write (D10); got:\n{stdout}"
    );

    // Donor MMIO patterns at the real 82540EM offsets, based at 0xf0000000:
    // CTRL RMW (R then W @0x0000 with SLU set), the TDBAH-then-TDBAL
    // high-then-low split, the EMPTY-ring doorbell (TDT=0 — the L2-safety
    // invariant), the TCTL.EN RMW + field readback, the runtime-index array
    // writes (mta[3] @0x520c, vfta[5] @0x5614, ra[1] @0x5408/0x540c with the
    // AV bit), and the read-clear ICR read.
    for line in [
        "R32 @0xf0000000",                 // CTRL RMW read
        "W32 @0xf0000000 = 0x00000040",    // CTRL write: SLU over regfile 0
        "W32 @0xf0003804 = 0x00000001",    // TDBAH (HIGH first)
        "W32 @0xf0003800 = 0x00002000",    // TDBAL (LOW second)
        "W32 @0xf0003808 = 0x00000080",    // TDLEN
        "W32 @0xf0003818 = 0x00000000",    // TDT doorbell, ring EMPTY
        "R32 @0xf0000400",                 // TCTL RMW read
        "W32 @0xf0000400 = 0x00000002",    // TCTL.EN write
        "W32 @0xf000520c = 0x00000000",    // mta[3] (runtime index)
        "W32 @0xf0005614 = 0x00000000",    // vfta[5] (runtime index)
        "W32 @0xf0005408 = 0xaabbccdd",    // ra[1].ral (array-of-struct)
        "W32 @0xf000540c = 0x80000000",    // ra[1].rah AV (write_field)
        "R32 @0xf00000c0",                 // ICR read-clear
        "W32 @0xf00000d8 = 0xffffffff",    // deinit: IMC mask-all
        "W32 @0xf0000400 = 0x00000000",    // deinit: TCTL quiesce
    ] {
        assert!(
            stdout.contains(line),
            "oracle trace must carry the MMIO event `{line}`; got:\n{stdout}"
        );
    }
    // Program order: TDBAH strictly before TDBAL (the donor's 64-bit split).
    let tdbah_pos = stdout.find("W32 @0xf0003804").expect("TDBAH event");
    let tdbal_pos = stdout.find("W32 @0xf0003800").expect("TDBAL event");
    assert!(
        tdbah_pos < tdbal_pos,
        "TX ring base must program HIGH (0x3804) before LOW (0x3800); got:\n{stdout}"
    );

    // Lifecycle + diagnostics: the orderly scenario with the four log levels'
    // reachable lines (error is the readback-fail path, dynamically absent).
    for line in [
        "LC call on_init",
        "e1000 probe start",
        // C's `%#x` alternate form prints zero WITHOUT the 0x prefix (unlike
        // Rust's `{:#x}`) — the oracle's zero-seeded config space renders so.
        "e1000 cfg 0:0 rev 0 bar0 0",
        "e1000 up",
        "LC teardown present=1",
        "LC call on_deinit",
        "e1000 deinit: masking interrupts",
    ] {
        assert!(
            stdout.contains(line),
            "oracle trace must carry `{line}`; got:\n{stdout}"
        );
    }

    // RECOGNITION SMOKE (T01 acceptance, re-pinned): re-bridge for model-fact
    // assertions — strictly AFTER the snapshot verdict so the extra bridge
    // cannot perturb the interner history the snapshot depends on.
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
/// acceptance; asserted on both the bless and verify paths).
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
    // The CONVERGED match set (backend-zephyr-c-e1000 D01): the campaign
    // 82540EM id + the 82545EM sibling row first, the zephyr DT entry LAST.
    assert_eq!(driver.descriptor.match_set.len(), 3, "Pci, Pci, DT");
    assert_eq!(
        &driver.descriptor.match_set[..2],
        &[
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
    let MatchEntry::Devicetree { compatible } = &driver.descriptor.match_set[2] else {
        panic!("the converged DT entry is appended LAST (D01)");
    };
    assert_eq!(model.strings.resolve(*compatible), "regseq,e1000");
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
