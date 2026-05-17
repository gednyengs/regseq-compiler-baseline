//! Integration tests for stage-1 step-6: top-level `RegseqModel`,
//! its constructor, and the construction-invariants checker.
//!
//! These tests cover:
//! - Empty-model construction (synthesized sentinel populated,
//!   primary crate at index 0, strings interner pre-populated).
//! - Round-trip through JSON for both empty and minimally-populated
//!   models.
//! - Each in-scope construction invariant: positive case (clean
//!   model validates) and negative case (deliberately violated
//!   model surfaces the expected variant).
//! - Deferred invariants (3, 6, 7, 9 + the bridge-input half of 4)
//!   noted via comments at their corresponding test sites.

use crate::{
    Body, BridgeInput, ConstId, CrateId, CrateInfo, DefPath, DefPathData, DefPathSegment, FnAbi,
    FnSig, FnSigId, FunctionInstanceData, FunctionInstanceFlavor, GenericArg, InlineHint, Instance,
    InstanceId, InstanceKind, InvariantId, InvariantViolation, Item, ItemId, ItemKind, Linkage,
    RegseqModel, Safety, SourceFileId, Span, StableCrateId, StringId, Ty, TyId,
};

fn default_sig() -> FnSig {
    FnSig {
        inputs: vec![],
        output: TyId::ZERO,
        abi: FnAbi::Rust,
        c_variadic: false,
        safety: Safety::Safe,
    }
}

fn fixture_item(model: &mut RegseqModel) -> Item {
    let name = model.strings.intern("fixture_item");
    let rendered = model.strings.intern("pl011-spec::fixture_item");
    Item {
        def_path: DefPath {
            crate_id: model.primary_crate,
            segments: vec![
                DefPathSegment {
                    data: DefPathData::CrateRoot,
                    disambiguator: 0,
                },
                DefPathSegment {
                    data: DefPathData::ValueNs(name),
                    disambiguator: 0,
                },
            ],
            rendered,
        },
        kind: ItemKind::Function,
        span: Span {
            file: SourceFileId::SYNTHESIZED,
            lo: 0,
            hi: 0,
        },
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

fn push_fixture_item(model: &mut RegseqModel) -> ItemId {
    let item = fixture_item(model);
    model.items.push(item)
}

fn function_kind(sig: FnSigId, body: Option<Body>) -> InstanceKind {
    InstanceKind::Function(FunctionInstanceData {
        fn_sig: sig,
        body,
        flavor: FunctionInstanceFlavor::Item,
        virtual_idx: None,
    })
}

// ------------------------------------------------------------------
// Constructor: empty model contract.
// ------------------------------------------------------------------

#[test]
fn with_primary_creates_synthesized_sentinel_at_index_zero() {
    let m = RegseqModel::with_primary("pl011-spec", StableCrateId(0xCAFE));

    // (8) source_files[0] is the synthesized sentinel.
    assert_eq!(m.source_files.len(), 1);
    let sentinel = &m.source_files[SourceFileId::SYNTHESIZED];
    assert_eq!(m.strings.resolve(sentinel.path), "<synthesized>");
    assert!(sentinel.content.is_none());
    assert_eq!(sentinel.line_starts, vec![0]);
}

#[test]
fn with_primary_registers_primary_crate_at_index_zero() {
    let m = RegseqModel::with_primary("pl011-spec", StableCrateId(0xCAFE));

    assert_eq!(m.crates.len(), 1);
    assert_eq!(m.primary_crate, CrateId::ZERO);
    let info = &m.crates[m.primary_crate];
    assert_eq!(m.strings.resolve(info.name), "pl011-spec");
    assert_eq!(info.stable_crate_id, StableCrateId(0xCAFE));
}

#[test]
fn with_primary_pre_interns_empty_string_then_synthesized_then_primary_name() {
    let m = RegseqModel::with_primary("pl011-spec", StableCrateId(0));
    // Pre-existing invariant of StringInterner: index 0 is "".
    assert_eq!(m.strings.resolve(StringId::ZERO), "");
    // Constructor interns "<synthesized>" (becomes index 1) and
    // then "pl011-spec" (becomes index 2). The exact StringIds are
    // implementation detail; round-trip through resolve is the
    // observable contract.
    assert!(m.strings.iter().any(|(_, s)| s == "<synthesized>"));
    assert!(m.strings.iter().any(|(_, s)| s == "pl011-spec"));
}

#[test]
fn with_primary_leaves_other_tables_empty() {
    let m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    assert!(m.types.is_empty());
    assert_eq!(m.consts.len(), 0);
    assert!(m.items.is_empty());
    assert!(m.item_def_paths.is_empty());
    assert!(m.item_synthetic_kinds.is_empty());
    assert_eq!(m.sigs.len(), 0);
    assert!(m.sig_c_abi_lowerings.is_empty());
    assert!(m.instances.is_empty());
    assert!(m.vtables.is_empty());
}

#[test]
fn compare_items_f4_orders_item_payloads_by_def_path() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let beta_name = m.strings.intern("beta");
    let beta_rendered = m.strings.intern("pl011-spec::beta");
    let alpha_name = m.strings.intern("alpha");
    let alpha_rendered = m.strings.intern("pl011-spec::alpha");
    let mut beta = fixture_item(&mut m);
    beta.def_path.segments[1] = DefPathSegment {
        data: DefPathData::ValueNs(beta_name),
        disambiguator: 0,
    };
    beta.def_path.rendered = beta_rendered;
    let mut alpha = fixture_item(&mut m);
    alpha.def_path.segments[1] = DefPathSegment {
        data: DefPathData::ValueNs(alpha_name),
        disambiguator: 0,
    };
    alpha.def_path.rendered = alpha_rendered;

    assert!(m.compare_items_f4(&alpha, &beta).is_lt());
    assert!(m.compare_items_f4(&beta, &alpha).is_gt());
}

#[test]
fn root_struct_exposes_spec_field_set() {
    let m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let RegseqModel {
        crates,
        primary_crate,
        target,
        build,
        strings,
        source_files,
        types,
        consts,
        sigs,
        items,
        item_def_paths,
        item_synthetic_kinds,
        sig_c_abi_lowerings,
        instances,
        vtables,
        closure_captures_struct_ty,
        domain_attrs,
    } = m;

    assert_eq!(primary_crate, CrateId::ZERO);
    assert_eq!(crates.len(), 1);
    assert_eq!(target.pointer_width, 64);
    assert_eq!(build.bridge_options_hash, 0);
    assert!(!strings.is_empty());
    assert_eq!(source_files.len(), 1);
    assert!(types.is_empty());
    assert_eq!(consts.len(), 0);
    assert_eq!(sigs.len(), 0);
    assert!(items.is_empty());
    assert!(item_def_paths.is_empty());
    assert!(item_synthetic_kinds.is_empty());
    assert!(sig_c_abi_lowerings.is_empty());
    assert!(instances.is_empty());
    assert!(vtables.is_empty());
    assert!(closure_captures_struct_ty.is_empty());
    assert_eq!(domain_attrs, crate::DomainAttrTables::default());
}

// ------------------------------------------------------------------
// Validation: positive cases.
// ------------------------------------------------------------------

#[test]
fn empty_model_passes_validate() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    assert_eq!(m.validate(&BridgeInput::matching(&m)), Ok(()));
}

#[test]
fn populated_model_passes_validate() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(0xAAAA));

    // Add a second crate.
    let dep_name = m.strings.intern("regseq");
    let dep_id = m.crates.push(CrateInfo {
        name: dep_name,
        stable_crate_id: StableCrateId(0xBBBB),
    });
    assert_ne!(dep_id, m.primary_crate);

    // Populate placeholder tables so InstanceKind references resolve.
    let ty = m.types.push(Ty::Bool);
    let init = m.consts.intern(crate::Const::ZST);
    let sig = m.sigs.intern(default_sig());
    m.sig_c_abi_lowerings.push(None);

    let item = push_fixture_item(&mut m);
    m.item_def_paths.push(crate::DefPathHash {
        stable_crate_id: StableCrateId(0xAAAA),
        local_hash: 0xCAFE,
    });
    m.item_synthetic_kinds.push(Default::default());
    m.domain_attrs.item_attrs.push(Vec::new());

    let symbol = m.strings.intern("foo");
    m.instances.push(Instance {
        item,
        generic_args: vec![GenericArg::Type(ty), GenericArg::Const(init)],
        emission_name: symbol,
        span: Span {
            file: SourceFileId::SYNTHESIZED,
            lo: 0,
            hi: 0,
        },
        kind: function_kind(sig, Some(Body::default())),
    });

    // Add a Static instance; another item entry for it.
    let item2 = push_fixture_item(&mut m);
    m.item_def_paths.push(crate::DefPathHash {
        stable_crate_id: StableCrateId(0xAAAA),
        local_hash: 0xF00D,
    });
    m.item_synthetic_kinds.push(Default::default());
    m.domain_attrs.item_attrs.push(Vec::new());
    let static_ty = m.types.push(Ty::Bool);
    let static_init = m.consts.intern(crate::Const::ZST);
    m.instances.push(Instance {
        item: item2,
        generic_args: vec![],
        emission_name: symbol,
        span: Span {
            file: SourceFileId::SYNTHESIZED,
            lo: 0,
            hi: 0,
        },
        kind: InstanceKind::Static {
            ty: static_ty,
            init: Some(static_init),
        },
    });

    assert_eq!(m.validate(&BridgeInput::matching(&m)), Ok(()));
}

#[test]
fn function_with_body_none_validates_when_sig_resolves() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let item = push_fixture_item(&mut m);
    m.item_def_paths.push(crate::DefPathHash {
        stable_crate_id: StableCrateId(1),
        local_hash: 0,
    });
    m.item_synthetic_kinds.push(Default::default());
    m.domain_attrs.item_attrs.push(Vec::new());
    let _ = m.types.push(Ty::Bool);
    let sig = m.sigs.intern(default_sig());
    m.sig_c_abi_lowerings.push(None);
    m.instances.push(Instance {
        item,
        generic_args: vec![],
        emission_name: StringId::ZERO,
        span: Span {
            file: SourceFileId::SYNTHESIZED,
            lo: 0,
            hi: 0,
        },
        kind: function_kind(sig, None),
    });

    // (10) `body: None` is reserved for foreign declarations whose
    // bodies are not in our crate set; the bridge does not synthesize
    // empty bodies, and validate must accept it without requiring a
    // Body entry.
    assert_eq!(m.validate(&BridgeInput::matching(&m)), Ok(()));
}

// ------------------------------------------------------------------
// Validation: negative cases, one per enforceable invariant.
// ------------------------------------------------------------------

#[test]
fn invariant_8_synthesized_sentinel_invalid_path() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    // Corrupt the path of source_files[0].
    let bogus = m.strings.intern("not-the-sentinel");
    m.source_files[SourceFileId::SYNTHESIZED].path = bogus;
    let err = m.validate(&BridgeInput::matching(&m)).unwrap_err();
    match err {
        InvariantViolation::SynthesizedSentinelInvalid { reason } => {
            assert!(reason.contains("synthesized"), "reason: {reason}");
        }
        other => panic!("expected SynthesizedSentinelInvalid, got {:?}", other),
    }
}

#[test]
fn invariant_8_synthesized_sentinel_with_content_rejected() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    m.source_files[SourceFileId::SYNTHESIZED].content = Some("nope".into());
    let err = m.validate(&BridgeInput::matching(&m)).unwrap_err();
    assert!(matches!(
        err,
        InvariantViolation::SynthesizedSentinelInvalid { reason: r } if r.contains("content")
    ));
}

#[test]
fn invariant_8_synthesized_sentinel_with_wrong_line_starts_rejected() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    m.source_files[SourceFileId::SYNTHESIZED].line_starts = vec![0, 1, 2];
    let err = m.validate(&BridgeInput::matching(&m)).unwrap_err();
    assert!(matches!(
        err,
        InvariantViolation::SynthesizedSentinelInvalid { reason: r } if r.contains("line_starts")
    ));
}

#[test]
fn invariant_4_primary_crate_out_of_range() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    m.primary_crate = CrateId::from_u32(99);
    let bridge_input = BridgeInput {
        spec_crate_stable_id: StableCrateId(1),
    };
    match m.validate(&bridge_input).unwrap_err() {
        InvariantViolation::PrimaryCrateOutOfRange { value, crates_len } => {
            assert_eq!(value, 99);
            assert_eq!(crates_len, 1);
        }
        other => panic!("expected PrimaryCrateOutOfRange, got {:?}", other),
    }
}

#[test]
fn invariant_4_primary_crate_stable_id_mismatch() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let bridge_input = BridgeInput {
        spec_crate_stable_id: StableCrateId(2),
    };
    match m.validate(&bridge_input).unwrap_err() {
        InvariantViolation::PrimaryCrateStableIdMismatch {
            model,
            bridge_input,
        } => {
            assert_eq!(model, StableCrateId(1));
            assert_eq!(bridge_input, StableCrateId(2));
        }
        other => panic!("expected PrimaryCrateStableIdMismatch, got {:?}", other),
    }
}

#[test]
fn invariant_5_duplicate_stable_crate_id() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(0xAAAA));
    let dep_name = m.strings.intern("regseq");
    m.crates.push(CrateInfo {
        name: dep_name,
        // Same stable_crate_id as the primary crate -> violation.
        stable_crate_id: StableCrateId(0xAAAA),
    });
    match m.validate(&BridgeInput::matching(&m)).unwrap_err() {
        InvariantViolation::DuplicateStableCrateId {
            stable_id,
            first,
            second,
        } => {
            assert_eq!(stable_id, StableCrateId(0xAAAA));
            assert_eq!(first, CrateId::from_u32(0));
            assert_eq!(second, CrateId::from_u32(1));
        }
        other => panic!("expected DuplicateStableCrateId, got {:?}", other),
    }
}

#[test]
fn invariant_2_item_def_paths_length_mismatch() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    push_fixture_item(&mut m);
    // item_def_paths is empty -> mismatch.
    match m.validate(&BridgeInput::matching(&m)).unwrap_err() {
        InvariantViolation::ItemDefPathsLengthMismatch {
            items_len,
            def_paths_len,
        } => {
            assert_eq!(items_len, 1);
            assert_eq!(def_paths_len, 0);
        }
        other => panic!("expected ItemDefPathsLengthMismatch, got {:?}", other),
    }
}

#[test]
fn invariant_2_item_synthetic_kinds_length_mismatch() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    push_fixture_item(&mut m);
    m.item_def_paths.push(crate::DefPathHash {
        stable_crate_id: StableCrateId(1),
        local_hash: 0,
    });
    match m.validate(&BridgeInput::matching(&m)).unwrap_err() {
        InvariantViolation::ItemSyntheticKindsLengthMismatch {
            items_len,
            synthetic_kinds_len,
        } => {
            assert_eq!(items_len, 1);
            assert_eq!(synthetic_kinds_len, 0);
        }
        other => panic!("expected ItemSyntheticKindsLengthMismatch, got {:?}", other),
    }
}

#[test]
fn invariant_1_dangling_item_id_in_instance() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let _ = m.types.push(Ty::Bool);
    let sig = m.sigs.intern(default_sig());
    m.sig_c_abi_lowerings.push(None);
    m.instances.push(Instance {
        // No items pushed; ItemId(0) is dangling.
        item: ItemId::from_u32(0),
        generic_args: vec![],
        emission_name: StringId::ZERO,
        span: Span {
            file: SourceFileId::SYNTHESIZED,
            lo: 0,
            hi: 0,
        },
        kind: function_kind(sig, None),
    });
    match m.validate(&BridgeInput::matching(&m)).unwrap_err() {
        InvariantViolation::DanglingId {
            field,
            value,
            table_len,
        } => {
            assert!(field.contains("Instance.item"), "field: {field}");
            assert_eq!(value, 0);
            assert_eq!(table_len, 0);
        }
        other => panic!("expected DanglingId on ItemId, got {:?}", other),
    }
}

#[test]
fn invariant_1_dangling_string_id_in_crate_info() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let bad = StringId::from_u32(9999);
    m.crates.push(CrateInfo {
        name: bad,
        stable_crate_id: StableCrateId(2),
    });
    match m.validate(&BridgeInput::matching(&m)).unwrap_err() {
        InvariantViolation::DanglingId { field, value, .. } => {
            assert!(field.contains("CrateInfo.name"), "field: {field}");
            assert_eq!(value, 9999);
        }
        other => panic!("expected DanglingId on StringId, got {:?}", other),
    }
}

#[test]
fn invariant_1_dangling_source_file_id_in_instance_span() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let item = push_fixture_item(&mut m);
    m.item_def_paths.push(crate::DefPathHash {
        stable_crate_id: StableCrateId(1),
        local_hash: 0,
    });
    m.item_synthetic_kinds.push(Default::default());
    m.domain_attrs.item_attrs.push(Vec::new());
    let _ = m.types.push(Ty::Bool);
    let sig = m.sigs.intern(default_sig());
    m.sig_c_abi_lowerings.push(None);
    m.instances.push(Instance {
        item,
        generic_args: vec![],
        emission_name: StringId::ZERO,
        span: Span {
            file: SourceFileId::from_u32(42),
            lo: 0,
            hi: 0,
        },
        kind: function_kind(sig, None),
    });
    match m.validate(&BridgeInput::matching(&m)).unwrap_err() {
        InvariantViolation::DanglingId { field, value, .. } => {
            assert!(field.contains("Instance.span.file"), "field: {field}");
            assert_eq!(value, 42);
        }
        other => panic!("expected DanglingId on SourceFileId, got {:?}", other),
    }
}

#[test]
fn invariant_1_dangling_ty_id_in_generic_args() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let item = push_fixture_item(&mut m);
    m.item_def_paths.push(crate::DefPathHash {
        stable_crate_id: StableCrateId(1),
        local_hash: 0,
    });
    m.item_synthetic_kinds.push(Default::default());
    m.domain_attrs.item_attrs.push(Vec::new());
    let _ = m.types.push(Ty::Bool);
    let sig = m.sigs.intern(default_sig());
    m.sig_c_abi_lowerings.push(None);
    m.instances.push(Instance {
        item,
        generic_args: vec![GenericArg::Type(TyId::from_u32(5))],
        emission_name: StringId::ZERO,
        span: Span {
            file: SourceFileId::SYNTHESIZED,
            lo: 0,
            hi: 0,
        },
        kind: function_kind(sig, None),
    });
    match m.validate(&BridgeInput::matching(&m)).unwrap_err() {
        InvariantViolation::DanglingId { field, value, .. } => {
            assert!(field.contains("generic_args"), "field: {field}");
            assert_eq!(value, 5);
        }
        other => panic!("expected DanglingId on TyId, got {:?}", other),
    }
}

#[test]
fn invariant_1_dangling_const_id_in_generic_args() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let item = push_fixture_item(&mut m);
    m.item_def_paths.push(crate::DefPathHash {
        stable_crate_id: StableCrateId(1),
        local_hash: 0,
    });
    m.item_synthetic_kinds.push(Default::default());
    m.domain_attrs.item_attrs.push(Vec::new());
    let _ = m.types.push(Ty::Bool);
    let sig = m.sigs.intern(default_sig());
    m.sig_c_abi_lowerings.push(None);
    m.instances.push(Instance {
        item,
        generic_args: vec![GenericArg::Const(ConstId::from_u32(7))],
        emission_name: StringId::ZERO,
        span: Span {
            file: SourceFileId::SYNTHESIZED,
            lo: 0,
            hi: 0,
        },
        kind: function_kind(sig, None),
    });
    match m.validate(&BridgeInput::matching(&m)).unwrap_err() {
        InvariantViolation::DanglingId { field, value, .. } => {
            assert!(field.contains("generic_args"), "field: {field}");
            assert_eq!(value, 7);
        }
        other => panic!("expected DanglingId on ConstId, got {:?}", other),
    }
}

#[test]
fn invariant_1_dangling_fn_sig_id_in_function_kind() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let item = push_fixture_item(&mut m);
    m.item_def_paths.push(crate::DefPathHash {
        stable_crate_id: StableCrateId(1),
        local_hash: 0,
    });
    m.item_synthetic_kinds.push(Default::default());
    m.domain_attrs.item_attrs.push(Vec::new());
    m.instances.push(Instance {
        item,
        generic_args: vec![],
        emission_name: StringId::ZERO,
        span: Span {
            file: SourceFileId::SYNTHESIZED,
            lo: 0,
            hi: 0,
        },
        kind: function_kind(FnSigId::from_u32(3), None),
    });
    match m.validate(&BridgeInput::matching(&m)).unwrap_err() {
        InvariantViolation::DanglingId { field, value, .. } => {
            assert!(field.contains("Function.fn_sig"), "field: {field}");
            assert_eq!(value, 3);
        }
        other => panic!("expected DanglingId on FnSigId, got {:?}", other),
    }
}

#[test]
fn invariant_10_inline_body_in_function_kind_validates() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let item = push_fixture_item(&mut m);
    m.item_def_paths.push(crate::DefPathHash {
        stable_crate_id: StableCrateId(1),
        local_hash: 0,
    });
    m.item_synthetic_kinds.push(Default::default());
    m.domain_attrs.item_attrs.push(Vec::new());
    let _ = m.types.push(Ty::Bool);
    let sig = m.sigs.intern(default_sig());
    m.sig_c_abi_lowerings.push(None);
    m.instances.push(Instance {
        item,
        generic_args: vec![],
        emission_name: StringId::ZERO,
        span: Span {
            file: SourceFileId::SYNTHESIZED,
            lo: 0,
            hi: 0,
        },
        kind: function_kind(sig, Some(Body::default())),
    });
    assert_eq!(m.validate(&BridgeInput::matching(&m)), Ok(()));
}

#[test]
fn invariant_1_dangling_ty_id_in_static_kind() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let item = push_fixture_item(&mut m);
    m.item_def_paths.push(crate::DefPathHash {
        stable_crate_id: StableCrateId(1),
        local_hash: 0,
    });
    m.item_synthetic_kinds.push(Default::default());
    m.domain_attrs.item_attrs.push(Vec::new());
    m.instances.push(Instance {
        item,
        generic_args: vec![],
        emission_name: StringId::ZERO,
        span: Span {
            file: SourceFileId::SYNTHESIZED,
            lo: 0,
            hi: 0,
        },
        kind: InstanceKind::Static {
            ty: TyId::from_u32(0),
            init: Some(ConstId::from_u32(0)),
        },
    });
    match m.validate(&BridgeInput::matching(&m)).unwrap_err() {
        InvariantViolation::DanglingId { field, value, .. } => {
            assert!(field.contains("Static.ty"), "field: {field}");
            assert_eq!(value, 0);
        }
        other => panic!("expected DanglingId on Static.ty, got {:?}", other),
    }
}

#[test]
fn invariant_1_dangling_layout_in_struct_enum_union() {
    for kind in [
        InstanceKind::Struct {
            layout: TyId::from_u32(7),
        },
        InstanceKind::Enum {
            layout: TyId::from_u32(7),
        },
        InstanceKind::Union {
            layout: TyId::from_u32(7),
        },
    ] {
        let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
        let item = push_fixture_item(&mut m);
        m.item_def_paths.push(crate::DefPathHash {
            stable_crate_id: StableCrateId(1),
            local_hash: 0,
        });
        m.item_synthetic_kinds.push(Default::default());
        m.domain_attrs.item_attrs.push(Vec::new());
        m.instances.push(Instance {
            item,
            generic_args: vec![],
            emission_name: StringId::ZERO,
            span: Span {
                file: SourceFileId::SYNTHESIZED,
                lo: 0,
                hi: 0,
            },
            kind: kind.clone(),
        });
        match m.validate(&BridgeInput::matching(&m)).unwrap_err() {
            InvariantViolation::DanglingId { field, value, .. } => {
                assert!(field.contains("layout"), "field: {field}");
                assert_eq!(value, 7);
            }
            other => panic!(
                "expected DanglingId on layout for {:?}, got {:?}",
                kind, other
            ),
        }
    }
}

#[test]
fn invariant_1_dangling_string_id_in_source_file_path() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    // Push a real source file with a deliberately-dangling path
    // StringId (a value larger than any interned string).
    m.source_files.push(crate::SourceFile {
        path: StringId::from_u32(9999),
        content: None,
        line_starts: vec![0],
    });
    match m.validate(&BridgeInput::matching(&m)).unwrap_err() {
        InvariantViolation::DanglingId { field, value, .. } => {
            assert!(field.contains("SourceFile.path"), "field: {field}");
            assert_eq!(value, 9999);
        }
        other => panic!("expected DanglingId on SourceFile.path, got {:?}", other),
    }
}

#[test]
fn invariant_1_dangling_string_id_in_instance_emission_name() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let item = push_fixture_item(&mut m);
    m.item_def_paths.push(crate::DefPathHash {
        stable_crate_id: StableCrateId(1),
        local_hash: 0,
    });
    m.item_synthetic_kinds.push(Default::default());
    m.domain_attrs.item_attrs.push(Vec::new());
    let _ = m.types.push(Ty::Bool);
    let sig = m.sigs.intern(default_sig());
    m.sig_c_abi_lowerings.push(None);
    m.instances.push(Instance {
        item,
        generic_args: vec![],
        emission_name: StringId::from_u32(9999),
        span: Span {
            file: SourceFileId::SYNTHESIZED,
            lo: 0,
            hi: 0,
        },
        kind: function_kind(sig, None),
    });
    match m.validate(&BridgeInput::matching(&m)).unwrap_err() {
        InvariantViolation::DanglingId { field, value, .. } => {
            assert!(field.contains("Instance.emission_name"), "field: {field}");
            assert_eq!(value, 9999);
        }
        other => panic!("expected DanglingId on emission_name, got {:?}", other),
    }
}

#[test]
fn invariant_1_dangling_const_id_in_static_init() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let item = push_fixture_item(&mut m);
    m.item_def_paths.push(crate::DefPathHash {
        stable_crate_id: StableCrateId(1),
        local_hash: 0,
    });
    m.item_synthetic_kinds.push(Default::default());
    m.domain_attrs.item_attrs.push(Vec::new());
    let ty = m.types.push(Ty::Bool);
    m.instances.push(Instance {
        item,
        generic_args: vec![],
        emission_name: StringId::ZERO,
        span: Span {
            file: SourceFileId::SYNTHESIZED,
            lo: 0,
            hi: 0,
        },
        kind: InstanceKind::Static {
            ty,
            // No consts pushed -> ConstId(0) is dangling.
            init: Some(ConstId::from_u32(0)),
        },
    });
    match m.validate(&BridgeInput::matching(&m)).unwrap_err() {
        InvariantViolation::DanglingId { field, value, .. } => {
            assert!(field.contains("Static.init"), "field: {field}");
            assert_eq!(value, 0);
        }
        other => panic!("expected DanglingId on Static.init, got {:?}", other),
    }
}

#[test]
fn invariant_1_dangling_ty_id_in_const_kind() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let item = push_fixture_item(&mut m);
    m.item_def_paths.push(crate::DefPathHash {
        stable_crate_id: StableCrateId(1),
        local_hash: 0,
    });
    m.item_synthetic_kinds.push(Default::default());
    m.domain_attrs.item_attrs.push(Vec::new());
    m.instances.push(Instance {
        item,
        generic_args: vec![],
        emission_name: StringId::ZERO,
        span: Span {
            file: SourceFileId::SYNTHESIZED,
            lo: 0,
            hi: 0,
        },
        kind: InstanceKind::Const {
            ty: TyId::from_u32(0),
            value: ConstId::from_u32(0),
        },
    });
    match m.validate(&BridgeInput::matching(&m)).unwrap_err() {
        InvariantViolation::DanglingId { field, value, .. } => {
            assert!(field.contains("Const.ty"), "field: {field}");
            assert_eq!(value, 0);
        }
        other => panic!("expected DanglingId on Const.ty, got {:?}", other),
    }
}

#[test]
fn invariant_1_dangling_const_id_in_const_kind_value() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let item = push_fixture_item(&mut m);
    m.item_def_paths.push(crate::DefPathHash {
        stable_crate_id: StableCrateId(1),
        local_hash: 0,
    });
    m.item_synthetic_kinds.push(Default::default());
    m.domain_attrs.item_attrs.push(Vec::new());
    let ty = m.types.push(Ty::Bool);
    m.instances.push(Instance {
        item,
        generic_args: vec![],
        emission_name: StringId::ZERO,
        span: Span {
            file: SourceFileId::SYNTHESIZED,
            lo: 0,
            hi: 0,
        },
        kind: InstanceKind::Const {
            ty,
            // No consts pushed -> ConstId(0) is dangling.
            value: ConstId::from_u32(0),
        },
    });
    match m.validate(&BridgeInput::matching(&m)).unwrap_err() {
        InvariantViolation::DanglingId { field, value, .. } => {
            assert!(field.contains("Const.value"), "field: {field}");
            assert_eq!(value, 0);
        }
        other => panic!("expected DanglingId on Const.value, got {:?}", other),
    }
}

// ------------------------------------------------------------------
// JSON round-trip on the top-level struct.
// ------------------------------------------------------------------

#[test]
fn empty_model_round_trips_through_json() {
    let m = RegseqModel::with_primary("pl011-spec", StableCrateId(0xAAAA));
    let json = serde_json::to_string(&m).unwrap();
    let back: RegseqModel = serde_json::from_str(&json).unwrap();
    assert_eq!(m, back);
}

#[test]
fn populated_model_round_trips_through_json() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(0xAAAA));
    let _ = m.types.push(Ty::Bool);
    let _ = m.consts.intern(crate::Const::ZST);
    let sig = m.sigs.intern(default_sig());
    m.sig_c_abi_lowerings.push(None);
    let item = push_fixture_item(&mut m);
    m.item_def_paths.push(crate::DefPathHash {
        stable_crate_id: StableCrateId(0xAAAA),
        local_hash: 0xCAFE,
    });
    m.item_synthetic_kinds.push(Default::default());
    m.domain_attrs.item_attrs.push(Vec::new());
    let symbol = m.strings.intern("foo");
    m.instances.push(Instance {
        item,
        generic_args: vec![],
        emission_name: symbol,
        span: Span {
            file: SourceFileId::SYNTHESIZED,
            lo: 0,
            hi: 0,
        },
        kind: function_kind(sig, Some(Body::default())),
    });

    let json = serde_json::to_string(&m).unwrap();
    let mut back: RegseqModel = serde_json::from_str(&json).unwrap();
    assert_eq!(m, back);
    assert_eq!(back.validate(&BridgeInput::matching(&back)), Ok(()));
}

#[test]
fn vtables_are_skipped_during_json_round_trip() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(0xAAAA));
    m.vtables.insert(
        crate::PrincipalTraitKey {
            trait_path: crate::DefPathHash {
                stable_crate_id: StableCrateId(1),
                local_hash: 2,
            },
            trait_args: crate::ArgsKey::EMPTY,
            projection_bindings: crate::ProjectionBindingsKey(0),
        },
        ItemId::ZERO,
    );

    let value = serde_json::to_value(&m).unwrap();
    assert!(value.get("vtables").is_none());

    let back: RegseqModel = serde_json::from_value(value).unwrap();
    assert!(back.vtables.is_empty());
}

#[test]
fn model_json_top_level_shape_has_named_fields() {
    let m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let v = serde_json::to_value(&m).unwrap();
    assert!(v.is_object(), "RegseqModel serializes to a JSON object");
    for field in [
        "crates",
        "primary_crate",
        "target",
        "build",
        "strings",
        "source_files",
        "types",
        "consts",
        "items",
        "item_def_paths",
        "item_synthetic_kinds",
        "sigs",
        "sig_c_abi_lowerings",
        "instances",
        "domain_attrs",
    ] {
        assert!(v.get(field).is_some(), "missing field: {field}");
    }
}

#[test]
fn invariant_registry_has_full_non_empty_descriptors() {
    let ids = all_invariant_ids();
    assert_eq!(ids.len(), 23);
    for id in ids {
        assert!(!crate::invariant_descriptor(id).is_empty());
    }
}

#[test]
fn invariant_registry_match_is_closed() {
    for id in all_invariant_ids() {
        let n = match id {
            InvariantId::TablesPopulatedBeforeReference => 1,
            InvariantId::ItemSideTablesMirrorItems => 2,
            InvariantId::ItemsSortedAtConstruction => 3,
            InvariantId::PrimaryCrateValidAndMatchesSpec => 4,
            InvariantId::StableCrateIdsUnique => 5,
            InvariantId::DefPathCrateIdsResolve => 6,
            InvariantId::NoTcxReferences => 7,
            InvariantId::SynthesizedSourceFileSentinel => 8,
            InvariantId::InstanceClosureCompleteness => 9,
            InvariantId::FunctionBodyPresenceMatchesKind => 10,
            InvariantId::ContentKeyedInternerUniqueness => 11,
            InvariantId::UnsizedMarkerPlacement => 12,
            InvariantId::DynamicWellFormedness => 13,
            InvariantId::InstanceKindLayoutConsistency => 14,
            InvariantId::TargetInfoWellFormedness => 15,
            InvariantId::CAbiLoweringRecordsMatchSignatures => 16,
            InvariantId::BuildInfoWellFormedness => 17,
            InvariantId::ItemDefPathGlobalUniqueness => 18,
            InvariantId::OpaqueHandleEligibilityConsistency => 19,
            InvariantId::StableCrateIdZeroReserved => 20,
            InvariantId::DiagLogCallArg0IsStrRef => 21,
            InvariantId::DiagLogCallArityFloor => 22,
            InvariantId::DiagLogCallArgAdmissibleLogArgSource => 23,
        };
        assert!((1..=23).contains(&n));
    }
}

fn all_invariant_ids() -> [InvariantId; 23] {
    [
        InvariantId::TablesPopulatedBeforeReference,
        InvariantId::ItemSideTablesMirrorItems,
        InvariantId::ItemsSortedAtConstruction,
        InvariantId::PrimaryCrateValidAndMatchesSpec,
        InvariantId::StableCrateIdsUnique,
        InvariantId::DefPathCrateIdsResolve,
        InvariantId::NoTcxReferences,
        InvariantId::SynthesizedSourceFileSentinel,
        InvariantId::InstanceClosureCompleteness,
        InvariantId::FunctionBodyPresenceMatchesKind,
        InvariantId::ContentKeyedInternerUniqueness,
        InvariantId::UnsizedMarkerPlacement,
        InvariantId::DynamicWellFormedness,
        InvariantId::InstanceKindLayoutConsistency,
        InvariantId::TargetInfoWellFormedness,
        InvariantId::CAbiLoweringRecordsMatchSignatures,
        InvariantId::BuildInfoWellFormedness,
        InvariantId::ItemDefPathGlobalUniqueness,
        InvariantId::OpaqueHandleEligibilityConsistency,
        InvariantId::StableCrateIdZeroReserved,
        InvariantId::DiagLogCallArg0IsStrRef,
        InvariantId::DiagLogCallArityFloor,
        InvariantId::DiagLogCallArgAdmissibleLogArgSource,
    ]
}

fn helper_fixture() -> (RegseqModel, InstanceId) {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let item = push_fixture_item(&mut m);
    m.item_def_paths.push(crate::DefPathHash {
        stable_crate_id: StableCrateId(1),
        local_hash: 0,
    });
    m.item_synthetic_kinds.push(Default::default());
    m.domain_attrs.item_attrs.push(Vec::new());
    let sig = m.sigs.intern(default_sig());
    m.sig_c_abi_lowerings.push(None);
    let id = m.instances.push(Instance {
        item,
        generic_args: vec![],
        emission_name: StringId::ZERO,
        span: Span {
            file: SourceFileId::SYNTHESIZED,
            lo: 0,
            hi: 0,
        },
        kind: function_kind(sig, Some(Body::default())),
    });
    (m, id)
}

#[test]
fn instance_fn_sig_helper_returns_function_signature() {
    let (m, id) = helper_fixture();
    assert!(m.instance_fn_sig(id).is_some());
}

#[test]
fn instance_body_helper_returns_inline_body() {
    let (m, id) = helper_fixture();
    assert!(m.instance_body(id).is_some());
}

#[test]
fn instance_def_path_helper_signature_compiles() {
    let (m, id) = helper_fixture();
    assert_eq!(
        m.instance_def_path(id),
        &m.items[m.instances[id].item].def_path
    );
}

#[test]
fn instance_span_helper_signature_compiles() {
    let (m, id) = helper_fixture();
    assert_eq!(m.instance_span(id), m.items[m.instances[id].item].span);
}

#[test]
fn instance_linkage_helper_signature_compiles() {
    let (m, id) = helper_fixture();
    assert_eq!(m.instance_linkage(id), Linkage::Default);
}

#[test]
fn instance_link_section_helper_signature_compiles() {
    let (m, id) = helper_fixture();
    assert_eq!(m.instance_link_section(id), None);
}

#[test]
fn instance_inline_hint_helper_signature_compiles() {
    let (m, id) = helper_fixture();
    assert_eq!(m.instance_inline_hint(id), InlineHint::None);
}

#[test]
fn instance_is_unsafe_helper_signature_compiles() {
    let (m, id) = helper_fixture();
    assert!(!m.instance_is_unsafe(id));
}

#[test]
fn instance_lang_item_helpers_signature_compile() {
    let (m, id) = helper_fixture();
    assert!(!m.instance_is_phantom_data(id));
    assert!(!m.instance_is_unsafe_cell(id));
    assert!(!m.instance_is_manually_drop(id));
}

#[test]
fn instance_opaque_handle_eligible_helper_signature_compiles() {
    let (m, id) = helper_fixture();
    assert!(!m.instance_opaque_handle_eligible(id));
}

// ------------------------------------------------------------------
// regseq-driver-49 Change A: instance_intrinsic helper
// (spec.md section 3.5.2 + section 3.6.4).
// ------------------------------------------------------------------

#[test]
fn instance_intrinsic_returns_log_kind_when_attr_present() {
    let (mut m, id) = helper_fixture();
    let item = m.instances[id].item;
    m.domain_attrs.item_attrs[item].push(crate::AttrData::Intrinsic(crate::IntrinsicKind::Log(
        crate::LogLevel::Info,
    )));
    assert_eq!(
        m.instance_intrinsic(id),
        Some(&crate::IntrinsicKind::Log(crate::LogLevel::Info))
    );
}

#[test]
fn instance_intrinsic_returns_none_when_item_attrs_empty() {
    let (m, id) = helper_fixture();
    // helper_fixture pushes an empty Vec onto item_attrs.
    let item = m.instances[id].item;
    assert!(m.domain_attrs.item_attrs[item].is_empty());
    assert_eq!(m.instance_intrinsic(id), None);
}

#[test]
fn instance_intrinsic_returns_none_for_out_of_range_instance_id() {
    let (m, _id) = helper_fixture();
    let ghost = InstanceId::from_u32(999);
    assert!(ghost.index() >= m.instances.len());
    assert_eq!(m.instance_intrinsic(ghost), None);
}

#[test]
fn instance_intrinsic_returns_none_when_item_attrs_table_too_short() {
    // Guard the defensive `item.index() >= item_attrs.len()` branch.
    let (mut m, id) = helper_fixture();
    // Empty the side table so the Instance's item index is now out
    // of range vs item_attrs.
    m.domain_attrs.item_attrs.replace_inner(Vec::new());
    assert_eq!(m.instance_intrinsic(id), None);
}

#[test]
fn instance_intrinsic_stable_across_f4_2_permutation() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    // Build two function instances with distinct rendered names so
    // the F4-2 ordering produces a deterministic non-identity sort.
    let beta_leaf = m.strings.intern("beta");
    let beta_rendered = m.strings.intern("pl011-spec::beta");
    let alpha_leaf = m.strings.intern("alpha");
    let alpha_rendered = m.strings.intern("pl011-spec::alpha");
    let mut beta = fixture_item(&mut m);
    beta.def_path.segments[1] = DefPathSegment {
        data: DefPathData::ValueNs(beta_leaf),
        disambiguator: 0,
    };
    beta.def_path.rendered = beta_rendered;
    let beta_item = m.items.push(beta);
    m.item_def_paths.push(crate::DefPathHash {
        stable_crate_id: StableCrateId(1),
        local_hash: 1,
    });
    m.item_synthetic_kinds.push(Default::default());
    m.domain_attrs
        .item_attrs
        .push(vec![crate::AttrData::Intrinsic(crate::IntrinsicKind::Log(
            crate::LogLevel::Warn,
        ))]);

    let mut alpha = fixture_item(&mut m);
    alpha.def_path.segments[1] = DefPathSegment {
        data: DefPathData::ValueNs(alpha_leaf),
        disambiguator: 0,
    };
    alpha.def_path.rendered = alpha_rendered;
    let alpha_item = m.items.push(alpha);
    m.item_def_paths.push(crate::DefPathHash {
        stable_crate_id: StableCrateId(1),
        local_hash: 2,
    });
    m.item_synthetic_kinds.push(Default::default());
    m.domain_attrs
        .item_attrs
        .push(vec![crate::AttrData::Intrinsic(crate::IntrinsicKind::Log(
            crate::LogLevel::Debug,
        ))]);

    let sig = m.sigs.intern(default_sig());
    m.sig_c_abi_lowerings.push(None);

    let beta_inst = m.instances.push(Instance {
        item: beta_item,
        generic_args: vec![],
        emission_name: m.strings.intern("beta_sym"),
        span: Span {
            file: SourceFileId::SYNTHESIZED,
            lo: 0,
            hi: 0,
        },
        kind: function_kind(sig, None),
    });
    let alpha_inst = m.instances.push(Instance {
        item: alpha_item,
        generic_args: vec![],
        emission_name: m.strings.intern("alpha_sym"),
        span: Span {
            file: SourceFileId::SYNTHESIZED,
            lo: 0,
            hi: 0,
        },
        kind: function_kind(sig, None),
    });

    // Pre-permutation observations.
    let beta_kind_before = m.instance_intrinsic(beta_inst).cloned();
    let alpha_kind_before = m.instance_intrinsic(alpha_inst).cloned();
    assert_eq!(
        beta_kind_before,
        Some(crate::IntrinsicKind::Log(crate::LogLevel::Warn))
    );
    assert_eq!(
        alpha_kind_before,
        Some(crate::IntrinsicKind::Log(crate::LogLevel::Debug))
    );

    // `sort_instances_by_f4_2` permutes in place; recover the new
    // ids by matching the surviving Instance.item against the items
    // we constructed above (each item appears in exactly one
    // Instance in this fixture).
    m.sort_instances_by_f4_2();
    let beta_after = (0..m.instances.len())
        .map(|i| InstanceId::from_u32(i as u32))
        .find(|id| m.instances[*id].item == beta_item)
        .expect("beta instance survives");
    let alpha_after = (0..m.instances.len())
        .map(|i| InstanceId::from_u32(i as u32))
        .find(|id| m.instances[*id].item == alpha_item)
        .expect("alpha instance survives");
    assert_eq!(m.instance_intrinsic(beta_after).cloned(), beta_kind_before);
    assert_eq!(
        m.instance_intrinsic(alpha_after).cloned(),
        alpha_kind_before
    );
}

// ------------------------------------------------------------------
// regseq-driver-47: sort_items_by_f4 + global ItemId remap
// (spec.md section 3.4 Phase 3/4 + section 16.1).
// ------------------------------------------------------------------

mod sort_items_by_f4 {
    use super::*;
    use crate::{
        domain_attr::{AttrData, FieldPath, IntrinsicKind, LogLevel},
        ids::{FieldIdx, VariantIdx},
        synthetic::{ContentHash, PrincipalTraitKey, ProjectionBindingsKey, SyntheticItemKind},
        ArgsKey, DefPathHash,
    };

    fn named_item(model: &mut RegseqModel, leaf: &str) -> Item {
        let leaf_id = model.strings.intern(leaf);
        let rendered = model.strings.intern(&format!("pl011-spec::{leaf}"));
        Item {
            def_path: DefPath {
                crate_id: model.primary_crate,
                segments: vec![
                    DefPathSegment {
                        data: DefPathData::CrateRoot,
                        disambiguator: 0,
                    },
                    DefPathSegment {
                        data: DefPathData::ValueNs(leaf_id),
                        disambiguator: 0,
                    },
                ],
                rendered,
            },
            kind: ItemKind::Function,
            span: Span {
                file: SourceFileId::SYNTHESIZED,
                lo: 0,
                hi: 0,
            },
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

    fn push_named(model: &mut RegseqModel, leaf: &str, local_hash: u64) -> ItemId {
        let item = named_item(model, leaf);
        let id = model.items.push(item);
        model.item_def_paths.push(DefPathHash {
            stable_crate_id: StableCrateId(1),
            local_hash,
        });
        model.item_synthetic_kinds.push(SyntheticItemKind::None);
        model.domain_attrs.item_attrs.push(Vec::new());
        id
    }

    fn principal(local_hash: u64) -> PrincipalTraitKey {
        PrincipalTraitKey {
            trait_path: DefPathHash {
                stable_crate_id: StableCrateId(2),
                local_hash,
            },
            trait_args: ArgsKey::EMPTY,
            projection_bindings: ProjectionBindingsKey(0),
        }
    }

    fn field_path(item: ItemId) -> FieldPath {
        FieldPath {
            item,
            variant: VariantIdx::from_u32(0),
            field: FieldIdx::from_u32(0),
        }
    }

    fn attr_vec() -> Vec<AttrData> {
        vec![AttrData::Intrinsic(IntrinsicKind::Log(LogLevel::Info))]
    }

    // Test Plan item 1.
    #[test]
    fn already_sorted_model_is_unchanged_by_sort() {
        let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
        let _alpha = push_named(&mut m, "alpha", 1);
        let _beta = push_named(&mut m, "beta", 2);
        let before_items = m.items.clone();
        let before_def_paths = m.item_def_paths.clone();
        let before_synth = m.item_synthetic_kinds.clone();
        let remap = m.sort_items_by_f4();
        assert_eq!(
            remap,
            vec![ItemId::from_u32(0), ItemId::from_u32(1)],
            "idempotent sort returns identity remap"
        );
        assert_eq!(m.items, before_items);
        assert_eq!(m.item_def_paths, before_def_paths);
        assert_eq!(m.item_synthetic_kinds, before_synth);
    }

    // Test Plan item 2.
    #[test]
    fn two_item_reverse_order_sorts_and_remaps_instance_item() {
        let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
        let beta = push_named(&mut m, "beta", 1);
        let alpha = push_named(&mut m, "alpha", 2);
        let sig = m.sigs.intern(default_sig());
        m.sig_c_abi_lowerings.push(None);
        let beta_inst = m.instances.push(Instance {
            item: beta,
            generic_args: vec![],
            emission_name: StringId::ZERO,
            span: Span {
                file: SourceFileId::SYNTHESIZED,
                lo: 0,
                hi: 0,
            },
            kind: function_kind(sig, None),
        });
        let alpha_inst = m.instances.push(Instance {
            item: alpha,
            generic_args: vec![],
            emission_name: StringId::ZERO,
            span: Span {
                file: SourceFileId::SYNTHESIZED,
                lo: 0,
                hi: 0,
            },
            kind: function_kind(sig, None),
        });

        let remap = m.sort_items_by_f4();

        // Items now in F4 order (alpha, beta).
        let leaf_at = |id: ItemId| -> String {
            let segment = &m.items[id].def_path.segments[1];
            match &segment.data {
                DefPathData::ValueNs(name) => m.strings.resolve(*name).to_string(),
                other => panic!("expected ValueNs segment, got {:?}", other),
            }
        };
        assert_eq!(leaf_at(ItemId::from_u32(0)), "alpha");
        assert_eq!(leaf_at(ItemId::from_u32(1)), "beta");
        // Old beta (ItemId 0) was permuted to ItemId 1.
        assert_eq!(remap[beta.index()], ItemId::from_u32(1));
        assert_eq!(remap[alpha.index()], ItemId::from_u32(0));
        assert_eq!(m.instances[beta_inst].item, ItemId::from_u32(1));
        assert_eq!(m.instances[alpha_inst].item, ItemId::from_u32(0));
    }

    // Test Plan item 3: every nominal Ty variant
    // (Struct/Enum/Union/FnDef/Closure) carrying an ItemId must be
    // remapped in lock-step with the items permutation per
    // .harness/tasks/regseq-driver/47-...md Implementation Notes A.2
    // step 5 (Ty::Enum/Struct/Union/FnDef/Closure).
    #[test]
    fn ty_nominal_item_id_remap_lockstep_with_items() {
        let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
        let beta = push_named(&mut m, "beta", 1);
        let alpha = push_named(&mut m, "alpha", 2);
        let trivial_layout = crate::Layout {
            size: 0,
            align: 1,
            pack: None,
            uninhabited: false,
            kind: crate::LayoutKind::Struct { fields: vec![] },
        };
        let struct_ty = m.types.intern(Ty::Struct {
            item: beta,
            args: vec![],
            layout: trivial_layout.clone(),
        });
        let enum_ty = m.types.intern(Ty::Enum {
            item: beta,
            args: vec![],
            layout: trivial_layout.clone(),
        });
        let union_ty = m.types.intern(Ty::Union {
            item: beta,
            args: vec![],
            layout: trivial_layout,
        });
        let fn_def_ty = m.types.intern(Ty::FnDef(alpha, vec![]));
        let closure_ty = m.types.intern(Ty::Closure(alpha, vec![]));

        let remap = m.sort_items_by_f4();
        assert_eq!(remap[beta.index()], ItemId::from_u32(1));
        assert_eq!(remap[alpha.index()], ItemId::from_u32(0));

        match m.types.resolve(struct_ty) {
            Ty::Struct { item, .. } => assert_eq!(*item, ItemId::from_u32(1)),
            other => panic!("expected Ty::Struct, got {:?}", other),
        }
        match m.types.resolve(enum_ty) {
            Ty::Enum { item, .. } => assert_eq!(*item, ItemId::from_u32(1)),
            other => panic!("expected Ty::Enum, got {:?}", other),
        }
        match m.types.resolve(union_ty) {
            Ty::Union { item, .. } => assert_eq!(*item, ItemId::from_u32(1)),
            other => panic!("expected Ty::Union, got {:?}", other),
        }
        match m.types.resolve(fn_def_ty) {
            Ty::FnDef(item, _) => assert_eq!(*item, ItemId::from_u32(0)),
            other => panic!("expected Ty::FnDef, got {:?}", other),
        }
        match m.types.resolve(closure_ty) {
            Ty::Closure(item, _) => assert_eq!(*item, ItemId::from_u32(0)),
            other => panic!("expected Ty::Closure, got {:?}", other),
        }

        // Lookup map has been rebuilt: find_nominal under the new
        // ItemIds resolves each entry to its original TyId.
        assert_eq!(
            m.types
                .find_nominal(crate::NominalKind::Struct, ItemId::from_u32(1), &vec![]),
            Some(struct_ty)
        );
        assert_eq!(
            m.types
                .find_nominal(crate::NominalKind::Enum, ItemId::from_u32(1), &vec![]),
            Some(enum_ty)
        );
        assert_eq!(
            m.types
                .find_nominal(crate::NominalKind::Union, ItemId::from_u32(1), &vec![]),
            Some(union_ty)
        );
        assert_eq!(
            m.types
                .find_nominal(crate::NominalKind::FnDef, ItemId::from_u32(0), &vec![]),
            Some(fn_def_ty)
        );
        assert_eq!(
            m.types
                .find_nominal(crate::NominalKind::Closure, ItemId::from_u32(0), &vec![]),
            Some(closure_ty)
        );
    }

    // Test Plan item 4.
    #[test]
    fn vtables_hashmap_value_is_remapped() {
        let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
        let beta = push_named(&mut m, "beta", 1);
        let _alpha = push_named(&mut m, "alpha", 2);
        m.vtables.insert(principal(7), beta);
        let _ = m.sort_items_by_f4();
        assert_eq!(m.vtables.get(&principal(7)), Some(&ItemId::from_u32(1)));
    }

    // Test Plan item 5: every IndexVec<ItemId, _> side table must be
    // permuted (not remapped) by sort_items_by_f4 per
    // .harness/tasks/regseq-driver/47-...md Implementation Notes A.2
    // step 3: `item_def_paths`, `item_synthetic_kinds`, and
    // `domain_attrs.item_attrs`.
    #[test]
    fn indexvec_side_tables_are_permuted_in_lockstep() {
        let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
        let gamma = push_named(&mut m, "gamma", 1);
        let alpha = push_named(&mut m, "alpha", 2);
        let beta = push_named(&mut m, "beta", 3);

        let gamma_kind = SyntheticItemKind::ConstMemoryStatic {
            content_hash: ContentHash(0x11),
        };
        let alpha_kind = SyntheticItemKind::ConstMemoryStatic {
            content_hash: ContentHash(0x22),
        };
        let beta_kind = SyntheticItemKind::ConstMemoryStatic {
            content_hash: ContentHash(0x33),
        };
        m.item_synthetic_kinds[gamma] = gamma_kind.clone();
        m.item_synthetic_kinds[alpha] = alpha_kind.clone();
        m.item_synthetic_kinds[beta] = beta_kind.clone();

        let gamma_def_path = m.item_def_paths[gamma];
        let alpha_def_path = m.item_def_paths[alpha];
        let beta_def_path = m.item_def_paths[beta];

        let gamma_attrs = vec![AttrData::Intrinsic(IntrinsicKind::Log(LogLevel::Debug))];
        let alpha_attrs = vec![AttrData::Intrinsic(IntrinsicKind::Log(LogLevel::Info))];
        let beta_attrs = vec![AttrData::Intrinsic(IntrinsicKind::Log(LogLevel::Warn))];
        m.domain_attrs.item_attrs[gamma] = gamma_attrs.clone();
        m.domain_attrs.item_attrs[alpha] = alpha_attrs.clone();
        m.domain_attrs.item_attrs[beta] = beta_attrs.clone();

        let remap = m.sort_items_by_f4();
        // After F4 sort alpha < beta < gamma by leaf-segment name.
        assert_eq!(remap[alpha.index()], ItemId::from_u32(0));
        assert_eq!(remap[beta.index()], ItemId::from_u32(1));
        assert_eq!(remap[gamma.index()], ItemId::from_u32(2));

        // item_synthetic_kinds (IndexVec<ItemId, SyntheticItemKind>).
        assert_eq!(m.item_synthetic_kinds[remap[alpha.index()]], alpha_kind);
        assert_eq!(m.item_synthetic_kinds[remap[beta.index()]], beta_kind);
        assert_eq!(m.item_synthetic_kinds[remap[gamma.index()]], gamma_kind);

        // item_def_paths (IndexVec<ItemId, DefPathHash>).
        assert_eq!(m.item_def_paths[remap[alpha.index()]], alpha_def_path);
        assert_eq!(m.item_def_paths[remap[beta.index()]], beta_def_path);
        assert_eq!(m.item_def_paths[remap[gamma.index()]], gamma_def_path);

        // domain_attrs.item_attrs (IndexVec<ItemId, Vec<AttrData>>).
        assert_eq!(m.domain_attrs.item_attrs[remap[alpha.index()]], alpha_attrs);
        assert_eq!(m.domain_attrs.item_attrs[remap[beta.index()]], beta_attrs);
        assert_eq!(m.domain_attrs.item_attrs[remap[gamma.index()]], gamma_attrs);
    }

    // Test Plan item 6.
    #[test]
    fn domain_attrs_field_attrs_key_is_remapped() {
        let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
        let beta = push_named(&mut m, "beta", 1);
        let _alpha = push_named(&mut m, "alpha", 2);
        let value = attr_vec();
        m.domain_attrs
            .field_attrs
            .insert(field_path(beta), value.clone());

        let remap = m.sort_items_by_f4();
        // beta moved from ItemId(0) to ItemId(1).
        assert_eq!(remap[beta.index()], ItemId::from_u32(1));
        assert_eq!(
            m.domain_attrs
                .field_attrs
                .get(&field_path(ItemId::from_u32(1))),
            Some(&value)
        );
        assert!(m
            .domain_attrs
            .field_attrs
            .get(&field_path(ItemId::from_u32(0)))
            .is_none());
    }

    // Test Plan item 7 is bridge-side (HandleTable::remap_item_ids)
    // and lives in the regseq-driver test surface so it can reach
    // `HandleTable` without inverting the crate dependency
    // (regseq-model does not depend on regseq-driver).

    // Test Plan item 8.
    #[test]
    fn post_sort_f4_monotonicity_debug_check_passes() {
        let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
        let _beta = push_named(&mut m, "beta", 1);
        let _alpha = push_named(&mut m, "alpha", 2);
        let _gamma = push_named(&mut m, "gamma", 3);
        // The implementation runs its debug-build monotonicity check
        // inside sort_items_by_f4; this call must not panic.
        let _ = m.sort_items_by_f4();
        // Re-validate via the public validator -- the F4 monotonicity
        // check fires here too (validate_f4_item_order).
        assert_eq!(m.validate(&BridgeInput::matching(&m)), Ok(()));
    }
}

// ------------------------------------------------------------------
// regseq-driver-48: removal helpers + Ty/FnSig walkers
// (spec.md section 26.6 carrier-survival contract).
// ------------------------------------------------------------------

mod dead_item_elim_helpers {
    use super::*;
    use crate::cabi::{CAbiLowering, CAbiShape, SubstReason};
    use crate::ids::{BlockId, FieldIdx, LocalId, VariantIdx};
    use crate::synthetic::SyntheticItemKind;
    use crate::Const;
    use crate::{
        AggregateKind, AssertKind, BasicBlock, BinOp, CallCallee, CastKind, CopyNonOverlapping,
        DefPathHash, EnumShape, FieldLayout, FloatKind, IndexVec, IntKind, Layout, LayoutKind,
        LocalDecl, Mutability, NonDivergingIntrinsic, Operand, Place, PlaceElem, PointerCoercion,
        Provenance, Rvalue, Statement, StatementKind, SwitchTargets, TagEncoding, TagScalar,
        Terminator, TerminatorKind, UintKind, UnOp, VariantLayout, VariantRange, WrappingRange,
    };

    fn named_item(model: &mut RegseqModel, leaf: &str, kind: ItemKind) -> ItemId {
        let leaf_id = model.strings.intern(leaf);
        let rendered = model.strings.intern(&format!("pl011-spec::{leaf}"));
        let item = Item {
            def_path: DefPath {
                crate_id: model.primary_crate,
                segments: vec![
                    DefPathSegment {
                        data: DefPathData::CrateRoot,
                        disambiguator: 0,
                    },
                    DefPathSegment {
                        data: DefPathData::TypeNs(leaf_id),
                        disambiguator: 0,
                    },
                ],
                rendered,
            },
            kind,
            span: Span {
                file: SourceFileId::SYNTHESIZED,
                lo: 0,
                hi: 0,
            },
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
        model.item_def_paths.push(DefPathHash {
            stable_crate_id: StableCrateId(1),
            local_hash: id.as_u32() as u64 + 100,
        });
        model.item_synthetic_kinds.push(SyntheticItemKind::None);
        model.domain_attrs.item_attrs.push(Vec::new());
        id
    }

    fn unit_layout() -> Layout {
        crate::UNIT_LAYOUT
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

    fn push_function_instance(
        model: &mut RegseqModel,
        item: ItemId,
        fn_sig: FnSigId,
        body: Option<Body>,
    ) -> InstanceId {
        model.instances.push(Instance {
            item,
            generic_args: vec![],
            emission_name: StringId::ZERO,
            span: Span {
                file: SourceFileId::SYNTHESIZED,
                lo: 0,
                hi: 0,
            },
            kind: InstanceKind::Function(FunctionInstanceData {
                fn_sig,
                body,
                flavor: FunctionInstanceFlavor::Item,
                virtual_idx: None,
            }),
        })
    }

    fn default_unit_sig(model: &mut RegseqModel) -> FnSigId {
        let unit_ty = model.types.intern(Ty::Tuple {
            fields: vec![],
            layout: unit_layout(),
        });
        let sig = model.sigs.intern(FnSig {
            inputs: vec![],
            output: unit_ty,
            abi: FnAbi::Rust,
            c_variadic: false,
            safety: Safety::Safe,
        });
        model.sig_c_abi_lowerings.push(None);
        sig
    }

    // ----------------- Test Plan items 1-4 + 7: removal helpers -----------------

    /// Test Plan item 1: `remove_items_and_remap` -- middle-item
    /// removal compacts `model.items`, every IndexVec<ItemId, _>
    /// side table, and remaps the surviving Instance.item reference.
    #[test]
    fn remove_items_and_remap_compacts_middle_item_and_remaps_reference() {
        let mut m = RegseqModel::with_primary("primary", StableCrateId(1));
        let _item_0 = named_item(&mut m, "item_a", ItemKind::Function);
        let _item_1 = named_item(&mut m, "item_b", ItemKind::Function);
        let item_2 = named_item(&mut m, "item_c", ItemKind::Function);
        let sig = default_unit_sig(&mut m);
        let _inst = push_function_instance(&mut m, item_2, sig, None);

        let item_keep = vec![true, false, true];
        let remap = m.remove_items_and_remap(&item_keep);
        assert_eq!(remap.len(), 3);
        assert_eq!(remap[0], Some(ItemId::from_u32(0)));
        assert_eq!(remap[1], None);
        assert_eq!(remap[2], Some(ItemId::from_u32(1)));
        assert_eq!(m.items.len(), 2);
        assert_eq!(m.item_def_paths.len(), 2);
        assert_eq!(m.item_synthetic_kinds.len(), 2);
        assert_eq!(m.domain_attrs.item_attrs.len(), 2);
        // The surviving Instance now references the new index of the
        // former item_2 (post-compaction position 1).
        assert_eq!(m.instances[InstanceId::ZERO].item, ItemId::from_u32(1));
    }

    /// Test Plan item 2: `remove_instances_and_remap` -- removal
    /// with surviving Call terminator remap.
    #[test]
    fn remove_instances_and_remap_remaps_call_callee_inside_body() {
        let mut m = RegseqModel::with_primary("primary", StableCrateId(1));
        let item = named_item(&mut m, "caller", ItemKind::Function);
        let sig = default_unit_sig(&mut m);

        let _inst_0 = push_function_instance(&mut m, item, sig, None);
        let _inst_1 = push_function_instance(&mut m, item, sig, None);
        let inst_2 = push_function_instance(&mut m, item, sig, None);

        // Body containing a Call terminator targeting Instance 2.
        let mut basic_blocks = IndexVec::new();
        basic_blocks.push(BasicBlock {
            statements: Box::new([]),
            terminator: Terminator {
                kind: TerminatorKind::Call {
                    callee: CallCallee::Direct(inst_2),
                    args: Box::new([]),
                    destination: Place::from(LocalId::RETURN_PLACE),
                    target: None,
                },
                span: Span {
                    file: SourceFileId::SYNTHESIZED,
                    lo: 0,
                    hi: 0,
                },
            },
        });
        let unit_ty = m.types.intern(Ty::Tuple {
            fields: vec![],
            layout: unit_layout(),
        });
        let mut local_decls = IndexVec::new();
        local_decls.push(LocalDecl {
            ty: unit_ty,
            mutability: Mutability::Mut,
            span: Span::dummy(),
            debug_name: None,
        });
        let body = Body {
            basic_blocks,
            local_decls,
            arg_count: 0,
            span: Span::dummy(),
        };
        let inst_with_body = push_function_instance(&mut m, item, sig, Some(body));

        // Remove Instance 1.
        let mut keep = vec![true; m.instances.len()];
        keep[1] = false;
        let remap = m.remove_instances_and_remap(&keep);
        assert_eq!(remap[1], None);
        assert_eq!(remap[2], Some(InstanceId::from_u32(1)));

        // The body's Call callee was remapped from Instance 2 to
        // the new position 1 (post-compaction).
        let new_inst_with_body = match remap[inst_with_body.index()] {
            Some(id) => id,
            None => panic!("expected the body-carrying instance to survive"),
        };
        let kind = &m.instances[new_inst_with_body].kind;
        let body_ref = kind.as_function().unwrap().body.as_ref().unwrap();
        match &body_ref.basic_blocks[BlockId::from_u32(0)].terminator.kind {
            TerminatorKind::Call {
                callee: CallCallee::Direct(id),
                ..
            } => {
                assert_eq!(*id, InstanceId::from_u32(1));
            }
            other => panic!("expected Call terminator, got {other:?}"),
        }
    }

    /// Test Plan item 3: `remove_tys_and_remap` remaps TyId
    /// references and rebuilds `closure_captures_struct_ty`.
    #[test]
    fn remove_tys_and_remap_remaps_ty_id_refs_and_rebuilds_capture_map() {
        let mut m = RegseqModel::with_primary("primary", StableCrateId(1));
        // Three Tys: a marker struct (Ty 0), a slated-for-removal
        // marker (Ty 1), and a second struct (Ty 2).
        let item_a = named_item(&mut m, "a", ItemKind::Struct);
        let item_b = named_item(&mut m, "b", ItemKind::Struct);
        let item_c = named_item(&mut m, "c", ItemKind::Struct);
        let ty_0 = m.types.intern(Ty::Struct {
            item: item_a,
            args: vec![],
            layout: empty_struct_layout(),
        });
        let ty_1 = m.types.intern(Ty::Struct {
            item: item_b,
            args: vec![],
            layout: empty_struct_layout(),
        });
        let ty_2 = m.types.intern(Ty::Struct {
            item: item_c,
            args: vec![],
            layout: empty_struct_layout(),
        });
        // Ty::Ref(Ty 2, _).
        let ref_ty = m.types.intern(Ty::Ref(ty_2, Provenance::SharedBorrow));

        // Instance whose Function body has a LocalDecl referencing Ty 2.
        let item_fn = named_item(&mut m, "f", ItemKind::Function);
        let unit_ty = m.types.intern(Ty::Tuple {
            fields: vec![],
            layout: unit_layout(),
        });
        let sig = m.sigs.intern(FnSig {
            inputs: vec![],
            output: unit_ty,
            abi: FnAbi::Rust,
            c_variadic: false,
            safety: Safety::Safe,
        });
        m.sig_c_abi_lowerings.push(None);
        let mut local_decls = IndexVec::new();
        local_decls.push(LocalDecl {
            ty: unit_ty,
            mutability: Mutability::Mut,
            span: Span::dummy(),
            debug_name: None,
        });
        local_decls.push(LocalDecl {
            ty: ty_2,
            mutability: Mutability::Mut,
            span: Span::dummy(),
            debug_name: None,
        });
        let mut basic_blocks = IndexVec::new();
        basic_blocks.push(BasicBlock {
            statements: Box::new([]),
            terminator: Terminator {
                kind: TerminatorKind::Return,
                span: Span::dummy(),
            },
        });
        let body = Body {
            basic_blocks,
            local_decls,
            arg_count: 0,
            span: Span::dummy(),
        };
        let _inst = push_function_instance(&mut m, item_fn, sig, Some(body));

        // closure_captures_struct_ty entry (Ty 0 -> Ty 2). Neither
        // side is slated for removal but both should be remapped.
        m.closure_captures_struct_ty.insert(ty_0, ty_2);

        // Mark Ty 1 (the middle marker) for removal.
        let mut keep = vec![true; m.types.len()];
        keep[ty_1.index()] = false;
        let remap = m.remove_tys_and_remap(&keep);
        assert_eq!(remap[ty_1.index()], None);
        assert!(remap[ty_0.index()].is_some());
        assert!(remap[ty_2.index()].is_some());
        let new_ty_0 = remap[ty_0.index()].unwrap();
        let new_ty_2 = remap[ty_2.index()].unwrap();
        let new_ref_ty = remap[ref_ty.index()].unwrap();

        // LocalDecl.ty is the new index of Ty 2.
        let inst_idx = m.instances.len() - 1;
        let body_ref = m.instances[InstanceId::from_u32(inst_idx as u32)]
            .kind
            .as_function()
            .unwrap()
            .body
            .as_ref()
            .unwrap();
        assert_eq!(body_ref.local_decls[LocalId::from_u32(1)].ty, new_ty_2);

        // The Ty::Ref's inner TyId is the new index of Ty 2.
        match m.types.resolve(new_ref_ty) {
            Ty::Ref(pointee, _) => assert_eq!(*pointee, new_ty_2),
            other => panic!("expected Ty::Ref, got {other:?}"),
        }

        // TyInterner iter() yields the surviving Tys in insertion
        // order minus the removed. Starting Tys: ty_0/ty_1/ty_2
        // (structs), ref_ty, unit_ty = 5; minus ty_1 = 4 surviving.
        assert_eq!(m.types.len(), 4);
        // closure_captures_struct_ty has exactly one entry
        // `(new_ty_0, new_ty_2)`.
        assert_eq!(m.closure_captures_struct_ty.len(), 1);
        assert_eq!(m.closure_captures_struct_ty.get(&new_ty_0), Some(&new_ty_2));
    }

    /// Test Plan item 4: `remove_fn_sigs_and_remap` remaps FnSigId
    /// references and compacts `sig_c_abi_lowerings` in lock-step.
    #[test]
    fn remove_fn_sigs_and_remap_compacts_lockstep_and_remaps_refs() {
        let mut m = RegseqModel::with_primary("primary", StableCrateId(1));
        let u32_ty = m.types.intern(Ty::Uint(UintKind::U32));
        // Two distinct FnSigs.
        let sig_0 = m.sigs.intern(FnSig {
            inputs: vec![u32_ty],
            output: u32_ty,
            abi: FnAbi::Rust,
            c_variadic: false,
            safety: Safety::Safe,
        });
        let sig_1 = m.sigs.intern(FnSig {
            inputs: vec![],
            output: u32_ty,
            abi: FnAbi::Rust,
            c_variadic: false,
            safety: Safety::Safe,
        });
        // sig_c_abi_lowerings populated for both.
        m.sig_c_abi_lowerings.push(Some(CAbiLowering {
            inputs: vec![CAbiShape::Direct],
            output: CAbiShape::Direct,
        }));
        m.sig_c_abi_lowerings.push(Some(CAbiLowering {
            inputs: vec![],
            output: CAbiShape::Substituted {
                emit_as: u32_ty,
                reason: SubstReason::TransparentChain,
            },
        }));

        // Function Instance referencing FnSig 1.
        let item_fn = named_item(&mut m, "f", ItemKind::Function);
        let inst = push_function_instance(&mut m, item_fn, sig_1, None);

        // Ty::FnPtr(FnSig 1).
        let fn_ptr_ty = m.types.intern(Ty::FnPtr(sig_1));

        // Remove FnSig 0.
        let keep = vec![false, true];
        let remap = m.remove_fn_sigs_and_remap(&keep);
        assert_eq!(remap[sig_0.index()], None);
        assert_eq!(remap[sig_1.index()], Some(FnSigId::from_u32(0)));

        // The Instance's fn_sig is the new index of FnSig 1.
        let inst_data = m.instances[inst].kind.as_function().unwrap();
        assert_eq!(inst_data.fn_sig, FnSigId::from_u32(0));

        // Ty::FnPtr's inner FnSigId is the new index of FnSig 1.
        match m.types.resolve(fn_ptr_ty) {
            Ty::FnPtr(sig) => assert_eq!(*sig, FnSigId::from_u32(0)),
            other => panic!("expected Ty::FnPtr, got {other:?}"),
        }

        // sig_c_abi_lowerings compacted in lock-step.
        assert_eq!(m.sig_c_abi_lowerings.len(), m.sigs.len());
        assert_eq!(m.sig_c_abi_lowerings.len(), 1);
        // The surviving entry is the one originally at sig_1.
        let surviving = m.sig_c_abi_lowerings[FnSigId::ZERO]
            .as_ref()
            .expect("FnSig 1 lowering preserved");
        match &surviving.output {
            CAbiShape::Substituted { reason, .. } => {
                assert_eq!(*reason, SubstReason::TransparentChain);
            }
            other => panic!("expected substituted output, got {other:?}"),
        }
    }

    /// Test Plan item 7: empty removal (no-op): every keep entry
    /// is `true` so storage is unchanged and every remap is
    /// identity.
    #[test]
    fn no_op_removal_is_identity_for_every_helper() {
        let mut m = RegseqModel::with_primary("primary", StableCrateId(1));
        let item = named_item(&mut m, "a", ItemKind::Function);
        let sig = default_unit_sig(&mut m);
        let _inst = push_function_instance(&mut m, item, sig, None);

        let items_before = m.items.clone();
        let instances_before = m.instances.clone();
        let types_len_before = m.types.len();
        let sigs_len_before = m.sigs.len();

        let item_remap = m.remove_items_and_remap(&vec![true; m.items.len()]);
        let inst_remap = m.remove_instances_and_remap(&vec![true; m.instances.len()]);
        let ty_remap = m.remove_tys_and_remap(&vec![true; m.types.len()]);
        let sig_remap = m.remove_fn_sigs_and_remap(&vec![true; m.sigs.len()]);

        assert_eq!(m.items, items_before);
        assert_eq!(m.instances, instances_before);
        assert_eq!(m.types.len(), types_len_before);
        assert_eq!(m.sigs.len(), sigs_len_before);

        for (idx, slot) in item_remap.iter().enumerate() {
            assert_eq!(*slot, Some(ItemId::from_u32(idx as u32)));
        }
        for (idx, slot) in inst_remap.iter().enumerate() {
            assert_eq!(*slot, Some(InstanceId::from_u32(idx as u32)));
        }
        for (idx, slot) in ty_remap.iter().enumerate() {
            assert_eq!(*slot, Some(TyId::from_u32(idx as u32)));
        }
        for (idx, slot) in sig_remap.iter().enumerate() {
            assert_eq!(*slot, Some(FnSigId::from_u32(idx as u32)));
        }
    }

    // ----------------- Test Plan items 5-6: walker coverage -----------------

    /// Test Plan item 5: `for_each_ty_id_mut` visits every TyId
    /// reference enumerated in regseq-driver-48 Change D. Build a
    /// model exercising every surface and assert the visited count
    /// matches the expected total.
    #[test]
    fn for_each_ty_id_mut_visits_every_enumerated_surface() {
        let mut m = RegseqModel::with_primary("primary", StableCrateId(1));
        // Primitive seeds.
        let unit_ty = m.types.intern(Ty::Tuple {
            fields: vec![],
            layout: unit_layout(),
        });
        let bool_ty = m.types.intern(Ty::Bool);
        let u32_ty = m.types.intern(Ty::Uint(UintKind::U32));

        // Ty variants carrying TyIds.
        let array_ty = m.types.intern(Ty::Array(u32_ty, 4)); // 1 TyId
        let slice_ty = m.types.intern(Ty::Slice(u32_ty)); // 1 TyId
        let ref_ty = m.types.intern(Ty::Ref(u32_ty, Provenance::SharedBorrow)); // 1 TyId
        let ptr_ty = m.types.intern(Ty::Ptr(u32_ty, Mutability::Not)); // 1 TyId
        let dyn_ty = m.types.intern(Ty::Dynamic {
            auto_traits: vec![],
            vtable_ty: u32_ty,
        }); // 1 TyId
        let tuple_with_layout = m.types.intern(Ty::Tuple {
            fields: vec![u32_ty, bool_ty],
            layout: Layout {
                size: 5,
                align: 4,
                pack: None,
                uninhabited: false,
                kind: LayoutKind::Struct {
                    fields: vec![
                        FieldLayout {
                            name: StringId::ZERO,
                            ty: u32_ty,
                            offset: 0,
                        },
                        FieldLayout {
                            name: StringId::ZERO,
                            ty: bool_ty,
                            offset: 4,
                        },
                    ],
                },
            },
        }); // 2 Tuple.fields + 2 layout fields = 4 TyIds

        // Struct with nominal payload (GenericArgs + Layout).
        let item_struct = named_item(&mut m, "s", ItemKind::Struct);
        let struct_ty = m.types.intern(Ty::Struct {
            item: item_struct,
            args: vec![GenericArg::Type(u32_ty)], // 1 TyId via GenericArg
            layout: Layout {
                size: 4,
                align: 4,
                pack: None,
                uninhabited: false,
                kind: LayoutKind::Struct {
                    fields: vec![FieldLayout {
                        name: StringId::ZERO,
                        ty: u32_ty,
                        offset: 0,
                    }],
                },
            },
        }); // 1 args + 1 layout = 2 TyIds

        // Enum with multiple-variant layout including a TagScalar.
        let item_enum = named_item(&mut m, "e", ItemKind::Enum);
        let enum_ty = m.types.intern(Ty::Enum {
            item: item_enum,
            args: vec![GenericArg::Type(bool_ty)],
            layout: Layout {
                size: 4,
                align: 4,
                pack: None,
                uninhabited: false,
                kind: LayoutKind::Enum {
                    shape: EnumShape::Multiple {
                        tag: TagScalar {
                            offset: 0,
                            scalar_ty: u32_ty,
                            valid_range: WrappingRange { start: 0, end: 1 },
                        },
                        encoding: TagEncoding::Direct,
                        variants: vec![VariantLayout {
                            name: StringId::ZERO,
                            fields: vec![FieldLayout {
                                name: StringId::ZERO,
                                ty: bool_ty,
                                offset: 0,
                            }],
                            uninhabited: false,
                            discriminant: 0,
                            tag_value: None,
                        }],
                    },
                },
            },
        }); // 1 args + 1 tag.scalar_ty + 1 variant field = 3 TyIds

        // Union with simple layout.
        let item_union = named_item(&mut m, "u", ItemKind::Union);
        let union_ty = m.types.intern(Ty::Union {
            item: item_union,
            args: vec![GenericArg::Type(u32_ty)],
            layout: Layout {
                size: 4,
                align: 4,
                pack: None,
                uninhabited: false,
                kind: LayoutKind::Union {
                    fields: vec![FieldLayout {
                        name: StringId::ZERO,
                        ty: u32_ty,
                        offset: 0,
                    }],
                },
            },
        }); // 1 args + 1 layout = 2 TyIds

        // FnDef + Closure with GenericArgs.
        let item_fn = named_item(&mut m, "f", ItemKind::Function);
        let fn_def_ty = m
            .types
            .intern(Ty::FnDef(item_fn, vec![GenericArg::Type(u32_ty)])); // 1
        let closure_ty = m
            .types
            .intern(Ty::Closure(item_fn, vec![GenericArg::Type(bool_ty)])); // 1

        // FnSig with inputs + output.
        let sig = m.sigs.intern(FnSig {
            inputs: vec![u32_ty, bool_ty],
            output: u32_ty,
            abi: FnAbi::Rust,
            c_variadic: false,
            safety: Safety::Safe,
        }); // 2 inputs + 1 output = 3 TyIds
        m.sig_c_abi_lowerings.push(Some(CAbiLowering {
            inputs: vec![CAbiShape::Substituted {
                emit_as: u32_ty,
                reason: SubstReason::TransparentChain,
            }],
            output: CAbiShape::Substituted {
                emit_as: bool_ty,
                reason: SubstReason::TransparentChain,
            },
        })); // 1 input substituted + 1 output substituted = 2 TyIds

        // Const::NullPtr.
        let null_const = m.consts.intern(Const::NullPtr(u32_ty)); // 1 TyId

        // Body with every TyId-bearing surface (LocalDecl.ty,
        // Rvalue::Cast, every AggregateKind variant, PlaceElem::Field,
        // every TerminatorKind, NonDivergingIntrinsic).
        let mut local_decls: IndexVec<LocalId, LocalDecl> = IndexVec::new();
        local_decls.push(LocalDecl {
            ty: unit_ty,
            mutability: Mutability::Mut,
            span: Span::dummy(),
            debug_name: None,
        }); // _0 = return: 1 TyId
        local_decls.push(LocalDecl {
            ty: u32_ty,
            mutability: Mutability::Mut,
            span: Span::dummy(),
            debug_name: None,
        }); // _1 = u32 arg: 1 TyId
        local_decls.push(LocalDecl {
            ty: bool_ty,
            mutability: Mutability::Mut,
            span: Span::dummy(),
            debug_name: None,
        }); // _2 = bool tmp: 1 TyId

        let const_u32 = m.consts.intern(Const::Uint(UintKind::U32, 42));
        let l1 = LocalId::from_u32(1);
        let l2 = LocalId::from_u32(2);

        // Statement::Assign with PlaceElem::Field on lhs + Rvalue::Cast.
        let s1 = Statement {
            kind: StatementKind::Assign(
                Place {
                    local: l1,
                    projection: Box::new([PlaceElem::Field(FieldIdx::ZERO, u32_ty)]),
                }, // 1 TyId (Field)
                Rvalue::Cast(
                    CastKind::PointerCoercion(PointerCoercion::Unsize),
                    Operand::Constant(const_u32),
                    u32_ty,
                ), // 1 TyId
            ),
            span: Span::dummy(),
        };
        // Statement::Assign with every AggregateKind variant -- 7
        // total, each carrying one TyId.
        let s2 = Statement {
            kind: StatementKind::Assign(
                Place::from(l1),
                Rvalue::Aggregate(
                    AggregateKind::Array(u32_ty),
                    Box::new([Operand::Constant(const_u32)]),
                ),
            ),
            span: Span::dummy(),
        };
        let s3 = Statement {
            kind: StatementKind::Assign(
                Place::from(l1),
                Rvalue::Aggregate(AggregateKind::Tuple(u32_ty), Box::new([])),
            ),
            span: Span::dummy(),
        };
        let s4 = Statement {
            kind: StatementKind::Assign(
                Place::from(l1),
                Rvalue::Aggregate(AggregateKind::Struct(u32_ty), Box::new([])),
            ),
            span: Span::dummy(),
        };
        let s5 = Statement {
            kind: StatementKind::Assign(
                Place::from(l1),
                Rvalue::Aggregate(
                    AggregateKind::Enum(u32_ty, VariantIdx::from_u32(0)),
                    Box::new([]),
                ),
            ),
            span: Span::dummy(),
        };
        let s6 = Statement {
            kind: StatementKind::Assign(
                Place::from(l1),
                Rvalue::Aggregate(
                    AggregateKind::Union(u32_ty, FieldIdx::from_u32(0)),
                    Box::new([]),
                ),
            ),
            span: Span::dummy(),
        };
        let s7 = Statement {
            kind: StatementKind::Assign(
                Place::from(l1),
                Rvalue::Aggregate(AggregateKind::Closure(u32_ty), Box::new([])),
            ),
            span: Span::dummy(),
        };
        let s8 = Statement {
            kind: StatementKind::Assign(
                Place::from(l1),
                Rvalue::Aggregate(AggregateKind::RawPtr(u32_ty, Mutability::Not), Box::new([])),
            ),
            span: Span::dummy(),
        };

        let block_with_assigns = BasicBlock {
            statements: vec![s1, s2, s3, s4, s5, s6, s7, s8].into_boxed_slice(),
            terminator: Terminator {
                kind: TerminatorKind::Goto(BlockId::from_u32(1)),
                span: Span::dummy(),
            },
        };
        // Block 1 -- terminator visits operands with a Field
        // projection on destination.
        let block_with_call = BasicBlock {
            statements: Box::new([]),
            terminator: Terminator {
                kind: TerminatorKind::Call {
                    callee: CallCallee::Indirect(Operand::Copy(Place {
                        local: l1,
                        projection: Box::new([PlaceElem::Field(FieldIdx::ZERO, u32_ty)]),
                    })), // 1 TyId via callee operand place
                    args: Box::new([Operand::Copy(Place {
                        local: l1,
                        projection: Box::new([PlaceElem::Field(FieldIdx::ZERO, bool_ty)]),
                    })]), // 1 TyId via arg place
                    destination: Place {
                        local: l1,
                        projection: Box::new([PlaceElem::Field(FieldIdx::ZERO, u32_ty)]),
                    }, // 1 TyId via destination place
                    target: None,
                },
                span: Span::dummy(),
            },
        };
        let mut basic_blocks = IndexVec::new();
        basic_blocks.push(block_with_assigns);
        basic_blocks.push(block_with_call);

        let body = Body {
            basic_blocks,
            local_decls,
            arg_count: 1,
            span: Span::dummy(),
        };
        // Instance carrying generic_args + body. The Static
        // /Const/Struct/Enum/Union instance kinds carrying a TyId
        // are exercised by separate instance pushes below.
        let function_instance = push_function_instance(&mut m, item_fn, sig, Some(body));
        m.instances[function_instance].generic_args = vec![GenericArg::Type(u32_ty)];
        // Static instance: 1 TyId via kind.
        let _static_instance_id = m.instances.push(Instance {
            item: item_struct,
            generic_args: vec![],
            emission_name: StringId::ZERO,
            span: Span::dummy(),
            kind: InstanceKind::Static {
                ty: struct_ty,
                init: None,
            },
        });
        // Const instance: 1 TyId via kind.
        let const_zst = m.consts.intern(Const::ZST);
        let _const_instance_id = m.instances.push(Instance {
            item: item_struct,
            generic_args: vec![],
            emission_name: StringId::ZERO,
            span: Span::dummy(),
            kind: InstanceKind::Const {
                ty: u32_ty,
                value: const_zst,
            },
        });
        // Struct/Enum/Union instances: 1 TyId each via kind.layout.
        let _struct_instance_id = m.instances.push(Instance {
            item: item_struct,
            generic_args: vec![],
            emission_name: StringId::ZERO,
            span: Span::dummy(),
            kind: InstanceKind::Struct { layout: struct_ty },
        });
        let _enum_instance_id = m.instances.push(Instance {
            item: item_enum,
            generic_args: vec![],
            emission_name: StringId::ZERO,
            span: Span::dummy(),
            kind: InstanceKind::Enum { layout: enum_ty },
        });
        let _union_instance_id = m.instances.push(Instance {
            item: item_union,
            generic_args: vec![],
            emission_name: StringId::ZERO,
            span: Span::dummy(),
            kind: InstanceKind::Union { layout: union_ty },
        });

        // Build an enumerator closure that counts visits.
        let mut visited = 0usize;
        m.for_each_ty_id_mut(|_| visited += 1);

        // Expected count, by surface (mirroring the comments above):
        //   Ty interner entries:
        //     Array (1) + Slice (1) + Ref (1) + Ptr (1) + Dynamic (1)
        //     + Tuple_with_layout: 2 fields + 2 layout fields = 4
        //     + Struct {args=1, layout=1} = 2
        //     + Enum {args=1, tag.scalar_ty=1, variant field=1} = 3
        //     + Union {args=1, layout=1} = 2
        //     + FnDef.args = 1
        //     + Closure.args = 1
        //   = 18 from TyInterner.
        //   FnSig inputs (2) + output (1) = 3.
        //   sig_c_abi_lowerings substituted: 1 input + 1 output = 2.
        //   Body: 3 local_decls + s1(2 PlaceField + Cast) + s2..s8(7
        //         AggregateKind one each) + block_with_call(3 place
        //         field TyIds) = 3 + 2 + 7 + 3 = 15.
        //     Detailed: 3 local_decls + 1 s1.lhs.PlaceField +
        //               1 s1.Cast + 7 s2..s8 AggregateKinds + 3
        //               block_with_call PlaceFields = 15.
        //   Const::NullPtr = 1.
        //   Instance: 1 generic_arg(function_instance) +
        //             1 static.kind.ty + 1 const.kind.ty +
        //             1 struct.kind.layout + 1 enum.kind.layout +
        //             1 union.kind.layout = 6.
        let expected_total = 18 + 3 + 2 + 15 + 1 + 6;
        assert_eq!(visited, expected_total);
        // The lookup map is rebuilt: a no-op walk preserves
        // identity, so each Ty resolves to its original TyId.
        assert_eq!(m.types.find(&Ty::Uint(UintKind::U32)), Some(u32_ty));
        assert_eq!(m.types.find(&Ty::Bool), Some(bool_ty));
        // Silence unused-binding warnings for IDs we constructed
        // to populate the model graph.
        let _ = (
            array_ty,
            slice_ty,
            ref_ty,
            ptr_ty,
            dyn_ty,
            tuple_with_layout,
            struct_ty,
            enum_ty,
            union_ty,
            fn_def_ty,
            closure_ty,
            null_const,
            l2,
            const_u32,
        );
    }

    /// Test Plan item 6: `for_each_fn_sig_id_mut` visits every
    /// FnSigId reference enumerated in regseq-driver-48 Change D.
    #[test]
    fn for_each_fn_sig_id_mut_visits_function_instances_and_fn_ptr_ty() {
        let mut m = RegseqModel::with_primary("primary", StableCrateId(1));
        let u32_ty = m.types.intern(Ty::Uint(UintKind::U32));
        let sig = m.sigs.intern(FnSig {
            inputs: vec![],
            output: u32_ty,
            abi: FnAbi::Rust,
            c_variadic: false,
            safety: Safety::Safe,
        });
        m.sig_c_abi_lowerings.push(None);
        let _fn_ptr_ty = m.types.intern(Ty::FnPtr(sig));
        let item_fn = named_item(&mut m, "f", ItemKind::Function);
        let _i1 = push_function_instance(&mut m, item_fn, sig, None);
        let _i2 = push_function_instance(&mut m, item_fn, sig, None);

        let mut visited = 0usize;
        m.for_each_fn_sig_id_mut(|_| visited += 1);
        // 2 InstanceKind::Function.fn_sig + 1 Ty::FnPtr = 3.
        assert_eq!(visited, 3);
    }

    // Test Plan items 8-9 (eliminate_diag_log_peel_residue) live in
    // `crates/regseq-driver/src/bridge/diag_log/dead_item_elim.rs::mod tests`
    // so they can reach BridgeCtxt + DiagLogPeelResidue without
    // pulling regseq-driver into regseq-model's dependency graph.
    // The integration test at
    // `crates/regseq-driver/tests/bridge_diag_log_dead_item_elim.rs`
    // covers Test Plan items 10-12.

    // Compile-time touch: bring in unused imports for clippy parity.
    #[test]
    fn imports_compile() {
        let _ = (
            BinOp::Add,
            UnOp::Not,
            Mutability::Mut,
            FloatKind::F32,
            IntKind::I32,
            SwitchTargets {
                cases: Box::new([]),
                otherwise: BlockId::from_u32(0),
            },
            VariantRange {
                start: VariantIdx::ZERO,
                end: VariantIdx::ZERO,
            },
            VariantLayout {
                name: StringId::ZERO,
                fields: vec![],
                uninhabited: false,
                discriminant: 0,
                tag_value: None,
            },
            TagEncoding::Direct,
            AssertKind::NullPointerDereference,
            NonDivergingIntrinsic::Assume(Operand::Constant(ConstId::ZERO)),
            CopyNonOverlapping {
                src: Operand::Constant(ConstId::ZERO),
                dst: Operand::Constant(ConstId::ZERO),
                count: Operand::Constant(ConstId::ZERO),
            },
        );
    }
}

// ------------------------------------------------------------------
// regseq-driver-49 Change B: I-DiagLog-CallArity exception in
// `validate_call` (spec.md section 23.6.2 B-4 / B-6 +
// section 26.4 + section 26.6).
// ------------------------------------------------------------------

mod validate_log_intrinsic_call {
    use super::*;
    use crate::ids::{BlockId, LocalId};
    use crate::{
        AttrData, BasicBlock, CallCallee, IndexVec, IntKind, IntrinsicKind, LocalDecl, LogLevel,
        Mutability, Operand, Place, Provenance, Terminator, TerminatorKind, UintKind,
    };

    /// Build a fresh model with a single primary crate set up for
    /// the validator's per-terminator pass.
    fn fresh_model() -> RegseqModel {
        RegseqModel::with_primary("pl011-spec", StableCrateId(0xAAAA))
    }

    /// Push an Item with a given leaf name and synthesise its side
    /// tables. Returns the resulting `ItemId`.
    fn push_named_item(m: &mut RegseqModel, leaf: &str, hash: u64) -> ItemId {
        let leaf_id = m.strings.intern(leaf);
        let rendered = m.strings.intern(&format!("pl011-spec::{leaf}"));
        let item = Item {
            def_path: DefPath {
                crate_id: m.primary_crate,
                segments: vec![
                    DefPathSegment {
                        data: DefPathData::CrateRoot,
                        disambiguator: 0,
                    },
                    DefPathSegment {
                        data: DefPathData::ValueNs(leaf_id),
                        disambiguator: 0,
                    },
                ],
                rendered,
            },
            kind: ItemKind::Function,
            span: Span {
                file: SourceFileId::SYNTHESIZED,
                lo: 0,
                hi: 0,
            },
            linkage: Linkage::Default,
            link_section: None,
            inline_hint: InlineHint::None,
            is_unsafe: false,
            is_phantom_data: false,
            is_unsafe_cell: false,
            is_manually_drop: false,
            opaque_handle_eligible: false,
        };
        let id = m.items.push(item);
        m.item_def_paths.push(crate::DefPathHash {
            stable_crate_id: StableCrateId(0xAAAA),
            local_hash: hash,
        });
        m.item_synthetic_kinds.push(Default::default());
        m.domain_attrs.item_attrs.push(Vec::new());
        id
    }

    fn unit_layout() -> crate::Layout {
        crate::UNIT_LAYOUT
    }

    fn unit_ty(m: &mut RegseqModel) -> TyId {
        m.types.intern(Ty::Tuple {
            fields: vec![],
            layout: unit_layout(),
        })
    }

    fn str_ref_ty(m: &mut RegseqModel) -> TyId {
        let str_ty = m.types.intern(Ty::Str);
        m.types.intern(Ty::Ref(str_ty, Provenance::SharedBorrow))
    }

    /// Build a Function instance with no body and the given FnSig.
    fn push_function(m: &mut RegseqModel, item: ItemId, sig: FnSigId) -> InstanceId {
        m.instances.push(Instance {
            item,
            generic_args: vec![],
            emission_name: StringId::ZERO,
            span: Span {
                file: SourceFileId::SYNTHESIZED,
                lo: 0,
                hi: 0,
            },
            kind: function_kind(sig, None),
        })
    }

    fn intern_sig(m: &mut RegseqModel, inputs: Vec<TyId>, output: TyId) -> FnSigId {
        let sig = m.sigs.intern(FnSig {
            inputs,
            output,
            abi: FnAbi::Rust,
            c_variadic: false,
            safety: Safety::Safe,
        });
        m.sig_c_abi_lowerings.push(None);
        sig
    }

    /// Construct a caller body whose `_0` is the unit destination
    /// and remaining locals carry the supplied `arg_tys`. Returns
    /// the body and a parallel `Vec<LocalId>` for `_1..` so callers
    /// can build `Operand::Move(...)` references.
    fn build_caller_body(
        m: &mut RegseqModel,
        callee_callee: CallCallee,
        arg_tys: &[TyId],
        args_override: Option<Box<[Operand]>>,
    ) -> Body {
        let unit = unit_ty(m);
        let mut local_decls = IndexVec::new();
        // _0: return place (unit).
        local_decls.push(LocalDecl {
            ty: unit,
            mutability: Mutability::Mut,
            span: Span::dummy(),
            debug_name: None,
        });
        let mut arg_operands: Vec<Operand> = Vec::with_capacity(arg_tys.len());
        for ty in arg_tys.iter().copied() {
            let local_id = LocalId::from_u32(local_decls.len() as u32);
            local_decls.push(LocalDecl {
                ty,
                mutability: Mutability::Mut,
                span: Span::dummy(),
                debug_name: None,
            });
            arg_operands.push(Operand::Move(Place::from(local_id)));
        }
        let args: Box<[Operand]> = match args_override {
            Some(boxed) => boxed,
            None => arg_operands.into_boxed_slice(),
        };
        let mut basic_blocks = IndexVec::new();
        basic_blocks.push(BasicBlock {
            statements: Box::new([]),
            terminator: Terminator {
                kind: TerminatorKind::Call {
                    callee: callee_callee,
                    args,
                    destination: Place::from(LocalId::RETURN_PLACE),
                    target: Some(BlockId::from_u32(1)),
                },
                span: Span::dummy(),
            },
        });
        // BlockId 1: the post-Call resume block (a trivial Return).
        basic_blocks.push(BasicBlock {
            statements: Box::new([]),
            terminator: Terminator {
                kind: TerminatorKind::Return,
                span: Span::dummy(),
            },
        });
        Body {
            basic_blocks,
            local_decls,
            arg_count: 0,
            span: Span::dummy(),
        }
    }

    /// Convenience: install a Log-intrinsic Function callee whose
    /// FnSig matches the post-`regseq-driver-48` rewritten shape
    /// `(&'static str,) -> ()`. The callee's item carries an
    /// `AttrData::Intrinsic(IntrinsicKind::Log(_))` entry.
    fn install_log_callee(
        m: &mut RegseqModel,
        leaf: &str,
        hash: u64,
        level: LogLevel,
    ) -> InstanceId {
        let str_ref = str_ref_ty(m);
        let unit = unit_ty(m);
        let sig = intern_sig(m, vec![str_ref], unit);
        let item = push_named_item(m, leaf, hash);
        m.domain_attrs.item_attrs[item].push(AttrData::Intrinsic(IntrinsicKind::Log(level)));
        push_function(m, item, sig)
    }

    /// Convenience: install a non-Log Function callee with the
    /// given input/output types.
    fn install_plain_callee(
        m: &mut RegseqModel,
        leaf: &str,
        hash: u64,
        inputs: Vec<TyId>,
        output: TyId,
    ) -> InstanceId {
        let sig = intern_sig(m, inputs, output);
        let item = push_named_item(m, leaf, hash);
        push_function(m, item, sig)
    }

    /// Run the validator's per-call check on the given owner /
    /// body. Bypasses the rest of `validate()` so the focused
    /// invariants under test are isolated.
    fn run_validate(
        m: &RegseqModel,
        owner: InstanceId,
        body: &Body,
    ) -> Result<(), InvariantViolation> {
        m.validate_body_terminator_invariants(owner, body)
    }

    // -------- Test Plan item 5 --------
    #[test]
    fn accepts_canonical_post_peel_shape_str_ref_and_u32() {
        // Per spec, args = [Operand::Constant(<&str>),
        // Operand::Move(<u32_local>)] -- the post-DiagLogPeel shape
        // wraps the format-template literal as `Operand::Constant`
        // and the peeled value-typed expression as `Operand::Move`.
        let mut m = fresh_model();
        let log_callee = install_log_callee(&mut m, "log_info", 0x10, LogLevel::Info);
        let u32_ty = m.types.intern(Ty::Uint(UintKind::U32));
        let caller_item = push_named_item(&mut m, "caller_a", 0x20);
        let unit = unit_ty(&mut m);
        let caller_sig = intern_sig(&mut m, vec![], unit);
        // Format-template `&'static str` literal lives in a const
        // slot; the bridge surfaces it as `Operand::Constant`. The
        // model-side `const_ty` resolves a `Const::Bytes` to
        // `NotApplicable` per spec.md section 20.5 (operand-context
        // typing for non-scalar constants is owned upstream), so
        // the validator's `args[0]` check skips per the
        // `TypeRecovery::NotApplicable` arm. The acceptance via a
        // recovered `Ty::Ref(Ty::Str, _)` is locked in by
        // `accepts_args_one_str_ref` (Test Plan item 11).
        let fmt_const = m.consts.intern(crate::Const::Bytes(vec![]));
        // _0: return, _1: u32 arg slot.
        let mut local_decls = crate::IndexVec::new();
        local_decls.push(LocalDecl {
            ty: unit,
            mutability: Mutability::Mut,
            span: Span::dummy(),
            debug_name: None,
        });
        let u32_local = LocalId::from_u32(local_decls.len() as u32);
        local_decls.push(LocalDecl {
            ty: u32_ty,
            mutability: Mutability::Mut,
            span: Span::dummy(),
            debug_name: None,
        });
        let mut basic_blocks = crate::IndexVec::new();
        basic_blocks.push(BasicBlock {
            statements: Box::new([]),
            terminator: Terminator {
                kind: TerminatorKind::Call {
                    callee: CallCallee::Direct(log_callee),
                    args: Box::new([
                        Operand::Constant(fmt_const),
                        Operand::Move(Place::from(u32_local)),
                    ]),
                    destination: Place::from(LocalId::RETURN_PLACE),
                    target: Some(BlockId::from_u32(1)),
                },
                span: Span::dummy(),
            },
        });
        basic_blocks.push(BasicBlock {
            statements: Box::new([]),
            terminator: Terminator {
                kind: TerminatorKind::Return,
                span: Span::dummy(),
            },
        });
        let body = Body {
            basic_blocks,
            local_decls,
            arg_count: 0,
            span: Span::dummy(),
        };
        let caller = push_function(&mut m, caller_item, caller_sig);
        m.instances[caller].kind = function_kind(caller_sig, Some(body.clone()));
        assert_eq!(run_validate(&m, caller, &body), Ok(()));
    }

    // -------- Test Plan item 6 --------
    #[test]
    fn rejects_args_zero_not_str_ref() {
        let mut m = fresh_model();
        let log_callee = install_log_callee(&mut m, "log_info", 0x10, LogLevel::Info);
        let u32_ty = m.types.intern(Ty::Uint(UintKind::U32));
        let caller_item = push_named_item(&mut m, "caller_b", 0x20);
        let unit = unit_ty(&mut m);
        let caller_sig = intern_sig(&mut m, vec![], unit);
        // args[0] = u32 local, args[1] = u32 local. The first arg
        // breaks the &str-ref contract on `args[0]`.
        let body = build_caller_body(
            &mut m,
            CallCallee::Direct(log_callee),
            &[u32_ty, u32_ty],
            None,
        );
        let caller = push_function(&mut m, caller_item, caller_sig);
        match run_validate(&m, caller, &body) {
            Err(InvariantViolation::DiagLogCallArg0NotStrRef { actual_ty, .. }) => {
                assert_eq!(actual_ty, u32_ty);
            }
            other => panic!("expected DiagLogCallArg0NotStrRef, got {other:?}"),
        }
    }

    // -------- Test Plan item 7 --------
    #[test]
    fn rejects_args_one_not_admissible_log_arg_source() {
        let mut m = fresh_model();
        let log_callee = install_log_callee(&mut m, "log_info", 0x10, LogLevel::Info);
        let str_ref = str_ref_ty(&mut m);
        // Tuple unit -- not in the admissible-LogArg-source set.
        let unit = unit_ty(&mut m);
        let caller_item = push_named_item(&mut m, "caller_c", 0x20);
        let caller_sig = intern_sig(&mut m, vec![], unit);
        let body = build_caller_body(
            &mut m,
            CallCallee::Direct(log_callee),
            &[str_ref, unit],
            None,
        );
        let caller = push_function(&mut m, caller_item, caller_sig);
        match run_validate(&m, caller, &body) {
            Err(InvariantViolation::DiagLogCallArgNotAdmissible { idx, actual_ty, .. }) => {
                assert_eq!(idx, 1);
                assert_eq!(actual_ty, unit);
            }
            other => panic!("expected DiagLogCallArgNotAdmissible, got {other:?}"),
        }
    }

    // -------- Test Plan item 8 --------
    #[test]
    fn rejects_empty_args() {
        let mut m = fresh_model();
        let log_callee = install_log_callee(&mut m, "log_info", 0x10, LogLevel::Info);
        let unit = unit_ty(&mut m);
        let caller_item = push_named_item(&mut m, "caller_d", 0x20);
        let caller_sig = intern_sig(&mut m, vec![], unit);
        let body = build_caller_body(
            &mut m,
            CallCallee::Direct(log_callee),
            &[],
            Some(Box::new([])),
        );
        let caller = push_function(&mut m, caller_item, caller_sig);
        match run_validate(&m, caller, &body) {
            Err(InvariantViolation::DiagLogCallArityBelowMinimum { .. }) => {}
            other => panic!("expected DiagLogCallArityBelowMinimum, got {other:?}"),
        }
    }

    // -------- Test Plan item 9 --------
    #[test]
    fn non_log_direct_call_uses_generic_b4() {
        let mut m = fresh_model();
        let u32_ty = m.types.intern(Ty::Uint(UintKind::U32));
        let unit = unit_ty(&mut m);
        // Plain callee with one declared input.
        let plain_callee = install_plain_callee(&mut m, "plain_callee", 0x10, vec![u32_ty], unit);
        // Caller passes two args -- arity mismatch.
        let caller_item = push_named_item(&mut m, "caller_e", 0x20);
        let caller_sig = intern_sig(&mut m, vec![], unit);
        let body = build_caller_body(
            &mut m,
            CallCallee::Direct(plain_callee),
            &[u32_ty, u32_ty],
            None,
        );
        let caller = push_function(&mut m, caller_item, caller_sig);
        match run_validate(&m, caller, &body) {
            Err(InvariantViolation::CallArgsArityMismatch {
                expected, actual, ..
            }) => {
                assert_eq!(expected, 1);
                assert_eq!(actual, 2);
            }
            other => panic!("expected CallArgsArityMismatch, got {other:?}"),
        }
    }

    // -------- Test Plan item 10 --------
    #[test]
    fn indirect_call_does_not_receive_log_exception() {
        let mut m = fresh_model();
        let str_ref = str_ref_ty(&mut m);
        let u32_ty = m.types.intern(Ty::Uint(UintKind::U32));
        let unit = unit_ty(&mut m);
        // FnSig with a single u32 input mismatches the two-arg call
        // shape; the generic B-4 path must fire because the
        // Log-intrinsic exception applies only to Direct callees
        // per spec.md section 23.6.2 B-5.
        let target_sig = intern_sig(&mut m, vec![u32_ty], unit);
        let fn_ptr_ty = m.types.intern(Ty::FnPtr(target_sig));
        let caller_item = push_named_item(&mut m, "caller_f", 0x20);
        let caller_sig = intern_sig(&mut m, vec![], unit);
        // Caller body has _0 (return), _1 (fn_ptr), then two args.
        let mut local_decls = IndexVec::new();
        local_decls.push(LocalDecl {
            ty: unit,
            mutability: Mutability::Mut,
            span: Span::dummy(),
            debug_name: None,
        });
        let fnptr_local = LocalId::from_u32(local_decls.len() as u32);
        local_decls.push(LocalDecl {
            ty: fn_ptr_ty,
            mutability: Mutability::Mut,
            span: Span::dummy(),
            debug_name: None,
        });
        let str_local = LocalId::from_u32(local_decls.len() as u32);
        local_decls.push(LocalDecl {
            ty: str_ref,
            mutability: Mutability::Mut,
            span: Span::dummy(),
            debug_name: None,
        });
        let u32_local = LocalId::from_u32(local_decls.len() as u32);
        local_decls.push(LocalDecl {
            ty: u32_ty,
            mutability: Mutability::Mut,
            span: Span::dummy(),
            debug_name: None,
        });
        let mut basic_blocks = IndexVec::new();
        basic_blocks.push(BasicBlock {
            statements: Box::new([]),
            terminator: Terminator {
                kind: TerminatorKind::Call {
                    callee: CallCallee::Indirect(Operand::Copy(Place::from(fnptr_local))),
                    args: Box::new([
                        Operand::Move(Place::from(str_local)),
                        Operand::Move(Place::from(u32_local)),
                    ]),
                    destination: Place::from(LocalId::RETURN_PLACE),
                    target: Some(BlockId::from_u32(1)),
                },
                span: Span::dummy(),
            },
        });
        basic_blocks.push(BasicBlock {
            statements: Box::new([]),
            terminator: Terminator {
                kind: TerminatorKind::Return,
                span: Span::dummy(),
            },
        });
        let body = Body {
            basic_blocks,
            local_decls,
            arg_count: 0,
            span: Span::dummy(),
        };
        let caller = push_function(&mut m, caller_item, caller_sig);
        match run_validate(&m, caller, &body) {
            Err(InvariantViolation::CallArgsArityMismatch {
                expected, actual, ..
            }) => {
                assert_eq!(expected, 1);
                assert_eq!(actual, 2);
            }
            other => panic!("expected CallArgsArityMismatch, got {other:?}"),
        }
    }

    // -------- Test Plan item 11 --------
    #[test]
    fn accepts_args_one_str_ref() {
        let mut m = fresh_model();
        let log_callee = install_log_callee(&mut m, "log_warn", 0x10, LogLevel::Warn);
        let str_ref = str_ref_ty(&mut m);
        let caller_item = push_named_item(&mut m, "caller_g", 0x20);
        let unit = unit_ty(&mut m);
        let caller_sig = intern_sig(&mut m, vec![], unit);
        let body = build_caller_body(
            &mut m,
            CallCallee::Direct(log_callee),
            &[str_ref, str_ref],
            None,
        );
        let caller = push_function(&mut m, caller_item, caller_sig);
        assert_eq!(run_validate(&m, caller, &body), Ok(()));
    }

    // -------- Test Plan item 12 --------
    #[test]
    fn accepts_every_admissible_log_arg_source_at_args_one() {
        let mut m = fresh_model();
        let log_callee = install_log_callee(&mut m, "log_debug", 0x10, LogLevel::Debug);
        let str_ref = str_ref_ty(&mut m);
        let unit = unit_ty(&mut m);
        let caller_sig = intern_sig(&mut m, vec![], unit);
        // Build admissible-Ty matrix in one go.
        let bool_ty = m.types.intern(Ty::Bool);
        let u8_ty = m.types.intern(Ty::Uint(UintKind::U8));
        let u16_ty = m.types.intern(Ty::Uint(UintKind::U16));
        let u32_ty = m.types.intern(Ty::Uint(UintKind::U32));
        let u64_ty = m.types.intern(Ty::Uint(UintKind::U64));
        let i8_ty = m.types.intern(Ty::Int(IntKind::I8));
        let i16_ty = m.types.intern(Ty::Int(IntKind::I16));
        let i32_ty = m.types.intern(Ty::Int(IntKind::I32));
        let i64_ty = m.types.intern(Ty::Int(IntKind::I64));
        let admissibles = [
            bool_ty, u8_ty, u16_ty, u32_ty, u64_ty, i8_ty, i16_ty, i32_ty, i64_ty,
        ];
        for (idx, ty) in admissibles.iter().copied().enumerate() {
            let caller_item =
                push_named_item(&mut m, &format!("caller_h_{idx}"), 0x30 + idx as u64);
            let body =
                build_caller_body(&mut m, CallCallee::Direct(log_callee), &[str_ref, ty], None);
            let caller = push_function(&mut m, caller_item, caller_sig);
            assert_eq!(
                run_validate(&m, caller, &body),
                Ok(()),
                "Ty index {idx} ({:?}) must be admissible",
                m.types.resolve(ty)
            );
        }
    }
}
