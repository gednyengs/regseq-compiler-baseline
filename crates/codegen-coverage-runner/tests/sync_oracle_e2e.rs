//! Synchronization (locks) generic-c oracle (M1 / T05): a `#[rseq::driver]`
//! with one `IrqSpinLock<u32>` field whose `on_init` acquires + releases it
//! lowers, on the generic-c backend, to the `regseq_lock_acquire(id,"IRQSPIN")`
//! / `regseq_lock_release(id)` oracle hooks; compiling + running the synthesized
//! lifecycle TU produces the balanced acquire/release trace and exits 0 (the
//! self-checking SYNC_HELPERS abort on a discipline violation, so a clean exit
//! IS the oracle pass for: balanced acquire/release, no double-acquire, LIFO
//! guard-drop, release-only-the-held).
//!
//! The fixture lives in a per-call temp corpus (no committed golden); the
//! assertions are on the emitted C + the run trace.

#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::{Path, PathBuf};

use codegen_coverage_runner::{manifest, pipeline, toolchain};
use test_ir_fixtures::{try_bridge_fixture_path, BridgeOutcome};

fn cc() -> &'static Path {
    Path::new("cc")
}

/// A temp single-fixture corpus removed on drop.
struct TempCorpus {
    dir: PathBuf,
}

impl TempCorpus {
    fn new(label: &str) -> TempCorpus {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!(
            "ccr-sync-oracle-{label}-{}-{nanos}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).expect("create temp corpus dir");
        TempCorpus { dir }
    }
}

impl Drop for TempCorpus {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// Write a single-`IrqSpinLock` driver fixture (named `name`, `on_init` body
/// `init_body`) into the temp corpus and return its dir.
fn write_fixture(corpus: &TempCorpus, name: &str, init_body: &str) -> PathBuf {
    write_fixture_with(
        corpus,
        name,
        "pub lock: IrqSpinLock<u32>",
        init_body,
    )
}

/// Write a lock-driver fixture with arbitrary `fields` + `on_init` `init_body`.
fn write_fixture_with(
    corpus: &TempCorpus,
    name: &str,
    fields: &str,
    init_body: &str,
) -> PathBuf {
    let fixture_dir = corpus.dir.join(name);
    std::fs::create_dir_all(&fixture_dir).expect("create fixture dir");

    // Include the committed lifecycle + sync author surfaces by ABSOLUTE path
    // (`include!` resolves relative to the temp input.rs; `CARGO_MANIFEST_DIR`
    // is unset when the bridge runs rustc, so bake the paths in here).
    let lifecycle = format!(
        "{}/fixtures/_lifecycle_surface.rs",
        env!("CARGO_MANIFEST_DIR")
    );
    let sync = format!("{}/fixtures/_sync_surface.rs", env!("CARGO_MANIFEST_DIR"));
    let input = format!(
        "#![no_std]\n#![feature(register_tool)]\n#![register_tool(rseq)]\n\
         include!(\"{lifecycle}\");\n\
         include!(\"{sync}\");\n\n\
         #[rseq::driver(name = \"synclock\", compatible = [\"acme,synclock\"])]\n\
         pub struct SyncDrv {{ {fields} }}\n\n\
         impl Lifecycle for SyncDrv {{\n\
         \tfn on_init(&mut self) -> Result<(), RegseqError> {{\n\
         \t\t{init_body}\n\
         \t\tOk(())\n\t}}\n}}\n"
    );
    std::fs::write(fixture_dir.join("input.rs"), input).expect("write input.rs");
    std::fs::write(
        fixture_dir.join("manifest.toml"),
        format!("name = \"{name}\"\nkind = \"rust\"\ncovers = [\"SYN-S11\"]\nlifecycle_scenario = \"orderly\"\nexpect = \"pass\"\n"),
    )
    .expect("write manifest.toml");
    fixture_dir
}

/// Bridge + bind + emit-for-entry the fixture named `name` in `corpus`.
fn emit(corpus: &TempCorpus, name: &str) -> regseq_backends::EntryEmit {
    let fixtures = manifest::discover(&corpus.dir).expect("discover temp corpus");
    let fixture = fixtures
        .iter()
        .find(|f| f.manifest.name == name)
        .expect("fixture discovered");
    let mut model = match try_bridge_fixture_path(&fixture.input) {
        BridgeOutcome::Ok(m) => m,
        other => panic!("bridge failed: {other:?}"),
    };
    let identity = pipeline::bind_l2_platform(&mut model, fixture).expect("L2 bind");
    let options = pipeline::backend_options_for(fixture).expect("backend options");
    let context = regseq_backends::BackendContext::new(options, identity);
    regseq_backends::emit_generic_c_for_entry_with_context(
        &model,
        &fixture.manifest.entry,
        &context,
    )
    .expect("generic-c emit must succeed")
}

#[test]
fn irqspinlock_acquire_release_lowers_and_oracle_passes() {
    let corpus = TempCorpus::new("m1");
    let _dir = write_fixture(&corpus, "sync-irqspinlock", "let _g = self.lock.lock();");
    let emit = emit(&corpus, "sync-irqspinlock");

    // T04 — the lock-oracle hooks are declared + the acquire/release sites lower
    // to them with the witness kind + lock_id 0.
    assert!(
        emit.tu.contains("SYNC lock_acquire"),
        "SYNC_HELPERS block must be emitted for a lock TU:\n{}",
        emit.tu
    );
    assert!(
        emit.tu.contains("regseq_lock_acquire(0u, \"IRQSPIN\")"),
        "the IrqSpinLock acquire must lower to regseq_lock_acquire(0u, \"IRQSPIN\"):\n{}",
        emit.tu
    );
    assert!(
        emit.tu.contains("regseq_lock_release(0u)"),
        "the guard drop must lower to regseq_lock_release(0u):\n{}",
        emit.tu
    );

    // T05 — compile + run; the self-checking oracle accepts the balanced trace.
    let run = toolchain::compile_and_run(&emit.tu, &emit.shim, cc()).expect("compile + run");
    assert_eq!(
        run.exit_code,
        Some(0),
        "the lock-discipline oracle must accept a balanced acquire/release (exit 0); \
         stdout:\n{}\nstderr:\n{}",
        run.stdout,
        run.stderr
    );
    assert!(
        !run.stdout.contains("SYNC error"),
        "no discipline violation must be reported; stdout:\n{}",
        run.stdout
    );
    assert!(
        run.stdout.contains("SYNC lock_acquire id=0 kind=IRQSPIN"),
        "the acquire event must appear in the trace; stdout:\n{}",
        run.stdout
    );
    assert!(
        run.stdout.contains("SYNC lock_release id=0"),
        "the release event must appear in the trace; stdout:\n{}",
        run.stdout
    );
}

#[test]
fn all_kinds_and_completion_lower_and_oracle_passes() {
    // The full first-slice surface in one driver: Mutex (id 0), SpinLock (id 1),
    // IrqSpinLock (id 2), Completion (id 3). on_init runs the completion ops
    // (no lock held), then a nested lock block (acquire 0,1,2; LIFO release
    // 2,1,0 at block end). The generic-c oracle must accept the whole trace.
    let corpus = TempCorpus::new("m2-all");
    let _dir = write_fixture_with(
        &corpus,
        "sync-all-kinds",
        "pub m: Mutex<u32>, pub s: SpinLock<u32>, pub irq: IrqSpinLock<u32>, \
         #[rseq::sync(cardinality = SingleWaiter)] pub done: Completion",
        "self.done.signal();\n\t\tself.done.reset();\n\t\t\
         let _ = self.done.wait(Duration::from_us(1));\n\t\t\
         { let _g1 = self.m.lock(); let _g2 = self.s.lock(); let _g3 = self.irq.lock(); }",
    );
    let emit = emit(&corpus, "sync-all-kinds");

    // Each kind lowers to its labelled acquire; the completion ops lower to the
    // completion hooks (the Completion is resource id 3).
    for needle in [
        "regseq_lock_acquire(0u, \"MUTEX\")",
        "regseq_lock_acquire(1u, \"SPIN\")",
        "regseq_lock_acquire(2u, \"IRQSPIN\")",
        "regseq_completion_signal(3u)",
        "regseq_completion_reset(3u)",
        "regseq_completion_wait(3u)",
    ] {
        assert!(emit.tu.contains(needle), "missing `{needle}`:\n{}", emit.tu);
    }

    let run = toolchain::compile_and_run(&emit.tu, &emit.shim, cc()).expect("compile + run");
    assert_eq!(
        run.exit_code,
        Some(0),
        "the oracle must accept the full-surface trace (exit 0); stdout:\n{}\nstderr:\n{}",
        run.stdout,
        run.stderr
    );
    assert!(!run.stdout.contains("SYNC error"), "no violation; stdout:\n{}", run.stdout);
    for ev in [
        "SYNC completion_signal id=3",
        "SYNC completion_reset id=3",
        "SYNC completion_wait id=3",
        "SYNC lock_acquire id=0 kind=MUTEX",
        "SYNC lock_acquire id=1 kind=SPIN",
        "SYNC lock_acquire id=2 kind=IRQSPIN",
    ] {
        assert!(run.stdout.contains(ev), "missing trace `{ev}`; stdout:\n{}", run.stdout);
    }
}

#[test]
fn guard_drop_early_releases_and_oracle_passes() {
    // An explicit `guard_drop_early(self)` is the release site (the guard is
    // moved into it, so there is no scope-end drop-glue). The oracle sees a
    // balanced acquire/release and exits 0.
    let corpus = TempCorpus::new("m2-early");
    let _dir = write_fixture(
        &corpus,
        "sync-drop-early",
        "let g = self.lock.lock();\n\t\tg.guard_drop_early();",
    );
    let emit = emit(&corpus, "sync-drop-early");
    assert!(
        emit.tu.contains("regseq_lock_acquire(0u, \"IRQSPIN\")")
            && emit.tu.contains("regseq_lock_release(0u)"),
        "drop_early must lower to acquire + release:\n{}",
        emit.tu
    );
    let run = toolchain::compile_and_run(&emit.tu, &emit.shim, cc()).expect("compile + run");
    assert_eq!(run.exit_code, Some(0), "stdout:\n{}\nstderr:\n{}", run.stdout, run.stderr);
    assert!(
        run.stdout.contains("SYNC lock_acquire id=0") && run.stdout.contains("SYNC lock_release id=0"),
        "stdout:\n{}",
        run.stdout
    );
}

#[test]
fn recursive_double_acquire_is_rejected_at_the_bridge() {
    // Negative: acquiring the SAME lock twice without releasing the first is a
    // non-recursive double-acquire (a deadlock on a real spinlock). As of M3/T12
    // this is rejected STATICALLY at the bridge (the recursive-acquire check, a
    // consumer of the T10 held-region marking) — caught even earlier than the
    // generic-c oracle's runtime double-acquire abort, and crucially BEFORE the
    // OS backends lower it to a real deadlocking lock. So the spec never reaches
    // codegen.
    //
    // (The oracle's runtime `SYNC error double-acquire` net remains in
    // `SYNC_HELPERS` as defense-in-depth; it is the very check the T13
    // IRQ-reentry leg relies on NOT firing — an IrqSpinLock-masked mainline
    // region must keep a simulated IRQ from re-entering and double-acquiring.)
    let corpus = TempCorpus::new("m3-recursive");
    let _dir = write_fixture(
        &corpus,
        "sync-double-acquire",
        "let _g1 = self.lock.lock();\n\t\tlet _g2 = self.lock.lock();",
    );
    let fixtures = manifest::discover(&corpus.dir).expect("discover temp corpus");
    let fixture = fixtures
        .iter()
        .find(|f| f.manifest.name == "sync-double-acquire")
        .expect("fixture discovered");
    let outcome = try_bridge_fixture_path(&fixture.input);
    match outcome {
        BridgeOutcome::Rejected(err) => {
            let rendered = format!("{err:?}");
            assert!(
                rendered.contains("ForbiddenTryLockOrUnlock") && rendered.contains("recursive"),
                "a recursive same-lock acquire must reject as ForbiddenTryLockOrUnlock; got: {rendered}"
            );
        }
        other => panic!("a recursive double-acquire must be bridge-rejected; got {other:?}"),
    }
}
