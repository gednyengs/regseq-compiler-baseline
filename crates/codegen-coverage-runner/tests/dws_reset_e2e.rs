//! The deferred-work-item-scheduling keystone behavioral-oracle gate
//! (deferred-work-item-scheduling construct, PLAN T15): the `deferred-work-reset`
//! fixture passes the six-stage dual oracle in a DEDICATED clean process (the
//! emit's MIR is sensitive to rustc's process-global interner history — so each
//! golden-snapshot test is the SOLE test in its binary, like `irq_pl011_e2e` /
//! `clock_pl011_managed_e2e`). The blessed `expected.stdout` is the `WRK` trace
//! contract made observable (C05): the compiler-managed `init_work` rides the
//! preamble before the author `on_init`; the author `self.work.submit()` arms the
//! sole `Work`; the harness-only `regseq_work_drain()` (after `on_init` returns)
//! dispatches it in (simulated) sleepable thread context (the `WRK dispatch` line
//! proves the wrapper invoked the handler); the recovered-`self` wiring is
//! snapshotted byte-exact in `golden.c` (the wrapper threads `&__t->inner`; the
//! handler writes `self.done` through it — the mutation is proven by code shape,
//! not a separate runtime print); `cancel_work` runs after `on_deinit` (last in
//! this single-resource fixture); and the post-cancel re-arm probe (a
//! harness-side submit+drain AFTER teardown)
//! dispatches NOTHING (C05 item 4 — a cancelled `Work` cannot re-arm). This
//! fixture FREEZES the `WRK …` trace spelling (OI-4).
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
fn deferred_work_reset_oracle() {
    let dir = dwork_corpus().join("deferred-work-reset");
    let fixtures = manifest::discover(&dwork_corpus()).expect("discover deferred-work corpus");
    let fixture = fixtures
        .iter()
        .find(|f| f.manifest.name == "deferred-work-reset")
        .expect("deferred-work-reset fixture discovered");

    // The construct has 0 L1 / 0 L2 properties (GRILL D02) — no platform.toml.
    assert!(
        fixture.platform.is_none(),
        "deferred-work-reset has 0 L2 — it must NOT carry a platform.toml"
    );

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

    // Stages 1-6 (the process's first — interner-clean — bridge): byte-exact
    // golden.c (stage 3) + byte-exact expected.stdout (stage 6).
    let result = pipeline::run_fixture(fixture, cc());
    assert!(
        result.is_green(),
        "deferred-work-reset must pass all six stages: {:?}",
        result.verdict
    );

    // STORAGE-CONSISTENCY GATE on the committed golden (GRILL T07): `Work` is an
    // opaque ZST with no in-struct storage, so `&f` is the compiler-owned slot
    // `&__regseq_work_slots[idx]` — and init/submit/cancel for the one resource
    // MUST all key on the SAME slot index (0).
    let golden = std::fs::read_to_string(dir.join("golden.c")).expect("read golden.c");
    assert!(
        golden.contains("#define REGSEQ_WORK_SLOTS 1"),
        "the slot table is sized to the TU's exact Work-resource count (1)"
    );
    assert!(
        golden.contains("regseq_work_init(&__regseq_work_slots[0]"),
        "init_work keys on slot 0"
    );
    assert!(
        golden.contains("regseq_work_submit(&__regseq_work_slots[0])"),
        "submit keys on the SAME slot 0 (storage consistency, T07)"
    );
    assert!(
        golden.contains("regseq_work_cancel(&__regseq_work_slots[0])"),
        "cancel_work keys on the SAME slot 0 (storage consistency, T07)"
    );

    // WRAPPER LOCK-FREEDOM GATE (C05 item 3): the self-recovering wrapper recovers
    // the driver and runs the handler with NO lock acquisition. Scope the check to
    // the wrapper body so the whole-file scan never confuses "clock" for a lock.
    let wstart = golden
        .find("static void rseq_work_")
        .expect("golden.c defines the self-recovering wrapper");
    let wbody = &golden[wstart..];
    let wend = wbody
        .find("\n}\n")
        .expect("wrapper has a closing brace");
    let wrapper = &wbody[..wend];
    assert!(
        !wrapper.contains("lock"),
        "the deferred-work wrapper must acquire NO lock (C05 item 3); wrapper:\n{wrapper}"
    );
    // RECOVERED-SELF GATE (C05 items 1/2): the wrapper recovers the driver instance
    // from the registered ctx (`&__t->inner`, the interrupt-wrapper shape) before
    // running the handler — this is the code-shape evidence that the handler's
    // `self.done` write lands on the live instance (the `WRK dispatch` line proves
    // the wrapper ran; this proves it ran over the recovered self).
    assert!(
        wrapper.contains("&__t->inner"),
        "the wrapper must recover the driver instance via &__t->inner; wrapper:\n{wrapper}"
    );

    // BEHAVIORAL GATES (dual oracle) on the WRK interleaving.
    let stdout =
        std::fs::read_to_string(dir.join("expected.stdout")).expect("read expected.stdout");
    let pos = |needle: &str| -> usize {
        stdout
            .find(needle)
            .unwrap_or_else(|| panic!("expected.stdout must contain {needle:?}; got:\n{stdout}"))
    };
    let rpos = |needle: &str| -> usize {
        stdout
            .rfind(needle)
            .unwrap_or_else(|| panic!("expected.stdout must contain {needle:?}; got:\n{stdout}"))
    };

    let init = pos("WRK init 0");
    let on_init = pos("LC call on_init");
    let submit = pos("WRK submit 0");
    let drain = pos("WRK drain");
    let dispatch = pos("WRK dispatch 0");
    let cancel = pos("WRK cancel 0");

    // init_work precedes the author on_init (compiler-owned lifecycle entry, I05).
    assert!(init < on_init, "init_work must precede the author on_init (I05)");
    // The author `self.work.submit()` fires inside on_init.
    assert!(on_init < submit, "the author submit fires inside on_init");
    // The harness drain (after on_init returns) dispatches the submitted work (D09).
    assert!(
        submit < drain && drain < dispatch,
        "the harness drain dispatches the submitted work (D09)"
    );
    // cancel_work runs last in the teardown, after the (single) legitimate
    // dispatch (I04/C07).
    assert!(dispatch < cancel, "cancel_work runs last, before free (I04/C07)");

    // POST-CANCEL RE-ARM PROBE (C05 item 4): a harness-side submit+drain AFTER the
    // teardown's cancel must NOT re-arm — the work dispatches exactly ONCE in the
    // whole run (the legitimate pre-cancel dispatch), and the last dispatch is
    // strictly before the cancel.
    let probe_submit = rpos("WRK submit 0");
    let probe_drain = rpos("WRK drain");
    assert!(
        probe_submit > cancel,
        "the post-cancel probe re-submits the (cancelled) Work after teardown"
    );
    assert!(
        probe_drain > probe_submit,
        "the post-cancel probe drains after re-submitting"
    );
    assert_eq!(
        stdout.matches("WRK dispatch 0").count(),
        1,
        "the Work dispatches exactly once — the post-cancel submit+drain must NOT re-arm (C05 item 4)"
    );
    assert!(
        rpos("WRK dispatch") < cancel,
        "no work dispatches after cancel — a cancelled Work cannot re-arm (C05 item 4)"
    );
}
