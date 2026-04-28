//! Integration tests for stage-1 step-5: `DefPathData`,
//! `DefPathSegment`, `DefPath`, `DefPathHash`,
//! `def_path_data_sort_rank`.

use regseq_model::{
    def_path_data_sort_rank, CrateId, DefPath, DefPathData, DefPathHash, DefPathSegment,
    StableCrateId, StringId,
};

// ------------------------------------------------------------------
// DefPathData: 19 variants, mirrors rustc; round-trip + rank table
// coverage. The constructor list below is authoritative -- adding a
// variant means adding a row here, ensuring the sort-rank coverage
// test sees it.
// ------------------------------------------------------------------

fn all_def_path_data_variants() -> Vec<DefPathData> {
    let s = StringId::from_u32(13);
    vec![
        DefPathData::CrateRoot,
        DefPathData::Impl,
        DefPathData::ForeignMod,
        DefPathData::Use,
        DefPathData::GlobalAsm,
        DefPathData::TypeNs(s),
        DefPathData::ValueNs(s),
        DefPathData::MacroNs(s),
        DefPathData::LifetimeNs(s),
        DefPathData::Closure,
        DefPathData::Ctor,
        DefPathData::AnonConst,
        DefPathData::LateAnonConst,
        DefPathData::DesugaredAnonymousLifetime,
        DefPathData::OpaqueTy,
        DefPathData::OpaqueLifetime(s),
        DefPathData::AnonAssocTy(s),
        DefPathData::SyntheticCoroutineBody,
        DefPathData::NestedStatic,
    ]
}

#[test]
fn def_path_data_has_19_variants() {
    assert_eq!(
        all_def_path_data_variants().len(),
        19,
        "DefPathData is closed at 19 variants per the pinned toolchain"
    );
}

#[test]
fn def_path_data_round_trips_all_variants() {
    for variant in all_def_path_data_variants() {
        let json = serde_json::to_string(&variant).unwrap();
        let back: DefPathData = serde_json::from_str(&json).unwrap();
        assert_eq!(variant, back, "round-trip preserves variant {json}");
    }
}

#[test]
fn def_path_data_uses_externally_tagged_serde_for_named_variants() {
    let v = DefPathData::TypeNs(StringId::from_u32(5));
    let json = serde_json::to_value(&v).unwrap();
    assert_eq!(json, serde_json::json!({"TypeNs": 5}));

    let v = DefPathData::Impl;
    let json = serde_json::to_value(&v).unwrap();
    assert_eq!(json, serde_json::json!("Impl"));
}

// ------------------------------------------------------------------
// def_path_data_sort_rank: pinned 0..=18, append-only.
// ------------------------------------------------------------------

#[test]
fn def_path_data_sort_rank_matches_pinned_table() {
    let s = StringId::from_u32(0);
    let table: &[(DefPathData, u8)] = &[
        (DefPathData::CrateRoot, 0),
        (DefPathData::Impl, 1),
        (DefPathData::ForeignMod, 2),
        (DefPathData::Use, 3),
        (DefPathData::GlobalAsm, 4),
        (DefPathData::TypeNs(s), 5),
        (DefPathData::ValueNs(s), 6),
        (DefPathData::MacroNs(s), 7),
        (DefPathData::LifetimeNs(s), 8),
        (DefPathData::Closure, 9),
        (DefPathData::Ctor, 10),
        (DefPathData::AnonConst, 11),
        (DefPathData::LateAnonConst, 12),
        (DefPathData::DesugaredAnonymousLifetime, 13),
        (DefPathData::OpaqueTy, 14),
        (DefPathData::OpaqueLifetime(s), 15),
        (DefPathData::AnonAssocTy(s), 16),
        (DefPathData::SyntheticCoroutineBody, 17),
        (DefPathData::NestedStatic, 18),
    ];

    for (data, expected) in table {
        let actual = def_path_data_sort_rank(data);
        assert_eq!(
            actual, *expected,
            "rank for {:?} must be pinned at {} (got {})",
            data, expected, actual
        );
    }
}

#[test]
fn def_path_data_sort_ranks_are_unique_and_dense() {
    let ranks: Vec<u8> = all_def_path_data_variants()
        .iter()
        .map(def_path_data_sort_rank)
        .collect();
    let mut sorted = ranks.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(
        sorted.len(),
        ranks.len(),
        "ranks are unique across variants"
    );
    assert_eq!(
        sorted,
        (0u8..19).collect::<Vec<_>>(),
        "ranks form the dense range 0..=18"
    );
}

#[test]
fn def_path_data_sort_rank_ignores_payload() {
    let a = DefPathData::TypeNs(StringId::from_u32(0));
    let b = DefPathData::TypeNs(StringId::from_u32(1024));
    assert_eq!(def_path_data_sort_rank(&a), def_path_data_sort_rank(&b));
}

// ------------------------------------------------------------------
// DefPathSegment + DefPath + DefPathHash structural / wire shape.
// ------------------------------------------------------------------

#[test]
fn def_path_segment_round_trips() {
    let seg = DefPathSegment {
        data: DefPathData::ValueNs(StringId::from_u32(11)),
        disambiguator: 2,
    };
    let json = serde_json::to_string(&seg).unwrap();
    let back: DefPathSegment = serde_json::from_str(&json).unwrap();
    assert_eq!(seg, back);
}

#[test]
fn def_path_segment_json_shape_has_named_fields() {
    let seg = DefPathSegment {
        data: DefPathData::Impl,
        disambiguator: 3,
    };
    let v = serde_json::to_value(&seg).unwrap();
    assert!(v.is_object());
    assert_eq!(v["data"], serde_json::json!("Impl"));
    assert_eq!(v["disambiguator"], serde_json::json!(3));
}

#[test]
fn def_path_round_trips() {
    let path = DefPath {
        crate_id: CrateId::from_u32(3),
        segments: vec![
            DefPathSegment {
                data: DefPathData::CrateRoot,
                disambiguator: 0,
            },
            DefPathSegment {
                data: DefPathData::TypeNs(StringId::from_u32(5)),
                disambiguator: 0,
            },
            DefPathSegment {
                data: DefPathData::Impl,
                disambiguator: 1,
            },
            DefPathSegment {
                data: DefPathData::ValueNs(StringId::from_u32(6)),
                disambiguator: 0,
            },
        ],
        rendered: StringId::from_u32(99),
    };
    let json = serde_json::to_string(&path).unwrap();
    let back: DefPath = serde_json::from_str(&json).unwrap();
    assert_eq!(path, back);
}

#[test]
fn def_path_json_shape_has_named_fields() {
    let path = DefPath {
        crate_id: CrateId::from_u32(0),
        segments: vec![],
        rendered: StringId::from_u32(7),
    };
    let v = serde_json::to_value(&path).unwrap();
    assert!(v.is_object());
    assert_eq!(v["crate_id"], serde_json::json!(0));
    assert!(v["segments"].is_array());
    assert_eq!(v["rendered"], serde_json::json!(7));
}

#[test]
fn def_path_hash_round_trips() {
    let h = DefPathHash {
        stable_crate_id: StableCrateId(0xDEAD_BEEF),
        local_hash: 0xCAFE_BABE_F00D_BAA5,
    };
    let json = serde_json::to_string(&h).unwrap();
    let back: DefPathHash = serde_json::from_str(&json).unwrap();
    assert_eq!(h, back);
}

#[test]
fn def_path_hash_json_shape_has_named_fields() {
    let h = DefPathHash {
        stable_crate_id: StableCrateId(7),
        local_hash: 11,
    };
    let v = serde_json::to_value(&h).unwrap();
    assert!(v.is_object());
    assert_eq!(v["stable_crate_id"], serde_json::json!(7));
    assert_eq!(v["local_hash"], serde_json::json!(11));
}
