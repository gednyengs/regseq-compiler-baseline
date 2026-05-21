//! regseq-driver-62 Test Plan B2: synthesis pre-scan substitutes
//! `Instance.args` into the polymorphic body before walking call-site
//! terminators.
//!
//! Bridges a fixture whose `apply::<{closure}>` monomorphization
//! contains a `<F as Fn>::call(&f, (x, y))` Call terminator where the
//! receiver type `F` is `TyKind::Param(0)` in the polymorphic MIR.
//! Pre-fix, the Phase-2 synthesis pre-scan at
//! `crates/regseq-driver/src/bridge/mod.rs:769` fetched the
//! polymorphic body via `tcx.optimized_mir(local_def_id)` without
//! applying `instance.args`, so the call site reached
//! `canonicalize_call_site_callee` with `Self == Param(0)` and
//! `Instance::try_resolve` returned `Ok(None)`. The bridge converted
//! that into `BridgeError::Phase4(...)` at
//! `crates/regseq-driver/src/bridge/mod.rs:1042-1046`.
//!
//! Post-fix, the pre-scan substitutes `instance.args` via
//! `instance_mir(instance.def)` +
//! `instantiate_mir_and_normalize_erasing_regions` (mirroring the
//! regseq-driver-60 body materializer fix at
//! `crates/regseq-driver/src/bridge/body/materialise.rs:287-293`), so
//! the resolver sees the monomorphized closure type and resolves
//! cleanly.
//!
//! Assertions (per spec.md task plan B2):
//!   - `bridge_fixture("closure_apply_indirection")` succeeds.
//!   - A Function Instance keyed on the `apply` Item exists with
//!     `flavor == FunctionInstanceFlavor::Item` and `body.is_some()`.
//!   - The resulting model carries at least one
//!     `SyntheticItemKind::ClosureCall { call_kind: Call, .. }` Item
//!     (the closure's `Fn::call` shim, synthesised by the bridge per
//!     spec.md section 13.1).

#![feature(rustc_private)]

extern crate rustc_driver;

use regseq_model::{
    ClosureCallKind, FunctionInstanceFlavor, InstanceKind, ItemId, ItemKind, SyntheticItemKind,
};
use test_ir_fixtures::bridge_fixture;

const FIXTURE: &str = "closure_apply_indirection";

#[test]
fn closure_apply_indirection_pre_scan_substitutes_instance_args() {
    let model = bridge_fixture(FIXTURE);

    // 1. The `apply` Function Item must exist and carry a Function
    //    Instance whose flavor is `Item` and whose body materialized
    //    (the entire pipeline must reach Phase 5 without
    //    `BridgeError::Phase4`).
    let apply_item = model
        .items
        .iter()
        .enumerate()
        .find_map(|(idx, item)| {
            if !matches!(item.kind, ItemKind::Function) {
                return None;
            }
            let rendered = model.strings.resolve(item.def_path.rendered);
            if !rendered.ends_with("::apply") {
                return None;
            }
            let item_id = ItemId::from_u32(idx as u32);
            if !matches!(model.item_synthetic_kinds[item_id], SyntheticItemKind::None) {
                return None;
            }
            Some(item_id)
        })
        .expect("source Function Item `apply` must surface in model.items");

    let apply_instances: Vec<&regseq_model::Instance> = model
        .instances
        .iter()
        .filter(|inst| inst.item == apply_item && matches!(inst.kind, InstanceKind::Function(_)))
        .collect();
    assert!(
        !apply_instances.is_empty(),
        "at least one Function Instance must be created for `apply::<{{closure}}>`",
    );
    let apply_instance = apply_instances[0];

    let function_data = apply_instance
        .kind
        .as_function()
        .expect("`apply` Instance must be a Function");
    assert!(
        matches!(function_data.flavor, FunctionInstanceFlavor::Item),
        "Function Instance for `apply` must have flavor Item; got {:?}",
        function_data.flavor,
    );
    assert!(
        function_data.body.is_some(),
        "Function Instance for `apply` must have body == Some(_) post-materialization; pre-fix this would fail at Phase 4 with BridgeError::Phase4 before body materialization runs",
    );

    // 2. The bridge must synthesise at least one
    //    `SyntheticItemKind::ClosureCall { call_kind: Call, .. }` Item
    //    -- the closure's `Fn::call` shim. This proves the synthesis
    //    pre-scan succeeded for the polymorphic `apply` body and the
    //    closure-call synthesis path ran (eager `bridge/mod.rs:2278+`
    //    or downstream).
    let closure_call_count = model
        .item_synthetic_kinds
        .iter()
        .filter(|kind| {
            matches!(
                kind,
                SyntheticItemKind::ClosureCall {
                    call_kind: ClosureCallKind::Call,
                    ..
                }
            )
        })
        .count();
    assert!(
        closure_call_count >= 1,
        "fixture must produce at least one SyntheticItemKind::ClosureCall {{ call_kind: Call, .. }} Item; got {closure_call_count}",
    );
}
