//! regseq-driver-82 (RJ-013): a body that uses a value whose type is a
//! return-position `impl Trait` (RPIT) opaque from a callee must bridge
//! `Ok` -- the opaque is revealed to its concrete hidden type (a closure
//! or primitive) during per-Instance body materialisation, not rejected
//! as `NonRigidTy { kind: Alias }`.
//!
//! The reveal is body-context-scoped via a transient `BridgeCtxt` flag
//! (`reveal_opaque_types`) set by the `RevealOpaqueScope` RAII guard at
//! `materialise_function_body_inner` AND `construct_closure_call_instances`.
//! It MUST NOT fire during identity-signature translation, so the RJ-009
//! bucket-4 `signature_non_rigid` carve-out (B6) stays covered.
//!
//! Each bridge run is wrapped in `catch_unwind` so a regression that
//! ICEs surfaces as a test failure rather than aborting the binary.
//!
//! Note on "no Alias": the model `Ty` enum has no `Alias` variant -- the
//! `Alias` rejection is a rustc-side `BridgeError::NonRigidTy { kind:
//! Alias }`. A `BridgeOutcome::Ok` therefore *is* the proof that no
//! opaque survived unrevealed; the positive assertions below add that
//! the revealed hidden type (closure / primitive) is present in the
//! model.

#![feature(rustc_private)]

extern crate rustc_driver;

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use regseq_model::{ItemId, ItemKind, RegseqModel, SyntheticItemKind, Ty};
use test_ir_fixtures::{try_bridge_fixture_path, BridgeOutcome};

/// Write `source` to a uniquely named temp `.rs` file and return its
/// path. The OS reclaims the temp dir.
fn write_snippet(label: &str, source: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time after unix epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("regseq-rpit-opaque-{label}-{nanos}.rs"));
    std::fs::write(&path, source).expect("write temp snippet");
    path
}

/// Bridge `source` and assert the bridge AND `validate()` both succeed,
/// returning the model.
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

/// True iff at least one interned `Ty` is a `Ty::Closure` -- the shape an
/// `impl Fn*` RPIT opaque reveals to.
fn has_closure_ty(model: &RegseqModel) -> bool {
    model
        .types
        .iter()
        .any(|(_, ty)| matches!(ty, Ty::Closure(_, _)))
}

/// Count `SyntheticItemKind::ClosureCall` items in the model.
fn closure_call_synth_count(model: &RegseqModel) -> usize {
    model
        .item_synthetic_kinds
        .iter()
        .filter(|kind| matches!(kind, SyntheticItemKind::ClosureCall { .. }))
        .count()
}

/// Find the source `Function` Item whose rendered def-path ends in
/// `::<leaf>` (and is not a synthetic item).
fn find_function_item(model: &RegseqModel, leaf: &str) -> ItemId {
    let want = format!("::{leaf}");
    for (idx, item) in model.items.iter().enumerate() {
        let id = ItemId::from_u32(idx as u32);
        if !matches!(item.kind, ItemKind::Function) {
            continue;
        }
        if !matches!(model.item_synthetic_kinds[id], SyntheticItemKind::None) {
            continue;
        }
        let rendered = model.strings.resolve(item.def_path.rendered);
        if rendered.ends_with(&want) {
            return id;
        }
    }
    panic!(
        "no source Function Item ending in `::{leaf}`; items: {:?}",
        model
            .items
            .iter()
            .map(|it| model.strings.resolve(it.def_path.rendered))
            .collect::<Vec<_>>()
    );
}

/// Count `Function` Instances keyed on `item`.
fn function_instance_count(model: &RegseqModel, item: ItemId) -> usize {
    model
        .instances
        .iter()
        .filter(|inst| {
            inst.item == item && matches!(inst.kind, regseq_model::InstanceKind::Function(_))
        })
        .count()
}

const MK_FN: &str = "#![no_std]\npub fn mk() -> impl Fn() -> u32 { || 0 }\n";

/// B1 let-form reveal: a body that binds a value of an RPIT `impl Fn`
/// opaque bridges `Ok` (the opaque is revealed, not rejected) and a
/// `Ty::Closure` is present in the model.
#[test]
fn b1_let_form_rpit_opaque_reveals_to_closure() {
    let model = bridge_ok("b1", &format!("{MK_FN}pub fn f() {{ let _g = mk(); }}\n"));
    assert!(
        has_closure_ty(&model),
        "b1: expected a revealed Ty::Closure (the `impl Fn() -> u32` hidden type); found none",
    );
}

/// B2 call-form: the revealed closure is invoked, so a
/// `SyntheticItemKind::ClosureCall` Item is present.
#[test]
fn b2_call_form_rpit_opaque_invokes_revealed_closure() {
    let model = bridge_ok(
        "b2",
        &format!("{MK_FN}pub fn f() -> u32 {{ let g = mk(); g() }}\n"),
    );
    assert!(
        closure_call_synth_count(&model) >= 1,
        "b2: expected at least one SyntheticItemKind::ClosureCall (the revealed closure is invoked); got 0",
    );
}

/// B3 non-Fn opaque: an `impl Copy` RPIT reveals to a primitive (`u32`),
/// proving the fix is not Fn-specific.
#[test]
fn b3_non_fn_opaque_reveals() {
    bridge_ok(
        "b3",
        "#![no_std]\npub fn mk() -> impl Copy { 5u32 }\npub fn f() { let _g = mk(); }\n",
    );
}

/// B4 nested RPIT: `outer()` returns `inner()`'s opaque; used in a body
/// bridges `Ok`.
#[test]
fn b4_nested_rpit_reveals() {
    let model = bridge_ok(
        "b4",
        "#![no_std]\n\
pub fn inner() -> impl Fn() -> u32 { || 0 }\n\
pub fn outer() -> impl Fn() -> u32 { inner() }\n\
pub fn f() -> u32 { let g = outer(); g() }\n",
    );
    assert!(
        has_closure_ty(&model),
        "b4: nested RPIT should reveal to a Ty::Closure; found none",
    );
}

/// B4b closure-signature RPIT (Change B site 2): a closure whose
/// signature carries an RPIT opaque exercises
/// `construct_closure_call_instances`; without the guard there it would
/// still reject.
#[test]
fn b4b_closure_signature_rpit_reveals() {
    bridge_ok(
        "b4b",
        &format!("{MK_FN}pub fn f() -> u32 {{ let c = || mk(); let g = c(); g() }}\n"),
    );
}

/// B5a control: the defining-fn-only case (`mk` alone, no caller) stays
/// `Ok` -- the opaque bridges in its defining scope.
#[test]
fn b5a_defining_fn_only_stays_ok() {
    bridge_ok("b5a", MK_FN);
}

/// B5b control: a bare closure local (no RPIT) stays `Ok` -- the reveal
/// target path is unregressed.
#[test]
fn b5b_bare_closure_local_stays_ok() {
    let model = bridge_ok(
        "b5b",
        "#![no_std]\npub fn f() -> u32 { let g = || 0u32; g() }\n",
    );
    assert!(
        closure_call_synth_count(&model) >= 1,
        "b5b: bare closure call should route through a ClosureCall shim",
    );
}

/// B6 RJ-009 REGRESSION GUARD (load-bearing): a private, uncalled
/// `fn opaque_adder() -> impl Fn(i32) -> i32` still has its identity
/// signature reject `NonRigidTy`, so its Item has `signature_non_rigid
/// == true` and ZERO Instances. This proves the body-context reveal did
/// NOT leak into identity-signature translation.
#[test]
fn b6_rj009_signature_non_rigid_carveout_preserved() {
    let model = bridge_ok(
        "b6",
        "#![no_std]\n\
fn opaque_adder() -> impl Fn(i32) -> i32 { |x| x + 1 }\n\
pub fn entry() -> i32 { 0 }\n",
    );

    let item = find_function_item(&model, "opaque_adder");
    let flags = model.item_bridge_flags.get(&item).unwrap_or_else(|| {
        panic!("b6: opaque_adder Item must carry BridgeItemFlags (signature_non_rigid carve-out)")
    });
    assert!(
        flags.signature_non_rigid,
        "b6: opaque_adder must have signature_non_rigid == true (RJ-009 carve-out); got {flags:?}",
    );
    assert_eq!(
        function_instance_count(&model, item),
        0,
        "b6: opaque_adder must have zero Function Instances (the carve-out pushes none); the reveal must NOT have leaked into identity-signature translation",
    );
}
