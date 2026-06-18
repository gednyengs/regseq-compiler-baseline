//! Per-OS synchronization lowering (T07/C02): the `sync-os` AMBA driver — all
//! four lock/signal kinds — bridged once, then lowered by `emit_linux_c` and
//! `emit_zephyr_c`, asserting the C02 primitive matrix + the per-OS guard hidden
//! state (Linux `unsigned long flags`, Zephyr `k_spinlock_key_t key`).
//!
//! Sole-binary (the bridged MIR is sensitive to rustc's process-global interner
//! history); substring assertions, no committed byte golden.

#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::{Path, PathBuf};

use codegen_coverage_runner::{manifest, pipeline};
use test_ir_fixtures::{try_bridge_fixture_path, BridgeOutcome};

fn sync_os_corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/_sync_os")
}

fn bridged() -> regseq_model::RegseqModel {
    let fixtures = manifest::discover(&sync_os_corpus()).expect("discover _sync_os corpus");
    let fixture = fixtures
        .iter()
        .find(|f| f.manifest.name == "sync-os")
        .expect("sync-os fixture discovered");
    let mut model = match try_bridge_fixture_path(&fixture.input) {
        BridgeOutcome::Ok(m) => m,
        other => panic!("bridge failed: {other:?}"),
    };
    pipeline::bind_l2_platform(&mut model, fixture).expect("L2 bind");
    model
}

/// Codex-M2 blocking #1 / T07-lowering §D4: a `Completion::wait` whose `Result`
/// is CONSUMED must be rejected at recognition (never wrong code), not lowered
/// to a discarded result over an unset destination.
#[test]
fn completion_wait_consumed_result_is_rejected() {
    use regseq_driver::bridge::{BridgeError, SyncDiagnostic};
    let fixtures = manifest::discover(&sync_os_corpus()).expect("discover _sync_os corpus");
    let fixture = fixtures
        .iter()
        .find(|f| f.manifest.name == "sync-wait-consumed")
        .expect("sync-wait-consumed fixture discovered");
    match try_bridge_fixture_path(&fixture.input) {
        BridgeOutcome::Rejected(BridgeError::SynchronizationValidation(
            SyncDiagnostic::CompletionWaitResultConsumed { field },
        )) => {
            assert_eq!(field, "done", "the rejected field is the Completion");
        }
        other => panic!(
            "expected CompletionWaitResultConsumed rejection, got: {other:?}"
        ),
    }
}

/// The taint pass also catches a wait result consumed through a BORROW
/// (`r.is_ok()` takes `&r`), not just a direct move/discriminant.
#[test]
fn completion_wait_consumed_by_borrow_is_rejected() {
    use regseq_driver::bridge::{BridgeError, SyncDiagnostic};
    let fixtures = manifest::discover(&sync_os_corpus()).expect("discover _sync_os corpus");
    let fixture = fixtures
        .iter()
        .find(|f| f.manifest.name == "sync-wait-byref")
        .expect("sync-wait-byref fixture discovered");
    match try_bridge_fixture_path(&fixture.input) {
        BridgeOutcome::Rejected(BridgeError::SynchronizationValidation(
            SyncDiagnostic::CompletionWaitResultConsumed { .. },
        )) => {}
        other => panic!("expected CompletionWaitResultConsumed (by borrow), got: {other:?}"),
    }
}

/// An unrecognized `cardinality` value is rejected, not silently defaulted.
#[test]
fn unknown_cardinality_is_rejected() {
    use regseq_driver::bridge::{BridgeError, SyncDiagnostic};
    let fixtures = manifest::discover(&sync_os_corpus()).expect("discover _sync_os corpus");
    let fixture = fixtures
        .iter()
        .find(|f| f.manifest.name == "sync-cardinality-bogus")
        .expect("sync-cardinality-bogus fixture discovered");
    match try_bridge_fixture_path(&fixture.input) {
        BridgeOutcome::Rejected(BridgeError::SynchronizationValidation(
            SyncDiagnostic::UnknownCardinality { field, value },
        )) => {
            assert_eq!(field, "done");
            assert_eq!(value, "Bogus");
        }
        other => panic!("expected UnknownCardinality rejection, got: {other:?}"),
    }
}

/// A non-identifier (literal) cardinality value is also rejected (not defaulted).
#[test]
fn non_identifier_cardinality_is_rejected() {
    use regseq_driver::bridge::{BridgeError, SyncDiagnostic};
    let fixtures = manifest::discover(&sync_os_corpus()).expect("discover _sync_os corpus");
    let fixture = fixtures
        .iter()
        .find(|f| f.manifest.name == "sync-cardinality-literal")
        .expect("sync-cardinality-literal fixture discovered");
    match try_bridge_fixture_path(&fixture.input) {
        BridgeOutcome::Rejected(BridgeError::SynchronizationValidation(
            SyncDiagnostic::UnknownCardinality { .. },
        )) => {}
        other => panic!("expected UnknownCardinality (literal) rejection, got: {other:?}"),
    }
}

fn assert_has(tu: &str, needle: &str, what: &str) {
    assert!(tu.contains(needle), "{what}: expected `{needle}` in:\n{tu}");
}

fn assert_absent(tu: &str, needle: &str, what: &str) {
    assert!(!tu.contains(needle), "{what}: did NOT expect `{needle}` in:\n{tu}");
}

fn assert_before(tu: &str, first: &str, second: &str, what: &str) {
    let a = tu.find(first).unwrap_or_else(|| panic!("{what}: `{first}` not found"));
    let b = tu.find(second).unwrap_or_else(|| panic!("{what}: `{second}` not found"));
    assert!(a < b, "{what}: expected `{first}` before `{second}` in:\n{tu}");
}

/// T07/C02 — the full per-OS lowering matrix + the per-OS guard hidden-state
/// round-trip + the Zephyr `SpinLock ≡ IrqSpinLock` collapse (I06), over the
/// `sync-os` AMBA driver. One bridge; both backends emitted from the same model.
#[test]
fn per_os_c02_lowering_matrix() {
    let model = bridged();
    let linux = regseq_backends::emit_linux_c(&model).expect("emit_linux_c");
    let zephyr = regseq_backends::emit_zephyr_c(&model).expect("emit_zephyr_c");
    let lx = &linux.tu;
    let zp = &zephyr.tu;

    // -- Linux struct field substitution + per-kind includes (D1) -----------
    assert_has(lx, "struct mutex mtx;", "linux: Mutex field");
    assert_has(lx, "spinlock_t spin;", "linux: SpinLock field");
    assert_has(lx, "spinlock_t irqspin;", "linux: IrqSpinLock field");
    assert_has(lx, "struct completion done;", "linux: Completion field");
    assert_has(lx, "#include <linux/mutex.h>", "linux: mutex include");
    assert_has(lx, "#include <linux/spinlock.h>", "linux: spinlock include");
    assert_has(lx, "#include <linux/completion.h>", "linux: completion include");

    // -- Linux C02 primitive matrix -----------------------------------------
    assert_has(lx, "mutex_lock(&_1->mtx);", "linux: Mutex acquire");
    assert_has(lx, "mutex_unlock(&_1->mtx);", "linux: Mutex release");
    assert_has(lx, "spin_lock(&_1->spin);", "linux: SpinLock acquire");
    assert_has(lx, "spin_unlock(&_1->spin);", "linux: SpinLock release");
    assert_has(lx, "complete(&_1->done);", "linux: completion signal");
    assert_has(lx, "reinit_completion(&_1->done);", "linux: completion reset");
    assert_has(lx, "wait_for_completion_timeout(&_1->done,", "linux: completion wait");

    // -- Linux IrqSpinLock guard-state round-trip (S10, review D2): the same
    //    `unsigned long flags` local is WRITTEN by irqsave and CONSUMED by
    //    irqrestore over the SAME lock object. -------------------------------
    let flags = extract_token(lx, "spin_lock_irqsave(&_1->irqspin, ", ")");
    assert_has(lx, &format!("unsigned long {flags};"), "linux: flags local declared");
    assert_has(
        lx,
        &format!("spin_lock_irqsave(&_1->irqspin, {flags});"),
        "linux: irqsave writes flags",
    );
    assert_has(
        lx,
        &format!("spin_unlock_irqrestore(&_1->irqspin, {flags});"),
        "linux: irqrestore consumes the same flags",
    );

    // -- Zephyr struct field substitution + the collapse (D1/I06) -----------
    assert_has(zp, "struct k_mutex mtx;", "zephyr: Mutex field");
    assert_has(zp, "struct k_spinlock spin;", "zephyr: SpinLock field (collapse)");
    assert_has(zp, "struct k_spinlock irqspin;", "zephyr: IrqSpinLock field (collapse)");
    assert_has(zp, "struct k_sem done;", "zephyr: Completion field");
    assert_has(zp, "#include <zephyr/kernel.h>", "zephyr: kernel include");
    assert_has(zp, "#include <zephyr/spinlock.h>", "zephyr: spinlock include");

    // -- Zephyr C02 primitive matrix ----------------------------------------
    assert_has(zp, "k_mutex_lock(&_1->mtx, K_FOREVER);", "zephyr: Mutex acquire");
    assert_has(zp, "k_mutex_unlock(&_1->mtx);", "zephyr: Mutex release");
    assert_has(zp, "k_sem_give(&_1->done);", "zephyr: completion signal");
    assert_has(zp, "k_sem_reset(&_1->done);", "zephyr: completion reset");
    assert_has(zp, "k_sem_take(&_1->done, K_NSEC(", "zephyr: completion wait");

    // -- Zephyr SpinLock ≡ IrqSpinLock collapse: BOTH use the key-carrying
    //    `k_spin_lock`/`k_spin_unlock` (I06) -------------------------------
    assert_has(zp, "k_spin_lock(&_1->spin);", "zephyr: SpinLock acquire (collapse)");
    assert_has(zp, "k_spin_lock(&_1->irqspin);", "zephyr: IrqSpinLock acquire (collapse)");

    // -- Zephyr guard-state round-trip (S10, review D2): `k_spinlock_key_t key`
    //    ASSIGNED FROM the acquire and CONSUMED by the unlock (contrast Linux's
    //    irqsave-written flags). --------------------------------------------
    let key = extract_token(zp, "k_spin_unlock(&_1->irqspin, ", ")");
    assert_has(zp, &format!("k_spinlock_key_t {key};"), "zephyr: key local declared");
    assert_has(
        zp,
        &format!("{key} = k_spin_lock(&_1->irqspin);"),
        "zephyr: key assigned from acquire",
    );

    // -- Anti-narrowing / no-payload (D1): neither backend leaks the facade
    //    types nor stores the protected `T`. --------------------------------
    for (tu, os) in [(lx, "linux"), (zp, "zephyr")] {
        assert_absent(tu, "struct IrqSpinLock", &format!("{os}: no facade leak"));
        assert_absent(tu, "struct Guard", &format!("{os}: no guard struct leak"));
        assert_absent(tu, "UnsafeCell", &format!("{os}: no payload-cell leak"));
    }
}

/// T08/C05 — the `init_primitives` / `destroy_primitives` lifecycle placement:
/// per-OS init in the probe/init hub BEFORE `on_init` (lock-live ⊇ line-live),
/// the generic-c oracle's `lock_init`-before-`lock_acquire` + `lock_destroy`
/// after release (the I05 invariant made checkable), and the Zephyr
/// spinlock-no-init / destroy-no-op dispositions.
#[test]
fn c05_init_destroy_placement() {
    let model = bridged();
    let linux = regseq_backends::emit_linux_c(&model).expect("emit_linux_c");
    let zephyr = regseq_backends::emit_zephyr_c(&model).expect("emit_zephyr_c");
    let generic = regseq_backends::emit_generic_c(&model).expect("emit_generic_c");
    let lx = &linux.tu;
    let zp = &zephyr.tu;
    let gc = &generic;

    // -- Linux: init_primitives in the probe, BEFORE on_init -----------------
    assert_has(lx, "mutex_init(&priv->inner.mtx);", "linux: Mutex init");
    assert_has(lx, "spin_lock_init(&priv->inner.spin);", "linux: SpinLock init");
    assert_has(lx, "spin_lock_init(&priv->inner.irqspin);", "linux: IrqSpinLock init");
    assert_has(lx, "init_completion(&priv->inner.done);", "linux: Completion init");
    assert_before(lx, "mutex_init(&priv->inner.mtx);", "sync_os_on_init(&priv->inner)", "linux: init before on_init");

    // -- Zephyr: k_mutex_init/k_sem_init in the init hub, BEFORE on_init; a
    //    k_spinlock needs NO runtime init (zero-init valid). ----------------
    assert_has(zp, "k_mutex_init(&data->inner.mtx);", "zephyr: Mutex init");
    assert_has(zp, "k_sem_init(&data->inner.done, 0, 1);", "zephyr: Completion init");
    assert_absent(zp, "k_spinlock_init", "zephyr: spinlock takes no runtime init");
    assert_before(zp, "k_mutex_init(&data->inner.mtx);", "sync_os_on_init(&data->inner)", "zephyr: init before on_init");

    // -- generic-c oracle: lock_init BEFORE the first lock_acquire (acquire-
    //    after-init, I05), and lock_destroy emitted after the section. ------
    assert_has(gc, "regseq_lock_init(", "generic-c: init emitted");
    assert_has(gc, "regseq_lock_destroy(", "generic-c: destroy emitted");
    assert_before(gc, "regseq_lock_init(", "regseq_lock_acquire(", "generic-c: init before acquire");
    // The oracle's acquire/completion ops route through the shared
    // acquire-after-init guard (which prints `SYNC error <op>-before-init` and
    // aborts at runtime) + the id-bounds overflow guard.
    assert_has(gc, "regseq_lock_require_inited(", "generic-c: oracle wires the acquire-after-init guard");
    assert_has(gc, "-before-init", "generic-c: the init-guard diagnostic is present");
}

/// Pull the single token between `after` and the next `until` (a tiny golden
/// helper so the per-OS guard-state assertions name the actual emitted local).
fn extract_token(tu: &str, after: &str, until: &str) -> String {
    let start = tu
        .find(after)
        .unwrap_or_else(|| panic!("`{after}` not found in:\n{tu}"))
        + after.len();
    let rest = &tu[start..];
    let end = rest.find(until).expect("closing token");
    rest[..end].trim().to_string()
}
