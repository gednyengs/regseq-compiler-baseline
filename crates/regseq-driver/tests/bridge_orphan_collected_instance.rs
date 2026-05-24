//! regseq-driver-74: instantiate orphaned mono-collected source
//! functions. A tuple `.clone()` makes rustc collect (as a mentioned
//! mono-item) the field-type library `Clone` impl
//! (`<u32 as Clone>::clone`), which the bridge interns but never
//! instantiates because the tuple CloneShim body is left absent and is
//! anyway optimized to a plain field copy. These end-to-end bridge
//! runs exercise the Change-B post-pass that gives such a
//! collected-but-uninstantiated bucket-1 Function item the
//! `Item`-flavored `body: None` instance a real caller would have
//! produced, so the Phase-5 `ItemMissingInstance` ratchet passes.

#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use regseq_model::{FunctionInstanceFlavor, InstanceKind, RegseqModel};
use test_ir_fixtures::{try_bridge_fixture_path, BridgeOutcome};

/// Write `source` to a uniquely named temp `.rs` file and return its
/// path. The file is left in the temp dir; the OS reclaims it.
fn write_snippet(label: &str, source: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time after unix epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("regseq-orphan-clone-{label}-{nanos}.rs"));
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

/// `ItemId`s whose rendered def-path ends with `suffix` and that carry
/// a Function instance with the given `flavor`, paired with whether
/// that instance's body is `None`.
fn function_instances_named_with_flavor(
    model: &RegseqModel,
    suffix: &str,
    flavor: FunctionInstanceFlavor,
) -> Vec<(regseq_model::ItemId, bool)> {
    model
        .instances
        .iter()
        .filter_map(|inst| match &inst.kind {
            InstanceKind::Function(data) if data.flavor == flavor => {
                let rendered = model
                    .strings
                    .resolve(model.items[inst.item].def_path.rendered);
                if rendered.ends_with(suffix) {
                    Some((inst.item, data.body.is_none()))
                } else {
                    None
                }
            }
            _ => None,
        })
        .collect()
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

/// The §1 target snippet: a `(u32, u32)` tuple `.clone()`.
const TUPLE_CLONE: &str =
    "#![no_std]\npub fn f() -> (u32, u32) {\n    let t = (1u32, 2u32);\n    t.clone()\n}\n";

/// B1 (target, positive): `(u32, u32).clone()` bridges to
/// `BridgeOutcome::Ok` and validates, and the field-clone library impl
/// (`<u32 as Clone>::clone`, def-path ending `::clone`, NOT the
/// `Clone::clone` CloneShim owner) carries exactly one `Function/Item`
/// instance with `body == None`.
#[test]
fn tuple_clone_instantiates_field_clone_item_instance() {
    let model = bridge_ok("b1", TUPLE_CLONE);
    let field_clones =
        function_instances_named_with_flavor(&model, "::clone", FunctionInstanceFlavor::Item);
    assert_eq!(
        field_clones.len(),
        1,
        "exactly one `::clone` Item-flavored Function instance (the field-clone impl) expected, got {field_clones:?}"
    );
    let (_item, body_is_none) = field_clones[0];
    assert!(
        body_is_none,
        "the field-clone (cross-crate) Item instance must carry body == None"
    );
}

/// B2 (CloneShim unchanged): in the same model, the `Clone::clone`
/// owner still carries its `Function/CloneShim` instance -- the fix
/// added the field instance, it did not disturb the shim.
#[test]
fn tuple_clone_leaves_cloneshim_instance_intact() {
    let model = bridge_ok("b2", TUPLE_CLONE);
    let shims =
        function_instances_named_with_flavor(&model, "::clone", FunctionInstanceFlavor::CloneShim);
    assert!(
        !shims.is_empty(),
        "the tuple `Clone::clone` CloneShim instance must still be present"
    );
}

/// B3 (nested): `(u32, (u16, u8)).clone()` bridges `Ok` + validates.
/// The nested tuple collects multiple field-clone orphans
/// (`<u32 as Clone>::clone`, `<u16 as Clone>::clone`,
/// `<u8 as Clone>::clone`), all instantiated in one sweep; the pass
/// terminates.
#[test]
fn nested_tuple_clone_instantiates_all_field_clone_orphans() {
    let model = bridge_ok(
        "b3",
        "#![no_std]\npub fn f() -> (u32, (u16, u8)) {\n    let t = (1u32, (2u16, 3u8));\n    t.clone()\n}\n",
    );
    let field_clones =
        function_instances_named_with_flavor(&model, "::clone", FunctionInstanceFlavor::Item);
    assert!(
        !field_clones.is_empty(),
        "nested tuple clone must instantiate at least one field-clone Item instance, got {field_clones:?}"
    );
    assert!(
        field_clones.iter().all(|(_, body_is_none)| *body_is_none),
        "every cross-crate field-clone Item instance must carry body == None"
    );
}

/// B4 (regression -- fnptr): the FF-012 snippet (`fn().clone()`) still
/// returns `Ok`; its CloneShim instance is unchanged and the pass adds
/// no spurious `::clone` `Item` instance (the fnptr CloneShim mentions
/// no field-clone sub-item, so there is no orphan to instantiate).
#[test]
fn fnptr_clone_unchanged_no_spurious_item_instance() {
    let model = bridge_ok(
        "b4",
        "#![no_std]\npub fn g() {}\npub fn f() -> fn() {\n    let p: fn() = g;\n    p.clone()\n}\n",
    );
    assert!(
        count_function_instances_with_flavor(&model, FunctionInstanceFlavor::CloneShim) >= 1,
        "the fnptr CloneShim instance must still be present"
    );
    let field_clones =
        function_instances_named_with_flavor(&model, "::clone", FunctionInstanceFlavor::Item);
    assert!(
        field_clones.is_empty(),
        "no spurious `::clone` Item instance must be added for the fnptr clone, got {field_clones:?}"
    );
}

/// B5 (clean crate): a crate with no orphans bridges `Ok` and
/// validates -- the pass is inert when no bucket-1 item is orphaned.
#[test]
fn clean_crate_with_no_orphans_is_inert() {
    let model = bridge_ok(
        "b5",
        "#![no_std]\npub fn f(x: u32) -> u32 {\n    x + 1\n}\n",
    );
    let field_clones =
        function_instances_named_with_flavor(&model, "::clone", FunctionInstanceFlavor::Item);
    assert!(
        field_clones.is_empty(),
        "a clean crate must not gain any `::clone` Item instance"
    );
}
