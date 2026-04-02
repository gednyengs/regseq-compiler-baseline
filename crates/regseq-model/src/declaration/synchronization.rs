//! Synchronization sub-family declarations.
//!
//! Synchronization is structurally distinct from other resources due to its
//! multiple sub-families: field declarations, lock operations, completion
//! operations, and guard scopes. Each sub-family is kept separate to
//! preserve the typed distinctions required by later analysis.

use serde::{Deserialize, Serialize};

use super::provenance::Provenance;

/// Set of synchronization declarations across all sub-families.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SynchronizationDeclSet {
    /// Synchronization field declarations.
    pub sync_fields: Vec<SyncFieldDecl>,
    /// Lock operation evidence (construct-owned).
    pub lock_operations: Vec<LockOperationDecl>,
    /// Completion operation evidence (construct-owned).
    pub completion_operations: Vec<CompletionOperationDecl>,
    /// Guard scope evidence (construct-owned).
    pub guard_scopes: Vec<GuardScopeDecl>,
}

/// A normalized synchronization field declaration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncFieldDecl {
    /// Field name on the driver struct.
    pub field_name: String,
    /// Kind of synchronization primitive.
    pub kind: SyncFieldKind,
    /// Payload type, if applicable (e.g. for mutex wrapping a value).
    pub payload_type: Option<String>,
    /// Provenance.
    pub provenance: Provenance,
}

/// Typed synchronization primitive kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SyncFieldKind {
    /// Mutex primitive.
    Mutex,
    /// Spin lock primitive.
    SpinLock,
    /// IRQ-safe spin lock primitive.
    IrqSpinLock,
    /// Completion primitive.
    Completion,
}

impl SyncFieldKind {
    /// Parse from the raw string representation.
    pub fn from_raw(s: &str) -> Option<Self> {
        match s {
            "mutex" => Some(Self::Mutex),
            "spinlock" => Some(Self::SpinLock),
            "irq_spinlock" => Some(Self::IrqSpinLock),
            "completion" => Some(Self::Completion),
            _ => None,
        }
    }

    /// Return the canonical string label.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Mutex => "mutex",
            Self::SpinLock => "spinlock",
            Self::IrqSpinLock => "irq_spinlock",
            Self::Completion => "completion",
        }
    }
}

/// A normalized lock operation evidence record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LockOperationDecl {
    /// Kind of lock primitive used.
    pub lock_kind: SyncFieldKind,
    /// Function containing this lock operation.
    pub containing_fn: String,
    /// Provenance.
    pub provenance: Provenance,
}

/// A normalized completion operation evidence record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompletionOperationDecl {
    /// Completion method invoked.
    pub method: CompletionMethod,
    /// Function containing this completion operation.
    pub containing_fn: String,
    /// Provenance.
    pub provenance: Provenance,
}

/// Typed completion operation method.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CompletionMethod {
    /// Wait for completion.
    Wait,
    /// Signal completion.
    Signal,
    /// Reset the completion.
    Reset,
}

impl CompletionMethod {
    /// Parse from the raw string representation.
    pub fn from_raw(s: &str) -> Option<Self> {
        match s {
            "wait" => Some(Self::Wait),
            "signal" => Some(Self::Signal),
            "reset" => Some(Self::Reset),
            _ => None,
        }
    }

    /// Return the canonical string label.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Wait => "wait",
            Self::Signal => "signal",
            Self::Reset => "reset",
        }
    }
}

/// A normalized guard scope evidence record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GuardScopeDecl {
    /// Kind of lock primitive guarding this scope.
    pub lock_kind: SyncFieldKind,
    /// Function containing this guard scope.
    pub containing_fn: String,
    /// Provenance.
    pub provenance: Provenance,
}
