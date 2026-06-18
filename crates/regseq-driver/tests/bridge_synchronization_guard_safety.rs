//! M3 / T12 — the move/borrow-enforced halves of the §7 rejection catalog.
//!
//! `guard-data-escape` (§7.4), `guard-use-after-consume` (§7.11), and the
//! `try_lock` / standalone-`unlock` halves of `forbidden-try-lock-or-unlock`
//! (§7.5/6/7) are **not** bridge diagnostics in the first slice: the author
//! surface makes them *unrepresentable*, so the Rust compiler rejects them
//! before the bridge ever runs (PLAN T12 — "most enforced for free by Rust
//! move/borrow semantics … fixtures only"). These tests pin that: each
//! violation source fails a `rustc` type-check, while the clean control passes.
//! (The reachable recursive-acquire half of `forbidden-try-lock-or-unlock` IS a
//! bridge diagnostic — see
//! `bridge_synchronization_recognition::recursive_same_lock_acquire_is_rejected`.)
//!
//! These are a subprocess `rustc --emit=metadata` type-check (the rseq tool
//! attributes are inert to rustc under `register_tool`, so the only thing under
//! test is the *Rust* legality of the body) — not a bridge run; the bridge's
//! `after_analysis` hook is reached even on a borrow error, so it cannot
//! classify these as rejected.

use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

const LIFECYCLE_SURFACE: &str =
    include_str!("../../codegen-coverage-runner/fixtures/_lifecycle_surface.rs");
const SYNC_SURFACE: &str = include_str!("../../codegen-coverage-runner/fixtures/_sync_surface.rs");

fn sysroot() -> String {
    let out = Command::new("rustc")
        .args(["--print", "sysroot"])
        .output()
        .expect("rustc --print sysroot");
    String::from_utf8(out.stdout).unwrap().trim().to_string()
}

/// Compose a one-`IrqSpinLock` driver whose `on_init` body is `init_body`, write
/// it to a fresh temp dir, and `rustc --emit=metadata` it. Returns whether the
/// type-check SUCCEEDED (true) or rustc rejected the source (false).
fn rustc_accepts(tag: &str, init_body: &str) -> bool {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir: PathBuf = std::env::temp_dir().join(format!("regseq-guard-safety-{tag}-{nanos}"));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let source = format!(
        "#![feature(register_tool)]\n#![register_tool(rseq)]\n#![allow(unused)]\n\
         {LIFECYCLE_SURFACE}\n{SYNC_SURFACE}\n\
         #[rseq::driver(name = \"d\", compatible = [\"acme,d\"])]\n\
         pub struct D {{ pub lock: IrqSpinLock<u32> }}\n\
         impl Lifecycle for D {{\n    fn on_init(&mut self) -> Result<(), RegseqError> {{\n        {init_body}\n        Ok(())\n    }}\n}}\n\
         pub fn make() -> D {{ loop {{}} }}\npub fn entry() {{}}\n"
    );
    let src = dir.join("lib.rs");
    std::fs::write(&src, source).expect("write source");
    let status = Command::new("rustc")
        .args(["--sysroot", &sysroot()])
        .args(["--edition", "2021"])
        .args(["--crate-type", "rlib"])
        .args(["--emit", "metadata"])
        .arg("--out-dir")
        .arg(&dir)
        .arg(&src)
        .status()
        .expect("run rustc");
    let _ = std::fs::remove_dir_all(&dir);
    status.success()
}

#[test]
fn clean_acquire_is_accepted_control() {
    // The positive control: the same shape WITHOUT a violation type-checks, so a
    // rejection below is attributable to the violation, not the scaffold.
    assert!(
        rustc_accepts("control", "let _g = self.lock.lock();"),
        "the clean control must type-check"
    );
}

#[test]
fn guard_use_after_consume_is_rustc_rejected() {
    // `guard_drop_early(self)` consumes the guard by value; using it afterward is
    // a use-after-move — rustc rejects it, so the first slice needs no bridge
    // diagnostic for §7.11.
    assert!(
        !rustc_accepts(
            "use_after_consume",
            "let g = self.lock.lock();\n        g.guard_drop_early();\n        g.guard_drop_early();",
        ),
        "use-after-guard_drop_early must be rejected by rustc"
    );
}

#[test]
fn guard_data_escape_is_rustc_rejected() {
    // The first-slice guard is OPAQUE (no `Deref`/`DerefMut`), so the protected
    // `&T` cannot be obtained — borrowed data cannot escape the critical section
    // (§7.4). The deref attempt is a type error.
    assert!(
        !rustc_accepts(
            "data_escape",
            "let g = self.lock.lock();\n        let _r: &u32 = &*g;",
        ),
        "dereferencing the opaque guard must be rejected by rustc"
    );
}

#[test]
fn standalone_unlock_is_rustc_rejected() {
    // There is no standalone `unlock` (release is via the guard's drop /
    // `guard_drop_early` only — I02); the method does not exist.
    assert!(
        !rustc_accepts("unlock", "self.lock.unlock();"),
        "a standalone `unlock` method does not exist and must be rejected by rustc"
    );
}

#[test]
fn try_lock_is_rustc_rejected() {
    // There is no `try_lock` in the first-slice surface (only blocking `lock`).
    assert!(
        !rustc_accepts("try_lock", "let _g = self.lock.try_lock();"),
        "a `try_lock` method does not exist and must be rejected by rustc"
    );
}
