//! Device-lifecycle M5 gate (shutdown-OMIT): the `lifecycle-pl011` fixture passes
//! the six-stage dual oracle in a DEDICATED, clean process (the emit's MIR is
//! sensitive to rustc's process-global interner history, so the byte-snapshot is
//! reproducible only here — D17). Beyond the snapshot, this asserts the
//! omit-when-absent shutdown rule (D10/I02): the verified `golden.c` must contain
//! NO `on_system_shutdown` glue (the AMBA/serial driver does not override it), and
//! must carry the AMBA match in the registration trace.
//!
//! SOLE test in this binary (like `lifecycle_e2e` / `corpus_e2e`): two bridge
//! runs in one process would perturb each other's interner history. `BLESS=1`
//! writes `golden.c` + `expected.stdout` from a single clean emit, then returns
//! without also verifying.

#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::{Path, PathBuf};

use codegen_coverage_runner::{manifest, pipeline, toolchain};
use test_ir_fixtures::{try_bridge_fixture_path, BridgeOutcome};

/// The host `cc`, matching the runner's default.
fn cc() -> &'static Path {
    Path::new("cc")
}

fn lifecycle_corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/_lifecycle")
}

#[test]
fn lifecycle_pl011_m5_gate() {
    let dir = lifecycle_corpus().join("lifecycle-pl011");

    if std::env::var_os("BLESS").is_some() {
        let fixtures = manifest::discover(&lifecycle_corpus()).expect("discover lifecycle corpus");
        let fixture = fixtures
            .iter()
            .find(|f| f.manifest.name == "lifecycle-pl011")
            .expect("lifecycle-pl011 fixture discovered");
        let options = pipeline::backend_options_for(fixture).expect("backend options");
        let model = match try_bridge_fixture_path(&dir.join("input.rs")) {
            BridgeOutcome::Ok(m) => m,
            other => panic!("bless bridge failed: {other:?}"),
        };
        let emit = regseq_backends::emit_generic_c_for_entry_with(
            &model,
            &fixture.manifest.entry,
            &options,
        )
        .expect("emit must succeed");
        let run = toolchain::compile_and_run(&emit.tu, &emit.shim, cc())
            .expect("compile + run must succeed");
        std::fs::write(dir.join("golden.c"), &emit.tu).expect("write golden.c");
        std::fs::write(dir.join("expected.stdout"), &run.stdout).expect("write expected.stdout");
        return;
    }

    let fixtures = manifest::discover(&lifecycle_corpus()).expect("discover lifecycle corpus");
    let fixture = fixtures
        .iter()
        .find(|f| f.manifest.name == "lifecycle-pl011")
        .expect("lifecycle-pl011 fixture discovered");
    let result = pipeline::run_fixture(fixture, cc());
    assert!(
        result.is_green(),
        "lifecycle-pl011 must pass all six stages: {:?}",
        result.verdict
    );

    // M5 GATE (omit-when-absent): the golden must contain NO on_system_shutdown
    // glue (the driver does not override it — D10/I02), and must carry the AMBA
    // match in the registration trace (LC-4).
    let golden = std::fs::read_to_string(dir.join("golden.c")).expect("read golden.c");
    assert!(
        !golden.contains("on_system_shutdown"),
        "golden.c must contain NO on_system_shutdown glue (omit-when-absent, D10/I02)"
    );
    assert!(
        golden.contains("amba:0x41011/0xfffff"),
        "golden.c must carry the AMBA match in the registration trace (LC-4)"
    );
}
