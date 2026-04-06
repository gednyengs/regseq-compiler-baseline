//! Item-level type references for the Layer 2 item hierarchy.
//!
//! `TypeRef` is the item-hierarchy-scoped type reference, NOT the full
//! `SemanticType` universe. `SemanticType` will be introduced with the
//! semantic body IR in a later task.

use serde::{Deserialize, Serialize};

use super::identity::ItemId;

/// Item-level type reference.
///
/// Used for struct field types, constant types, and impl self-types within
/// the item hierarchy schema.
///
/// This is deliberately simpler than the full `SemanticType` universe
/// (which includes carriers like `Option<T>`, domain types like
/// `RegisterBlock`, etc.). `SemanticType` will be introduced alongside
/// `SemanticBody` in the body lowering task.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum TypeRef {
    /// Reference to a named item (struct, enum) in the item store.
    Item { id: ItemId },

    /// Built-in primitive type.
    Primitive { ty: PrimitiveType },

    /// Unresolved or externally-named type carried from extraction.
    ///
    /// This is a fallback type reference for types not yet resolved to
    /// `ItemId` or `PrimitiveType`. It is NOT authoritative for identity
    /// and must never be used for cross-link semantics or canonical
    /// resolution. Cross-layer identity always uses `CanonicalPath`.
    Named { name: String },
}

/// Primitive types representable without an item reference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PrimitiveType {
    Bool,
    U8,
    U16,
    U32,
    U64,
    I8,
    I16,
    I32,
    I64,
    Usize,
}
