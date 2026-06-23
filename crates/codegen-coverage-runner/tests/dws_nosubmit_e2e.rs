//! The deferred-work-item-scheduling T-nosubmit distinguisher gate
//! (deferred-work-item-scheduling construct, PLAN T16): the
//! `deferred-work-nosubmit` fixture — a required, type-recognized `Work` with a
//! bound `#[rseq::work_handler]` but NO `self.work.submit()` — passes the
//! six-stage dual oracle in a DEDICATED clean process (interner-isolated, like
//! the reset keystone). It PROVES the `init_work`/`cancel_work` lifecycle is
//! RESOURCE-gated, not submit-gated (GRILL D07/D13): both still emit, the
//! `regseq_work_*` prelude is still stitched, yet the `WRK` trace is `init …
//! cancel` with no `submit`/`drain`/`dispatch` (the harness drain + post-cancel
//! probe are submit-gated, so a never-submitted `Work` skips them). Without this
//! the reset golden cannot distinguish a correct resource-gated build from a
//! wrong intrinsic-gated one.
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

fn dwork_corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/_deferred_work")
}

#[test]
fn deferred_work_nosubmit_oracle() {
    let dir = dwork_corpus().join("deferred-work-nosubmit");
    let fixtures = manifest::discover(&dwork_corpus()).expect("discover deferred-work corpus");
    let fixture = fixtures
        .iter()
        .find(|f| f.manifest.name == "deferred-work-nosubmit")
        .expect("deferred-work-nosubmit fixture discovered");

    if std::env::var_os("BLESS").is_some() {
        let options = pipeline::backend_options_for(fixture).expect("backend options");
        let mut model = match try_bridge_fixture_path(&fixture.input) {
            BridgeOutcome::Ok(m) => m,
            other => panic!("bless bridge failed: {other:?}"),
        };
        let identity =
            pipeline::bind_l2_platform(&mut model, fixture).expect("L2 bind (no-op, 0 L2)");
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
        "deferred-work-nosubmit must pass all six stages: {:?}",
        result.verdict
    );

    // RESOURCE-GATING GATES on the committed golden (D07/D13): the `regseq_work_*`
    // prelude + `init_work` + `cancel_work` are emitted even with no submit, but
    // there is NO submit-lowering call and NO harness drain call.
    let golden = std::fs::read_to_string(dir.join("golden.c")).expect("read golden.c");
    assert!(
        golden.contains("#define REGSEQ_WORK_SLOTS 1"),
        "the WORK_HELPERS prelude is stitched (resource-gated, D07)"
    );
    assert!(
        golden.contains("regseq_work_init(&__regseq_work_slots[0]"),
        "init_work is emitted resource-gated, even with no submit (D13)"
    );
    assert!(
        golden.contains("regseq_work_cancel(&__regseq_work_slots[0]"),
        "cancel_work is emitted resource-gated, even with no submit (D13)"
    );
    assert!(
        !golden.contains("regseq_work_submit(&__regseq_work_slots"),
        "no submit lowering: the author never calls `self.work.submit()`"
    );
    assert!(
        !golden.contains("regseq_work_drain();"),
        "no harness drain call: the drain is submit-gated (nothing to drain)"
    );

    // BEHAVIORAL GATE (dual oracle): the WRK trace is `init … cancel` only.
    let stdout =
        std::fs::read_to_string(dir.join("expected.stdout")).expect("read expected.stdout");
    let init = stdout
        .find("WRK init 0")
        .unwrap_or_else(|| panic!("expected.stdout must contain WRK init 0; got:\n{stdout}"));
    let cancel = stdout
        .find("WRK cancel 0")
        .unwrap_or_else(|| panic!("expected.stdout must contain WRK cancel 0; got:\n{stdout}"));
    assert!(
        init < cancel,
        "init_work precedes cancel_work (the resource lifetime frame)"
    );
    assert!(
        !stdout.contains("WRK submit"),
        "no submit in the trace (the author never schedules the work):\n{stdout}"
    );
    assert!(
        !stdout.contains("WRK drain"),
        "no harness drain in the trace (submit-gated):\n{stdout}"
    );
    assert!(
        !stdout.contains("WRK dispatch"),
        "no dispatch in the trace (the handler is never run):\n{stdout}"
    );
}
