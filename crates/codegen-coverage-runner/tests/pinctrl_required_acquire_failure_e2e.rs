//! The pin-controller-and-GPIO-configuration M3 required-ACQUIRE-FAILURE oracle
//! (PLAN T18): the `pinctrl-required-acquire-failure` fixture proves the REQUIRED
//! form CONSUMES the acquire rc and HARD-FAILS (C04/D03). The harness forces
//! `acquire_pinctrl` to fail (`pinctrl_fail_acquire = "uart_pins"` →
//! `backend.pinctrl.fail-acquire`); the required form aborts init in the preamble:
//! the trace shows `PIN acquire uart_pins default` (the attempt) + `LC acquire-fail
//! pinctrl uart_pins`, and `on_init` is NEVER reached — no `LC call on_init`, no
//! `PIN apply`, no register write (I02: pinctrl never touched MMIO). This is the
//! generic-c half; the Linux/Zephyr halves are T22 (M4). Sole test in its binary
//! (interner isolation). `BLESS=1` writes the goldens.

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
fn pinctrl_required_acquire_failure_oracle() {
    let dir = pinctrl_corpus().join("pinctrl-required-acquire-failure");
    let fixtures = manifest::discover(&pinctrl_corpus()).expect("discover pinctrl corpus");
    let fixture = fixtures
        .iter()
        .find(|f| f.manifest.name == "pinctrl-required-acquire-failure")
        .expect("pinctrl-required-acquire-failure fixture discovered");

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
        "pinctrl-required-acquire-failure must pass all six stages: {:?}",
        result.verdict
    );

    // The generated source: the required rc-check + abort, and the armed injection.
    let golden = std::fs::read_to_string(dir.join("golden.c")).expect("read golden.c");
    assert!(
        golden.contains("regseq_pinctrl_fail_acquire_id = \"uart_pins\";"),
        "the harness must arm the acquire-failure injection for uart_pins (T18)"
    );
    // The id rides as a `%s` ARGUMENT, never the printf format string (injection
    // safe, Codex T18 fix): the golden SOURCE has `…pinctrl %s\n", "uart_pins"`.
    // (The runtime TRACE — asserted below — is the resolved `…pinctrl uart_pins`.)
    assert!(
        golden.contains("if (regseq_pinctrl_acquire(\"uart_pins\", \"default\") != 0)")
            && golden.contains("LC acquire-fail pinctrl %s\\n\", \"uart_pins\""),
        "the required form must consume the rc and hard-fail (abort) on failure"
    );

    // The behavioral oracle: acquire attempted + acquire-fail, then init aborts —
    // on_init never runs, no apply, no register write (I02).
    let stdout =
        std::fs::read_to_string(dir.join("expected.stdout")).expect("read expected.stdout");
    let acquire = stdout
        .find("PIN acquire uart_pins default")
        .expect("the required acquire is attempted (traced) before it fails");
    let fail = stdout
        .find("LC acquire-fail pinctrl uart_pins")
        .unwrap_or_else(|| panic!("the required acquire must hard-fail; got:\n{stdout}"));
    assert!(
        acquire < fail,
        "the acquire attempt traces before the hard-fail marker"
    );
    assert!(
        !stdout.contains("LC call on_init"),
        "init must abort BEFORE on_init (the required hard-fail); got:\n{stdout}"
    );
    assert!(
        !stdout.contains("PIN apply"),
        "no author apply runs after the acquire hard-fail"
    );
    assert!(
        !stdout.contains("W32 "),
        "I02: pinctrl never touched MMIO, and the aborted init never reached the author write"
    );
}
