//! The CONVERGED E1000 fixture under the ZEPHYR binding
//! (backend-zephyr-c-e1000 T06/C05): the ONE `input.rs` — D01 converged
//! match set — bridged against `platform.zephyr.toml` (qemu_x86, bar 0,
//! dead base/size literals) through the certified six-stage generic_c path:
//! bridge → L2 bind → emit → snapshot → cc → run → stdout oracle.
//!
//! The blessed `expected.zephyr.stdout` is the COMMITTED ZEPHYR-LEG
//! CANONICAL ORACLE TRACE (D10): the zephyr L3 trace differential compares
//! the generated Zephyr PCIe driver's QEMU capture against it.
//!
//! `BLESS=1` writes `golden.zephyr-oracle.c` + `expected.zephyr.stdout`
//! (the zephyr-binding C05 oracle pair — never the linux pair) from a
//! single clean bind+emit+run — the established F-1 SOLE-BINARY EXCEPTION:
//! generic_c SOURCE stays frozen; only this invoking binary blesses these
//! two artifacts. The bless happened strictly AFTER the T05 R08
//! unblessed-first proof.
//!
//! SOLE test in this binary (interner isolation, like
//! `pl011_composed_zephyr_e2e`).

#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::{Path, PathBuf};

use codegen_coverage_runner::{manifest, pipeline, toolchain};
use regseq_model::TargetOs;
use test_ir_fixtures::{try_bridge_fixture_path, BridgeOutcome};

/// The host `cc`, matching the runner's default.
fn cc() -> &'static Path {
    Path::new("cc")
}

fn composed_corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/_composed")
}

#[test]
fn e1000_composed_zephyr_six_stage_oracle() {
    let fixtures = manifest::discover(&composed_corpus()).expect("discover composed corpus");
    let fixture = fixtures
        .iter()
        .find(|f| f.manifest.name == "e1000-composed")
        .expect("e1000-composed fixture discovered")
        .with_binding(manifest::OsBinding::Zephyr)
        .expect("the converged fixture resolves the zephyr binding (C05)");
    let fixture = &fixture;

    // The bound fixture MUST carry its per-OS platform file: per the
    // ratified A-10 policy the L2 literals are the resolution trigger.
    assert!(
        fixture.platform.is_some(),
        "e1000-composed must resolve platform.zephyr.toml (A-10 trigger, C05)"
    );

    if std::env::var_os("BLESS").is_some() {
        // The bless path applies the IDENTICAL L2 bind + identity resolution
        // run_fixture does, so the blessed golden reflects the BOUND model
        // and the traced `[platform]` identity.
        let options = pipeline::backend_options_for(fixture).expect("backend options");
        let mut model = match try_bridge_fixture_path(&fixture.input) {
            BridgeOutcome::Ok(m) => m,
            other => panic!("bless bridge failed: {other:?}"),
        };
        let identity = pipeline::bind_l2_platform(&mut model, fixture).expect("L2 bind (bless)");
        assert_eq!(identity.os, Some(TargetOs::Zephyr));
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
        "e1000-composed (zephyr binding) must pass all six stages: {:?}",
        result.verdict
    );

    // COMPOSITION GATES on the committed golden: the PCI acquisition
    // preamble feeding map_region with the zephyr-binding platform literals
    // (A-10: L2 is the trigger; bar=0), under the traced qemu_x86/x86
    // Zephyr identity.
    let golden =
        std::fs::read_to_string(&fixture.golden_c).expect("read golden.zephyr-oracle.c");
    assert!(
        golden.contains("regseq_bus_map_region(0xf0000000ull, 0x20000ull, 0);"),
        "golden must map the declared (base, size, bar) from platform.zephyr.toml"
    );
    assert!(
        golden.contains("regseq_bus_enable_device();"),
        "golden must CALL the PCI-only enable_device in the acquisition preamble"
    );
    assert!(
        golden.contains("/* regseq target platform: os=zephyr board=qemu_x86 arch=x86 */"),
        "golden must carry the [platform] identity trace from platform.zephyr.toml"
    );

    // BEHAVIORAL GATES on the zephyr-leg canonical oracle trace: the
    // registration keys on match_set.first() (the pci row — normalizer
    // metadata, never compared by L3); the config-op and donor MMIO event
    // streams are the SAME spec behavior the linux-leg oracle pins (ONE
    // input.rs); teardown events included (authored on_deinit).
    let stdout = std::fs::read_to_string(&fixture.expected_stdout)
        .expect("read expected.zephyr.stdout");
    assert!(
        stdout.contains("LC register e1000 match=pci:0x8086:0x100e"),
        "the register marker keys on match_set.first() (D01); got:\n{stdout}"
    );
    for line in [
        "BUS enable_device",
        "BUS map base=0xf0000000 size=0x20000 bar=0",
        "BUS config_read off=0x0 width=2 = 0x0",
        "BUS config_read off=0x2 width=2 = 0x0",
        "BUS config_read off=0x8 width=1 = 0x0",
        "BUS config_read off=0x10 width=4 = 0x0",
        "BUS config_write off=0xc width=1 val=0x10",
        "W32 @0xf0003804 = 0x00000001",
        "W32 @0xf0003800 = 0x00002000",
        "W32 @0xf0003818 = 0x00000000",
        "LC call on_init",
        "e1000 probe start",
        "e1000 up",
        "LC teardown present=1",
        "LC call on_deinit",
        "e1000 deinit: masking interrupts",
        "W32 @0xf00000d8 = 0xffffffff",
    ] {
        assert!(
            stdout.contains(line),
            "the zephyr-leg oracle trace must carry `{line}`; got:\n{stdout}"
        );
    }
    assert_eq!(
        stdout.matches("BUS config_write").count(),
        1,
        "exactly ONE benign config write; got:\n{stdout}"
    );
}
