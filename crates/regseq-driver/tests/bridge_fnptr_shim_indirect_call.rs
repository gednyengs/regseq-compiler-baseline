//! regseq-driver-77: a bare `fn` coerced to a function pointer and
//! invoked through a generic `Fn` / `FnMut` / `FnOnce` bound
//! monomorphises to an `InstanceKind::FnPtrShim` whose `Fn*::call`
//! signature is `extern "rust-call"`. At HEAD the bridge rejected it
//! with `UnsupportedAbi { RustCall }`. The fix lowers the fn-pointer
//! `Fn*::call` to a `CallCallee::Indirect` through the
//! (deref'd-per-kind) fn-pointer receiver, so no `FnPtrShim` Instance is
//! materialised -- mirroring how the closure `Fn::call` family is
//! covered via the `ClosureCall` rewrite.
//!
//! Each case wraps the in-process bridge run in `catch_unwind` so a
//! regression ICE surfaces as a test failure rather than aborting the
//! whole test binary.

#![feature(rustc_private)]

extern crate rustc_driver;

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use regseq_model::{
    CallCallee, FunctionInstanceFlavor, RegseqModel, SyntheticItemKind, TerminatorKind,
};
use test_ir_fixtures::{try_bridge_fixture_path, BridgeOutcome};

/// Write `source` to a uniquely named temp `.rs` file and return its
/// path. The file is left in the temp dir; the OS reclaims it.
fn write_snippet(label: &str, source: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time after unix epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("regseq-fnptr-shim-{label}-{nanos}.rs"));
    std::fs::write(&path, source).expect("write temp snippet");
    path
}

/// Bridge `source` and assert the bridge AND `validate()` both succeed,
/// returning the model. The bridge call is wrapped in `catch_unwind`:
/// at HEAD these snippets reject (or, once the call no longer rejects
/// first, the unmasked no-op drop could ICE), so a regression must be
/// reported as a failure, not propagate as an abort.
fn bridge_ok(label: &str, source: &str) -> RegseqModel {
    let path = write_snippet(label, source);
    let outcome = catch_unwind(AssertUnwindSafe(|| try_bridge_fixture_path(&path)));
    let _ = std::fs::remove_file(&path);
    match outcome {
        Ok(BridgeOutcome::Ok(model)) => model,
        Ok(other) => panic!("expected BridgeOutcome::Ok for {label}, got {other:?}"),
        Err(_) => panic!("bridge panicked (regression ICE?) for {label}"),
    }
}

/// Bridge `source` and assert the bridge `Rejected` (the boundary case).
fn bridge_rejected(label: &str, source: &str) {
    let path = write_snippet(label, source);
    let outcome = catch_unwind(AssertUnwindSafe(|| try_bridge_fixture_path(&path)));
    let _ = std::fs::remove_file(&path);
    match outcome {
        Ok(BridgeOutcome::Rejected(_)) => {}
        Ok(other) => panic!("expected BridgeOutcome::Rejected for {label}, got {other:?}"),
        Err(_) => panic!("bridge panicked (regression ICE?) for {label}"),
    }
}

/// True if any materialised Function-Instance body contains a Call
/// terminator whose callee is `CallCallee::Indirect`.
fn has_indirect_call(model: &RegseqModel) -> bool {
    model.instances.iter().any(|instance| {
        let Some(function) = instance.kind.as_function() else {
            return false;
        };
        let Some(body) = function.body.as_ref() else {
            return false;
        };
        body.basic_blocks.iter().any(|block| {
            matches!(
                block.terminator.kind,
                TerminatorKind::Call {
                    callee: CallCallee::Indirect(_),
                    ..
                }
            )
        })
    })
}

/// True if any Function Instance carries the `FnPtrShim` flavor. The fix
/// must never materialise a FnPtrShim Instance: the fn-pointer
/// `Fn*::call` is lowered past it to an indirect call.
fn has_fnptrshim_instance(model: &RegseqModel) -> bool {
    model.instances.iter().any(|instance| {
        instance
            .kind
            .as_function()
            .is_some_and(|function| function.flavor == FunctionInstanceFlavor::FnPtrShim)
    })
}

/// Count `SyntheticItemKind::ClosureCall` items in the model.
fn closure_call_synth_count(model: &RegseqModel) -> usize {
    model
        .item_synthetic_kinds
        .iter()
        .filter(|kind| matches!(kind, SyntheticItemKind::ClosureCall { .. }))
        .count()
}

/// Assert the model lowered the fn-pointer `Fn*::call` to an indirect
/// call and never materialised a FnPtrShim Instance.
fn assert_indirect_no_fnptrshim(model: &RegseqModel, label: &str) {
    assert!(
        has_indirect_call(model),
        "{label}: expected at least one CallCallee::Indirect Call terminator; got none",
    );
    assert!(
        !has_fnptrshim_instance(model),
        "{label}: a FnPtrShim-flavored Instance was materialised; the fn-pointer Fn*::call must be lowered past it to an indirect call",
    );
}

/// B1 (Fn, zero-arg): the canonical coerced-fn-pointer-via-`Fn` repro.
#[test]
fn fn_zero_arg_fnptr_lowers_to_indirect() {
    let model = bridge_ok(
        "b1",
        "#![no_std]\nfn g() -> u32 { 7 }\nfn call_it<F: Fn() -> u32>(f: F) -> u32 { f() }\npub fn f() -> u32 { call_it(g as fn() -> u32) }\n",
    );
    assert_indirect_no_fnptrshim(&model, "b1");
}

/// B2 (FnMut, zero-arg): the `&mut F` receiver, deref'd.
#[test]
fn fnmut_zero_arg_fnptr_lowers_to_indirect() {
    let model = bridge_ok(
        "b2",
        "#![no_std]\nfn g() -> u32 { 7 }\nfn call_it<F: FnMut() -> u32>(mut f: F) -> u32 { f() }\npub fn f() -> u32 { call_it(g as fn() -> u32) }\n",
    );
    assert_indirect_no_fnptrshim(&model, "b2");
}

/// B3 (FnOnce, zero-arg): the by-value `copy _1` receiver.
#[test]
fn fnonce_zero_arg_fnptr_lowers_to_indirect() {
    let model = bridge_ok(
        "b3",
        "#![no_std]\nfn g() -> u32 { 7 }\nfn call_it<F: FnOnce() -> u32>(f: F) -> u32 { f() }\npub fn f() -> u32 { call_it(g as fn() -> u32) }\n",
    );
    assert_indirect_no_fnptrshim(&model, "b3");
}

/// B4 (Fn, non-ZST arg): the args tuple is a `Move` place, untupled to
/// one positional operand.
#[test]
fn fn_non_zst_arg_fnptr_lowers_to_indirect() {
    let model = bridge_ok(
        "b4",
        "#![no_std]\nfn g(x: u32) -> u32 { x + 1 }\nfn call_it<F: Fn(u32) -> u32>(f: F) -> u32 { f(41) }\npub fn f() -> u32 { call_it(g as fn(u32) -> u32) }\n",
    );
    assert_indirect_no_fnptrshim(&model, "b4");
}

/// B5 (control: direct fn-ptr call): already indirect at HEAD; must stay
/// `Ok` and indirect, with no FnPtrShim Instance.
#[test]
fn direct_fnptr_call_unchanged() {
    let model = bridge_ok(
        "b5",
        "#![no_std]\npub fn f(g: fn() -> u32) -> u32 { g() }\n",
    );
    assert!(
        has_indirect_call(&model),
        "b5: direct fn-pointer call must bridge to a CallCallee::Indirect",
    );
    assert!(
        !has_fnptrshim_instance(&model),
        "b5: direct fn-pointer call must not materialise a FnPtrShim Instance",
    );
}

/// B6 (control: closure via `Fn`): still routes through a `ClosureCall`
/// synth Instance (unchanged), NOT the fn-ptr indirect path.
#[test]
fn closure_via_fn_routes_through_closure_call() {
    let model = bridge_ok(
        "b6",
        "#![no_std]\nfn call_it<F: Fn() -> u32>(f: F) -> u32 { f() }\npub fn f() -> u32 { let x = 5u32; call_it(move || x) }\n",
    );
    assert!(
        closure_call_synth_count(&model) >= 1,
        "b6: closure-via-Fn must route through a SyntheticItemKind::ClosureCall instance",
    );
    assert!(
        !has_fnptrshim_instance(&model),
        "b6: closure-via-Fn must not materialise a FnPtrShim Instance",
    );
}

/// B7 (boundary): bare-FnDef-via-`Fn` (`call_it(g)` without `as fn()`)
/// is a SEPARATE `::Output`-over-FnDef gap (out of scope) and stays
/// rejected.
#[test]
fn bare_fndef_via_fn_still_rejected() {
    bridge_rejected(
        "b7",
        "#![no_std]\nfn g() -> u32 { 7 }\nfn call_it<F: Fn() -> u32>(f: F) -> u32 { f() }\npub fn f() -> u32 { call_it(g) }\n",
    );
}
