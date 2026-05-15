//! Closed const-value vocabulary and canonical byte encoding.
//!
//! Type-level admissible consts are the scalar variants `Bool`,
//! `Char`, `Int`, `Uint`, and `Float`. All 11 variants are
//! admissible in value-level positions.

use serde::{Deserialize, Serialize};

use crate::args_key::{canonical_ty_bytes, encode_ty_with_tables};
use crate::const_interner::ConstInterner;
use crate::def_path::DefPathHash;
use crate::fn_sig_interner::FnSigInterner;
use crate::ids::{ConstId, TyId, VariantIdx};
use crate::index_vec::IndexVec;
use crate::instance_ref::InstanceRef;
use crate::model::RegseqModel;
use crate::ty::{FloatKind, IntKind, UintKind};
use crate::ty_interner::TyInterner;

/// Closed const-value vocabulary.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Const {
    /// Boolean scalar.
    Bool(bool),
    /// Unicode scalar value as a 32-bit code point.
    Char(u32),
    /// Signed integer scalar. Smaller widths are sign-extended to
    /// `u128`.
    Int(IntKind, u128),
    /// Unsigned integer scalar. Smaller widths are zero-extended.
    Uint(UintKind, u128),
    /// IEEE-754 bit pattern.
    Float(FloatKind, u128),
    /// Zero-sized value. Single canonical representative for all
    /// zero-sized types.
    ZST,
    /// Byte-level value mixing initialized bytes, uninit bytes, and
    /// pointer-bearing relocations.
    Bytes(Vec<Byte>),
    /// Structured aggregate children.
    Aggregate(Vec<ConstId>),
    /// Enum value with explicit variant index and payload children.
    EnumValue {
        /// Variant index.
        variant_idx: VariantIdx,
        /// Payload constants.
        payload: Vec<ConstId>,
    },
    /// Typed null pointer.
    NullPtr(TyId),
    /// Whole-pointer relocation.
    Reloc(InstanceRef),
}

impl Const {
    /// Returns true for consts admitted in `GenericArg::Const`.
    pub fn is_type_level_admissible(&self) -> bool {
        matches!(
            self,
            Const::Bool(_)
                | Const::Char(_)
                | Const::Int(_, _)
                | Const::Uint(_, _)
                | Const::Float(_, _)
        )
    }
}

/// One byte cell in a byte-level const value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Byte {
    /// Uninitialized byte.
    Uninit,
    /// Initialized payload byte.
    Value(u8),
    /// Pointer-bearing relocation occupying one pointer slot.
    Reloc(InstanceRef),
}

/// Returns the canonical byte stream for an interned const.
pub fn canonical_const_bytes(model: &RegseqModel, id: ConstId) -> Vec<u8> {
    let mut out = Vec::new();
    encode_const_id(model, id, &mut out);
    out
}

pub(crate) fn encode_const_id(model: &RegseqModel, id: ConstId, out: &mut Vec<u8>) {
    encode_const_with_depth(model, &model.consts[id], out, model.consts.len() + 1);
}

pub(crate) fn encode_const_id_with_tables(
    consts: &ConstInterner,
    types: &TyInterner,
    sigs: &FnSigInterner,
    item_def_paths: &IndexVec<crate::ItemId, DefPathHash>,
    id: ConstId,
    out: &mut Vec<u8>,
) {
    encode_const_with_tables(
        consts,
        types,
        sigs,
        item_def_paths,
        &consts[id],
        out,
        consts.len() + 1,
    );
}

pub(crate) fn encode_const_with_depth(
    model: &RegseqModel,
    value: &Const,
    out: &mut Vec<u8>,
    remaining_depth: usize,
) {
    assert!(
        remaining_depth != 0,
        "const canonicalization recursion limit reached"
    );

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
                encode_byte(*byte, out);
            }
        }
        Const::Aggregate(children) => {
            out.push(7);
            push_u32(out, children.len());
            for child in children {
                encode_const_with_depth(model, &model.consts[*child], out, remaining_depth - 1);
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
                encode_const_with_depth(model, &model.consts[*child], out, remaining_depth - 1);
            }
        }
        Const::NullPtr(ty) => {
            out.push(9);
            out.extend_from_slice(&canonical_ty_bytes(model, *ty));
        }
        Const::Reloc(instance) => {
            out.push(10);
            out.extend_from_slice(&instance.canonical_bytes());
        }
    }
}

fn encode_const_with_tables(
    consts: &ConstInterner,
    types: &TyInterner,
    sigs: &FnSigInterner,
    item_def_paths: &IndexVec<crate::ItemId, DefPathHash>,
    value: &Const,
    out: &mut Vec<u8>,
    remaining_depth: usize,
) {
    assert!(
        remaining_depth != 0,
        "const canonicalization recursion limit reached"
    );

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
                encode_byte(*byte, out);
            }
        }
        Const::Aggregate(children) => {
            out.push(7);
            push_u32(out, children.len());
            for child in children {
                encode_const_with_tables(
                    consts,
                    types,
                    sigs,
                    item_def_paths,
                    &consts[*child],
                    out,
                    remaining_depth - 1,
                );
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
                encode_const_with_tables(
                    consts,
                    types,
                    sigs,
                    item_def_paths,
                    &consts[*child],
                    out,
                    remaining_depth - 1,
                );
            }
        }
        Const::NullPtr(ty) => {
            out.push(9);
            encode_ty_with_tables(types, sigs, Some(consts), item_def_paths, *ty, out);
        }
        Const::Reloc(instance) => {
            out.push(10);
            out.extend_from_slice(&instance.canonical_bytes());
        }
    }
}

pub(crate) fn encode_byte(byte: Byte, out: &mut Vec<u8>) {
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

fn push_u32(out: &mut Vec<u8>, value: usize) {
    let value = u32::try_from(value).expect("canonical length exceeds u32::MAX");
    out.extend_from_slice(&value.to_le_bytes());
}

#[allow(dead_code)]
fn _const_exhaustiveness_check(value: &Const) {
    match value {
        Const::Bool(_)
        | Const::Char(_)
        | Const::Int(_, _)
        | Const::Uint(_, _)
        | Const::Float(_, _)
        | Const::ZST
        | Const::Bytes(_)
        | Const::Aggregate(_)
        | Const::EnumValue { .. }
        | Const::NullPtr(_)
        | Const::Reloc(_) => (),
    }
}

#[allow(dead_code)]
fn _byte_exhaustiveness_check(value: &Byte) {
    match value {
        Byte::Uninit | Byte::Value(_) | Byte::Reloc(_) => (),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::args_key::ArgsKey;
    use crate::def_path::DefPathHash;
    use crate::ty::Ty;
    use crate::{Mutability, StableCrateId};

    fn cid(value: u32) -> ConstId {
        ConstId::from_u32(value)
    }

    fn tid(value: u32) -> TyId {
        TyId::from_u32(value)
    }

    fn vid(value: u32) -> VariantIdx {
        VariantIdx::from_u32(value)
    }

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

    fn model() -> RegseqModel {
        RegseqModel::with_primary("primary", StableCrateId(1))
    }

    #[test]
    fn const_variants_construct() {
        assert_eq!(Const::Bool(true), Const::Bool(true));
        assert_eq!(Const::Bool(false), Const::Bool(false));
        assert_eq!(Const::Char(0x61), Const::Char(0x61));
        assert_eq!(Const::Char(0x11_0000), Const::Char(0x11_0000));
        assert_eq!(Const::Int(IntKind::I32, 42), Const::Int(IntKind::I32, 42));
        assert_eq!(
            Const::Int(IntKind::I8, u128::MAX),
            Const::Int(IntKind::I8, u128::MAX)
        );
        assert_eq!(
            Const::Uint(UintKind::U64, 0xDEAD_BEEF),
            Const::Uint(UintKind::U64, 0xDEAD_BEEF)
        );
        assert_eq!(
            Const::Float(FloatKind::F32, 0x3f80_0000),
            Const::Float(FloatKind::F32, 0x3f80_0000)
        );
        assert_eq!(Const::ZST, Const::ZST);
        assert_eq!(
            Const::Bytes(vec![Byte::Value(0x12), Byte::Uninit, Byte::Value(0x34)]),
            Const::Bytes(vec![Byte::Value(0x12), Byte::Uninit, Byte::Value(0x34)])
        );
        assert_eq!(
            Const::Aggregate(vec![cid(1), cid(2)]),
            Const::Aggregate(vec![cid(1), cid(2)])
        );
        assert_eq!(
            Const::EnumValue {
                variant_idx: vid(0),
                payload: vec![]
            },
            Const::EnumValue {
                variant_idx: vid(0),
                payload: vec![]
            }
        );
        assert_eq!(Const::NullPtr(tid(1)), Const::NullPtr(tid(1)));
        assert_eq!(Const::Reloc(instance_ref()), Const::Reloc(instance_ref()));
    }

    #[test]
    fn byte_variants_construct_and_copy() {
        let a = Byte::Uninit;
        let b = Byte::Value(255);
        let c = Byte::Reloc(instance_ref());
        let _again = c;
        assert_eq!(a, Byte::Uninit);
        assert_eq!(b, Byte::Value(255));
        assert_eq!(c, Byte::Reloc(instance_ref()));
    }

    #[test]
    fn is_type_level_admissible_matches_partition_table() {
        for value in [
            Const::Bool(true),
            Const::Char(0),
            Const::Int(IntKind::I8, 0),
            Const::Uint(UintKind::U8, 0),
            Const::Float(FloatKind::F32, 0),
        ] {
            assert!(value.is_type_level_admissible());
        }
        for value in [
            Const::ZST,
            Const::Bytes(vec![]),
            Const::Aggregate(vec![]),
            Const::EnumValue {
                variant_idx: vid(0),
                payload: vec![],
            },
            Const::NullPtr(tid(0)),
            Const::Reloc(instance_ref()),
        ] {
            assert!(!value.is_type_level_admissible());
        }
    }

    #[test]
    fn scalar_worked_examples_encode() {
        let mut model = model();
        let true_id = model.consts.intern(Const::Bool(true));
        let false_id = model.consts.intern(Const::Bool(false));
        let int_id = model
            .consts
            .intern(Const::Int(IntKind::I32, (-7_i32 as i128) as u128));
        let float_id = model
            .consts
            .intern(Const::Float(FloatKind::F32, 0x3f80_0000));
        let zst_id = model.consts.intern(Const::ZST);

        let mut int_expected = vec![2, 2];
        int_expected.extend_from_slice(&((-7_i32 as i128) as u128).to_le_bytes());
        let mut float_expected = vec![4, 1];
        float_expected.extend_from_slice(&0x3f80_0000_u128.to_le_bytes());

        assert_eq!(canonical_const_bytes(&model, true_id), vec![0, 1]);
        assert_eq!(canonical_const_bytes(&model, false_id), vec![0, 0]);
        assert_eq!(canonical_const_bytes(&model, int_id), int_expected);
        assert_eq!(canonical_const_bytes(&model, float_id), float_expected);
        assert_eq!(canonical_const_bytes(&model, zst_id), vec![5]);
    }

    #[test]
    fn byte_worked_examples_encode() {
        let mut model = model();
        let value_id = model.consts.intern(Const::Bytes(vec![Byte::Value(0xAB)]));
        let uninit_id = model.consts.intern(Const::Bytes(vec![Byte::Uninit]));

        assert_eq!(
            canonical_const_bytes(&model, value_id),
            vec![6, 1, 0, 0, 0, 1, 0xAB]
        );
        assert_eq!(
            canonical_const_bytes(&model, uninit_id),
            vec![6, 1, 0, 0, 0, 0]
        );
    }

    #[test]
    fn aggregate_and_enum_worked_examples_inline_children() {
        let mut model = model();
        let c1 = model.consts.intern(Const::Bool(true));
        let c2 = model.consts.intern(Const::Int(IntKind::I32, 42));
        let aggregate = model.consts.intern(Const::Aggregate(vec![c1, c2]));
        let enum_value = model.consts.intern(Const::EnumValue {
            variant_idx: vid(1),
            payload: vec![c1],
        });

        let mut int_42 = vec![2, 2];
        int_42.extend_from_slice(&42_u128.to_le_bytes());

        let mut aggregate_expected = vec![7, 2, 0, 0, 0, 0, 1];
        aggregate_expected.extend_from_slice(&int_42);
        assert_eq!(canonical_const_bytes(&model, aggregate), aggregate_expected);

        assert_eq!(
            canonical_const_bytes(&model, enum_value),
            vec![8, 1, 0, 0, 0, 1, 0, 0, 0, 0, 1]
        );
    }

    #[test]
    #[should_panic(expected = "const canonicalization recursion limit reached")]
    fn canonical_const_panics_on_self_referential_aggregate() {
        // Task spec line 472-478: hand-build a tampered model where
        // a `Const::Aggregate(vec![own_id])` references itself, and
        // assert the canonicalizer reaches a documented bound or
        // panics. Proper cycle rejection lives at P1a-VAL-03 step 9
        // (const-graph acyclicity); `canonical_const` is allowed to
        // assume acyclicity (spec.md:3232-3241) and bounds its
        // recursion at `model.consts.len() + 1`.
        let mut model = model();
        let id = model.consts.intern(Const::Aggregate(vec![ConstId::ZERO]));
        // `intern` of an empty interner returns ConstId(0); storage
        // now holds a self-referential `Aggregate([ConstId(0)])`.
        let _ = canonical_const_bytes(&model, id);
    }

    #[test]
    fn null_ptr_and_reloc_worked_examples_encode() {
        let mut model = model();
        let int_ty = model.types.intern(Ty::Int(IntKind::I32));
        let null = model.consts.intern(Const::NullPtr(int_ty));
        let reloc = model.consts.intern(Const::Reloc(instance_ref()));

        let null_expected = vec![9, 2, 2];
        assert_eq!(canonical_const_bytes(&model, null), null_expected);

        let mut reloc_expected = vec![10];
        reloc_expected.extend_from_slice(&instance_ref().canonical_bytes());
        assert_eq!(canonical_const_bytes(&model, reloc), reloc_expected);
    }

    #[test]
    fn byte_sub_encoding_matches_tags() {
        let mut out = Vec::new();
        encode_byte(Byte::Uninit, &mut out);
        assert_eq!(out, vec![0]);

        out.clear();
        encode_byte(Byte::Value(0xAB), &mut out);
        assert_eq!(out, vec![1, 0xAB]);

        out.clear();
        encode_byte(Byte::Reloc(instance_ref()), &mut out);
        assert_eq!(out.len(), 35);
        assert_eq!(out[0], 2);
        assert_eq!(&out[1..], instance_ref().canonical_bytes().as_slice());
    }

    #[test]
    fn closed_enum_exhaustiveness_scaffolds_compile() {
        _const_exhaustiveness_check(&Const::Bool(true));
        _byte_exhaustiveness_check(&Byte::Value(0));
    }

    #[test]
    fn serde_round_trips_const_and_byte_values() {
        let value = Const::Bytes(vec![Byte::Reloc(instance_ref())]);
        let json = serde_json::to_string(&value).unwrap();
        assert_eq!(serde_json::from_str::<Const>(&json).unwrap(), value);
    }

    #[test]
    fn null_ptr_uses_canonical_ty_not_local_ty_id() {
        let mut a = model();
        let bool_a = a.types.intern(Ty::Bool);
        let null_a = a.consts.intern(Const::NullPtr(bool_a));

        let mut b = model();
        let _char_b = b.types.intern(Ty::Char);
        let bool_b = b.types.intern(Ty::Bool);
        let null_b = b.consts.intern(Const::NullPtr(bool_b));

        assert_ne!(bool_a, bool_b);
        assert_eq!(
            canonical_const_bytes(&a, null_a),
            canonical_const_bytes(&b, null_b)
        );
    }

    #[test]
    fn pointer_relocation_shape_can_appear_in_aggregate() {
        let mut model = model();
        let reloc = model.consts.intern(Const::Reloc(instance_ref()));
        let bytes = model.consts.intern(Const::Uint(UintKind::Usize, 8));
        let aggregate = model.consts.intern(Const::Aggregate(vec![reloc, bytes]));
        assert!(canonical_const_bytes(&model, aggregate).starts_with(&[7, 2, 0, 0, 0]));
    }

    #[test]
    fn value_level_variants_can_include_pointer_types() {
        let mut model = model();
        let bool_ty = model.types.intern(Ty::Bool);
        let ptr_ty = model.types.intern(Ty::Ptr(bool_ty, Mutability::Not));
        let null = model.consts.intern(Const::NullPtr(ptr_ty));
        assert_eq!(canonical_const_bytes(&model, null), vec![9, 10, 0, 0]);
    }

    #[test]
    fn zero_sized_default_is_not_invented() {
        assert_eq!(Const::ZST, Const::ZST);
    }
}
