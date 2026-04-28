//! Type system for the Regseq Model IR.
//!
//! Defines [`RegseqType`] (10 variants), [`ScalarType`] (10 variants),
//! and [`Mutability`] (2 variants). Recursive variants use `Box` to keep
//! `RegseqType` a finite-size enum.

use serde::{Deserialize, Serialize};

use crate::identity::TypeDefId;

/// The Regseq Model type system. Represents types that flow through
/// expressions, function signatures, and local declarations.
///
/// All types are concrete (monomorphized). No lifetimes, trait bounds,
/// or generics.
#[derive(Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data")]
pub enum RegseqType {
    Scalar(ScalarType),
    Bool,
    Unit,
    Never,
    Option(Box<RegseqType>),
    Result(Box<RegseqType>, Box<RegseqType>),
    Array(Box<RegseqType>, usize),
    Slice(Box<RegseqType>),
    Ref(Box<RegseqType>, Mutability),
    Defined(TypeDefId),
}

impl std::fmt::Debug for RegseqType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RegseqType::Scalar(s) => write!(f, "Scalar({:?})", s),
            RegseqType::Bool => write!(f, "Bool"),
            RegseqType::Unit => write!(f, "Unit"),
            RegseqType::Never => write!(f, "Never"),
            RegseqType::Option(inner) => write!(f, "Option({:?})", inner),
            RegseqType::Result(ok, err) => write!(f, "Result({:?}, {:?})", ok, err),
            RegseqType::Array(elem, len) => write!(f, "Array({:?}, {})", elem, len),
            RegseqType::Slice(elem) => write!(f, "Slice({:?})", elem),
            RegseqType::Ref(inner, m) => write!(f, "Ref({:?}, {:?})", inner, m),
            RegseqType::Defined(id) => write!(f, "Defined({:?})", id),
        }
    }
}

/// Scalar types. Fixed-width (`U8`..`U64`, `I8`..`I64`) and
/// target-dependent (`Usize`, `Isize`).
#[derive(Copy, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data")]
pub enum ScalarType {
    U8,
    U16,
    U32,
    U64,
    I8,
    I16,
    I32,
    I64,
    Usize,
    Isize,
}

impl std::fmt::Debug for ScalarType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            ScalarType::U8 => "U8",
            ScalarType::U16 => "U16",
            ScalarType::U32 => "U32",
            ScalarType::U64 => "U64",
            ScalarType::I8 => "I8",
            ScalarType::I16 => "I16",
            ScalarType::I32 => "I32",
            ScalarType::I64 => "I64",
            ScalarType::Usize => "Usize",
            ScalarType::Isize => "Isize",
        };
        write!(f, "{}", name)
    }
}

/// Reference mutability.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data")]
pub enum Mutability {
    Shared,
    Mutable,
}

impl std::fmt::Debug for Mutability {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Mutability::Shared => write!(f, "Shared"),
            Mutability::Mutable => write!(f, "Mutable"),
        }
    }
}

// All tests in this module exercise post-HIR/MIR output (constructed model,
// JSON serialization, snapshots). Voided pending MonoRust + RegseqModel
// redesign. See docs/discussions/compiler-refactoring/test-audit.md.
#[cfg(all(test, feature = "post-monorust-tests"))]
mod tests {
    use super::*;
    use crate::identity::{DeclId, TypeDefId};

    // -- Construction: each of the 10 RegseqType variants --

    #[test]
    fn construct_scalar() {
        let _ = RegseqType::Scalar(ScalarType::U32);
    }

    #[test]
    fn construct_bool() {
        let _ = RegseqType::Bool;
    }

    #[test]
    fn construct_unit() {
        let _ = RegseqType::Unit;
    }

    #[test]
    fn construct_never() {
        let _ = RegseqType::Never;
    }

    #[test]
    fn construct_option() {
        let _ = RegseqType::Option(Box::new(RegseqType::Scalar(ScalarType::U32)));
    }

    #[test]
    fn construct_result() {
        let _ = RegseqType::Result(
            Box::new(RegseqType::Unit),
            Box::new(RegseqType::Defined(TypeDefId::from(DeclId::new(0)))),
        );
    }

    #[test]
    fn construct_array() {
        let _ = RegseqType::Array(Box::new(RegseqType::Scalar(ScalarType::U8)), 4);
    }

    #[test]
    fn construct_slice() {
        let _ = RegseqType::Slice(Box::new(RegseqType::Scalar(ScalarType::U8)));
    }

    #[test]
    fn construct_ref() {
        let _ = RegseqType::Ref(Box::new(RegseqType::Bool), Mutability::Mutable);
    }

    #[test]
    fn construct_defined() {
        let _ = RegseqType::Defined(TypeDefId::from(DeclId::new(42)));
    }

    // -- Nested types --

    #[test]
    fn nested_option_scalar() {
        let t = RegseqType::Option(Box::new(RegseqType::Scalar(ScalarType::U32)));
        assert_eq!(
            t,
            RegseqType::Option(Box::new(RegseqType::Scalar(ScalarType::U32)))
        );
    }

    #[test]
    fn nested_result_unit_defined() {
        let id = TypeDefId::from(DeclId::new(5));
        let t = RegseqType::Result(
            Box::new(RegseqType::Unit),
            Box::new(RegseqType::Defined(id)),
        );
        assert_eq!(
            t,
            RegseqType::Result(
                Box::new(RegseqType::Unit),
                Box::new(RegseqType::Defined(id))
            )
        );
    }

    #[test]
    fn nested_ref_array_scalar_mutable() {
        let t = RegseqType::Ref(
            Box::new(RegseqType::Array(
                Box::new(RegseqType::Scalar(ScalarType::U8)),
                4,
            )),
            Mutability::Mutable,
        );
        assert_eq!(
            t,
            RegseqType::Ref(
                Box::new(RegseqType::Array(
                    Box::new(RegseqType::Scalar(ScalarType::U8)),
                    4,
                )),
                Mutability::Mutable,
            )
        );
    }

    // -- Equality --

    #[test]
    fn scalar_u32_equals_scalar_u32() {
        assert_eq!(
            RegseqType::Scalar(ScalarType::U32),
            RegseqType::Scalar(ScalarType::U32)
        );
    }

    #[test]
    fn scalar_u32_not_equals_scalar_u16() {
        assert_ne!(
            RegseqType::Scalar(ScalarType::U32),
            RegseqType::Scalar(ScalarType::U16)
        );
    }

    // -- Debug format --

    #[test]
    fn debug_scalar() {
        assert_eq!(
            format!("{:?}", RegseqType::Scalar(ScalarType::U32)),
            "Scalar(U32)"
        );
    }

    #[test]
    fn debug_ref_bool_mutable() {
        let t = RegseqType::Ref(Box::new(RegseqType::Bool), Mutability::Mutable);
        assert_eq!(format!("{:?}", t), "Ref(Bool, Mutable)");
    }

    // -- Serde round-trip: each RegseqType variant --

    #[test]
    fn serde_round_trip_scalar() {
        let t = RegseqType::Scalar(ScalarType::U32);
        let json = serde_json::to_string(&t).unwrap();
        assert_eq!(t, serde_json::from_str::<RegseqType>(&json).unwrap());
    }

    #[test]
    fn serde_round_trip_bool() {
        let t = RegseqType::Bool;
        let json = serde_json::to_string(&t).unwrap();
        assert_eq!(t, serde_json::from_str::<RegseqType>(&json).unwrap());
    }

    #[test]
    fn serde_round_trip_unit() {
        let t = RegseqType::Unit;
        let json = serde_json::to_string(&t).unwrap();
        assert_eq!(t, serde_json::from_str::<RegseqType>(&json).unwrap());
    }

    #[test]
    fn serde_round_trip_never() {
        let t = RegseqType::Never;
        let json = serde_json::to_string(&t).unwrap();
        assert_eq!(t, serde_json::from_str::<RegseqType>(&json).unwrap());
    }

    #[test]
    fn serde_round_trip_option() {
        let t = RegseqType::Option(Box::new(RegseqType::Scalar(ScalarType::U32)));
        let json = serde_json::to_string(&t).unwrap();
        assert_eq!(t, serde_json::from_str::<RegseqType>(&json).unwrap());
    }

    #[test]
    fn serde_round_trip_result() {
        let id = TypeDefId::from(DeclId::new(5));
        let t = RegseqType::Result(
            Box::new(RegseqType::Unit),
            Box::new(RegseqType::Defined(id)),
        );
        let json = serde_json::to_string(&t).unwrap();
        assert_eq!(t, serde_json::from_str::<RegseqType>(&json).unwrap());
    }

    #[test]
    fn serde_round_trip_array() {
        let t = RegseqType::Array(Box::new(RegseqType::Scalar(ScalarType::U8)), 4);
        let json = serde_json::to_string(&t).unwrap();
        assert_eq!(t, serde_json::from_str::<RegseqType>(&json).unwrap());
    }

    #[test]
    fn serde_round_trip_slice() {
        let t = RegseqType::Slice(Box::new(RegseqType::Scalar(ScalarType::U8)));
        let json = serde_json::to_string(&t).unwrap();
        assert_eq!(t, serde_json::from_str::<RegseqType>(&json).unwrap());
    }

    #[test]
    fn serde_round_trip_ref() {
        let t = RegseqType::Ref(Box::new(RegseqType::Bool), Mutability::Shared);
        let json = serde_json::to_string(&t).unwrap();
        assert_eq!(t, serde_json::from_str::<RegseqType>(&json).unwrap());
    }

    #[test]
    fn serde_round_trip_defined() {
        let t = RegseqType::Defined(TypeDefId::from(DeclId::new(42)));
        let json = serde_json::to_string(&t).unwrap();
        assert_eq!(t, serde_json::from_str::<RegseqType>(&json).unwrap());
    }

    #[test]
    fn serde_round_trip_nested() {
        let t = RegseqType::Ref(
            Box::new(RegseqType::Array(
                Box::new(RegseqType::Scalar(ScalarType::U8)),
                4,
            )),
            Mutability::Mutable,
        );
        let json = serde_json::to_string(&t).unwrap();
        assert_eq!(t, serde_json::from_str::<RegseqType>(&json).unwrap());
    }

    // -- Serde round-trip: ScalarType independently --

    #[test]
    fn serde_round_trip_all_scalar_types() {
        let variants = [
            ScalarType::U8,
            ScalarType::U16,
            ScalarType::U32,
            ScalarType::U64,
            ScalarType::I8,
            ScalarType::I16,
            ScalarType::I32,
            ScalarType::I64,
            ScalarType::Usize,
            ScalarType::Isize,
        ];
        for v in &variants {
            let json = serde_json::to_string(v).unwrap();
            assert_eq!(*v, serde_json::from_str::<ScalarType>(&json).unwrap());
        }
    }

    // -- Serde round-trip: Mutability independently --

    #[test]
    fn serde_round_trip_mutability() {
        for m in &[Mutability::Shared, Mutability::Mutable] {
            let json = serde_json::to_string(m).unwrap();
            assert_eq!(*m, serde_json::from_str::<Mutability>(&json).unwrap());
        }
    }

    // -- Serde format: adjacently-tagged --

    #[test]
    fn serde_adjacently_tagged_unit_variant() {
        let json = serde_json::to_string(&ScalarType::U32).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["kind"], "U32");
        assert!(v.get("data").is_none());
    }

    #[test]
    fn serde_adjacently_tagged_data_variant() {
        let t = RegseqType::Scalar(ScalarType::U32);
        let json = serde_json::to_string(&t).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["kind"], "Scalar");
        assert!(v.get("data").is_some());
    }

    #[test]
    fn serde_adjacently_tagged_bool_no_data() {
        let t = RegseqType::Bool;
        let json = serde_json::to_string(&t).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["kind"], "Bool");
        assert!(v.get("data").is_none());
    }
}
