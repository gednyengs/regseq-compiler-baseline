//! regseq-driver-58 Test Plan B6: bucket-keyed
//! `validate_bridge_completeness` enforcement.
//!
//! Exercises the four-bucket policy implemented by Change G:
//!   - Bucket 1 (non-generic + signature-rigid + non-foreign):
//!     exactly 1 Instance required.
//!   - Bucket 2 (foreign): exactly 1 Instance with `body: None`
//!     (Function) / `init: None` (Static).
//!   - Bucket 3 (generic): 0+ Instances permitted.
//!   - Bucket 4 (Function + `signature_non_rigid`): exactly 0
//!     Instances permitted.
//!
//! Companion file to `validate_item_instance_coverage.rs` (the
//! regseq-driver-55 source-fn-carve-out corpus, retained for the
//! historical v3 carve-out). This file pins the v4 four-bucket
//! contract on the validator side; the bridge-side coverage lives in
//! the regseq-driver integration tests
//! (`crates/regseq-driver/tests/bridge_eager_hir_enumeration.rs`
//! etc.).

use regseq_model::{
    BridgeItemFlags, Const, DefPath, DefPathData, DefPathHash, DefPathSegment, FieldLayout, FnAbi,
    FnSig, FunctionInstanceData, FunctionInstanceFlavor, GenericArg, InlineHint, Instance,
    InstanceKind, InvariantViolation, Item, ItemId, ItemKind, Layout, LayoutKind, Linkage,
    Mutability, RegseqModel, Safety, Span, StableCrateId, StringId, SyntheticItemKind, Ty, TyId,
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

fn empty_struct_layout() -> Layout {
    Layout {
        size: 0,
        align: 1,
        pack: None,
        uninhabited: false,
        kind: LayoutKind::Struct { fields: vec![] },
    }
}

fn unit_fn_sig(model: &mut RegseqModel) -> regseq_model::FnSigId {
    let unit = model.types.intern(Ty::Tuple {
        fields: vec![],
        layout: empty_struct_layout(),
    });
    model.sigs.intern(FnSig {
        inputs: vec![],
        output: unit,
        abi: FnAbi::Rust,
        c_variadic: false,
        safety: Safety::Safe,
    })
}

/// Push a source-derived Item without filling `item_bridge_flags`,
/// returning its `ItemId`. The caller is expected to populate
/// `model.item_bridge_flags` directly if a non-default classification
/// is needed.
fn push_item(
    model: &mut RegseqModel,
    name: &str,
    kind: ItemKind,
    linkage: Linkage,
    local_hash: u64,
) -> ItemId {
    let name_id = model.strings.intern(name);
    let crate_name = model
        .strings
        .resolve(model.crates[model.primary_crate].name)
        .to_owned();
    let rendered = model.strings.intern(&format!("{crate_name}::{name}"));
    let segment_data = match kind {
        ItemKind::Function | ItemKind::Static { .. } | ItemKind::Const => {
            DefPathData::ValueNs(name_id)
        }
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
        is_result: false,
    };
    let id = model.items.push(item);
    model.item_def_paths.push(hash(local_hash));
    model.item_synthetic_kinds.push(SyntheticItemKind::None);
    model.domain_attrs.item_attrs.push(Vec::new());
    id
}

fn struct_instance(model: &mut RegseqModel, item: ItemId) -> Instance {
    let layout = model.types.intern(Ty::Struct {
        item,
        args: vec![],
        layout: empty_struct_layout(),
    });
    let emission_name = model.items[item].def_path.rendered;
    Instance {
        item,
        generic_args: vec![],
        emission_name,
        span: Span::dummy(),
        kind: InstanceKind::Struct { layout },
    }
}

fn function_instance(
    fn_sig: regseq_model::FnSigId,
    item: ItemId,
    body: Option<regseq_model::Body>,
    emission_name: StringId,
) -> Instance {
    Instance {
        item,
        generic_args: vec![],
        emission_name,
        span: Span::dummy(),
        kind: InstanceKind::Function(FunctionInstanceData {
            fn_sig,
            body,
            flavor: FunctionInstanceFlavor::Item,
            virtual_idx: None,
        }),
    }
}

fn static_instance(item: ItemId, ty: TyId, init: Option<regseq_model::ConstId>) -> Instance {
    Instance {
        item,
        generic_args: vec![],
        emission_name: StringId::ZERO,
        span: Span::dummy(),
        kind: InstanceKind::Static { ty, init },
    }
}

// ------------------------------------------------------------------
// Bucket 1: bucket-1 Struct Item with ZERO Instances must fail.
// ------------------------------------------------------------------
#[test]
fn bucket1_struct_zero_instances_rejected() {
    let mut model = base_model();
    let item = push_item(&mut model, "Lonely", ItemKind::Struct, Linkage::Default, 1);
    assert!(matches!(
        model.validate_bridge_completeness(),
        Err(InvariantViolation::ItemMissingInstance {
            item: m_item,
            item_kind: ItemKind::Struct,
        }) if m_item == item
    ));
}

// ------------------------------------------------------------------
// Bucket 1: foreign Function Item with no Instance must fail.
// ------------------------------------------------------------------
#[test]
fn bucket2_foreign_function_zero_instances_rejected() {
    let mut model = base_model();
    let item = push_item(
        &mut model,
        "extern_helper",
        ItemKind::Function,
        Linkage::Foreign,
        1,
    );
    assert!(matches!(
        model.validate_bridge_completeness(),
        Err(InvariantViolation::ItemMissingInstance {
            item: m_item,
            item_kind: ItemKind::Function,
        }) if m_item == item
    ));
}

// ------------------------------------------------------------------
// Bucket 1: bucket-1 Struct Item with TWO matching Instances must
// fail. Spec.md regseq-driver-58 Acceptance Criterion 2: the bucket-1
// cardinality is EXACTLY 1.
// ------------------------------------------------------------------
#[test]
fn bucket1_struct_multiple_instances_rejected() {
    let mut model = base_model();
    let item = push_item(&mut model, "Twin", ItemKind::Struct, Linkage::Default, 1);
    let inst_a = struct_instance(&mut model, item);
    let inst_b = struct_instance(&mut model, item);
    model.instances.push(inst_a);
    model.instances.push(inst_b);
    assert!(matches!(
        model.validate_bridge_completeness(),
        Err(InvariantViolation::Bucket1MultipleInstances {
            item: m_item,
            item_kind: ItemKind::Struct,
            count: 2,
        }) if m_item == item
    ));
}

/// Build a Function instance carrying an explicit `flavor`, so a
/// single source item can own its primary `Item` instance alongside an
/// auxiliary shim instance.
fn function_instance_with_flavor(
    fn_sig: regseq_model::FnSigId,
    item: ItemId,
    body: Option<regseq_model::Body>,
    emission_name: StringId,
    flavor: FunctionInstanceFlavor,
) -> Instance {
    Instance {
        item,
        generic_args: vec![],
        emission_name,
        span: Span::dummy(),
        kind: InstanceKind::Function(FunctionInstanceData {
            fn_sig,
            body,
            flavor,
            virtual_idx: None,
        }),
    }
}

// ------------------------------------------------------------------
// Bucket 1 (regseq-driver-73): a source `Function` item may own its
// one primary `Item` instance PLUS an auxiliary shim-flavored Function
// instance that shares the source `DefId` (e.g. a `#[track_caller]` fn
// reified into a fn pointer surfaces a `ReifyShim` instance on the same
// item). The cardinality bound applies only to the primary instance.
// ------------------------------------------------------------------
#[test]
fn bucket1_function_item_with_primary_plus_shim_instance_accepted() {
    let mut model = base_model();
    let item = push_item(&mut model, "t", ItemKind::Function, Linkage::Default, 1);
    let sig = unit_fn_sig(&mut model);
    let emission = model.items[item].def_path.rendered;
    model.instances.push(function_instance_with_flavor(
        sig,
        item,
        Some(regseq_model::Body::default()),
        emission,
        FunctionInstanceFlavor::Item,
    ));
    model.instances.push(function_instance_with_flavor(
        sig,
        item,
        None,
        emission,
        FunctionInstanceFlavor::ReifyShim,
    ));
    assert_eq!(model.validate_bridge_completeness(), Ok(()));
}

// ------------------------------------------------------------------
// Bucket 1 (regseq-driver-73): two PRIMARY (`Item`-flavored) Function
// instances on one source item is still a duplicate-monomorphization
// bug and must be rejected; the relaxation only permits auxiliary
// shims, not duplicate primaries.
// ------------------------------------------------------------------
#[test]
fn bucket1_function_item_with_two_primary_instances_rejected() {
    let mut model = base_model();
    let item = push_item(&mut model, "t", ItemKind::Function, Linkage::Default, 1);
    let sig = unit_fn_sig(&mut model);
    let emission = model.items[item].def_path.rendered;
    model.instances.push(function_instance_with_flavor(
        sig,
        item,
        Some(regseq_model::Body::default()),
        emission,
        FunctionInstanceFlavor::Item,
    ));
    model.instances.push(function_instance_with_flavor(
        sig,
        item,
        Some(regseq_model::Body::default()),
        emission,
        FunctionInstanceFlavor::Item,
    ));
    assert!(matches!(
        model.validate_bridge_completeness(),
        Err(InvariantViolation::Bucket1MultipleInstances {
            item: m_item,
            item_kind: ItemKind::Function,
            count: 2,
        }) if m_item == item
    ));
}

// ------------------------------------------------------------------
// Bucket 1 (regseq-driver-73 hardening): the per-item relaxation only
// bounds the PRIMARY count, so two IDENTICAL shim-flavored instances
// (same canonical key) on one item slip past it -- the global
// unique-instance-key backstop must reject them.
// ------------------------------------------------------------------
#[test]
fn bucket1_function_item_with_duplicate_shim_instances_rejected() {
    let mut model = base_model();
    let item = push_item(&mut model, "t", ItemKind::Function, Linkage::Default, 1);
    let sig = unit_fn_sig(&mut model);
    let emission = model.items[item].def_path.rendered;
    model.instances.push(function_instance_with_flavor(
        sig,
        item,
        Some(regseq_model::Body::default()),
        emission,
        FunctionInstanceFlavor::Item,
    ));
    model.instances.push(function_instance_with_flavor(
        sig,
        item,
        None,
        emission,
        FunctionInstanceFlavor::ReifyShim,
    ));
    model.instances.push(function_instance_with_flavor(
        sig,
        item,
        None,
        emission,
        FunctionInstanceFlavor::ReifyShim,
    ));
    assert!(matches!(
        model.validate_bridge_completeness(),
        Err(InvariantViolation::DuplicateInstanceKey { .. })
    ));
}

// ------------------------------------------------------------------
// Bucket 1 (regseq-driver-73): a primary plus several DISTINCT-flavored
// shim instances (distinct keys) on one item is legitimate and accepted
// -- the backstop only rejects identical-key duplicates.
// ------------------------------------------------------------------
#[test]
fn bucket1_function_item_with_distinct_shims_accepted() {
    let mut model = base_model();
    let item = push_item(&mut model, "t", ItemKind::Function, Linkage::Default, 1);
    let sig = unit_fn_sig(&mut model);
    let emission = model.items[item].def_path.rendered;
    model.instances.push(function_instance_with_flavor(
        sig,
        item,
        Some(regseq_model::Body::default()),
        emission,
        FunctionInstanceFlavor::Item,
    ));
    model.instances.push(function_instance_with_flavor(
        sig,
        item,
        None,
        emission,
        FunctionInstanceFlavor::ReifyShim,
    ));
    model.instances.push(function_instance_with_flavor(
        sig,
        item,
        None,
        emission,
        FunctionInstanceFlavor::CloneShim,
    ));
    assert_eq!(model.validate_bridge_completeness(), Ok(()));
}

// ------------------------------------------------------------------
// Bucket 2: foreign Function Instance with `body: Some(_)` must fail
// the foreign-body-None invariant per spec.md section 9.4
// (`spec.md:2642-2648`).
// ------------------------------------------------------------------
#[test]
fn bucket2_foreign_function_body_must_be_none() {
    let mut model = base_model();
    let item = push_item(
        &mut model,
        "extern_with_body",
        ItemKind::Function,
        Linkage::Foreign,
        1,
    );
    let sig = unit_fn_sig(&mut model);
    let emission = model.items[item].def_path.rendered;
    // A body that is `Some(_)` for a foreign Item is the carved-out
    // shape. Construct a minimal `Body` (the contents don't matter
    // for the validator's body/init check) and assert rejection.
    let body = regseq_model::Body::default();
    model
        .instances
        .push(function_instance(sig, item, Some(body), emission));
    assert!(matches!(
        model.validate_bridge_completeness(),
        Err(InvariantViolation::ForeignInstanceBodyOrInitNotNone {
            item: m_item,
            item_kind: ItemKind::Function,
        }) if m_item == item
    ));
}

// ------------------------------------------------------------------
// Bucket 2: foreign Static Instance with `init: Some(_)` must fail
// the foreign-init-None invariant per spec.md section 9.4
// (`spec.md:2668-2677`).
// ------------------------------------------------------------------
#[test]
fn bucket2_foreign_static_init_must_be_none() {
    let mut model = base_model();
    let item = push_item(
        &mut model,
        "EXTERN_WITH_INIT",
        ItemKind::Static {
            mutability: Mutability::Not,
        },
        Linkage::Foreign,
        1,
    );
    let ty = model.types.intern(Ty::Bool);
    let init = model.consts.intern(Const::Bool(false));
    model.instances.push(static_instance(item, ty, Some(init)));
    assert!(matches!(
        model.validate_bridge_completeness(),
        Err(InvariantViolation::ForeignInstanceBodyOrInitNotNone {
            item: m_item,
            item_kind: ItemKind::Static { .. },
        }) if m_item == item
    ));
}

// ------------------------------------------------------------------
// Bucket 3: generic Item with 0 Instances must pass; with 2 must
// also pass (generics may have any number of monomorphisations).
// ------------------------------------------------------------------
#[test]
fn bucket3_generic_zero_instances_accepted() {
    let mut model = base_model();
    let item = push_item(&mut model, "Slot", ItemKind::Struct, Linkage::Default, 1);
    model.item_bridge_flags.insert(
        item,
        BridgeItemFlags {
            is_generic: true,
            signature_non_rigid: false,
        },
    );
    assert_eq!(model.validate_bridge_completeness(), Ok(()));
}

#[test]
fn bucket3_generic_two_instances_accepted() {
    let mut model = base_model();
    let item = push_item(&mut model, "Pair", ItemKind::Struct, Linkage::Default, 1);
    model.item_bridge_flags.insert(
        item,
        BridgeItemFlags {
            is_generic: true,
            signature_non_rigid: false,
        },
    );
    // Two distinct monomorphisations: `Pair<bool>` and `Pair<()>`.
    let bool_ty = model.types.intern(Ty::Bool);
    let unit_ty = model.types.intern(Ty::Tuple {
        fields: vec![],
        layout: empty_struct_layout(),
    });
    let layout_a = model.types.intern(Ty::Struct {
        item,
        args: vec![GenericArg::Type(bool_ty)],
        layout: Layout {
            size: 1,
            align: 1,
            pack: None,
            uninhabited: false,
            kind: LayoutKind::Struct {
                fields: vec![FieldLayout {
                    offset: 0,
                    ty: bool_ty,
                    name: StringId::ZERO,
                }],
            },
        },
    });
    let layout_b = model.types.intern(Ty::Struct {
        item,
        args: vec![GenericArg::Type(unit_ty)],
        layout: Layout {
            size: 0,
            align: 1,
            pack: None,
            uninhabited: false,
            kind: LayoutKind::Struct {
                fields: vec![FieldLayout {
                    offset: 0,
                    ty: unit_ty,
                    name: StringId::ZERO,
                }],
            },
        },
    });
    let emission = model.items[item].def_path.rendered;
    model.instances.push(Instance {
        item,
        generic_args: vec![GenericArg::Type(bool_ty)],
        emission_name: emission,
        span: Span::dummy(),
        kind: InstanceKind::Struct { layout: layout_a },
    });
    model.instances.push(Instance {
        item,
        generic_args: vec![GenericArg::Type(unit_ty)],
        emission_name: emission,
        span: Span::dummy(),
        kind: InstanceKind::Struct { layout: layout_b },
    });
    assert_eq!(model.validate_bridge_completeness(), Ok(()));
}

// ------------------------------------------------------------------
// Bucket 4: Function Item with `signature_non_rigid == true` and
// zero Instances must pass.
// ------------------------------------------------------------------
#[test]
fn bucket4_rpit_function_zero_instances_accepted() {
    let mut model = base_model();
    let item = push_item(
        &mut model,
        "make_counter",
        ItemKind::Function,
        Linkage::Default,
        1,
    );
    model.item_bridge_flags.insert(
        item,
        BridgeItemFlags {
            is_generic: false,
            signature_non_rigid: true,
        },
    );
    assert_eq!(model.validate_bridge_completeness(), Ok(()));
}

// ------------------------------------------------------------------
// Bucket 4: Function Item with `signature_non_rigid == true` and a
// matching Instance must FAIL. A non-rigid signature cannot be
// monomorphised, so any Instance for such an Item is a bridge bug.
// ------------------------------------------------------------------
#[test]
fn bucket4_rpit_function_with_instance_rejected() {
    let mut model = base_model();
    let item = push_item(
        &mut model,
        "make_counter_bad",
        ItemKind::Function,
        Linkage::Default,
        1,
    );
    model.item_bridge_flags.insert(
        item,
        BridgeItemFlags {
            is_generic: false,
            signature_non_rigid: true,
        },
    );
    let sig = unit_fn_sig(&mut model);
    let emission = model.items[item].def_path.rendered;
    model
        .instances
        .push(function_instance(sig, item, None, emission));
    assert!(matches!(
        model.validate_bridge_completeness(),
        Err(InvariantViolation::Bucket4HasInstance {
            item: m_item,
            count: 1,
        }) if m_item == item
    ));
}
