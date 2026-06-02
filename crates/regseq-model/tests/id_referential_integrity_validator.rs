use std::sync::Arc;

use regseq_model::{
    render_def_path, ArgsKey, BasicBlock, BlockId, Body, BridgeInput, Byte, CallCallee, Const,
    ConstId, ConstInterner, CopyNonOverlapping, CrateId, DefPath, DefPathData, DefPathHash,
    DefPathSegment, FieldIdx, FnAbi, FnSig, FnSigInterner, FunctionInstanceData,
    FunctionInstanceFlavor, GenericArg, IndexVec, InlineHint, Instance, InstanceKind, InstanceRef,
    InvariantViolation, Item, ItemId, ItemKind, Layout, LayoutKind, Linkage, LocalDecl, LocalId,
    Mutability, NonDivergingIntrinsic, Operand, Place, PlaceElem, PrincipalTraitKey,
    ProjectionBindingsKey, RegseqModel, Rvalue, Safety, SourceFile, SourceFileId, Span,
    StableCrateId, Statement, StatementKind, StringId, SwitchTargets, SyntheticItemKind,
    Terminator, TerminatorKind, Ty, TyId, TyInterner, UintKind, UnOp, VariantIdx, WrappingRange,
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
        is_result: false,
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
        is_result: false,
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
fn instance_ref_of_returns_validator_accepted_reference() {
    let mut model = base_model();
    let item = push_item(&mut model, "driver", ItemKind::Function, 1);
    add_function_instance(&mut model, item);

    // The reference read back from the materialized instance equals the
    // canonical fields the validator builds its accepted set from.
    let reference = model.instance_ref_of(regseq_model::InstanceId::ZERO);
    assert_eq!(
        reference,
        InstanceRef {
            regseq_kind_rank: 0,
            def_path_hash: hash(1),
            args_key: ArgsKey::EMPTY,
            fn_shim_disambig: 0,
        }
    );

    // A `Const::Reloc` carrying that reference validates, proving the
    // bridge can use `instance_ref_of` to emit relocations that cannot
    // drift from `check_instance_ref`'s accepted set.
    model.consts.intern(Const::Reloc(reference));
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

// ---------------------------------------------------------------------
// Body-level invariant tests (audit-finding 1-6 additions).
//
// These helpers construct a self-contained RegseqModel that carries
// the matching nominal types, a single function instance whose body
// hosts the offending statement / terminator, and the minimum
// supporting state the joint validator needs (FnSig, return type, two
// blocks for Goto / Assert / Call return paths). Every test below
// asserts the precise InvariantViolation variant returned by the
// validator after the audit-finding work landed.
// ---------------------------------------------------------------------

fn unit_layout() -> Layout {
    Layout {
        size: 0,
        align: 1,
        pack: None,
        uninhabited: false,
        kind: LayoutKind::Struct { fields: vec![] },
    }
}

struct BodyFixture {
    model: RegseqModel,
    bool_ty: TyId,
    u32_ty: TyId,
    unit_ty: TyId,
    bool_const: ConstId,
    u32_const: ConstId,
    next_local_hash: u64,
}

impl BodyFixture {
    fn new() -> Self {
        let mut model = RegseqModel::with_primary("pl011_spec", StableCrateId(1));
        let bool_ty = model.types.intern(Ty::Bool);
        let u32_ty = model.types.intern(Ty::Uint(UintKind::U32));
        let unit_ty = model.types.intern(Ty::Tuple {
            fields: vec![],
            layout: unit_layout(),
        });
        let bool_const = model.consts.intern(Const::Bool(true));
        let u32_const = model.consts.intern(Const::Uint(UintKind::U32, 0));
        Self {
            model,
            bool_ty,
            u32_ty,
            unit_ty,
            bool_const,
            u32_const,
            next_local_hash: 1,
        }
    }

    /// Pushes an item with a per-fixture monotonically increasing
    /// DefPathHash and a name that strictly orders after every
    /// previously-pushed item per the F4 sort key
    /// (`(crate_name, stable_crate_id, segments)`). Items in the
    /// fixture share a crate; the per-test name encodes the sort
    /// position via a numeric prefix.
    fn push_named_item(&mut self, name: &str, kind: ItemKind) -> ItemId {
        let local_hash = self.next_local_hash;
        self.next_local_hash += 1;
        let prefixed = format!("z_{:03}_{}", local_hash, name);
        push_item(&mut self.model, &prefixed, kind, local_hash)
    }

    fn local(ty: TyId) -> LocalDecl {
        LocalDecl {
            ty,
            mutability: Mutability::Mut,
            span: Span::dummy(),
            debug_name: None,
        }
    }

    fn body_from_blocks(local_tys: &[TyId], blocks: Vec<BasicBlock>) -> Body {
        let mut local_decls = IndexVec::new();
        for ty in local_tys {
            local_decls.push(Self::local(*ty));
        }
        let mut basic_blocks = IndexVec::new();
        for block in blocks {
            basic_blocks.push(block);
        }
        Body {
            basic_blocks,
            local_decls,
            arg_count: 0,
            span: Span::dummy(),
        }
    }

    fn push_function_with_body(
        &mut self,
        body: Body,
        inputs: Vec<TyId>,
        output: TyId,
    ) -> regseq_model::InstanceId {
        let item = self.push_named_item("driver", ItemKind::Function);
        let sig = self.model.sigs.intern(FnSig {
            inputs,
            output,
            abi: FnAbi::Rust,
            c_variadic: false,
            safety: Safety::Safe,
        });
        self.model.sig_c_abi_lowerings.push(None);
        self.model.instances.push(Instance {
            item,
            generic_args: vec![],
            emission_name: StringId::ZERO,
            span: Span::dummy(),
            kind: InstanceKind::Function(FunctionInstanceData {
                fn_sig: sig,
                body: Some(body),
                flavor: FunctionInstanceFlavor::Item,
                virtual_idx: None,
            }),
        })
    }
}

fn return_block() -> BasicBlock {
    BasicBlock {
        statements: Box::new([]),
        terminator: Terminator {
            kind: TerminatorKind::Return,
            span: Span::dummy(),
        },
    }
}

fn block_with_terminator(kind: TerminatorKind) -> BasicBlock {
    BasicBlock {
        statements: Box::new([]),
        terminator: Terminator {
            kind,
            span: Span::dummy(),
        },
    }
}

fn block_with_statements(statements: Vec<Statement>) -> BasicBlock {
    BasicBlock {
        statements: statements.into_boxed_slice(),
        terminator: Terminator {
            kind: TerminatorKind::Return,
            span: Span::dummy(),
        },
    }
}

fn place_local(local: u32) -> Place {
    Place::from(LocalId::from_u32(local))
}

#[test]
fn statement_b1_assign_type_mismatch_rejects() {
    let mut f = BodyFixture::new();
    // local _0: u32, rvalue: bool constant -> mismatch.
    let stmt = Statement {
        kind: StatementKind::Assign(place_local(0), Rvalue::Use(Operand::Constant(f.bool_const))),
        span: Span::dummy(),
    };
    let body = BodyFixture::body_from_blocks(&[f.u32_ty], vec![block_with_statements(vec![stmt])]);
    f.push_function_with_body(body, vec![], f.u32_ty);
    let err = f
        .model
        .validate(&BridgeInput::matching(&f.model))
        .unwrap_err();
    match err {
        InvariantViolation::StatementAssignTypeMismatch {
            place_ty,
            rvalue_ty,
            ..
        } => {
            assert_eq!(place_ty, f.u32_ty);
            assert_eq!(rvalue_ty, f.bool_ty);
        }
        other => panic!("expected StatementAssignTypeMismatch, got {other:?}"),
    }
}

#[test]
fn statement_b2_set_discriminant_variant_idx_out_of_range_rejects() {
    let mut f = BodyFixture::new();
    // Build a 2-variant enum type.
    let item = f.push_named_item("Enum2", ItemKind::Enum);
    let v0 = f.model.strings.intern("V0");
    let v1 = f.model.strings.intern("V1");
    let enum_ty = f.model.types.intern(Ty::Enum {
        item,
        args: vec![],
        layout: Layout {
            size: 4,
            align: 4,
            pack: None,
            uninhabited: false,
            kind: LayoutKind::Enum {
                shape: regseq_model::EnumShape::Multiple {
                    tag: regseq_model::TagScalar {
                        scalar_ty: f.u32_ty,
                        offset: 0,
                        valid_range: WrappingRange { start: 0, end: 1 },
                    },
                    encoding: regseq_model::TagEncoding::Direct,
                    variants: vec![
                        regseq_model::VariantLayout {
                            name: v0,
                            fields: vec![],
                            uninhabited: false,
                            discriminant: 0,
                            tag_value: Some(0),
                        },
                        regseq_model::VariantLayout {
                            name: v1,
                            fields: vec![],
                            uninhabited: false,
                            discriminant: 1,
                            tag_value: Some(1),
                        },
                    ],
                },
            },
        },
    });
    let stmt = Statement {
        kind: StatementKind::SetDiscriminant(place_local(0), VariantIdx::from_u32(5)),
        span: Span::dummy(),
    };
    let body = BodyFixture::body_from_blocks(&[enum_ty], vec![block_with_statements(vec![stmt])]);
    f.push_function_with_body(body, vec![], f.unit_ty);
    let err = f
        .model
        .validate(&BridgeInput::matching(&f.model))
        .unwrap_err();
    assert!(matches!(
        err,
        InvariantViolation::SetDiscriminantVariantIdxOutOfRange { max: 2, .. }
    ));
}

#[test]
fn statement_b3_set_discriminant_non_enum_target_rejects() {
    let mut f = BodyFixture::new();
    // local _0 is u32 (non-enum) -> SetDiscriminant should reject.
    let stmt = Statement {
        kind: StatementKind::SetDiscriminant(place_local(0), VariantIdx::from_u32(0)),
        span: Span::dummy(),
    };
    let body = BodyFixture::body_from_blocks(&[f.u32_ty], vec![block_with_statements(vec![stmt])]);
    f.push_function_with_body(body, vec![], f.unit_ty);
    let err = f
        .model
        .validate(&BridgeInput::matching(&f.model))
        .unwrap_err();
    assert!(matches!(
        err,
        InvariantViolation::SetDiscriminantNonEnumTarget { place_ty, .. } if place_ty == f.u32_ty
    ));
}

#[test]
fn statement_b4_assume_operand_not_bool_rejects() {
    let mut f = BodyFixture::new();
    let stmt = Statement {
        kind: StatementKind::Intrinsic(NonDivergingIntrinsic::Assume(Operand::Constant(
            f.u32_const,
        ))),
        span: Span::dummy(),
    };
    let body = BodyFixture::body_from_blocks(&[], vec![block_with_statements(vec![stmt])]);
    f.push_function_with_body(body, vec![], f.unit_ty);
    let err = f
        .model
        .validate(&BridgeInput::matching(&f.model))
        .unwrap_err();
    assert!(matches!(
        err,
        InvariantViolation::AssumeOperandNotBool { operand_ty, .. } if operand_ty == f.u32_ty
    ));
}

#[test]
fn statement_b5_copy_non_overlapping_count_type_rejects() {
    let mut f = BodyFixture::new();
    let usize_ty = f.model.types.intern(Ty::Uint(UintKind::Usize));
    // The `count` operand recovers its type via the const interner's
    // `find(&Ty::Uint(Usize))`, so the usize TYPE must be interned (else
    // `expect_operand_ty(count)` is NotApplicable -> the count arm fires
    // before the src/dst/pointee checks under test).
    f.model.types.intern(Ty::Uint(UintKind::Usize));
    let usize_const = f.model.consts.intern(Const::Uint(UintKind::Usize, 0));
    let _ = usize_const;
    let _ = usize_ty;
    let ptr_u32 = f.model.types.intern(Ty::Ptr(f.u32_ty, Mutability::Mut));
    // src/dst typed as *mut u32 (pass through local _0/_1). count is
    // a u32 (wrong type; spec requires Ty::Uint(Usize)).
    let stmt = Statement {
        kind: StatementKind::Intrinsic(NonDivergingIntrinsic::CopyNonOverlapping(
            CopyNonOverlapping {
                src: Operand::Copy(place_local(0)),
                dst: Operand::Copy(place_local(1)),
                count: Operand::Constant(f.u32_const),
            },
        )),
        span: Span::dummy(),
    };
    let body =
        BodyFixture::body_from_blocks(&[ptr_u32, ptr_u32], vec![block_with_statements(vec![stmt])]);
    f.push_function_with_body(body, vec![], f.unit_ty);
    let err = f
        .model
        .validate(&BridgeInput::matching(&f.model))
        .unwrap_err();
    assert!(matches!(
        err,
        InvariantViolation::CopyNonOverlappingOperandType { which: "count", .. }
    ));
}

#[test]
fn terminator_b1_switch_int_discr_type_mismatch_rejects() {
    let mut f = BodyFixture::new();
    // Build a 2-variant enum type whose value is the SwitchInt
    // discriminant (an enum value, not an integer-typed Operand).
    let item = f.push_named_item("Enum2", ItemKind::Enum);
    let v0 = f.model.strings.intern("V0");
    let enum_ty = f.model.types.intern(Ty::Enum {
        item,
        args: vec![],
        layout: Layout {
            size: 4,
            align: 4,
            pack: None,
            uninhabited: false,
            kind: LayoutKind::Enum {
                shape: regseq_model::EnumShape::Single {
                    variant: regseq_model::VariantLayout {
                        name: v0,
                        fields: vec![],
                        uninhabited: false,
                        discriminant: 0,
                        tag_value: None,
                    },
                    scalar: None,
                },
            },
        },
    });
    let switch = block_with_terminator(TerminatorKind::SwitchInt {
        discr: Operand::Copy(place_local(0)),
        targets: SwitchTargets {
            cases: Box::new([]),
            otherwise: BlockId::from_u32(1),
        },
    });
    let body = BodyFixture::body_from_blocks(&[enum_ty], vec![switch, return_block()]);
    f.push_function_with_body(body, vec![], f.unit_ty);
    let err = f
        .model
        .validate(&BridgeInput::matching(&f.model))
        .unwrap_err();
    assert!(matches!(
        err,
        InvariantViolation::SwitchIntDiscrTypeMismatch { operand_ty, .. } if operand_ty == enum_ty
    ));
}

#[test]
fn terminator_b2_assert_cond_not_bool_rejects() {
    let mut f = BodyFixture::new();
    let assert = block_with_terminator(TerminatorKind::Assert {
        cond: Operand::Constant(f.u32_const),
        expected: true,
        msg: regseq_model::AssertKind::NullPointerDereference,
        target: BlockId::from_u32(1),
    });
    let body = BodyFixture::body_from_blocks(&[], vec![assert, return_block()]);
    f.push_function_with_body(body, vec![], f.unit_ty);
    let err = f
        .model
        .validate(&BridgeInput::matching(&f.model))
        .unwrap_err();
    assert!(matches!(
        err,
        InvariantViolation::AssertCondNotBool { cond_ty, .. } if cond_ty == f.u32_ty
    ));
}

#[test]
fn terminator_b3_call_destination_type_mismatch_rejects() {
    let mut f = BodyFixture::new();
    // Function callee returns u32; destination place is bool.
    let callee_sig = f.model.sigs.intern(FnSig {
        inputs: vec![],
        output: f.u32_ty,
        abi: FnAbi::Rust,
        c_variadic: false,
        safety: Safety::Safe,
    });
    f.model.sig_c_abi_lowerings.push(None);
    let callee_item = f.push_named_item("callee", ItemKind::Function);
    let callee_id = f.model.instances.push(Instance {
        item: callee_item,
        generic_args: vec![],
        emission_name: StringId::ZERO,
        span: Span::dummy(),
        kind: InstanceKind::Function(FunctionInstanceData {
            fn_sig: callee_sig,
            body: Some(Body::default()),
            flavor: FunctionInstanceFlavor::Item,
            virtual_idx: None,
        }),
    });
    let call = block_with_terminator(TerminatorKind::Call {
        callee: CallCallee::Direct(callee_id),
        args: Box::new([]),
        // local _0: bool != callee output u32.
        destination: place_local(0),
        target: Some(BlockId::from_u32(1)),
    });
    let body = BodyFixture::body_from_blocks(&[f.bool_ty], vec![call, return_block()]);
    f.push_function_with_body(body, vec![], f.unit_ty);
    let err = f
        .model
        .validate(&BridgeInput::matching(&f.model))
        .unwrap_err();
    match err {
        InvariantViolation::CallDestinationTypeMismatch {
            expected_ty,
            actual_ty,
            ..
        } => {
            assert_eq!(expected_ty, f.u32_ty);
            assert_eq!(actual_ty, f.bool_ty);
        }
        other => panic!("expected CallDestinationTypeMismatch, got {other:?}"),
    }
}

#[test]
fn terminator_b4_call_args_arity_mismatch_rejects_direct() {
    let mut f = BodyFixture::new();
    let callee_sig = f.model.sigs.intern(FnSig {
        inputs: vec![f.u32_ty, f.u32_ty],
        output: f.unit_ty,
        abi: FnAbi::Rust,
        c_variadic: false,
        safety: Safety::Safe,
    });
    f.model.sig_c_abi_lowerings.push(None);
    let callee_item = f.push_named_item("callee", ItemKind::Function);
    let callee_id = f.model.instances.push(Instance {
        item: callee_item,
        generic_args: vec![],
        emission_name: StringId::ZERO,
        span: Span::dummy(),
        kind: InstanceKind::Function(FunctionInstanceData {
            fn_sig: callee_sig,
            body: Some(Body::default()),
            flavor: FunctionInstanceFlavor::Item,
            virtual_idx: None,
        }),
    });
    let call = block_with_terminator(TerminatorKind::Call {
        callee: CallCallee::Direct(callee_id),
        // Only 1 arg but sig has 2 inputs.
        args: Box::new([Operand::Constant(f.u32_const)]),
        destination: place_local(0),
        target: Some(BlockId::from_u32(1)),
    });
    let body = BodyFixture::body_from_blocks(&[f.unit_ty], vec![call, return_block()]);
    f.push_function_with_body(body, vec![], f.unit_ty);
    let err = f
        .model
        .validate(&BridgeInput::matching(&f.model))
        .unwrap_err();
    assert!(matches!(
        err,
        InvariantViolation::CallArgsArityMismatch {
            expected: 2,
            actual: 1,
            ..
        }
    ));
}

#[test]
fn terminator_b4_call_args_arity_mismatch_rejects_indirect() {
    let mut f = BodyFixture::new();
    let callee_sig = f.model.sigs.intern(FnSig {
        inputs: vec![f.u32_ty],
        output: f.unit_ty,
        abi: FnAbi::Rust,
        c_variadic: false,
        safety: Safety::Safe,
    });
    f.model.sig_c_abi_lowerings.push(None);
    let fn_ptr_ty = f.model.types.intern(Ty::FnPtr(callee_sig));
    // Indirect callee operand whose type is the fn ptr above.
    let call = block_with_terminator(TerminatorKind::Call {
        callee: CallCallee::Indirect(Operand::Copy(place_local(0))),
        args: Box::new([]),
        destination: place_local(1),
        target: Some(BlockId::from_u32(1)),
    });
    let body = BodyFixture::body_from_blocks(&[fn_ptr_ty, f.unit_ty], vec![call, return_block()]);
    f.push_function_with_body(body, vec![], f.unit_ty);
    let err = f
        .model
        .validate(&BridgeInput::matching(&f.model))
        .unwrap_err();
    assert!(matches!(
        err,
        InvariantViolation::CallArgsArityMismatch {
            expected: 1,
            actual: 0,
            ..
        }
    ));
}

#[test]
fn terminator_b5_call_direct_callee_not_function_rejects() {
    let mut f = BodyFixture::new();
    // Build a Static instance the call site references.
    let static_item = f.push_named_item(
        "GLOBAL",
        ItemKind::Static {
            mutability: Mutability::Not,
        },
    );
    let static_id = f.model.instances.push(Instance {
        item: static_item,
        generic_args: vec![],
        emission_name: StringId::ZERO,
        span: Span::dummy(),
        kind: InstanceKind::Static {
            ty: f.u32_ty,
            init: None,
        },
    });
    let call = block_with_terminator(TerminatorKind::Call {
        callee: CallCallee::Direct(static_id),
        args: Box::new([]),
        destination: place_local(0),
        target: Some(BlockId::from_u32(1)),
    });
    let body = BodyFixture::body_from_blocks(&[f.unit_ty], vec![call, return_block()]);
    f.push_function_with_body(body, vec![], f.unit_ty);
    let err = f
        .model
        .validate(&BridgeInput::matching(&f.model))
        .unwrap_err();
    assert!(matches!(
        err,
        InvariantViolation::CallDirectCalleeNotFunction { callee, .. } if callee == static_id
    ));
}

#[test]
fn terminator_b5_call_indirect_callee_not_fn_ptr_rejects() {
    let mut f = BodyFixture::new();
    // Indirect callee operand is u32, not a function pointer.
    let call = block_with_terminator(TerminatorKind::Call {
        callee: CallCallee::Indirect(Operand::Constant(f.u32_const)),
        args: Box::new([]),
        destination: place_local(0),
        target: Some(BlockId::from_u32(1)),
    });
    let body = BodyFixture::body_from_blocks(&[f.unit_ty], vec![call, return_block()]);
    f.push_function_with_body(body, vec![], f.unit_ty);
    let err = f
        .model
        .validate(&BridgeInput::matching(&f.model))
        .unwrap_err();
    assert!(matches!(
        err,
        InvariantViolation::CallIndirectCalleeNotFnPtr { operand_ty, .. } if operand_ty == f.u32_ty
    ));
}

#[test]
fn terminator_b6_call_arg_type_mismatch_rejects_direct() {
    let mut f = BodyFixture::new();
    let callee_sig = f.model.sigs.intern(FnSig {
        inputs: vec![f.u32_ty],
        output: f.unit_ty,
        abi: FnAbi::Rust,
        c_variadic: false,
        safety: Safety::Safe,
    });
    f.model.sig_c_abi_lowerings.push(None);
    let callee_item = f.push_named_item("callee", ItemKind::Function);
    let callee_id = f.model.instances.push(Instance {
        item: callee_item,
        generic_args: vec![],
        emission_name: StringId::ZERO,
        span: Span::dummy(),
        kind: InstanceKind::Function(FunctionInstanceData {
            fn_sig: callee_sig,
            body: Some(Body::default()),
            flavor: FunctionInstanceFlavor::Item,
            virtual_idx: None,
        }),
    });
    // Pass a bool to a u32-typed param -> type mismatch.
    let call = block_with_terminator(TerminatorKind::Call {
        callee: CallCallee::Direct(callee_id),
        args: Box::new([Operand::Constant(f.bool_const)]),
        destination: place_local(0),
        target: Some(BlockId::from_u32(1)),
    });
    let body = BodyFixture::body_from_blocks(&[f.unit_ty], vec![call, return_block()]);
    f.push_function_with_body(body, vec![], f.unit_ty);
    let err = f
        .model
        .validate(&BridgeInput::matching(&f.model))
        .unwrap_err();
    match err {
        InvariantViolation::CallArgTypeMismatch {
            idx,
            expected_ty,
            actual_ty,
            ..
        } => {
            assert_eq!(idx, 0);
            assert_eq!(expected_ty, f.u32_ty);
            assert_eq!(actual_ty, f.bool_ty);
        }
        other => panic!("expected CallArgTypeMismatch, got {other:?}"),
    }
}

#[test]
fn terminator_b3_call_destination_type_mismatch_rejects_indirect() {
    let mut f = BodyFixture::new();
    // Indirect callee whose recovered signature returns u32; the
    // destination is bool -- spec section 23.6 B-3 mismatch.
    let callee_sig = f.model.sigs.intern(FnSig {
        inputs: vec![],
        output: f.u32_ty,
        abi: FnAbi::Rust,
        c_variadic: false,
        safety: Safety::Safe,
    });
    f.model.sig_c_abi_lowerings.push(None);
    let fn_ptr_ty = f.model.types.intern(Ty::FnPtr(callee_sig));
    let call = block_with_terminator(TerminatorKind::Call {
        callee: CallCallee::Indirect(Operand::Copy(place_local(0))),
        args: Box::new([]),
        // local _1: bool != callee output u32.
        destination: place_local(1),
        target: Some(BlockId::from_u32(1)),
    });
    let body = BodyFixture::body_from_blocks(&[fn_ptr_ty, f.bool_ty], vec![call, return_block()]);
    f.push_function_with_body(body, vec![], f.unit_ty);
    let err = f
        .model
        .validate(&BridgeInput::matching(&f.model))
        .unwrap_err();
    match err {
        InvariantViolation::CallDestinationTypeMismatch {
            expected_ty,
            actual_ty,
            ..
        } => {
            assert_eq!(expected_ty, f.u32_ty);
            assert_eq!(actual_ty, f.bool_ty);
        }
        other => panic!("expected CallDestinationTypeMismatch, got {other:?}"),
    }
}

#[test]
fn terminator_b6_call_arg_type_mismatch_rejects_indirect() {
    let mut f = BodyFixture::new();
    // Indirect callee whose recovered signature requires a u32 input;
    // the test passes a bool constant -- spec section 23.6 B-6
    // mismatch.
    let callee_sig = f.model.sigs.intern(FnSig {
        inputs: vec![f.u32_ty],
        output: f.unit_ty,
        abi: FnAbi::Rust,
        c_variadic: false,
        safety: Safety::Safe,
    });
    f.model.sig_c_abi_lowerings.push(None);
    let fn_ptr_ty = f.model.types.intern(Ty::FnPtr(callee_sig));
    let call = block_with_terminator(TerminatorKind::Call {
        callee: CallCallee::Indirect(Operand::Copy(place_local(0))),
        args: Box::new([Operand::Constant(f.bool_const)]),
        destination: place_local(1),
        target: Some(BlockId::from_u32(1)),
    });
    let body = BodyFixture::body_from_blocks(&[fn_ptr_ty, f.unit_ty], vec![call, return_block()]);
    f.push_function_with_body(body, vec![], f.unit_ty);
    let err = f
        .model
        .validate(&BridgeInput::matching(&f.model))
        .unwrap_err();
    match err {
        InvariantViolation::CallArgTypeMismatch {
            idx,
            expected_ty,
            actual_ty,
            ..
        } => {
            assert_eq!(idx, 0);
            assert_eq!(expected_ty, f.u32_ty);
            assert_eq!(actual_ty, f.bool_ty);
        }
        other => panic!("expected CallArgTypeMismatch, got {other:?}"),
    }
}

#[test]
fn terminator_b8_virtual_idx_inconsistent_rejects() {
    let mut f = BodyFixture::new();
    // Use a u32-returning callee signature so the interner key is
    // distinct from the unit-returning outer-function signature added
    // by `push_function_with_body`.
    let callee_sig = f.model.sigs.intern(FnSig {
        inputs: vec![],
        output: f.u32_ty,
        abi: FnAbi::Rust,
        c_variadic: false,
        safety: Safety::Safe,
    });
    f.model.sig_c_abi_lowerings.push(None);
    let callee_item = f.push_named_item("callee", ItemKind::Function);
    // Virtual flavor but virtual_idx is None.
    let callee_id = f.model.instances.push(Instance {
        item: callee_item,
        generic_args: vec![],
        emission_name: StringId::ZERO,
        span: Span::dummy(),
        kind: InstanceKind::Function(FunctionInstanceData {
            fn_sig: callee_sig,
            body: Some(Body::default()),
            flavor: FunctionInstanceFlavor::Virtual,
            virtual_idx: None,
        }),
    });
    let call = block_with_terminator(TerminatorKind::Call {
        callee: CallCallee::Direct(callee_id),
        args: Box::new([]),
        destination: place_local(0),
        target: Some(BlockId::from_u32(1)),
    });
    let body = BodyFixture::body_from_blocks(&[f.unit_ty], vec![call, return_block()]);
    f.push_function_with_body(body, vec![], f.unit_ty);
    let err = f
        .model
        .validate(&BridgeInput::matching(&f.model))
        .unwrap_err();
    match err {
        InvariantViolation::CallVirtualIdxInconsistent { callee, .. } => {
            assert_eq!(callee, callee_id);
        }
        other => panic!("expected CallVirtualIdxInconsistent, got {other:?}"),
    }
}

#[test]
fn terminator_b9_target_divergence_mismatch_rejects_non_diverging_no_target() {
    let mut f = BodyFixture::new();
    // Use a u32-returning signature so it doesn't collide with the
    // unit-returning outer-function signature in push_function_with_body.
    let callee_sig = f.model.sigs.intern(FnSig {
        inputs: vec![],
        output: f.u32_ty,
        abi: FnAbi::Rust,
        c_variadic: false,
        safety: Safety::Safe,
    });
    f.model.sig_c_abi_lowerings.push(None);
    let callee_item = f.push_named_item("callee", ItemKind::Function);
    let callee_id = f.model.instances.push(Instance {
        item: callee_item,
        generic_args: vec![],
        emission_name: StringId::ZERO,
        span: Span::dummy(),
        kind: InstanceKind::Function(FunctionInstanceData {
            fn_sig: callee_sig,
            body: Some(Body::default()),
            flavor: FunctionInstanceFlavor::Item,
            virtual_idx: None,
        }),
    });
    let call = block_with_terminator(TerminatorKind::Call {
        callee: CallCallee::Direct(callee_id),
        args: Box::new([]),
        destination: place_local(0),
        target: None,
    });
    let body = BodyFixture::body_from_blocks(&[f.unit_ty], vec![call]);
    f.push_function_with_body(body, vec![], f.unit_ty);
    let err = f
        .model
        .validate(&BridgeInput::matching(&f.model))
        .unwrap_err();
    match err {
        InvariantViolation::CallTargetDivergenceMismatch { .. } => {}
        other => panic!("expected CallTargetDivergenceMismatch, got {other:?}"),
    }
}

#[test]
fn statement_b1_assign_malformed_rejects_ptr_metadata_on_non_fat_pointer() {
    // `PtrMetadata` applied to a non-fat-pointer operand cannot
    // recover a metadata type. body_ty's rvalue_ty returns
    // `Malformed`; the statement-site B-1 validator must surface
    // `StatementAssignMalformed` rather than silently skipping.
    let mut f = BodyFixture::new();
    let usize_ty = f.model.types.intern(Ty::Uint(UintKind::Usize));
    let stmt = Statement {
        kind: StatementKind::Assign(
            place_local(1),
            // PtrMetadata on a u32 operand has no recoverable
            // metadata pointee.
            Rvalue::UnaryOp(UnOp::PtrMetadata, Operand::Copy(place_local(0))),
        ),
        span: Span::dummy(),
    };
    let body = BodyFixture::body_from_blocks(
        &[f.u32_ty, usize_ty],
        vec![block_with_statements(vec![stmt])],
    );
    f.push_function_with_body(body, vec![], f.unit_ty);
    let err = f
        .model
        .validate(&BridgeInput::matching(&f.model))
        .unwrap_err();
    match err {
        InvariantViolation::StatementAssignMalformed { side, .. } => {
            assert_eq!(side, "rvalue");
        }
        other => panic!("expected StatementAssignMalformed, got {other:?}"),
    }
}

#[test]
fn body_ty_closure_rejects_dangling_const_id_in_operand() {
    // A dangling `Operand::Constant(ConstId)` is rejected by
    // body_ty_closure_operand with a `DanglingId` referencing the
    // operand-context ConstId.
    let mut f = BodyFixture::new();
    let dangling_const = ConstId::from_u32(f.model.consts.len() as u32 + 99);
    let stmt = Statement {
        kind: StatementKind::Assign(
            place_local(0),
            Rvalue::Use(Operand::Constant(dangling_const)),
        ),
        span: Span::dummy(),
    };
    let body = BodyFixture::body_from_blocks(&[f.u32_ty], vec![block_with_statements(vec![stmt])]);
    f.push_function_with_body(body, vec![], f.unit_ty);
    let err = f
        .model
        .validate(&BridgeInput::matching(&f.model))
        .unwrap_err();
    assert!(matches!(
        err,
        InvariantViolation::DanglingId { field, .. }
            if field == "Operand::Constant.ConstId"
    ));
}

#[test]
fn terminator_b9_target_divergence_mismatch_rejects_indirect_divergent_target_some() {
    let mut f = BodyFixture::new();
    let never_ty = f.model.types.intern(Ty::Never);
    // Indirect callee whose recovered signature returns Never; the
    // Call carries a Some(_) target -- spec section 23.6 B-9
    // mismatch covering the Indirect side.
    let callee_sig = f.model.sigs.intern(FnSig {
        inputs: vec![],
        output: never_ty,
        abi: FnAbi::Rust,
        c_variadic: false,
        safety: Safety::Safe,
    });
    f.model.sig_c_abi_lowerings.push(None);
    let fn_ptr_ty = f.model.types.intern(Ty::FnPtr(callee_sig));
    let call = block_with_terminator(TerminatorKind::Call {
        callee: CallCallee::Indirect(Operand::Copy(place_local(0))),
        args: Box::new([]),
        destination: place_local(1),
        target: Some(BlockId::from_u32(1)),
    });
    let body = BodyFixture::body_from_blocks(&[fn_ptr_ty, f.unit_ty], vec![call, return_block()]);
    f.push_function_with_body(body, vec![], f.unit_ty);
    let err = f
        .model
        .validate(&BridgeInput::matching(&f.model))
        .unwrap_err();
    match err {
        InvariantViolation::CallTargetDivergenceMismatch { .. } => {}
        other => panic!("expected CallTargetDivergenceMismatch, got {other:?}"),
    }
}

#[test]
fn terminator_b7_direct_callee_out_of_range_rejects() {
    // Spec section 23.6.2 B-7: every `Direct(InstanceId)` referenced
    // from any Terminator::Call resolves in `model.instances`.
    let mut f = BodyFixture::new();
    let call = block_with_terminator(TerminatorKind::Call {
        callee: CallCallee::Direct(regseq_model::InstanceId::from_u32(999)),
        args: Box::new([]),
        destination: place_local(0),
        target: Some(BlockId::from_u32(1)),
    });
    let body = BodyFixture::body_from_blocks(&[f.unit_ty], vec![call, return_block()]);
    f.push_function_with_body(body, vec![], f.unit_ty);
    let err = f
        .model
        .validate(&BridgeInput::matching(&f.model))
        .unwrap_err();
    match err {
        InvariantViolation::DirectCalleeOutOfRange { callee, .. } => {
            assert_eq!(callee, regseq_model::InstanceId::from_u32(999));
        }
        other => panic!("expected DirectCalleeOutOfRange, got {other:?}"),
    }
}

#[test]
fn body_ty_closure_rejects_dangling_local_decl_ty() {
    let mut f = BodyFixture::new();
    let dangling = TyId::from_u32(f.model.types.len() as u32 + 99);
    let body = BodyFixture::body_from_blocks(&[dangling], vec![return_block()]);
    f.push_function_with_body(body, vec![], f.unit_ty);
    let err = f
        .model
        .validate(&BridgeInput::matching(&f.model))
        .unwrap_err();
    assert!(matches!(
        err,
        InvariantViolation::BodyTyIdOutOfRange {
            site: "LocalDecl.ty",
            ..
        }
    ));
}

#[test]
fn place_validator_structural_error_at_index_5_not_masked_by_type_error_at_index_0() {
    // Construct a place whose projection chain has:
    //   index 0: Field(0, wrong_ty) -- type mismatch
    //   index 5: Deref -- structural violation (Deref must be first)
    // The audit-mandated structural pre-pass must report the Deref
    // (structural) error, not the field (type-narrowing) error.
    let mut f = BodyFixture::new();
    let struct_item = f.push_named_item("S", ItemKind::Struct);
    let struct_layout = Layout {
        size: 4,
        align: 4,
        pack: None,
        uninhabited: false,
        kind: LayoutKind::Struct {
            fields: vec![regseq_model::FieldLayout {
                name: f.model.strings.intern("x"),
                ty: f.u32_ty,
                offset: 0,
            }],
        },
    };
    let struct_ty = f.model.types.intern(Ty::Struct {
        item: struct_item,
        args: vec![],
        layout: struct_layout,
    });
    // Wrong field type on the projection (claims bool when layout
    // says u32) -- type error at index 0.
    let bad_field_ty = f.bool_ty;
    let projection: Box<[PlaceElem]> = Box::new([
        PlaceElem::Field(FieldIdx::from_u32(0), bad_field_ty),
        PlaceElem::Field(FieldIdx::from_u32(0), f.u32_ty),
        PlaceElem::Field(FieldIdx::from_u32(0), f.u32_ty),
        PlaceElem::Field(FieldIdx::from_u32(0), f.u32_ty),
        PlaceElem::Field(FieldIdx::from_u32(0), f.u32_ty),
        // Structural violation: Deref past index 0.
        PlaceElem::Deref,
    ]);
    let bad_place = Place {
        local: LocalId::from_u32(0),
        projection,
    };
    let stmt = Statement {
        kind: StatementKind::Assign(bad_place, Rvalue::Use(Operand::Constant(f.u32_const))),
        span: Span::dummy(),
    };
    let body = BodyFixture::body_from_blocks(&[struct_ty], vec![block_with_statements(vec![stmt])]);
    f.push_function_with_body(body, vec![], f.unit_ty);
    let err = f
        .model
        .validate(&BridgeInput::matching(&f.model))
        .unwrap_err();
    match err {
        InvariantViolation::PlaceWalk { error, .. } => match error.kind {
            regseq_model::PlaceValidationErrorKind::DerefNotFirst { index } => {
                assert_eq!(index, 5);
            }
            other => panic!("expected DerefNotFirst structural error, got {other:?}"),
        },
        other => panic!("expected PlaceWalk, got {other:?}"),
    }
}

#[test]
fn rvalue_ty_ptr_metadata_on_dyn_returns_vtable_ptr() {
    // Build:
    //   _0: *const dyn Trait (fat pointer)
    //   _1: *const VTableStruct (the expected metadata type)
    // Assign _1 = PtrMetadata(Copy(_0)) -- should NOT mismatch.
    let mut f = BodyFixture::new();
    let vtable_item = f.push_named_item("VT", ItemKind::Struct);
    let vtable_layout = Layout {
        size: 8,
        align: 8,
        pack: None,
        uninhabited: false,
        kind: LayoutKind::Struct { fields: vec![] },
    };
    let vtable_ty = f.model.types.intern(Ty::Struct {
        item: vtable_item,
        args: vec![],
        layout: vtable_layout,
    });
    let dynamic_ty = f.model.types.intern(Ty::Dynamic {
        auto_traits: vec![],
        vtable_ty,
    });
    let ptr_dyn = f.model.types.intern(Ty::Ptr(dynamic_ty, Mutability::Not));
    let ptr_vtable = f.model.types.intern(Ty::Ptr(vtable_ty, Mutability::Not));
    let stmt = Statement {
        kind: StatementKind::Assign(
            place_local(1),
            Rvalue::UnaryOp(UnOp::PtrMetadata, Operand::Copy(place_local(0))),
        ),
        span: Span::dummy(),
    };
    let body = BodyFixture::body_from_blocks(
        &[ptr_dyn, ptr_vtable],
        vec![block_with_statements(vec![stmt])],
    );
    f.push_function_with_body(body, vec![], f.unit_ty);
    let result = f.model.validate(&BridgeInput::matching(&f.model));
    assert_eq!(result, Ok(()));
}

#[test]
fn rvalue_ty_ptr_metadata_on_slice_returns_usize() {
    // Build:
    //   _0: *const [u32]
    //   _1: usize
    // Assign _1 = PtrMetadata(Copy(_0)) -- should NOT mismatch.
    let mut f = BodyFixture::new();
    let usize_ty = f.model.types.intern(Ty::Uint(UintKind::Usize));
    let slice_u32 = f.model.types.intern(Ty::Slice(f.u32_ty));
    let ptr_slice = f.model.types.intern(Ty::Ptr(slice_u32, Mutability::Not));
    let stmt = Statement {
        kind: StatementKind::Assign(
            place_local(1),
            Rvalue::UnaryOp(UnOp::PtrMetadata, Operand::Copy(place_local(0))),
        ),
        span: Span::dummy(),
    };
    let body = BodyFixture::body_from_blocks(
        &[ptr_slice, usize_ty],
        vec![block_with_statements(vec![stmt])],
    );
    f.push_function_with_body(body, vec![], f.unit_ty);
    let result = f.model.validate(&BridgeInput::matching(&f.model));
    assert_eq!(result, Ok(()));
}

#[test]
fn invariant_2_pass_full_alignment() {
    // items / item_def_paths / item_synthetic_kinds all aligned.
    let mut model = base_model();
    push_item(&mut model, "a", ItemKind::Function, 1);
    push_item(&mut model, "b", ItemKind::Function, 2);
    assert_eq!(validate(&mut model), Ok(()));
}

#[test]
fn invariant_6_pass_full_closure() {
    let mut model = base_model();
    push_item(&mut model, "driver", ItemKind::Function, 1);
    assert_eq!(validate(&mut model), Ok(()));
}

#[test]
fn invariant_18_pass_unique_def_path_hashes() {
    let mut model = base_model();
    push_item(&mut model, "a", ItemKind::Function, 1);
    push_item(&mut model, "b", ItemKind::Function, 2);
    push_item(&mut model, "c", ItemKind::Function, 3);
    assert_eq!(validate(&mut model), Ok(()));
}

// ----------------------------------------------------------------------
// regseq-driver F-a: Section-7 validator-coverage completions (driver-78
// audit §2/§3). VL-021 (no test), VL-045 src/dst/pointee arms (partial ->
// complete). VL-041 side="place" is probed below.
// ----------------------------------------------------------------------

fn place_with_projection(local: u32, projection: Vec<PlaceElem>) -> Place {
    Place {
        local: LocalId::from_u32(local),
        projection: projection.into_boxed_slice(),
    }
}

#[test]
fn vl021_synthetic_kind_function_with_closure_captures_rejected() {
    // VL-021 `SyntheticKindItemKindMismatch`: ClosureCaptures is a
    // synthesised capture STRUCT, so attaching it to an ItemKind::Function
    // item is inconsistent. `item_synthetic_kinds` is a pub field.
    let mut f = BodyFixture::new();
    // A complete valid driver instance so the bucket/referential
    // validators have nothing else to reject.
    let body = BodyFixture::body_from_blocks(&[], vec![return_block()]);
    f.push_function_with_body(body, vec![], f.unit_ty);
    // Inject the mismatch on a fresh Function item carrying a
    // ClosureCaptures synthetic kind (valid only on a Struct).
    let item = f.push_named_item("captures_on_function", ItemKind::Function);
    f.model.item_synthetic_kinds[item] = SyntheticItemKind::ClosureCaptures {
        closure: hash(0xC0FFEE),
    };
    match validate(&mut f.model).unwrap_err() {
        InvariantViolation::SyntheticKindItemKindMismatch { item: got, .. } => {
            assert_eq!(got, item);
        }
        other => panic!("expected SyntheticKindItemKindMismatch, got {other:?}"),
    }
}

#[test]
fn vl045_copy_non_overlapping_src_not_pointer_rejects() {
    let mut f = BodyFixture::new();
    let ptr_u32 = f.model.types.intern(Ty::Ptr(f.u32_ty, Mutability::Mut));
    // The `count` operand recovers its type via the const interner's
    // `find(&Ty::Uint(Usize))`, so the usize TYPE must be interned (else
    // `expect_operand_ty(count)` is NotApplicable -> the count arm fires
    // before the src/dst/pointee checks under test).
    f.model.types.intern(Ty::Uint(UintKind::Usize));
    let usize_const = f.model.consts.intern(Const::Uint(UintKind::Usize, 0));
    // _0: u32 (non-pointer src); _1: *mut u32 (dst). count: usize.
    let stmt = Statement {
        kind: StatementKind::Intrinsic(NonDivergingIntrinsic::CopyNonOverlapping(
            CopyNonOverlapping {
                src: Operand::Copy(place_local(0)),
                dst: Operand::Copy(place_local(1)),
                count: Operand::Constant(usize_const),
            },
        )),
        span: Span::dummy(),
    };
    let body = BodyFixture::body_from_blocks(
        &[f.u32_ty, ptr_u32],
        vec![block_with_statements(vec![stmt])],
    );
    f.push_function_with_body(body, vec![], f.unit_ty);
    assert!(matches!(
        validate(&mut f.model).unwrap_err(),
        InvariantViolation::CopyNonOverlappingOperandType { which: "src", .. }
    ));
}

#[test]
fn vl045_copy_non_overlapping_dst_not_pointer_rejects() {
    let mut f = BodyFixture::new();
    let ptr_u32 = f.model.types.intern(Ty::Ptr(f.u32_ty, Mutability::Mut));
    // The `count` operand recovers its type via the const interner's
    // `find(&Ty::Uint(Usize))`, so the usize TYPE must be interned (else
    // `expect_operand_ty(count)` is NotApplicable -> the count arm fires
    // before the src/dst/pointee checks under test).
    f.model.types.intern(Ty::Uint(UintKind::Usize));
    let usize_const = f.model.consts.intern(Const::Uint(UintKind::Usize, 0));
    // _0: *mut u32 (pointer src); _1: u32 (non-pointer dst). count: usize.
    let stmt = Statement {
        kind: StatementKind::Intrinsic(NonDivergingIntrinsic::CopyNonOverlapping(
            CopyNonOverlapping {
                src: Operand::Copy(place_local(0)),
                dst: Operand::Copy(place_local(1)),
                count: Operand::Constant(usize_const),
            },
        )),
        span: Span::dummy(),
    };
    let body = BodyFixture::body_from_blocks(
        &[ptr_u32, f.u32_ty],
        vec![block_with_statements(vec![stmt])],
    );
    f.push_function_with_body(body, vec![], f.unit_ty);
    assert!(matches!(
        validate(&mut f.model).unwrap_err(),
        InvariantViolation::CopyNonOverlappingOperandType { which: "dst", .. }
    ));
}

#[test]
fn vl045_copy_non_overlapping_pointee_mismatch_rejects() {
    let mut f = BodyFixture::new();
    let ptr_u32 = f.model.types.intern(Ty::Ptr(f.u32_ty, Mutability::Mut));
    let ptr_bool = f.model.types.intern(Ty::Ptr(f.bool_ty, Mutability::Mut));
    // The `count` operand recovers its type via the const interner's
    // `find(&Ty::Uint(Usize))`, so the usize TYPE must be interned (else
    // `expect_operand_ty(count)` is NotApplicable -> the count arm fires
    // before the src/dst/pointee checks under test).
    f.model.types.intern(Ty::Uint(UintKind::Usize));
    let usize_const = f.model.consts.intern(Const::Uint(UintKind::Usize, 0));
    // _0: *mut u32, _1: *mut bool -> pointees disagree. count: usize.
    let stmt = Statement {
        kind: StatementKind::Intrinsic(NonDivergingIntrinsic::CopyNonOverlapping(
            CopyNonOverlapping {
                src: Operand::Copy(place_local(0)),
                dst: Operand::Copy(place_local(1)),
                count: Operand::Constant(usize_const),
            },
        )),
        span: Span::dummy(),
    };
    let body = BodyFixture::body_from_blocks(
        &[ptr_u32, ptr_bool],
        vec![block_with_statements(vec![stmt])],
    );
    f.push_function_with_body(body, vec![], f.unit_ty);
    assert!(matches!(
        validate(&mut f.model).unwrap_err(),
        InvariantViolation::CopyNonOverlappingOperandType {
            which: "pointee",
            ..
        }
    ));
}

#[test]
fn vl041_assign_malformed_place_side_is_defensively_shadowed() {
    // VL-041 `StatementAssignMalformed { side: "place" }` is a DEFENSIVE
    // backstop, not reachable from a structurally-valid place: any place
    // whose `place_ty` is Malformed (e.g. a `[Deref]` on a non-pointer
    // local) is rejected FIRST by the structural `validate_body_places`
    // pass (model.rs runs it before `validate_body_statement_invariants`),
    // surfacing `PlaceWalk { DerefOnNonPointer }` rather than the
    // statement-site `side="place"` arm. The `body_ty.rs::place_ty` doc
    // (lines 63-66) states this explicitly: the structural pre-pass owns
    // the malformed-place rejection; the statement arm re-checks
    // defensively. This mirrors VL-018/019 (other unreachable-by-
    // construction backstops). The reachable `side="rvalue"` arm is
    // covered by `statement_b1_assign_malformed_rejects_ptr_metadata_on_non_fat_pointer`.
    let mut f = BodyFixture::new();
    let dest = place_with_projection(0, vec![PlaceElem::Deref]);
    let stmt = Statement {
        kind: StatementKind::Assign(dest, Rvalue::Use(Operand::Constant(f.u32_const))),
        span: Span::dummy(),
    };
    let body = BodyFixture::body_from_blocks(&[f.u32_ty], vec![block_with_statements(vec![stmt])]);
    f.push_function_with_body(body, vec![], f.unit_ty);
    // The structural place validator catches the malformed place first.
    assert!(matches!(
        validate(&mut f.model).unwrap_err(),
        InvariantViolation::PlaceWalk { .. }
    ));
}
