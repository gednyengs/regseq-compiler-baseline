//! Integration tests for stage-1 step-1: the `Idx` trait,
//! `newtype_index!` macro, and `CrateId`.
//!
//! Locks in the three properties named in
//! `docs/discussions/regseq-model/status.md`:
//! - JSON round-trip preserves the logical index.
//! - Niche encoding stays internal (the `NonMaxU32` wrapper and the
//!   `private_use_methods_instead` field are not reachable from
//!   here; if this file compiles, encapsulation holds).
//! - `Option<CrateId>` is 4 bytes.

use regseq_model::{CrateId, Idx};
use std::cmp::Ordering;
use std::mem::size_of;

#[test]
fn crate_id_round_trips_through_json() {
    for raw in [0u32, 1, 7, 1024, CrateId::MAX_AS_U32] {
        let id = CrateId::from_u32(raw);
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, raw.to_string(), "wire shape is the logical index");
        let back: CrateId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, back);
        assert_eq!(back.as_u32(), raw);
    }
}

#[test]
fn crate_id_rejects_max_u32_on_deserialize() {
    let json = u32::MAX.to_string();
    let result: Result<CrateId, _> = serde_json::from_str(&json);
    let err = result.expect_err("u32::MAX must be rejected (niche-reserved)");
    let msg = err.to_string();
    assert!(msg.contains("CrateId"), "error names the type: {msg}");
    assert!(
        msg.contains(&u32::MAX.to_string()),
        "error names the offending value: {msg}"
    );
    assert!(msg.contains("MAX"), "error references the bound: {msg}");
}

#[test]
fn idx_round_trip_through_trait() {
    let id = <CrateId as Idx>::new(42);
    assert_eq!(<CrateId as Idx>::index(id), 42);
    assert_eq!(<CrateId as Idx>::new(<CrateId as Idx>::index(id)), id);
}

#[test]
fn crate_id_ord_matches_logical_u32() {
    let a = CrateId::from_u32(3);
    let b = CrateId::from_u32(7);
    assert!(a < b);
    assert_eq!(a.cmp(&b), Ordering::Less);
    assert_eq!(b.cmp(&a), Ordering::Greater);
    assert_eq!(a.cmp(&a), Ordering::Equal);
}

#[test]
fn crate_id_const_associates() {
    assert_eq!(CrateId::ZERO.as_u32(), 0);
    assert_eq!(CrateId::MAX.as_u32(), CrateId::MAX_AS_U32);
    assert_eq!(CrateId::MAX_AS_U32, u32::MAX - 1);
}

#[test]
#[should_panic]
fn from_u32_panics_on_u32_max() {
    let _ = CrateId::from_u32(u32::MAX);
}

#[test]
#[should_panic]
fn idx_new_panics_above_max() {
    let _ = <CrateId as Idx>::new(CrateId::MAX_AS_U32 as usize + 1);
}

#[test]
fn option_crate_id_is_four_bytes() {
    assert_eq!(size_of::<Option<CrateId>>(), 4);
    assert_eq!(size_of::<CrateId>(), 4);
}

#[test]
fn debug_format_matches_typename_paren() {
    let id = CrateId::from_u32(7);
    assert_eq!(format!("{:?}", id), "CrateId(7)");
}
