//! The composed PL011 oracle gate (backend-linux-c unit, M1/C2).
//!
//! The five refined construct surfaces (register-access, BRM-AMBA,
//! device-lifecycle, diagnostics-logging, RegseqError) flow the full certified
//! six-stage path together: bridge → L2 bind → emit → snapshot → cc → run →
//! stdout oracle. The blessed `expected.stdout` is the CANONICAL PL011 ORACLE
//! TRACE (GRILL D06): the L3 trace differential compares the generated Linux
//! driver's QEMU MMIO capture against it, so this file is doubly protected
//! (I04 + the L3 artifact).
//!
//! `BLESS=1` writes `golden.c` + `expected.stdout` from a single clean
//! bind+emit+run, then returns without also verifying (the keystone pattern).
//!
//! SOLE test in this binary (like `bus_keystone_e2e` / `lifecycle_pl011_e2e`):
//! the emit's MIR is sensitive to rustc's process-global interner history, so
//! the byte-snapshot is reproducible only in a dedicated clean process (D17).
//! Ordering inside the test preserves that: the verify path runs the pipeline
//! (its bridge is the process's FIRST bridge, matching the bless process)
//! before the recognition smoke re-bridges for model-fact assertions.
//!
//! `#![feature(rustc_private)]` + `extern crate rustc_driver;` link the
//! rustc-backed bridge (stage 1) into the test binary.

#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::{Path, PathBuf};

use codegen_coverage_runner::{manifest, pipeline, toolchain};
use regseq_model::{
    AttrData, BusType, FrameworkType, IntrinsicKind, LogLevel, MatchEntry, MmioOp, RegseqModel,
    TargetOs,
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
fn pl011_composed_six_stage_oracle() {
    let dir = composed_corpus().join("pl011-composed");
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
        "pl011-composed must pass all six stages: {:?}",
        result.verdict
    );

    // COMPOSITION GATES on the committed golden: the AMBA acquisition preamble
    // (claim_regions + get_handle, NO PCI enable_device) feeding map_region
    // with the real platform literals (A-10: L2 is the trigger; -1 = no BAR on
    // an AMBA region), under the traced qemu-virt/aarch64 Linux identity.
    let golden = std::fs::read_to_string(dir.join("golden.c")).expect("read golden.c");
    assert!(
        golden.contains("regseq_bus_map_region(0x9000000ull, 0x1000ull, -1);"),
        "golden.c must map the real declared AMBA (base, size) from platform.toml"
    );
    assert!(
        golden.contains("regseq_bus_claim_regions(-1);")
            && golden.contains("regseq_bus_get_handle();"),
        "golden.c must emit the every-bus acquisition preamble before map_region"
    );
    // The prelude always DEFINES the (unused) helper; what must be absent for
    // an AMBA driver is the preamble CALL.
    assert!(
        !golden.contains("regseq_bus_enable_device();"),
        "enable_device is PCI-only and must NOT be called for an AMBA driver"
    );
    assert!(
        golden.contains("/* regseq target platform: os=linux board=qemu-virt arch=aarch64 */"),
        "golden.c must carry the [platform] identity trace from platform.toml"
    );

    // BEHAVIORAL GATES (dual oracle) on the canonical PL011 oracle trace:
    // register access routes through the BASED accessor at the real mapped
    // QEMU virt addresses — CR (base + 0x30) programmed during bring-up, ICR
    // (base + 0x44) cleared — and the bring-up math lands the 24 MHz / 115200
    // divisors (IBRD 13, FBRD 1) in the info log line.
    let stdout =
        std::fs::read_to_string(dir.join("expected.stdout")).expect("read expected.stdout");
    assert!(
        stdout.contains("@0x9000030"),
        "oracle trace must show the based CR access (0x9000000 + 0x30); got:\n{stdout}"
    );
    assert!(
        stdout.contains("@0x9000044"),
        "oracle trace must show the based ICR access (0x9000000 + 0x44); got:\n{stdout}"
    );
    assert!(
        stdout.contains("pl011 baud divisors: 13 1") && stdout.contains("pl011 up"),
        "oracle trace must carry the bring-up log lines; got:\n{stdout}"
    );
    assert!(
        !stdout.contains("BUS enable_device"),
        "the AMBA acquisition must not trace the PCI-only enable_device; got:\n{stdout}"
    );

    // RECOGNITION SMOKE (T02): re-bridge for model-fact assertions — strictly
    // AFTER the snapshot verdict so the extra bridge cannot perturb the
    // interner history the snapshot depends on.
    let mut model = match try_bridge_fixture_path(&fixture.input) {
        BridgeOutcome::Ok(m) => m,
        other => panic!("pl011-composed must bridge cleanly: {other:?}"),
    };
    let identity = pipeline::bind_l2_platform(&mut model, fixture).expect("L2 bind + validate");
    assert_eq!(
        identity.os,
        Some(TargetOs::Linux),
        "[platform] identity must resolve os=linux"
    );
    assert_recognition_facts(&model);
}

/// The five-construct recognition facts on the bound model (T02 acceptance).
fn assert_recognition_facts(model: &RegseqModel) {
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
        vec![
            MatchEntry::Amba {
                id: 0x0004_1011,
                mask: 0x000f_ffff
            },
            MatchEntry::Amba {
                id: 0x00ab_c123,
                mask: 0x00ff_ffff
            },
        ],
        "the real PL011 periphid + the campaign's reserved test periphid"
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
