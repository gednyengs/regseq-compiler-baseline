//! Peripheral-clock-management COMPOSED-interaction oracle (clock construct,
//! PLAN T17): the `clock-pl011-composed` fixture — region + MANAGED clock +
//! interrupt on one driver — passes the generic-c six-stage oracle, and the
//! blessed `expected.stdout` witnesses the C07 phase ordering at RUNTIME:
//!
//!   preamble : BUS map  ->  CLK acquire  ->  IRQ register      (map -> clock -> irq)
//!   teardown : IRQ unregister  ->  CLK release  ->  LC postamble (exact reverse)
//!
//! The static per-target placement is witnessed separately by
//! `clock_composed_witness_e2e` (linux_c / zephyr_c). `BLESS=1` writes
//! `golden.c` + `expected.stdout`. SOLE test in this binary (interner isolation).

#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::{Path, PathBuf};

use codegen_coverage_runner::{manifest, pipeline, toolchain};
use test_ir_fixtures::{try_bridge_fixture_path, BridgeOutcome};

fn cc() -> &'static Path {
    Path::new("cc")
}

fn clock_corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/_clock")
}

#[test]
fn clock_composed_oracle() {
    let dir = clock_corpus().join("clock-pl011-composed");
    let fixtures = manifest::discover(&clock_corpus()).expect("discover clock corpus");
    let fixture = fixtures
        .iter()
        .find(|f| f.manifest.name == "clock-pl011-composed")
        .expect("clock-pl011-composed fixture discovered");

    if std::env::var_os("BLESS").is_some() {
        let options = pipeline::backend_options_for(fixture).expect("backend options");
        let mut model = match try_bridge_fixture_path(&fixture.input) {
            BridgeOutcome::Ok(m) => m,
            other => panic!("bless bridge failed: {other:?}"),
        };
        let identity = pipeline::bind_l2_platform(&mut model, fixture).expect("L2 bind (bless)");
        let context = regseq_backends::BackendContext::new(options, identity);
        let emit = regseq_backends::emit_generic_c_for_entry_with_context(
            &model,
            &fixture.manifest.entry,
            &context,
        )
        .expect("emit must succeed");
        let run = toolchain::compile_and_run(&emit.tu, &emit.shim, cc())
            .expect("compile + run must succeed");
        std::fs::write(&fixture.golden_c, &emit.tu).expect("write golden.c");
        std::fs::write(&fixture.expected_stdout, &run.stdout).expect("write expected.stdout");
        return;
    }

    let result = pipeline::run_fixture(fixture, cc());
    assert!(
        result.is_green(),
        "clock-pl011-composed must pass all six stages: {:?}",
        result.verdict
    );

    let stdout =
        std::fs::read_to_string(dir.join("expected.stdout")).expect("read expected.stdout");
    let pos = |needle: &str| -> usize {
        stdout
            .find(needle)
            .unwrap_or_else(|| panic!("expected.stdout must contain {needle:?}; got:\n{stdout}"))
    };
    let rpos = |needle: &str| -> usize {
        stdout
            .rfind(needle)
            .unwrap_or_else(|| panic!("expected.stdout must contain {needle:?}; got:\n{stdout}"))
    };

    // Preamble: map_region -> acquire_clock -> interrupt-register (C07/D16).
    let map = pos("BUS map base=0x9000000");
    let acquire = pos("CLK acquire uartclk");
    let irq_register = pos("IRQ register line=37");
    let on_init = pos("LC call on_init");
    assert!(
        map < acquire && acquire < irq_register && irq_register < on_init,
        "preamble must order map_region -> acquire_clock -> interrupt-register -> on_init (C07)"
    );

    // Teardown: interrupt-deregister -> clock-release (exact reverse of preamble).
    let on_deinit = pos("LC call on_deinit");
    let irq_unregister = rpos("IRQ unregister line=37");
    let release = pos("CLK release uartclk");
    let postamble = pos("LC postamble");
    assert!(
        on_deinit < irq_unregister && irq_unregister < release && release < postamble,
        "teardown must order on_deinit -> interrupt-deregister -> clock-release -> unmap (C07 reverse)"
    );
}
