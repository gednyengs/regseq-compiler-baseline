//! regseq-driver-56 Test Plan B5: end-to-end static-dispatch
//! trait-method callee canonicalization via `test-ir-fixtures`.
//!
//! Bridges `crates/test-ir-fixtures/fixtures/trait_method_resolution.rs`
//! through the full phase-1a `regseq-driver` pipeline and asserts:
//!
//!   1. `model.validate(...)` and `model.validate_bridge_completeness()`
//!      both pass.
//!   2. Exactly TWO Function Instances (one per source-derived fn:
//!      the impl method `<S as T>::m` and the caller). Zero Function
//!      Instances with `body == None` for source-defined fns.
//!   3. No surviving Instance is keyed on a trait DECLARATION Item:
//!      every Function Instance whose Item's def_path terminal segment
//!      is the trait-method name `m` lives under an `Impl` def-path
//!      segment.
//!   4. An `insta` snapshot pins
//!      `PrettyPrinter::new(&model).render()` so the rendered call
//!      site (`{impl#0}::m(...)`) is locked against regressions.

#![feature(rustc_private)]

extern crate rustc_driver;

use regseq_model::{DefPathData, InstanceKind};
use test_ir_fixtures::{bridge_fixture, fixture_names, render_default};

#[test]
fn bridged_trait_method_resolution_validates_renders_and_keys_on_impl() {
    let mut model = bridge_fixture(fixture_names::TRAIT_METHOD_RESOLUTION);

    let bridge_input = regseq_model::BridgeInput::matching(&model);
    model.validate(&bridge_input).expect("validate must pass");
    model
        .validate_bridge_completeness()
        .expect("validate_bridge_completeness must pass");

    // Exactly TWO Function Instances (impl method + caller); zero
    // source-defined Function Instances with `body == None`.
    let function_instances: Vec<&regseq_model::Instance> = model
        .instances
        .iter()
        .filter(|inst| matches!(inst.kind, InstanceKind::Function(_)))
        .collect();
    assert_eq!(
        function_instances.len(),
        2,
        "expected exactly 2 Function Instances, got {}",
        function_instances.len(),
    );
    for inst in &function_instances {
        let Some(function) = inst.kind.as_function() else {
            unreachable!()
        };
        assert!(
            function.body.is_some(),
            "source-defined Function Instance {} has body == None",
            model.strings.resolve(inst.emission_name),
        );
    }

    // No surviving Function Instance is keyed on a trait DECLARATION
    // Item: any Item whose terminal segment is the trait method name
    // `m` must have a `DefPathData::Impl` ancestor (the impl-method
    // canonical form). The rejection predicate captures the
    // regseq-driver-56 spec section 8.1 contract structurally without
    // requiring a `TyCtxt`-side lookup.
    for inst in &function_instances {
        if !matches!(inst.kind, InstanceKind::Function(_)) {
            continue;
        }
        let item = &model.items[inst.item];
        let Some(last) = item.def_path.segments.last() else {
            continue;
        };
        let leaf = match last.data {
            DefPathData::ValueNs(id) | DefPathData::TypeNs(id) => model.strings.resolve(id),
            _ => continue,
        };
        if leaf != "m" {
            continue;
        }
        let has_impl_ancestor = item
            .def_path
            .segments
            .iter()
            .any(|s| matches!(s.data, DefPathData::Impl));
        assert!(
            has_impl_ancestor,
            "Function Instance {:?} is keyed on a trait DECLARATION Item (terminal `m` without `{{impl#N}}` ancestor)",
            model.strings.resolve(item.def_path.rendered),
        );
    }

    insta::assert_snapshot!("render", render_default(&model));
}
