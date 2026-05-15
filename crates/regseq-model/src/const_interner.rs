//! Content-keyed const interner.

use std::collections::HashMap;
use std::ops::Index;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::args_key::encode_ty_with_tables;
use crate::const_value::{Byte, Const};
use crate::def_path::DefPathHash;
use crate::fn_sig_interner::FnSigInterner;
use crate::ids::{ConstId, ItemId};
use crate::index_vec::IndexVec;
use crate::ty_interner::TyInterner;

/// Content-keyed const interner.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConstInterner {
    storage: IndexVec<ConstId, Const>,
    lookup: HashMap<ConstIdentityKey, ConstId>,
    ty_context: Option<ConstTyContext>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct ConstIdentityKey(Vec<u8>);

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ConstTyContext {
    types: TyInterner,
    sigs: FnSigInterner,
    item_def_paths: IndexVec<ItemId, DefPathHash>,
}

impl ConstInterner {
    /// Returns an empty const interner.
    pub fn new() -> Self {
        Self {
            storage: IndexVec::new(),
            lookup: HashMap::new(),
            ty_context: None,
        }
    }

    /// Sets the type context used to canonicalize `Const::NullPtr`.
    pub fn set_type_context(
        &mut self,
        types: &TyInterner,
        sigs: &FnSigInterner,
        item_def_paths: &IndexVec<ItemId, DefPathHash>,
    ) {
        self.ty_context = Some(ConstTyContext {
            types: types.clone(),
            sigs: sigs.clone(),
            item_def_paths: item_def_paths.clone(),
        });
    }

    /// Interns `c` and returns the content identity's `ConstId`.
    pub fn intern(&mut self, c: Const) -> ConstId {
        let key = ConstIdentityKey::from_const(&self.storage, self.ty_context.as_ref(), &c)
            .expect("const interner input must reference earlier acyclic consts");
        if let Some(id) = self.lookup.get(&key) {
            return *id;
        }
        let id = self.storage.push(c);
        self.lookup.insert(key, id);
        id
    }

    /// Compatibility alias for `intern`.
    pub fn push(&mut self, c: Const) -> ConstId {
        self.intern(c)
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

    /// `true` when no consts are interned.
    pub fn is_empty(&self) -> bool {
        self.storage.is_empty()
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
            let key =
                ConstIdentityKey::from_const_in_storage(&storage, None, value, storage.len() + 1)
                    .map_err(serde::de::Error::custom)?;
            if let Some(first) = lookup.insert(key, id) {
                return Err(serde::de::Error::custom(format!(
                    "duplicate Const identity at ConstId {} and ConstId {}",
                    first.as_u32(),
                    id.as_u32()
                )));
            }
        }

        Ok(Self {
            storage,
            lookup,
            ty_context: None,
        })
    }
}

impl ConstIdentityKey {
    pub(crate) fn from_const(
        storage: &IndexVec<ConstId, Const>,
        ty_context: Option<&ConstTyContext>,
        value: &Const,
    ) -> Result<Self, String> {
        Self::from_const_in_storage(storage, ty_context, value, storage.len() + 1)
    }

    fn from_const_in_storage(
        storage: &IndexVec<ConstId, Const>,
        ty_context: Option<&ConstTyContext>,
        value: &Const,
        remaining_depth: usize,
    ) -> Result<Self, String> {
        let mut out = Vec::new();
        encode_identity_const(storage, ty_context, value, remaining_depth, &mut out)?;
        Ok(Self(out))
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

fn encode_identity_const(
    storage: &IndexVec<ConstId, Const>,
    ty_context: Option<&ConstTyContext>,
    value: &Const,
    remaining_depth: usize,
    out: &mut Vec<u8>,
) -> Result<(), String> {
    if remaining_depth == 0 {
        return Err("const identity recursion limit reached".into());
    }

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
                let Some(child_value) = get_const(storage, *child) else {
                    return Err(format!(
                        "ConstId reference {} is out of bounds for storage len {}",
                        child.as_u32(),
                        storage.len()
                    ));
                };
                encode_identity_const(storage, ty_context, child_value, remaining_depth - 1, out)?;
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
                let Some(child_value) = get_const(storage, *child) else {
                    return Err(format!(
                        "ConstId reference {} is out of bounds for storage len {}",
                        child.as_u32(),
                        storage.len()
                    ));
                };
                encode_identity_const(storage, ty_context, child_value, remaining_depth - 1, out)?;
            }
        }
        Const::NullPtr(ty) => {
            out.push(9);
            let Some(ty_context) = ty_context else {
                return Err("Const::NullPtr identity requires type context".into());
            };
            encode_ty_with_tables(
                &ty_context.types,
                &ty_context.sigs,
                None,
                &ty_context.item_def_paths,
                *ty,
                out,
            );
        }
        Const::Reloc(instance) => {
            out.push(10);
            out.extend_from_slice(&instance.canonical_bytes());
        }
    }
    Ok(())
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

fn get_const(storage: &IndexVec<ConstId, Const>, id: ConstId) -> Option<&Const> {
    if id.index() < storage.len() {
        Some(&storage[id])
    } else {
        None
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
    use crate::ty::{FloatKind, IntKind, Ty, UintKind};
    use crate::{FnSigInterner, IndexVec, StableCrateId, TyId, TyInterner, VariantIdx};

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

    fn set_primitive_type_context(interner: &mut ConstInterner) {
        let mut types = TyInterner::new();
        types.intern(Ty::Bool);
        types.intern(Ty::Char);
        types.intern(Ty::Int(IntKind::I32));
        let sigs = FnSigInterner::new();
        let item_def_paths = IndexVec::new();
        interner.set_type_context(&types, &sigs, &item_def_paths);
    }

    #[test]
    fn new_returns_empty_interner() {
        let interner = ConstInterner::new();
        assert_eq!(interner.len(), 0);
        assert!(interner.is_empty());
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

        let key =
            ConstIdentityKey::from_const(&interner.storage, None, &Const::Bool(true)).unwrap();
        assert_eq!(interner.find(&key), Some(true_a));
        let missing =
            ConstIdentityKey::from_const(&interner.storage, None, &Const::Char(0)).unwrap();
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
        set_primitive_type_context(&mut interner);
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
        let key = ConstIdentityKey::from_const(&back.storage, None, &Const::Bool(true)).unwrap();
        assert_eq!(back.find(&key), Some(a));
    }

    #[test]
    fn null_ptr_identity_requires_and_uses_type_context() {
        let mut missing_context = ConstInterner::new();
        let err = ConstIdentityKey::from_const(
            &missing_context.storage,
            missing_context.ty_context.as_ref(),
            &Const::NullPtr(TyId::ZERO),
        )
        .unwrap_err();
        assert!(err.contains("type context"));

        set_primitive_type_context(&mut missing_context);
        let bool_null = missing_context.intern(Const::NullPtr(TyId::ZERO));
        let char_null = missing_context.intern(Const::NullPtr(TyId::from_u32(1)));
        assert_ne!(bool_null, char_null);
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
    fn serde_rejects_self_referential_const_graph() {
        let err = serde_json::from_str::<ConstInterner>(r#"[{"Aggregate":[0]}]"#).unwrap_err();
        assert!(err.to_string().contains("recursion limit"));
    }

    #[test]
    fn const_id_option_is_four_bytes() {
        const _: () = assert!(core::mem::size_of::<Option<ConstId>>() == 4);
        assert_eq!(core::mem::size_of::<Option<ConstId>>(), 4);
    }
}
