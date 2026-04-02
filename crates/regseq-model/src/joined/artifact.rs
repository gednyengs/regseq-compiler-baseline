//! Joined-artifact types for the Layer 1 + Layer 2 join.
//!
//! Each resource family has its own joined-entry type that carries the
//! declaration-side facts, optional binding-side facts, and the join
//! outcome. Unmatched declaration-only and binding-only entries are kept
//! as separate lists so they remain reviewable.

use serde::{Deserialize, Serialize};

use crate::binding::{
    BindingHeader, BindingProvenance, ClockBinding, InterruptBinding, JoinKey, MmioBinding,
    PinCtrlBinding, PropertyBinding, ResetBinding,
};
use crate::declaration::provenance::Provenance;
use crate::declaration::resources::{
    ClockFieldDecl, InterruptBindingHints, InterruptFieldShape, MmioFieldDecl, PinCtrlFieldDecl,
    ResetFieldDecl,
};
use crate::platform::PlatformValue;

/// The top-level joined artifact combining declaration and binding models.
///
/// Makes explicit what joined, what remained declaration-only, and what
/// remained binding-only. This is a reviewer-facing artifact that does
/// not contain backend/lowering structure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JoinedArtifact {
    /// Platform header from the binding model (deployment metadata).
    pub header: JoinedHeader,

    // ---- Properties ----
    /// Joined property entries (declaration matched with binding).
    pub properties_joined: Vec<PropertyJoinEntry>,
    /// Declaration-only property entries (no matching binding).
    pub properties_decl_only: Vec<PropertyDeclOnly>,
    /// Binding-only property entries (no matching declaration).
    pub properties_binding_only: Vec<PropertyBinding>,

    // ---- MMIO ----
    /// Joined MMIO entries.
    pub mmio_joined: Vec<MmioJoinEntry>,
    /// Declaration-only MMIO entries.
    pub mmio_decl_only: Vec<MmioFieldDecl>,
    /// Binding-only MMIO entries.
    pub mmio_binding_only: Vec<MmioBinding>,

    // ---- Interrupts ----
    /// Joined interrupt entries.
    pub interrupts_joined: Vec<InterruptJoinEntry>,
    /// Declaration-only interrupt entries.
    pub interrupts_decl_only: Vec<InterruptDeclOnly>,
    /// Binding-only interrupt entries.
    pub interrupts_binding_only: Vec<InterruptBinding>,

    // ---- Clocks ----
    /// Joined clock entries.
    pub clocks_joined: Vec<ClockJoinEntry>,
    /// Declaration-only clock entries.
    pub clocks_decl_only: Vec<ClockFieldDecl>,
    /// Binding-only clock entries.
    pub clocks_binding_only: Vec<ClockBinding>,

    // ---- Resets ----
    /// Joined reset entries.
    pub resets_joined: Vec<ResetJoinEntry>,
    /// Declaration-only reset entries.
    pub resets_decl_only: Vec<ResetFieldDecl>,
    /// Binding-only reset entries.
    pub resets_binding_only: Vec<ResetBinding>,

    // ---- PinCtrl ----
    /// Joined pin controller entries.
    pub pinctrl_joined: Vec<PinCtrlJoinEntry>,
    /// Declaration-only pin controller entries.
    pub pinctrl_decl_only: Vec<PinCtrlFieldDecl>,
    /// Binding-only pin controller entries.
    pub pinctrl_binding_only: Vec<PinCtrlBinding>,
}

/// Header from the binding model, passed through to the joined artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JoinedHeader {
    /// Schema version.
    pub version: String,
    /// Target OS.
    pub os: String,
    /// Target architecture.
    pub arch: String,
    /// Board label.
    pub board: String,
    /// Provenance from the binding side.
    pub binding_provenance: BindingProvenance,
}

impl From<BindingHeader> for JoinedHeader {
    fn from(h: BindingHeader) -> Self {
        Self {
            version: h.version,
            os: h.os,
            arch: h.arch,
            board: h.board,
            binding_provenance: h.provenance,
        }
    }
}

/// Outcome of a per-entry join attempt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum JoinOutcome {
    /// Both sides matched and the binding is complete (all expected
    /// binding fields are present).
    Matched,
    /// Both sides matched but the binding is incomplete (some fields
    /// are absent).
    MatchedIncomplete,
}

// ---- Property join ----

/// A joined property entry: declaration matched with optional binding value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PropertyJoinEntry {
    /// Property id (the join key).
    pub property_id: String,
    /// Field name on the driver struct.
    pub field_name: String,
    /// Field type as declared.
    pub field_type: String,
    /// Whether the property is required by the declaration.
    pub required: bool,
    /// Default literal value from the declaration, if any.
    pub default_literal: Option<String>,
    /// Binding-side value from Layer 2, if present.
    pub binding_value: Option<PlatformValue>,
    /// Join outcome.
    pub outcome: JoinOutcome,
    /// Declaration-side provenance.
    pub decl_provenance: Provenance,
    /// Binding-side provenance.
    pub binding_provenance: Option<BindingProvenance>,
}

/// A declaration-only property entry (no matching binding).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PropertyDeclOnly {
    /// Property id.
    pub property_id: String,
    /// Field name.
    pub field_name: String,
    /// Field type.
    pub field_type: String,
    /// Whether the property is required.
    pub required: bool,
    /// Default literal value, if any.
    pub default_literal: Option<String>,
    /// Declaration-side provenance.
    pub provenance: Provenance,
}

// ---- MMIO join ----

/// A joined MMIO entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MmioJoinEntry {
    /// Field name on the driver struct.
    pub field_name: String,
    /// MMIO id used as the join key.
    pub mmio_id: String,
    /// Register block type name from the declaration.
    pub register_block_type: Option<String>,
    /// Base address from the binding, if present.
    pub base: Option<i64>,
    /// Size from the binding, if present.
    pub size: Option<i64>,
    /// Join outcome.
    pub outcome: JoinOutcome,
    /// Declaration-side provenance.
    pub decl_provenance: Provenance,
    /// Binding-side provenance.
    pub binding_provenance: BindingProvenance,
}

// ---- Interrupt join ----

/// A joined interrupt entry.
///
/// Binding-model values are authoritative. Declaration-side binding hints
/// are preserved only as diagnostic annotations and are never merged into
/// the authoritative joined binding fields.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterruptJoinEntry {
    /// Field name on the driver struct.
    pub field_name: String,
    /// Interrupt id used as the join key.
    pub interrupt_id: String,
    /// Shape of the interrupt field from the declaration.
    pub shape: InterruptFieldShape,
    /// Authoritative trigger type from the binding.
    pub trigger_type: Option<String>,
    /// Authoritative trigger polarity from the binding.
    pub trigger_polarity: Option<String>,
    /// Authoritative priority from the binding.
    pub priority: Option<String>,
    /// Authoritative shared flag from the binding.
    pub shared: Option<bool>,
    /// Declaration-side binding hints preserved as diagnostic annotations only.
    /// These are NOT authoritative and must not override binding values.
    pub decl_binding_hints: InterruptBindingHints,
    /// Join outcome.
    pub outcome: JoinOutcome,
    /// Declaration-side provenance.
    pub decl_provenance: Provenance,
    /// Binding-side provenance.
    pub binding_provenance: BindingProvenance,
}

/// A declaration-only interrupt entry (no matching binding).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterruptDeclOnly {
    /// Field name.
    pub field_name: String,
    /// Interrupt id.
    pub interrupt_id: String,
    /// Shape.
    pub shape: InterruptFieldShape,
    /// Binding hints from declaration side (diagnostic only).
    pub binding_hints: InterruptBindingHints,
    /// Declaration-side provenance.
    pub provenance: Provenance,
}

// ---- Clock join ----

/// A joined clock entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClockJoinEntry {
    /// Field name on the driver struct.
    pub field_name: String,
    /// Clock id used as the join key.
    pub clock_id: String,
    /// Whether the clock is required by the declaration.
    pub required: bool,
    /// Default frequency from the binding, if present.
    pub default_frequency: Option<i64>,
    /// Join outcome.
    pub outcome: JoinOutcome,
    /// Declaration-side provenance.
    pub decl_provenance: Provenance,
    /// Binding-side provenance.
    pub binding_provenance: BindingProvenance,
}

// ---- Reset join ----

/// A joined reset entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResetJoinEntry {
    /// Field name used as the join key (field-name join).
    pub field_name: String,
    /// Whether the reset is required by the declaration.
    pub required: bool,
    /// Reset line id from the binding, if present.
    pub reset_line_id: Option<String>,
    /// Reset shared flag from the binding, if present.
    pub reset_shared: Option<bool>,
    /// Join outcome.
    pub outcome: JoinOutcome,
    /// Declaration-side provenance.
    pub decl_provenance: Provenance,
    /// Binding-side provenance.
    pub binding_provenance: BindingProvenance,
}

// ---- PinCtrl join ----

/// A joined pin controller entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PinCtrlJoinEntry {
    /// Field name on the driver struct.
    pub field_name: String,
    /// Join key used (explicit id or field name).
    pub join_key: JoinKey,
    /// Whether the pinctrl is required by the declaration.
    pub required: bool,
    /// Available pin states from the binding, if present.
    pub pin_states: Option<Vec<String>>,
    /// Join outcome.
    pub outcome: JoinOutcome,
    /// Declaration-side provenance.
    pub decl_provenance: Provenance,
    /// Binding-side provenance.
    pub binding_provenance: BindingProvenance,
}
