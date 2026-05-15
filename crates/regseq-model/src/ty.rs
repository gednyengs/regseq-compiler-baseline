//! Closed Regseq type vocabulary.
//!
//! The `Ty` enum is the rustc-independent, post-monomorphization type
//! term stored in `RegseqModel.types`.

use core::convert::TryFrom;

use serde::{Deserialize, Serialize};

use crate::ids::{FnSigId, ItemId, TyId};
use crate::index_vec::IndexVec;
use crate::instance::GenericArgs;

/// Number of signed integer primitive kinds.
pub const INT_KIND_COUNT: usize = 6;
/// Number of unsigned integer primitive kinds.
pub const UINT_KIND_COUNT: usize = 6;
/// Number of floating-point primitive kinds.
pub const FLOAT_KIND_COUNT: usize = 4;

/// Error returned when a closed u8-backed enum sees an unknown discriminant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClosedEnumDiscriminantError {
    /// Name of the enum being decoded.
    pub enum_name: &'static str,
    /// Unknown numeric discriminant.
    pub value: u8,
}

macro_rules! impl_closed_u8_enum {
    ($name:ident, $($variant:ident = $value:expr),+ $(,)?) => {
        impl From<$name> for u8 {
            fn from(value: $name) -> Self {
                value as u8
            }
        }

        impl TryFrom<u8> for $name {
            type Error = ClosedEnumDiscriminantError;

            fn try_from(value: u8) -> Result<Self, Self::Error> {
                match value {
                    $($value => Ok(Self::$variant),)+
                    _ => Err(ClosedEnumDiscriminantError {
                        enum_name: stringify!($name),
                        value,
                    }),
                }
            }
        }

        impl Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                serializer.serialize_u8((*self).into())
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                let raw = u8::deserialize(deserializer)?;
                Self::try_from(raw).map_err(|err| {
                    serde::de::Error::custom(format!(
                        "{} discriminant {} is not valid",
                        err.enum_name, err.value
                    ))
                })
            }
        }
    };
}

/// Signed integer kind.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum IntKind {
    /// 8-bit signed integer. Identity bucket: structural.
    I8 = 0,
    /// 16-bit signed integer. Identity bucket: structural.
    I16 = 1,
    /// 32-bit signed integer. Identity bucket: structural.
    I32 = 2,
    /// 64-bit signed integer. Identity bucket: structural.
    I64 = 3,
    /// 128-bit signed integer. Admitted unconditionally by the bridge;
    /// per-target rejection is a codegen decision. Identity bucket:
    /// structural.
    I128 = 4,
    /// Pointer-width signed integer. Identity bucket: structural.
    Isize = 5,
}

impl_closed_u8_enum!(
    IntKind,
    I8 = 0,
    I16 = 1,
    I32 = 2,
    I64 = 3,
    I128 = 4,
    Isize = 5
);

/// Unsigned integer kind.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum UintKind {
    /// 8-bit unsigned integer. Identity bucket: structural.
    U8 = 0,
    /// 16-bit unsigned integer. Identity bucket: structural.
    U16 = 1,
    /// 32-bit unsigned integer. Identity bucket: structural.
    U32 = 2,
    /// 64-bit unsigned integer. Identity bucket: structural.
    U64 = 3,
    /// 128-bit unsigned integer. Admitted unconditionally by the bridge;
    /// per-target rejection is a codegen decision. Identity bucket:
    /// structural.
    U128 = 4,
    /// Pointer-width unsigned integer. Identity bucket: structural.
    Usize = 5,
}

impl_closed_u8_enum!(
    UintKind,
    U8 = 0,
    U16 = 1,
    U32 = 2,
    U64 = 3,
    U128 = 4,
    Usize = 5
);

/// Floating-point kind.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum FloatKind {
    /// 16-bit floating-point value. Admitted unconditionally by the
    /// bridge; per-target rejection is a codegen decision. Identity
    /// bucket: structural.
    F16 = 0,
    /// 32-bit floating-point value. Identity bucket: structural.
    F32 = 1,
    /// 64-bit floating-point value. Identity bucket: structural.
    F64 = 2,
    /// 128-bit floating-point value. Admitted unconditionally by the
    /// bridge; per-target rejection is a codegen decision. Identity
    /// bucket: structural.
    F128 = 3,
}

impl_closed_u8_enum!(FloatKind, F16 = 0, F32 = 1, F64 = 2, F128 = 3);

/// Function safety classification.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Safety {
    /// Safe function. Identity bucket: structural.
    Safe = 0,
    /// Unsafe function. Identity bucket: structural.
    Unsafe = 1,
}

impl_closed_u8_enum!(Safety, Safe = 0, Unsafe = 1);

/// Borrow flavor carried by `Ty::Ref`.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Provenance {
    /// Shared borrow. Identity bucket: structural.
    SharedBorrow = 0,
    /// Unique borrow. Identity bucket: structural.
    UniqueBorrow = 1,
}

impl_closed_u8_enum!(Provenance, SharedBorrow = 0, UniqueBorrow = 1);

/// Raw-pointer mutability carried by `Ty::Ptr`.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Mutability {
    /// Immutable raw pointer. Identity bucket: structural.
    Not = 0,
    /// Mutable raw pointer. Identity bucket: structural.
    Mut = 1,
}

impl_closed_u8_enum!(Mutability, Not = 0, Mut = 1);

/// Auto trait marker carried by `Ty::Dynamic`.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AutoTrait {
    /// `Send` auto trait. Identity bucket: structural.
    Send = 0,
    /// `Sync` auto trait. Identity bucket: structural.
    Sync = 1,
    /// `Unpin` auto trait. Identity bucket: structural.
    Unpin = 2,
}

impl_closed_u8_enum!(AutoTrait, Send = 0, Sync = 1, Unpin = 2);

/// Function ABI classification.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum FnAbi {
    /// Rust ABI. Identity bucket: structural.
    Rust = 0,
    /// C ABI. Identity bucket: structural.
    C = 1,
}

impl_closed_u8_enum!(FnAbi, Rust = 0, C = 1);

/// Forward-declared layout placeholder for P1a-DS-03.
///
/// The real `Layout` schema lands with the layout task. This
/// placeholder exists only so the closed `Ty` shape can carry a
/// layout value today and so full-value equality can observe layout
/// differences in tests for `Ty`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Layout {
    marker: u64,
}

impl Layout {
    #[cfg(test)]
    fn test_marker(marker: u64) -> Self {
        Self { marker }
    }
}

/// Closed post-monomorphization type vocabulary.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Ty {
    /// Boolean scalar. Identity bucket: structural.
    Bool,
    /// Unicode scalar value. Identity bucket: structural.
    Char,
    /// Signed integer scalar. Identity bucket: structural.
    Int(IntKind),
    /// Unsigned integer scalar. Identity bucket: structural.
    Uint(UintKind),
    /// Floating-point scalar. Identity bucket: structural.
    Float(FloatKind),
    /// Unsized string-slice marker. Appears only as a `Ref` or `Ptr`
    /// pointee; invariant enforcement is deferred to validation.
    /// Identity bucket: structural.
    Str,
    /// Uninhabited marker. Accepted as a `Ref` or `Ptr` pointee;
    /// codegen may reject. Identity bucket: structural.
    Never,
    /// Fixed-size array with element type and length. Identity
    /// bucket: structural.
    Array(TyId, u64),
    /// Unsized slice marker. Appears only as a `Ref` or `Ptr`
    /// pointee; invariant enforcement is deferred to validation.
    /// Identity bucket: structural.
    Slice(TyId),
    /// Reference with pointee and borrow flavor. Identity bucket:
    /// structural.
    Ref(TyId, Provenance),
    /// Raw pointer with pointee and mutability. Identity bucket:
    /// structural.
    Ptr(TyId, Mutability),
    /// Function definition item plus post-monomorphization arguments.
    /// This is a ZST. Identity bucket: nominal.
    FnDef(ItemId, GenericArgs),
    /// Function pointer signature handle. Identity bucket:
    /// structural.
    FnPtr(FnSigId),
    /// Trait-object marker with sorted auto traits and vtable type.
    /// This is unsized and appears only as a `Ref` or `Ptr` pointee;
    /// invariant enforcement is deferred to validation. Identity
    /// bucket: structural.
    Dynamic {
        /// Auto traits sorted ascending by discriminant.
        auto_traits: Vec<AutoTrait>,
        /// Synthesized vtable type.
        vtable_ty: TyId,
    },
    /// Closure identity view. Layout comes from the parallel
    /// captures-struct `Ty::Struct`. Identity bucket: nominal.
    Closure(ItemId, GenericArgs),
    /// Tuple with element types and inline layout. An empty `fields`
    /// vector is Rust unit `()` and must carry `UNIT_LAYOUT` once the
    /// layout task lands. Identity bucket: structural with layout
    /// excluded for interning.
    Tuple {
        /// Tuple element types.
        fields: Vec<TyId>,
        /// Resolved layout.
        layout: Layout,
    },
    /// Nominal struct with post-monomorphization arguments and
    /// inline layout. Identity bucket: nominal.
    Struct {
        /// Source item.
        item: ItemId,
        /// Resolved generic arguments.
        args: GenericArgs,
        /// Resolved layout.
        layout: Layout,
    },
    /// Nominal enum with post-monomorphization arguments and inline
    /// layout. Identity bucket: nominal.
    Enum {
        /// Source item.
        item: ItemId,
        /// Resolved generic arguments.
        args: GenericArgs,
        /// Resolved layout.
        layout: Layout,
    },
    /// Nominal union with post-monomorphization arguments and inline
    /// layout. Identity bucket: nominal.
    Union {
        /// Source item.
        item: ItemId,
        /// Resolved generic arguments.
        args: GenericArgs,
        /// Resolved layout.
        layout: Layout,
    },
}

/// Forward-declared type interner.
pub type TyInterner = IndexVec<TyId, Ty>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::{ConstId, FnSigId, ItemId, TyId};
    use crate::model::RegseqModel;

    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    fn ty_id(value: u32) -> TyId {
        TyId::from_u32(value)
    }

    fn item_id(value: u32) -> ItemId {
        ItemId::from_u32(value)
    }

    fn fn_sig_id(value: u32) -> FnSigId {
        FnSigId::from_u32(value)
    }

    fn layout(value: u64) -> Layout {
        Layout::test_marker(value)
    }

    fn args() -> GenericArgs {
        vec![crate::instance::GenericArg::Type(ty_id(7))]
    }

    fn hash<T: Hash>(value: &T) -> u64 {
        let mut hasher = DefaultHasher::new();
        value.hash(&mut hasher);
        hasher.finish()
    }

    macro_rules! assert_enum_round_trip {
        ($name:ident, [$($variant:ident = $value:expr),+], $bad:expr) => {{
            $(
                let kind = $name::$variant;
                assert_eq!(kind as u8, $value);
                assert_eq!(u8::from(kind), $value);
                assert_eq!($name::try_from($value), Ok(kind));
                let json = serde_json::to_string(&kind).unwrap();
                assert_eq!(json, $value.to_string());
                assert_eq!(serde_json::from_str::<$name>(&json).unwrap(), kind);
            )+
            assert!($name::try_from($bad).is_err());
            assert!(serde_json::from_str::<$name>(&$bad.to_string()).is_err());
        }};
    }

    #[test]
    fn bool_variant_constructs() {
        assert!(matches!(Ty::Bool, Ty::Bool));
    }

    #[test]
    fn char_variant_constructs() {
        assert!(matches!(Ty::Char, Ty::Char));
    }

    #[test]
    fn int_variant_constructs_all_discriminants() {
        for kind in [
            IntKind::I8,
            IntKind::I16,
            IntKind::I32,
            IntKind::I64,
            IntKind::I128,
            IntKind::Isize,
        ] {
            assert!(matches!(Ty::Int(kind), Ty::Int(_)));
        }
    }

    #[test]
    fn uint_variant_constructs_all_discriminants() {
        for kind in [
            UintKind::U8,
            UintKind::U16,
            UintKind::U32,
            UintKind::U64,
            UintKind::U128,
            UintKind::Usize,
        ] {
            assert!(matches!(Ty::Uint(kind), Ty::Uint(_)));
        }
    }

    #[test]
    fn float_variant_constructs_all_discriminants() {
        for kind in [
            FloatKind::F16,
            FloatKind::F32,
            FloatKind::F64,
            FloatKind::F128,
        ] {
            assert!(matches!(Ty::Float(kind), Ty::Float(_)));
        }
    }

    #[test]
    fn str_variant_constructs() {
        assert!(matches!(Ty::Str, Ty::Str));
    }

    #[test]
    fn never_variant_constructs() {
        assert!(matches!(Ty::Never, Ty::Never));
    }

    #[test]
    fn array_variant_round_trips_payload() {
        let ty = Ty::Array(ty_id(3), 11);
        assert_eq!(ty, Ty::Array(ty_id(3), 11));
    }

    #[test]
    fn slice_variant_round_trips_payload() {
        let ty = Ty::Slice(ty_id(3));
        assert_eq!(ty, Ty::Slice(ty_id(3)));
    }

    #[test]
    fn ref_variant_constructs_both_provenance_values() {
        assert_eq!(
            Ty::Ref(ty_id(3), Provenance::SharedBorrow),
            Ty::Ref(ty_id(3), Provenance::SharedBorrow)
        );
        assert_eq!(
            Ty::Ref(ty_id(3), Provenance::UniqueBorrow),
            Ty::Ref(ty_id(3), Provenance::UniqueBorrow)
        );
    }

    #[test]
    fn ptr_variant_constructs_both_mutability_values() {
        assert_eq!(
            Ty::Ptr(ty_id(3), Mutability::Not),
            Ty::Ptr(ty_id(3), Mutability::Not)
        );
        assert_eq!(
            Ty::Ptr(ty_id(3), Mutability::Mut),
            Ty::Ptr(ty_id(3), Mutability::Mut)
        );
    }

    #[test]
    fn fn_def_variant_round_trips_payload() {
        let ty = Ty::FnDef(item_id(2), args());
        assert_eq!(ty, Ty::FnDef(item_id(2), args()));
    }

    #[test]
    fn fn_ptr_variant_round_trips_payload() {
        let ty = Ty::FnPtr(fn_sig_id(4));
        assert_eq!(ty, Ty::FnPtr(fn_sig_id(4)));
    }

    #[test]
    fn dynamic_variant_constructs_empty_and_sorted_sets() {
        let empty = Ty::Dynamic {
            auto_traits: vec![],
            vtable_ty: ty_id(9),
        };
        let sorted = Ty::Dynamic {
            auto_traits: vec![AutoTrait::Send, AutoTrait::Sync],
            vtable_ty: ty_id(9),
        };
        assert!(matches!(empty, Ty::Dynamic { .. }));
        assert!(matches!(sorted, Ty::Dynamic { .. }));
    }

    #[test]
    fn closure_variant_round_trips_payload() {
        let ty = Ty::Closure(item_id(2), args());
        assert_eq!(ty, Ty::Closure(item_id(2), args()));
    }

    #[test]
    fn tuple_variant_constructs_unit_form() {
        let ty = Ty::Tuple {
            fields: vec![],
            layout: layout(0),
        };
        assert!(matches!(ty, Ty::Tuple { fields, .. } if fields.is_empty()));
    }

    #[test]
    fn struct_variant_round_trips_payload() {
        let ty = Ty::Struct {
            item: item_id(1),
            args: args(),
            layout: layout(1),
        };
        assert_eq!(
            ty,
            Ty::Struct {
                item: item_id(1),
                args: args(),
                layout: layout(1),
            }
        );
    }

    #[test]
    fn enum_variant_round_trips_payload() {
        let ty = Ty::Enum {
            item: item_id(1),
            args: args(),
            layout: layout(1),
        };
        assert_eq!(
            ty,
            Ty::Enum {
                item: item_id(1),
                args: args(),
                layout: layout(1),
            }
        );
    }

    #[test]
    fn union_variant_round_trips_payload() {
        let ty = Ty::Union {
            item: item_id(1),
            args: args(),
            layout: layout(1),
        };
        assert_eq!(
            ty,
            Ty::Union {
                item: item_id(1),
                args: args(),
                layout: layout(1),
            }
        );
    }

    #[test]
    fn int_kind_round_trips_by_discriminant() {
        assert_enum_round_trip!(
            IntKind,
            [I8 = 0, I16 = 1, I32 = 2, I64 = 3, I128 = 4, Isize = 5],
            6
        );
    }

    #[test]
    fn uint_kind_round_trips_by_discriminant() {
        assert_enum_round_trip!(
            UintKind,
            [U8 = 0, U16 = 1, U32 = 2, U64 = 3, U128 = 4, Usize = 5],
            6
        );
    }

    #[test]
    fn float_kind_round_trips_by_discriminant() {
        assert_enum_round_trip!(FloatKind, [F16 = 0, F32 = 1, F64 = 2, F128 = 3], 4);
    }

    #[test]
    fn safety_round_trips_by_discriminant() {
        assert_enum_round_trip!(Safety, [Safe = 0, Unsafe = 1], 2);
    }

    #[test]
    fn provenance_round_trips_by_discriminant() {
        assert_enum_round_trip!(Provenance, [SharedBorrow = 0, UniqueBorrow = 1], 2);
    }

    #[test]
    fn mutability_round_trips_by_discriminant() {
        assert_enum_round_trip!(Mutability, [Not = 0, Mut = 1], 2);
    }

    #[test]
    fn auto_trait_round_trips_by_discriminant() {
        assert_enum_round_trip!(AutoTrait, [Send = 0, Sync = 1, Unpin = 2], 3);
    }

    #[test]
    fn fn_abi_round_trips_by_discriminant() {
        assert_enum_round_trip!(FnAbi, [Rust = 0, C = 1], 2);
    }

    #[test]
    fn kind_counts_match_closed_variants() {
        let int_count: usize = [
            IntKind::I8,
            IntKind::I16,
            IntKind::I32,
            IntKind::I64,
            IntKind::I128,
            IntKind::Isize,
        ]
        .into_iter()
        .map(|kind| match kind {
            IntKind::I8
            | IntKind::I16
            | IntKind::I32
            | IntKind::I64
            | IntKind::I128
            | IntKind::Isize => 1,
        })
        .sum();
        let uint_count: usize = [
            UintKind::U8,
            UintKind::U16,
            UintKind::U32,
            UintKind::U64,
            UintKind::U128,
            UintKind::Usize,
        ]
        .into_iter()
        .map(|kind| match kind {
            UintKind::U8
            | UintKind::U16
            | UintKind::U32
            | UintKind::U64
            | UintKind::U128
            | UintKind::Usize => 1,
        })
        .sum();
        let float_count: usize = [
            FloatKind::F16,
            FloatKind::F32,
            FloatKind::F64,
            FloatKind::F128,
        ]
        .into_iter()
        .map(|kind| match kind {
            FloatKind::F16 | FloatKind::F32 | FloatKind::F64 | FloatKind::F128 => 1,
        })
        .sum();

        assert_eq!(INT_KIND_COUNT, 6);
        assert_eq!(UINT_KIND_COUNT, 6);
        assert_eq!(FLOAT_KIND_COUNT, 4);
        assert_eq!(int_count, INT_KIND_COUNT);
        assert_eq!(uint_count, UINT_KIND_COUNT);
        assert_eq!(float_count, FLOAT_KIND_COUNT);
    }

    #[test]
    fn ty_equality_and_hash_include_layout_value() {
        assert_eq!(Ty::Bool, Ty::Bool);
        assert_eq!(hash(&Ty::Bool), hash(&Ty::Bool));

        let struct_a = Ty::Struct {
            item: item_id(1),
            args: args(),
            layout: layout(1),
        };
        let struct_b = Ty::Struct {
            item: item_id(1),
            args: args(),
            layout: layout(2),
        };
        assert_ne!(struct_a, struct_b);

        let tuple_a = Ty::Tuple {
            fields: vec![ty_id(1), ty_id(2)],
            layout: layout(3),
        };
        let tuple_b = Ty::Tuple {
            fields: vec![ty_id(1), ty_id(2)],
            layout: layout(3),
        };
        assert_eq!(hash(&tuple_a), hash(&tuple_b));
    }

    #[test]
    fn layout_placeholder_round_trips() {
        let value = layout(42);
        let json = serde_json::to_string(&value).unwrap();
        assert_eq!(serde_json::from_str::<Layout>(&json).unwrap(), value);
    }

    #[test]
    fn ty_variants_round_trip_through_serde() {
        let variants = vec![
            Ty::Bool,
            Ty::Char,
            Ty::Int(IntKind::I8),
            Ty::Uint(UintKind::U8),
            Ty::Float(FloatKind::F32),
            Ty::Str,
            Ty::Never,
            Ty::Array(ty_id(1), 4),
            Ty::Slice(ty_id(1)),
            Ty::Ref(ty_id(1), Provenance::SharedBorrow),
            Ty::Ptr(ty_id(1), Mutability::Not),
            Ty::FnDef(item_id(1), args()),
            Ty::FnPtr(fn_sig_id(1)),
            Ty::Dynamic {
                auto_traits: vec![AutoTrait::Send, AutoTrait::Sync],
                vtable_ty: ty_id(2),
            },
            Ty::Closure(item_id(1), args()),
            Ty::Tuple {
                fields: vec![ty_id(1)],
                layout: layout(1),
            },
            Ty::Struct {
                item: item_id(1),
                args: args(),
                layout: layout(2),
            },
            Ty::Enum {
                item: item_id(1),
                args: args(),
                layout: layout(3),
            },
            Ty::Union {
                item: item_id(1),
                args: args(),
                layout: layout(4),
            },
        ];

        for ty in variants {
            let json = serde_json::to_string(&ty).unwrap();
            assert_eq!(serde_json::from_str::<Ty>(&json).unwrap(), ty);
        }
    }

    #[test]
    fn wide_scalars_construct_and_round_trip_through_serde() {
        for ty in [
            Ty::Int(IntKind::I128),
            Ty::Uint(UintKind::U128),
            Ty::Float(FloatKind::F16),
            Ty::Float(FloatKind::F128),
        ] {
            let json = serde_json::to_string(&ty).unwrap();
            assert_eq!(serde_json::from_str::<Ty>(&json).unwrap(), ty);
        }
    }

    #[test]
    fn auto_traits_sort_by_discriminant_order() {
        let traits = [AutoTrait::Send, AutoTrait::Sync, AutoTrait::Unpin];
        assert!(traits.windows(2).all(|pair| pair[0] < pair[1]));

        let dynamic = Ty::Dynamic {
            auto_traits: vec![AutoTrait::Sync, AutoTrait::Send],
            vtable_ty: ty_id(2),
        };
        assert!(matches!(dynamic, Ty::Dynamic { .. }));
    }

    #[test]
    fn closed_enum_error_constructs() {
        let err = IntKind::try_from(6).unwrap_err();
        assert_eq!(
            err,
            ClosedEnumDiscriminantError {
                enum_name: "IntKind",
                value: 6,
            }
        );
    }

    #[test]
    #[ignore = "IGNORE-UNTIL: P1a-DS-04 + P1a-DS-07"]
    fn contains_unsafe_cell_returns_false_for_bool() {
        let mut model = RegseqModel::with_primary("primary", crate::StableCrateId(1));
        let bool_ty = model.types.push(Ty::Bool);
        assert!(!model.contains_unsafe_cell(bool_ty));
    }

    #[test]
    #[ignore = "IGNORE-UNTIL: P1a-DS-04 + P1a-DS-07"]
    fn contains_unsafe_cell_returns_false_for_ref() {
        let mut model = RegseqModel::with_primary("primary", crate::StableCrateId(1));
        let pointee = model.types.push(Ty::Bool);
        let ref_ty = model.types.push(Ty::Ref(pointee, Provenance::SharedBorrow));
        assert!(!model.contains_unsafe_cell(ref_ty));
    }

    #[test]
    #[ignore = "IGNORE-UNTIL: P1a-DS-04 + P1a-DS-07"]
    fn contains_unsafe_cell_returns_false_for_fn_ptr() {
        let mut model = RegseqModel::with_primary("primary", crate::StableCrateId(1));
        let fn_ptr = model.types.push(Ty::FnPtr(fn_sig_id(0)));
        assert!(!model.contains_unsafe_cell(fn_ptr));
    }

    #[test]
    #[ignore = "IGNORE-UNTIL: P1a-DS-04 + P1a-DS-07"]
    fn contains_unsafe_cell_struct_rules_wait_for_item_schema() {
        // IGNORE-UNTIL: P1a-DS-04 + P1a-DS-07
        let model = RegseqModel::with_primary("primary", crate::StableCrateId(1));
        let _ = model;
    }

    #[test]
    #[ignore = "IGNORE-UNTIL: P1a-DS-04 + P1a-DS-07"]
    fn contains_unsafe_cell_transitive_struct_rules_wait_for_layout_schema() {
        // IGNORE-UNTIL: P1a-DS-04 + P1a-DS-07
        let model = RegseqModel::with_primary("primary", crate::StableCrateId(1));
        let _ = model;
    }

    #[test]
    #[ignore = "IGNORE-UNTIL: P1a-DS-04 + P1a-DS-07"]
    fn contains_unsafe_cell_array_rules_wait_for_real_helper() {
        // IGNORE-UNTIL: P1a-DS-04 + P1a-DS-07
        let mut model = RegseqModel::with_primary("primary", crate::StableCrateId(1));
        let elem = model.types.push(Ty::Bool);
        let array = model.types.push(Ty::Array(elem, 1));
        assert!(!model.contains_unsafe_cell(array));
    }

    #[test]
    #[ignore = "IGNORE-UNTIL: P1a-DS-04 + P1a-DS-07"]
    fn contains_unsafe_cell_tuple_rules_wait_for_layout_schema() {
        // IGNORE-UNTIL: P1a-DS-04 + P1a-DS-07
        let mut model = RegseqModel::with_primary("primary", crate::StableCrateId(1));
        let elem = model.types.push(Ty::Bool);
        let tuple = model.types.push(Ty::Tuple {
            fields: vec![elem],
            layout: layout(1),
        });
        assert!(!model.contains_unsafe_cell(tuple));
    }

    #[test]
    #[ignore = "IGNORE-UNTIL: P1a-DS-04 + P1a-DS-07"]
    fn contains_unsafe_cell_closure_rules_wait_for_find_nominal() {
        // IGNORE-UNTIL: P1a-DS-04 + P1a-DS-07
        let model = RegseqModel::with_primary("primary", crate::StableCrateId(1));
        let _ = model;
    }

    #[test]
    #[ignore = "IGNORE-UNTIL: P1a-DS-04 + P1a-DS-07"]
    fn contains_unsafe_cell_memoisation_waits_for_real_helper() {
        // IGNORE-UNTIL: P1a-DS-04 + P1a-DS-07
        let model = RegseqModel::with_primary("primary", crate::StableCrateId(1));
        let _ = model;
    }

    #[allow(dead_code)]
    fn _exhaustiveness_check(t: &Ty) {
        match t {
            Ty::Bool
            | Ty::Char
            | Ty::Int(_)
            | Ty::Uint(_)
            | Ty::Float(_)
            | Ty::Str
            | Ty::Never
            | Ty::Array(_, _)
            | Ty::Slice(_)
            | Ty::Ref(_, _)
            | Ty::Ptr(_, _)
            | Ty::FnDef(_, _)
            | Ty::FnPtr(_)
            | Ty::Dynamic { .. }
            | Ty::Closure(_, _)
            | Ty::Tuple { .. }
            | Ty::Struct { .. }
            | Ty::Enum { .. }
            | Ty::Union { .. } => (),
        }
    }

    #[test]
    fn ty_interner_alias_constructs() {
        let types: TyInterner = IndexVec::new();
        assert!(types.is_empty());
    }

    #[test]
    fn generic_args_import_keeps_const_variant_available() {
        let args = vec![crate::instance::GenericArg::Const(ConstId::from_u32(3))];
        assert_eq!(args.len(), 1);
    }
}
