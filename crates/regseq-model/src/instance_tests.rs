//! Tests for `Instance`, `InstanceKind`, instance flavor ranks, and
//! bridge-side instance keys.

use std::mem::size_of;

use crate::{
    regseq_instance_kind_rank, rustc_instance_kind_sort_rank, ArgsKey, Body, ConstId, FnSigId,
    FunctionInstanceData, FunctionInstanceFlavor, GenericArg, Instance, InstanceId, InstanceKind,
    ItemId, RegseqInstanceKey, SourceFileId, Span, StringId, TyId,
};

fn span_zero() -> Span {
    Span {
        file: SourceFileId::SYNTHESIZED,
        lo: 0,
        hi: 0,
    }
}

fn function_data(
    fn_sig: FnSigId,
    body: Option<Body>,
    flavor: FunctionInstanceFlavor,
    virtual_idx: Option<u32>,
) -> FunctionInstanceData {
    FunctionInstanceData {
        fn_sig,
        body,
        flavor,
        virtual_idx,
    }
}

fn function_kind(fn_sig: FnSigId, body: Option<Body>) -> InstanceKind {
    InstanceKind::Function(function_data(
        fn_sig,
        body,
        FunctionInstanceFlavor::Item,
        None,
    ))
}

fn all_flavors() -> Vec<FunctionInstanceFlavor> {
    use FunctionInstanceFlavor::*;
    vec![
        Item,
        Intrinsic,
        VTableShim,
        ReifyShim,
        FnPtrShim,
        Virtual,
        ClosureOnceShim,
        ConstructCoroutineInClosureShim,
        ThreadLocalShim,
        FutureDropPollShim,
        DropGlue,
        CloneShim,
        FnPtrAddrShim,
        AsyncDropGlueCtorShim,
        AsyncDropGlue,
    ]
}

#[test]
fn generic_arg_variants_construct_and_compare() {
    let ty = GenericArg::Type(TyId::from_u32(1));
    let konst = GenericArg::Const(ConstId::from_u32(2));

    assert_ne!(ty, konst);
    assert_eq!(ty.clone(), GenericArg::Type(TyId::from_u32(1)));
    assert_eq!(konst.clone(), GenericArg::Const(ConstId::from_u32(2)));
}

#[test]
fn instance_constructs_with_emission_name() {
    let kind = function_kind(FnSigId::from_u32(3), Some(Body::default()));
    let inst = Instance {
        item: ItemId::from_u32(2),
        generic_args: vec![GenericArg::Type(TyId::from_u32(4))],
        emission_name: StringId::from_u32(5),
        span: span_zero(),
        kind: kind.clone(),
    };

    assert_eq!(inst.item, ItemId::from_u32(2));
    assert_eq!(inst.generic_args, vec![GenericArg::Type(TyId::from_u32(4))]);
    assert_eq!(inst.emission_name, StringId::from_u32(5));
    assert_eq!(inst.span, span_zero());
    assert_eq!(inst.kind, kind);
}

#[test]
fn function_instance_kind_constructs_with_body() {
    let kind = InstanceKind::Function(function_data(
        FnSigId::from_u32(7),
        Some(Body::default()),
        FunctionInstanceFlavor::Item,
        None,
    ));
    let data = kind.as_function().unwrap();

    assert_eq!(data.fn_sig, FnSigId::from_u32(7));
    assert_eq!(data.body, Some(Body::default()));
    assert_eq!(data.flavor, FunctionInstanceFlavor::Item);
    assert_eq!(data.virtual_idx, None);
    assert_eq!(data.fn_sig(), FnSigId::from_u32(7));
    assert_eq!(data.body(), Some(&Body::default()));
    assert_eq!(data.flavor(), FunctionInstanceFlavor::Item);
    assert_eq!(data.virtual_idx(), None);
}

#[test]
fn function_instance_kind_constructs_foreign_declaration_shape() {
    let kind = function_kind(FnSigId::from_u32(8), None);
    let data = kind.as_function().unwrap();

    assert_eq!(data.fn_sig, FnSigId::from_u32(8));
    assert!(data.body.is_none());
}

#[test]
fn function_instance_kind_constructs_virtual_flavor_shape() {
    let kind = InstanceKind::Function(function_data(
        FnSigId::from_u32(9),
        Some(Body::default()),
        FunctionInstanceFlavor::Virtual,
        Some(0),
    ));
    let data = kind.as_function().unwrap();

    assert_eq!(data.flavor, FunctionInstanceFlavor::Virtual);
    assert_eq!(data.virtual_idx, Some(0));
}

#[test]
fn static_instance_kind_constructs_source_defined_and_foreign_shapes() {
    let source_defined = InstanceKind::Static {
        ty: TyId::from_u32(10),
        init: Some(ConstId::from_u32(11)),
    };
    let foreign = InstanceKind::Static {
        ty: TyId::from_u32(12),
        init: None,
    };

    assert!(matches!(
        source_defined,
        InstanceKind::Static {
            ty,
            init: Some(init)
        } if ty == TyId::from_u32(10) && init == ConstId::from_u32(11)
    ));
    assert!(matches!(foreign, InstanceKind::Static { init: None, .. }));
}

#[test]
fn const_struct_enum_union_instance_kinds_construct() {
    let konst = InstanceKind::Const {
        ty: TyId::from_u32(13),
        value: ConstId::from_u32(14),
    };
    let strukt = InstanceKind::Struct {
        layout: TyId::from_u32(15),
    };
    let enm = InstanceKind::Enum {
        layout: TyId::from_u32(16),
    };
    let union = InstanceKind::Union {
        layout: TyId::from_u32(17),
    };

    assert!(matches!(
        konst,
        InstanceKind::Const { ty, value }
            if ty == TyId::from_u32(13) && value == ConstId::from_u32(14)
    ));
    assert_ne!(strukt, enm);
    assert_ne!(enm, union);
    assert_ne!(strukt, union);
}

#[test]
fn as_function_negative_for_non_function_variants() {
    let variants = vec![
        InstanceKind::Static {
            ty: TyId::ZERO,
            init: None,
        },
        InstanceKind::Const {
            ty: TyId::ZERO,
            value: ConstId::ZERO,
        },
        InstanceKind::Struct { layout: TyId::ZERO },
        InstanceKind::Enum { layout: TyId::ZERO },
        InstanceKind::Union { layout: TyId::ZERO },
    ];

    for kind in variants {
        assert!(kind.as_function().is_none());
    }
}

#[test]
fn as_function_mut_allows_payload_mutation() {
    let mut kind = function_kind(FnSigId::from_u32(18), Some(Body::default()));
    let data = kind.as_function_mut().unwrap();
    data.virtual_idx = Some(3);
    assert_eq!(data.body_mut(), Some(&mut Body::default()));

    assert_eq!(kind.as_function().unwrap().virtual_idx, Some(3));
}

#[test]
fn function_instance_flavor_has_all_distinct_variants() {
    let mut variants = all_flavors();
    variants.sort();
    variants.dedup();

    assert_eq!(variants.len(), 15);
}

#[test]
fn rustc_instance_kind_sort_rank_matches_pinned_table() {
    use FunctionInstanceFlavor::*;
    let cases: &[(FunctionInstanceFlavor, u8)] = &[
        (Item, 0),
        (Intrinsic, 1),
        (VTableShim, 2),
        (ReifyShim, 3),
        (FnPtrShim, 4),
        (Virtual, 5),
        (ClosureOnceShim, 6),
        (ConstructCoroutineInClosureShim, 7),
        (ThreadLocalShim, 8),
        (FutureDropPollShim, 9),
        (DropGlue, 10),
        (CloneShim, 11),
        (FnPtrAddrShim, 12),
        (AsyncDropGlueCtorShim, 13),
        (AsyncDropGlue, 14),
    ];

    for (flavor, expected) in cases {
        assert_eq!(rustc_instance_kind_sort_rank(*flavor), *expected);
    }
}

#[test]
fn regseq_instance_kind_rank_matches_pinned_table() {
    let cases: &[(InstanceKind, u8)] = &[
        (function_kind(FnSigId::ZERO, None), 0),
        (
            InstanceKind::Static {
                ty: TyId::ZERO,
                init: None,
            },
            1,
        ),
        (
            InstanceKind::Const {
                ty: TyId::ZERO,
                value: ConstId::ZERO,
            },
            2,
        ),
        (InstanceKind::Struct { layout: TyId::ZERO }, 3),
        (InstanceKind::Enum { layout: TyId::ZERO }, 4),
        (InstanceKind::Union { layout: TyId::ZERO }, 5),
    ];

    for (kind, expected) in cases {
        assert_eq!(regseq_instance_kind_rank(kind), *expected);
    }
}

#[test]
fn regseq_instance_key_constructs_and_compares_by_each_field() {
    let base = RegseqInstanceKey {
        regseq_kind_rank: 0,
        item: ItemId::from_u32(1),
        generic_args_key: ArgsKey(2),
        fn_shim_disambig: 3,
    };

    assert_eq!(base, base);
    assert_ne!(
        base,
        RegseqInstanceKey {
            regseq_kind_rank: 1,
            ..base
        }
    );
    assert_ne!(
        base,
        RegseqInstanceKey {
            item: ItemId::from_u32(9),
            ..base
        }
    );
    assert_ne!(
        base,
        RegseqInstanceKey {
            generic_args_key: ArgsKey(9),
            ..base
        }
    );
    assert_ne!(
        base,
        RegseqInstanceKey {
            fn_shim_disambig: 9,
            ..base
        }
    );
}

#[test]
fn instance_id_keeps_stage_one_niche_size() {
    assert_eq!(size_of::<InstanceId>(), 4);
    assert_eq!(size_of::<Option<InstanceId>>(), 4);
}

#[test]
fn generic_arg_round_trips_through_serde() {
    for arg in [
        GenericArg::Type(TyId::from_u32(1)),
        GenericArg::Const(ConstId::from_u32(2)),
    ] {
        let json = serde_json::to_string(&arg).unwrap();
        assert_eq!(serde_json::from_str::<GenericArg>(&json).unwrap(), arg);
    }
}

#[test]
fn instance_kind_round_trips_each_variant_through_serde() {
    let variants = vec![
        function_kind(FnSigId::from_u32(1), Some(Body::default())),
        function_kind(FnSigId::from_u32(2), None),
        InstanceKind::Static {
            ty: TyId::from_u32(3),
            init: Some(ConstId::from_u32(4)),
        },
        InstanceKind::Static {
            ty: TyId::from_u32(5),
            init: None,
        },
        InstanceKind::Const {
            ty: TyId::from_u32(6),
            value: ConstId::from_u32(7),
        },
        InstanceKind::Struct {
            layout: TyId::from_u32(8),
        },
        InstanceKind::Enum {
            layout: TyId::from_u32(9),
        },
        InstanceKind::Union {
            layout: TyId::from_u32(10),
        },
    ];

    for kind in variants {
        let json = serde_json::to_string(&kind).unwrap();
        assert_eq!(serde_json::from_str::<InstanceKind>(&json).unwrap(), kind);
    }
}

#[test]
fn function_instance_flavor_round_trips_each_variant_through_serde() {
    for flavor in all_flavors() {
        let json = serde_json::to_string(&flavor).unwrap();
        assert_eq!(
            serde_json::from_str::<FunctionInstanceFlavor>(&json).unwrap(),
            flavor
        );
    }
}

#[test]
fn instance_round_trips_through_serde() {
    let inst = Instance {
        item: ItemId::from_u32(1),
        generic_args: vec![
            GenericArg::Type(TyId::from_u32(2)),
            GenericArg::Const(ConstId::from_u32(3)),
        ],
        emission_name: StringId::from_u32(4),
        span: span_zero(),
        kind: function_kind(FnSigId::from_u32(5), Some(Body::default())),
    };
    let json = serde_json::to_string(&inst).unwrap();

    assert_eq!(serde_json::from_str::<Instance>(&json).unwrap(), inst);
}

#[test]
fn regseq_instance_key_round_trips_through_serde() {
    let key = RegseqInstanceKey {
        regseq_kind_rank: 0,
        item: ItemId::from_u32(1),
        generic_args_key: ArgsKey(2),
        fn_shim_disambig: 3,
    };
    let json = serde_json::to_string(&key).unwrap();

    assert_eq!(
        serde_json::from_str::<RegseqInstanceKey>(&json).unwrap(),
        key
    );
}
