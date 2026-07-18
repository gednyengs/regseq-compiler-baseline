//! The deferred-work-item-scheduling M1 vertical-slice smoke (PLAN T10): the
//! `deferred-work-reset` fixture bridges + emits + `cc`-compiles + runs cleanly,
//! and its stdout carries the `WRK` trace oracle
//! `init -> submit -> drain⇒dispatch -> cancel` in order. The byte-exact golden
//! (`golden.c` + `expected.stdout`) is blessed in M3/T15; this M1 gate only
//! asserts a clean run + the four-phase ordering.
//!
//! Interner-isolated single-test binary (the emit's MIR is sensitive to rustc's
//! process-global interner history, like `irq`/`clock`), running the same
//! bridge → bind → emit → cc → run path the keystone golden e2e uses.

#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::{Path, PathBuf};

use codegen_coverage_runner::{manifest, pipeline, toolchain};
use test_ir_fixtures::{try_bridge_fixture_path, BridgeOutcome};

/// The host `cc`, matching the runner's default.
fn cc() -> &'static Path {
    Path::new("cc")
}

fn dwork_corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/_deferred_work")
}

#[test]
fn deferred_work_reset_smoke() {
    let fixtures = manifest::discover(&dwork_corpus()).expect("discover deferred-work corpus");
    let fixture = fixtures
        .iter()
        .find(|f| f.manifest.name == "deferred-work-reset")
        .expect("deferred-work-reset fixture discovered");

    // Bridge → bind (no platform.toml: 0 L2, so the bind is the no-bindings
    // path) → emit → cc → run, the same path the keystone golden e2e uses.
    let options = pipeline::backend_options_for(fixture).expect("backend options");
    let mut model = match try_bridge_fixture_path(&fixture.input) {
        BridgeOutcome::Ok(m) => m,
        other => panic!("bridge must succeed for the M1 vertical slice: {other:?}"),
    };
    let identity = pipeline::bind_l2_platform(&mut model, fixture).expect("L2 bind (no-op, 0 L2)");
    let context = regseq_backends::BackendContext::new(options, identity);
    let emit = regseq_backends::emit_generic_c_for_entry_with_context(
        &model,
        &fixture.manifest.entry,
        &context,
    )
    .expect("emit must succeed");
    let run =
        toolchain::compile_and_run(&emit.tu, &emit.shim, cc()).expect("compile + run must succeed");
    let stdout = run.stdout;

    // The WRK trace oracle: init -> (on_init: submit) -> drain⇒dispatch -> cancel.
    let pos = |needle: &str| -> usize {
        stdout
            .find(needle)
            .unwrap_or_else(|| panic!("stdout must contain {needle:?}; got:\n{stdout}"))
    };
    let init = pos("WRK init 0");
    let on_init = pos("LC call on_init");
    let submit = pos("WRK submit 0");
    let drain = pos("WRK drain");
    let dispatch = pos("WRK dispatch 0");
    let cancel = pos("WRK cancel 0");

    assert!(
        init < on_init,
        "init_work must precede the author on_init (compiler-owned lifecycle entry, I05)"
    );
    assert!(
        on_init < submit,
        "the author `self.work.submit()` fires inside on_init"
    );
    assert!(
        submit < drain && drain < dispatch,
        "the harness drain (after on_init returns) dispatches the submitted work (D09)"
    );
    assert!(
        dispatch < cancel,
        "cancel_work runs last, in the teardown before free (I04/C07)"
    );
}
