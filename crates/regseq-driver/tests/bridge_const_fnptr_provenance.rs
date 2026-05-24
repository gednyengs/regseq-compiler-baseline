//! regseq-driver-73: accept `GlobalAlloc::Function` provenance in const
//! translation. These end-to-end bridge runs exercise both rejection
//! sites (`translate_scalar` -> `Const::Reloc`, `allocation_bytes` ->
//! `Byte::Reloc`) through the full five-phase pipeline plus
//! `RegseqModel::validate`, so they lock both the materialization
//! mechanism (Change A/B) and the `instance_ref_of` match (Change D).

#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use regseq_driver::bridge::BridgeError;
use regseq_model::{FunctionInstanceFlavor, InstanceKind, RegseqModel};
use test_ir_fixtures::{try_bridge_fixture_path, BridgeOutcome};

/// Write `source` to a uniquely named temp `.rs` file and return its
/// path. The file is left in the temp dir; the OS reclaims it.
fn write_snippet(label: &str, source: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time after unix epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("regseq-fnptr-prov-{label}-{nanos}.rs"));
    std::fs::write(&path, source).expect("write temp snippet");
    path
}

/// Bridge `source` and assert the bridge AND `validate()` both
/// succeed, returning the model for further inspection.
fn bridge_ok(label: &str, source: &str) -> RegseqModel {
    let path = write_snippet(label, source);
    let outcome = try_bridge_fixture_path(&path);
    let _ = std::fs::remove_file(&path);
    match outcome {
        BridgeOutcome::Ok(model) => model,
        other => panic!("expected BridgeOutcome::Ok for {label}, got {other:?}"),
    }
}

/// Count Function instances carrying `flavor`.
fn count_function_instances_with_flavor(
    model: &RegseqModel,
    flavor: FunctionInstanceFlavor,
) -> usize {
    model
        .instances
        .iter()
        .filter(|inst| matches!(&inst.kind, InstanceKind::Function(data) if data.flavor == flavor))
        .count()
}

/// Count Function instances whose owning item's rendered def-path ends
/// with `suffix` (e.g. `"::t"`).
fn count_function_instances_named(model: &RegseqModel, suffix: &str) -> usize {
    model
        .instances
        .iter()
        .filter(|inst| matches!(&inst.kind, InstanceKind::Function(_)))
        .filter(|inst| {
            let rendered = model.items[inst.item].def_path.rendered;
            model.strings.resolve(rendered).ends_with(suffix)
        })
        .count()
}

/// B1 (FF-004): a `#[track_caller]` fn reified into a `static fn()`
/// carries `Function { ReifyShim }` provenance; the bridge materializes
/// a `ReifyShim`-flavored Function instance and the model validates.
#[test]
fn track_caller_static_reify_yields_reifyshim_instance() {
    let model = bridge_ok(
        "ff004",
        "#![no_std]\n#[track_caller]\npub fn t() {}\npub static FP: fn() = t;\n",
    );
    assert_eq!(
        count_function_instances_with_flavor(&model, FunctionInstanceFlavor::ReifyShim),
        1,
        "exactly one ReifyShim-flavored Function instance expected"
    );
}

/// B2 (`Const::Reloc` scalar path, top-level static): a plain-fn
/// `static fn()` previously rejected with `UnsupportedConst` now
/// bridges to an `Item`-flavored Function instance and validates.
#[test]
fn plain_fn_static_yields_item_instance() {
    let model = bridge_ok(
        "b2",
        "#![no_std]\npub fn t() {}\npub static FP: fn() = t;\n",
    );
    assert!(
        count_function_instances_named(&model, "::t") >= 1,
        "the plain fn `t` must be materialized as a Function instance"
    );
}

/// B3 (`Const::Reloc` scalar path, top-level const): a fn-ptr `const`
/// read through a function bridges OK, locking the const-initializer
/// phase path (Risk R-2).
#[test]
fn fn_ptr_const_bridges_through_const_initializer() {
    let model = bridge_ok(
        "b3",
        "#![no_std]\npub fn t() {}\npub const FP: fn() = t;\npub fn u() -> fn() { FP }\n",
    );
    assert!(
        count_function_instances_named(&model, "::t") >= 1,
        "the fn `t` behind the const fn-ptr must be materialized"
    );
}

/// B4 (`Byte::Reloc` allocation path): a fn-ptr array `static`
/// routes through `allocation_bytes`; both targets materialize and the
/// model validates.
#[test]
fn fn_ptr_array_static_materializes_both_targets() {
    let model = bridge_ok(
        "b4",
        "#![no_std]\npub fn a() {}\npub fn b() {}\npub static T: [fn(); 2] = [a, b];\n",
    );
    assert!(
        count_function_instances_named(&model, "::a") >= 1,
        "fn `a` must be materialized"
    );
    assert!(
        count_function_instances_named(&model, "::b") >= 1,
        "fn `b` must be materialized"
    );
}

/// B5 (negative, no scope creep): a `&dyn Trait` embedded in an
/// aggregate static routes its vtable pointer through `allocation_bytes`
/// with `GlobalAlloc::VTable` provenance, which MUST remain rejected
/// with the existing `UnsupportedConst` wording.
#[test]
fn vtable_provenance_in_aggregate_static_still_rejected() {
    // A `const` (not a `static`, which would require `Sync`) tuple
    // embedding a `&dyn Tr` puts the trait object's vtable pointer in
    // the const allocation; that pointer carries `GlobalAlloc::VTable`
    // provenance and routes through `allocation_bytes`.
    let path = write_snippet(
        "b5",
        "#![no_std]\n\
         pub trait Tr { fn m(&self); }\n\
         pub struct S;\n\
         impl Tr for S { fn m(&self) {} }\n\
         pub const T: (&dyn Tr,) = (&S,);\n\
         pub fn use_t() -> &'static dyn Tr { T.0 }\n",
    );
    let outcome = try_bridge_fixture_path(&path);
    let _ = std::fs::remove_file(&path);
    match outcome {
        BridgeOutcome::Rejected(BridgeError::UnsupportedConst { const_repr }) => {
            assert!(
                const_repr.contains("provenance over"),
                "rejection must use the existing UnsupportedConst provenance wording; got {const_repr:?}"
            );
        }
        other => panic!("expected UnsupportedConst rejection for VTable provenance, got {other:?}"),
    }
}

/// B6 (dedup): a fn used BOTH behind a const fn-ptr AND called directly
/// must converge on a single Function instance (the const-materialized
/// instance reuses the call-site instance via the shared
/// `(item, args_key, flavor)` cache).
#[test]
fn const_fn_ptr_and_direct_call_converge_on_one_instance() {
    let model = bridge_ok(
        "b6",
        "#![no_std]\npub fn t() {}\npub static FP: fn() = t;\npub fn u() { t(); }\n",
    );
    assert_eq!(
        count_function_instances_named(&model, "::t"),
        1,
        "the const fn-ptr target and the direct call site must share one instance"
    );
}
