//! The device-interrupt init-failure gate (construct #6, PLAN T07): the
//! `irq-initfail` fixture passes the six-stage dual oracle in a DEDICATED
//! clean process (D17). The C01 Err branch made observable: the line is
//! registered DISABLED in the preamble, `on_init` Errs, NO `IRQ enable` ever
//! traces (R05/I01), and the reverse unwind includes the `IRQ unregister`
//! after the rollback callback (R16).
//!
//! `BLESS=1` writes `golden.c` + `expected.stdout` from a single clean
//! bind+emit+run, then returns without also verifying.
//!
//! SOLE test in this binary (interner isolation).

#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::{Path, PathBuf};

use codegen_coverage_runner::{manifest, pipeline, toolchain};
use test_ir_fixtures::{try_bridge_fixture_path, BridgeOutcome};

/// The host `cc`, matching the runner's default.
fn cc() -> &'static Path {
    Path::new("cc")
}

fn irq_corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/_irq")
}

#[test]
fn irq_initfail_unwind_gate() {
    let dir = irq_corpus().join("irq-initfail");
    let fixtures = manifest::discover(&irq_corpus()).expect("discover irq corpus");
    let fixture = fixtures
        .iter()
        .find(|f| f.manifest.name == "irq-initfail")
        .expect("irq-initfail fixture discovered")
        .with_binding(manifest::OsBinding::Linux)
        .expect("irq-initfail resolves the linux binding");
    let fixture = &fixture;

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
        "irq-initfail must pass all six stages: {:?}",
        result.verdict
    );

    // BEHAVIORAL GATES on the Err branch (R05/R16): registration traces, the
    // rollback runs, the unwind unregisters — and NO enable fires anywhere.
    let stdout =
        std::fs::read_to_string(dir.join("expected.stdout")).expect("read expected.stdout");
    let pos = |needle: &str| -> usize {
        stdout
            .find(needle)
            .unwrap_or_else(|| panic!("expected.stdout must contain {needle:?}; got:\n{stdout}"))
    };
    let register = pos("IRQ register line=37");
    let err = pos("LC on_init -> Err");
    let rollback = pos("LC call on_init_failure");
    let unregister = pos("IRQ unregister line=37");
    assert!(
        register < err && err < rollback && rollback < unregister,
        "the unwind must run register -> Err -> rollback -> unregister (R16)"
    );
    assert!(
        !stdout.contains("IRQ enable"),
        "NO IRQ enable may trace on the Err path (R05/I01); got:\n{stdout}"
    );
    assert!(
        !stdout.contains("IRQ disable"),
        "no disable on the unwind — the line was never enabled (D02); got:\n{stdout}"
    );
}
