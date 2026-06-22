//! The pin-controller-and-GPIO-configuration optional-PRESENT generic-c oracle,
//! TRAIT-PATH ONLY (PLAN M4/T23): the `pinctrl-pl011-optional-bound-trait` fixture
//! proves the `OptionalPinCtrl` extension-trait call path reaches the bound
//! resource when the `[pinctrl.<id>]` section is present — BOTH author applies go
//! through the trait (`apply` directly on `Option<PinCtrl>`), no plain `if let`
//! path. Present ⇒ no fold: both emit `regseq_pinctrl_apply`. This is the
//! first-class generic-c sibling of `pinctrl-pl011-optional-bound` whose lowering
//! the Linux/Zephyr emission witnesses reuse (the plain-`if let` switchInt/Downcast
//! is the T15 generic-c shape; real-backend `if let` is design-in).
//!
//! Sole test in its binary (the emit's MIR is sensitive to rustc's process-global
//! interner history, like `clock_pl011_managed_e2e`). `BLESS=1` writes
//! `golden.c` + `expected.stdout` from one clean bind+emit+run.

#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::{Path, PathBuf};

use codegen_coverage_runner::{manifest, pipeline, toolchain};
use test_ir_fixtures::{try_bridge_fixture_path, BridgeOutcome};

fn cc() -> &'static Path {
    Path::new("cc")
}

fn pinctrl_corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/_pinctrl")
}

#[test]
fn pinctrl_pl011_optional_bound_trait_oracle() {
    let dir = pinctrl_corpus().join("pinctrl-pl011-optional-bound-trait");
    let fixtures = manifest::discover(&pinctrl_corpus()).expect("discover pinctrl corpus");
    let fixture = fixtures
        .iter()
        .find(|f| f.manifest.name == "pinctrl-pl011-optional-bound-trait")
        .expect("pinctrl-pl011-optional-bound-trait fixture discovered");

    assert!(
        fixture.platform.is_some(),
        "pinctrl-pl011-optional-bound-trait must carry platform.toml (the present [pinctrl.<id>] join)"
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

    let result = pipeline::run_fixture(fixture, cc());
    assert!(
        result.is_green(),
        "pinctrl-pl011-optional-bound-trait must pass all six stages: {:?}",
        result.verdict
    );

    let golden = std::fs::read_to_string(dir.join("golden.c")).expect("read golden.c");
    // BOTH trait-path applies route to the bound hook (no fold): "sleep" before
    // the write, "default" after.
    assert!(
        golden.contains("regseq_pinctrl_apply(\"uart_pins\", \"sleep\")"),
        "the trait path must route apply::<SleepState>() to the bound hook"
    );
    assert!(
        golden.contains("regseq_pinctrl_apply(\"uart_pins\", \"default\")"),
        "the trait path must route apply::<DefaultState>() to the bound hook"
    );
    assert!(
        !golden.contains("optional-absent fold"),
        "a PRESENT optional resource must not emit the Err(PinctrlAbsent) fold"
    );

    let stdout =
        std::fs::read_to_string(dir.join("expected.stdout")).expect("read expected.stdout");
    let pos = |needle: &str| -> usize {
        stdout
            .find(needle)
            .unwrap_or_else(|| panic!("expected.stdout must contain {needle:?}; got:\n{stdout}"))
    };
    let acquire = pos("PIN acquire uart_pins default");
    let on_init = pos("LC call on_init");
    let apply_sleep = pos("PIN apply uart_pins sleep");
    let write = pos("W32 @0x9000000 = 0x00000042");
    let apply_default = pos("PIN apply uart_pins default");
    let release = pos("PIN release uart_pins");
    assert!(
        acquire < on_init,
        "the compiler-managed acquire (default) precedes the first author body op (C07/D08)"
    );
    assert!(
        on_init < apply_sleep && apply_sleep < write,
        "the trait apply (sleep) must precede the register write"
    );
    assert!(
        write < apply_default,
        "the trait apply (default) follows the register write (the fixture's order)"
    );
    assert!(
        apply_default < release,
        "both author applies precede the compiler-managed release (LIFO, C07/D08)"
    );
}
