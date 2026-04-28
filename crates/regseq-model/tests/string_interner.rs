//! Integration tests for stage-1 step 3: `StringInterner`.
//!
//! Validates the spec requirements from `StringInterner` in
//! `docs/discussions/regseq-model/stage-1-vocabulary.md`:
//! - `new()` pre-interns the empty string at `StringId::ZERO`.
//! - `intern` is content-deduplicating; equal strings collapse to
//!   the same `StringId`.
//! - `resolve(id)` returns the interned string for any assigned ID.
//! - JSON wire shape is a flat array of strings; `lookup` is
//!   reconstructed on deserialize.
//! - Deserialize rejects: empty array, non-empty index 0, and
//!   duplicate entries.

use regseq_model::{StringId, StringInterner};

#[test]
fn new_pre_interns_empty_string_at_zero() {
    let interner = StringInterner::new();
    assert_eq!(interner.len(), 1);
    assert_eq!(interner.resolve(StringId::ZERO), "");
}

#[test]
fn default_matches_new() {
    let a = StringInterner::default();
    let b = StringInterner::new();
    assert_eq!(a.len(), b.len());
    assert_eq!(a.resolve(StringId::ZERO), b.resolve(StringId::ZERO));
}

#[test]
fn intern_empty_string_returns_zero() {
    let mut interner = StringInterner::new();
    let id = interner.intern("");
    assert_eq!(id, StringId::ZERO);
    assert_eq!(interner.len(), 1, "empty re-intern must not grow storage");
}

#[test]
fn intern_assigns_sequential_ids_starting_at_one() {
    let mut interner = StringInterner::new();
    let foo = interner.intern("foo");
    let bar = interner.intern("bar");
    let baz = interner.intern("baz");
    assert_eq!(foo, StringId::from_u32(1));
    assert_eq!(bar, StringId::from_u32(2));
    assert_eq!(baz, StringId::from_u32(3));
    assert_eq!(interner.len(), 4);
}

#[test]
fn intern_is_content_deduplicating() {
    let mut interner = StringInterner::new();
    let a = interner.intern("hello");
    let b = interner.intern("world");
    let a_again = interner.intern("hello");
    assert_eq!(a, a_again);
    assert_ne!(a, b);
    assert_eq!(interner.len(), 3, "duplicate must not allocate a new ID");
}

#[test]
fn resolve_returns_interned_string() {
    let mut interner = StringInterner::new();
    let id = interner.intern("payload");
    assert_eq!(interner.resolve(id), "payload");
    assert_eq!(interner.resolve(StringId::ZERO), "");
}

#[test]
fn iter_yields_in_id_order() {
    let mut interner = StringInterner::new();
    interner.intern("alpha");
    interner.intern("beta");
    let pairs: Vec<(StringId, &str)> = interner.iter().collect();
    assert_eq!(
        pairs,
        vec![
            (StringId::from_u32(0), ""),
            (StringId::from_u32(1), "alpha"),
            (StringId::from_u32(2), "beta"),
        ]
    );
}

#[test]
fn json_wire_shape_is_flat_array_of_strings() {
    let mut interner = StringInterner::new();
    interner.intern("foo");
    interner.intern("bar");
    let json = serde_json::to_string(&interner).unwrap();
    assert_eq!(json, r#"["","foo","bar"]"#);
}

#[test]
fn json_round_trip_preserves_string_ids() {
    let mut original = StringInterner::new();
    let id_foo = original.intern("foo");
    let id_bar = original.intern("bar");
    let id_baz = original.intern("baz");

    let json = serde_json::to_string(&original).unwrap();
    let recovered: StringInterner = serde_json::from_str(&json).unwrap();

    assert_eq!(recovered.len(), original.len());
    assert_eq!(recovered.resolve(StringId::ZERO), "");
    assert_eq!(recovered.resolve(id_foo), "foo");
    assert_eq!(recovered.resolve(id_bar), "bar");
    assert_eq!(recovered.resolve(id_baz), "baz");
}

#[test]
fn json_round_trip_rebuilds_lookup_for_intern() {
    // After deserialize, intern-ing an existing string must return
    // its original ID, proving the lookup map was rebuilt.
    let mut original = StringInterner::new();
    let id = original.intern("foo");
    let json = serde_json::to_string(&original).unwrap();
    let mut recovered: StringInterner = serde_json::from_str(&json).unwrap();
    assert_eq!(recovered.intern("foo"), id);
    assert_eq!(recovered.len(), original.len());
}

#[test]
fn deserialize_rejects_empty_array() {
    let result: Result<StringInterner, _> = serde_json::from_str("[]");
    let err = result.expect_err("empty interner must be rejected");
    let msg = err.to_string();
    assert!(msg.contains("empty"), "error explains the failure: {msg}");
}

#[test]
fn deserialize_rejects_non_empty_index_zero() {
    let result: Result<StringInterner, _> = serde_json::from_str(r#"["not-empty","foo"]"#);
    let err = result.expect_err("non-empty index 0 must be rejected");
    let msg = err.to_string();
    assert!(msg.contains("index 0"), "error names the position: {msg}");
    assert!(msg.contains("empty"), "error names the requirement: {msg}");
}

#[test]
fn deserialize_rejects_duplicate_entries() {
    let result: Result<StringInterner, _> =
        serde_json::from_str(r#"["","foo","bar","foo"]"#);
    let err = result.expect_err("duplicate entry must be rejected");
    let msg = err.to_string();
    assert!(msg.contains("duplicate"), "error names the cause: {msg}");
    assert!(msg.contains("foo"), "error names the duplicated value: {msg}");
}

#[test]
fn deserialize_rejects_duplicate_empty_strings() {
    // Two empty strings at indices 0 and 5 must also be rejected.
    let result: Result<StringInterner, _> =
        serde_json::from_str(r#"["","a","b","c","d",""]"#);
    let err = result.expect_err("duplicate empty entry must be rejected");
    assert!(
        err.to_string().contains("duplicate"),
        "error names the cause: {err}"
    );
}

#[test]
fn debug_format_omits_lookup_for_determinism() {
    let mut interner = StringInterner::new();
    interner.intern("foo");
    let dbg = format!("{:?}", interner);
    assert!(
        dbg.contains("storage"),
        "Debug must surface storage: {dbg}"
    );
    assert!(
        !dbg.contains("lookup"),
        "Debug must not surface lookup (nondeterministic ordering): {dbg}"
    );
}
