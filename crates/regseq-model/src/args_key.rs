//! Generic-argument identity and canonical byte encoding.
//!
//! `ArgsKey` is the stable, cross-build content fingerprint for a
//! fully-resolved `GenericArgs` vector. The byte stream resolves
//! nested `TyId` values to type content and resolves nominal
//! `ItemId` values through `RegseqModel.item_def_paths`. `StringId`
//! canonicalization is length-prefixed UTF-8 content when a future
//! encoded value carries a `StringId`; the type variants in this task
//! do not carry one directly.

use serde::{Deserialize, Serialize};

use crate::ids::{ConstId, TyId};
use crate::model::RegseqModel;
use crate::ty::Ty;

/// One generic argument substituted into a monomorphized item.
///
/// Lifetimes are erased at the bridge and never appear in
/// `RegseqModel`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GenericArg {
    /// Type argument.
    Type(TyId),
    /// Const argument.
    Const(ConstId),
}

/// Vector of generic substitutions in rustc generic-argument order.
pub type GenericArgs = Vec<GenericArg>;

/// Stable 128-bit content key for a `GenericArgs` byte stream.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ArgsKey(pub u128);

impl ArgsKey {
    /// Canonical `ArgsKey` for an empty `GenericArgs` value.
    ///
    /// Computed: SHA-256(\x00\x00\x00\x00)[..16] big-endian.
    pub const EMPTY: ArgsKey = ArgsKey(0xdf3f_6198_04a9_2fdb_4057_192d_c43d_d748);
}

/// Returns the SHA-256-truncated key for `args`.
pub fn args_key_for_generic_args(model: &RegseqModel, args: &GenericArgs) -> ArgsKey {
    args_key_from_canonical_bytes(&canonical_args_bytes(model, args))
}

/// Returns the canonical byte stream for `args`.
pub fn canonical_args_bytes(model: &RegseqModel, args: &GenericArgs) -> Vec<u8> {
    let mut out = Vec::new();
    encode_args(model, args, &mut out);
    out
}

/// Returns the canonical byte stream for `ty`.
pub fn canonical_ty_bytes(model: &RegseqModel, ty: TyId) -> Vec<u8> {
    let mut out = Vec::new();
    encode_ty(model, ty, &mut out);
    out
}

fn args_key_from_canonical_bytes(bytes: &[u8]) -> ArgsKey {
    use sha2::{Digest, Sha256};

    let digest = Sha256::digest(bytes);
    let mut head = [0u8; 16];
    head.copy_from_slice(&digest[..16]);
    ArgsKey(u128::from_be_bytes(head))
}

fn encode_args(model: &RegseqModel, args: &GenericArgs, out: &mut Vec<u8>) {
    push_u32(out, args.len());
    for arg in args {
        match *arg {
            GenericArg::Type(ty) => {
                out.push(0);
                encode_ty(model, ty, out);
            }
            GenericArg::Const(_) => {
                out.push(1);
                // FORWARD: P1a-DS-05: canonical Const bytes.
                todo!("FORWARD: P1a-DS-05: canonical Const bytes")
            }
        }
    }
}

fn encode_ty(model: &RegseqModel, ty: TyId, out: &mut Vec<u8>) {
    match &model.types[ty] {
        Ty::Bool => out.push(0),
        Ty::Char => out.push(1),
        Ty::Int(kind) => {
            out.push(2);
            out.push((*kind).into());
        }
        Ty::Uint(kind) => {
            out.push(3);
            out.push((*kind).into());
        }
        Ty::Float(kind) => {
            out.push(4);
            out.push((*kind).into());
        }
        Ty::Str => out.push(5),
        Ty::Never => out.push(6),
        Ty::Array(elem, len) => {
            out.push(7);
            encode_ty(model, *elem, out);
            out.extend_from_slice(&len.to_le_bytes());
        }
        Ty::Slice(elem) => {
            out.push(8);
            encode_ty(model, *elem, out);
        }
        Ty::Ref(pointee, provenance) => {
            out.push(9);
            encode_ty(model, *pointee, out);
            out.push((*provenance).into());
        }
        Ty::Ptr(pointee, mutability) => {
            out.push(10);
            encode_ty(model, *pointee, out);
            out.push((*mutability).into());
        }
        Ty::FnDef(item, args) => {
            out.push(11);
            encode_item_def_path_hash(model, *item, out);
            encode_args(model, args, out);
        }
        Ty::FnPtr(_) => {
            out.push(12);
            // FORWARD: P1a-DS-06: canonical FnSig bytes.
            todo!("FORWARD: P1a-DS-06: canonical FnSig bytes")
        }
        Ty::Dynamic {
            auto_traits,
            vtable_ty,
        } => {
            out.push(13);
            let mut sorted: Vec<u8> = auto_traits.iter().copied().map(u8::from).collect();
            sorted.sort_unstable();
            push_u32(out, sorted.len());
            out.extend_from_slice(&sorted);
            encode_ty(model, *vtable_ty, out);
        }
        Ty::Closure(item, args) => {
            out.push(14);
            encode_item_def_path_hash(model, *item, out);
            encode_args(model, args, out);
        }
        Ty::Tuple { fields, .. } => {
            out.push(15);
            push_u32(out, fields.len());
            for field in fields {
                encode_ty(model, *field, out);
            }
        }
        Ty::Struct { item, args, .. } => {
            out.push(16);
            encode_item_def_path_hash(model, *item, out);
            encode_args(model, args, out);
        }
        Ty::Enum { item, args, .. } => {
            out.push(17);
            encode_item_def_path_hash(model, *item, out);
            encode_args(model, args, out);
        }
        Ty::Union { item, args, .. } => {
            out.push(18);
            encode_item_def_path_hash(model, *item, out);
            encode_args(model, args, out);
        }
    }
}

fn encode_item_def_path_hash(model: &RegseqModel, item: crate::ItemId, out: &mut Vec<u8>) {
    let hash = model.item_def_paths[item];
    out.extend_from_slice(&hash.stable_crate_id.0.to_le_bytes());
    out.extend_from_slice(&hash.local_hash.to_le_bytes());
}

fn push_u32(out: &mut Vec<u8>, value: usize) {
    let value = u32::try_from(value).expect("canonical length exceeds u32::MAX");
    out.extend_from_slice(&value.to_le_bytes());
}

#[allow(dead_code)]
fn _generic_arg_exhaustiveness(a: &GenericArg) {
    match a {
        GenericArg::Type(_) | GenericArg::Const(_) => (),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::def_path::DefPathHash;
    use crate::ids::{ConstId, FnSigId, ItemId};
    use crate::layout::{FieldLayout, Layout, LayoutKind};
    use crate::ty::{AutoTrait, FloatKind, IntKind, Mutability, Provenance};
    use crate::{StableCrateId, StringId};

    use std::collections::HashSet;

    fn ty(value: u32) -> TyId {
        TyId::from_u32(value)
    }

    fn item(value: u32) -> ItemId {
        ItemId::from_u32(value)
    }

    fn field_layout(field_ty: TyId) -> Layout {
        Layout {
            size: 8,
            align: 8,
            pack: None,
            uninhabited: false,
            kind: LayoutKind::Struct {
                fields: vec![FieldLayout {
                    name: StringId::ZERO,
                    ty: field_ty,
                    offset: 0,
                }],
            },
        }
    }

    fn model_with_def_path() -> RegseqModel {
        let mut model = RegseqModel::with_primary("primary", StableCrateId(1));
        model.item_def_paths.push(DefPathHash {
            stable_crate_id: StableCrateId(0x11_22_33_44_55_66_77_88),
            local_hash: 0x99_aa_bb_cc_dd_ee_ff_00,
        });
        model
    }

    fn hash_bytes() -> [u8; 16] {
        let mut out = Vec::new();
        out.extend_from_slice(&0x11_22_33_44_55_66_77_88_u64.to_le_bytes());
        out.extend_from_slice(&0x99_aa_bb_cc_dd_ee_ff_00_u64.to_le_bytes());
        out.try_into().unwrap()
    }

    #[test]
    fn args_key_constructs_and_orders() {
        let a = ArgsKey(0);
        let b = ArgsKey(1);
        assert!(a < b);
    }

    #[test]
    fn args_key_empty_matches_runtime_sha256() {
        let bytes = [0, 0, 0, 0];
        assert_eq!(args_key_from_canonical_bytes(&bytes), ArgsKey::EMPTY);
    }

    #[test]
    fn args_key_round_trips_through_serde() {
        let key = ArgsKey(0x1234);
        let json = serde_json::to_string(&key).unwrap();
        assert_eq!(serde_json::from_str::<ArgsKey>(&json).unwrap(), key);
    }

    #[test]
    fn args_key_copy_eq_hash_ord_traits_are_available() {
        let key = ArgsKey(9);
        let copied = key;
        let mut set = HashSet::new();
        set.insert(copied);
        assert!(set.contains(&key));
    }

    #[test]
    fn empty_generic_args_encode_as_zero_length() {
        let model = RegseqModel::with_primary("primary", StableCrateId(1));
        assert_eq!(canonical_args_bytes(&model, &vec![]), vec![0, 0, 0, 0]);
        assert_eq!(args_key_for_generic_args(&model, &vec![]), ArgsKey::EMPTY);
    }

    #[test]
    fn generic_type_arg_encodes_disc_then_canonical_ty() {
        let mut model = RegseqModel::with_primary("primary", StableCrateId(1));
        let bool_ty = model.types.intern(Ty::Bool);
        let args = vec![GenericArg::Type(bool_ty)];
        assert_eq!(canonical_args_bytes(&model, &args), vec![1, 0, 0, 0, 0, 0]);
    }

    #[test]
    #[ignore = "IGNORE-UNTIL: P1a-DS-05"]
    fn generic_const_arg_encodes_const_payload() {
        // IGNORE-UNTIL: P1a-DS-05
        let model = RegseqModel::with_primary("primary", StableCrateId(1));
        let args = vec![GenericArg::Const(ConstId::from_u32(0))];
        let _ = canonical_args_bytes(&model, &args);
    }

    #[test]
    fn primitive_ty_canonical_bytes_match_tags() {
        let mut model = RegseqModel::with_primary("primary", StableCrateId(1));
        let cases = vec![
            (Ty::Bool, vec![0]),
            (Ty::Char, vec![1]),
            (Ty::Int(IntKind::I32), vec![2, 2]),
            (Ty::Uint(crate::UintKind::U64), vec![3, 3]),
            (Ty::Float(FloatKind::F64), vec![4, 2]),
            (Ty::Str, vec![5]),
            (Ty::Never, vec![6]),
        ];
        for (ty_value, expected) in cases {
            let id = model.types.intern(ty_value);
            assert_eq!(canonical_ty_bytes(&model, id), expected);
        }
    }

    #[test]
    fn recursive_ty_canonical_bytes_resolve_nested_ty_ids() {
        let mut model = RegseqModel::with_primary("primary", StableCrateId(1));
        let bool_ty = model.types.intern(Ty::Bool);
        let array = model.types.intern(Ty::Array(bool_ty, 7));
        let slice = model.types.intern(Ty::Slice(bool_ty));
        let ref_ty = model
            .types
            .intern(Ty::Ref(bool_ty, Provenance::SharedBorrow));
        let ptr_ty = model.types.intern(Ty::Ptr(bool_ty, Mutability::Mut));

        let mut array_expected = vec![7, 0];
        array_expected.extend_from_slice(&7_u64.to_le_bytes());
        assert_eq!(canonical_ty_bytes(&model, array), array_expected);
        assert_eq!(canonical_ty_bytes(&model, slice), vec![8, 0]);
        assert_eq!(canonical_ty_bytes(&model, ref_ty), vec![9, 0, 0]);
        assert_eq!(canonical_ty_bytes(&model, ptr_ty), vec![10, 0, 1]);
    }

    #[test]
    fn nominal_ty_canonical_bytes_use_def_path_hash_and_args() {
        let mut model = model_with_def_path();
        let bool_ty = model.types.intern(Ty::Bool);
        let args = vec![GenericArg::Type(bool_ty)];
        let fn_def = model.types.intern(Ty::FnDef(item(0), args.clone()));
        let closure = model.types.intern(Ty::Closure(item(0), args.clone()));
        let struct_ty = model.types.intern(Ty::Struct {
            item: item(0),
            args: args.clone(),
            layout: field_layout(bool_ty),
        });
        let enum_ty = model.types.intern(Ty::Enum {
            item: item(0),
            args: args.clone(),
            layout: field_layout(bool_ty),
        });
        let union_ty = model.types.intern(Ty::Union {
            item: item(0),
            args,
            layout: field_layout(bool_ty),
        });

        for (id, tag) in [
            (fn_def, 11),
            (closure, 14),
            (struct_ty, 16),
            (enum_ty, 17),
            (union_ty, 18),
        ] {
            let mut expected = vec![tag];
            expected.extend_from_slice(&hash_bytes());
            expected.extend_from_slice(&[1, 0, 0, 0, 0, 0]);
            assert_eq!(canonical_ty_bytes(&model, id), expected);
        }
    }

    #[test]
    fn dynamic_and_tuple_canonical_bytes_match_spec_order() {
        let mut model = RegseqModel::with_primary("primary", StableCrateId(1));
        let bool_ty = model.types.intern(Ty::Bool);
        let char_ty = model.types.intern(Ty::Char);
        let dynamic = model.types.intern(Ty::Dynamic {
            auto_traits: vec![AutoTrait::Sync, AutoTrait::Send],
            vtable_ty: bool_ty,
        });
        let tuple = model.types.intern(Ty::Tuple {
            fields: vec![bool_ty, char_ty],
            layout: field_layout(bool_ty),
        });

        assert_eq!(
            canonical_ty_bytes(&model, dynamic),
            vec![13, 2, 0, 0, 0, 0, 1, 0]
        );
        assert_eq!(
            canonical_ty_bytes(&model, tuple),
            vec![15, 2, 0, 0, 0, 0, 1]
        );
    }

    #[test]
    #[ignore = "IGNORE-UNTIL: P1a-DS-06"]
    fn fn_ptr_canonical_bytes_wait_for_fn_sig_interner() {
        // IGNORE-UNTIL: P1a-DS-06
        let mut model = RegseqModel::with_primary("primary", StableCrateId(1));
        let fn_ptr = model.types.intern(Ty::FnPtr(FnSigId::from_u32(0)));
        let _ = canonical_ty_bytes(&model, fn_ptr);
    }

    #[test]
    fn args_key_is_stable_across_local_ty_id_assignment_order() {
        let mut a = RegseqModel::with_primary("primary", StableCrateId(1));
        let bool_a = a.types.intern(Ty::Bool);
        let _char_a = a.types.intern(Ty::Char);

        let mut b = RegseqModel::with_primary("primary", StableCrateId(1));
        let _char_b = b.types.intern(Ty::Char);
        let _int_b = b.types.intern(Ty::Int(IntKind::I8));
        let bool_b = b.types.intern(Ty::Bool);

        assert_ne!(bool_a, bool_b);
        assert_eq!(
            args_key_for_generic_args(&a, &vec![GenericArg::Type(bool_a)]),
            args_key_for_generic_args(&b, &vec![GenericArg::Type(bool_b)])
        );
    }

    #[test]
    fn generic_arg_exhaustiveness_scaffold_compiles() {
        _generic_arg_exhaustiveness(&GenericArg::Type(ty(0)));
        _generic_arg_exhaustiveness(&GenericArg::Const(ConstId::from_u32(0)));
    }
}
