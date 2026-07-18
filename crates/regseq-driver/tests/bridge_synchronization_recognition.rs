//! Bridge integration for synchronization lock-field recognition (M1 / T03).
//!
//! A `#[rseq::driver]` struct with an `IrqSpinLock<T>` field must bridge (the
//! lock field is routed away from the device-lifecycle user-data classifier),
//! and the recognized facts must appear in the model: a `SyncResource` of kind
//! `IrqSpinLock` in `synchronization.drivers`, plus an `AttrData::SyncResource`
//! field marker in `domain_attrs.field_attrs`.

#![feature(rustc_private)]

extern crate rustc_driver;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_session;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex, Once};
use std::time::{SystemTime, UNIX_EPOCH};

use regseq_driver::bridge::{run_bridge, BridgeInput};
use regseq_model::{AttrData, RegseqModel, SyncKind, SyncOp};
use rustc_middle::ty::TyCtxt;

static INIT: Once = Once::new();
static COMPILER_LOCK: Mutex<()> = Mutex::new(());

fn init_rustc() {
    INIT.call_once(|| {
        let early_dcx =
            rustc_session::EarlyDiagCtxt::new(rustc_session::config::ErrorOutputType::default());
        rustc_driver::init_rustc_env_logger(&early_dcx);
    });
}

fn get_sysroot() -> String {
    let output = Command::new("rustc")
        .arg("--print")
        .arg("sysroot")
        .output()
        .expect("failed to run rustc --print sysroot");
    String::from_utf8(output.stdout).unwrap().trim().to_string()
}

fn fresh_test_dir(crate_name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("regseq-bridge-sync-{crate_name}-{nanos}"))
}

/// Run the full bridge over `source`; on success apply `f` to the model, on
/// rejection return the rendered `BridgeError`.
fn with_model<R, F>(crate_name: &str, source: &str, f: F) -> Result<R, String>
where
    R: Send + 'static,
    F: FnOnce(&RegseqModel) -> R + Send + 'static,
{
    let _lock = COMPILER_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    init_rustc();

    let test_dir = fresh_test_dir(crate_name);
    std::fs::create_dir_all(&test_dir).unwrap();
    let src_path = test_dir.join("lib.rs");
    std::fs::write(&src_path, source).unwrap();

    let result: Arc<Mutex<Option<Result<R, String>>>> = Arc::new(Mutex::new(None));
    let callback = Arc::new(Mutex::new(Some(
        Box::new(f) as Box<dyn FnOnce(&RegseqModel) -> R + Send>
    )));

    struct Callbacks<R: Send + 'static> {
        result: Arc<Mutex<Option<Result<R, String>>>>,
        callback: Arc<Mutex<Option<Box<dyn FnOnce(&RegseqModel) -> R + Send>>>>,
    }

    impl<R: Send + 'static> rustc_driver::Callbacks for Callbacks<R> {
        fn config(&mut self, config: &mut rustc_interface::interface::Config) {
            config.opts.unstable_opts.no_codegen = true;
            config.opts.unstable_opts.mir_opt_level = Some(1);
            config
                .opts
                .unstable_opts
                .crate_attr
                .push("allow(unused, non_snake_case)".to_string());
        }

        fn after_analysis<'tcx>(
            &mut self,
            _compiler: &rustc_interface::interface::Compiler,
            tcx: TyCtxt<'tcx>,
        ) -> rustc_driver::Compilation {
            let outcome = match run_bridge(BridgeInput::new(tcx)) {
                Ok(model) => {
                    let f = self.callback.lock().unwrap().take().expect("callback once");
                    Ok(f(&model))
                }
                Err(err) => Err(err.to_string()),
            };
            *self.result.lock().unwrap() = Some(outcome);
            rustc_driver::Compilation::Stop
        }
    }

    let sysroot = get_sysroot();
    let args = vec![
        "rustc".to_string(),
        src_path.to_str().unwrap().to_string(),
        "--sysroot".to_string(),
        sysroot,
        "--edition".to_string(),
        "2021".to_string(),
        "--crate-type".to_string(),
        "rlib".to_string(),
        "--crate-name".to_string(),
        crate_name.to_string(),
        // The bridge's BasicBlock pipeline prunes unwind cleanup blocks under
        // `panic=abort` (the driver compilation contract); a body holding two
        // guards (a nested / recursive / may-sleep-while-held lock) has drop
        // unwind edges that survive otherwise. The e2e fixture toolchain builds
        // with abort too — match it here so multi-guard bodies bridge.
        "-C".to_string(),
        "panic=abort".to_string(),
        "--out-dir".to_string(),
        test_dir.to_str().unwrap().to_string(),
    ];

    let mut callbacks = Callbacks {
        result: result.clone(),
        callback,
    };
    rustc_driver::run_compiler(&args, &mut callbacks);
    let output = result
        .lock()
        .unwrap()
        .take()
        .expect("after_analysis should produce a result");
    std::fs::remove_dir_all(Path::new(&test_dir)).ok();
    output
}

const LIFECYCLE_SURFACE: &str =
    include_str!("../../codegen-coverage-runner/fixtures/_lifecycle_surface.rs");
const SYNC_SURFACE: &str = include_str!("../../codegen-coverage-runner/fixtures/_sync_surface.rs");
const INTERRUPT_SURFACE: &str =
    include_str!("../../codegen-coverage-runner/fixtures/_interrupt_surface.rs");

/// A `regseq::delay::sleep` facade declared inline against the sync surface's
/// `Duration` (so a fixture can exercise the may-sleep `Sleep`-intrinsic arm of
/// the may-sleep-while-held check without a second `Duration` declaration).
const SLEEP_FACADE: &str =
    "#[inline(never)]\n#[rseq::intrinsic(\"sleep\")]\npub fn sleep(_d: Duration) {}\n";

/// A one-lock driver whose `Lifecycle::on_init` body runs `init_body`.
fn sync_source(fields: &str, init_body: &str) -> String {
    format!(
        "#![feature(register_tool)]\n#![register_tool(rseq)]\n\
         {LIFECYCLE_SURFACE}\n{SYNC_SURFACE}\n\
         #[rseq::driver(name = \"d\", compatible = [\"acme,d\"])]\n\
         pub struct D {{ {fields} }}\n\
         impl Lifecycle for D {{\n    fn on_init(&mut self) -> Result<(), RegseqError> {{\n        {init_body}\n        Ok(())\n    }}\n}}\n\
         pub fn make() -> D {{ loop {{}} }}\npub fn entry() {{}}\n"
    )
}

/// A one-lock driver as above, plus the inline `sleep` facade (for the
/// `Sleep`-intrinsic may-sleep-while-held arm).
fn sleep_source(fields: &str, init_body: &str) -> String {
    format!(
        "#![feature(register_tool)]\n#![register_tool(rseq)]\n\
         {LIFECYCLE_SURFACE}\n{SYNC_SURFACE}\n{SLEEP_FACADE}\n\
         #[rseq::driver(name = \"d\", compatible = [\"acme,d\"])]\n\
         pub struct D {{ {fields} }}\n\
         impl Lifecycle for D {{\n    fn on_init(&mut self) -> Result<(), RegseqError> {{\n        {init_body}\n        Ok(())\n    }}\n}}\n\
         pub fn make() -> D {{ loop {{}} }}\npub fn entry() {{}}\n"
    )
}

/// A driver with one `Interrupt` resource + the given lock `fields`, whose bare
/// `#[rseq::isr]` handler body runs `isr_body` (the IRQ-context body the §7.1
/// check consumes the `IsrBinding` marking for).
fn sync_isr_source(fields: &str, isr_body: &str) -> String {
    format!(
        "#![feature(register_tool)]\n#![register_tool(rseq)]\n\
         {LIFECYCLE_SURFACE}\n{INTERRUPT_SURFACE}\n{SYNC_SURFACE}\n\
         #[rseq::driver(name = \"d\", compatible = [\"acme,d\"])]\n\
         pub struct D {{ #[rseq::interrupt(id = \"irq\")] pub irq: Interrupt, {fields} }}\n\
         impl D {{\n    #[rseq::isr]\n    pub fn handle(&mut self) -> IrqResult {{\n        {isr_body}\n        IrqResult::Handled\n    }}\n}}\n\
         impl Lifecycle for D {{\n    fn on_init(&mut self) -> Result<(), RegseqError> {{ Ok(()) }}\n}}\n\
         pub fn make() -> D {{ loop {{}} }}\npub fn entry() {{}}\n"
    )
}

#[test]
fn irqspinlock_field_recognized_as_sync_resource() {
    let (n_drivers, kind, protected, has_marker) = with_model(
        "sync_field",
        &sync_source("pub lock: IrqSpinLock<u32>", "let _g = self.lock.lock();"),
        |model| {
            let n = model.synchronization.drivers.len();
            let driver = model
                .synchronization
                .drivers
                .values()
                .next()
                .expect("one sync driver");
            let res = &driver.resources[0];
            let has_marker = model.domain_attrs.field_attrs.values().any(|attrs| {
                attrs.iter().any(|a| {
                    matches!(
                        a,
                        AttrData::SyncResource {
                            kind: SyncKind::IrqSpinLock
                        }
                    )
                })
            });
            (n, res.kind, res.protected_type, has_marker)
        },
    )
    .expect("a driver with an IrqSpinLock field must bridge");

    assert_eq!(n_drivers, 1, "exactly one driver has a sync surface");
    assert_eq!(
        kind,
        SyncKind::IrqSpinLock,
        "the field's kind is IrqSpinLock"
    );
    assert_eq!(
        protected, None,
        "M1 records no protected type (T07 fills it)"
    );
    assert!(has_marker, "field_attrs must carry the SyncResource marker");
}

#[test]
fn lock_call_recognized_as_acquire_site_with_release() {
    let (n_sites, lock_id, op, n_releases) = with_model(
        "sync_acquire",
        &sync_source("pub lock: IrqSpinLock<u32>", "let _g = self.lock.lock();"),
        |model| {
            let driver = model
                .synchronization
                .drivers
                .values()
                .next()
                .expect("one sync driver");
            let site = driver
                .acquire_sites
                .first()
                .expect("one acquire site recognized");
            (
                driver.acquire_sites.len(),
                site.lock_id,
                site.op,
                site.release_blocks.len(),
            )
        },
    )
    .expect("a lock() acquire must bridge");

    assert_eq!(n_sites, 1, "exactly one acquire site");
    assert_eq!(lock_id, 0, "the only lock has lock_id 0");
    assert_eq!(op, SyncOp::LockAcquire, "the op is a lock acquire");
    assert!(
        n_releases >= 1,
        "the guard's drop-glue release must be recognized (got {n_releases})"
    );
}

#[test]
fn allwaiters_cardinality_is_rejected() {
    // `cardinality = AllWaiters` is deferred (no witness; Zephyr companion-condvar
    // underspecified — GRILL C01/C02). The bridge must reject it.
    let err = with_model(
        "sync_allwaiters",
        &sync_source(
            "#[rseq::sync(cardinality = AllWaiters)] pub done: Completion",
            "self.done.signal();",
        ),
        |_| (),
    )
    .expect_err("AllWaiters must reject");
    assert!(
        err.contains("synchronization validation:") && err.contains("AllWaiters"),
        "AllWaiters reject must render the synchronization diagnostic; got: {err}"
    );
}

#[test]
fn singlewaiter_completion_is_accepted() {
    // The default/only first-slice cardinality bridges.
    let n = with_model(
        "sync_single",
        &sync_source(
            "#[rseq::sync(cardinality = SingleWaiter)] pub done: Completion",
            "self.done.signal();",
        ),
        |model| {
            model
                .synchronization
                .drivers
                .values()
                .next()
                .map(|d| d.completion_sites.len())
                .unwrap_or(0)
        },
    )
    .expect("SingleWaiter Completion must bridge");
    assert_eq!(n, 1, "the signal() op is recognized as a completion site");
}

#[test]
fn driver_without_lock_field_has_no_sync_surface() {
    let n_drivers = with_model("sync_none", &sync_source("pub n: u32", ""), |model| {
        model.synchronization.drivers.len()
    })
    .expect("a lock-less driver must bridge");
    assert_eq!(n_drivers, 0, "no lock field => no synchronization surface");
}

// ============================================================================
// M3 / T10 — the produced spinlock-held atomic-section marking.
// ============================================================================

#[test]
fn irqspinlock_acquire_produces_held_region_over_live_range() {
    let (n_regions, kind, lock_id, acquire_excluded, n_blocks, releases_in_region) = with_model(
        "sync_region",
        &sync_source("pub lock: IrqSpinLock<u32>", "let _g = self.lock.lock();"),
        |model| {
            let driver = model
                .synchronization
                .drivers
                .values()
                .next()
                .expect("one sync driver");
            let region = driver.held_regions.first().expect("one held region");
            let site = driver
                .acquire_sites
                .iter()
                .find(|s| s.lock_id == region.lock_id)
                .expect("the producing acquire site");
            // The acquire block is excluded (the guard is not held while the
            // acquire `Call` itself runs); every release block IS in the region
            // (the guard is held through its statements, dropped at terminator).
            let acquire_excluded = !region.blocks.contains(&region.acquire_block);
            let releases_in_region = site
                .release_blocks
                .iter()
                .all(|b| region.blocks.contains(b));
            (
                driver.held_regions.len(),
                region.kind,
                region.lock_id,
                acquire_excluded,
                region.blocks.len(),
                releases_in_region,
            )
        },
    )
    .expect("a spinlock acquire must bridge and produce a region");

    assert_eq!(n_regions, 1, "one spinlock acquire => one held region");
    assert_eq!(
        kind,
        SyncKind::IrqSpinLock,
        "the region carries the spinning kind"
    );
    assert_eq!(lock_id, 0, "the region tracks the only lock (id 0)");
    assert!(
        acquire_excluded,
        "the acquire block is not in the held region"
    );
    assert!(
        n_blocks >= 1,
        "the guard is held in at least the release block"
    );
    assert!(
        releases_in_region,
        "the guard's release blocks bound the held region (it ends there)"
    );
}

#[test]
fn mutex_acquire_produces_no_region_but_spinlock_does() {
    // A `Mutex` guard is NOT a spinlock-held atomic section (sleeping while
    // holding a sleeping mutex is legal), so it produces no `SpinlockHeldRegion`;
    // the `SpinLock` (sequential, separate scope) does.
    let (n_regions, region_kind) = with_model(
        "sync_mutex_noregion",
        &sync_source(
            "pub m: Mutex<u32>, pub s: SpinLock<u32>",
            "{ let _gm = self.m.lock(); }\n        { let _gs = self.s.lock(); }",
        ),
        |model| {
            let d = model.synchronization.drivers.values().next().unwrap();
            (d.held_regions.len(), d.held_regions.first().map(|r| r.kind))
        },
    )
    .expect("must bridge");
    assert_eq!(
        n_regions, 1,
        "only the SpinLock produces a region; the Mutex does not"
    );
    assert_eq!(region_kind, Some(SyncKind::SpinLock));
}

// ============================================================================
// M3 / T11 — §7.1 ISR-legality + §7.2 wait-context (consume IsrBinding).
// ============================================================================

#[test]
fn mutex_lock_in_isr_is_rejected() {
    let err = with_model(
        "sync_isr_mutex",
        &sync_isr_source("pub m: Mutex<u32>", "let _g = self.m.lock();"),
        |_| (),
    )
    .expect_err("a Mutex::lock in an #[rseq::isr] body must reject");
    assert!(
        err.contains("synchronization validation:")
            && err.contains("Mutex::lock")
            && err.contains("rseq::isr"),
        "the §7.1 reject must name the forbidden op + the IRQ context; got: {err}"
    );
}

#[test]
fn spinlock_lock_in_isr_is_rejected() {
    let err = with_model(
        "sync_isr_spinlock",
        &sync_isr_source("pub s: SpinLock<u32>", "let _g = self.s.lock();"),
        |_| (),
    )
    .expect_err("a SpinLock::lock (thread-only) in an #[rseq::isr] body must reject");
    assert!(
        err.contains("SpinLock::lock") && err.contains("rseq::isr"),
        "the §7.1 reject must name SpinLock::lock; got: {err}"
    );
}

#[test]
fn completion_wait_in_isr_is_rejected() {
    let err = with_model(
        "sync_isr_wait",
        &sync_isr_source(
            "#[rseq::sync(cardinality = SingleWaiter)] pub done: Completion",
            "let _ = self.done.wait(Duration::from_us(1));",
        ),
        |_| (),
    )
    .expect_err("a Completion::wait in an #[rseq::isr] body must reject (§7.2)");
    assert!(
        err.contains("Completion::wait") && err.contains("rseq::isr"),
        "the §7.2 reject must name Completion::wait; got: {err}"
    );
}

#[test]
fn irqspinlock_lock_in_isr_is_accepted() {
    // IrqSpinLock is the ISR-safe acquire — legal in IRQ context, and the
    // produced region lives in the ISR body.
    let n_regions = with_model(
        "sync_isr_irqspin",
        &sync_isr_source("pub lock: IrqSpinLock<u32>", "let _g = self.lock.lock();"),
        |model| {
            model
                .synchronization
                .drivers
                .values()
                .next()
                .map(|d| d.held_regions.len())
                .unwrap_or(0)
        },
    )
    .expect("an IrqSpinLock acquire in an #[rseq::isr] body is legal and must bridge");
    assert_eq!(
        n_regions, 1,
        "the ISR-body IrqSpinLock acquire produces a region"
    );
}

#[test]
fn completion_signal_in_isr_is_accepted() {
    // signal()/reset() are ISR-safe (I07).
    let n_completions = with_model(
        "sync_isr_signal",
        &sync_isr_source(
            "#[rseq::sync(cardinality = SingleWaiter)] pub done: Completion",
            "self.done.signal();",
        ),
        |model| {
            model
                .synchronization
                .drivers
                .values()
                .next()
                .map(|d| d.completion_sites.len())
                .unwrap_or(0)
        },
    )
    .expect("Completion::signal in an #[rseq::isr] body is ISR-safe and must bridge");
    assert_eq!(n_completions, 1, "the signal() op is recognized");
}

// ============================================================================
// M3 / T12 — may-sleep-while-held + recursive same-lock acquire.
// ============================================================================

#[test]
fn mutex_lock_in_spinlock_region_is_rejected() {
    let err = with_model(
        "sync_sleep_mutex",
        &sync_source(
            "pub s: SpinLock<u32>, pub m: Mutex<u32>",
            "let _gs = self.s.lock();\n        let _gm = self.m.lock();",
        ),
        |_| (),
    )
    .expect_err("acquiring a (sleeping) Mutex while a spinlock is held must reject");
    assert!(
        err.contains("may-sleep op `Mutex::lock`") && err.contains("`s`"),
        "the may-sleep-while-held reject must name the op + held spinlock; got: {err}"
    );
}

#[test]
fn completion_wait_in_spinlock_region_is_rejected() {
    let err = with_model(
        "sync_sleep_wait",
        &sync_source(
            "pub s: IrqSpinLock<u32>, #[rseq::sync(cardinality = SingleWaiter)] pub done: Completion",
            "let _gs = self.s.lock();\n        let _ = self.done.wait(Duration::from_us(1));",
        ),
        |_| (),
    )
    .expect_err("Completion::wait while a spinlock is held must reject");
    assert!(
        err.contains("may-sleep op `Completion::wait`"),
        "the may-sleep-while-held reject must name Completion::wait; got: {err}"
    );
}

#[test]
fn sleep_intrinsic_in_spinlock_region_is_rejected() {
    let err = with_model(
        "sync_sleep_intrinsic",
        &sleep_source(
            "pub s: IrqSpinLock<u32>",
            "let _gs = self.s.lock();\n        sleep(Duration::from_us(1));",
        ),
        |_| (),
    )
    .expect_err("a delay::sleep while a spinlock is held must reject");
    assert!(
        err.contains("may-sleep op `regseq::delay::sleep`"),
        "the may-sleep predicate must agree with timed-delay's; got: {err}"
    );
}

#[test]
fn sleep_after_guard_drop_early_is_accepted() {
    // The region ends at `guard_drop_early`, not the syntactic scope: a sleep
    // AFTER the early release is outside the atomic section and must NOT reject.
    with_model(
        "sync_sleep_after_drop",
        &sleep_source(
            "pub s: IrqSpinLock<u32>",
            "let g = self.s.lock();\n        g.guard_drop_early();\n        sleep(Duration::from_us(1));",
        ),
        |_| (),
    )
    .expect("a sleep after the guard is dropped early is outside the region and must bridge");
}

#[test]
fn recursive_same_lock_acquire_is_rejected() {
    // `lock(&self)` permits repeated shared borrows, so this COMPILES, but it
    // re-acquires a non-recursive lock while already held — a self-deadlock the
    // OS backends would lower to a real lock. Detected statically via the region.
    let err = with_model(
        "sync_recursive",
        &sync_source(
            "pub m: Mutex<u32>",
            "let _g1 = self.m.lock();\n        let _g2 = self.m.lock();",
        ),
        |_| (),
    )
    .expect_err("a recursive same-lock acquire must reject");
    assert!(
        err.contains("forbidden lock op") && err.contains("recursive") && err.contains("`m`"),
        "the recursive-acquire reject must name the field; got: {err}"
    );
}

#[test]
fn sequential_same_lock_acquire_is_accepted() {
    // Two acquisitions of the same lock in DISJOINT scopes (the first guard is
    // dropped before the second is acquired) do not overlap — no false positive.
    with_model(
        "sync_sequential",
        &sync_source(
            "pub m: Mutex<u32>",
            "{ let _g1 = self.m.lock(); }\n        let _g2 = self.m.lock();",
        ),
        |_| (),
    )
    .expect("disjoint same-lock acquisitions must bridge (no spurious recursive reject)");
}

#[test]
fn sleep_in_a_loop_inside_a_spinlock_region_is_rejected() {
    // Back-edge coverage: the held region's BFS must reach a loop BODY block, so a
    // may-sleep call inside a loop nested in the critical section is still caught.
    let err = with_model(
        "sync_sleep_loop",
        &sleep_source(
            "pub s: IrqSpinLock<u32>",
            "let _gs = self.s.lock();\n        let mut i = 0u32;\n        \
             while i < 4u32 {\n            sleep(Duration::from_us(1));\n            \
             i = i.wrapping_add(1);\n        }",
        ),
        |_| (),
    )
    .expect_err("a sleep in a loop inside a spinlock region must reject");
    assert!(
        err.contains("may-sleep op `regseq::delay::sleep`"),
        "the region must span loop-body blocks (back-edge); got: {err}"
    );
}

#[test]
fn lock_acquired_and_released_each_loop_iteration_is_accepted() {
    // No false positive on a back-edge: a lock acquired AND dropped within the
    // loop body each iteration is not held across the back-edge, so re-acquiring
    // it next iteration is not recursive.
    with_model(
        "sync_loop_reacquire",
        &sync_source(
            "pub m: Mutex<u32>",
            "let mut i = 0u32;\n        while i < 4u32 {\n            \
             { let _g = self.m.lock(); }\n            i = i.wrapping_add(1);\n        }",
        ),
        |_| (),
    )
    .expect("acquiring+releasing a lock each loop iteration is not recursive and must bridge");
}

#[test]
fn nested_distinct_spinlocks_are_accepted() {
    // Nesting two DIFFERENT spinlocks is legal (LIFO, not recursive, no sleep).
    let n_regions = with_model(
        "sync_nested_distinct",
        &sync_source(
            "pub a: IrqSpinLock<u32>, pub b: IrqSpinLock<u32>",
            "let _ga = self.a.lock();\n        let _gb = self.b.lock();",
        ),
        |model| {
            model
                .synchronization
                .drivers
                .values()
                .next()
                .map(|d| d.held_regions.len())
                .unwrap_or(0)
        },
    )
    .expect("nesting distinct spinlocks is legal and must bridge");
    assert_eq!(
        n_regions, 2,
        "each distinct spinlock acquire produces its own region"
    );
}
