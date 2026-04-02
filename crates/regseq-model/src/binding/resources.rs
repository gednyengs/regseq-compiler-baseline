//! Resource-family binding entries from Layer 2 construct sections.
//!
//! Each resource family has its own typed binding struct with individually
//! optional fields, an explicit join key, and per-entity provenance.
//! This replaces the generic `BTreeMap<String, PlatformValue>` passthrough
//! from the parser output with typed per-family entries.

use serde::{Deserialize, Serialize};

use super::join_key::JoinKey;
use super::provenance::BindingProvenance;

// ---- MMIO ----

/// A normalized MMIO binding entry from `[mmio.{id}]`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MmioBinding {
    /// Join key identifying this MMIO region (explicit-id join).
    pub join_key: JoinKey,
    /// Base address of the MMIO region.
    pub base: Option<i64>,
    /// Size of the MMIO region in bytes.
    pub size: Option<i64>,
    /// Provenance tracing back to the Layer 2 source.
    pub provenance: BindingProvenance,
}

// ---- Interrupts ----

/// A normalized interrupt binding entry from `[interrupt.{id}]`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterruptBinding {
    /// Join key identifying this interrupt resource (explicit-id join).
    pub join_key: JoinKey,
    /// Trigger type (e.g. `"edge"`, `"level"`).
    pub trigger_type: Option<String>,
    /// Trigger polarity (e.g. `"rising"`, `"high"`).
    pub trigger_polarity: Option<String>,
    /// Priority level.
    pub priority: Option<String>,
    /// Whether the interrupt line is shared.
    pub shared: Option<bool>,
    /// Provenance tracing back to the Layer 2 source.
    pub provenance: BindingProvenance,
}

// ---- Clock ----

/// A normalized clock binding entry from `[clock.{id}]`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClockBinding {
    /// Join key identifying this clock resource (explicit-id join).
    pub join_key: JoinKey,
    /// Default frequency in Hz, if specified.
    pub default_frequency: Option<i64>,
    /// Provenance tracing back to the Layer 2 source.
    pub provenance: BindingProvenance,
}

// ---- Reset ----

/// A normalized reset binding entry from `[reset.{field_name}]`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResetBinding {
    /// Join key identifying this reset resource (field-name join).
    pub join_key: JoinKey,
    /// Reset line identifier.
    pub reset_line_id: Option<String>,
    /// Whether the reset line is shared.
    pub reset_shared: Option<bool>,
    /// Provenance tracing back to the Layer 2 source.
    pub provenance: BindingProvenance,
}

// ---- PinCtrl ----

/// A normalized pin controller binding entry from `[pinctrl.{id_or_field_name}]`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PinCtrlBinding {
    /// Join key identifying this pin controller resource.
    /// May be either an explicit id or a field name, depending on the
    /// Layer 2 file content.
    pub join_key: JoinKey,
    /// Available pin states (e.g. `["default", "sleep"]`).
    pub pin_states: Option<Vec<String>>,
    /// Provenance tracing back to the Layer 2 source.
    pub provenance: BindingProvenance,
}
