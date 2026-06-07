//! Device-lifecycle M7 gate: the `lifecycle-handle` fixture (the first
//! non-fieldless driver: user-data `count` + register-access resource `regs`)
//! passes the six-stage dual oracle in a DEDICATED, clean process (D17). Beyond
//! the snapshot, this asserts the register-access composition (the register write
//! hits the recovered offset) and the user-data zero-init (the first write
//! observes 0x0).
//!
//! SOLE test in this binary (interner sensitivity, D17). `BLESS=1` writes
//! `golden.c` + `expected.stdout` from a single clean emit, then returns.

#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::{Path, PathBuf};

use codegen_coverage_runner::{manifest, pipeline, toolchain};
use test_ir_fixtures::{try_bridge_fixture_path, BridgeOutcome};

fn cc() -> &'static Path {
    Path::new("cc")
}

fn lifecycle_corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/_lifecycle")
}

#[test]
fn lifecycle_handle_m7_gate() {
    let dir = lifecycle_corpus().join("lifecycle-handle");

    if std::env::var_os("BLESS").is_some() {
        let fixtures = manifest::discover(&lifecycle_corpus()).expect("discover lifecycle corpus");
        let fixture = fixtures
            .iter()
            .find(|f| f.manifest.name == "lifecycle-handle")
            .expect("lifecycle-handle fixture discovered");
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
        .find(|f| f.manifest.name == "lifecycle-handle")
        .expect("lifecycle-handle fixture discovered");
    let result = pipeline::run_fixture(fixture, cc());
    assert!(
        result.is_green(),
        "lifecycle-handle must pass all six stages: {:?}",
        result.verdict
    );

    let golden = std::fs::read_to_string(dir.join("golden.c")).expect("read golden.c");
    assert!(
        golden.contains("regseq_mmio_write(0, 4,"),
        "golden.c must lower the register write to regseq_mmio_write at the recovered offset 0 \
         (the register-access composition: dr's offset within the block, NOT regs' offset in the driver)"
    );
    assert!(
        golden.contains("regseq_lc_preamble(\"regs\")")
            && golden.contains("regseq_lc_postamble(\"regs\", 0)")
            && golden.contains("regseq_lc_postamble(\"regs\", 1)"),
        "golden.c must bracket the callbacks with the resource preamble + reverse postambles \
         (released for orderly, skipped(surprise) for surprise)"
    );
    // LC-12 (Policy-P container morph): the target-view container embeds the spec
    // driver as `inner` + wraps it with __rseq_dev/__rseq_state; the callbacks run
    // spec-view over &inst.inner (CR-7). The register access composes through the
    // morph (offset 0, asserted above) with the stdout trace unchanged.
    assert!(
        golden.contains("struct __rseq_lc_0_handle_target {")
            && golden.contains("void *__rseq_dev;")
            && golden.contains("struct input_Handle inner;")
            && golden.contains("uint32_t __rseq_state;")
            && golden.contains("on_init(&inst.inner)"),
        "golden.c must emit the Policy-P target-view container (embedding the spec \
         driver as inner, wrapped with __rseq_dev/__rseq_state) and run the callbacks \
         over &inst.inner (LC-12)"
    );
}
