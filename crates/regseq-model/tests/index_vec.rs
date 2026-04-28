//! Integration tests for stage-1 step 2: `IndexVec<I, T>`.
//!
//! Validates the four spec requirements from the IndexVec section
//! of `docs/discussions/regseq-model/stage-1-vocabulary.md`:
//! - `push` returns the assigned ID;
//! - `iter`/`iter_mut`/indexing by `I` work and yield in order;
//! - length-as-`I::index` semantics (iteration order equals ID
//!   order);
//! - JSON serialization is a flat array of `T`, with the phantom
//!   type parameter invisible.

use regseq_model::{CrateId, IndexVec};

#[test]
fn new_is_empty() {
    let v: IndexVec<CrateId, String> = IndexVec::new();
    assert_eq!(v.len(), 0);
    assert!(v.is_empty());
}

#[test]
fn default_equals_new() {
    let a: IndexVec<CrateId, String> = IndexVec::default();
    let b: IndexVec<CrateId, String> = IndexVec::new();
    assert_eq!(a, b);
}

#[test]
fn push_returns_sequential_ids_starting_at_zero() {
    let mut v: IndexVec<CrateId, &'static str> = IndexVec::new();
    let id0 = v.push("alpha");
    let id1 = v.push("beta");
    let id2 = v.push("gamma");
    assert_eq!(id0, CrateId::from_u32(0));
    assert_eq!(id1, CrateId::from_u32(1));
    assert_eq!(id2, CrateId::from_u32(2));
    assert_eq!(v.len(), 3);
}

#[test]
fn indexing_by_id_returns_pushed_value() {
    let mut v: IndexVec<CrateId, &'static str> = IndexVec::new();
    let id0 = v.push("alpha");
    let id1 = v.push("beta");
    assert_eq!(v[id0], "alpha");
    assert_eq!(v[id1], "beta");
}

#[test]
fn index_mut_writes_through() {
    let mut v: IndexVec<CrateId, String> = IndexVec::new();
    let id = v.push("before".to_string());
    v[id] = "after".to_string();
    assert_eq!(v[id], "after");
}

#[test]
fn iter_yields_in_id_order() {
    let mut v: IndexVec<CrateId, &'static str> = IndexVec::new();
    v.push("first");
    v.push("second");
    v.push("third");
    let collected: Vec<&&str> = v.iter().collect();
    assert_eq!(collected, vec![&"first", &"second", &"third"]);
}

#[test]
fn iter_mut_visits_in_id_order() {
    let mut v: IndexVec<CrateId, i32> = IndexVec::new();
    v.push(1);
    v.push(2);
    v.push(3);
    for x in v.iter_mut() {
        *x *= 10;
    }
    let collected: Vec<i32> = v.iter().copied().collect();
    assert_eq!(collected, vec![10, 20, 30]);
}

#[test]
fn json_wire_shape_is_flat_array() {
    let mut v: IndexVec<CrateId, &'static str> = IndexVec::new();
    v.push("alpha");
    v.push("beta");
    let json = serde_json::to_string(&v).unwrap();
    assert_eq!(
        json, r#"["alpha","beta"]"#,
        "phantom-data type parameter must not surface in JSON",
    );
}

#[test]
fn json_round_trip_preserves_order_and_ids() {
    let mut original: IndexVec<CrateId, String> = IndexVec::new();
    let id0 = original.push("alpha".to_string());
    let id1 = original.push("beta".to_string());

    let json = serde_json::to_string(&original).unwrap();
    let recovered: IndexVec<CrateId, String> = serde_json::from_str(&json).unwrap();

    assert_eq!(recovered.len(), original.len());
    assert_eq!(recovered[id0], original[id0]);
    assert_eq!(recovered[id1], original[id1]);
    assert_eq!(recovered, original);
}

#[test]
fn json_empty_vec_round_trips() {
    let original: IndexVec<CrateId, u32> = IndexVec::new();
    let json = serde_json::to_string(&original).unwrap();
    assert_eq!(json, "[]");
    let recovered: IndexVec<CrateId, u32> = serde_json::from_str(&json).unwrap();
    assert_eq!(recovered, original);
}

#[test]
fn debug_format_omits_phantom_field() {
    let mut v: IndexVec<CrateId, i32> = IndexVec::new();
    v.push(7);
    v.push(11);
    let dbg = format!("{:?}", v);
    assert_eq!(dbg, "[7, 11]", "Debug must not surface phantom marker");
}
