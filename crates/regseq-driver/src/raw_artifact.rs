use serde::{Deserialize, Serialize};

use crate::extract::callback_context::CallbackContext;
use crate::extract::clock::{ClockCallSite, ClockField, ClockMethod, ClockSurface};
use crate::extract::driver_identity::DriverIdentity;
use crate::extract::interrupts::{
    InterruptFieldShape, InterruptLayer2Properties, InterruptResourceDecl, InterruptSurface,
    IsrBindTarget, IsrBinding,
};
use crate::extract::logging::{LogArgKind, LogCallSite, LogLevel, LoggingSurface};
use crate::extract::pinctrl::{PinCtrlCallSite, PinCtrlField, PinCtrlSurface};
use crate::extract::pm::{PmGate, PmGateProperties, PmGateSurface};
use crate::extract::property::{PropertyField, PropertyLint, PropertySurface};
use crate::extract::register_access::{
    MmioFieldDecl, PollCallSite, RegisterAccessSurface, RegisterBlockDecl, RegisterDescriptorFact,
    RegisterDescriptorFieldFact, RegisterFieldDecl, RegisterNamedValueFact, WrapperCallSite,
    WrapperMethod,
};
use crate::extract::reset::{ResetCallSite, ResetField, ResetMethod, ResetSurface};
use crate::extract::synchronization::{
    CompletionMethod, CompletionOperation, GuardScope, LockOperation, SyncFieldDecl, SyncFieldKind,
    SynchronizationSurface,
};
use crate::extract::timed_delay::{
    DelayIntrinsic, DelayPathKind, DurationCtor, DurationCtorSite, TimedDelayCallSite,
    TimedDelaySurface,
};
use crate::extract::traits::{
    LifecycleImpl, PowerManagementImpl, SerialConsoleImpl, SerialImpl, TraitSurface,
};

// ---------------------------------------------------------------------------
// Top-level artifact
// ---------------------------------------------------------------------------

/// Raw frontend artifact: a serializable snapshot of all extraction families
/// captured from a target crate. This is explicitly pre-normalized -- it
/// preserves the extractor outputs without joining, collapsing, or
/// interpreting them into a backend-oriented model.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RawFrontendArtifact {
    /// Driver name extracted from `#[rseq::driver(name = "...")]`.
    pub driver_name: String,
    /// Target OS from compiler options.
    pub target_os: String,
    /// Driver identity attributes.
    pub identity: Option<RawDriverIdentity>,
    /// Register/MMIO extraction facts.
    pub register_access: Option<RawRegisterAccessSurface>,
    /// Trait callback capture facts.
    pub trait_callbacks: Option<RawTraitSurface>,
    /// Firmware-property declarations.
    pub firmware_properties: Option<RawPropertySurface>,
    /// Clock resource declarations.
    pub clock: Option<RawClockSurface>,
    /// ResetLine resource declarations.
    pub reset: Option<RawResetSurface>,
    /// PinCtrl resource declarations.
    pub pinctrl: Option<RawPinCtrlSurface>,
    /// Interrupt surface facts.
    pub interrupts: Option<RawInterruptSurface>,
    /// Synchronization surface facts.
    pub synchronization: Option<RawSynchronizationSurface>,
    /// Timed-delay intrinsic facts.
    pub timed_delay: Option<RawTimedDelaySurface>,
    /// Diagnostic logging facts.
    pub logging: Option<RawLoggingSurface>,
    /// PM gate facts.
    pub pm_gate: Option<RawPmGateSurface>,
}

// ---------------------------------------------------------------------------
// Driver identity
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RawDriverIdentity {
    pub driver_name: String,
    pub compatible: Vec<String>,
    pub device_class: String,
}

impl From<&DriverIdentity> for RawDriverIdentity {
    fn from(id: &DriverIdentity) -> Self {
        Self {
            driver_name: id.driver_name.clone(),
            compatible: id.compatible.clone(),
            device_class: id.device_class.clone(),
        }
    }
}

// ---------------------------------------------------------------------------
// Register access surface
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RawRegisterAccessSurface {
    pub register_blocks: Vec<RawRegisterBlockDecl>,
    pub mmio_fields: Vec<RawMmioFieldDecl>,
    pub wrapper_call_sites: Vec<RawWrapperCallSite>,
    pub poll_call_sites: Vec<RawPollCallSite>,
    pub register_descriptors: Vec<RawRegisterDescriptorFact>,
}

impl From<&RegisterAccessSurface> for RawRegisterAccessSurface {
    fn from(s: &RegisterAccessSurface) -> Self {
        Self {
            register_blocks: s.register_blocks.iter().map(Into::into).collect(),
            mmio_fields: s.mmio_fields.iter().map(Into::into).collect(),
            wrapper_call_sites: s.wrapper_call_sites.iter().map(Into::into).collect(),
            poll_call_sites: s.poll_call_sites.iter().map(Into::into).collect(),
            register_descriptors: s.register_descriptors.iter().map(Into::into).collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RawRegisterBlockDecl {
    pub name: String,
    pub fields: Vec<RawRegisterFieldDecl>,
}

impl From<&RegisterBlockDecl> for RawRegisterBlockDecl {
    fn from(b: &RegisterBlockDecl) -> Self {
        Self {
            name: b.name.clone(),
            fields: b.fields.iter().map(Into::into).collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RawRegisterFieldDecl {
    pub name: String,
    pub access_width: Option<u32>,
    pub endian: Option<String>,
    pub side_effects: Option<String>,
    pub hw_updates: Option<String>,
}

impl From<&RegisterFieldDecl> for RawRegisterFieldDecl {
    fn from(f: &RegisterFieldDecl) -> Self {
        Self {
            name: f.name.clone(),
            access_width: f.access_width,
            endian: f.endian.clone(),
            side_effects: f.side_effects.clone(),
            hw_updates: f.hw_updates.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RawMmioFieldDecl {
    pub field_name: String,
    pub mmio_id: Option<String>,
    pub register_block_type: Option<String>,
}

impl From<&MmioFieldDecl> for RawMmioFieldDecl {
    fn from(f: &MmioFieldDecl) -> Self {
        Self {
            field_name: f.field_name.clone(),
            mmio_id: f.mmio_id.clone(),
            register_block_type: f.register_block_type.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RawWrapperCallSite {
    pub method: String,
    pub containing_fn: String,
}

impl From<&WrapperCallSite> for RawWrapperCallSite {
    fn from(s: &WrapperCallSite) -> Self {
        Self {
            method: match s.method {
                WrapperMethod::Read => "read",
                WrapperMethod::Write => "write",
                WrapperMethod::Modify => "modify",
                WrapperMethod::Set => "set",
            }
            .to_string(),
            containing_fn: s.containing_fn.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RawPollCallSite {
    pub containing_fn: String,
}

impl From<&PollCallSite> for RawPollCallSite {
    fn from(s: &PollCallSite) -> Self {
        Self {
            containing_fn: s.containing_fn.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RawRegisterDescriptorFact {
    pub register_module: String,
    pub fields: Vec<RawRegisterDescriptorFieldFact>,
}

impl From<&RegisterDescriptorFact> for RawRegisterDescriptorFact {
    fn from(f: &RegisterDescriptorFact) -> Self {
        Self {
            register_module: f.register_module.clone(),
            fields: f.fields.iter().map(Into::into).collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RawRegisterDescriptorFieldFact {
    pub field_name: String,
    pub offset: u32,
    pub width: u32,
    pub named_values: Vec<RawRegisterNamedValueFact>,
}

impl From<&RegisterDescriptorFieldFact> for RawRegisterDescriptorFieldFact {
    fn from(f: &RegisterDescriptorFieldFact) -> Self {
        Self {
            field_name: f.field_name.clone(),
            offset: f.offset,
            width: f.width,
            named_values: f.named_values.iter().map(Into::into).collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RawRegisterNamedValueFact {
    pub name: String,
    pub value: u32,
}

impl From<&RegisterNamedValueFact> for RawRegisterNamedValueFact {
    fn from(v: &RegisterNamedValueFact) -> Self {
        Self {
            name: v.name.clone(),
            value: v.value,
        }
    }
}

// ---------------------------------------------------------------------------
// Trait callback surface
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RawTraitSurface {
    pub lifecycle_impls: Vec<RawTraitImpl>,
    pub serial_impls: Vec<RawTraitImpl>,
    pub serial_console_impls: Vec<RawTraitImpl>,
    pub power_management_impls: Vec<RawTraitImpl>,
}

impl From<&TraitSurface> for RawTraitSurface {
    fn from(s: &TraitSurface) -> Self {
        Self {
            lifecycle_impls: s
                .lifecycle_impls
                .iter()
                .map(RawTraitImpl::from_lifecycle)
                .collect(),
            serial_impls: s
                .serial_impls
                .iter()
                .map(RawTraitImpl::from_serial)
                .collect(),
            serial_console_impls: s
                .serial_console_impls
                .iter()
                .map(RawTraitImpl::from_serial_console)
                .collect(),
            power_management_impls: s
                .power_management_impls
                .iter()
                .map(RawTraitImpl::from_pm)
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RawTraitImpl {
    pub driver_type: String,
    pub context: String,
}

impl RawTraitImpl {
    fn from_lifecycle(i: &LifecycleImpl) -> Self {
        Self {
            driver_type: i.driver_type.clone(),
            context: callback_context_label(&i.context),
        }
    }

    fn from_serial(i: &SerialImpl) -> Self {
        Self {
            driver_type: i.driver_type.clone(),
            context: callback_context_label(&i.context),
        }
    }

    fn from_serial_console(i: &SerialConsoleImpl) -> Self {
        Self {
            driver_type: i.driver_type.clone(),
            context: callback_context_label(&i.context),
        }
    }

    fn from_pm(i: &PowerManagementImpl) -> Self {
        Self {
            driver_type: i.driver_type.clone(),
            context: callback_context_label(&i.context),
        }
    }
}

// ---------------------------------------------------------------------------
// Firmware property surface
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RawPropertySurface {
    pub fields: Vec<RawPropertyField>,
    pub lints: Vec<RawPropertyLint>,
}

impl From<&PropertySurface> for RawPropertySurface {
    fn from(s: &PropertySurface) -> Self {
        Self {
            fields: s.fields.iter().map(Into::into).collect(),
            lints: s.lints.iter().map(Into::into).collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RawPropertyField {
    pub driver_type: String,
    pub field_name: String,
    pub field_type: String,
    pub property_id: String,
    pub default_literal: Option<String>,
    pub vendor_prefix: Option<String>,
    pub required: bool,
}

impl From<&PropertyField> for RawPropertyField {
    fn from(f: &PropertyField) -> Self {
        Self {
            driver_type: f.driver_type.clone(),
            field_name: f.field_name.clone(),
            field_type: f.field_type.clone(),
            property_id: f.property_id.clone(),
            default_literal: f.default_literal.clone(),
            vendor_prefix: f.vendor_prefix.clone(),
            required: f.required,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind")]
pub enum RawPropertyLint {
    RequiredWithDefault {
        driver_type: String,
        field_name: String,
        property_id: String,
    },
}

impl From<&PropertyLint> for RawPropertyLint {
    fn from(l: &PropertyLint) -> Self {
        match l {
            PropertyLint::RequiredWithDefault {
                driver_type,
                field_name,
                property_id,
            } => Self::RequiredWithDefault {
                driver_type: driver_type.clone(),
                field_name: field_name.clone(),
                property_id: property_id.clone(),
            },
        }
    }
}

// ---------------------------------------------------------------------------
// Clock surface
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RawClockSurface {
    pub fields: Vec<RawClockField>,
    pub call_sites: Vec<RawClockCallSite>,
}

impl From<&ClockSurface> for RawClockSurface {
    fn from(s: &ClockSurface) -> Self {
        Self {
            fields: s.fields.iter().map(Into::into).collect(),
            call_sites: s.call_sites.iter().map(Into::into).collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RawClockField {
    pub field_name: String,
    pub clock_id: String,
    pub required: bool,
}

impl From<&ClockField> for RawClockField {
    fn from(f: &ClockField) -> Self {
        Self {
            field_name: f.field_name.clone(),
            clock_id: f.clock_id.clone(),
            required: f.required,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RawClockCallSite {
    pub method: String,
    pub containing_fn: String,
}

impl From<&ClockCallSite> for RawClockCallSite {
    fn from(s: &ClockCallSite) -> Self {
        Self {
            method: match s.method {
                ClockMethod::Enable => "enable",
                ClockMethod::Disable => "disable",
                ClockMethod::GetRate => "get_rate",
            }
            .to_string(),
            containing_fn: s.containing_fn.clone(),
        }
    }
}

// ---------------------------------------------------------------------------
// Reset surface
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RawResetSurface {
    pub fields: Vec<RawResetField>,
    pub call_sites: Vec<RawResetCallSite>,
}

impl From<&ResetSurface> for RawResetSurface {
    fn from(s: &ResetSurface) -> Self {
        Self {
            fields: s.fields.iter().map(Into::into).collect(),
            call_sites: s.call_sites.iter().map(Into::into).collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RawResetField {
    pub field_name: String,
    pub required: bool,
}

impl From<&ResetField> for RawResetField {
    fn from(f: &ResetField) -> Self {
        Self {
            field_name: f.field_name.clone(),
            required: f.required,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RawResetCallSite {
    pub method: String,
    pub containing_fn: String,
}

impl From<&ResetCallSite> for RawResetCallSite {
    fn from(s: &ResetCallSite) -> Self {
        Self {
            method: match s.method {
                ResetMethod::Assert => "assert",
                ResetMethod::Deassert => "deassert",
                ResetMethod::Toggle => "toggle",
            }
            .to_string(),
            containing_fn: s.containing_fn.clone(),
        }
    }
}

// ---------------------------------------------------------------------------
// PinCtrl surface
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RawPinCtrlSurface {
    pub fields: Vec<RawPinCtrlField>,
    pub call_sites: Vec<RawPinCtrlCallSite>,
}

impl From<&PinCtrlSurface> for RawPinCtrlSurface {
    fn from(s: &PinCtrlSurface) -> Self {
        Self {
            fields: s.fields.iter().map(Into::into).collect(),
            call_sites: s.call_sites.iter().map(Into::into).collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RawPinCtrlField {
    pub field_name: String,
    pub required: bool,
    pub pinctrl_id: Option<String>,
}

impl From<&PinCtrlField> for RawPinCtrlField {
    fn from(f: &PinCtrlField) -> Self {
        Self {
            field_name: f.field_name.clone(),
            required: f.required,
            pinctrl_id: f.pinctrl_id.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RawPinCtrlCallSite {
    pub state_type: String,
    pub containing_fn: String,
}

impl From<&PinCtrlCallSite> for RawPinCtrlCallSite {
    fn from(s: &PinCtrlCallSite) -> Self {
        Self {
            state_type: s.state_type.clone(),
            containing_fn: s.containing_fn.clone(),
        }
    }
}

// ---------------------------------------------------------------------------
// Interrupt surface
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RawInterruptSurface {
    pub resources: Vec<RawInterruptResourceDecl>,
    pub isr_bindings: Vec<RawIsrBinding>,
}

impl From<&InterruptSurface> for RawInterruptSurface {
    fn from(s: &InterruptSurface) -> Self {
        Self {
            resources: s.resources.iter().map(Into::into).collect(),
            isr_bindings: s.isr_bindings.iter().map(Into::into).collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RawInterruptResourceDecl {
    pub field_name: String,
    pub interrupt_id: String,
    pub shape: String,
    pub layer2: RawInterruptLayer2Properties,
}

impl From<&InterruptResourceDecl> for RawInterruptResourceDecl {
    fn from(r: &InterruptResourceDecl) -> Self {
        Self {
            field_name: r.field_name.clone(),
            interrupt_id: r.interrupt_id.clone(),
            shape: match r.shape {
                InterruptFieldShape::Single => "single".to_string(),
                InterruptFieldShape::Optional => "optional".to_string(),
                InterruptFieldShape::Array(n) => format!("array({n})"),
            },
            layer2: (&r.layer2).into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RawInterruptLayer2Properties {
    pub trigger_type: Option<String>,
    pub trigger_polarity: Option<String>,
    pub priority: Option<String>,
    pub shared: Option<bool>,
}

impl From<&InterruptLayer2Properties> for RawInterruptLayer2Properties {
    fn from(p: &InterruptLayer2Properties) -> Self {
        Self {
            trigger_type: p.trigger_type.clone(),
            trigger_polarity: p.trigger_polarity.clone(),
            priority: p.priority.clone(),
            shared: p.shared,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RawIsrBinding {
    pub handler_name: String,
    pub bind_target: String,
    pub has_index_param: bool,
    pub context: String,
}

impl From<&IsrBinding> for RawIsrBinding {
    fn from(b: &IsrBinding) -> Self {
        Self {
            handler_name: b.handler_name.clone(),
            bind_target: match &b.bind_target {
                IsrBindTarget::Auto => "auto".to_string(),
                IsrBindTarget::ExplicitField(f) => f.clone(),
            },
            has_index_param: b.has_index_param,
            context: callback_context_label(&b.context),
        }
    }
}

// ---------------------------------------------------------------------------
// Synchronization surface
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RawSynchronizationSurface {
    pub sync_fields: Vec<RawSyncFieldDecl>,
    pub lock_operations: Vec<RawLockOperation>,
    pub completion_operations: Vec<RawCompletionOperation>,
    pub guard_scopes: Vec<RawGuardScope>,
}

impl From<&SynchronizationSurface> for RawSynchronizationSurface {
    fn from(s: &SynchronizationSurface) -> Self {
        Self {
            sync_fields: s.sync_fields.iter().map(Into::into).collect(),
            lock_operations: s.lock_operations.iter().map(Into::into).collect(),
            completion_operations: s.completion_operations.iter().map(Into::into).collect(),
            guard_scopes: s.guard_scopes.iter().map(Into::into).collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RawSyncFieldDecl {
    pub field_name: String,
    pub kind: String,
    pub payload_type: Option<String>,
}

impl From<&SyncFieldDecl> for RawSyncFieldDecl {
    fn from(f: &SyncFieldDecl) -> Self {
        Self {
            field_name: f.field_name.clone(),
            kind: sync_kind_label(&f.kind),
            payload_type: f.payload_type.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RawLockOperation {
    pub lock_kind: String,
    pub containing_fn: String,
}

impl From<&LockOperation> for RawLockOperation {
    fn from(o: &LockOperation) -> Self {
        Self {
            lock_kind: sync_kind_label(&o.lock_kind),
            containing_fn: o.containing_fn.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RawCompletionOperation {
    pub method: String,
    pub containing_fn: String,
}

impl From<&CompletionOperation> for RawCompletionOperation {
    fn from(o: &CompletionOperation) -> Self {
        Self {
            method: match o.method {
                CompletionMethod::Wait => "wait",
                CompletionMethod::Signal => "signal",
                CompletionMethod::Reset => "reset",
            }
            .to_string(),
            containing_fn: o.containing_fn.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RawGuardScope {
    pub lock_kind: String,
    pub containing_fn: String,
}

impl From<&GuardScope> for RawGuardScope {
    fn from(g: &GuardScope) -> Self {
        Self {
            lock_kind: sync_kind_label(&g.lock_kind),
            containing_fn: g.containing_fn.clone(),
        }
    }
}

// ---------------------------------------------------------------------------
// Timed-delay surface
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RawTimedDelaySurface {
    pub call_sites: Vec<RawTimedDelayCallSite>,
    pub duration_ctors: Vec<RawDurationCtorSite>,
}

impl From<&TimedDelaySurface> for RawTimedDelaySurface {
    fn from(s: &TimedDelaySurface) -> Self {
        Self {
            call_sites: s.call_sites.iter().map(Into::into).collect(),
            duration_ctors: s.duration_ctors.iter().map(Into::into).collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RawTimedDelayCallSite {
    pub intrinsic: String,
    pub path_kind: String,
    pub containing_fn: String,
}

impl From<&TimedDelayCallSite> for RawTimedDelayCallSite {
    fn from(s: &TimedDelayCallSite) -> Self {
        Self {
            intrinsic: match s.intrinsic {
                DelayIntrinsic::BusyWait => "busy_wait",
                DelayIntrinsic::Sleep => "sleep",
            }
            .to_string(),
            path_kind: match s.path_kind {
                DelayPathKind::Canonical => "canonical",
                DelayPathKind::Compatibility => "compatibility",
            }
            .to_string(),
            containing_fn: s.containing_fn.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RawDurationCtorSite {
    pub ctor: String,
    pub containing_fn: String,
}

impl From<&DurationCtorSite> for RawDurationCtorSite {
    fn from(s: &DurationCtorSite) -> Self {
        Self {
            ctor: match s.ctor {
                DurationCtor::FromUs => "from_us",
                DurationCtor::FromMs => "from_ms",
                DurationCtor::FromS => "from_s",
            }
            .to_string(),
            containing_fn: s.containing_fn.clone(),
        }
    }
}

// ---------------------------------------------------------------------------
// Logging surface
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RawLoggingSurface {
    pub call_sites: Vec<RawLogCallSite>,
}

impl From<&LoggingSurface> for RawLoggingSurface {
    fn from(s: &LoggingSurface) -> Self {
        Self {
            call_sites: s.call_sites.iter().map(Into::into).collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RawLogCallSite {
    pub level: String,
    pub containing_fn: String,
    pub canonical: bool,
    pub template: String,
    pub arg_count: usize,
    pub arg_kinds: Vec<String>,
}

impl From<&LogCallSite> for RawLogCallSite {
    fn from(s: &LogCallSite) -> Self {
        Self {
            level: match s.level {
                LogLevel::Error => "error",
                LogLevel::Warn => "warn",
                LogLevel::Info => "info",
                LogLevel::Debug => "debug",
            }
            .to_string(),
            containing_fn: s.containing_fn.clone(),
            canonical: s.canonical,
            template: s.template.clone(),
            arg_count: s.arg_count,
            arg_kinds: s.arg_kinds.iter().map(log_arg_kind_label).collect(),
        }
    }
}

// ---------------------------------------------------------------------------
// PM gate surface
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RawPmGateSurface {
    pub gates: Vec<RawPmGate>,
}

impl From<&PmGateSurface> for RawPmGateSurface {
    fn from(s: &PmGateSurface) -> Self {
        Self {
            gates: s.gates.iter().map(Into::into).collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RawPmGate {
    pub driver_type: String,
    pub properties: RawPmGateProperties,
    pub has_pm_impl: bool,
}

impl From<&PmGate> for RawPmGate {
    fn from(g: &PmGate) -> Self {
        Self {
            driver_type: g.driver_type.clone(),
            properties: (&g.properties).into(),
            has_pm_impl: g.has_pm_impl,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RawPmGateProperties {
    pub idle: bool,
    pub wakeup_capable: bool,
}

impl From<&PmGateProperties> for RawPmGateProperties {
    fn from(p: &PmGateProperties) -> Self {
        Self {
            idle: p.idle,
            wakeup_capable: p.wakeup_capable,
        }
    }
}

// ---------------------------------------------------------------------------
// Shared label helpers
// ---------------------------------------------------------------------------

fn callback_context_label(ctx: &CallbackContext) -> String {
    ctx.short_label().to_string()
}

fn sync_kind_label(kind: &SyncFieldKind) -> String {
    match kind {
        SyncFieldKind::Mutex => "mutex",
        SyncFieldKind::SpinLock => "spinlock",
        SyncFieldKind::IrqSpinLock => "irq_spinlock",
        SyncFieldKind::Completion => "completion",
    }
    .to_string()
}

fn log_arg_kind_label(kind: &LogArgKind) -> String {
    match kind {
        LogArgKind::U8 => "u8",
        LogArgKind::U16 => "u16",
        LogArgKind::U32 => "u32",
        LogArgKind::U64 => "u64",
        LogArgKind::Usize => "usize",
        LogArgKind::I8 => "i8",
        LogArgKind::I16 => "i16",
        LogArgKind::I32 => "i32",
        LogArgKind::I64 => "i64",
        LogArgKind::Isize => "isize",
        LogArgKind::Bool => "bool",
        LogArgKind::Str => "str",
        LogArgKind::Ptr => "ptr",
    }
    .to_string()
}
