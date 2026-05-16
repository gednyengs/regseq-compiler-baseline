use std::sync::Arc;

use regseq_model::{
    render_def_path, ArgsKey, Body, BridgeInput, Byte, Const, ConstInterner, CrateId, DefPath,
    DefPathData, DefPathHash, DefPathSegment, FnAbi, FnSig, FnSigInterner, FunctionInstanceData,
    FunctionInstanceFlavor, GenericArg, IndexVec, InlineHint, Instance, InstanceKind, InstanceRef,
    InvariantViolation, Item, ItemId, ItemKind, Linkage, PrincipalTraitKey, ProjectionBindingsKey,
    RegseqModel, Safety, SourceFile, SourceFileId, Span, StableCrateId, StringId,
    SyntheticItemKind, Ty, TyId, TyInterner,
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

fn principal(local_hash: u64) -> PrincipalTraitKey {
    PrincipalTraitKey {
        trait_path: hash(local_hash),
        trait_args: ArgsKey::EMPTY,
        projection_bindings: ProjectionBindingsKey(0),
    }
}

fn def_path(model: &mut RegseqModel, name: &str, crate_id: CrateId) -> DefPath {
    let name_id = model.strings.intern(name);
    let crate_name = model
        .strings
        .resolve(model.crates[crate_id].name)
        .to_owned();
    let rendered = model.strings.intern(&format!("{crate_name}::{name}"));
    DefPath {
        crate_id,
        segments: vec![
            DefPathSegment {
                data: DefPathData::CrateRoot,
                disambiguator: 0,
            },
            DefPathSegment {
                data: DefPathData::ValueNs(name_id),
                disambiguator: 0,
            },
        ],
        rendered,
    }
}

fn item(model: &mut RegseqModel, name: &str, kind: ItemKind) -> Item {
    Item {
        def_path: def_path(model, name, model.primary_crate),
        kind,
        span: Span::dummy(),
        linkage: Linkage::Default,
        link_section: None,
        inline_hint: InlineHint::None,
        is_unsafe: false,
        is_phantom_data: false,
        is_unsafe_cell: false,
        is_manually_drop: false,
        opaque_handle_eligible: false,
    }
}

fn push_item(model: &mut RegseqModel, name: &str, kind: ItemKind, local_hash: u64) -> ItemId {
    let item = item(model, name, kind);
    let id = model.items.push(item);
    model.item_def_paths.push(hash(local_hash));
    model.item_synthetic_kinds.push(SyntheticItemKind::None);
    model.domain_attrs.item_attrs.push(Vec::new());
    id
}

fn push_item_with_data(
    model: &mut RegseqModel,
    data: DefPathData,
    disambiguator: u32,
    local_hash: u64,
) -> ItemId {
    push_item_with_data_in_crate(model, model.primary_crate, data, disambiguator, local_hash)
}

fn push_item_with_data_in_crate(
    model: &mut RegseqModel,
    crate_id: CrateId,
    data: DefPathData,
    disambiguator: u32,
    local_hash: u64,
) -> ItemId {
    let mut path = DefPath {
        crate_id,
        segments: vec![DefPathSegment {
            data,
            disambiguator,
        }],
        rendered: StringId::ZERO,
    };
    let rendered = render_def_path(&path, &model.crates, &model.strings);
    path.rendered = model.strings.intern(&rendered);
    let item = Item {
        def_path: path,
        kind: ItemKind::Function,
        span: Span::dummy(),
        linkage: Linkage::Default,
        link_section: None,
        inline_hint: InlineHint::None,
        is_unsafe: false,
        is_phantom_data: false,
        is_unsafe_cell: false,
        is_manually_drop: false,
        opaque_handle_eligible: false,
    };
    let id = model.items.push(item);
    model.item_def_paths.push(hash(local_hash));
    model.item_synthetic_kinds.push(SyntheticItemKind::None);
    model.domain_attrs.item_attrs.push(Vec::new());
    id
}

fn all_ranked_def_path_data(model: &mut RegseqModel) -> Vec<DefPathData> {
    let n = model.strings.intern("n");
    vec![
        DefPathData::CrateRoot,
        DefPathData::Impl,
        DefPathData::ForeignMod,
        DefPathData::Use,
        DefPathData::GlobalAsm,
        DefPathData::TypeNs(n),
        DefPathData::ValueNs(n),
        DefPathData::MacroNs(n),
        DefPathData::LifetimeNs(n),
        DefPathData::Closure,
        DefPathData::Ctor,
        DefPathData::AnonConst,
        DefPathData::LateAnonConst,
        DefPathData::DesugaredAnonymousLifetime,
        DefPathData::OpaqueTy,
        DefPathData::OpaqueLifetime(n),
        DefPathData::AnonAssocTy(n),
        DefPathData::SyntheticCoroutineBody,
        DefPathData::NestedStatic,
    ]
}

fn sig() -> FnSig {
    FnSig {
        inputs: vec![],
        output: TyId::ZERO,
        abi: FnAbi::Rust,
        c_variadic: false,
        safety: Safety::Safe,
    }
}

fn base_model() -> RegseqModel {
    let mut model = RegseqModel::with_primary("pl011_spec", StableCrateId(1));
    model.types.push(Ty::Bool);
    model
}

fn add_function_instance(model: &mut RegseqModel, item: ItemId) {
    let sig = model.sigs.intern(sig());
    model.sig_c_abi_lowerings.push(None);
    model.instances.push(Instance {
        item,
        generic_args: vec![],
        emission_name: StringId::ZERO,
        span: Span::dummy(),
        kind: InstanceKind::Function(FunctionInstanceData {
            fn_sig: sig,
            body: Some(Body::default()),
            flavor: FunctionInstanceFlavor::Item,
            virtual_idx: None,
        }),
    });
}

#[test]
fn invariant_1_pass_full_closure() {
    let mut model = base_model();
    let item = push_item(&mut model, "driver", ItemKind::Function, 1);
    let c = model.consts.intern(Const::NullPtr(TyId::ZERO));
    let sig = model.sigs.intern(FnSig {
        inputs: vec![TyId::ZERO],
        output: TyId::ZERO,
        abi: FnAbi::Rust,
        c_variadic: false,
        safety: Safety::Safe,
    });
    model.sig_c_abi_lowerings.push(None);
    model.instances.push(Instance {
        item,
        generic_args: vec![GenericArg::Type(TyId::ZERO), GenericArg::Const(c)],
        emission_name: StringId::ZERO,
        span: Span::dummy(),
        kind: InstanceKind::Function(FunctionInstanceData {
            fn_sig: sig,
            body: Some(Body::default()),
            flavor: FunctionInstanceFlavor::Item,
            virtual_idx: None,
        }),
    });
    assert_eq!(validate(&mut model), Ok(()));
}

#[test]
fn invariant_1_reject_dangling_ty_in_const_null_ptr() {
    let mut model = base_model();
    let item = push_item(&mut model, "driver", ItemKind::Function, 1);
    add_function_instance(&mut model, item);
    model.consts.intern(Const::NullPtr(TyId::from_u32(99)));
    assert!(matches!(
        validate(&mut model),
        Err(InvariantViolation::DanglingId { field, value: 99, .. })
            if field.contains("Const")
    ));
}

#[test]
fn invariant_2_reject_synthetic_kinds_short() {
    let mut model = base_model();
    let item = item(&mut model, "driver", ItemKind::Function);
    let _ = model.items.push(item);
    model.item_def_paths.push(hash(1));
    assert!(matches!(
        validate(&mut model),
        Err(InvariantViolation::ItemSyntheticKindsLengthMismatch { .. })
    ));
}

#[test]
fn invariant_6_reject_def_path_crate_id_out_of_bounds() {
    let mut model = base_model();
    let mut item = item(&mut model, "driver", ItemKind::Function);
    item.def_path.crate_id = CrateId::from_u32(9);
    model.items.push(item);
    model.item_def_paths.push(hash(1));
    model.item_synthetic_kinds.push(SyntheticItemKind::None);
    model.domain_attrs.item_attrs.push(Vec::new());
    assert!(matches!(
        validate(&mut model),
        Err(InvariantViolation::DefPathCrateIdOutOfBounds { crate_id, .. })
            if crate_id == CrateId::from_u32(9)
    ));
}

#[test]
fn invariant_9_reject_const_reloc() {
    let mut model = base_model();
    let item = push_item(&mut model, "driver", ItemKind::Function, 1);
    add_function_instance(&mut model, item);
    model.consts.intern(Const::Reloc(InstanceRef {
        regseq_kind_rank: 0,
        def_path_hash: hash(999),
        args_key: ArgsKey::EMPTY,
        fn_shim_disambig: 0,
    }));
    assert!(matches!(
        validate(&mut model),
        Err(InvariantViolation::DanglingId { field, .. }) if field.contains("relocation")
    ));
}

#[test]
fn invariant_9_pass_const_reloc_and_bytes_reloc() {
    let mut model = base_model();
    let item = push_item(&mut model, "driver", ItemKind::Function, 1);
    add_function_instance(&mut model, item);
    let instance = InstanceRef {
        regseq_kind_rank: 0,
        def_path_hash: hash(1),
        args_key: ArgsKey::EMPTY,
        fn_shim_disambig: 0,
    };
    model.consts.intern(Const::Reloc(instance));
    model
        .consts
        .intern(Const::Bytes(vec![Byte::Reloc(instance)]));
    assert_eq!(validate(&mut model), Ok(()));
}

#[test]
fn invariant_11_ty_interner_pass_and_reject_duplicate() {
    let mut passing = base_model();
    passing.types.intern(Ty::Uint(regseq_model::UintKind::U32));
    assert_eq!(validate(&mut passing), Ok(()));

    let mut failing = RegseqModel::with_primary("pl011_spec", StableCrateId(1));
    let mut storage = IndexVec::new();
    storage.push(Ty::Bool);
    storage.push(Ty::Bool);
    failing.types = TyInterner::from_storage_unchecked_for_validation(storage);
    assert!(matches!(
        validate(&mut failing),
        Err(InvariantViolation::DuplicateInternerEntry { interner, first: 0, second: 1 })
            if interner == "TyInterner"
    ));
}

#[test]
fn invariant_11_fn_sig_interner_pass_and_reject_duplicate() {
    let mut passing = base_model();
    passing.sigs.intern(sig());
    passing.sig_c_abi_lowerings.push(None);
    assert_eq!(validate(&mut passing), Ok(()));

    let mut failing = base_model();
    let mut storage = IndexVec::new();
    storage.push(sig());
    storage.push(sig());
    failing.sigs = FnSigInterner::from_storage_unchecked_for_validation(storage);
    failing.sig_c_abi_lowerings.push(None);
    failing.sig_c_abi_lowerings.push(None);
    assert!(matches!(
        validate(&mut failing),
        Err(InvariantViolation::DuplicateInternerEntry { interner, first: 0, second: 1 })
            if interner == "FnSigInterner"
    ));
}

#[test]
fn invariant_11_const_interner_pass_and_reject_duplicate() {
    let mut passing = base_model();
    passing.consts.intern(Const::Bool(true));
    passing.consts.intern(Const::Bool(false));
    assert_eq!(validate(&mut passing), Ok(()));

    let mut failing = base_model();
    let mut storage = IndexVec::new();
    storage.push(Const::Bool(true));
    storage.push(Const::Bool(true));
    failing.consts = ConstInterner::from_storage_unchecked_for_validation(storage);
    assert!(matches!(
        validate(&mut failing),
        Err(InvariantViolation::DuplicateInternerEntry { interner, first: 0, second: 1 })
            if interner == "ConstInterner"
    ));
}

#[test]
fn invariant_18_reject_duplicate_item_def_path_hash() {
    let mut model = base_model();
    push_item(&mut model, "a", ItemKind::Function, 1);
    push_item(&mut model, "b", ItemKind::Function, 1);
    assert!(matches!(
        validate(&mut model),
        Err(InvariantViolation::DuplicateItemDefPathHash { first, second, .. })
            if first == ItemId::from_u32(0) && second == ItemId::from_u32(1)
    ));
}

#[test]
fn f4_sort_pass_canonical_fixture() {
    let mut model = base_model();
    let dep_a_name = model.strings.intern("zz_dep_a");
    let dep_a = model.crates.push(regseq_model::CrateInfo {
        name: dep_a_name,
        stable_crate_id: StableCrateId(2),
    });
    let dep_b_name = model.strings.intern("zz_dep_b");
    let dep_b = model.crates.push(regseq_model::CrateInfo {
        name: dep_b_name,
        stable_crate_id: StableCrateId(3),
    });
    let n = model.strings.intern("n");
    push_item_with_data(&mut model, DefPathData::CrateRoot, 0, 1);
    push_item_with_data(&mut model, DefPathData::Impl, 0, 2);
    push_item_with_data(&mut model, DefPathData::TypeNs(n), 0, 3);
    push_item_with_data_in_crate(&mut model, dep_a, DefPathData::CrateRoot, 0, 4);
    push_item_with_data_in_crate(&mut model, dep_b, DefPathData::ValueNs(n), 0, 5);
    assert_eq!(validate(&mut model), Ok(()));
}

#[test]
fn f4_sort_reject_permuted_input() {
    let mut model = base_model();
    push_item(&mut model, "b", ItemKind::Function, 2);
    push_item(&mut model, "a", ItemKind::Function, 1);
    assert!(matches!(
        validate(&mut model),
        Err(InvariantViolation::F4OutOfOrder { first, second })
            if first == ItemId::from_u32(0) && second == ItemId::from_u32(1)
    ));
}

#[test]
fn f4_sort_pass_per_def_path_data_rank() {
    let mut model = base_model();
    for (idx, data) in all_ranked_def_path_data(&mut model).into_iter().enumerate() {
        push_item_with_data(&mut model, data, 0, idx as u64 + 1);
    }
    assert_eq!(validate(&mut model), Ok(()));
}

#[test]
fn f4_sort_pass_ties_admitted() {
    let mut model = base_model();
    push_item(&mut model, "a", ItemKind::Function, 1);
    let mut duplicate_key = item(&mut model, "a", ItemKind::Function);
    duplicate_key.def_path.rendered = model.items[ItemId::ZERO].def_path.rendered;
    model.items.push(duplicate_key);
    model.item_def_paths.push(hash(2));
    model.item_synthetic_kinds.push(SyntheticItemKind::None);
    model.domain_attrs.item_attrs.push(Vec::new());
    assert_eq!(validate(&mut model), Ok(()));
}

#[test]
fn f4_2_sort_reject() {
    let mut model = base_model();
    let item = push_item(&mut model, "driver", ItemKind::Function, 1);
    let sig = model.sigs.intern(sig());
    model.sig_c_abi_lowerings.push(None);
    model.instances.push(Instance {
        item,
        generic_args: vec![],
        emission_name: StringId::ZERO,
        span: Span::dummy(),
        kind: InstanceKind::Static {
            ty: TyId::ZERO,
            init: None,
        },
    });
    model.instances.push(Instance {
        item,
        generic_args: vec![],
        emission_name: StringId::ZERO,
        span: Span::dummy(),
        kind: InstanceKind::Function(FunctionInstanceData {
            fn_sig: sig,
            body: Some(Body::default()),
            flavor: FunctionInstanceFlavor::Item,
            virtual_idx: None,
        }),
    });
    assert!(matches!(
        validate(&mut model),
        Err(InvariantViolation::F4OutOfOrderInstance { .. })
    ));
}

#[test]
fn f4_2_sort_pass() {
    let mut model = base_model();
    let item = push_item(&mut model, "driver", ItemKind::Function, 1);
    let sig = model.sigs.intern(sig());
    model.sig_c_abi_lowerings.push(None);
    model.instances.push(Instance {
        item,
        generic_args: vec![],
        emission_name: StringId::ZERO,
        span: Span::dummy(),
        kind: InstanceKind::Function(FunctionInstanceData {
            fn_sig: sig,
            body: Some(Body::default()),
            flavor: FunctionInstanceFlavor::Item,
            virtual_idx: None,
        }),
    });
    model.instances.push(Instance {
        item,
        generic_args: vec![],
        emission_name: StringId::ZERO,
        span: Span::dummy(),
        kind: InstanceKind::Static {
            ty: TyId::ZERO,
            init: None,
        },
    });
    assert_eq!(validate(&mut model), Ok(()));
}

#[test]
fn def_path_render_reject_malformed() {
    let mut model = base_model();
    let id = push_item(&mut model, "driver", ItemKind::Function, 1);
    model.items[id].def_path.rendered = model.strings.intern("wrong");
    assert!(matches!(
        validate(&mut model),
        Err(InvariantViolation::DefPathRenderMismatch { item, .. }) if item == id
    ));
}

#[test]
fn span_bounds_reject_not_on_utf8_boundary_lo() {
    let mut model = base_model();
    let path = model.strings.intern("src/lib.rs");
    let file = model.source_files.push(SourceFile {
        path,
        content: Some(Arc::from("éx")),
        line_starts: vec![0],
    });
    let id = push_item(&mut model, "driver", ItemKind::Function, 1);
    model.items[id].span = Span { file, lo: 1, hi: 2 };
    assert!(matches!(
        validate(&mut model),
        Err(InvariantViolation::SpanNotOnUtf8Boundary { end: "lo", .. })
    ));
}

#[test]
fn span_bounds_pass_content_some() {
    let mut model = base_model();
    let path = model.strings.intern("src/lib.rs");
    let file = model.source_files.push(SourceFile {
        path,
        content: Some(Arc::from("éx")),
        line_starts: vec![0],
    });
    let id = push_item(&mut model, "driver", ItemKind::Function, 1);
    model.items[id].span = Span { file, lo: 0, hi: 2 };
    assert_eq!(validate(&mut model), Ok(()));
}

#[test]
fn span_bounds_reject_lo_gt_hi() {
    let mut model = base_model();
    let path = model.strings.intern("src/lib.rs");
    let file = model.source_files.push(SourceFile {
        path,
        content: Some(Arc::from("abc")),
        line_starts: vec![0],
    });
    let id = push_item(&mut model, "driver", ItemKind::Function, 1);
    model.items[id].span = Span { file, lo: 2, hi: 1 };
    assert!(matches!(
        validate(&mut model),
        Err(InvariantViolation::SpanOutOfBounds { .. })
    ));
}

#[test]
fn span_bounds_reject_hi_gt_content_len() {
    let mut model = base_model();
    let path = model.strings.intern("src/lib.rs");
    let file = model.source_files.push(SourceFile {
        path,
        content: Some(Arc::from("abc")),
        line_starts: vec![0],
    });
    let id = push_item(&mut model, "driver", ItemKind::Function, 1);
    model.items[id].span = Span { file, lo: 0, hi: 4 };
    assert!(matches!(
        validate(&mut model),
        Err(InvariantViolation::SpanOutOfBounds { .. })
    ));
}

#[test]
fn span_bounds_reject_not_on_utf8_boundary_hi() {
    let mut model = base_model();
    let path = model.strings.intern("src/lib.rs");
    let file = model.source_files.push(SourceFile {
        path,
        content: Some(Arc::from("éx")),
        line_starts: vec![0],
    });
    let id = push_item(&mut model, "driver", ItemKind::Function, 1);
    model.items[id].span = Span { file, lo: 0, hi: 1 };
    assert!(matches!(
        validate(&mut model),
        Err(InvariantViolation::SpanNotOnUtf8Boundary { end: "hi", .. })
    ));
}

#[test]
fn span_bounds_reject_file_out_of_bounds() {
    let mut model = base_model();
    let id = push_item(&mut model, "driver", ItemKind::Function, 1);
    model.items[id].span = Span {
        file: SourceFileId::from_u32(99),
        lo: 0,
        hi: 0,
    };
    assert!(matches!(
        validate(&mut model),
        Err(InvariantViolation::DanglingId { field, value: 99, .. }) if field.contains("Item.span")
    ));
}

#[test]
fn span_bounds_pass_content_none_sentinel() {
    let mut model = base_model();
    let id = push_item(&mut model, "driver", ItemKind::Function, 1);
    model.items[id].span = Span {
        file: SourceFileId::SYNTHESIZED,
        lo: 999,
        hi: 1000,
    };
    assert_eq!(validate(&mut model), Ok(()));
}

#[test]
fn phase_c_vtables_rebuild_pass() {
    let mut model = base_model();
    let first = push_item(&mut model, "a", ItemKind::Struct, 1);
    let second = push_item(&mut model, "b", ItemKind::Struct, 2);
    let first_principal = principal(1);
    let second_principal = principal(2);
    model.item_synthetic_kinds[first] = SyntheticItemKind::VTableType {
        principal: first_principal.clone(),
    };
    model.item_synthetic_kinds[second] = SyntheticItemKind::VTableType {
        principal: second_principal.clone(),
    };
    model.item_def_paths[first] = DefPathHash {
        stable_crate_id: StableCrateId(0),
        local_hash: 1,
    };
    model.item_def_paths[second] = DefPathHash {
        stable_crate_id: StableCrateId(0),
        local_hash: 2,
    };
    assert_eq!(validate(&mut model), Ok(()));
    assert_eq!(model.vtables.get(&first_principal), Some(&first));
    assert_eq!(model.vtables.get(&second_principal), Some(&second));
}

#[test]
fn phase_c_vtables_rebuild_reject_duplicate_principal() {
    let mut model = base_model();
    let first = push_item(&mut model, "a", ItemKind::Struct, 1);
    let second = push_item(&mut model, "b", ItemKind::Struct, 2);
    let principal = principal(1);
    model.item_synthetic_kinds[first] = SyntheticItemKind::VTableType {
        principal: principal.clone(),
    };
    model.item_synthetic_kinds[second] = SyntheticItemKind::VTableType { principal };
    model.item_def_paths[first] = DefPathHash {
        stable_crate_id: StableCrateId(0),
        local_hash: 1,
    };
    model.item_def_paths[second] = DefPathHash {
        stable_crate_id: StableCrateId(0),
        local_hash: 2,
    };
    assert!(matches!(
        validate(&mut model),
        Err(InvariantViolation::DuplicateVTablePrincipal { first: a, second: b, .. })
            if a == first && b == second
    ));
}

#[test]
fn phase_c_vtables_empty_pass() {
    let mut model = base_model();
    push_item(&mut model, "driver", ItemKind::Function, 1);
    model.vtables.insert(principal(1), ItemId::ZERO);
    assert_eq!(validate(&mut model), Ok(()));
    assert!(model.vtables.is_empty());
}

#[test]
fn phase_b_then_c_ordering() {
    let mut model = base_model();
    let first = push_item(&mut model, "a", ItemKind::Struct, 1);
    let second = push_item(&mut model, "b", ItemKind::Struct, 2);
    let principal = principal(1);
    model.item_synthetic_kinds[first] = SyntheticItemKind::VTableType {
        principal: principal.clone(),
    };
    model.item_synthetic_kinds[second] = SyntheticItemKind::VTableType { principal };
    let mut storage = IndexVec::new();
    storage.push(Const::Bool(true));
    storage.push(Const::Bool(true));
    model.consts = ConstInterner::from_storage_unchecked_for_validation(storage);
    assert!(matches!(
        validate(&mut model),
        Err(InvariantViolation::DuplicateInternerEntry { interner, .. })
            if interner == "ConstInterner"
    ));
}
