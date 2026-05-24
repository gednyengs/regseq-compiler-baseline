//! regseq-driver-75: closure calls whose monomorphised `Fn*::call`
//! args-tuple arrives as an `Operand::Constant` must bridge through the
//! `ClosureCall` synth shim instead of falling through to
//! `translate_call_callee` -> `tcx.fn_sig(closure)`, which ICEs
//! (`to get the signature of a closure, use args.as_closure().sig() not
//! fn_sig()`). The optimized MIR `tcx.instance_mir` returns folds a
//! zero-arg args-tuple to `const ()` and an all-ZST args-tuple to a
//! `const (...)`; both previously hit the rewrite's catch-all bail.
//!
//! Each case wraps the in-process bridge run in `catch_unwind` so a
//! regression ICE surfaces as a test failure rather than aborting the
//! whole test binary.

#![feature(rustc_private)]

extern crate rustc_driver;

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use regseq_model::{RegseqModel, SyntheticItemKind};
use test_ir_fixtures::{try_bridge_fixture_path, BridgeOutcome};

/// Write `source` to a uniquely named temp `.rs` file and return its
/// path. The file is left in the temp dir; the OS reclaims it.
fn write_snippet(label: &str, source: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time after unix epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("regseq-zero-arg-closure-{label}-{nanos}.rs"));
    std::fs::write(&path, source).expect("write temp snippet");
    path
}

/// Bridge `source` and assert the bridge AND `validate()` both succeed,
/// returning the model. The bridge call is wrapped in `catch_unwind`:
/// at HEAD these snippets trigger a rustc ICE, so a regression must be
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

/// Count `SyntheticItemKind::ClosureCall` items in the model.
fn closure_call_synth_count(model: &RegseqModel) -> usize {
    model
        .item_synthetic_kinds
        .iter()
        .filter(|kind| matches!(kind, SyntheticItemKind::ClosureCall { .. }))
        .count()
}

/// Assert the model routed the call through a `ClosureCall` synth
/// instance (the spec-13.1 closure-call shim), not a direct
/// closure-`Item` call.
fn assert_routed_through_closure_call(model: &RegseqModel, label: &str) {
    assert!(
        closure_call_synth_count(model) >= 1,
        "{label}: expected at least one SyntheticItemKind::ClosureCall item (closure-call shim); got 0",
    );
}

/// B1 (zero-arg, capturing): the minimal `const ()` args-tuple repro.
#[test]
fn zero_arg_capturing_closure_call_bridges_through_shim() {
    let model = bridge_ok(
        "b1",
        "#![no_std]\npub fn f() -> u32 {\n    let x = 5u32;\n    let c = move || x;\n    c()\n}\n",
    );
    assert_routed_through_closure_call(&model, "b1");
}

/// B2 (zero-arg, non-capturing): empty captures, still a `const ()`
/// args-tuple.
#[test]
fn zero_arg_non_capturing_closure_call_bridges_through_shim() {
    let model = bridge_ok(
        "b2",
        "#![no_std]\npub fn f() -> u32 {\n    let c = || 7u32;\n    c()\n}\n",
    );
    assert_routed_through_closure_call(&model, "b2");
}

/// B3 (FnMut zero-arg): the `CallMut` receiver shape / call kind.
#[test]
fn zero_arg_fnmut_closure_call_bridges_through_shim() {
    let model = bridge_ok(
        "b3",
        "#![no_std]\npub fn f() -> u32 {\n    let mut n = 0u32;\n    let mut c = move || { n += 1; n };\n    c()\n}\n",
    );
    assert_routed_through_closure_call(&model, "b3");
}

/// B4 (all-ZST arg): a non-empty constant args-tuple (`c(())`), the
/// materialise-into-synthetic-local-and-project sub-case. MUST bridge
/// `Ok`, not reject.
#[test]
fn all_zst_arg_closure_call_materialises_and_bridges() {
    let model = bridge_ok(
        "b4",
        "#![no_std]\npub fn f() -> u32 {\n    let c = |_u: ()| 1u32;\n    c(())\n}\n",
    );
    assert_routed_through_closure_call(&model, "b4");
}

/// B5 (used cloned closure -- the original B1-audit repro): the clone
/// path was already fine post-R2; only the zero-arg call ICEd.
#[test]
fn used_cloned_closure_call_bridges_through_shim() {
    let model = bridge_ok(
        "b5",
        "#![no_std]\npub fn f() -> u32 {\n    let x = 5u32;\n    let c = move || x;\n    let d = c.clone();\n    d()\n}\n",
    );
    assert_routed_through_closure_call(&model, "b5");
}

/// B6 (regression -- non-ZST arg): the args-tuple is a `Move`/`Copy`
/// place, unchanged from HEAD; must still bridge `Ok` through the
/// existing place-projection arm.
#[test]
fn non_zst_arg_closure_call_unchanged() {
    let model = bridge_ok(
        "b6",
        "#![no_std]\npub fn f() -> u32 {\n    let c = |n: u32| n + 1;\n    c(41)\n}\n",
    );
    assert_routed_through_closure_call(&model, "b6");
}
