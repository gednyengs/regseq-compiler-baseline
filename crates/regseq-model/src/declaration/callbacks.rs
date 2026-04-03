//! Callback-family impl declarations and callback contexts.
//!
//! The raw artifact currently preserves callback-family impl facts, not
//! method-by-method callback declarations. This module normalizes at
//! impl level accordingly. Method-level callback identity can be added as
//! a follow-on refinement if analysis pressure proves it necessary.

use serde::{Deserialize, Serialize};

use super::provenance::Provenance;

/// Set of all callback-family impl declarations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CallbackDeclSet {
    /// Lifecycle trait impl declarations.
    pub lifecycle_impls: Vec<CallbackImplDecl>,
    /// Serial trait impl declarations.
    pub serial_impls: Vec<CallbackImplDecl>,
    /// Serial console trait impl declarations (optional companion trait).
    pub serial_console_impls: Vec<CallbackImplDecl>,
    /// Power management trait impl declarations.
    pub power_management_impls: Vec<CallbackImplDecl>,
}

/// A single callback-family impl declaration.
///
/// Normalized at impl level (not method level) to match what the raw
/// artifact currently provides.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CallbackImplDecl {
    /// The driver type that implements this callback family.
    pub driver_type: String,
    /// The callback context class for this impl.
    pub context: CallbackContext,
    /// Which callback family this impl belongs to.
    pub family: CallbackFamily,
    /// Provenance back to the raw artifact.
    pub provenance: Provenance,
}

/// Typed callback family identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum CallbackFamily {
    /// Driver lifecycle (init, deinit).
    Lifecycle,
    /// Per-device-class serial callbacks.
    Serial,
    /// Optional serial console companion.
    SerialConsole,
    /// Suspend/resume power management.
    PowerManagement,
}

impl CallbackFamily {
    /// Return the canonical string label.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Lifecycle => "lifecycle",
            Self::Serial => "serial",
            Self::SerialConsole => "serial_console",
            Self::PowerManagement => "power_management",
        }
    }
}

/// Callback-entry context class.
///
/// Maps the three accepted context classes from the synchronization and
/// callback-context baseline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CallbackContext {
    /// Framework-managed entry (`F`).
    Framework,
    /// Subsystem-serialized entry (`S`).
    SubsystemSerialized,
    /// Explicit handler-managed entry (`H`).
    HandlerManaged,
}

impl CallbackContext {
    /// Parse from the raw short label.
    pub fn from_raw(s: &str) -> Option<Self> {
        match s {
            "F" | "framework" => Some(Self::Framework),
            "S" | "subsystem_serialized" => Some(Self::SubsystemSerialized),
            "H" | "handler_managed" => Some(Self::HandlerManaged),
            _ => None,
        }
    }

    /// Return the canonical short label.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Framework => "F",
            Self::SubsystemSerialized => "S",
            Self::HandlerManaged => "H",
        }
    }
}
