//! The peripheral-clock-management M1 behavioral-oracle gate (clock construct,
//! PLAN T08): the `clock-pl011-managed` fixture passes the six-stage dual oracle
//! in a DEDICATED clean process (the emit's MIR is sensitive to rustc's
//! process-global interner history — so each golden-snapshot test is the SOLE
//! test in its binary, like `irq_pl011_e2e`). The blessed `expected.stdout` is
//! the CLK trace contract made observable (C05): the compiler-managed
//! `CLK acquire` rides the preamble after the bus/map_region keystone; the
//! author `CLK enable` precedes the first register write; the managed
//! `CLK rate` is the live recorded rate (no fold, D10/R10); `CLK disable`
//! (author quiesce) precedes the compiler-managed `CLK release` (LIFO, D16).
//! This fixture FREEZES the `CLK …` trace spelling (OI-3).
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

fn clock_corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/_clock")
}

#[test]
fn clock_pl011_managed_oracle() {
    let dir = clock_corpus().join("clock-pl011-managed");
    let fixtures = manifest::discover(&clock_corpus()).expect("discover clock corpus");
    let fixture = fixtures
        .iter()
        .find(|f| f.manifest.name == "clock-pl011-managed")
        .expect("clock-pl011-managed fixture discovered");

    assert!(
        fixture.platform.is_some(),
        "clock-pl011-managed must carry platform.toml (the [clock.<id>] join, C01)"
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
        "clock-pl011-managed must pass all six stages: {:?}",
        result.verdict
    );

    // PLACEMENT GATES on the committed golden (C07 structural form): the
    // compiler-managed acquire (with the resolved frequency) rides the preamble
    // AFTER map_region; the managed ops route to the host hooks; the release
    // postamble fires.
    let golden = std::fs::read_to_string(dir.join("golden.c")).expect("read golden.c");
    let map_at = golden
        .find("regseq_bus_map_region(0x9000000ull")
        .expect("golden.c maps the AMBA region");
    let acquire_at = golden
        .find("regseq_clock_acquire(\"uartclk\", 24000000u);")
        .expect("golden.c synthesizes acquire_clock with the resolved frequency");
    assert!(
        map_at < acquire_at,
        "acquire_clock must follow the map_region keystone (C07/D16)"
    );
    assert!(
        golden.contains("regseq_clock_enable(\"uartclk\")"),
        "managed enable must route to the host hook (C04)"
    );
    assert!(
        golden.contains("regseq_clock_get_rate(\"uartclk\")"),
        "managed get_rate must be the live hook query, NOT a const-fold (D10/R10)"
    );
    assert!(
        golden.contains("regseq_clock_disable(\"uartclk\")"),
        "managed disable must route to the host hook (C04)"
    );
    assert!(
        golden.contains("regseq_clock_release(\"uartclk\");"),
        "the compiler-managed release postamble must fire (C07)"
    );

    // BEHAVIORAL GATES (dual oracle) on the CLK interleaving.
    let stdout =
        std::fs::read_to_string(dir.join("expected.stdout")).expect("read expected.stdout");
    let pos = |needle: &str| -> usize {
        stdout
            .find(needle)
            .unwrap_or_else(|| panic!("expected.stdout must contain {needle:?}; got:\n{stdout}"))
    };
    let acquire = pos("CLK acquire uartclk");
    let on_init = pos("LC call on_init");
    let enable = pos("CLK enable uartclk");
    let rate = pos("CLK rate uartclk = 0x16e3600");
    let write = pos("W32 @0x9000000 = 0x016e3600");
    let on_deinit = pos("LC call on_deinit");
    let disable = pos("CLK disable uartclk");
    let release = pos("CLK release uartclk");
    assert!(
        acquire < on_init,
        "the compiler-managed acquire must precede the first author body op (C07)"
    );
    assert!(
        on_init < enable && enable < write,
        "the author CLK enable must precede the first register write (C05/D11 fixture assertion)"
    );
    assert!(
        enable < rate && rate < write,
        "the live get_rate (0x16e3600 = 24 MHz) flows into the register write"
    );
    assert!(
        on_deinit < disable && disable < release,
        "author disable must precede the compiler-managed release (LIFO, C07/D16)"
    );
    assert!(acquire < release, "acquire/release frame the clock's lifetime");
}
