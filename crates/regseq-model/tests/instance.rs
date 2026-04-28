//! Integration tests for stage-1 step-5 instance types:
//! `GenericArg`, `GenericArgs`, `Instance`, `InstanceKind`,
//! `regseq_instance_kind_rank`, `RustcInstanceKindTag`, and
//! `rustc_instance_kind_sort_rank`.

use regseq_model::{
    regseq_instance_kind_rank, rustc_instance_kind_sort_rank, BodyId, ConstId, FnSigId, GenericArg,
    Instance, InstanceKind, ItemId, RustcInstanceKindTag, SourceFileId, Span, StringId, TyId,
};

// ------------------------------------------------------------------
// GenericArg: externally-tagged enum.
// ------------------------------------------------------------------

#[test]
fn generic_arg_type_serializes_as_externally_tagged() {
    let arg = GenericArg::Type(TyId::from_u32(3));
    let json = serde_json::to_value(&arg).unwrap();
    assert_eq!(json, serde_json::json!({"Type": 3}));
    let back: GenericArg = serde_json::from_value(json).unwrap();
    assert_eq!(arg, back);
}

#[test]
fn generic_arg_const_serializes_as_externally_tagged() {
    let arg = GenericArg::Const(ConstId::from_u32(12));
    let json = serde_json::to_value(&arg).unwrap();
    assert_eq!(json, serde_json::json!({"Const": 12}));
    let back: GenericArg = serde_json::from_value(json).unwrap();
    assert_eq!(arg, back);
}

#[test]
fn generic_args_round_trip_as_array() {
    let args: Vec<GenericArg> = vec![
        GenericArg::Type(TyId::from_u32(1)),
        GenericArg::Const(ConstId::from_u32(2)),
        GenericArg::Type(TyId::from_u32(3)),
    ];
    let json = serde_json::to_string(&args).unwrap();
    let back: Vec<GenericArg> = serde_json::from_str(&json).unwrap();
    assert_eq!(args, back);
}

// ------------------------------------------------------------------
// InstanceKind: 6 variants, externally-tagged with named fields per
// the spec serialization-conventions table.
// ------------------------------------------------------------------

fn span_zero() -> Span {
    Span {
        file: SourceFileId::SYNTHESIZED,
        lo: 0,
        hi: 0,
    }
}

fn all_instance_kinds() -> Vec<InstanceKind> {
    vec![
        InstanceKind::Function {
            sig: FnSigId::from_u32(1),
            body: Some(BodyId::from_u32(2)),
        },
        InstanceKind::Function {
            sig: FnSigId::from_u32(3),
            body: None,
        },
        InstanceKind::Static {
            ty: TyId::from_u32(4),
            init: ConstId::from_u32(5),
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
    ]
}

#[test]
fn instance_kind_round_trips_all_variants_including_foreign() {
    for kind in all_instance_kinds() {
        let json = serde_json::to_string(&kind).unwrap();
        let back: InstanceKind = serde_json::from_str(&json).unwrap();
        assert_eq!(kind, back, "round-trip preserves variant: {json}");
    }
}

#[test]
fn instance_kind_function_with_body_json_shape() {
    let k = InstanceKind::Function {
        sig: FnSigId::from_u32(2),
        body: Some(BodyId::from_u32(7)),
    };
    let v = serde_json::to_value(&k).unwrap();
    assert_eq!(v, serde_json::json!({"Function": {"sig": 2, "body": 7}}));
}

#[test]
fn instance_kind_function_without_body_serializes_body_as_null() {
    let k = InstanceKind::Function {
        sig: FnSigId::from_u32(2),
        body: None,
    };
    let v = serde_json::to_value(&k).unwrap();
    assert_eq!(
        v,
        serde_json::json!({"Function": {"sig": 2, "body": null}})
    );
}

#[test]
fn instance_kind_static_const_struct_enum_union_json_shape() {
    let cases = vec![
        (
            InstanceKind::Static {
                ty: TyId::from_u32(3),
                init: ConstId::from_u32(12),
            },
            serde_json::json!({"Static": {"ty": 3, "init": 12}}),
        ),
        (
            InstanceKind::Const {
                ty: TyId::from_u32(4),
                value: ConstId::from_u32(8),
            },
            serde_json::json!({"Const": {"ty": 4, "value": 8}}),
        ),
        (
            InstanceKind::Struct {
                layout: TyId::from_u32(5),
            },
            serde_json::json!({"Struct": {"layout": 5}}),
        ),
        (
            InstanceKind::Enum {
                layout: TyId::from_u32(6),
            },
            serde_json::json!({"Enum": {"layout": 6}}),
        ),
        (
            InstanceKind::Union {
                layout: TyId::from_u32(7),
            },
            serde_json::json!({"Union": {"layout": 7}}),
        ),
    ];
    for (kind, expected) in cases {
        let v = serde_json::to_value(&kind).unwrap();
        assert_eq!(v, expected, "wire shape mismatch for {:?}", kind);
    }
}

// ------------------------------------------------------------------
// regseq_instance_kind_rank: pinned 0..=5, append-only over 6
// emission kinds.
// ------------------------------------------------------------------

#[test]
fn regseq_instance_kind_rank_matches_pinned_table() {
    let cases: &[(InstanceKind, u8)] = &[
        (
            InstanceKind::Function {
                sig: FnSigId::from_u32(0),
                body: None,
            },
            0,
        ),
        (
            InstanceKind::Static {
                ty: TyId::from_u32(0),
                init: ConstId::from_u32(0),
            },
            1,
        ),
        (
            InstanceKind::Const {
                ty: TyId::from_u32(0),
                value: ConstId::from_u32(0),
            },
            2,
        ),
        (
            InstanceKind::Struct {
                layout: TyId::from_u32(0),
            },
            3,
        ),
        (
            InstanceKind::Enum {
                layout: TyId::from_u32(0),
            },
            4,
        ),
        (
            InstanceKind::Union {
                layout: TyId::from_u32(0),
            },
            5,
        ),
    ];
    for (kind, expected) in cases {
        assert_eq!(
            regseq_instance_kind_rank(kind),
            *expected,
            "rank for {:?} must be pinned at {}",
            kind,
            expected
        );
    }
}

#[test]
fn regseq_instance_kind_ranks_are_unique_and_dense() {
    let kinds: Vec<InstanceKind> = vec![
        InstanceKind::Function {
            sig: FnSigId::from_u32(0),
            body: None,
        },
        InstanceKind::Static {
            ty: TyId::from_u32(0),
            init: ConstId::from_u32(0),
        },
        InstanceKind::Const {
            ty: TyId::from_u32(0),
            value: ConstId::from_u32(0),
        },
        InstanceKind::Struct {
            layout: TyId::from_u32(0),
        },
        InstanceKind::Enum {
            layout: TyId::from_u32(0),
        },
        InstanceKind::Union {
            layout: TyId::from_u32(0),
        },
    ];
    let ranks: Vec<u8> = kinds.iter().map(regseq_instance_kind_rank).collect();
    let mut sorted = ranks.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(
        sorted.len(),
        ranks.len(),
        "ranks are unique across the six emission kinds"
    );
    assert_eq!(
        sorted,
        (0u8..6).collect::<Vec<_>>(),
        "ranks form the dense range 0..=5"
    );
}

#[test]
fn regseq_instance_kind_rank_ignores_payload_and_function_body_presence() {
    let with = InstanceKind::Function {
        sig: FnSigId::from_u32(99),
        body: Some(BodyId::from_u32(11)),
    };
    let without = InstanceKind::Function {
        sig: FnSigId::from_u32(1),
        body: None,
    };
    assert_eq!(
        regseq_instance_kind_rank(&with),
        regseq_instance_kind_rank(&without)
    );
}

// ------------------------------------------------------------------
// RustcInstanceKindTag + rustc_instance_kind_sort_rank: pinned
// 0..=14 matching the rustc 15-variant taxonomy.
// ------------------------------------------------------------------

fn all_rustc_tags() -> Vec<RustcInstanceKindTag> {
    use RustcInstanceKindTag::*;
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
fn rustc_instance_kind_tag_has_15_variants() {
    assert_eq!(
        all_rustc_tags().len(),
        15,
        "RustcInstanceKindTag mirrors rustc's 15-variant InstanceKind"
    );
}

#[test]
fn rustc_instance_kind_sort_rank_matches_pinned_table() {
    use RustcInstanceKindTag::*;
    let cases: &[(RustcInstanceKindTag, u8)] = &[
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
    for (tag, expected) in cases {
        assert_eq!(rustc_instance_kind_sort_rank(*tag), *expected);
    }
}

#[test]
fn rustc_instance_kind_sort_ranks_are_unique_and_dense() {
    let ranks: Vec<u8> = all_rustc_tags()
        .iter()
        .copied()
        .map(rustc_instance_kind_sort_rank)
        .collect();
    let mut sorted = ranks.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(sorted.len(), ranks.len(), "ranks are unique across tags");
    assert_eq!(
        sorted,
        (0u8..15).collect::<Vec<_>>(),
        "ranks form the dense range 0..=14"
    );
}

// ------------------------------------------------------------------
// Instance struct: round-trip + spec wire shape.
// ------------------------------------------------------------------

#[test]
fn instance_round_trips() {
    let inst = Instance {
        item: ItemId::from_u32(2),
        generic_args: vec![
            GenericArg::Type(TyId::from_u32(0)),
            GenericArg::Const(ConstId::from_u32(1)),
        ],
        symbol_name: StringId::from_u32(33),
        span: span_zero(),
        kind: InstanceKind::Function {
            sig: FnSigId::from_u32(0),
            body: Some(BodyId::from_u32(7)),
        },
    };
    let json = serde_json::to_string(&inst).unwrap();
    let back: Instance = serde_json::from_str(&json).unwrap();
    assert_eq!(inst, back);
}

#[test]
fn instance_round_trips_with_synthesized_span_and_no_body() {
    let inst = Instance {
        item: ItemId::from_u32(0),
        generic_args: vec![],
        symbol_name: StringId::ZERO,
        span: span_zero(),
        kind: InstanceKind::Function {
            sig: FnSigId::from_u32(1),
            body: None,
        },
    };
    let json = serde_json::to_string(&inst).unwrap();
    let back: Instance = serde_json::from_str(&json).unwrap();
    assert_eq!(inst, back);
    assert_eq!(back.span.file, SourceFileId::SYNTHESIZED);
}

#[test]
fn instance_json_top_level_shape() {
    let inst = Instance {
        item: ItemId::from_u32(2),
        generic_args: vec![GenericArg::Type(TyId::from_u32(3))],
        symbol_name: StringId::from_u32(33),
        span: Span {
            file: SourceFileId::from_u32(4),
            lo: 100,
            hi: 200,
        },
        kind: InstanceKind::Static {
            ty: TyId::from_u32(5),
            init: ConstId::from_u32(6),
        },
    };
    let v = serde_json::to_value(&inst).unwrap();
    assert!(v.is_object());
    assert_eq!(v["item"], serde_json::json!(2));
    assert_eq!(v["generic_args"], serde_json::json!([{"Type": 3}]));
    assert_eq!(v["symbol_name"], serde_json::json!(33));
    assert_eq!(v["span"], serde_json::json!({"file": 4, "lo": 100, "hi": 200}));
    assert_eq!(
        v["kind"],
        serde_json::json!({"Static": {"ty": 5, "init": 6}})
    );
}

// ------------------------------------------------------------------
// Cross-check: every Regseq InstanceKind with rank 0 (Function)
// composes meaningfully with any RustcInstanceKindTag for the F4-2
// fn_shim_disambig field. We don't compute the full key here (that
// is bridge state per spec) but assert the coexistence.
// ------------------------------------------------------------------

#[test]
fn function_kind_rank_zero_composes_with_each_rustc_tag() {
    let function = InstanceKind::Function {
        sig: FnSigId::from_u32(0),
        body: None,
    };
    assert_eq!(regseq_instance_kind_rank(&function), 0);
    for tag in all_rustc_tags() {
        let _disambig = rustc_instance_kind_sort_rank(tag);
        // Witness: `disambig` is a u8, freely composable with the
        // Regseq rank as the F4-2 sort key tail.
    }
}
