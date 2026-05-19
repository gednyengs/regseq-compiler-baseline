//! regseq-driver-55 Test Plan B: Item-vs-Instance coverage
//! validator (Change G).
//!
//! Invokes the validator via the canonical API shape
//! `RegseqModel::validate(&mut self, bridge_input: &BridgeInput)`
//! with the empty-bridge helper `BridgeInput::matching(&model)`,
//! mirroring the established hand-built-model pattern in
//! `crates/regseq-model/tests/id_referential_integrity_validator.rs`.

use regseq_model::{
    BridgeInput, DefPath, DefPathData, DefPathHash, DefPathSegment, GenericArg, InlineHint,
    Instance, InstanceKind, InvariantViolation, Item, ItemId, ItemKind, Layout, LayoutKind,
    Linkage, RegseqModel, Span, StableCrateId, StringId, SyntheticItemKind, Ty,
};

fn validate(model: &mut RegseqModel) -> Result<(), InvariantViolation> {
    model.validate(&BridgeInput::matching(model))
}

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

/// Push one source-derived `ItemKind::Struct` Item with the given
/// linkage and return its `ItemId`.
fn push_struct_item(model: &mut RegseqModel, name: &str, linkage: Linkage) -> ItemId {
    let name_id = model.strings.intern(name);
    let crate_name = model
        .strings
        .resolve(model.crates[model.primary_crate].name)
        .to_owned();
    let rendered = model.strings.intern(&format!("{crate_name}::{name}"));
    let item = Item {
        def_path: DefPath {
            crate_id: model.primary_crate,
            segments: vec![
                DefPathSegment {
                    data: DefPathData::CrateRoot,
                    disambiguator: 0,
                },
                DefPathSegment {
                    data: DefPathData::TypeNs(name_id),
                    disambiguator: 0,
                },
            ],
            rendered,
        },
        kind: ItemKind::Struct,
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
// with the missing-coverage error naming the uncovered Item.
#[test]
fn b1_struct_item_without_instance_is_rejected() {
    let mut model = base_model();
    let item = push_struct_item(&mut model, "Lonely", Linkage::Default);
    assert!(
        matches!(
            validate(&mut model),
            Err(InvariantViolation::ItemMissingInstance {
                item: m_item,
                item_kind: ItemKind::Struct,
            }) if m_item == item
        ),
        "expected ItemMissingInstance for the uncovered Struct Item, got {:?}",
        validate(&mut model)
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
        validate(&mut model),
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
        validate(&mut model),
        Ok(()),
        "non-foreign Struct Item with a matching Struct Instance must validate"
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
    assert_eq!(validate(&mut model), Ok(()));
}
