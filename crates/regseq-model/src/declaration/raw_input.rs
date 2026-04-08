//! Serde-only mirror types for the raw frontend artifact JSON.
//!
//! These types exist solely to deserialize the JSON output of
//! `regseq-driver`'s `RawFrontendArtifact` without pulling in the
//! driver's nightly rustc dependency graph. They match the raw JSON
//! shape field-for-field.

use serde::{Deserialize, Serialize};

/// Top-level raw frontend artifact as serialized to JSON by `regseq-driver`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawArtifactInput {
    pub driver_name: String,
    pub target_os: String,
    pub identity: Option<RawIdentityInput>,
    pub register_access: Option<RawRegisterAccessInput>,
    pub trait_callbacks: Option<RawTraitSurfaceInput>,
    pub firmware_properties: Option<RawPropertySurfaceInput>,
    pub clock: Option<RawClockSurfaceInput>,
    pub reset: Option<RawResetSurfaceInput>,
    pub pinctrl: Option<RawPinCtrlSurfaceInput>,
    pub interrupts: Option<RawInterruptSurfaceInput>,
    pub synchronization: Option<RawSynchronizationSurfaceInput>,
    pub timed_delay: Option<RawTimedDelaySurfaceInput>,
    pub logging: Option<RawLoggingSurfaceInput>,
    pub pm_gate: Option<RawPmGateSurfaceInput>,
    #[serde(default)]
    pub bodies: Option<RawBodySurfaceInput>,
    /// Identity enrichment surface: canonical paths, ownership, field identity.
    #[serde(default)]
    pub identity_enrichment: Option<RawIdentityEnrichmentInput>,
}

// ---- Identity ----

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawIdentityInput {
    pub driver_name: String,
    pub compatible: Vec<String>,
    pub device_class: String,
    /// Portable bus/fabric identity from `#[rseq::driver(bus = "...")]`.
    #[serde(default)]
    pub bus: Option<String>,
}

// ---- Register access ----

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawRegisterAccessInput {
    pub register_blocks: Vec<RawRegisterBlockInput>,
    pub mmio_fields: Vec<RawMmioFieldInput>,
    pub wrapper_call_sites: Vec<RawWrapperCallSiteInput>,
    pub poll_call_sites: Vec<RawPollCallSiteInput>,
    pub register_descriptors: Vec<RawRegisterDescriptorInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawRegisterBlockInput {
    pub name: String,
    pub fields: Vec<RawRegisterFieldInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawRegisterFieldInput {
    pub name: String,
    pub access_width: Option<u32>,
    pub endian: Option<String>,
    pub side_effects: Option<String>,
    pub hw_updates: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawMmioFieldInput {
    pub field_name: String,
    pub mmio_id: Option<String>,
    pub register_block_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawWrapperCallSiteInput {
    pub method: String,
    pub containing_fn: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawPollCallSiteInput {
    pub containing_fn: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawRegisterDescriptorInput {
    pub register_module: String,
    pub fields: Vec<RawRegisterDescriptorFieldInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawRegisterDescriptorFieldInput {
    pub field_name: String,
    pub offset: u32,
    pub width: u32,
    pub named_values: Vec<RawRegisterNamedValueInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawRegisterNamedValueInput {
    pub name: String,
    pub value: u32,
}

// ---- Trait callbacks ----

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawTraitSurfaceInput {
    pub lifecycle_impls: Vec<RawTraitImplInput>,
    pub serial_impls: Vec<RawTraitImplInput>,
    pub serial_console_impls: Vec<RawTraitImplInput>,
    pub power_management_impls: Vec<RawTraitImplInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawTraitImplInput {
    pub driver_type: String,
    pub context: String,
}

// ---- Firmware properties ----

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawPropertySurfaceInput {
    pub fields: Vec<RawPropertyFieldInput>,
    pub lints: Vec<RawPropertyLintInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawPropertyFieldInput {
    pub driver_type: String,
    pub field_name: String,
    pub field_type: String,
    pub property_id: String,
    pub default_literal: Option<String>,
    pub vendor_prefix: Option<String>,
    pub required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum RawPropertyLintInput {
    RequiredWithDefault {
        driver_type: String,
        field_name: String,
        property_id: String,
    },
}

// ---- Clock ----

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawClockSurfaceInput {
    pub fields: Vec<RawClockFieldInput>,
    pub call_sites: Vec<RawClockCallSiteInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawClockFieldInput {
    pub field_name: String,
    pub clock_id: String,
    pub required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawClockCallSiteInput {
    pub method: String,
    pub containing_fn: String,
}

// ---- Reset ----

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawResetSurfaceInput {
    pub fields: Vec<RawResetFieldInput>,
    pub call_sites: Vec<RawResetCallSiteInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawResetFieldInput {
    pub field_name: String,
    pub required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawResetCallSiteInput {
    pub method: String,
    pub containing_fn: String,
}

// ---- PinCtrl ----

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawPinCtrlSurfaceInput {
    pub fields: Vec<RawPinCtrlFieldInput>,
    pub call_sites: Vec<RawPinCtrlCallSiteInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawPinCtrlFieldInput {
    pub field_name: String,
    pub required: bool,
    pub pinctrl_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawPinCtrlCallSiteInput {
    pub state_type: String,
    pub containing_fn: String,
}

// ---- Interrupts ----

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawInterruptSurfaceInput {
    pub resources: Vec<RawInterruptResourceInput>,
    pub isr_bindings: Vec<RawIsrBindingInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawInterruptResourceInput {
    pub field_name: String,
    pub interrupt_id: String,
    pub shape: String,
    pub layer2: RawInterruptLayer2Input,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawInterruptLayer2Input {
    pub trigger_type: Option<String>,
    pub trigger_polarity: Option<String>,
    pub priority: Option<String>,
    pub shared: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawIsrBindingInput {
    pub handler_name: String,
    pub bind_target: String,
    pub has_index_param: bool,
    pub context: String,
}

// ---- Synchronization ----

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawSynchronizationSurfaceInput {
    pub sync_fields: Vec<RawSyncFieldInput>,
    pub lock_operations: Vec<RawLockOperationInput>,
    pub completion_operations: Vec<RawCompletionOperationInput>,
    pub guard_scopes: Vec<RawGuardScopeInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawSyncFieldInput {
    pub field_name: String,
    pub kind: String,
    pub payload_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawLockOperationInput {
    pub lock_kind: String,
    pub containing_fn: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawCompletionOperationInput {
    pub method: String,
    pub containing_fn: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawGuardScopeInput {
    pub lock_kind: String,
    pub containing_fn: String,
}

// ---- Timed delay ----

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawTimedDelaySurfaceInput {
    pub call_sites: Vec<RawTimedDelayCallSiteInput>,
    pub duration_ctors: Vec<RawDurationCtorSiteInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawTimedDelayCallSiteInput {
    pub intrinsic: String,
    pub path_kind: String,
    pub containing_fn: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawDurationCtorSiteInput {
    pub ctor: String,
    pub containing_fn: String,
}

// ---- Logging ----

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawLoggingSurfaceInput {
    pub call_sites: Vec<RawLogCallSiteInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawLogCallSiteInput {
    pub level: String,
    pub containing_fn: String,
    pub canonical: bool,
    pub template: String,
    pub arg_count: usize,
    pub arg_kinds: Vec<String>,
}

// ---- PM gate ----

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawPmGateSurfaceInput {
    pub gates: Vec<RawPmGateInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawPmGateInput {
    pub driver_type: String,
    pub properties: RawPmGatePropertiesInput,
    pub has_pm_impl: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawPmGatePropertiesInput {
    pub idle: bool,
    pub wakeup_capable: bool,
}

// ---- Bodies (MIR-derived) ----

/// MIR-derived body surface from the raw frontend artifact.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawBodySurfaceInput {
    /// Extracted function bodies.
    pub functions: Vec<RawMirExtractedFnInput>,
}

/// A MIR-extracted function entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawMirExtractedFnInput {
    /// Function name.
    pub name: String,
    /// Stable def-path for provenance.
    pub def_path: String,
    /// Function classification.
    pub kind: RawFnKindInput,
    /// Receiver style.
    pub receiver: String,
    /// Parameters (excluding self).
    pub params: Vec<RawFnParamInput>,
    /// Return type, if non-unit.
    #[serde(default)]
    pub return_type: Option<super::mir_body::MirTy>,
    /// MIR body (typed deserialization matching driver's MIR extraction output).
    pub body: super::mir_body::MirBody,
    /// Helper functions called from this body.
    pub helper_calls: Vec<String>,
    /// Monomorphization key for generic-instantiated functions.
    #[serde(default)]
    pub monomorphization: Option<super::mir_body::MonomorphizationKey>,
}

/// Function classification.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum RawFnKindInput {
    /// Trait callback.
    Callback {
        trait_name: String,
        method_name: String,
    },
    /// ISR handler.
    Isr,
    /// Helper method.
    Helper,
    /// Free function.
    FreeFunction,
}

/// Function parameter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawFnParamInput {
    /// Parameter name.
    pub name: String,
    /// Parameter type.
    pub ty: super::mir_body::MirTy,
}

// ---- Identity enrichment ----

/// Identity and ownership facts from the Phase 0 extraction enrichment.
///
/// This is the canonical Phase 0 handoff consumed by later phases to build
/// `Module`, `ItemStore`, and `CrossLinkTable`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawIdentityEnrichmentInput {
    /// Module containment facts.
    pub modules: Vec<RawModuleFactInput>,
    /// All structs with identity and field-level facts.
    pub structs: Vec<RawStructFactInput>,
    /// All enums with identity.
    pub enums: Vec<RawEnumFactInput>,
    /// All impl blocks with method/constant membership.
    pub impl_blocks: Vec<RawImplBlockFactInput>,
    /// All free (non-method) functions.
    pub free_functions: Vec<RawFunctionFactInput>,
}

/// A module with its direct children.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawModuleFactInput {
    /// Canonical path of the module.
    pub canonical_path: String,
    /// Short module name.
    pub name: String,
    /// Canonical paths of items directly contained in this module.
    pub child_items: Vec<String>,
}

/// A struct with field-level identity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawStructFactInput {
    /// Canonical path of the struct.
    pub canonical_path: String,
    /// Short struct name.
    pub name: String,
    /// Canonical path of the containing module.
    pub module_path: String,
    /// Whether this struct carries `#[rseq::driver]`.
    pub is_driver: bool,
    /// Whether this struct carries `#[rseq::register_block]`.
    pub is_register_block: bool,
    /// Field-level identity facts.
    pub fields: Vec<RawFieldFactInput>,
}

/// Field-level identity for cross-link resolution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawFieldFactInput {
    /// Short field name.
    pub name: String,
    /// Canonical path: parent struct path + `::` + field name.
    pub canonical_path: String,
    /// Resource classification from `#[rseq::*]` attributes on this field.
    #[serde(default)]
    pub rseq_kind: Option<RawRseqFieldKindInput>,
}

/// Resource classification derived from `#[rseq::*]` attributes on struct fields.
///
/// Typed mirror of the driver-side `RseqFieldKind`. Tagged enum, not stringly
/// typed.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum RawRseqFieldKindInput {
    /// Field carries `#[rseq::mmio(...)]`.
    Mmio { id: String },
    /// Field carries `#[rseq::clock(...)]`.
    Clock { id: String },
    /// Field carries `#[rseq::pinctrl(...)]`.
    PinCtrl { id: String },
    /// Field carries `#[rseq::interrupt(...)]`.
    Interrupt { id: String },
    /// Field carries `#[rseq::property(...)]`.
    Property { id: String },
}

/// An enum with canonical identity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawEnumFactInput {
    /// Canonical path of the enum.
    pub canonical_path: String,
    /// Short enum name.
    pub name: String,
    /// Canonical path of the containing module.
    pub module_path: String,
    /// Variant names.
    pub variants: Vec<String>,
}

/// An impl block with structural membership facts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawImplBlockFactInput {
    /// Canonical path of the impl block.
    pub canonical_path: String,
    /// Canonical path of the self type.
    pub self_type_path: String,
    /// Short self type name.
    pub self_type_name: String,
    /// If trait impl: the trait's canonical path.
    pub trait_path: Option<String>,
    /// If trait impl: short trait name.
    pub trait_name: Option<String>,
    /// Whether this is an inherent impl (no trait).
    pub is_inherent: bool,
    /// Methods in this impl block.
    pub methods: Vec<RawImplMethodFactInput>,
    /// Associated constants in this impl block.
    pub constants: Vec<RawAssociatedConstantFactInput>,
}

/// A method within an impl block.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawImplMethodFactInput {
    /// Short method name.
    pub name: String,
    /// Canonical path of the method.
    pub canonical_path: String,
    /// Whether this method carries `#[rseq::isr]`.
    pub is_isr: bool,
}

/// An associated constant within an impl block.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawAssociatedConstantFactInput {
    /// Short constant name.
    pub name: String,
    /// Canonical path of the constant.
    pub canonical_path: String,
    /// Type as rendered from HIR.
    pub ty: String,
    /// Canonical path of the owning impl block.
    pub owner_impl_path: String,
}

/// A free (non-method) function.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawFunctionFactInput {
    /// Short function name.
    pub name: String,
    /// Canonical path of the function.
    pub canonical_path: String,
    /// Canonical path of the containing module.
    pub module_path: String,
}
