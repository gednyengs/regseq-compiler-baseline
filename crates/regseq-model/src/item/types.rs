//! Core item types for the Layer 2 item hierarchy.
//!
//! `Item` is the top-level discriminated union of all first-class items.
//! `StructDef` and `EnumDef` define type-level items with field/variant
//! identity.

use serde::{Deserialize, Serialize};

use crate::declaration::Provenance;

use super::attributes::{FieldAttribute, ItemAttribute};
use super::constant::ConstantDef;
use super::function::FunctionDef;
use super::identity::{CanonicalPath, ItemId};
use super::impl_def::ImplDef;
use super::type_ref::TypeRef;

/// A first-class Regseq model item.
///
/// `TypeAlias` is intentionally deferred per the plan's slicing guidance
/// (no current evidence of need).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "item_kind")]
pub enum Item {
    /// Struct definition (driver struct, register blocks, config types).
    Struct(StructDef),
    /// Enum definition (interrupt classes, mode enums, error types).
    Enum(EnumDef),
    /// Function definition (header-only at this task boundary).
    Function(FunctionDef),
    /// Impl block (groups methods with their owning type and optional trait).
    Impl(ImplDef),
    /// Named constant (associated constants, masks, configuration values).
    Constant(ConstantDef),
}

impl Item {
    /// Return the embedded `ItemId` from whichever variant this item is.
    pub fn id(&self) -> ItemId {
        match self {
            Item::Struct(s) => s.id,
            Item::Enum(e) => e.id,
            Item::Function(f) => f.id,
            Item::Impl(i) => i.id,
            Item::Constant(c) => c.id,
        }
    }

    /// Set the embedded `ItemId` on whichever variant this item is.
    ///
    /// Called by `ItemStore::push()` to synchronize the item's internal id
    /// with its store-assigned position.
    pub fn set_id(&mut self, id: ItemId) {
        match self {
            Item::Struct(s) => s.id = id,
            Item::Enum(e) => e.id = id,
            Item::Function(f) => f.id = id,
            Item::Impl(i) => i.id = id,
            Item::Constant(c) => c.id = id,
        }
    }
}

/// A struct definition in the Regseq model.
///
/// Covers: driver structs, register block structs, configuration types.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StructDef {
    pub id: ItemId,
    pub canonical_path: CanonicalPath,
    pub name: String,
    pub fields: Vec<StructFieldDef>,
    /// Struct-level attributes (e.g. repr(C), register_block metadata).
    pub attributes: Vec<ItemAttribute>,
    pub provenance: Provenance,
}

/// A struct field definition with canonical identity for cross-linking.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StructFieldDef {
    /// Canonical path for field-level cross-referencing.
    /// Example: `"pl011_full_spec::driver::Pl011FullSpec::regs"`
    pub canonical_path: CanonicalPath,
    pub name: String,
    /// Type reference (item-level only; not `SemanticType`).
    pub ty: TypeRef,
    /// Field-level attributes (e.g. resource classification from `#[rseq::*]`).
    pub attributes: Vec<FieldAttribute>,
}

/// An enum definition in the Regseq model.
///
/// Covers: interrupt class enums, error types, mode/config enums.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnumDef {
    pub id: ItemId,
    pub canonical_path: CanonicalPath,
    pub name: String,
    pub variants: Vec<EnumVariantDef>,
    pub provenance: Provenance,
}

/// An enum variant definition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnumVariantDef {
    pub name: String,
    /// Discriminant value, if explicitly assigned.
    pub discriminant: Option<u64>,
    /// Variant fields (empty for unit variants).
    pub fields: Vec<VariantFieldDef>,
}

/// A field within a non-unit enum variant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VariantFieldDef {
    pub name: Option<String>,
    pub ty: TypeRef,
}
