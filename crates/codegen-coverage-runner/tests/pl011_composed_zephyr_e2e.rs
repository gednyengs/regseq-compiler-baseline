//! The CONVERGED PL011 fixture under the ZEPHYR binding (spec-convergence-
//! pl011 C05): the ONE `input.rs` — D09 converged match set, shared
//! `on_deinit` — bridged against `platform.zephyr.toml` (the eval DUT,
//! second PL011, base 0x09040000) through the certified six-stage generic_c
//! path: bridge → L2 bind → emit → snapshot → cc → run → stdout oracle.
//!
//! The blessed `expected.zephyr.stdout` is the COMMITTED ZEPHYR-LEG
//! CANONICAL ORACLE TRACE (GRILL D10): the zephyr L3 trace differential
//! compares the generated Zephyr driver's QEMU MMIO capture against it —
//! TEARDOWN EVENTS INCLUDED (the authored `on_deinit` quiesce writes), so
//! this file is doubly protected (I04 + the L3 artifact).
//!
//! `BLESS=1` writes `golden.zephyr-oracle.c` + `expected.zephyr.stdout`
//! (the zephyr-binding C05 artifact pair — never the linux pair) from a
//! single clean bind+emit+run, then returns without also verifying (the
//! keystone pattern).
//!
//! SOLE test in this binary (like `pl011_composed_e2e`): the emit's MIR is
//! sensitive to rustc's process-global interner history, so the
//! byte-snapshot is reproducible only in a dedicated clean process.
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
fn pl011_composed_zephyr_six_stage_oracle() {
    let fixtures = manifest::discover(&composed_corpus()).expect("discover composed corpus");
    let fixture = fixtures
        .iter()
        .find(|f| f.manifest.name == "pl011-composed")
        .expect("pl011-composed fixture discovered")
        .with_binding(manifest::OsBinding::Zephyr)
        .expect("the converged fixture resolves the zephyr binding (C05)");
    let fixture = &fixture;

    // The bound fixture MUST carry its per-OS platform file: per the ratified
    // A-10 policy the L2 literals are the resolution trigger (and the D11
    // emitted cross-check consumes them).
    assert!(
        fixture.platform.is_some(),
        "pl011-composed must resolve platform.zephyr.toml (A-10 trigger, C05)"
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
        assert_eq!(identity.os, Some(TargetOs::Zephyr));
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
        std::fs::write(&fixture.golden_c, &emit.tu).expect("write golden.zephyr-oracle.c");
        std::fs::write(&fixture.expected_stdout, &run.stdout)
            .expect("write expected.zephyr.stdout");
        return;
    }

    // Stages 1-6 (the process's first — interner-clean — bridge).
    let result = pipeline::run_fixture(fixture, cc());
    assert!(
        result.is_green(),
        "pl011-composed (zephyr binding) must pass all six stages: {:?}",
        result.verdict
    );

    // COMPOSITION GATES on the committed golden: the every-bus acquisition
    // preamble (claim_regions + get_handle, NO PCI enable_device) feeding
    // map_region with the real DUT platform literals (A-10: L2 is the
    // trigger; -1 = no BAR on an AMBA region), under the traced
    // qemu_cortex_a53/aarch64 Zephyr identity.
    let golden =
        std::fs::read_to_string(&fixture.golden_c).expect("read golden.zephyr-oracle.c");
    assert!(
        golden.contains("regseq_bus_map_region(0x9040000ull, 0x1000ull, -1);"),
        "golden.c must map the real declared DUT (base, size) from platform.toml"
    );
    assert!(
        golden.contains("regseq_bus_claim_regions(-1);")
            && golden.contains("regseq_bus_get_handle();"),
        "golden.c must emit the every-bus acquisition preamble before map_region"
    );
    // The prelude always DEFINES the (unused) helper; what must be absent for
    // a non-PCI driver is the preamble CALL.
    assert!(
        !golden.contains("regseq_bus_enable_device();"),
        "enable_device is PCI-only and must NOT be called for this driver"
    );
    assert!(
        golden
            .contains("/* regseq target platform: os=zephyr board=qemu_cortex_a53 arch=aarch64 */"),
        "golden.c must carry the [platform] identity trace from platform.toml"
    );

    // BEHAVIORAL GATES (dual oracle) on the Zephyr-leg canonical oracle
    // trace: the Devicetree match registers; register access routes through
    // the BASED accessor at the real DUT addresses — CR (base + 0x30)
    // programmed during bring-up, ICR (base + 0x44) cleared — the bring-up
    // math lands the 24 MHz / 115200 divisors (IBRD 13, FBRD 1); and the
    // teardown suffix carries NO on_deinit call (GRILL risk #9 / D10's cited
    // whitelist entry class).
    let stdout = std::fs::read_to_string(&fixture.expected_stdout)
        .expect("read expected.zephyr.stdout");
    assert!(
        stdout.contains("match=amba:0x41011/0xfffff"),
        "the register marker keys on match_set.first() = the amba row (D09; \
         normalizer metadata, never compared by L3); got:\n{stdout}"
    );
    assert!(
        stdout.contains("@0x9040030"),
        "oracle trace must show the based CR access (0x9040000 + 0x30); got:\n{stdout}"
    );
    assert!(
        stdout.contains("@0x9040044"),
        "oracle trace must show the based ICR access (0x9040000 + 0x44); got:\n{stdout}"
    );
    assert!(
        stdout.contains("pl011 baud divisors: 13 1") && stdout.contains("pl011 up"),
        "oracle trace must carry the bring-up log lines; got:\n{stdout}"
    );
    assert!(
        stdout.contains("LC teardown present=1") && stdout.contains("LC call on_deinit"),
        "the orderly teardown suffix with the authored on_deinit; got:\n{stdout}"
    );
    assert!(
        stdout.contains("pl011 deinit: masking interrupts"),
        "the authored on_deinit body runs (its warn! log line); got:\n{stdout}"
    );
    // The three quiesce writes land at the DUT base (teardown events the L3
    // differential now compares, C07): IMSC<-0x0, CR<-disabled, ICR<-0x7ff.
    let teardown = stdout
        .split("LC call on_deinit")
        .nth(1)
        .expect("teardown segment");
    assert!(
        teardown.contains("@0x9040038") // IMSC = base + 0x38
            && teardown.contains("@0x9040030") // CR   = base + 0x30
            && teardown.contains("@0x9040044"), // ICR = base + 0x44
        "the on_deinit quiesce writes at the DUT base; got:\n{teardown}"
    );
    assert!(
        !stdout.contains("BUS enable_device"),
        "the acquisition must not trace the PCI-only enable_device; got:\n{stdout}"
    );

    // RECOGNITION SMOKE (T01): re-bridge for model-fact assertions — strictly
    // AFTER the snapshot verdict so the extra bridge cannot perturb the
    // interner history the snapshot depends on.
    let mut model = match try_bridge_fixture_path(&fixture.input) {
        BridgeOutcome::Ok(m) => m,
        other => panic!("pl011-composed (zephyr binding) must bridge cleanly: {other:?}"),
    };
    let identity = pipeline::bind_l2_platform(&mut model, fixture).expect("L2 bind + validate");
    assert_eq!(
        identity.os,
        Some(TargetOs::Zephyr),
        "[platform] identity must resolve os=zephyr"
    );
    assert_recognition_facts(&model);
}

/// The five-construct recognition facts on the bound model (the converged
/// fixture under the zephyr binding: D09 match set, shared `on_deinit`, the
/// DUT base).
fn assert_recognition_facts(model: &RegseqModel) {
    // -- device-lifecycle: one recognized driver carrying the CONVERGED D09
    //    match set, with on_init AND on_deinit overridden (shared text).
    assert_eq!(
        model.device_lifecycle.drivers.len(),
        1,
        "exactly one recognized driver"
    );
    let driver = model.device_lifecycle.drivers.values().next().unwrap();
    assert_eq!(
        driver.descriptor.match_set.len(),
        3,
        "the converged match set (amba, amba, dt — D09)"
    );
    match &driver.descriptor.match_set[2] {
        MatchEntry::Devicetree { compatible } => {
            assert_eq!(
                model.strings.resolve(*compatible),
                "regseq,pl011",
                "the D04 reserved compatible string, last (D09)"
            );
        }
        other => panic!("expected the devicetree entry last (D09), got {other:?}"),
    }
    assert!(driver.ops.on_init.is_some(), "on_init overridden");
    assert!(
        driver.ops.on_deinit.is_some(),
        "on_deinit authored as plain shared text (the converged spec)"
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
    //    tables must carry all five MMIO op kinds, all FOUR log levels (the
    //    `warn!` site lives in the now-shared `on_deinit`), and the
    //    RegseqError framework-type marker.
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
    assert!(regseq_error_marked, "RegseqError framework type recognized");
}
