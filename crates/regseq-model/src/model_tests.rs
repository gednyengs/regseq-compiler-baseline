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
fn invariant_registry_has_twenty_non_empty_descriptors() {
    let ids = all_invariant_ids();
    assert_eq!(ids.len(), 20);
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
        };
        assert!((1..=20).contains(&n));
    }
}

fn all_invariant_ids() -> [InvariantId; 20] {
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

#[test]
#[ignore]
fn instance_intrinsic_helper_signature_compiles() {
    // IGNORE-UNTIL: P1a-DS-09
    let (m, id) = helper_fixture();
    let _ = m.instance_intrinsic(id);
}
