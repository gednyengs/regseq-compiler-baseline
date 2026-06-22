//! The pin-controller-and-GPIO-configuration M3 optional-PRESENT behavioral
//! oracle (PLAN T16): the `pinctrl-pl011-optional-bound` fixture proves BOTH
//! optional call paths (D02) reach the bound resource when the `[pinctrl.<id>]`
//! section is present. The plain `if let Some(p) = &self.pinctrl` local-tracing
//! path (D13/T15) applies "sleep" — recovered through the match binding back to
//! the `Option<PinCtrl>` field by the shared `recover_pinctrl_receiver`; the
//! `OptionalPinCtrl` extension-trait path applies "default" directly on the
//! `Option<PinCtrl>`. Present ⇒ no fold: both emit `regseq_pinctrl_apply`. The
//! D06 present-materialization sets the field's `Option` tag to `Some` so the
//! runtime `if let` switch takes the present branch.
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
fn pinctrl_pl011_optional_bound_oracle() {
    let dir = pinctrl_corpus().join("pinctrl-pl011-optional-bound");
    let fixtures = manifest::discover(&pinctrl_corpus()).expect("discover pinctrl corpus");
    let fixture = fixtures
        .iter()
        .find(|f| f.manifest.name == "pinctrl-pl011-optional-bound")
        .expect("pinctrl-pl011-optional-bound fixture discovered");

    assert!(
        fixture.platform.is_some(),
        "pinctrl-pl011-optional-bound must carry platform.toml (the present [pinctrl.<id>] join)"
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
        "pinctrl-pl011-optional-bound must pass all six stages: {:?}",
        result.verdict
    );

    let golden = std::fs::read_to_string(dir.join("golden.c")).expect("read golden.c");

    // The D06 present-materialization: the synthesized instance sets the
    // `Option<PinCtrl>` field's tag to `Some` (so the runtime `if let` switch
    // takes the present branch). Shape-derived, on `inst.inner.<field>`.
    assert!(
        golden.contains("Some (D06 present materialization)"),
        "optional-PRESENT must materialize the Option<PinCtrl> Some tag on the instance"
    );

    // BOTH call paths route to the bound hook (no fold): the if-let path applies
    // "sleep", the trait path applies "default".
    assert!(
        golden.contains("regseq_pinctrl_apply(\"uart_pins\", \"sleep\")"),
        "the if-let path must route apply::<SleepState>() to the bound hook"
    );
    assert!(
        golden.contains("regseq_pinctrl_apply(\"uart_pins\", \"default\")"),
        "the OptionalPinCtrl trait path must route apply::<DefaultState>() to the bound hook"
    );
    // Present ⇒ NO absent fold anywhere.
    assert!(
        !golden.contains("optional-absent fold"),
        "a PRESENT optional resource must not emit the Err(PinctrlAbsent) fold"
    );

    // Dual oracle on the PIN interleaving: acquire(default) before on_init; the
    // if-let apply(sleep) precedes the write; the trait apply(default) follows it;
    // release frames the lifetime.
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
        "the if-let apply (sleep) must precede the register write (D02/D13)"
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
