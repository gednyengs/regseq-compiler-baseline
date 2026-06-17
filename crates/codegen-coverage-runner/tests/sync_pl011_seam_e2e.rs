//! M4/T14 — the BILATERAL PL011 ISR↔mainline seam, generated + verified.
//!
//! The `sync-pl011-seam` fixture is the GENERATED locked PL011 driver in
//! miniature: a bound `#[rseq::isr]` AND a mainline reconfigure path BOTH acquire
//! the SAME `IrqSpinLock<IsrState>`. This test confirms the construct lowers that
//! seam correctly on Linux + Zephyr — the lock that orders the two contexts (the
//! mechanism that discharges the Tier-2 pin) is the SAME field on both sides, the
//! ISR wrapper still carries no auto-acquire (G-SYN-17), and `init_primitives`
//! precedes the interrupt register (C05/I05). The live SMP race test remains the
//! documented operational gate (`pl011/TIER2-PIN-device-interrupt.md`); this is
//! the codegen-level proof that the locked driver the campaign needs is producible.
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

/// Bridge the `sync-pl011-seam` input once (unbound), then L2-bind a fresh clone
/// to the requested OS's platform file and emit.
fn emit_for(binding: manifest::OsBinding) -> String {
    let fixtures = manifest::discover(&corpus()).expect("discover _sync_irq corpus");
    let base = fixtures
        .iter()
        .find(|f| f.manifest.name == "sync-pl011-seam")
        .expect("sync-pl011-seam fixture discovered");
    let unbound = match try_bridge_fixture_path(&base.input) {
        BridgeOutcome::Ok(m) => m,
        other => panic!("bridge failed: {other:?}"),
    };
    let fixture = base.with_binding(binding).expect("resolve OS binding");
    let mut model = unbound.clone();
    pipeline::bind_l2_platform(&mut model, &fixture).expect("L2 bind");
    match binding {
        manifest::OsBinding::Linux => regseq_backends::emit_linux_c(&model)
            .expect("emit_linux_c")
            .tu,
        manifest::OsBinding::Zephyr => regseq_backends::emit_zephyr_c(&model)
            .expect("emit_zephyr_c")
            .tu,
    }
}

/// The text of the C function DEFINITION whose signature starts at `sig`, up to
/// its top-level closing brace. The TU emits forward declarations (`sig …);`)
/// before definitions (`sig …)\n{`), so we skip occurrences that are declarations
/// and slice the one whose signature line is followed by `\n{`.
fn fn_body<'a>(tu: &'a str, sig: &str) -> &'a str {
    let mut from = 0usize;
    loop {
        let rel = tu[from..]
            .find(sig)
            .unwrap_or_else(|| panic!("definition `{sig}` not found in:\n{tu}"));
        let start = from + rel;
        let line_end = tu[start..].find('\n').map(|n| start + n).unwrap_or(tu.len());
        // A definition's signature line is immediately followed by a `{` line; a
        // forward declaration's line ends with `;`.
        if tu[line_end..].starts_with("\n{") {
            let rest = &tu[start..];
            let end = rest.find("\n}\n").expect("function close") + start + 3;
            return &tu[start..end];
        }
        from = line_end + 1;
    }
}

/// Assert `first` appears before `second` in `tu`.
fn assert_before(tu: &str, first: &str, second: &str, what: &str) {
    let a = tu.find(first).unwrap_or_else(|| panic!("{what}: `{first}` not found"));
    let b = tu.find(second).unwrap_or_else(|| panic!("{what}: `{second}` not found"));
    assert!(a < b, "{what}: expected `{first}` before `{second}`");
}

#[test]
fn linux_isr_and_mainline_acquire_the_same_seam_lock() {
    let lx = &emit_for(manifest::OsBinding::Linux);

    // The lock field lowers to the per-OS primitive (spinlock_t) — the SAME field
    // both contexts acquire.
    assert!(
        lx.contains("spinlock_t isr_state;"),
        "the IrqSpinLock<IsrState> field must lower to a spinlock_t named isr_state:\n{lx}"
    );

    // The ISR wrapper dispatches; NO auto-acquire glue (G-SYN-17).
    let lx_wrapper = fn_body(lx, "static irqreturn_t rseq_isr_sync_pl011_seam(");
    assert!(
        !lx_wrapper.contains("spin_lock"),
        "the ISR wrapper must NOT auto-acquire:\n{lx_wrapper}"
    );

    // ISR side: the handler body explicitly acquires the seam lock + releases on
    // BOTH exits (early NotHandled + Handled).
    let isr = fn_body(lx, "static struct IrqResult sync_pl011_seam_handle_irq(");
    assert!(
        isr.contains("spin_lock_irqsave(&_1->isr_state,"),
        "the ISR must acquire the seam lock:\n{isr}"
    );
    assert_eq!(
        isr.matches("spin_unlock_irqrestore(&_1->isr_state,").count(),
        2,
        "the ISR guard must release on BOTH branch exits:\n{isr}"
    );

    // MAINLINE side: the reconfigure path acquires the SAME lock — the bilateral
    // seam — and touches the shared `imsc` register WHILE held (the Open-2
    // in-guard register-placement scenario).
    let main = fn_body(lx, "static void sync_pl011_seam_mainline_reconfigure(");
    assert!(
        main.contains("spin_lock_irqsave(&_1->isr_state,"),
        "the mainline path must acquire the SAME seam lock:\n{main}"
    );
    assert!(
        main.contains("spin_unlock_irqrestore(&_1->isr_state,"),
        "the mainline path must release the seam lock:\n{main}"
    );
    // The in-guard register write (imsc) is present in the locked mainline body —
    // the Open-2 register-placement scenario. This DEMONSTRATES the in-guard
    // access (the write is in a body that acquires+releases the lock); the
    // STRUCTURAL in-guard proof (the write's block is inside the guard's
    // SpinlockHeldRegion) is the T10 marking, tested at the bridge level. Textual
    // order is NOT asserted (block-structured C: source order ≠ text order).
    assert!(
        main.contains("SYNC_PL011_SEAM_IMSC"),
        "the mainline path must carry the in-guard imsc register write:\n{main}"
    );

    // Lifecycle reachability: the mainline acquire is actually REACHED — on_init
    // (and on_deinit) call the reconfigure path, so the seam is exercised, not
    // merely defined.
    let init = fn_body(lx, "static struct Result sync_pl011_seam_on_init(");
    assert!(
        init.contains("sync_pl011_seam_mainline_reconfigure("),
        "on_init must call the mainline reconfigure path (the seam is reached):\n{init}"
    );

    // C05/I05: init_primitives (spin_lock_init) precedes the interrupt register.
    assert!(
        lx.contains("spin_lock_init(&priv->inner.isr_state);"),
        "the seam lock must be init'd in the probe:\n{lx}"
    );
    assert_before(
        lx,
        "spin_lock_init(&priv->inner.isr_state);",
        "request_irq(",
        "init_primitives before the interrupt register (C05/I05)",
    );
}

#[test]
fn zephyr_isr_and_mainline_acquire_the_same_seam_lock() {
    let zp = &emit_for(manifest::OsBinding::Zephyr);

    assert!(
        zp.contains("struct k_spinlock isr_state;"),
        "the IrqSpinLock<IsrState> field must lower to a k_spinlock named isr_state:\n{zp}"
    );

    let zp_wrapper = fn_body(zp, "static void rseq_isr_sync_pl011_seam(");
    assert!(
        !zp_wrapper.contains("k_spin_lock"),
        "the ISR wrapper must NOT auto-acquire:\n{zp_wrapper}"
    );

    let isr = fn_body(zp, "static struct IrqResult sync_pl011_seam_handle_irq(");
    assert!(
        isr.contains("k_spin_lock(&_1->isr_state)"),
        "the ISR must acquire the seam lock:\n{isr}"
    );
    assert_eq!(
        isr.matches("k_spin_unlock(&_1->isr_state,").count(),
        2,
        "the ISR guard must release on BOTH branch exits:\n{isr}"
    );

    let main = fn_body(zp, "static void sync_pl011_seam_mainline_reconfigure(");
    assert!(
        main.contains("k_spin_lock(&_1->isr_state)"),
        "the mainline path must acquire the SAME seam lock:\n{main}"
    );
    assert!(
        main.contains("k_spin_unlock(&_1->isr_state,"),
        "the mainline path must release the seam lock:\n{main}"
    );
}
