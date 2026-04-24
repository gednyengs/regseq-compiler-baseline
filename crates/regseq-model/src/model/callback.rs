//! Callback-family and callback-context model types.
//!
//! These tags are shared by RegseqModel query helpers, reviewer-facing
//! analyses, and backend lowering plans.

use serde::{Deserialize, Serialize};

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
