//! regseq-driver-55 Test Plan B: Item-vs-Instance
//! bridge-completeness check (Change G, spec v3).
//!
//! These tests invoke the new `validate_bridge_completeness`
//! entry point added by Change G, NOT the `validate(&BridgeInput)`
//! global model-correctness entry point. B1 additionally
//! cross-checks that `validate(&BridgeInput::matching(&model))`
//! returns `Ok(())` for the SAME partial model -- this pins the
//! deliberately-distinct entry-point contract: the
//! model-correctness path is permissive about Item-vs-Instance
//! coverage; the bridge-completeness path is strict. B4 pins the
//! v3 Source-`Function`-Item carve-out: Function-Instance coverage
//! is owned by `construct_function_instances` + lazy call-site
//! creation, not by `validate_bridge_completeness`. The
//! partial-model corpus at
//! `crates/regseq-model/tests/{id_referential_integrity_validator,
//! layout_validator}.rs` is intentionally left untouched and
//! continues to call `validate(&BridgeInput::matching(model))`.

use regseq_model::{
    BridgeInput, DefPath, DefPathData, DefPathHash, DefPathSegment, GenericArg, InlineHint,
    Instance, InstanceKind, InvariantViolation, Item, ItemId, ItemKind, Layout, LayoutKind,
    Linkage, RegseqModel, Span, StableCrateId, StringId, SyntheticItemKind, Ty,
};

fn hash(local_hash: u64) -> DefPathHash {
    DefPathHash {
        stable_crate_id: StableCrateId(1),
        local_hash,
    }
}

fn base_model() -> RegseqModel {
    let mut model = RegseqModel::with_primary("pl011_spec", StableCrateId(1));
    model.types.push(Ty::Bool);
    model
}

/// Push one source-derived Item of the given `kind` and `linkage`,
/// returning its `ItemId`.
fn push_source_item(
    model: &mut RegseqModel,
    name: &str,
    kind: ItemKind,
    linkage: Linkage,
) -> ItemId {
    let name_id = model.strings.intern(name);
    let crate_name = model
        .strings
        .resolve(model.crates[model.primary_crate].name)
        .to_owned();
    let rendered = model.strings.intern(&format!("{crate_name}::{name}"));
    let segment_data = match kind {
        ItemKind::Function => DefPathData::ValueNs(name_id),
        _ => DefPathData::TypeNs(name_id),
    };
    let item = Item {
        def_path: DefPath {
            crate_id: model.primary_crate,
            segments: vec![
                DefPathSegment {
                    data: DefPathData::CrateRoot,
                    disambiguator: 0,
                },
                DefPathSegment {
                    data: segment_data,
                    disambiguator: 0,
                },
            ],
            rendered,
        },
        kind,
        span: Span::dummy(),
        linkage,
        link_section: None,
        inline_hint: InlineHint::None,
        is_unsafe: false,
        is_phantom_data: false,
        is_unsafe_cell: false,
        is_manually_drop: false,
        opaque_handle_eligible: false,
    };
    let id = model.items.push(item);
    model.item_def_paths.push(hash(1));
    model.item_synthetic_kinds.push(SyntheticItemKind::None);
    model.domain_attrs.item_attrs.push(Vec::new());
    id
}

/// Convenience wrapper retained for the existing source-Struct
/// negative / positive cases (B1, B3, generic-args guard).
fn push_struct_item(model: &mut RegseqModel, name: &str, linkage: Linkage) -> ItemId {
    push_source_item(model, name, ItemKind::Struct, linkage)
}

fn empty_struct_layout() -> Layout {
    Layout {
        size: 0,
        align: 1,
        pack: None,
        uninhabited: false,
        kind: LayoutKind::Struct { fields: vec![] },
    }
}

// B1. Negative regression: a source-derived non-foreign
// `ItemKind::Struct` with ZERO matching Instance must be rejected
// by the STRICT bridge-completeness path; the PERMISSIVE
// model-correctness `validate` path must accept the same model.
#[test]
fn b1_struct_item_without_instance_is_rejected() {
    let mut model = base_model();
    let item = push_struct_item(&mut model, "Lonely", Linkage::Default);
    assert!(
        matches!(
            model.validate_bridge_completeness(),
            Err(InvariantViolation::ItemMissingInstance {
                item: m_item,
                item_kind: ItemKind::Struct,
            }) if m_item == item
        ),
        "expected ItemMissingInstance for the uncovered Struct Item, got {:?}",
        model.validate_bridge_completeness()
    );
    // Cross-check: the model-correctness `validate` entry point is
    // deliberately permissive about Item-vs-Instance coverage and
    // returns Ok(()) for the SAME partial model (Change G v2).
    let bridge_input = BridgeInput::matching(&model);
    assert_eq!(
        model.validate(&bridge_input),
        Ok(()),
        "the model-correctness validate path must stay permissive about coverage"
    );
}

// B2. Foreign carve-out (positive): the same shape but
// `linkage: Linkage::Foreign` must validate -- foreign source
// Items legitimately may have no Instance per the regseq-driver-55
// Non-Goals foreign-decl carve-out.
#[test]
fn b2_foreign_struct_item_without_instance_is_accepted() {
    let mut model = base_model();
    let _item = push_struct_item(&mut model, "Foreigner", Linkage::Foreign);
    assert_eq!(
        model.validate_bridge_completeness(),
        Ok(()),
        "foreign source Item without an Instance must be carved out"
    );
}

// B3. Source-derived positive path: a non-foreign
// `ItemKind::Struct` WITH a matching `InstanceKind::Struct`
// Instance validates. Without this case B1 alone could pass under
// a validator that rejects every non-foreign Item regardless of
// Instance presence.
#[test]
fn b3_struct_item_with_matching_instance_is_accepted() {
    let mut model = base_model();
    let item = push_struct_item(&mut model, "Covered", Linkage::Default);
    let struct_ty = model.types.intern(Ty::Struct {
        item,
        args: vec![],
        layout: empty_struct_layout(),
    });
    model.instances.push(Instance {
        item,
        generic_args: vec![],
        emission_name: StringId::ZERO,
        span: Span::dummy(),
        kind: InstanceKind::Struct { layout: struct_ty },
    });
    assert_eq!(
        model.validate_bridge_completeness(),
        Ok(()),
        "non-foreign Struct Item with a matching Struct Instance must pass bridge-completeness"
    );
}

// B4. Source `Function`-Item carve-out (positive): a non-foreign
// source `ItemKind::Function` with ZERO matching Instance must
// pass bridge-completeness. The v3 spec narrowing removes
// `Function` from Change G's kind set: Function-Instance
// coverage is owned by `construct_function_instances` + lazy
// call-site creation (`lookup_or_create_direct_instance`), not
// by this validator. The in-the-wild grounding is the pre-existing
// `crates/regseq-driver/tests/bridge_synth.rs:399` fixture, where a
// `pub fn closure_fixture() -> impl Fn(u32) -> u32` is rejected
// by `translate_fn_sig` as `BridgeError::NonRigidTy` and no body
// invokes it directly. Any future change that re-adds `Function`
// to the kind set must explicitly grandfather this case.
#[test]
fn b4_function_item_without_instance_is_accepted() {
    let mut model = base_model();
    let _item = push_source_item(
        &mut model,
        "closure_fixture",
        ItemKind::Function,
        Linkage::Default,
    );
    assert_eq!(
        model.validate_bridge_completeness(),
        Ok(()),
        "source Function Item without an Instance must be carved out (v3 kind-set narrowing)"
    );
}

// Guard: the carve-out is keyed on Foreign linkage AND source
// origin; a generic-arg-bearing covered Item still validates (the
// coverage check is presence-only, not args-aware).
#[test]
fn covered_item_with_generic_args_validates() {
    let mut model = base_model();
    let item = push_struct_item(&mut model, "Generic", Linkage::Default);
    let inner = model.types.intern(Ty::Bool);
    let struct_ty = model.types.intern(Ty::Struct {
        item,
        args: vec![GenericArg::Type(inner)],
        layout: empty_struct_layout(),
    });
    model.instances.push(Instance {
        item,
        generic_args: vec![GenericArg::Type(inner)],
        emission_name: StringId::ZERO,
        span: Span::dummy(),
        kind: InstanceKind::Struct { layout: struct_ty },
    });
    assert_eq!(model.validate_bridge_completeness(), Ok(()));
}
