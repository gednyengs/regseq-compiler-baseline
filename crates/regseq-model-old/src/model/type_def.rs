//! Type definition model types.
//!
//! Models struct and enum definitions discovered during extraction. Each
//! [`TypeDef`] carries a [`TypeDefClassification`] (see
//! [`crate::model::framework`]) distinguishing user-authored types from
//! Regseq abstractions and framework types, and an [`Attrs`] bag for
//! extensible semantic annotations captured from `#[rseq::attrs(..)]`.

use serde::Serialize;

use crate::identity::{Symbol, TypeDefId};
use crate::ir::{LiteralValue, RegseqType};
use crate::model::framework::TypeDefClassification;

/// A struct or enum type definition.
///
/// Represents top-level type definitions in the Regseq Model:
/// user-defined types, Regseq abstraction types, and Regseq framework
/// types (see spec.md Section 3.4).
#[derive(Clone, Eq, PartialEq, Debug, Serialize)]
pub struct TypeDef {
    /// The type's declaration identifier.
    pub id: TypeDefId,
    /// The type's identifier as an interned symbol.
    pub name: Symbol,
    /// Whether the type is a struct or enum and its contents.
    pub kind: TypeDefKind,
    /// Classification telling downstream passes how to lower this type.
    pub classification: TypeDefClassification,
    /// Extensible attribute bag for semantic annotations.
    pub attrs: Attrs,
}

/// Whether the type is a struct or enum.
#[derive(Clone, Eq, PartialEq, Debug, Serialize)]
#[serde(tag = "kind", content = "data")]
pub enum TypeDefKind {
    /// Struct type with named fields.
    Struct {
        /// Named fields of the struct.
        fields: Vec<FieldDef>,
    },
    /// Enum type with variants.
    Enum {
        /// Variants of the enum.
        variants: Vec<VariantDef>,
    },
}

/// A named field in a struct or enum variant.
#[derive(Clone, Eq, PartialEq, Debug, Serialize)]
pub struct FieldDef {
    /// Field identifier.
    pub name: Symbol,
    /// Field type.
    pub ty: RegseqType,
}

/// An enum variant.
///
/// `fields` is empty for unit variants and populated for data variants.
/// `discriminant` is `Some` only for explicit discriminant values.
#[derive(Clone, Eq, PartialEq, Debug, Serialize)]
pub struct VariantDef {
    /// Variant identifier.
    pub name: Symbol,
    /// Variant fields. Empty for unit variants.
    pub fields: Vec<FieldDef>,
    /// Explicit discriminant value, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub discriminant: Option<LiteralValue>,
}

/// Extensible attribute bag for semantic annotations.
///
/// Captures `#[rseq::attrs(..)]` annotations generically. No concrete
/// attribute keys are defined at this time -- the mechanism exists for
/// future semantic annotations (ISR metadata, callback context,
/// inlining hints, etc.). See spec.md Section 8 (Attrs).
#[derive(Clone, Eq, PartialEq, Debug, Default, Serialize)]
pub struct Attrs {
    /// The attribute entries in the bag.
    pub entries: Vec<AttrEntry>,
}

/// A single key-value attribute entry.
#[derive(Clone, Eq, PartialEq, Debug, Serialize)]
pub struct AttrEntry {
    /// Attribute key.
    pub key: String,
    /// Attribute value.
    pub value: AttrValue,
}

/// The value of an attribute entry.
#[derive(Clone, Eq, PartialEq, Debug, Serialize)]
#[serde(tag = "kind", content = "data")]
pub enum AttrValue {
    /// A string value (positional or scalar).
    String(String),
    /// A boolean value.
    Bool(bool),
    /// An integer value.
    Int(i64),
    /// No value (presence-only attribute).
    Unit,
    /// A keyword-argument map, e.g. `#[rseq::pin_state(id = "default")]`
    /// or `#[rseq::isr(bind_target = "IRQ_UART", has_index_param = true)]`.
    /// Inner entries preserve authored order.
    KeyValues(Vec<AttrEntry>),
}

// All tests in this module exercise post-HIR/MIR output (constructed model,
// JSON serialization, snapshots). Voided pending MonoRust + RegseqModel
// redesign. See docs/discussions/compiler-refactoring/test-audit.md.
#[cfg(all(test, feature = "post-monorust-tests"))]
mod tests {
    use super::*;
    use crate::identity::{DeclId, SymbolInterner};
    use crate::ir::{RegseqAbstraction, ScalarType};
    use crate::model::framework::FrameworkTypeKind;

    fn sym(interner: &mut SymbolInterner, s: &str) -> Symbol {
        interner.intern(s)
    }

    // -- Struct construction ---------------------------------------------

    #[test]
    fn constructs_user_defined_struct_with_two_fields() {
        let mut interner = SymbolInterner::new();
        let type_name = sym(&mut interner, "Point");
        let f0 = sym(&mut interner, "x");
        let f1 = sym(&mut interner, "y");

        let td = TypeDef {
            id: TypeDefId::from(DeclId::new(1)),
            name: type_name,
            kind: TypeDefKind::Struct {
                fields: vec![
                    FieldDef {
                        name: f0,
                        ty: RegseqType::Scalar(ScalarType::I32),
                    },
                    FieldDef {
                        name: f1,
                        ty: RegseqType::Scalar(ScalarType::I32),
                    },
                ],
            },
            classification: TypeDefClassification::UserDefined,
            attrs: Attrs::default(),
        };

        match &td.kind {
            TypeDefKind::Struct { fields } => assert_eq!(fields.len(), 2),
            _ => panic!("expected struct"),
        }
        assert_eq!(td.classification, TypeDefClassification::UserDefined);
        assert!(td.attrs.entries.is_empty());
    }

    // -- Enum construction -----------------------------------------------

    #[test]
    fn constructs_enum_with_unit_and_data_variants() {
        let mut interner = SymbolInterner::new();
        let type_name = sym(&mut interner, "Message");
        let v0 = sym(&mut interner, "Ping");
        let v1 = sym(&mut interner, "Data");
        let data_field = sym(&mut interner, "0");

        let td = TypeDef {
            id: TypeDefId::from(DeclId::new(2)),
            name: type_name,
            kind: TypeDefKind::Enum {
                variants: vec![
                    VariantDef {
                        name: v0,
                        fields: vec![],
                        discriminant: None,
                    },
                    VariantDef {
                        name: v1,
                        fields: vec![FieldDef {
                            name: data_field,
                            ty: RegseqType::Scalar(ScalarType::U32),
                        }],
                        discriminant: Some(LiteralValue::U32(1)),
                    },
                ],
            },
            classification: TypeDefClassification::UserDefined,
            attrs: Attrs::default(),
        };

        match &td.kind {
            TypeDefKind::Enum { variants } => {
                assert_eq!(variants.len(), 2);
                assert!(variants[0].fields.is_empty());
                assert!(variants[0].discriminant.is_none());
                assert_eq!(variants[1].fields.len(), 1);
                assert_eq!(variants[1].discriminant, Some(LiteralValue::U32(1)));
            }
            _ => panic!("expected enum"),
        }
    }

    // -- Framework classification ---------------------------------------

    #[test]
    fn constructs_type_def_with_framework_classification() {
        let mut interner = SymbolInterner::new();
        let type_name = sym(&mut interner, "Duration");

        let td = TypeDef {
            id: TypeDefId::from(DeclId::new(3)),
            name: type_name,
            kind: TypeDefKind::Struct { fields: vec![] },
            classification: TypeDefClassification::Framework(FrameworkTypeKind::Duration),
            attrs: Attrs::default(),
        };

        assert_eq!(
            td.classification,
            TypeDefClassification::Framework(FrameworkTypeKind::Duration)
        );
    }

    // -- Abstraction classification -------------------------------------

    #[test]
    fn constructs_type_def_with_abstraction_classification() {
        let mut interner = SymbolInterner::new();
        let type_name = sym(&mut interner, "Clock");

        let td = TypeDef {
            id: TypeDefId::from(DeclId::new(4)),
            name: type_name,
            kind: TypeDefKind::Struct { fields: vec![] },
            classification: TypeDefClassification::Abstraction(RegseqAbstraction::Clock),
            attrs: Attrs::default(),
        };

        assert_eq!(
            td.classification,
            TypeDefClassification::Abstraction(RegseqAbstraction::Clock)
        );
    }

    // -- Attrs ----------------------------------------------------------

    #[test]
    fn attrs_default_has_empty_entries() {
        let attrs = Attrs::default();
        assert!(attrs.entries.is_empty());
    }

    #[test]
    fn attrs_construct_with_mixed_entry_types() {
        let attrs = Attrs {
            entries: vec![
                AttrEntry {
                    key: "name".into(),
                    value: AttrValue::String("foo".into()),
                },
                AttrEntry {
                    key: "enabled".into(),
                    value: AttrValue::Bool(true),
                },
                AttrEntry {
                    key: "present".into(),
                    value: AttrValue::Unit,
                },
            ],
        };

        assert_eq!(attrs.entries.len(), 3);
        assert_eq!(attrs.entries[0].value, AttrValue::String("foo".into()));
        assert_eq!(attrs.entries[1].value, AttrValue::Bool(true));
        assert_eq!(attrs.entries[2].value, AttrValue::Unit);
    }

    // -- Equality --------------------------------------------------------

    #[test]
    fn type_def_kind_struct_and_enum_are_unequal() {
        let s = TypeDefKind::Struct { fields: vec![] };
        let e = TypeDefKind::Enum { variants: vec![] };
        assert_ne!(s, e);
    }
}
