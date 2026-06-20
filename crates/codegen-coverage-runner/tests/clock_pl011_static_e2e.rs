//! The peripheral-clock-management M1 behavioral-oracle gate (clock construct,
//! PLAN T13): the `clock-pl011-static` fixture passes the six-stage dual oracle
//! in a DEDICATED clean process (SOLE test in its binary, like the managed
//! pair member). It witnesses the static const-fold (D07): the SAME author body
//! as the managed fixture lowers with NO `CLK enable`/`disable`/`rate` hook calls
//! (enable/disable are no-ops; get_rate const-folds to `default_frequency`),
//! while the compiler-managed `CLK acquire`/`release` still frame the lifetime.
//! The folded rate (48 MHz) appears in the register write, visibly distinct from
//! the managed pair member's live 24 MHz — the behavioral-oracle distinction
//! (D09). This is a behavioral-oracle test, NOT a backend-emission witness.
//!
//! `BLESS=1` writes `golden.c` + `expected.stdout` from a single clean
//! bind+emit+run, then returns without also verifying.

#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::{Path, PathBuf};

use codegen_coverage_runner::{manifest, pipeline, toolchain};
use test_ir_fixtures::{try_bridge_fixture_path, BridgeOutcome};

/// The host `cc`, matching the runner's default.
fn cc() -> &'static Path {
    Path::new("cc")
}

fn clock_corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/_clock")
}

#[test]
fn clock_pl011_static_oracle() {
    let dir = clock_corpus().join("clock-pl011-static");
    let fixtures = manifest::discover(&clock_corpus()).expect("discover clock corpus");
    let fixture = fixtures
        .iter()
        .find(|f| f.manifest.name == "clock-pl011-static")
        .expect("clock-pl011-static fixture discovered");

    assert!(
        fixture.platform.is_some(),
        "clock-pl011-static must carry platform.toml (the [clock.<id>] join, C01)"
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
        "clock-pl011-static must pass all six stages: {:?}",
        result.verdict
    );

    // PLACEMENT GATES on the committed golden (D07, the backend-owned fold):
    // acquire/release still emit (with the frequency); enable/disable/get_rate
    // do NOT route to the host hooks (no-op / const-fold).
    let golden = std::fs::read_to_string(dir.join("golden.c")).expect("read golden.c");
    assert!(
        golden.contains("regseq_clock_acquire(\"uartclk\", 48000000u);"),
        "acquire_clock still records the static frequency (C07/D07)"
    );
    assert!(
        golden.contains("regseq_clock_release(\"uartclk\");"),
        "the compiler-managed release postamble still fires for a static clock (C07)"
    );
    // The CLOCK_HELPERS prelude DEFINES all five hooks (gated on resource
    // presence), so check for the CALL form (`("uartclk")`) — static enable/
    // disable/get_rate must not be CALLED (acquire/release still are).
    assert!(
        !golden.contains("regseq_clock_enable(\"uartclk\")")
            && !golden.contains("regseq_clock_disable(\"uartclk\")")
            && !golden.contains("regseq_clock_get_rate(\"uartclk\")"),
        "static enable/disable/get_rate must const-fold/no-op — NO host hook CALLS (D07/D10)"
    );
    assert!(
        golden.contains("48000000u"),
        "static get_rate must const-fold to the literal default_frequency (D07/D10)"
    );

    // BEHAVIORAL GATES: the acquire/release framing is present; the enable/
    // disable/rate hook lines are ABSENT (folded); the folded 48 MHz rate
    // appears in the register write.
    let stdout =
        std::fs::read_to_string(dir.join("expected.stdout")).expect("read expected.stdout");
    let pos = |needle: &str| -> usize {
        stdout
            .find(needle)
            .unwrap_or_else(|| panic!("expected.stdout must contain {needle:?}; got:\n{stdout}"))
    };
    let acquire = pos("CLK acquire uartclk");
    let write = pos("W32 @0x9000000 = 0x02dc6c00");
    let release = pos("CLK release uartclk");
    assert!(
        acquire < write && write < release,
        "the static oracle frames the folded-rate write between acquire and release"
    );
    assert!(
        !stdout.contains("CLK enable")
            && !stdout.contains("CLK disable")
            && !stdout.contains("CLK rate"),
        "static enable/disable/get_rate emit NO CLK hook trace (no-op / folded — D07/D09):\n{stdout}"
    );
}
