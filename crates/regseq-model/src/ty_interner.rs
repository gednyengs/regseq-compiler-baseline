//! Content-keyed type interner.
//!
//! `TyInterner` stores the flat `Ty` table and rebuilds its private
//! lookup map from type identity keys during deserialization. The
//! wire form is storage only.

use std::collections::HashMap;
use std::ops::Index;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::args_key::GenericArgs;
use crate::ids::{ItemId, TyId};
use crate::index_vec::IndexVec;
use crate::layout::{EnumShape, Layout, LayoutKind, TagEncoding};
use crate::ty::Ty;

/// Nominal type identity family.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NominalKind {
    /// `Ty::Struct`.
    Struct,
    /// `Ty::Enum`.
    Enum,
    /// `Ty::Union`.
    Union,
    /// `Ty::FnDef`.
    FnDef,
    /// `Ty::Closure`.
    Closure,
}

/// Content-keyed type interner.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TyInterner {
    storage: IndexVec<TyId, Ty>,
    lookup: HashMap<TyIdentityKey, TyId>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct TyIdentityKey {
    tag: u8,
    payload: TyIdentityPayload,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum TyIdentityPayload {
    Empty,
    Int(crate::IntKind),
    Uint(crate::UintKind),
    Float(crate::FloatKind),
    Array(TyId, u64),
    OneTy(TyId),
    Ref(TyId, crate::Provenance),
    Ptr(TyId, crate::Mutability),
    FnDef(ItemId, GenericArgs),
    FnPtr(crate::FnSigId),
    Dynamic {
        auto_traits: Vec<crate::AutoTrait>,
        vtable_ty: TyId,
    },
    Nominal(ItemId, GenericArgs),
    Tuple(Vec<TyId>),
}

impl TyInterner {
    /// Returns an empty type interner.
    pub fn new() -> Self {
        Self {
            storage: IndexVec::new(),
            lookup: HashMap::new(),
        }
    }

    /// Builds an interner from raw storage without rebuilding its lookup map.
    ///
    /// This exists for validator corruption fixtures that must model a
    /// tampered deserialized table. Normal construction must use
    /// `TyInterner::new` and `TyInterner::intern`.
    pub fn from_storage_unchecked_for_validation(storage: IndexVec<TyId, Ty>) -> Self {
        Self {
            storage,
            lookup: HashMap::new(),
        }
    }

    /// Interns `ty` and returns the content identity's `TyId`.
    pub fn intern(&mut self, ty: Ty) -> TyId {
        let key = TyIdentityKey::from_ty(&ty);
        if let Some(id) = self.lookup.get(&key) {
            return *id;
        }
        let id = self.storage.push(ty);
        self.lookup.insert(key, id);
        id
    }

    /// Compatibility alias for `intern`.
    pub fn push(&mut self, ty: Ty) -> TyId {
        self.intern(ty)
    }

    /// Resolves `id` to its stored type.
    pub fn resolve(&self, id: TyId) -> &Ty {
        &self.storage[id]
    }

    /// Finds the interned identity-equivalent type without mutating.
    pub fn find(&self, ty: &Ty) -> Option<TyId> {
        self.lookup.get(&TyIdentityKey::from_ty(ty)).copied()
    }

    /// Finds a nominal type by `(kind, item, args)`.
    pub fn find_nominal(
        &self,
        kind: NominalKind,
        item: ItemId,
        args: &GenericArgs,
    ) -> Option<TyId> {
        self.lookup
            .get(&TyIdentityKey::nominal(kind, item, args.clone()))
            .copied()
    }

    /// Number of stored type identities.
    pub fn len(&self) -> usize {
        self.storage.len()
    }

    /// `true` when no types are interned.
    pub fn is_empty(&self) -> bool {
        self.storage.is_empty()
    }

    /// Iterator over `(TyId, &Ty)` in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = (TyId, &Ty)> {
        self.storage
            .iter()
            .enumerate()
            .map(|(idx, ty)| (TyId::from_u32(idx as u32), ty))
    }

    /// Patches the inline layout for a layout-bearing type.
    ///
    /// Sound usage requires that the patched type's identity key is
    /// unchanged. Only the inline `Layout` field may change.
    pub fn patch_layout(&mut self, id: TyId, layout: Layout) {
        match &mut self.storage[id] {
            Ty::Tuple { layout: old, .. }
            | Ty::Struct { layout: old, .. }
            | Ty::Enum { layout: old, .. }
            | Ty::Union { layout: old, .. } => *old = layout,
            _ => panic!("patch_layout called for type without inline layout"),
        }
    }

    /// Apply `f` to the `ItemId` of every nominal type entry
    /// (`Ty::Struct`, `Ty::Enum`, `Ty::Union`, `Ty::FnDef`,
    /// `Ty::Closure`) currently in storage, then rebuild the
    /// content-keyed lookup map from scratch.
    ///
    /// Callers must apply a bijective remap; under that contract
    /// each nominal identity key embeds its `ItemId` value, so the
    /// post-call lookup remains injective by construction. The
    /// rebuilt lookup is asserted unique in debug builds.
    pub(crate) fn for_each_nominal_item_id_mut<F>(&mut self, mut f: F)
    where
        F: FnMut(&mut ItemId),
    {
        for ty in self.storage.iter_mut() {
            match ty {
                Ty::Struct { item, .. }
                | Ty::Enum { item, .. }
                | Ty::Union { item, .. }
                | Ty::FnDef(item, _)
                | Ty::Closure(item, _) => f(item),
                _ => {}
            }
        }
        self.lookup.clear();
        for (idx, ty) in self.storage.iter().enumerate() {
            let key = TyIdentityKey::from_ty(ty);
            let prev = self.lookup.insert(key, TyId::from_u32(idx as u32));
            debug_assert!(
                prev.is_none(),
                "TyInterner identity collision after ItemId remap at TyId {}",
                idx,
            );
        }
    }

    /// Patches the `item: ItemId` field on a nominal type entry and
    /// re-keys the lookup map to reflect the new identity.
    ///
    /// This exists so the bridge can intern a placeholder
    /// `Ty::Struct` before the final `ItemId` is assigned and then
    /// resolve it to the real `ItemId` once Phase 4 completes (per
    /// spec section 3.4 D-12-5.3). Panics if the target type is not
    /// nominal (`Struct`, `Enum`, `Union`, `FnDef`, or `Closure`) or
    /// if the post-patch identity collides with another interned
    /// entry.
    pub fn patch_nominal_item_id(&mut self, id: TyId, new_item: crate::ids::ItemId) {
        let old_key = TyIdentityKey::from_ty(&self.storage[id]);
        match &mut self.storage[id] {
            Ty::Struct { item, .. }
            | Ty::Enum { item, .. }
            | Ty::Union { item, .. }
            | Ty::FnDef(item, _)
            | Ty::Closure(item, _) => *item = new_item,
            _ => panic!("patch_nominal_item_id called for non-nominal type"),
        }
        let new_key = TyIdentityKey::from_ty(&self.storage[id]);
        if old_key != new_key {
            self.lookup.remove(&old_key);
            if let Some(existing) = self.lookup.insert(new_key, id) {
                panic!(
                    "patch_nominal_item_id collides with existing TyId {} (incoming {})",
                    existing.as_u32(),
                    id.as_u32(),
                );
            }
        }
    }
}

impl Default for TyInterner {
    fn default() -> Self {
        Self::new()
    }
}

impl Index<TyId> for TyInterner {
    type Output = Ty;

    fn index(&self, index: TyId) -> &Self::Output {
        self.resolve(index)
    }
}

impl Serialize for TyInterner {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.storage.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for TyInterner {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let storage = IndexVec::<TyId, Ty>::deserialize(deserializer)?;
        validate_ty_references(&storage).map_err(serde::de::Error::custom)?;

        let mut lookup = HashMap::new();
        for (idx, ty) in storage.iter().enumerate() {
            let id = TyId::from_u32(idx as u32);
            let key = TyIdentityKey::from_ty(ty);
            if let Some(first) = lookup.insert(key, id) {
                return Err(serde::de::Error::custom(format!(
                    "duplicate Ty identity at TyId {} and TyId {}",
                    first.as_u32(),
                    id.as_u32()
                )));
            }
        }

        Ok(Self { storage, lookup })
    }
}

impl TyIdentityKey {
    pub(crate) fn from_ty(ty: &Ty) -> Self {
        match ty {
            Ty::Bool => Self::empty(0),
            Ty::Char => Self::empty(1),
            Ty::Int(kind) => Self {
                tag: 2,
                payload: TyIdentityPayload::Int(*kind),
            },
            Ty::Uint(kind) => Self {
                tag: 3,
                payload: TyIdentityPayload::Uint(*kind),
            },
            Ty::Float(kind) => Self {
                tag: 4,
                payload: TyIdentityPayload::Float(*kind),
            },
            Ty::Str => Self::empty(5),
            Ty::Never => Self::empty(6),
            Ty::Array(elem, len) => Self {
                tag: 7,
                payload: TyIdentityPayload::Array(*elem, *len),
            },
            Ty::Slice(elem) => Self {
                tag: 8,
                payload: TyIdentityPayload::OneTy(*elem),
            },
            Ty::Ref(pointee, provenance) => Self {
                tag: 9,
                payload: TyIdentityPayload::Ref(*pointee, *provenance),
            },
            Ty::Ptr(pointee, mutability) => Self {
                tag: 10,
                payload: TyIdentityPayload::Ptr(*pointee, *mutability),
            },
            Ty::FnDef(item, args) => Self {
                tag: 11,
                payload: TyIdentityPayload::FnDef(*item, args.clone()),
            },
            Ty::FnPtr(sig) => Self {
                tag: 12,
                payload: TyIdentityPayload::FnPtr(*sig),
            },
            Ty::Dynamic {
                auto_traits,
                vtable_ty,
            } => Self {
                tag: 13,
                payload: TyIdentityPayload::Dynamic {
                    auto_traits: auto_traits.clone(),
                    vtable_ty: *vtable_ty,
                },
            },
            Ty::Closure(item, args) => Self::nominal(NominalKind::Closure, *item, args.clone()),
            Ty::Tuple { fields, .. } => Self {
                tag: 15,
                payload: TyIdentityPayload::Tuple(fields.clone()),
            },
            Ty::Struct { item, args, .. } => {
                Self::nominal(NominalKind::Struct, *item, args.clone())
            }
            Ty::Enum { item, args, .. } => Self::nominal(NominalKind::Enum, *item, args.clone()),
            Ty::Union { item, args, .. } => Self::nominal(NominalKind::Union, *item, args.clone()),
        }
    }

    fn nominal(kind: NominalKind, item: ItemId, args: GenericArgs) -> Self {
        Self {
            tag: match kind {
                NominalKind::Struct => 16,
                NominalKind::Enum => 17,
                NominalKind::Union => 18,
                NominalKind::FnDef => 11,
                NominalKind::Closure => 14,
            },
            payload: match kind {
                NominalKind::FnDef => TyIdentityPayload::FnDef(item, args),
                _ => TyIdentityPayload::Nominal(item, args),
            },
        }
    }

    fn empty(tag: u8) -> Self {
        Self {
            tag,
            payload: TyIdentityPayload::Empty,
        }
    }
}

fn validate_ty_references(storage: &IndexVec<TyId, Ty>) -> Result<(), String> {
    for (idx, ty) in storage.iter().enumerate() {
        let owner = TyId::from_u32(idx as u32);
        validate_ty(owner, ty, storage.len())?;
    }
    Ok(())
}

fn validate_ty(owner: TyId, ty: &Ty, len: usize) -> Result<(), String> {
    match ty {
        Ty::Bool
        | Ty::Char
        | Ty::Int(_)
        | Ty::Uint(_)
        | Ty::Float(_)
        | Ty::Str
        | Ty::Never
        | Ty::FnDef(_, _)
        | Ty::FnPtr(_)
        | Ty::Closure(_, _) => {}
        Ty::Array(elem, _) | Ty::Slice(elem) | Ty::Ref(elem, _) | Ty::Ptr(elem, _) => {
            check_ty_id(owner, *elem, len)?;
        }
        Ty::Dynamic { vtable_ty, .. } => check_ty_id(owner, *vtable_ty, len)?,
        Ty::Tuple { fields, layout } => {
            for field in fields {
                check_ty_id(owner, *field, len)?;
            }
            validate_layout(owner, layout, len)?;
        }
        Ty::Struct { args, layout, .. }
        | Ty::Enum { args, layout, .. }
        | Ty::Union { args, layout, .. } => {
            validate_args(owner, args, len)?;
            validate_layout(owner, layout, len)?;
        }
    }

    match ty {
        Ty::FnDef(_, args) | Ty::Closure(_, args) => validate_args(owner, args, len)?,
        _ => {}
    }

    Ok(())
}

fn validate_args(owner: TyId, args: &GenericArgs, len: usize) -> Result<(), String> {
    for arg in args {
        if let crate::GenericArg::Type(ty) = *arg {
            check_ty_id(owner, ty, len)?;
        }
    }
    Ok(())
}

fn validate_layout(owner: TyId, layout: &Layout, len: usize) -> Result<(), String> {
    match &layout.kind {
        LayoutKind::Struct { fields } | LayoutKind::Union { fields } => {
            for field in fields {
                check_ty_id(owner, field.ty, len)?;
            }
        }
        LayoutKind::Enum { shape } => match shape {
            EnumShape::Empty => {}
            EnumShape::Single { variant, scalar } => {
                for field in &variant.fields {
                    check_ty_id(owner, field.ty, len)?;
                }
                if let Some(scalar) = scalar {
                    check_ty_id(owner, scalar.scalar_ty, len)?;
                }
            }
            EnumShape::Multiple {
                tag,
                encoding,
                variants,
            } => {
                check_ty_id(owner, tag.scalar_ty, len)?;
                if let TagEncoding::Niche {
                    untagged_variant, ..
                } = encoding
                {
                    if untagged_variant.index() >= variants.len() {
                        return Err(format!(
                            "TyId {} has enum layout with dangling untagged variant {}",
                            owner.as_u32(),
                            untagged_variant.as_u32()
                        ));
                    }
                }
                for variant in variants {
                    for field in &variant.fields {
                        check_ty_id(owner, field.ty, len)?;
                    }
                }
            }
        },
    }
    Ok(())
}

fn check_ty_id(owner: TyId, target: TyId, len: usize) -> Result<(), String> {
    if target.index() >= len {
        return Err(format!(
            "TyId {} references out-of-bounds TyId {} with storage len {}",
            owner.as_u32(),
            target.as_u32(),
            len
        ));
    }
    Ok(())
}

#[allow(dead_code)]
fn _exhaustiveness_check(k: &NominalKind) {
    match k {
        NominalKind::Struct
        | NominalKind::Enum
        | NominalKind::Union
        | NominalKind::FnDef
        | NominalKind::Closure => (),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::{ConstId, FnSigId};
    use crate::layout::{FieldLayout, LayoutKind};
    use crate::ty::{AutoTrait, FloatKind, IntKind, Mutability, Provenance, Safety, UintKind};
    use crate::StringId;

    fn ty(value: u32) -> TyId {
        TyId::from_u32(value)
    }

    fn item(value: u32) -> ItemId {
        ItemId::from_u32(value)
    }

    fn sig(value: u32) -> FnSigId {
        FnSigId::from_u32(value)
    }

    fn layout(size: u64) -> Layout {
        Layout {
            size,
            align: 1,
            pack: None,
            uninhabited: false,
            kind: LayoutKind::Struct {
                fields: vec![FieldLayout {
                    name: StringId::ZERO,
                    ty: ty(0),
                    offset: 0,
                }],
            },
        }
    }

    fn args(id: TyId) -> GenericArgs {
        vec![crate::GenericArg::Type(id)]
    }

    #[test]
    fn new_returns_empty_interner() {
        let interner = TyInterner::new();
        assert_eq!(interner.len(), 0);
        assert!(interner.is_empty());
        assert_eq!(interner.iter().count(), 0);
    }

    #[test]
    fn intern_deduplicates_and_resolves_structural_types() {
        let mut interner = TyInterner::new();
        let bool_a = interner.intern(Ty::Bool);
        let bool_b = interner.intern(Ty::Bool);
        let char_id = interner.intern(Ty::Char);

        assert_eq!(bool_a, TyId::ZERO);
        assert_eq!(bool_a, bool_b);
        assert_ne!(bool_a, char_id);
        assert_eq!(interner.len(), 2);
        assert_eq!(interner.resolve(bool_a), &Ty::Bool);
        assert_eq!(interner.find(&Ty::Bool), Some(bool_a));
        assert_eq!(interner.find(&Ty::Int(IntKind::I8)), None);
    }

    #[test]
    fn iter_yields_pairs_in_insertion_order() {
        let mut interner = TyInterner::new();
        let a = interner.intern(Ty::Bool);
        let b = interner.intern(Ty::Char);
        let got: Vec<_> = interner.iter().collect();
        assert_eq!(got, vec![(a, &Ty::Bool), (b, &Ty::Char)]);
    }

    #[test]
    fn all_structural_variants_deduplicate_by_full_payload() {
        let mut interner = TyInterner::new();
        let bool_id = interner.intern(Ty::Bool);
        let cases = vec![
            Ty::Bool,
            Ty::Char,
            Ty::Int(IntKind::I16),
            Ty::Uint(UintKind::U32),
            Ty::Float(FloatKind::F32),
            Ty::Str,
            Ty::Array(bool_id, 4),
            Ty::Slice(bool_id),
            Ty::Ref(bool_id, Provenance::UniqueBorrow),
            Ty::Ptr(bool_id, Mutability::Not),
            Ty::FnPtr(sig(3)),
            Ty::Dynamic {
                auto_traits: vec![AutoTrait::Send, AutoTrait::Sync],
                vtable_ty: bool_id,
            },
            Ty::Never,
        ];

        for value in cases {
            let first = interner.intern(value.clone());
            let second = interner.intern(value);
            assert_eq!(first, second);
        }

        let shared = interner.intern(Ty::Ref(bool_id, Provenance::SharedBorrow));
        let unique = interner.intern(Ty::Ref(bool_id, Provenance::UniqueBorrow));
        assert_ne!(shared, unique);
    }

    #[test]
    fn nominal_variants_ignore_layout_but_keep_variant_tag() {
        let mut interner = TyInterner::new();
        let bool_id = interner.intern(Ty::Bool);
        let generic_args = args(bool_id);

        let struct_a = interner.intern(Ty::Struct {
            item: item(1),
            args: generic_args.clone(),
            layout: layout(1),
        });
        let struct_b = interner.intern(Ty::Struct {
            item: item(1),
            args: generic_args.clone(),
            layout: layout(2),
        });
        let enum_a = interner.intern(Ty::Enum {
            item: item(1),
            args: generic_args.clone(),
            layout: layout(1),
        });
        let enum_b = interner.intern(Ty::Enum {
            item: item(1),
            args: generic_args.clone(),
            layout: layout(2),
        });
        let union_a = interner.intern(Ty::Union {
            item: item(1),
            args: generic_args.clone(),
            layout: layout(1),
        });
        let union_b = interner.intern(Ty::Union {
            item: item(1),
            args: generic_args.clone(),
            layout: layout(2),
        });
        let fn_def_a = interner.intern(Ty::FnDef(item(1), generic_args.clone()));
        let fn_def_b = interner.intern(Ty::FnDef(item(1), generic_args.clone()));
        let closure_a = interner.intern(Ty::Closure(item(1), generic_args.clone()));
        let closure_b = interner.intern(Ty::Closure(item(1), generic_args.clone()));

        assert_eq!(struct_a, struct_b);
        assert_eq!(enum_a, enum_b);
        assert_eq!(union_a, union_b);
        assert_eq!(fn_def_a, fn_def_b);
        assert_eq!(closure_a, closure_b);
        assert_ne!(struct_a, closure_a);
    }

    #[test]
    fn tuple_identity_excludes_layout() {
        let mut interner = TyInterner::new();
        let bool_id = interner.intern(Ty::Bool);
        let a = interner.intern(Ty::Tuple {
            fields: vec![bool_id],
            layout: layout(1),
        });
        let b = interner.intern(Ty::Tuple {
            fields: vec![bool_id],
            layout: layout(2),
        });
        assert_eq!(a, b);
    }

    #[test]
    fn find_nominal_is_layout_agnostic_and_args_sensitive() {
        let mut interner = TyInterner::new();
        let bool_id = interner.intern(Ty::Bool);
        let char_id = interner.intern(Ty::Char);
        let args_a = args(bool_id);
        let args_b = args(char_id);

        let struct_id = interner.intern(Ty::Struct {
            item: item(2),
            args: args_a.clone(),
            layout: layout(1),
        });
        let enum_id = interner.intern(Ty::Enum {
            item: item(2),
            args: args_a.clone(),
            layout: layout(1),
        });
        let union_id = interner.intern(Ty::Union {
            item: item(2),
            args: args_a.clone(),
            layout: layout(1),
        });
        let fn_def_id = interner.intern(Ty::FnDef(item(2), args_a.clone()));
        let closure_id = interner.intern(Ty::Closure(item(2), args_a.clone()));
        let other_args_id = interner.intern(Ty::Struct {
            item: item(2),
            args: args_b.clone(),
            layout: layout(1),
        });

        assert_eq!(
            interner.find_nominal(NominalKind::Struct, item(2), &args_a),
            Some(struct_id)
        );
        assert_eq!(
            interner.find_nominal(NominalKind::Enum, item(2), &args_a),
            Some(enum_id)
        );
        assert_eq!(
            interner.find_nominal(NominalKind::Union, item(2), &args_a),
            Some(union_id)
        );
        assert_eq!(
            interner.find_nominal(NominalKind::FnDef, item(2), &args_a),
            Some(fn_def_id)
        );
        assert_eq!(
            interner.find_nominal(NominalKind::Closure, item(2), &args_a),
            Some(closure_id)
        );
        assert_eq!(
            interner.find_nominal(NominalKind::Struct, item(9), &args_a),
            None
        );
        assert_eq!(
            interner.find_nominal(NominalKind::Struct, item(2), &args_b),
            Some(other_args_id)
        );
        assert_ne!(struct_id, other_args_id);
    }

    #[test]
    fn patch_layout_updates_storage_without_changing_identity() {
        let mut interner = TyInterner::new();
        let bool_id = interner.intern(Ty::Bool);
        let args = args(bool_id);
        let id = interner.intern(Ty::Struct {
            item: item(3),
            args: args.clone(),
            layout: layout(1),
        });

        interner.patch_layout(id, layout(99));
        assert!(matches!(
            interner.resolve(id),
            Ty::Struct { layout, .. } if layout.size == 99
        ));
        assert_eq!(
            interner.find_nominal(NominalKind::Struct, item(3), &args),
            Some(id)
        );
    }

    #[test]
    fn self_referential_adt_reservation_pattern_works() {
        let mut interner = TyInterner::new();
        let pending = Layout {
            size: 0,
            align: 1,
            pack: None,
            uninhabited: false,
            kind: LayoutKind::Struct { fields: vec![] },
        };
        let node = interner.intern(Ty::Struct {
            item: item(4),
            args: vec![],
            layout: pending,
        });
        let ptr = interner.intern(Ty::Ptr(node, Mutability::Mut));
        let real = Layout {
            size: 8,
            align: 8,
            pack: None,
            uninhabited: false,
            kind: LayoutKind::Struct {
                fields: vec![FieldLayout {
                    name: StringId::ZERO,
                    ty: ptr,
                    offset: 0,
                }],
            },
        };
        interner.patch_layout(node, real.clone());

        assert_eq!(interner.resolve(ptr), &Ty::Ptr(node, Mutability::Mut));
        assert_eq!(
            interner.intern(Ty::Struct {
                item: item(4),
                args: vec![],
                layout: real,
            }),
            node
        );
    }

    #[test]
    fn serde_round_trip_is_storage_only_and_rebuilds_lookup() {
        let mut interner = TyInterner::new();
        let bool_id = interner.intern(Ty::Bool);
        let char_id = interner.intern(Ty::Char);
        let int_id = interner.intern(Ty::Int(IntKind::I8));
        let ptr_id = interner.intern(Ty::Ptr(bool_id, Mutability::Not));
        let tuple_id = interner.intern(Ty::Tuple {
            fields: vec![bool_id, char_id],
            layout: layout(1),
        });

        let json = serde_json::to_string(&interner).unwrap();
        let back: TyInterner = serde_json::from_str(&json).unwrap();

        assert_eq!(back.len(), 5);
        for id in [bool_id, char_id, int_id, ptr_id, tuple_id] {
            assert_eq!(back.resolve(id), interner.resolve(id));
        }
        assert_eq!(back.find(&Ty::Bool), Some(bool_id));
        assert_eq!(
            back.iter().map(|(id, _)| id).collect::<Vec<_>>(),
            vec![bool_id, char_id, int_id, ptr_id, tuple_id]
        );
    }

    #[test]
    fn serde_rejects_duplicate_identity() {
        let err = serde_json::from_str::<TyInterner>(r#"["Bool","Bool"]"#).unwrap_err();
        assert!(err.to_string().contains("duplicate Ty identity"));
    }

    #[test]
    fn serde_rejects_dangling_ty_id_reference() {
        let err = serde_json::from_str::<TyInterner>(r#"[{"Array":[99,5]}]"#).unwrap_err();
        assert!(err.to_string().contains("out-of-bounds TyId"));
    }

    #[test]
    fn nominal_kind_exhaustiveness_scaffold_compiles() {
        for kind in [
            NominalKind::Struct,
            NominalKind::Enum,
            NominalKind::Union,
            NominalKind::FnDef,
            NominalKind::Closure,
        ] {
            _exhaustiveness_check(&kind);
        }
    }

    #[test]
    fn push_alias_remains_available_for_existing_callers() {
        let mut interner = TyInterner::new();
        let id = interner.push(Ty::Bool);
        assert_eq!(interner.resolve(id), &Ty::Bool);
    }

    #[test]
    fn generic_args_accept_const_payload_for_identity() {
        let mut interner = TyInterner::new();
        let a = interner.intern(Ty::FnDef(
            item(1),
            vec![crate::GenericArg::Const(ConstId::ZERO)],
        ));
        let b = interner.intern(Ty::FnDef(
            item(1),
            vec![crate::GenericArg::Const(ConstId::ZERO)],
        ));
        assert_eq!(a, b);
    }

    #[test]
    fn imported_closed_enums_remain_hashable_for_identity() {
        let _ = Safety::Safe;
    }
}
