//! Integration tests for the P1a-VAL-03 layout / §3.3 validators.

use std::sync::Arc;

use regseq_model::{
    ArgsKey, AttrData, BridgeInput, CAbiLowering, CAbiShape, ClosureCallKind, Const, ConstInterner,
    ContentHash, CrateId, CrateInfo, DefPath, DefPathData, DefPathHash, DefPathSegment, Endian,
    EnumShape, FieldIdx, FieldLayout, FieldPath, FloatKind, FnAbi, FnSig, FunctionInstanceData,
    FunctionInstanceFlavor, GenericArg, IndexVec, InlineHint, Instance, InstanceKind, IntKind,
    IntrinsicKind, InvariantViolation, Item, ItemId, ItemKind, Layout, LayoutKind, Linkage,
    LogLevel, PrincipalTraitKey, ProjectionBindingsKey, Provenance, RegseqModel, Safety,
    SourceFile, Span, StableCrateId, StringId, SubstReason, SyntheticItemKind, TagEncoding,
    TagScalar, Ty, TyId, UintKind, VariantIdx, VariantLayout, VariantRange, WrappingRange,
    UNIT_LAYOUT,
};

fn validate(model: &mut RegseqModel) -> Result<(), InvariantViolation> {
    model.validate(&BridgeInput::matching(model))
}

fn hash(local: u64) -> DefPathHash {
    DefPathHash {
        stable_crate_id: StableCrateId(1),
        local_hash: local,
    }
}

fn synth_hash(local: u64) -> DefPathHash {
    DefPathHash {
        stable_crate_id: StableCrateId(0),
        local_hash: local,
    }
}

fn def_path(model: &mut RegseqModel, name: &str) -> DefPath {
    let name_id = model.strings.intern(name);
    let crate_name = model
        .strings
        .resolve(model.crates[model.primary_crate].name)
        .to_owned();
    let rendered = model.strings.intern(&format!("{crate_name}::{name}"));
    DefPath {
        crate_id: model.primary_crate,
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

fn make_item(model: &mut RegseqModel, name: &str, kind: ItemKind) -> Item {
    Item {
        def_path: def_path(model, name),
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

fn push_item(model: &mut RegseqModel, name: &str, kind: ItemKind, local: u64) -> ItemId {
    let item = make_item(model, name, kind);
    let id = model.items.push(item);
    model.item_def_paths.push(hash(local));
    model.item_synthetic_kinds.push(SyntheticItemKind::None);
    model.domain_attrs.item_attrs.push(Vec::new());
    id
}

fn push_synthetic_item(
    model: &mut RegseqModel,
    name: &str,
    kind: ItemKind,
    local: u64,
    synth: SyntheticItemKind,
) -> ItemId {
    let item = make_item(model, name, kind);
    let id = model.items.push(item);
    let hash = if matches!(synth, SyntheticItemKind::ClosureCaptures { .. }) {
        hash(local)
    } else {
        synth_hash(local)
    };
    model.item_def_paths.push(hash);
    model.item_synthetic_kinds.push(synth);
    model.domain_attrs.item_attrs.push(Vec::new());
    id
}

fn base_model() -> RegseqModel {
    let mut model = RegseqModel::with_primary("spec_crate", StableCrateId(1));
    model.types.intern(Ty::Bool);
    model
}

fn unit_tuple_ty(model: &mut RegseqModel) -> TyId {
    model.types.intern(Ty::Tuple {
        fields: vec![],
        layout: UNIT_LAYOUT,
    })
}

// -----------------------------------------------------------------------------
// Invariant 14 -- InstanceKind layout consistency
// -----------------------------------------------------------------------------

fn struct_layout(size: u64, align: u64, fields: Vec<FieldLayout>) -> Layout {
    Layout {
        size,
        align,
        pack: None,
        uninhabited: false,
        kind: LayoutKind::Struct { fields },
    }
}

fn empty_enum_layout() -> Layout {
    Layout {
        size: 0,
        align: 1,
        pack: None,
        uninhabited: false,
        kind: LayoutKind::Enum {
            shape: EnumShape::Empty,
        },
    }
}

fn union_layout() -> Layout {
    Layout {
        size: 4,
        align: 4,
        pack: None,
        uninhabited: false,
        kind: LayoutKind::Union { fields: vec![] },
    }
}

#[test]
fn invariant_14_struct_pass() {
    let mut model = base_model();
    let item = push_item(&mut model, "handle", ItemKind::Struct, 1);
    let ty = model.types.intern(Ty::Struct {
        item,
        args: vec![],
        layout: struct_layout(4, 4, vec![]),
    });
    model.instances.push(Instance {
        item,
        generic_args: vec![],
        emission_name: StringId::ZERO,
        span: Span::dummy(),
        kind: InstanceKind::Struct { layout: ty },
    });
    assert_eq!(validate(&mut model), Ok(()));
}

#[test]
fn invariant_14_enum_pass() {
    let mut model = base_model();
    let item = push_item(&mut model, "color", ItemKind::Enum, 1);
    let ty = model.types.intern(Ty::Enum {
        item,
        args: vec![],
        layout: empty_enum_layout(),
    });
    model.instances.push(Instance {
        item,
        generic_args: vec![],
        emission_name: StringId::ZERO,
        span: Span::dummy(),
        kind: InstanceKind::Enum { layout: ty },
    });
    assert_eq!(validate(&mut model), Ok(()));
}

#[test]
fn invariant_14_union_pass() {
    let mut model = base_model();
    let item = push_item(&mut model, "punner", ItemKind::Union, 1);
    let ty = model.types.intern(Ty::Union {
        item,
        args: vec![],
        layout: union_layout(),
    });
    model.instances.push(Instance {
        item,
        generic_args: vec![],
        emission_name: StringId::ZERO,
        span: Span::dummy(),
        kind: InstanceKind::Union { layout: ty },
    });
    assert_eq!(validate(&mut model), Ok(()));
}

#[test]
fn invariant_14_reject_kind_mismatch_struct_to_enum() {
    let mut model = base_model();
    let item = push_item(&mut model, "color", ItemKind::Enum, 1);
    let ty = model.types.intern(Ty::Enum {
        item,
        args: vec![],
        layout: empty_enum_layout(),
    });
    model.instances.push(Instance {
        item,
        generic_args: vec![],
        emission_name: StringId::ZERO,
        span: Span::dummy(),
        kind: InstanceKind::Struct { layout: ty },
    });
    assert!(matches!(
        validate(&mut model),
        Err(InvariantViolation::InstanceLayoutMismatch {
            expected_kind: "Ty::Struct",
            found_kind: "Ty::Enum",
            ..
        })
    ));
}

#[test]
fn invariant_14_reject_item_args_mismatch() {
    let mut model = base_model();
    let item = push_item(&mut model, "handle", ItemKind::Struct, 1);
    let other_item = push_item(&mut model, "other", ItemKind::Struct, 2);
    let ty = model.types.intern(Ty::Struct {
        item: other_item,
        args: vec![],
        layout: struct_layout(4, 4, vec![]),
    });
    model.instances.push(Instance {
        item,
        generic_args: vec![],
        emission_name: StringId::ZERO,
        span: Span::dummy(),
        kind: InstanceKind::Struct { layout: ty },
    });
    assert!(matches!(
        validate(&mut model),
        Err(InvariantViolation::InstanceLayoutItemArgsMismatch { .. })
    ));
}

// -----------------------------------------------------------------------------
// Invariant 19 -- opaque_handle_eligible
// -----------------------------------------------------------------------------

#[test]
fn invariant_19_pass_zst_non_generic_ref_only() {
    let mut model = base_model();
    let item = push_item(&mut model, "handle", ItemKind::Struct, 1);
    model.items[item].opaque_handle_eligible = true;
    let mono = model.types.intern(Ty::Struct {
        item,
        args: vec![],
        layout: struct_layout(0, 1, vec![]),
    });
    model.types.intern(Ty::Ref(mono, Provenance::SharedBorrow));
    assert_eq!(validate(&mut model), Ok(()));
}

#[test]
fn invariant_19_reject_non_struct_kind() {
    let mut model = base_model();
    let item = push_item(&mut model, "func", ItemKind::Function, 1);
    model.items[item].opaque_handle_eligible = true;
    assert!(matches!(
        validate(&mut model),
        Err(InvariantViolation::OpaqueHandleInconsistent { .. })
    ));
}

#[test]
fn invariant_19_reject_generic() {
    let mut model = base_model();
    let bool_ty = model.types.intern(Ty::Char);
    let item = push_item(&mut model, "handle", ItemKind::Struct, 1);
    model.items[item].opaque_handle_eligible = true;
    let _generic = model.types.intern(Ty::Struct {
        item,
        args: vec![GenericArg::Type(bool_ty)],
        layout: struct_layout(0, 1, vec![]),
    });
    assert!(matches!(
        validate(&mut model),
        Err(InvariantViolation::OpaqueHandleInconsistent { reason, .. })
            if reason.contains("exactly one") || reason.contains("non-generic")
    ));
}

#[test]
fn invariant_19_reject_non_zst_layout() {
    let mut model = base_model();
    let item = push_item(&mut model, "handle", ItemKind::Struct, 1);
    model.items[item].opaque_handle_eligible = true;
    model.types.intern(Ty::Struct {
        item,
        args: vec![],
        layout: struct_layout(4, 4, vec![]),
    });
    assert!(matches!(
        validate(&mut model),
        Err(InvariantViolation::OpaqueHandleInconsistent { reason, .. })
            if reason.contains("ZST")
    ));
}

#[test]
fn invariant_19_reject_by_value_use() {
    let mut model = base_model();
    let container = push_item(&mut model, "container", ItemKind::Struct, 2);
    let handle = push_item(&mut model, "handle", ItemKind::Struct, 1);
    model.items[handle].opaque_handle_eligible = true;
    let mono = model.types.intern(Ty::Struct {
        item: handle,
        args: vec![],
        layout: struct_layout(0, 1, vec![]),
    });
    let name = model.strings.intern("inner");
    let container_layout = struct_layout(
        0,
        1,
        vec![FieldLayout {
            name,
            ty: mono,
            offset: 0,
        }],
    );
    model.types.intern(Ty::Struct {
        item: container,
        args: vec![],
        layout: container_layout,
    });
    let result = validate(&mut model);
    assert!(
        matches!(
            &result,
            Err(InvariantViolation::OpaqueHandleInconsistent { reason, .. })
                if reason.contains("Ref/Ptr")
        ),
        "unexpected result: {result:?}"
    );
}

// -----------------------------------------------------------------------------
// Invariant 20 -- StableCrateId(0) enforcement
// -----------------------------------------------------------------------------

#[test]
fn invariant_20_pass() {
    let mut model = base_model();
    push_synthetic_item(
        &mut model,
        "closure_captures_item",
        ItemKind::Struct,
        8,
        SyntheticItemKind::ClosureCaptures { closure: hash(8) },
    );
    push_item(&mut model, "source_item", ItemKind::Function, 1);
    push_synthetic_item(
        &mut model,
        "synthetic_item",
        ItemKind::Function,
        7,
        SyntheticItemKind::DropGlue {
            self_ty: ArgsKey(7),
        },
    );
    assert_eq!(validate(&mut model), Ok(()));
}

#[test]
fn invariant_20_reject_real_crate_zero() {
    let mut model = base_model();
    let alt_name = model.strings.intern("aux");
    model.crates.push(CrateInfo {
        name: alt_name,
        stable_crate_id: StableCrateId(0),
    });
    assert!(matches!(
        validate(&mut model),
        Err(InvariantViolation::StableCrateIdZeroCollision { .. })
    ));
}

#[test]
fn invariant_20_reject_source_derived_synthetic_domain() {
    let mut model = base_model();
    let id = push_item(&mut model, "regular", ItemKind::Function, 1);
    model.item_def_paths[id] = synth_hash(99);
    assert!(matches!(
        validate(&mut model),
        Err(InvariantViolation::SyntheticDomainBoundary { reason, .. })
            if reason.contains("source-derived")
    ));
}

#[test]
fn invariant_20_reject_synthetic_non_zero_domain() {
    let mut model = base_model();
    let id = push_item(&mut model, "drop_glue", ItemKind::Function, 1);
    model.item_synthetic_kinds[id] = SyntheticItemKind::DropGlue {
        self_ty: ArgsKey(7),
    };
    assert!(matches!(
        validate(&mut model),
        Err(InvariantViolation::SyntheticDomainBoundary { reason, .. })
            if reason.contains("synthetic")
    ));
}

// -----------------------------------------------------------------------------
// Canonical UNIT_LAYOUT check
// -----------------------------------------------------------------------------

#[test]
fn unit_layout_pass() {
    let mut model = base_model();
    unit_tuple_ty(&mut model);
    assert_eq!(validate(&mut model), Ok(()));
}

#[test]
fn unit_layout_reject_non_canonical() {
    let mut model = base_model();
    model.types.intern(Ty::Tuple {
        fields: vec![],
        layout: struct_layout(4, 1, vec![]),
    });
    assert!(matches!(
        validate(&mut model),
        Err(InvariantViolation::UnitLayoutMismatch { .. })
    ));
}

// -----------------------------------------------------------------------------
// Target / build well-formedness (invariants 15 / 17)
// -----------------------------------------------------------------------------

#[test]
fn target_pass() {
    let mut model = base_model();
    assert_eq!(validate(&mut model), Ok(()));
}

#[test]
fn target_reject_pointer_width_invalid() {
    let mut model = base_model();
    model.target.pointer_width = 24;
    assert!(matches!(
        validate(&mut model),
        Err(InvariantViolation::TargetMalformed { reason })
            if reason.contains("pointer_width")
    ));
}

#[test]
fn target_reject_endian_disagrees() {
    let mut model = base_model();
    model.target.endian = Endian::Big;
    assert!(matches!(
        validate(&mut model),
        Err(InvariantViolation::TargetMalformed { reason })
            if reason.contains("endian")
    ));
}

#[test]
fn target_reject_scalar_layouts_missing() {
    let mut model = base_model();
    model.target.scalar_layouts.char_ = regseq_model::SizeAlign { size: 0, align: 0 };
    assert!(matches!(
        validate(&mut model),
        Err(InvariantViolation::TargetMalformed { reason })
            if reason.contains("char_")
    ));
}

#[test]
fn target_reject_scalar_layouts_non_power_of_two_alignment() {
    let mut model = base_model();
    model.target.scalar_layouts.char_ = regseq_model::SizeAlign { size: 4, align: 3 };
    assert!(matches!(
        validate(&mut model),
        Err(InvariantViolation::TargetMalformed { reason })
            if reason.contains("power of two")
    ));
}

#[test]
fn target_reject_scalar_layouts_wrong_int_width() {
    let mut model = base_model();
    model.target.scalar_layouts.int[2] = regseq_model::SizeAlign { size: 8, align: 8 };
    assert!(matches!(
        validate(&mut model),
        Err(InvariantViolation::TargetMalformed { reason })
            if reason.contains("IntKind")
    ));
}

#[test]
fn target_reject_scalar_layouts_oversized_alignment() {
    let mut model = base_model();
    model.target.scalar_layouts.int[2] = regseq_model::SizeAlign { size: 4, align: 8 };
    assert!(matches!(
        validate(&mut model),
        Err(InvariantViolation::TargetMalformed { reason })
            if reason.contains("alignment exceeds")
    ));
}

#[test]
fn build_pass() {
    let mut model = base_model();
    assert_eq!(validate(&mut model), Ok(()));
}

#[test]
fn build_reject_toolchain_empty() {
    let mut model = base_model();
    model.build.toolchain = StringId::ZERO;
    assert!(matches!(
        validate(&mut model),
        Err(InvariantViolation::BuildMalformed { .. })
    ));
}

// -----------------------------------------------------------------------------
// FnSig / CAbiLowering consistency
// -----------------------------------------------------------------------------

fn intern_sig(model: &mut RegseqModel, sig: FnSig, lowering: Option<CAbiLowering>) {
    model.sigs.intern(sig);
    model.sig_c_abi_lowerings.push(lowering);
}

#[test]
fn fn_sig_c_variadic_pass() {
    let mut model = base_model();
    let u32_ty = model.types.intern(Ty::Uint(UintKind::U32));
    intern_sig(
        &mut model,
        FnSig {
            inputs: vec![u32_ty],
            output: u32_ty,
            abi: FnAbi::C,
            c_variadic: true,
            safety: Safety::Safe,
        },
        Some(CAbiLowering {
            inputs: vec![CAbiShape::Direct],
            output: CAbiShape::Direct,
        }),
    );
    assert_eq!(validate(&mut model), Ok(()));
}

#[test]
fn fn_sig_c_variadic_reject_non_c_abi() {
    let mut model = base_model();
    let u32_ty = model.types.intern(Ty::Uint(UintKind::U32));
    intern_sig(
        &mut model,
        FnSig {
            inputs: vec![u32_ty],
            output: u32_ty,
            abi: FnAbi::Rust,
            c_variadic: true,
            safety: Safety::Safe,
        },
        None,
    );
    assert!(matches!(
        validate(&mut model),
        Err(InvariantViolation::FnSigMalformed { reason, .. })
            if reason.contains("c_variadic")
    ));
}

#[test]
fn fn_sig_c_variadic_reject_zero_inputs() {
    let mut model = base_model();
    let u32_ty = model.types.intern(Ty::Uint(UintKind::U32));
    intern_sig(
        &mut model,
        FnSig {
            inputs: vec![],
            output: u32_ty,
            abi: FnAbi::C,
            c_variadic: true,
            safety: Safety::Safe,
        },
        Some(CAbiLowering {
            inputs: vec![],
            output: CAbiShape::Direct,
        }),
    );
    assert!(matches!(
        validate(&mut model),
        Err(InvariantViolation::FnSigMalformed { reason, .. })
            if reason.contains("fixed input")
    ));
}

#[test]
fn fn_sig_reject_inputs_never() {
    let mut model = base_model();
    let never_ty = model.types.intern(Ty::Never);
    let u32_ty = model.types.intern(Ty::Uint(UintKind::U32));
    intern_sig(
        &mut model,
        FnSig {
            inputs: vec![never_ty],
            output: u32_ty,
            abi: FnAbi::Rust,
            c_variadic: false,
            safety: Safety::Safe,
        },
        None,
    );
    assert!(matches!(
        validate(&mut model),
        Err(InvariantViolation::FnSigMalformed { reason, .. })
            if reason.contains("Ty::Never")
    ));
}

#[test]
fn cabi_lowering_pass_arity() {
    let mut model = base_model();
    let u32_ty = model.types.intern(Ty::Uint(UintKind::U32));
    intern_sig(
        &mut model,
        FnSig {
            inputs: vec![u32_ty, u32_ty],
            output: u32_ty,
            abi: FnAbi::C,
            c_variadic: false,
            safety: Safety::Safe,
        },
        Some(CAbiLowering {
            inputs: vec![CAbiShape::Direct, CAbiShape::Direct],
            output: CAbiShape::Direct,
        }),
    );
    assert_eq!(validate(&mut model), Ok(()));
}

#[test]
fn cabi_lowering_reject_arity_mismatch() {
    let mut model = base_model();
    let u32_ty = model.types.intern(Ty::Uint(UintKind::U32));
    intern_sig(
        &mut model,
        FnSig {
            inputs: vec![u32_ty, u32_ty],
            output: u32_ty,
            abi: FnAbi::C,
            c_variadic: false,
            safety: Safety::Safe,
        },
        Some(CAbiLowering {
            inputs: vec![CAbiShape::Direct],
            output: CAbiShape::Direct,
        }),
    );
    assert!(matches!(
        validate(&mut model),
        Err(InvariantViolation::CAbiLoweringMalformed { reason, .. })
            if reason.contains("inputs.len()")
    ));
}

#[test]
fn cabi_lowering_reject_substituted_emit_as_dangling() {
    let mut model = base_model();
    let u32_ty = model.types.intern(Ty::Uint(UintKind::U32));
    intern_sig(
        &mut model,
        FnSig {
            inputs: vec![u32_ty],
            output: u32_ty,
            abi: FnAbi::C,
            c_variadic: false,
            safety: Safety::Safe,
        },
        Some(CAbiLowering {
            inputs: vec![CAbiShape::Substituted {
                emit_as: TyId::from_u32(99_999),
                reason: SubstReason::TransparentChain,
            }],
            output: CAbiShape::Direct,
        }),
    );
    assert!(matches!(
        validate(&mut model),
        Err(InvariantViolation::CAbiLoweringMalformed { reason, .. })
            if reason.contains("out of range")
    ));
}

#[test]
fn cabi_lowering_reject_tag_scalar_non_int() {
    let mut model = base_model();
    let u32_ty = model.types.intern(Ty::Uint(UintKind::U32));
    let bool_ty = model.types.intern(Ty::Bool);
    intern_sig(
        &mut model,
        FnSig {
            inputs: vec![u32_ty],
            output: u32_ty,
            abi: FnAbi::C,
            c_variadic: false,
            safety: Safety::Safe,
        },
        Some(CAbiLowering {
            inputs: vec![CAbiShape::Substituted {
                emit_as: bool_ty,
                reason: SubstReason::TagScalar,
            }],
            output: CAbiShape::Direct,
        }),
    );
    assert!(matches!(
        validate(&mut model),
        Err(InvariantViolation::CAbiLoweringMalformed { reason, .. })
            if reason.contains("TagScalar")
    ));
}

#[test]
fn cabi_lowering_reject_some_for_rust_abi() {
    let mut model = base_model();
    let u32_ty = model.types.intern(Ty::Uint(UintKind::U32));
    intern_sig(
        &mut model,
        FnSig {
            inputs: vec![u32_ty],
            output: u32_ty,
            abi: FnAbi::Rust,
            c_variadic: false,
            safety: Safety::Safe,
        },
        Some(CAbiLowering {
            inputs: vec![CAbiShape::Direct],
            output: CAbiShape::Direct,
        }),
    );
    assert!(matches!(
        validate(&mut model),
        Err(InvariantViolation::CAbiLoweringMalformed { reason, .. })
            if reason.contains("Rust")
    ));
}

#[test]
fn cabi_lowering_reject_none_for_c_abi() {
    let mut model = base_model();
    let u32_ty = model.types.intern(Ty::Uint(UintKind::U32));
    intern_sig(
        &mut model,
        FnSig {
            inputs: vec![u32_ty],
            output: u32_ty,
            abi: FnAbi::C,
            c_variadic: false,
            safety: Safety::Safe,
        },
        None,
    );
    assert!(matches!(
        validate(&mut model),
        Err(InvariantViolation::CAbiLoweringMalformed { reason, .. })
            if reason.contains("FnAbi::C")
    ));
}

// -----------------------------------------------------------------------------
// Layout sanity + tag-scalar width-fit
// -----------------------------------------------------------------------------

fn struct_with_field_offsets(
    model: &mut RegseqModel,
    offsets: &[(TyId, u64)],
    size: u64,
) -> Layout {
    let fields = offsets
        .iter()
        .map(|(ty, offset)| FieldLayout {
            name: model.strings.intern("f"),
            ty: *ty,
            offset: *offset,
        })
        .collect();
    struct_layout(size, 4, fields)
}

#[test]
fn layout_pass_struct() {
    let mut model = base_model();
    let u32_ty = model.types.intern(Ty::Uint(UintKind::U32));
    let layout = struct_with_field_offsets(&mut model, &[(u32_ty, 0), (u32_ty, 4)], 8);
    let item = push_item(&mut model, "row", ItemKind::Struct, 1);
    model.types.intern(Ty::Struct {
        item,
        args: vec![],
        layout,
    });
    assert_eq!(validate(&mut model), Ok(()));
}

#[test]
fn layout_reject_field_overlap() {
    let mut model = base_model();
    let u32_ty = model.types.intern(Ty::Uint(UintKind::U32));
    let layout = struct_with_field_offsets(&mut model, &[(u32_ty, 0), (u32_ty, 0)], 8);
    let item = push_item(&mut model, "row", ItemKind::Struct, 1);
    model.types.intern(Ty::Struct {
        item,
        args: vec![],
        layout,
    });
    assert!(matches!(
        validate(&mut model),
        Err(InvariantViolation::LayoutMalformed { reason, .. })
            if reason.contains("overlap")
    ));
}

#[test]
fn layout_reject_field_offset_misaligned() {
    let mut model = base_model();
    let u32_ty = model.types.intern(Ty::Uint(UintKind::U32));
    let layout = struct_with_field_offsets(&mut model, &[(u32_ty, 1)], 8);
    let item = push_item(&mut model, "row", ItemKind::Struct, 1);
    model.types.intern(Ty::Struct {
        item,
        args: vec![],
        layout,
    });
    assert!(matches!(
        validate(&mut model),
        Err(InvariantViolation::LayoutMalformed { reason, .. })
            if reason.contains("aligned")
    ));
}

#[test]
fn layout_reject_field_overflows_layout_size() {
    let mut model = base_model();
    let u32_ty = model.types.intern(Ty::Uint(UintKind::U32));
    let layout = struct_with_field_offsets(&mut model, &[(u32_ty, 8)], 8);
    let item = push_item(&mut model, "row", ItemKind::Struct, 1);
    model.types.intern(Ty::Struct {
        item,
        args: vec![],
        layout,
    });
    assert!(matches!(
        validate(&mut model),
        Err(InvariantViolation::LayoutMalformed { reason, .. })
            if reason.contains("exceeds")
    ));
}

#[test]
fn layout_pack_min_rule_pass() {
    let mut model = base_model();
    let u32_ty = model.types.intern(Ty::Uint(UintKind::U32));
    let name = model.strings.intern("f");
    let layout = Layout {
        size: 6,
        align: 2,
        pack: Some(2),
        uninhabited: false,
        kind: LayoutKind::Struct {
            fields: vec![FieldLayout {
                name,
                ty: u32_ty,
                offset: 2,
            }],
        },
    };
    let item = push_item(&mut model, "packed", ItemKind::Struct, 1);
    model.types.intern(Ty::Struct {
        item,
        args: vec![],
        layout,
    });
    assert_eq!(validate(&mut model), Ok(()));
}

fn enum_with_tag_values(scalar_ty: TyId, valid_end: u128, variant_tag: u128) -> Layout {
    Layout {
        size: 1,
        align: 1,
        pack: None,
        uninhabited: false,
        kind: LayoutKind::Enum {
            shape: EnumShape::Multiple {
                tag: TagScalar {
                    offset: 0,
                    scalar_ty,
                    valid_range: WrappingRange {
                        start: 0,
                        end: valid_end,
                    },
                },
                encoding: TagEncoding::Direct,
                variants: vec![VariantLayout {
                    name: StringId::ZERO,
                    fields: vec![],
                    uninhabited: false,
                    discriminant: variant_tag,
                    tag_value: Some(variant_tag),
                }],
            },
        },
    }
}

#[test]
fn tag_scalar_width_fit_pass() {
    let mut model = base_model();
    let u8_ty = model.types.intern(Ty::Uint(UintKind::U8));
    let item = push_item(&mut model, "color", ItemKind::Enum, 1);
    let name = model.strings.intern("variant_name");
    let layout = Layout {
        size: 1,
        align: 1,
        pack: None,
        uninhabited: false,
        kind: LayoutKind::Enum {
            shape: EnumShape::Multiple {
                tag: TagScalar {
                    offset: 0,
                    scalar_ty: u8_ty,
                    valid_range: WrappingRange { start: 0, end: 255 },
                },
                encoding: TagEncoding::Direct,
                variants: vec![VariantLayout {
                    name,
                    fields: vec![],
                    uninhabited: false,
                    discriminant: 7,
                    tag_value: Some(7),
                }],
            },
        },
    };
    model.types.intern(Ty::Enum {
        item,
        args: vec![],
        layout,
    });
    assert_eq!(validate(&mut model), Ok(()));
}

#[test]
fn tag_scalar_width_fit_reject_overflow() {
    let mut model = base_model();
    let u8_ty = model.types.intern(Ty::Uint(UintKind::U8));
    let item = push_item(&mut model, "color", ItemKind::Enum, 1);
    let layout = enum_with_tag_values(u8_ty, 256, 7);
    model.types.intern(Ty::Enum {
        item,
        args: vec![],
        layout,
    });
    assert!(matches!(
        validate(&mut model),
        Err(InvariantViolation::TagScalarOutOfRange {
            value: 256,
            scalar_width_bits: 8,
            ..
        })
    ));
}

// -----------------------------------------------------------------------------
// Const-graph acyclicity
// -----------------------------------------------------------------------------

#[test]
fn const_graph_acyclic_pass() {
    let mut model = base_model();
    let child = model.consts.intern(Const::Uint(UintKind::U8, 1));
    model.consts.intern(Const::Aggregate(vec![child, child]));
    assert_eq!(validate(&mut model), Ok(()));
}

#[test]
fn const_graph_dag_with_sharing_pass() {
    let mut model = base_model();
    let leaf = model.consts.intern(Const::Uint(UintKind::U8, 0));
    let mid = model.consts.intern(Const::Aggregate(vec![leaf]));
    model.consts.intern(Const::Aggregate(vec![mid, mid]));
    assert_eq!(validate(&mut model), Ok(()));
}

#[test]
fn const_graph_cycle_reject_self_loop() {
    let mut model = base_model();
    let mut storage = IndexVec::<regseq_model::ConstId, Const>::new();
    let self_id = storage.push(Const::ZST);
    storage[self_id] = Const::Aggregate(vec![self_id]);
    model.consts = ConstInterner::from_storage_unchecked_for_validation(storage);
    assert!(matches!(
        validate(&mut model),
        Err(InvariantViolation::ConstGraphCycle { .. })
    ));
}

#[test]
fn const_graph_cycle_reject_two_node() {
    let mut model = base_model();
    let mut storage = IndexVec::<regseq_model::ConstId, Const>::new();
    let a = storage.push(Const::ZST);
    let b = storage.push(Const::ZST);
    storage[a] = Const::Aggregate(vec![b]);
    storage[b] = Const::Aggregate(vec![a]);
    model.consts = ConstInterner::from_storage_unchecked_for_validation(storage);
    assert!(matches!(
        validate(&mut model),
        Err(InvariantViolation::ConstGraphCycle { .. })
    ));
}

#[test]
fn const_graph_cycle_reject_enum_value_payload() {
    let mut model = base_model();
    let mut storage = IndexVec::<regseq_model::ConstId, Const>::new();
    let id = storage.push(Const::ZST);
    storage[id] = Const::EnumValue {
        variant_idx: VariantIdx::from_u32(0),
        payload: vec![id],
    };
    model.consts = ConstInterner::from_storage_unchecked_for_validation(storage);
    assert!(matches!(
        validate(&mut model),
        Err(InvariantViolation::ConstGraphCycle { .. })
    ));
}

// -----------------------------------------------------------------------------
// Domain-attr validation
// -----------------------------------------------------------------------------

#[test]
fn domain_attr_item_attrs_len_pass() {
    let mut model = base_model();
    push_item(&mut model, "thing", ItemKind::Function, 1);
    assert_eq!(validate(&mut model), Ok(()));
}

#[test]
fn domain_attr_item_attrs_len_reject() {
    let mut model = base_model();
    push_item(&mut model, "thing", ItemKind::Function, 1);
    model.domain_attrs.item_attrs.push(Vec::new());
    assert!(matches!(
        validate(&mut model),
        Err(InvariantViolation::DomainAttrMalformed { reason })
            if reason.contains("item_attrs.len()")
    ));
}

#[test]
fn domain_attr_field_path_pass_struct() {
    let mut model = base_model();
    let u32_ty = model.types.intern(Ty::Uint(UintKind::U32));
    let item = push_item(&mut model, "row", ItemKind::Struct, 1);
    let name = model.strings.intern("inner");
    let layout = struct_layout(
        4,
        4,
        vec![FieldLayout {
            name,
            ty: u32_ty,
            offset: 0,
        }],
    );
    model.types.intern(Ty::Struct {
        item,
        args: vec![],
        layout,
    });
    model.domain_attrs.field_attrs.insert(
        FieldPath {
            item,
            variant: VariantIdx::from_u32(0),
            field: FieldIdx::from_u32(0),
        },
        vec![AttrData::Intrinsic(IntrinsicKind::Log(LogLevel::Info))],
    );
    assert_eq!(validate(&mut model), Ok(()));
}

#[test]
fn domain_attr_field_path_reject_non_adt_item() {
    let mut model = base_model();
    let item = push_item(&mut model, "func", ItemKind::Function, 1);
    model.domain_attrs.field_attrs.insert(
        FieldPath {
            item,
            variant: VariantIdx::from_u32(0),
            field: FieldIdx::from_u32(0),
        },
        vec![],
    );
    assert!(matches!(
        validate(&mut model),
        Err(InvariantViolation::DomainAttrMalformed { reason })
            if reason.contains("not an ADT")
    ));
}

#[test]
fn domain_attr_field_path_reject_variant_idx_out_of_range() {
    let mut model = base_model();
    let u32_ty = model.types.intern(Ty::Uint(UintKind::U32));
    let item = push_item(&mut model, "row", ItemKind::Struct, 1);
    let name = model.strings.intern("inner");
    let layout = struct_layout(
        4,
        4,
        vec![FieldLayout {
            name,
            ty: u32_ty,
            offset: 0,
        }],
    );
    model.types.intern(Ty::Struct {
        item,
        args: vec![],
        layout,
    });
    model.domain_attrs.field_attrs.insert(
        FieldPath {
            item,
            variant: VariantIdx::from_u32(1),
            field: FieldIdx::from_u32(0),
        },
        vec![],
    );
    assert!(matches!(
        validate(&mut model),
        Err(InvariantViolation::DomainAttrMalformed { reason })
            if reason.contains("variant")
    ));
}

#[test]
fn domain_attr_field_path_reject_field_idx_out_of_range() {
    let mut model = base_model();
    let u32_ty = model.types.intern(Ty::Uint(UintKind::U32));
    let item = push_item(&mut model, "row", ItemKind::Struct, 1);
    let name = model.strings.intern("inner");
    let layout = struct_layout(
        4,
        4,
        vec![FieldLayout {
            name,
            ty: u32_ty,
            offset: 0,
        }],
    );
    model.types.intern(Ty::Struct {
        item,
        args: vec![],
        layout,
    });
    model.domain_attrs.field_attrs.insert(
        FieldPath {
            item,
            variant: VariantIdx::from_u32(0),
            field: FieldIdx::from_u32(5),
        },
        vec![],
    );
    assert!(matches!(
        validate(&mut model),
        Err(InvariantViolation::DomainAttrMalformed { reason })
            if reason.contains("FieldIdx")
    ));
}

#[test]
fn domain_attr_nested_id_pass() {
    let mut model = base_model();
    let item = push_item(&mut model, "thing", ItemKind::Function, 1);
    model.domain_attrs.item_attrs[item]
        .push(AttrData::Intrinsic(IntrinsicKind::Log(LogLevel::Debug)));
    assert_eq!(validate(&mut model), Ok(()));
}

#[test]
fn domain_attr_nested_id_reject_dangling() {
    // AttrData::Intrinsic carries no nested IDs in the current schema;
    // dangling-id behavior is verified by validate_domain_attr_spans
    // via the VAL-02 walk. This is a smoke test that an Intrinsic
    // entry on an item index just past the end is structurally
    // covered by the length-mismatch rejection above.
    let mut model = base_model();
    push_item(&mut model, "thing", ItemKind::Function, 1);
    model
        .domain_attrs
        .item_attrs
        .push(vec![AttrData::Intrinsic(IntrinsicKind::Log(
            LogLevel::Debug,
        ))]);
    assert!(matches!(
        validate(&mut model),
        Err(InvariantViolation::DomainAttrMalformed { reason })
            if reason.contains("item_attrs.len()")
    ));
}

#[test]
fn intrinsic_singleton_pass() {
    let mut model = base_model();
    let item = push_item(&mut model, "thing", ItemKind::Function, 1);
    model.domain_attrs.item_attrs[item]
        .push(AttrData::Intrinsic(IntrinsicKind::Log(LogLevel::Info)));
    assert_eq!(validate(&mut model), Ok(()));
}

#[test]
fn intrinsic_singleton_reject_two_intrinsics() {
    let mut model = base_model();
    let item = push_item(&mut model, "thing", ItemKind::Function, 1);
    model.domain_attrs.item_attrs[item]
        .push(AttrData::Intrinsic(IntrinsicKind::Log(LogLevel::Info)));
    model.domain_attrs.item_attrs[item]
        .push(AttrData::Intrinsic(IntrinsicKind::Log(LogLevel::Warn)));
    assert!(matches!(
        validate(&mut model),
        Err(InvariantViolation::DomainAttrMalformed { reason })
            if reason.contains("Intrinsic-Singleton")
    ));
}

// -----------------------------------------------------------------------------
// Source-file validation
// -----------------------------------------------------------------------------

#[test]
fn source_file_pass() {
    let mut model = base_model();
    let path = model.strings.intern("src/lib.rs");
    model.source_files.push(SourceFile {
        path,
        content: Some(Arc::from("hello")),
        line_starts: vec![0, 3],
    });
    assert_eq!(validate(&mut model), Ok(()));
}

#[test]
fn source_file_reject_line_starts_non_monotonic() {
    let mut model = base_model();
    let path = model.strings.intern("src/lib.rs");
    model.source_files.push(SourceFile {
        path,
        content: Some(Arc::from("hello world\nmore\n")),
        line_starts: vec![0, 5, 3],
    });
    assert!(matches!(
        validate(&mut model),
        Err(InvariantViolation::SourceFileMalformed { reason, .. })
            if reason.contains("monotonic")
    ));
}

#[test]
fn source_file_reject_line_starts_does_not_start_at_zero() {
    let mut model = base_model();
    let path = model.strings.intern("src/lib.rs");
    model.source_files.push(SourceFile {
        path,
        content: Some(Arc::from("hello")),
        line_starts: vec![1, 5],
    });
    let result = validate(&mut model);
    assert!(
        matches!(
            &result,
            Err(InvariantViolation::SourceFileMalformed { reason, .. })
                if reason.contains("start")
        ),
        "unexpected result: {result:?}"
    );
}

#[test]
fn source_file_reject_line_start_exceeds_content_len() {
    let mut model = base_model();
    let path = model.strings.intern("src/lib.rs");
    model.source_files.push(SourceFile {
        path,
        content: Some(Arc::from("0123456789")),
        line_starts: vec![0, 999],
    });
    assert!(matches!(
        validate(&mut model),
        Err(InvariantViolation::SourceFileMalformed { reason, .. })
            if reason.contains("content length")
    ));
}

#[test]
fn source_file_reject_duplicate_path() {
    let mut model = base_model();
    let path = model.strings.intern("src/lib.rs");
    model.source_files.push(SourceFile {
        path,
        content: None,
        line_starts: vec![0],
    });
    model.source_files.push(SourceFile {
        path,
        content: None,
        line_starts: vec![0],
    });
    assert!(matches!(
        validate(&mut model),
        Err(InvariantViolation::DuplicateSourceFilePath { .. })
    ));
}

#[test]
fn source_file_pass_distinct_paths() {
    let mut model = base_model();
    let path_a = model.strings.intern("src/a.rs");
    let path_b = model.strings.intern("src/b.rs");
    model.source_files.push(SourceFile {
        path: path_a,
        content: None,
        line_starts: vec![0],
    });
    model.source_files.push(SourceFile {
        path: path_b,
        content: None,
        line_starts: vec![0],
    });
    assert_eq!(validate(&mut model), Ok(()));
}

// -----------------------------------------------------------------------------
// Silence unused-import warnings for partial-coverage symbols.
// -----------------------------------------------------------------------------

#[allow(dead_code)]
fn _imports() {
    let _ = (
        ClosureCallKind::Call,
        ContentHash(0),
        CrateId::ZERO,
        FloatKind::F32,
        FunctionInstanceData {
            fn_sig: regseq_model::FnSigId::ZERO,
            body: None,
            flavor: FunctionInstanceFlavor::Item,
            virtual_idx: None,
        },
        InstanceKind::Const {
            ty: TyId::ZERO,
            value: regseq_model::ConstId::ZERO,
        },
        IntKind::I32,
        PrincipalTraitKey {
            trait_path: synth_hash(0),
            trait_args: ArgsKey::EMPTY,
            projection_bindings: ProjectionBindingsKey(0),
        },
        VariantRange {
            start: VariantIdx::from_u32(0),
            end: VariantIdx::from_u32(0),
        },
    );
}
