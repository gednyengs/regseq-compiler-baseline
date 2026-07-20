//! The pin-controller-and-GPIO-configuration M3 required-PRESENT full-sequence
//! oracle (PLAN T16): the `pinctrl-pl011-required-bound` fixture exercises the
//! REQUIRED `PinCtrl` form (D03) with TWO author state transitions (sleep ->
//! default) on the direct `&PinCtrl` receiver, proving repeated `apply::<S>()`
//! state resolution beyond the M1 single-apply `-min`. acquire_pinctrl applies
//! "default"; the author applies "sleep" then "default"; release runs LIFO.
//!
//! Sole test in its binary (interner isolation). `BLESS=1` writes the goldens.

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
fn pinctrl_pl011_required_bound_oracle() {
    let dir = pinctrl_corpus().join("pinctrl-pl011-required-bound");
    let fixtures = manifest::discover(&pinctrl_corpus()).expect("discover pinctrl corpus");
    let fixture = fixtures
        .iter()
        .find(|f| f.manifest.name == "pinctrl-pl011-required-bound")
        .expect("pinctrl-pl011-required-bound fixture discovered");

    assert!(
        fixture.platform.is_some(),
        "required-bound carries platform.toml"
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
        "pinctrl-pl011-required-bound must pass all six stages: {:?}",
        result.verdict
    );

    let golden = std::fs::read_to_string(dir.join("golden.c")).expect("read golden.c");
    // Required form: no Some-materialization (PinCtrl is a ZST, no Option tag) and
    // no absent fold (a required resource is always resolved).
    assert!(
        !golden.contains("present materialization"),
        "the required (ZST) form has no Option discriminant to materialize"
    );
    assert!(
        !golden.contains("optional-absent fold"),
        "a required resource is never absent"
    );

    let stdout =
        std::fs::read_to_string(dir.join("expected.stdout")).expect("read expected.stdout");
    let pos = |needle: &str| -> usize {
        stdout
            .find(needle)
            .unwrap_or_else(|| panic!("expected.stdout must contain {needle:?}; got:\n{stdout}"))
    };
    let acquire = pos("PIN acquire uart_pins default");
    let apply_sleep = pos("PIN apply uart_pins sleep");
    let apply_default = pos("PIN apply uart_pins default");
    let write = pos("W32 @0x9000000 = 0x00000042");
    let release = pos("PIN release uart_pins");
    assert!(
        acquire < apply_sleep && apply_sleep < apply_default && apply_default < write,
        "the required form transitions default(acquire) -> sleep -> default before the write"
    );
    assert!(write < release, "release runs LIFO at teardown (C07/D08)");
}
