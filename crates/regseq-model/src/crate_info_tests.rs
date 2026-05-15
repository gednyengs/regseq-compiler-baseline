//! Integration tests for stage-1 step-5: `StableCrateId` +
//! `CrateInfo`. Locks in the JSON wire shape, transparent u64
//! serialization, and structural sanity.

use crate::{CrateInfo, StableCrateId, StringId};

#[test]
fn stable_crate_id_serializes_as_bare_u64() {
    let raw: u64 = 0xCAFE_BABE_DEAD_BEEF;
    let id = StableCrateId(raw);
    let json = serde_json::to_string(&id).unwrap();
    assert_eq!(json, raw.to_string(), "wire shape is the bare u64 decimal");
    let back: StableCrateId = serde_json::from_str(&json).unwrap();
    assert_eq!(id, back);
}

#[test]
fn stable_crate_id_round_trips_zero_and_max() {
    for raw in [0u64, 1, 0xFFFF_FFFF, u64::MAX] {
        let id = StableCrateId(raw);
        let json = serde_json::to_string(&id).unwrap();
        let back: StableCrateId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, back);
        assert_eq!(back.0, raw);
    }
}

#[test]
fn stable_crate_id_supports_ord_and_hash() {
    use std::collections::HashSet;

    let a = StableCrateId(1);
    let b = StableCrateId(2);
    assert!(a < b);

    let mut seen = HashSet::new();
    seen.insert(a);
    assert!(seen.contains(&a));
    assert!(!seen.contains(&b));
}

#[test]
fn crate_info_round_trips_through_json() {
    let info = CrateInfo {
        name: StringId::from_u32(7),
        stable_crate_id: StableCrateId(0xABCD),
    };
    let json = serde_json::to_string(&info).unwrap();
    let back: CrateInfo = serde_json::from_str(&json).unwrap();
    assert_eq!(info, back);
}

#[test]
fn crate_info_json_shape_is_object_with_named_fields() {
    let info = CrateInfo {
        name: StringId::from_u32(3),
        stable_crate_id: StableCrateId(42),
    };
    let v: serde_json::Value = serde_json::to_value(&info).unwrap();
    assert!(v.is_object(), "CrateInfo serializes to a JSON object");
    assert_eq!(v["name"], serde_json::json!(3));
    assert_eq!(v["stable_crate_id"], serde_json::json!(42));
}
