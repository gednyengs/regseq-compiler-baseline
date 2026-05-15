//! Content-keyed const interner.

use std::collections::HashMap;
use std::ops::Index;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::const_value::{Byte, Const};
use crate::ids::ConstId;
use crate::index_vec::IndexVec;

/// Content-keyed const interner.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConstInterner {
    storage: IndexVec<ConstId, Const>,
    lookup: HashMap<ConstIdentityKey, ConstId>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct ConstIdentityKey(Vec<u8>);

// The public API is fixed to the six spec-mandated methods (`new`,
// `intern`, `resolve`, `find`, `len`, `iter`) per task spec
// `.harness/tasks/regseq-model/69-const-vocabulary.md:140-155`;
// clippy's `len_without_is_empty` lint would push us to add a
// non-spec `is_empty`, so suppress it on this impl.
#[allow(clippy::len_without_is_empty)]
impl ConstInterner {
    /// Returns an empty const interner.
    pub fn new() -> Self {
        Self {
            storage: IndexVec::new(),
            lookup: HashMap::new(),
        }
    }

    /// Interns `c` and returns the content identity's `ConstId`.
    pub fn intern(&mut self, c: Const) -> ConstId {
        let key = ConstIdentityKey::from_const(&c);
        if let Some(id) = self.lookup.get(&key) {
            return *id;
        }
        let id = self.storage.push(c);
        self.lookup.insert(key, id);
        id
    }

    /// Resolves `id` to its stored const.
    pub fn resolve(&self, id: ConstId) -> &Const {
        &self.storage[id]
    }

    /// Finds an interned const by private identity key.
    #[allow(private_interfaces)]
    pub fn find(&self, key: &ConstIdentityKey) -> Option<ConstId> {
        self.lookup.get(key).copied()
    }

    /// Number of stored const identities.
    pub fn len(&self) -> usize {
        self.storage.len()
    }

    /// Iterator over `(ConstId, &Const)` in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = (ConstId, &Const)> {
        self.storage
            .iter()
            .enumerate()
            .map(|(idx, c)| (ConstId::from_u32(idx as u32), c))
    }
}

impl Default for ConstInterner {
    fn default() -> Self {
        Self::new()
    }
}

impl Index<ConstId> for ConstInterner {
    type Output = Const;

    fn index(&self, index: ConstId) -> &Self::Output {
        self.resolve(index)
    }
}

impl Serialize for ConstInterner {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.storage.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for ConstInterner {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let storage = IndexVec::<ConstId, Const>::deserialize(deserializer)?;
        validate_const_references(&storage).map_err(serde::de::Error::custom)?;

        let mut lookup = HashMap::new();
        for (idx, value) in storage.iter().enumerate() {
            let id = ConstId::from_u32(idx as u32);
            let key = ConstIdentityKey::from_const(value);
            if let Some(first) = lookup.insert(key, id) {
                return Err(serde::de::Error::custom(format!(
                    "duplicate Const identity at ConstId {} and ConstId {}",
                    first.as_u32(),
                    id.as_u32()
                )));
            }
        }

        Ok(Self { storage, lookup })
    }
}

impl ConstIdentityKey {
    /// Builds the identity key for `value`.
    ///
    /// The key reuses the §9.7 per-variant tags (0..=10) but stores
    /// nested `ConstId` and `TyId` references as raw little-endian
    /// `u32` payloads rather than resolving them recursively to
    /// canonical type or const content. This parallels DS-04's
    /// `TyIdentityKey` (see `ty_interner.rs`), where nested `TyId` /
    /// `ItemId` references are also stored raw (e.g.,
    /// `Array(TyId, u64)`, `FnDef(ItemId, GenericArgs)`).
    ///
    /// The within-model uniqueness invariant (spec construction
    /// invariant 11) still holds because `TyInterner` and
    /// `ConstInterner` are themselves content-keyed: distinct
    /// `TyId`s and `ConstId`s within one model already imply
    /// distinct underlying content. Cross-build canonical bytes
    /// (used for `ArgsKey` and §16.3 deterministic hashing) are
    /// computed separately by `canonical_const_bytes` in
    /// `const_value.rs`, which has the full model and resolves
    /// nested IDs to canonical content per spec.md:3134-3155.
    pub(crate) fn from_const(value: &Const) -> Self {
        let mut out = Vec::new();
        encode_identity_const(value, &mut out);
        Self(out)
    }
}

fn validate_const_references(storage: &IndexVec<ConstId, Const>) -> Result<(), String> {
    for (idx, value) in storage.iter().enumerate() {
        let owner = ConstId::from_u32(idx as u32);
        validate_const(owner, value, storage.len())?;
    }
    Ok(())
}

fn validate_const(owner: ConstId, value: &Const, len: usize) -> Result<(), String> {
    match value {
        Const::Aggregate(children) => {
            for child in children {
                check_const_id(owner, *child, len)?;
            }
        }
        Const::EnumValue { payload, .. } => {
            for child in payload {
                check_const_id(owner, *child, len)?;
            }
        }
        Const::Bool(_)
        | Const::Char(_)
        | Const::Int(_, _)
        | Const::Uint(_, _)
        | Const::Float(_, _)
        | Const::ZST
        | Const::Bytes(_)
        | Const::NullPtr(_)
        | Const::Reloc(_) => {}
    }
    Ok(())
}

fn encode_identity_const(value: &Const, out: &mut Vec<u8>) {
    match value {
        Const::Bool(value) => {
            out.push(0);
            out.push(u8::from(*value));
        }
        Const::Char(value) => {
            out.push(1);
            out.extend_from_slice(&value.to_le_bytes());
        }
        Const::Int(kind, value) => {
            out.push(2);
            out.push((*kind).into());
            out.extend_from_slice(&value.to_le_bytes());
        }
        Const::Uint(kind, value) => {
            out.push(3);
            out.push((*kind).into());
            out.extend_from_slice(&value.to_le_bytes());
        }
        Const::Float(kind, value) => {
            out.push(4);
            out.push((*kind).into());
            out.extend_from_slice(&value.to_le_bytes());
        }
        Const::ZST => out.push(5),
        Const::Bytes(bytes) => {
            out.push(6);
            push_u32(out, bytes.len());
            for byte in bytes {
                encode_identity_byte(*byte, out);
            }
        }
        Const::Aggregate(children) => {
            out.push(7);
            push_u32(out, children.len());
            for child in children {
                out.extend_from_slice(&child.as_u32().to_le_bytes());
            }
        }
        Const::EnumValue {
            variant_idx,
            payload,
        } => {
            out.push(8);
            out.extend_from_slice(&variant_idx.as_u32().to_le_bytes());
            push_u32(out, payload.len());
            for child in payload {
                out.extend_from_slice(&child.as_u32().to_le_bytes());
            }
        }
        Const::NullPtr(ty) => {
            out.push(9);
            out.extend_from_slice(&ty.as_u32().to_le_bytes());
        }
        Const::Reloc(instance) => {
            out.push(10);
            out.extend_from_slice(&instance.canonical_bytes());
        }
    }
}

fn encode_identity_byte(byte: Byte, out: &mut Vec<u8>) {
    match byte {
        Byte::Uninit => out.push(0),
        Byte::Value(value) => {
            out.push(1);
            out.push(value);
        }
        Byte::Reloc(instance) => {
            out.push(2);
            out.extend_from_slice(&instance.canonical_bytes());
        }
    }
}

fn check_const_id(owner: ConstId, target: ConstId, len: usize) -> Result<(), String> {
    if target.index() >= len {
        return Err(format!(
            "ConstId {} references out-of-bounds ConstId {} with storage len {}",
            owner.as_u32(),
            target.as_u32(),
            len
        ));
    }
    Ok(())
}

fn push_u32(out: &mut Vec<u8>, value: usize) {
    let value = u32::try_from(value).expect("canonical length exceeds u32::MAX");
    out.extend_from_slice(&value.to_le_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::args_key::ArgsKey;
    use crate::def_path::DefPathHash;
    use crate::instance_ref::InstanceRef;
    use crate::ty::{FloatKind, IntKind, UintKind};
    use crate::{StableCrateId, TyId, VariantIdx};

    fn instance_ref() -> InstanceRef {
        InstanceRef {
            regseq_kind_rank: 0,
            def_path_hash: DefPathHash {
                stable_crate_id: StableCrateId(0xAABB),
                local_hash: 0xCCDD,
            },
            args_key: ArgsKey(0xEEFF),
            fn_shim_disambig: 5,
        }
    }

    #[test]
    fn new_returns_empty_interner() {
        let interner = ConstInterner::new();
        assert_eq!(interner.len(), 0);
        assert_eq!(interner.iter().count(), 0);
    }

    #[test]
    fn intern_deduplicates_and_resolves_scalars() {
        let mut interner = ConstInterner::new();
        let true_a = interner.intern(Const::Bool(true));
        let true_b = interner.intern(Const::Bool(true));
        let false_id = interner.intern(Const::Bool(false));

        assert_eq!(true_a, ConstId::ZERO);
        assert_eq!(true_a, true_b);
        assert_ne!(true_a, false_id);
        assert_eq!(interner.len(), 2);
        assert_eq!(interner.resolve(true_a), &Const::Bool(true));

        let key = ConstIdentityKey::from_const(&Const::Bool(true));
        assert_eq!(interner.find(&key), Some(true_a));
        let missing = ConstIdentityKey::from_const(&Const::Char(0));
        assert_eq!(interner.find(&missing), None);
    }

    #[test]
    fn iter_yields_pairs_in_insertion_order() {
        let mut interner = ConstInterner::new();
        let a = interner.intern(Const::Bool(true));
        let b = interner.intern(Const::Bool(false));
        assert_eq!(
            interner.iter().collect::<Vec<_>>(),
            vec![(a, &Const::Bool(true)), (b, &Const::Bool(false))]
        );
    }

    #[test]
    fn per_variant_identity_deduplicates_equal_values() {
        let mut interner = ConstInterner::new();
        let child1 = interner.intern(Const::Bool(true));
        let child2 = interner.intern(Const::Int(IntKind::I32, 42));
        let values = vec![
            Const::Bool(true),
            Const::Char(0x61),
            Const::Int(IntKind::I32, 42),
            Const::Uint(UintKind::U64, 42),
            Const::Float(FloatKind::F32, 0x3f80_0000),
            Const::ZST,
            Const::Bytes(vec![Byte::Value(1), Byte::Value(2)]),
            Const::Aggregate(vec![child1, child2]),
            Const::EnumValue {
                variant_idx: VariantIdx::ZERO,
                payload: vec![child1],
            },
            Const::NullPtr(TyId::ZERO),
            Const::Reloc(instance_ref()),
        ];

        for value in values {
            let first = interner.intern(value.clone());
            let second = interner.intern(value);
            assert_eq!(first, second);
        }

        assert_ne!(
            interner.intern(Const::Int(IntKind::I32, 42)),
            interner.intern(Const::Int(IntKind::I64, 42))
        );
        assert_ne!(
            interner.intern(Const::NullPtr(TyId::from_u32(1))),
            interner.intern(Const::NullPtr(TyId::from_u32(2)))
        );
    }

    #[test]
    fn serde_round_trip_is_storage_only_and_rebuilds_lookup() {
        let mut interner = ConstInterner::new();
        let a = interner.intern(Const::Bool(true));
        let b = interner.intern(Const::Bool(false));
        let c = interner.intern(Const::Int(IntKind::I32, 42));
        let d = interner.intern(Const::Aggregate(vec![a, c]));
        let e = interner.intern(Const::Reloc(instance_ref()));

        let json = serde_json::to_string(&interner).unwrap();
        let back: ConstInterner = serde_json::from_str(&json).unwrap();

        assert_eq!(back.len(), 5);
        for id in [a, b, c, d, e] {
            assert_eq!(back.resolve(id), interner.resolve(id));
        }
        let key = ConstIdentityKey::from_const(&Const::Bool(true));
        assert_eq!(back.find(&key), Some(a));
    }

    #[test]
    fn serde_round_trips_null_ptr_payload() {
        // Regression: an earlier identity-key design required type
        // context to encode `Const::NullPtr(TyId)` and therefore
        // rejected any serialised model whose storage contained a
        // valid `NullPtr` payload at deserialise time. The identity
        // key now uses raw `TyId` bytes (sufficient under interner
        // construction invariant 11 because `TyInterner` is
        // content-keyed within a model), so round-trip succeeds.
        let mut interner = ConstInterner::new();
        let null_zero = interner.intern(Const::NullPtr(TyId::ZERO));
        let null_one = interner.intern(Const::NullPtr(TyId::from_u32(1)));
        let aggregate = interner.intern(Const::Aggregate(vec![null_zero, null_one]));

        let json = serde_json::to_string(&interner).unwrap();
        let back: ConstInterner = serde_json::from_str(&json).unwrap();

        assert_eq!(back.len(), 3);
        assert_eq!(back.resolve(null_zero), &Const::NullPtr(TyId::ZERO));
        assert_eq!(back.resolve(null_one), &Const::NullPtr(TyId::from_u32(1)));
        assert_eq!(
            back.resolve(aggregate),
            &Const::Aggregate(vec![null_zero, null_one])
        );

        // Post-deserialise lookup is functional for NullPtr.
        let key = ConstIdentityKey::from_const(&Const::NullPtr(TyId::ZERO));
        assert_eq!(back.find(&key), Some(null_zero));
    }

    #[test]
    fn null_ptr_identity_distinguishes_distinct_ty_ids() {
        // `Const::NullPtr` identity is keyed on the raw `TyId`.
        // Distinct `TyId`s within a model already imply distinct
        // type content (TyInterner is content-keyed), so this is
        // sufficient for invariant 11.
        let mut interner = ConstInterner::new();
        let bool_null = interner.intern(Const::NullPtr(TyId::ZERO));
        let char_null = interner.intern(Const::NullPtr(TyId::from_u32(1)));
        assert_ne!(bool_null, char_null);

        // Re-interning the same `TyId` collapses.
        let bool_null_again = interner.intern(Const::NullPtr(TyId::ZERO));
        assert_eq!(bool_null, bool_null_again);
    }

    #[test]
    fn serde_rejects_duplicate_identity() {
        let err =
            serde_json::from_str::<ConstInterner>(r#"[{"Bool":true},{"Bool":true}]"#).unwrap_err();
        assert!(err.to_string().contains("duplicate Const identity"));
    }

    #[test]
    fn serde_rejects_dangling_const_id_reference() {
        let err = serde_json::from_str::<ConstInterner>(r#"[{"Aggregate":[99]}]"#).unwrap_err();
        assert!(err.to_string().contains("out-of-bounds ConstId"));
    }

    #[test]
    fn const_id_option_is_four_bytes() {
        const _: () = assert!(core::mem::size_of::<Option<ConstId>>() == 4);
        assert_eq!(core::mem::size_of::<Option<ConstId>>(), 4);
    }
}
