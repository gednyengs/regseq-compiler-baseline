//! Function definition and role/kind enums for the item hierarchy.
//!
//! `FunctionKind` captures structural origin (where this function comes from
//! in the source). `FunctionRole` captures semantic purpose (what this
//! function does for the backend). These are orthogonal concerns and both
//! coexist on `FunctionDef`.
//!
//! `FunctionDef` is header-only at this task boundary. The executable body
//! (`SemanticBody`) will be added as a field in the semantic body lowering
//! task. See the doc comment on `FunctionDef` for the migration path.

use serde::{Deserialize, Serialize};

use crate::declaration::Provenance;

use super::identity::{CanonicalPath, ItemId};
use super::impl_def::TraitRef;

/// A function definition in the Regseq item hierarchy (header-only).
///
/// Carries identity, structural origin, and semantic role. The executable
/// body (`SemanticBody`) is deliberately absent at this task boundary.
///
/// Migration path: when `SemanticBody` is introduced in the semantic body
/// lowering task, add `pub body: SemanticBody` here. All `FunctionDef`
/// construction sites update to supply the body. This is an additive,
/// non-breaking change within the crate.
///
/// Distinct from `declaration::bodies::FnKind` / `declaration::mir_body::FnKind`,
/// which are declaration-layer classifications (Callback, Isr, Helper,
/// FreeFunction). `FunctionKind` here is a structural origin classification;
/// `FunctionRole` is the semantic purpose classification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FunctionDef {
    pub id: ItemId,
    pub canonical_path: CanonicalPath,
    pub name: String,
    /// Structural origin (free, inherent, trait).
    pub kind: FunctionKind,
    /// Semantic role for backend dispatch.
    pub role: FunctionRole,
    pub provenance: Provenance,
}

/// Structural origin: where this function comes from in the Rust source.
///
/// This is orthogonal to `FunctionRole` (semantic purpose). A `TraitMethod`
/// may be `Lifecycle(Init)` or `DeviceClassCallback { .. }`. An
/// `InherentMethod` may be `Helper` or `Isr { .. }`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum FunctionKind {
    /// Free function (not associated with any type).
    FreeFunction,
    /// Inherent method on a type.
    InherentMethod {
        /// The owning type.
        owner: ItemId,
    },
    /// Trait implementation method.
    TraitMethod {
        /// The trait being implemented.
        trait_ref: TraitRef,
        /// The implementing type.
        owner: ItemId,
    },
}

/// Semantic role for backend dispatch.
///
/// Classifies what a function does for the backend. This is the semantic
/// meaning, not the structural origin.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum FunctionRole {
    /// Lifecycle callback (init, deinit).
    Lifecycle { role: LifecycleRole },
    /// Device-class callback, validated against the declaration-layer schema.
    DeviceClassCallback {
        class: DeviceClassKind,
        callback: DeviceCallbackRole,
    },
    /// Interrupt service routine.
    Isr {
        /// Interrupt binding target for this ISR.
        binding: String,
    },
    /// Helper function (internal implementation detail).
    Helper,
}

/// Lifecycle role variants.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LifecycleRole {
    Init,
    Deinit,
}

/// Device class kind for callback classification.
///
/// `Other(String)` is a forward-compatibility extension point for device
/// classes not yet modeled as dedicated variants. It must not be used as a
/// backdoor for ad hoc semantic classification -- new device classes should
/// get dedicated enum variants.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeviceClassKind {
    Serial,
    Network,
    Storage,
    /// Extension point for device classes not yet modeled as dedicated variants.
    Other(String),
}

/// Device-class callback role.
///
/// First-slice callback vocabulary for the serial device class, plus a
/// schema-backed extension point.
///
/// `Named(String)` is not cross-item identity. It is a validated callback
/// schema member name for device classes not yet modeled as dedicated
/// variants. It must not be used for ad hoc semantic classification --
/// new device classes should get dedicated enum variants.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeviceCallbackRole {
    // Serial first slice
    PortOpen,
    PortClose,
    Configure,
    TxByte,
    RxChar,
    TxReady,
    TxEmpty,
    FlushRxFifo,
    FlushTxFifo,
    SendBreak,
    /// Schema-backed extension point for callbacks not yet modeled.
    Named(String),
}
