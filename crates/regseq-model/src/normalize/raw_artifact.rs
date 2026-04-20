//! Rustc-independent mirror of the Phase-5 `RawFrontendArtifact`
//! payload consumed by [`normalize`](super::normalize).
//!
//! The authoritative schema is defined in
//! `crates/regseq-driver/src/raw_artifact.rs`. That file lives in the
//! rustc-dependent driver crate so the extraction code can populate it
//! from `TyCtxt`; this mirror exists so the rustc-independent
//! `regseq-model` crate can deserialize the same JSON wire format
//! without the circular dependency.
//!
//! The shapes match the driver's schema field-for-field. Optional
//! surfaces use `Option<...>`. Newer surfaces (`intrinsic_registry`,
//! `framework_types`, `abstraction_types`, `source_locations`,
//! `identity_enrichment`) are represented so fixtures that carry them
//! round-trip.
//!
//! Enum-typed fields on the wire are modelled as `String` "tags" here
//! so the normalizer can detect and raise
//! [`NormalizeErrorKind::UnrecognizedEnumTag`] with the offending
//! string intact.
//!
//! [`NormalizeErrorKind::UnrecognizedEnumTag`]: super::NormalizeErrorKind::UnrecognizedEnumTag

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Top-level raw frontend artifact.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawFrontendArtifact {
    #[serde(default)]
    pub driver_name: String,
    #[serde(default)]
    pub target_os: String,
    #[serde(default)]
    pub identity: Option<RawDriverIdentity>,
    #[serde(default)]
    pub register_access: Option<RawRegisterAccessSurface>,
    #[serde(default)]
    pub trait_callbacks: Option<RawTraitSurface>,
    #[serde(default)]
    pub firmware_properties: Option<RawPropertySurface>,
    #[serde(default)]
    pub clock: Option<RawClockSurface>,
    #[serde(default)]
    pub reset: Option<RawResetSurface>,
    #[serde(default)]
    pub pinctrl: Option<RawPinCtrlSurface>,
    #[serde(default)]
    pub interrupts: Option<RawInterruptSurface>,
    #[serde(default)]
    pub synchronization: Option<RawSynchronizationSurface>,
    #[serde(default)]
    pub timed_delay: Option<RawTimedDelaySurface>,
    #[serde(default)]
    pub logging: Option<RawLoggingSurface>,
    #[serde(default)]
    pub pm_gate: Option<RawPmGateSurface>,
    #[serde(default)]
    pub bodies: Option<serde_json::Value>,
    #[serde(default)]
    pub identity_enrichment: Option<RawIdentityEnrichmentSurface>,
    #[serde(default)]
    pub intrinsic_registry: Option<serde_json::Value>,
    #[serde(default)]
    pub framework_types: Option<serde_json::Value>,
    #[serde(default)]
    pub abstraction_types: Option<serde_json::Value>,
    #[serde(default)]
    pub source_locations: Option<RawSourceLocationSurface>,
}

// ------------------------------------------------------------------- identity

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawDriverIdentity {
    #[serde(default)]
    pub driver_name: String,
    #[serde(default)]
    pub compatible: Vec<String>,
    #[serde(default)]
    pub device_class: String,
    #[serde(default)]
    pub bus: Option<String>,
}

// ---------------------------------------------------------- register access

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawRegisterAccessSurface {
    #[serde(default)]
    pub register_blocks: Vec<RawRegisterBlockDecl>,
    #[serde(default)]
    pub mmio_fields: Vec<RawMmioFieldDecl>,
    #[serde(default)]
    pub wrapper_call_sites: Vec<serde_json::Value>,
    #[serde(default)]
    pub poll_call_sites: Vec<serde_json::Value>,
    #[serde(default)]
    pub register_descriptors: Vec<RawRegisterDescriptorFact>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawRegisterBlockDecl {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub fields: Vec<RawRegisterFieldDecl>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawRegisterFieldDecl {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub access_width: Option<u32>,
    #[serde(default)]
    pub endian: Option<String>,
    #[serde(default)]
    pub side_effects: Option<String>,
    #[serde(default)]
    pub hw_updates: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawMmioFieldDecl {
    #[serde(default)]
    pub field_name: String,
    #[serde(default)]
    pub mmio_id: Option<String>,
    #[serde(default)]
    pub register_block_type: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawRegisterDescriptorFact {
    #[serde(default)]
    pub register_module: String,
    #[serde(default)]
    pub fields: Vec<RawRegisterDescriptorFieldFact>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawRegisterDescriptorFieldFact {
    #[serde(default)]
    pub field_name: String,
    #[serde(default)]
    pub offset: u32,
    #[serde(default)]
    pub width: u32,
    #[serde(default)]
    pub named_values: Vec<RawRegisterNamedValueFact>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawRegisterNamedValueFact {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub value: u32,
}

// -------------------------------------------------------------- trait surface

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawTraitSurface {
    #[serde(default)]
    pub lifecycle_impls: Vec<serde_json::Value>,
    #[serde(default)]
    pub serial_impls: Vec<serde_json::Value>,
    #[serde(default)]
    pub serial_console_impls: Vec<serde_json::Value>,
    #[serde(default)]
    pub power_management_impls: Vec<serde_json::Value>,
}

// ------------------------------------------------------------ properties

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawPropertySurface {
    #[serde(default)]
    pub fields: Vec<RawPropertyField>,
    #[serde(default)]
    pub lints: Vec<RawPropertyLint>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawPropertyField {
    #[serde(default)]
    pub driver_type: String,
    #[serde(default)]
    pub field_name: String,
    #[serde(default)]
    pub field_type: String,
    #[serde(default)]
    pub property_id: String,
    #[serde(default)]
    pub default_literal: Option<String>,
    #[serde(default)]
    pub vendor_prefix: Option<String>,
    #[serde(default)]
    pub required: bool,
}

/// Property-surface lint. `kind` is a free-form string so the
/// normalizer can surface [`NormalizeErrorKind::UnrecognizedEnumTag`]
/// on unknown values.
///
/// [`NormalizeErrorKind::UnrecognizedEnumTag`]: super::NormalizeErrorKind::UnrecognizedEnumTag
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawPropertyLint {
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub driver_type: String,
    #[serde(default)]
    pub field_name: String,
    #[serde(default)]
    pub property_id: String,
}

// -------------------------------------------------------------- clock

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawClockSurface {
    #[serde(default)]
    pub fields: Vec<RawClockField>,
    #[serde(default)]
    pub call_sites: Vec<serde_json::Value>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawClockField {
    #[serde(default)]
    pub field_name: String,
    #[serde(default)]
    pub clock_id: String,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub field_type: String,
}

// ---------------------------------------------------------------- reset

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawResetSurface {
    #[serde(default)]
    pub fields: Vec<RawResetField>,
    #[serde(default)]
    pub call_sites: Vec<serde_json::Value>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawResetField {
    #[serde(default)]
    pub field_name: String,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub field_type: String,
    #[serde(default)]
    pub reset_id: String,
}

// ---------------------------------------------------------------- pinctrl

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawPinCtrlSurface {
    #[serde(default)]
    pub fields: Vec<RawPinCtrlField>,
    #[serde(default)]
    pub call_sites: Vec<serde_json::Value>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawPinCtrlField {
    #[serde(default)]
    pub field_name: String,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub pinctrl_id: Option<String>,
    #[serde(default)]
    pub field_type: String,
}

// ---------------------------------------------------------------- interrupts

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawInterruptSurface {
    #[serde(default)]
    pub resources: Vec<RawInterruptResourceDecl>,
    #[serde(default)]
    pub isr_bindings: Vec<serde_json::Value>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawInterruptResourceDecl {
    #[serde(default)]
    pub field_name: String,
    #[serde(default)]
    pub interrupt_id: String,
    #[serde(default)]
    pub shape: String,
    #[serde(default)]
    pub field_type: String,
    #[serde(default)]
    pub layer2: RawInterruptLayer2Properties,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawInterruptLayer2Properties {
    #[serde(default)]
    pub trigger_type: Option<String>,
    #[serde(default)]
    pub trigger_polarity: Option<String>,
    #[serde(default)]
    pub priority: Option<String>,
    #[serde(default)]
    pub shared: Option<bool>,
}

// ------------------------------------------------------- synchronization

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawSynchronizationSurface {
    #[serde(default)]
    pub sync_fields: Vec<RawSyncFieldDecl>,
    #[serde(default)]
    pub lock_operations: Vec<serde_json::Value>,
    #[serde(default)]
    pub completion_operations: Vec<serde_json::Value>,
    #[serde(default)]
    pub guard_scopes: Vec<serde_json::Value>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawSyncFieldDecl {
    #[serde(default)]
    pub field_name: String,
    /// `"mutex"`, `"spinlock"`, `"irq_spinlock"`, `"completion"`. Any
    /// other value triggers `UnrecognizedEnumTag` at normalize time.
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub payload_type: Option<String>,
}

// --------------------------------------------------------------- timed-delay

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawTimedDelaySurface {
    #[serde(default)]
    pub call_sites: Vec<serde_json::Value>,
    #[serde(default)]
    pub duration_ctors: Vec<serde_json::Value>,
}

// --------------------------------------------------------------- logging

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawLoggingSurface {
    #[serde(default)]
    pub call_sites: Vec<serde_json::Value>,
}

// --------------------------------------------------------------- pm gate

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawPmGateSurface {
    #[serde(default)]
    pub gates: Vec<RawPmGate>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawPmGate {
    #[serde(default)]
    pub driver_type: String,
    #[serde(default)]
    pub properties: RawPmGateProperties,
    #[serde(default)]
    pub has_pm_impl: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawPmGateProperties {
    #[serde(default)]
    pub idle: bool,
    #[serde(default)]
    pub wakeup_capable: bool,
}

// ---------------------------------------------------- identity enrichment

/// Minimal rustc-independent mirror of the driver's
/// `IdentityEnrichmentSurface`. Only the fields consumed by
/// [`normalize`](super::normalize) are typed; others stay in
/// `serde_json::Value` to keep this mirror lean.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawIdentityEnrichmentSurface {
    #[serde(default)]
    pub modules: Vec<serde_json::Value>,
    #[serde(default)]
    pub structs: Vec<RawStructFact>,
    #[serde(default)]
    pub enums: Vec<serde_json::Value>,
    #[serde(default)]
    pub impl_blocks: Vec<serde_json::Value>,
    #[serde(default)]
    pub free_functions: Vec<serde_json::Value>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawStructFact {
    #[serde(default)]
    pub canonical_path: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub module_path: String,
    #[serde(default)]
    pub is_driver: bool,
    #[serde(default)]
    pub is_register_block: bool,
    #[serde(default)]
    pub fields: Vec<RawFieldFact>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawFieldFact {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub field_type: String,
    /// `"UserData"`, `"Resource"`, `"Property"`, etc. Any other value
    /// is `UnrecognizedEnumTag` when the field is interpreted.
    #[serde(default)]
    pub rseq_field_kind: String,
}

// --------------------------------------------------------- source locations

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawSourceLocationSurface {
    #[serde(default)]
    pub locations: BTreeMap<String, RawSourceLocation>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawSourceLocation {
    #[serde(default)]
    pub file: String,
    #[serde(default)]
    pub start_line: u32,
    #[serde(default)]
    pub start_col: u32,
    #[serde(default)]
    pub end_line: u32,
    #[serde(default)]
    pub end_col: u32,
}
