//! T09 / G-SYN-17 — `subsystem_locking` is a NULL surface: the framework ISR
//! wrapper does NOT auto-acquire on the bound handler's behalf. The combined
//! `sync-irq` fixture (a bound `#[rseq::isr]` that acquires an `IrqSpinLock`
//! EXPLICITLY) is lowered by linux_c + zephyr_c; the generated wrapper
//! (`rseq_isr_*`) carries no acquire glue, while the explicit `lock()` lowers
//! into the HANDLER body (the §6.1 worked example).
//!
//! Sole-binary (rustc interner sensitivity); substring assertions, no golden.

#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::{Path, PathBuf};

use codegen_coverage_runner::{manifest, pipeline};
use test_ir_fixtures::{try_bridge_fixture_path, BridgeOutcome};

fn corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/_sync_irq")
}

/// Bridge the `sync-irq` input once (unbound), then L2-bind a fresh clone to the
/// requested OS's platform file and emit. The combined fixture has a bus-managed
/// MMIO region, so the per-OS binding (`with_binding`) is required.
fn emit_for(binding: manifest::OsBinding) -> String {
    let fixtures = manifest::discover(&corpus()).expect("discover _sync_irq corpus");
    let base = fixtures
        .iter()
        .find(|f| f.manifest.name == "sync-irq")
        .expect("sync-irq fixture discovered");
    let unbound = match try_bridge_fixture_path(&base.input) {
        BridgeOutcome::Ok(m) => m,
        other => panic!("bridge failed: {other:?}"),
    };
    let fixture = base.with_binding(binding).expect("resolve OS binding");
    let mut model = unbound.clone();
    pipeline::bind_l2_platform(&mut model, &fixture).expect("L2 bind");
    match binding {
        manifest::OsBinding::Linux => {
            regseq_backends::emit_linux_c(&model)
                .expect("emit_linux_c")
                .tu
        }
        manifest::OsBinding::Zephyr => {
            regseq_backends::emit_zephyr_c(&model)
                .expect("emit_zephyr_c")
                .tu
        }
    }
}

/// The text of the C function whose definition starts at `sig`, up to its
/// top-level closing brace (`\n}\n`). The emitted wrappers have no nested
/// brace-blocks at column 0, so this isolates exactly one function.
fn fn_body<'a>(tu: &'a str, sig: &str) -> &'a str {
    let start = tu
        .find(sig)
        .unwrap_or_else(|| panic!("function `{sig}` not found in:\n{tu}"));
    let rest = &tu[start..];
    let end = rest.find("\n}\n").expect("function close") + start + 3;
    &tu[start..end]
}

#[test]
fn isr_wrapper_does_not_auto_acquire() {
    let lx = &emit_for(manifest::OsBinding::Linux);
    let zp = &emit_for(manifest::OsBinding::Zephyr);

    // -- Linux: the irqreturn_t wrapper dispatches, NO auto-acquire glue ------
    let lx_wrapper = fn_body(lx, "static irqreturn_t rseq_isr_sync_irq(");
    assert!(
        !lx_wrapper.contains("spin_lock"),
        "G-SYN-17 (linux): the ISR wrapper must NOT auto-acquire:\n{lx_wrapper}"
    );
    // The explicit author acquire lowered into the HANDLER body, not the wrapper.
    let lx_handler = fn_body(lx, "static struct IrqResult sync_irq_handle_irq(");
    assert!(
        lx_handler.contains("spin_lock_irqsave(&_1->lock,"),
        "linux: the handler body must carry the explicit IrqSpinLock acquire:\n{lx_handler}"
    );
    assert!(
        lx_handler.contains("spin_unlock_irqrestore(&_1->lock,"),
        "linux: the handler must release on exit (every path):\n{lx_handler}"
    );
    // The guard releases on BOTH ISR exits — the early `NotHandled` return and
    // the normal `Handled` (a branchy-CFG release, multiple release_blocks).
    assert_eq!(
        lx_handler
            .matches("spin_unlock_irqrestore(&_1->lock,")
            .count(),
        2,
        "linux: the guard must release on BOTH branch exits of the ISR:\n{lx_handler}"
    );
    // C05/I05: init_primitives (spin_lock_init) precedes the interrupt register
    // (request_irq) — the init-before-enable ordering with an actual ISR present.
    assert!(
        lx.contains("spin_lock_init(&priv->inner.lock);"),
        "linux: the IrqSpinLock must be init'd in the probe:\n{lx}"
    );
    assert_before(
        lx,
        "spin_lock_init(&priv->inner.lock);",
        "request_irq(",
        "linux: init_primitives before the interrupt register (C05/I05)",
    );

    // -- Zephyr: the void wrapper dispatches, NO auto-acquire glue ------------
    let zp_wrapper = fn_body(zp, "static void rseq_isr_sync_irq(");
    assert!(
        !zp_wrapper.contains("k_spin_lock"),
        "G-SYN-17 (zephyr): the ISR wrapper must NOT auto-acquire:\n{zp_wrapper}"
    );
    let zp_handler = fn_body(zp, "static struct IrqResult sync_irq_handle_irq(");
    assert!(
        zp_handler.contains("k_spin_lock(&_1->lock)"),
        "zephyr: the handler body must carry the explicit acquire:\n{zp_handler}"
    );
    assert!(
        zp_handler.contains("k_spin_unlock(&_1->lock,"),
        "zephyr: the handler must release on exit:\n{zp_handler}"
    );
    assert_eq!(
        zp_handler.matches("k_spin_unlock(&_1->lock,").count(),
        2,
        "zephyr: the guard must release on BOTH branch exits of the ISR:\n{zp_handler}"
    );
}

/// Assert `first` appears before `second` in `tu`.
fn assert_before(tu: &str, first: &str, second: &str, what: &str) {
    let a = tu
        .find(first)
        .unwrap_or_else(|| panic!("{what}: `{first}` not found"));
    let b = tu
        .find(second)
        .unwrap_or_else(|| panic!("{what}: `{second}` not found"));
    assert!(a < b, "{what}: expected `{first}` before `{second}`");
}
