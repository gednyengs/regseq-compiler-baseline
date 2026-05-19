//! regseq-driver-55 Test Plan C2: Phase-5 Instance-construction
//! completeness end-to-end.
//!
//! Bridges `crates/test-ir-fixtures/fixtures/instance_completeness.rs`
//! through the full phase-1a `regseq-driver` pipeline and asserts:
//!
//!   1. `model.validate(...)` passes via the canonical
//!      `BridgeInput::matching(&model)` helper -- this exercises the
//!      new `validate_item_instance_coverage` check (Change G) over
//!      a model that now carries every required Instance.
//!   2. `model.instances` carries the symmetric Instance shape for
//!      every in-scope source kind plus the synth-`VTableType`
//!      path (Changes C/D/E).
//!   3. An `insta` snapshot pins `PrettyPrinter::new(&model).render()`
//!      so future regressions surface here.

#![feature(rustc_private)]

extern crate rustc_driver;

use regseq_model::{InstanceKind, ItemKind, Mutability, RegseqModel, SyntheticItemKind};
use test_ir_fixtures::{bridge_fixture, fixture_names, render_default};

fn emission<'m>(model: &'m RegseqModel, inst: &regseq_model::Instance) -> &'m str {
    model.strings.resolve(inst.emission_name)
}

fn named<'m>(model: &'m RegseqModel, suffix: &str) -> Vec<&'m regseq_model::Instance> {
    let want = format!("::{suffix}");
    model
        .instances
        .iter()
        .filter(|inst| emission(model, inst).ends_with(&want))
        .collect()
}

#[test]
fn bridged_instance_completeness_validates_renders_and_covers_every_kind() {
    let mut model = bridge_fixture(fixture_names::INSTANCE_COMPLETENESS);

    // 1. Spec-letter validate lock (mirrors e2e_enum_round_trip):
    // `bridge_fixture` already validates internally, but the
    // explicit call is the regseq-driver-55 Test Plan C2 contract.
    let bridge_input = regseq_model::BridgeInput::matching(&model);
    model.validate(&bridge_input).expect("validate must pass");

    // 2a. Point -> exactly one InstanceKind::Struct.
    let point_structs: Vec<_> = named(&model, "Point")
        .into_iter()
        .filter(|i| matches!(i.kind, InstanceKind::Struct { .. }))
        .collect();
    assert_eq!(
        point_structs.len(),
        1,
        "Point must produce exactly one Struct Instance"
    );

    // 2b. Wrap<u32> + Wrap<u64> -> exactly two InstanceKind::Struct
    // (one per monomorphisation per Test Plan A4 cardinality).
    let wrap_structs: Vec<_> = named(&model, "Wrap")
        .into_iter()
        .filter(|i| matches!(i.kind, InstanceKind::Struct { .. }))
        .collect();
    assert_eq!(
        wrap_structs.len(),
        2,
        "Wrap<_> must produce exactly two Struct Instances (u32 + u64)"
    );
    assert_ne!(
        wrap_structs[0].generic_args, wrap_structs[1].generic_args,
        "the two Wrap Instances must carry distinct generic_args"
    );

    // 2c. State -> exactly one InstanceKind::Enum.
    let state_enums: Vec<_> = named(&model, "State")
        .into_iter()
        .filter(|i| matches!(i.kind, InstanceKind::Enum { .. }))
        .collect();
    assert_eq!(state_enums.len(), 1, "State must produce one Enum Instance");

    // 2d. Pair -> exactly one InstanceKind::Union.
    let pair_unions: Vec<_> = named(&model, "Pair")
        .into_iter()
        .filter(|i| matches!(i.kind, InstanceKind::Union { .. }))
        .collect();
    assert_eq!(pair_unions.len(), 1, "Pair must produce one Union Instance");

    // 2e. COUNTER + FLAG -> InstanceKind::Static { init: Some(_) }
    // each; FLAG's parent Item carries Mutability::Mut.
    for (name, want_mut) in [("COUNTER", Mutability::Not), ("FLAG", Mutability::Mut)] {
        let statics: Vec<_> = named(&model, name)
            .into_iter()
            .filter(|i| matches!(i.kind, InstanceKind::Static { .. }))
            .collect();
        assert_eq!(statics.len(), 1, "{name} must produce one Static Instance");
        let InstanceKind::Static { init, .. } = statics[0].kind else {
            unreachable!()
        };
        assert!(
            init.is_some(),
            "{name} (source static) must carry init: Some(_)"
        );
        match (&model.items[statics[0].item].kind, want_mut) {
            (ItemKind::Static { mutability }, expected) => assert_eq!(
                *mutability, expected,
                "{name} parent Item mutability mismatch"
            ),
            (other, _) => panic!("{name} parent Item must be ItemKind::Static, got {other:?}"),
        }
    }

    // 2f. LIMIT -> InstanceKind::Const.
    let limit_consts: Vec<_> = named(&model, "LIMIT")
        .into_iter()
        .filter(|i| matches!(i.kind, InstanceKind::Const { .. }))
        .collect();
    assert_eq!(limit_consts.len(), 1, "LIMIT must produce one Const Instance");

    // 2g. Synth VTableType -> at least one InstanceKind::Struct
    // whose parent Item carries SyntheticItemKind::VTableType.
    let vtable_structs = model
        .instances
        .iter()
        .filter(|i| matches!(i.kind, InstanceKind::Struct { .. }))
        .filter(|i| {
            matches!(
                model.item_synthetic_kinds[i.item],
                SyntheticItemKind::VTableType { .. }
            )
        })
        .count();
    assert!(
        vtable_structs >= 1,
        "synth VTableType must carry an InstanceKind::Struct row"
    );

    // 3. Snapshot pin: PrettyPrinter render is deterministic across
    // builds (F4 + F4-2 sort applied at the end of run_bridge).
    insta::assert_snapshot!("render", render_default(&model));
}
