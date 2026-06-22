//! The pin-controller-and-GPIO-configuration M1 behavioral-oracle gate (pinctrl
//! construct, PLAN T08): the `pinctrl-pl011-required-bound-min` fixture passes
//! the six-stage dual oracle in a DEDICATED clean process (the emit's MIR is
//! sensitive to rustc's process-global interner history — so each golden-snapshot
//! test is the SOLE test in its binary, like `clock_pl011_managed_e2e`). The
//! blessed `expected.stdout` is the PIN trace contract made observable (C05): the
//! compiler-managed `PIN acquire uart_pins default` rides the preamble after the
//! bus/map_region keystone (and BEFORE any clock acquire — pins-before-clock,
//! D08); the author `PIN apply uart_pins sleep` precedes the first register write
//! (the distinct default/sleep states prove the generic-`S`->state-name
//! resolution, D04/C03); `PIN release uart_pins` runs LIFO at teardown. This
//! fixture FREEZES the `PIN …` trace spelling (OI-3).
//!
//! `BLESS=1` writes `golden.c` + `expected.stdout` from a single clean
//! bind+emit+run, then returns without also verifying (the keystone pattern).

#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::{Path, PathBuf};

use codegen_coverage_runner::{manifest, pipeline, toolchain};
use test_ir_fixtures::{try_bridge_fixture_path, BridgeOutcome};

/// The host `cc`, matching the runner's default.
fn cc() -> &'static Path {
    Path::new("cc")
}

fn pinctrl_corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/_pinctrl")
}

#[test]
fn pinctrl_pl011_required_bound_min_oracle() {
    let dir = pinctrl_corpus().join("pinctrl-pl011-required-bound-min");
    let fixtures = manifest::discover(&pinctrl_corpus()).expect("discover pinctrl corpus");
    let fixture = fixtures
        .iter()
        .find(|f| f.manifest.name == "pinctrl-pl011-required-bound-min")
        .expect("pinctrl-pl011-required-bound-min fixture discovered");

    assert!(
        fixture.platform.is_some(),
        "pinctrl-pl011-required-bound-min must carry platform.toml (the [pinctrl.<id>] join, C01)"
    );

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

    // Stages 1-6 (the process's first — interner-clean — bridge).
    let result = pipeline::run_fixture(fixture, cc());
    assert!(
        result.is_green(),
        "pinctrl-pl011-required-bound-min must pass all six stages: {:?}",
        result.verdict
    );

    // PLACEMENT GATES on the committed golden (C07 structural form): the
    // compiler-managed acquire (applying "default") rides the preamble AFTER
    // map_region; the author apply routes to the host hook with the resolved
    // "sleep" state; the release postamble fires.
    let golden = std::fs::read_to_string(dir.join("golden.c")).expect("read golden.c");
    let map_at = golden
        .find("regseq_bus_map_region(0x9000000ull")
        .expect("golden.c maps the AMBA region");
    // The REQUIRED form now consumes the acquire rc and hard-fails (C04/D03/T18):
    // `if (regseq_pinctrl_acquire("uart_pins", "default") != 0) { … return; }`.
    let acquire_at = golden
        .find("if (regseq_pinctrl_acquire(\"uart_pins\", \"default\") != 0)")
        .expect("golden.c synthesizes the required acquire_pinctrl with its rc-check");
    assert!(
        map_at < acquire_at,
        "acquire_pinctrl must follow the map_region keystone (C07/D08)"
    );
    assert!(
        golden.contains("regseq_pinctrl_apply(\"uart_pins\", \"sleep\")"),
        "the author apply must route to the host hook with the resolved \"sleep\" state (C04/D04)"
    );
    assert!(
        golden.contains("regseq_pinctrl_release(\"uart_pins\");"),
        "the compiler-managed release_pinctrl postamble must fire (C07/D08)"
    );

    // BEHAVIORAL GATES (dual oracle) on the PIN interleaving.
    let stdout =
        std::fs::read_to_string(dir.join("expected.stdout")).expect("read expected.stdout");
    let pos = |needle: &str| -> usize {
        stdout
            .find(needle)
            .unwrap_or_else(|| panic!("expected.stdout must contain {needle:?}; got:\n{stdout}"))
    };
    let acquire = pos("PIN acquire uart_pins default");
    let on_init = pos("LC call on_init");
    let apply = pos("PIN apply uart_pins sleep");
    let write = pos("W32 @0x9000000 = 0x00000042");
    let release = pos("PIN release uart_pins");
    assert!(
        acquire < on_init,
        "the compiler-managed acquire (default) must precede the first author body op (C07/D08)"
    );
    assert!(
        on_init < apply && apply < write,
        "the author PIN apply (sleep) must precede the first register write (C05/D11)"
    );
    assert!(
        apply < release,
        "the author apply must precede the compiler-managed release (LIFO, C07/D08)"
    );
    assert!(
        acquire < release,
        "acquire/release frame the pinctrl resource's lifetime"
    );
}
