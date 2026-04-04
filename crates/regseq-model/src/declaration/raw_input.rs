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

// ---- Bodies ----

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawBodySurfaceInput {
    pub functions: Vec<RawExtractedFnInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawExtractedFnInput {
    pub name: String,
    pub kind: RawFnKindInput,
    pub receiver: String,
    pub params: Vec<RawFnParamInput>,
    #[serde(default)]
    pub return_type: Option<String>,
    pub body: RawBodyExprInput,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum RawFnKindInput {
    Callback {
        trait_name: String,
        method_name: String,
    },
    Isr,
    Helper,
    FreeFunction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawFnParamInput {
    pub name: String,
    pub ty: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum RawBodyStmtInput {
    Expr {
        expr: RawBodyExprInput,
    },
    Semi {
        expr: RawBodyExprInput,
    },
    Let {
        name: String,
        mutable: bool,
        #[serde(default)]
        ty: Option<String>,
        #[serde(default)]
        init: Option<RawBodyExprInput>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawMatchArmInput {
    pub pattern: String,
    #[serde(default)]
    pub guard: Option<RawBodyExprInput>,
    pub body: RawBodyExprInput,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawStructFieldInput {
    pub name: String,
    pub value: RawBodyExprInput,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum RawBodyExprInput {
    Block {
        stmts: Vec<RawBodyStmtInput>,
        #[serde(default)]
        expr: Option<Box<RawBodyExprInput>>,
    },
    MethodCall {
        receiver: Box<RawBodyExprInput>,
        method: String,
        #[serde(default)]
        type_args: Vec<String>,
        args: Vec<RawBodyExprInput>,
    },
    Call {
        path: String,
        #[serde(default)]
        type_args: Vec<String>,
        args: Vec<RawBodyExprInput>,
    },
    Field {
        base: Box<RawBodyExprInput>,
        name: String,
    },
    Path {
        path: String,
    },
    LitInt {
        value: String,
        #[serde(default)]
        suffix: Option<String>,
    },
    LitBool {
        value: bool,
    },
    LitStr {
        value: String,
    },
    Binary {
        op: String,
        lhs: Box<RawBodyExprInput>,
        rhs: Box<RawBodyExprInput>,
    },
    Unary {
        op: String,
        operand: Box<RawBodyExprInput>,
    },
    Cast {
        expr: Box<RawBodyExprInput>,
        ty: String,
    },
    If {
        cond: Box<RawBodyExprInput>,
        then_branch: Box<RawBodyExprInput>,
        #[serde(default)]
        else_branch: Option<Box<RawBodyExprInput>>,
    },
    Loop {
        body: Box<RawBodyExprInput>,
    },
    Return {
        #[serde(default)]
        expr: Option<Box<RawBodyExprInput>>,
    },
    Try {
        expr: Box<RawBodyExprInput>,
    },
    Ref {
        mutable: bool,
        expr: Box<RawBodyExprInput>,
    },
    Assign {
        target: Box<RawBodyExprInput>,
        value: Box<RawBodyExprInput>,
    },
    AssignOp {
        op: String,
        target: Box<RawBodyExprInput>,
        value: Box<RawBodyExprInput>,
    },
    Tuple {
        elements: Vec<RawBodyExprInput>,
    },
    Array {
        elements: Vec<RawBodyExprInput>,
    },
    Index {
        base: Box<RawBodyExprInput>,
        index: Box<RawBodyExprInput>,
    },
    Struct {
        path: String,
        fields: Vec<RawStructFieldInput>,
        #[serde(default)]
        base: Option<Box<RawBodyExprInput>>,
    },
    Closure {
        params: Vec<String>,
        body: Box<RawBodyExprInput>,
    },
    Match {
        scrutinee: Box<RawBodyExprInput>,
        arms: Vec<RawMatchArmInput>,
    },
    Break {
        #[serde(default)]
        expr: Option<Box<RawBodyExprInput>>,
    },
    Continue,
    Unsupported {
        hir_kind: String,
        #[serde(default)]
        span_text: Option<String>,
    },
}
