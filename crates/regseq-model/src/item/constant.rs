//! Constant definition for the item hierarchy.

use serde::{Deserialize, Serialize};

use crate::declaration::Provenance;

use super::identity::{CanonicalPath, ItemId};
use super::type_ref::TypeRef;

/// A named constant in the Regseq model.
///
/// Covers associated constants (DEFAULT_UART_CLOCK_HZ, RX_INTERRUPT_MASK),
/// module-level constants, and register bitfield values.
///
/// Constants with `owner: Some(id)` are associated constants on a type.
/// The owning type's `ImplDef` also lists them in `constants: Vec<ItemId>`.
/// The constant is reachable both through the impl and directly by `ItemId`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConstantDef {
    pub id: ItemId,
    pub canonical_path: CanonicalPath,
    /// Constant name as authored (e.g. "DEFAULT_UART_CLOCK_HZ").
    pub name: String,
    /// Owning type, if this is an associated constant.
    pub owner: Option<ItemId>,
    /// Type of the constant.
    pub ty: TypeRef,
    /// Statically known value, if evaluable.
    pub value: Option<ConstValue>,
    /// Backend-facing stable name for emission.
    /// Derived deterministically from `canonical_path`.
    pub stable_name: String,
    pub provenance: Provenance,
}

/// A statically known constant value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConstValue {
    Integer(i64),
    Bool(bool),
    Str(String),
}
