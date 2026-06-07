//! regseq-driver-59 Test Plan B9: default trait method body coverage.
//!
//! Bridges `crates/test-ir-fixtures/fixtures/trait_default_method.rs`
//! through the full phase-1a `regseq-driver` pipeline and asserts that
//! the trait's default method body surfaces as an
//! `ItemKind::Function` Item at the trait's DefPath (NOT at any
//! `{impl#N}` block), per the regseq-driver-58 Change H spec
//! amendment at `docs/discussions/regseq-model/spec.md:2351-2358`.
//!
//! Regression target: a future change to the eager HIR walk at
//! `crates/regseq-driver/src/bridge/mod.rs` that filtered out trait
//! associated fns, or swapped the `tcx.is_mir_available` predicate
//! for one that excludes default trait bodies, would silently break
//! the Change H contract. The structural assertions plus snapshot
//! pin the rendered default-body form.

#![feature(rustc_private)]

extern crate rustc_driver;

use regseq_model::{DefPathData, InstanceKind, ItemId, ItemKind};
use test_ir_fixtures::{bridge_fixture, fixture_names, render_default};

#[test]
fn bridged_trait_default_method_body_surfaces_at_trait_def_path() {
    let mut model = bridge_fixture(fixture_names::TRAIT_DEFAULT_METHOD);

    let bridge_input = regseq_model::BridgeInput::matching(&model);
    model.validate(&bridge_input).expect("validate must pass");
    model
        .validate_bridge_completeness()
        .expect("validate_bridge_completeness must pass");

    // Part 1: structural assertions on the model only -- no TyCtxt
    // escape, no rustc-callback-local helper required.

    // (a) Enumerate every Function Item whose def_path's terminal
    // segment is `DefPathData::ValueNs("greet")`.
    let greet_items: Vec<ItemId> = model
        .items
        .iter()
        .enumerate()
        .filter_map(|(idx, item)| {
            if !matches!(item.kind, ItemKind::Function) {
                return None;
            }
            let last = item.def_path.segments.last()?;
            match last.data {
                DefPathData::ValueNs(name_id) if model.strings.resolve(name_id) == "greet" => {
                    Some(ItemId::from_u32(idx as u32))
                }
                _ => None,
            }
        })
        .collect();

    // (b) Exactly one such Function Item exists (regression guard
    // against duplicate enumeration via both the trait-default-body
    // path AND any impl-method path).
    assert_eq!(
        greet_items.len(),
        1,
        "expected exactly one Function Item named `greet`; got {} (def_paths: {:?})",
        greet_items.len(),
        greet_items
            .iter()
            .map(|id| model.strings.resolve(model.items[*id].def_path.rendered))
            .collect::<Vec<_>>(),
    );
    let greet_item = greet_items[0];

    // (c) The Item is anchored at the trait container, not at any
    // `{impl#N}` block: no `DefPathData::Impl` segment exists along
    // the def_path. This is the load-bearing Change H assertion.
    let has_impl_ancestor = model.items[greet_item]
        .def_path
        .segments
        .iter()
        .any(|s| matches!(s.data, DefPathData::Impl));
    assert!(
        !has_impl_ancestor,
        "Function Item `greet` must NOT have a `DefPathData::Impl` ancestor (Change H: default body lives at the trait's DefPath); def_path = {}",
        model
            .strings
            .resolve(model.items[greet_item].def_path.rendered),
    );

    // (d) Bucket-3 classification: a trait-declaration default method is generic
    // over `Self` (a real monomorphization axis), so `is_generic == true`
    // (bucket 3, 0+ instances) — NOT bucket 1. Its signature is still rigid.
    // See `ty::source_bridge_item_flags` (this supersedes the old `own_params`
    // carve-out that forced trait defaults to bucket 1).
    let flags = model
        .item_bridge_flags
        .get(&greet_item)
        .copied()
        .unwrap_or_default();
    assert!(
        flags.is_generic,
        "Function Item `greet` (a trait default method) must have `is_generic == true` (bucket 3, generic over `Self`); got {flags:?}",
    );
    assert!(
        !flags.signature_non_rigid,
        "Function Item `greet` must have `signature_non_rigid == false`; got {flags:?}",
    );

    // (e) Exactly one Function Instance is keyed on `greet_item`, and
    // its body is materialized (Some, not None). Per Change H the
    // default body lives on the trait's DefPath; absence of an
    // Instance or absence of a body would mean the eager HIR walk
    // failed to surface the default body.
    let greet_instances: Vec<&regseq_model::Instance> = model
        .instances
        .iter()
        .filter(|inst| inst.item == greet_item && matches!(inst.kind, InstanceKind::Function(_)))
        .collect();
    assert_eq!(
        greet_instances.len(),
        1,
        "expected exactly one Function Instance for trait default body `greet`; got {}",
        greet_instances.len(),
    );
    let greet_function = greet_instances[0]
        .kind
        .as_function()
        .expect("greet Instance must be a Function");
    assert!(
        greet_function.body.is_some(),
        "trait default body `greet` Instance must have `body == Some(_)` (default body materialized)",
    );

    // (f) Negative: no Function Item exists with BOTH a
    // `DefPathData::Impl` ancestor AND terminal `ValueNs("greet")`.
    // Such an Item would mean the default body got mis-enumerated as
    // an impl method, violating Change H.
    let impl_anchored_greet_items: Vec<ItemId> = model
        .items
        .iter()
        .enumerate()
        .filter_map(|(idx, item)| {
            if !matches!(item.kind, ItemKind::Function) {
                return None;
            }
            let last = item.def_path.segments.last()?;
            let is_greet = matches!(
                last.data,
                DefPathData::ValueNs(name_id)
                    if model.strings.resolve(name_id) == "greet",
            );
            let has_impl = item
                .def_path
                .segments
                .iter()
                .any(|s| matches!(s.data, DefPathData::Impl));
            if is_greet && has_impl {
                Some(ItemId::from_u32(idx as u32))
            } else {
                None
            }
        })
        .collect();
    assert!(
        impl_anchored_greet_items.is_empty(),
        "no Function Item named `greet` may be anchored to an `{{impl#N}}` block (Change H: default body lives on the trait); got: {:?}",
        impl_anchored_greet_items
            .iter()
            .map(|id| model.strings.resolve(model.items[*id].def_path.rendered))
            .collect::<Vec<_>>(),
    );

    // (g) Negative: no Item exists whose def_path terminal segment is
    // `DefPathData::TypeNs("Greeter")`. The trait container itself
    // must remain erased per `spec.md:2354-2355` ("`ItemKind` has six
    // variants -- `Function`, `Static`, `Const`, `Struct`, `Enum`,
    // `Union`. No `Trait` variant.").
    let leaking_trait_items: Vec<&str> = model
        .items
        .iter()
        .filter_map(|item| {
            let last = item.def_path.segments.last()?;
            match last.data {
                DefPathData::TypeNs(name_id) if model.strings.resolve(name_id) == "Greeter" => {
                    Some(model.strings.resolve(item.def_path.rendered))
                }
                _ => None,
            }
        })
        .collect();
    assert!(
        leaking_trait_items.is_empty(),
        "trait container `Greeter` must NOT leak into `model.items` (six-variant ItemKind closed alphabet); got: {leaking_trait_items:?}",
    );

    // Part 2: snapshot assertion. Pins the rendered default-body
    // form so a regression that changed the rendering shape (or
    // dropped the body) shows up as a snapshot diff.
    let rendered = render_default(&model);
    assert!(
        rendered.contains("greet"),
        "rendered output must mention `greet`; got:\n{rendered}",
    );
    assert!(
        rendered.contains("entry"),
        "rendered output must mention `entry`; got:\n{rendered}",
    );
    assert!(
        rendered.contains("42_u32"),
        "rendered output must contain the default body literal `42_u32`; got:\n{rendered}",
    );
    // The default body must render as a full function definition,
    // not a `;`-terminated body-less declaration shaped like the
    // foreign-fn form. We use the rendered `greet` chunk to verify
    // the absence of the body-less `;` form.
    assert_default_body_renders_with_block(&rendered);

    insta::assert_snapshot!("render", rendered);
}

/// Verify that the `greet` function definition renders with a block
/// body (`{ ... }`) rather than a body-less declaration (`;`-
/// terminated like the foreign-fn form). Walks the line stream to
/// find the `fn ...::greet(...) -> u32` definition line and asserts
/// it terminates with `{` (or carries an inline block) rather than
/// `;`.
fn assert_default_body_renders_with_block(rendered: &str) {
    let signature_line = rendered
        .lines()
        .find(|line| line.contains("fn ") && line.contains("greet"))
        .unwrap_or_else(|| panic!("no `fn ...greet(...)` line in render:\n{rendered}"));
    assert!(
        signature_line.contains('{'),
        "trait default body for `greet` must render with a block body, not a `;`-terminated declaration; got line: {signature_line:?}",
    );
    assert!(
        !signature_line.trim_end().ends_with(';'),
        "trait default body for `greet` must NOT render as a body-less foreign-style declaration; got line: {signature_line:?}",
    );
}
