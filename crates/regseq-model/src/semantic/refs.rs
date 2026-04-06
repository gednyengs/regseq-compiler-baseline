//! Body-level semantic reference types.
//!
//! All references carry structured identity (`ItemId` + `CanonicalPath`)
//! rather than bare strings. Human-readable names remain for display and
//! emission, but identity flows through the structured carrying refs.

use serde::{Deserialize, Serialize};

use crate::item::identity::{CanonicalPath, ItemId};

use super::types::ResourceKind;

/// Reference to another function in the same semantic model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FunctionRef {
    /// Item ID of the target function.
    pub item: ItemId,
    /// Canonical path for unambiguous cross-referencing.
    pub canonical_path: CanonicalPath,
    /// Short display name (e.g., "set_baud_rate").
    pub name: String,
}

/// Reference to a driver-owned field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriverFieldRef {
    /// Item ID of the owning driver struct.
    pub owner: ItemId,
    /// Canonical path for field-level cross-referencing.
    /// Example: `"pl011_full_spec::driver::Pl011FullSpec::regs"`
    pub canonical_path: CanonicalPath,
    /// Short field name (e.g., "regs", "pending_tx").
    pub name: String,
}

/// Reference to a register within a register block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegisterRef {
    /// Item ID of the register block struct.
    pub block: ItemId,
    /// Canonical path for the register.
    pub canonical_path: CanonicalPath,
    /// Short register name (e.g., "ibrd", "cr").
    pub name: String,
}

/// Reference to a field within a register.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegisterFieldRef {
    /// The register containing this field.
    pub register: RegisterRef,
    /// Canonical path for the register field.
    pub canonical_path: CanonicalPath,
    /// Short field name (e.g., "baud_divint", "uarten").
    pub name: String,
}

/// SemanticModel-scoped interrupt-class identity.
///
/// Unique within a single `SemanticModel`. Not shared across models.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Serialize, Deserialize)]
pub struct InterruptClassId(pub u32);

/// Reference to a named interrupt class.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterruptClassRef {
    /// SemanticModel-scoped interrupt-class identity.
    pub id: InterruptClassId,
    /// Symbolic class name (e.g., "rx", "tx", "error").
    pub name: String,
}

/// Reference to a subsystem action from a closed, validated vocabulary.
///
/// Subsystem actions name semantic hooks, not target-OS helper names.
/// Linux, Zephyr, and future backends map these hooks to their own APIs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubsystemActionRef {
    /// Subsystem family.
    pub subsystem: SubsystemKind,
    /// Specific action within the subsystem.
    pub action: SubsystemActionKind,
}

/// Subsystem family for cross-subsystem actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SubsystemKind {
    /// Device-class I/O subsystem.
    DeviceClassIo,
    /// Buffering subsystem.
    Buffering,
}

/// Specific action within a subsystem.
///
/// Any subsystem action outside this closed vocabulary is a semantic-lowering
/// hard error. Backends do not interpret free-form subsystem or action strings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SubsystemActionKind {
    /// Ingest a single received unit into the subsystem buffer.
    IngestRxUnit,
    /// Publish the accumulated receive batch to the subsystem.
    PublishRxBatch,
}

/// A reference to a named constant with full identity.
///
/// Constants carry enough identity for deterministic ownership,
/// namespacing, and provenance. Backend-facing names are derived
/// separately from the symbolic identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConstantRef {
    /// Item ID of the `ConstantDef`, if the constant is in the model.
    /// `None` is reserved for framework-/backend-defined constants that are
    /// not authored `ConstantDef` items.
    pub id: Option<ItemId>,
    /// Canonical path for unambiguous cross-referencing.
    /// Example: `"pl011_full_spec::driver::Pl011FullSpec::DEFAULT_UART_CLOCK_HZ"`
    pub canonical_path: CanonicalPath,
    /// Short display name (e.g., "DEFAULT_UART_CLOCK_HZ").
    pub name: String,
    /// Backend-facing stable identifier for emission.
    /// Derived deterministically from `canonical_path`.
    pub stable_name: String,
    /// Statically known value, if available.
    pub known_value: Option<i128>,
}

/// A reference to a driver resource.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceRef {
    /// Resource kind (clock, reset, pinctrl).
    pub kind: ResourceKind,
    /// Driver struct field holding this resource.
    pub field: DriverFieldRef,
}
