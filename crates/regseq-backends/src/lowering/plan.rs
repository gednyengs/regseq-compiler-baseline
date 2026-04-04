//! Portable driver lowering plan types.
//!
//! `DriverLoweringPlan` is the compiler-owned semantic realization plan
//! that captures all driver obligations in a backend-neutral form. OS-specific
//! backend planners consume this plan rather than re-deriving facts from the
//! raw `AnalysisInput`.
//!
//! No Linux, Zephyr, or other OS-specific types belong here.

use regseq_model::declaration::callbacks::{CallbackContext, CallbackFamily};
use regseq_model::declaration::driver::DeviceClass;
use regseq_model::declaration::operations::{DelayIntrinsic, DelayPathKind, LogLevel};
use regseq_model::declaration::provenance::Provenance;
use regseq_model::declaration::registers::WrapperMethod;
use regseq_model::declaration::synchronization::SyncFieldKind;

use super::behavior::BehaviorPlan;

// ---------------------------------------------------------------------------
// Top-level plan
// ---------------------------------------------------------------------------

/// Portable driver lowering plan.
///
/// This is the compiler-owned semantic realization plan that all OS-specific
/// backend planners consume. It captures driver obligations that are
/// meaningful across backends, so that backend planners do not have to
/// re-derive core driver semantics from the raw models.
#[derive(Debug, Clone)]
pub struct DriverLoweringPlan {
    /// Driver identity and provenance.
    pub identity: DriverIdentity,
    /// Callback/control-flow inventory.
    pub callbacks: CallbackInventory,
    /// Resource obligations across all families.
    pub resources: ResourceObligations,
    /// Ordered resource acquisition steps for probe/init.
    pub acquisition_order: Vec<AcquisitionStep>,
    /// Ordered resource teardown steps for remove/deinit.
    pub teardown_order: Vec<TeardownStep>,
    /// Device-class role obligations.
    pub device_class_roles: DeviceClassRoles,
    /// Register-access and ordering obligations.
    pub register_obligations: RegisterObligations,
    /// Synchronization obligations.
    pub synchronization: SynchronizationObligations,
    /// Callback-body lowering actions in a backend-usable structured form.
    pub callback_actions: Vec<CallbackBodyPlan>,
    /// Structured per-function behavioral plan for emission.
    ///
    /// Captures the authored body-level behavior scoped to individual
    /// functions so that backend emission does not invent device-specific
    /// callback logic.
    pub behavior: BehaviorPlan,
}

// ---------------------------------------------------------------------------
// Driver identity
// ---------------------------------------------------------------------------

/// Driver identity and provenance extracted from the declaration model.
#[derive(Debug, Clone)]
pub struct DriverIdentity {
    /// Driver name from the `#[rseq::driver]` attribute.
    pub driver_name: String,
    /// Compatible strings for device matching.
    pub compatible: Vec<String>,
    /// Device class (e.g. SerialUart, Network).
    pub device_class: DeviceClass,
    /// Source modules that contributed to this driver specification.
    pub source_modules: Vec<String>,
    /// Provenance of the driver declaration.
    pub provenance: Provenance,
}

// ---------------------------------------------------------------------------
// Callback inventory
// ---------------------------------------------------------------------------

/// Callback/control-flow inventory across all families.
#[derive(Debug, Clone)]
pub struct CallbackInventory {
    /// Lifecycle callbacks (init, deinit).
    pub lifecycle: Vec<CallbackEntry>,
    /// Serial device-class callbacks.
    pub serial: Vec<CallbackEntry>,
    /// Serial console callbacks.
    pub serial_console: Vec<CallbackEntry>,
    /// Power management callbacks.
    pub power_management: Vec<CallbackEntry>,
}

/// A single callback entry in the inventory.
#[derive(Debug, Clone)]
pub struct CallbackEntry {
    /// Driver type that implements this callback.
    pub driver_type: String,
    /// Callback execution context (Framework, SubsystemSerialized, etc.).
    pub context: CallbackContext,
    /// Which callback family this belongs to.
    pub family: CallbackFamily,
    /// Provenance from the declaration.
    pub provenance: Provenance,
}

// ---------------------------------------------------------------------------
// Resource obligations
// ---------------------------------------------------------------------------

/// All resource obligations grouped by family.
#[derive(Debug, Clone)]
pub struct ResourceObligations {
    /// MMIO resource obligations.
    pub mmio: Vec<MmioObligation>,
    /// Interrupt resource obligations.
    pub interrupts: Vec<InterruptObligation>,
    /// Firmware property obligations.
    pub properties: Vec<PropertyObligation>,
    /// Clock resource obligations.
    pub clocks: Vec<ClockObligation>,
    /// Reset resource obligations.
    pub resets: Vec<ResetObligation>,
    /// Pin controller obligations.
    pub pinctrl: Vec<PinCtrlObligation>,
    /// Explicit portable bus identity carried into lowering.
    pub bus_identity: BusIdentity,
}

/// MMIO resource obligation.
#[derive(Debug, Clone)]
pub struct MmioObligation {
    /// Field name on the driver struct.
    pub field_name: String,
    /// MMIO identifier used for join matching.
    pub mmio_id: String,
    /// Associated register block type name, if any.
    pub register_block_type: Option<String>,
    /// Resource acquisition policy.
    pub policy: ResourcePolicy,
    /// Provenance.
    pub provenance: Provenance,
}

/// Interrupt resource obligation.
#[derive(Debug, Clone)]
pub struct InterruptObligation {
    /// Field name on the driver struct.
    pub field_name: String,
    /// Interrupt identifier.
    pub interrupt_id: String,
    /// Trigger type from the binding (e.g. "level", "edge").
    pub trigger_type: Option<String>,
    /// Whether the interrupt is shared.
    pub shared: Option<bool>,
    /// Resource acquisition policy.
    pub policy: ResourcePolicy,
    /// Provenance.
    pub provenance: Provenance,
}

/// Firmware property obligation.
#[derive(Debug, Clone)]
pub struct PropertyObligation {
    /// Property identifier.
    pub property_id: String,
    /// Field name on the driver struct.
    pub field_name: String,
    /// Property Rust type as declared.
    pub field_type: String,
    /// Whether the property is required.
    pub required: bool,
    /// Default literal from the declaration, if any.
    pub default_literal: Option<String>,
    /// Resource acquisition policy.
    pub policy: ResourcePolicy,
    /// Provenance.
    pub provenance: Provenance,
}

/// Clock resource obligation.
#[derive(Debug, Clone)]
pub struct ClockObligation {
    /// Field name on the driver struct.
    pub field_name: String,
    /// Clock identifier.
    pub clock_id: String,
    /// Whether the clock is required.
    pub required: bool,
    /// Default frequency in Hz from the deployment binding, if supplied.
    pub default_frequency: Option<i64>,
    /// Resource acquisition policy.
    pub policy: ResourcePolicy,
    /// Provenance.
    pub provenance: Provenance,
}

/// Reset resource obligation.
#[derive(Debug, Clone)]
pub struct ResetObligation {
    /// Field name on the driver struct.
    pub field_name: String,
    /// Whether the reset is required.
    pub required: bool,
    /// Reset line identifier from the deployment binding, if present.
    pub reset_line_id: Option<String>,
    /// Whether the reset line is shared, from the deployment binding.
    pub reset_shared: Option<bool>,
    /// Resource acquisition policy.
    pub policy: ResourcePolicy,
    /// Provenance.
    pub provenance: Provenance,
}

/// Pin controller obligation.
#[derive(Debug, Clone)]
pub struct PinCtrlObligation {
    /// Field name on the driver struct.
    pub field_name: String,
    /// Whether the pinctrl is required.
    pub required: bool,
    /// Pin controller identifier from the declaration, if present.
    pub pinctrl_id: Option<String>,
    /// Available pin states from the deployment binding (e.g. `["default", "sleep"]`).
    pub pin_states: Option<Vec<String>>,
    /// Resource acquisition policy.
    pub policy: ResourcePolicy,
    /// Provenance.
    pub provenance: Provenance,
}

/// Explicit portable bus identity carried through lowering.
#[derive(Debug, Clone)]
pub struct BusIdentity {
    /// Explicit bus type.
    pub bus_type: BusType,
    /// Compatible strings declared alongside the explicit bus identity.
    pub compatible: Vec<String>,
    /// AMBA peripheral ID, if this is an AMBA bus device.
    pub amba_id: Option<u32>,
    /// AMBA revision, if this is an AMBA bus device.
    pub amba_revision: Option<u32>,
}

/// Inferred bus type for the device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BusType {
    /// Platform bus (e.g. devicetree `platform_driver`).
    Platform,
    /// AMBA bus (ARM PrimeCell peripherals).
    Amba,
    /// PCI/PCIe bus.
    Pci,
    /// Other or unrecognized bus type.
    Other(String),
}

/// Optional-resource acquisition policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResourcePolicy {
    /// Resource is required; failure to acquire is a fatal error.
    Required,
    /// Resource is optional; if absent, use a fallback value.
    OptionalWithFallback,
    /// Resource is optional; if absent, skip entirely.
    OptionalSkip,
}

// ---------------------------------------------------------------------------
// Acquisition and teardown ordering
// ---------------------------------------------------------------------------

/// A single resource acquisition step in probe/init ordering.
#[derive(Debug, Clone)]
pub struct AcquisitionStep {
    /// Position in the acquisition sequence (0-based).
    pub order: usize,
    /// Which resource family this step acquires.
    pub resource_kind: ResourceKind,
    /// Identifier of the specific resource.
    pub resource_id: String,
    /// Acquisition policy for this step.
    pub policy: ResourcePolicy,
}

/// A single resource teardown step in remove/deinit ordering.
#[derive(Debug, Clone)]
pub struct TeardownStep {
    /// Position in the teardown sequence (0-based).
    pub order: usize,
    /// Which resource family this step releases.
    pub resource_kind: ResourceKind,
    /// Identifier of the specific resource.
    pub resource_id: String,
}

/// Resource family discriminant for ordering steps.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResourceKind {
    /// MMIO mapping.
    Mmio,
    /// Interrupt line.
    Interrupt,
    /// Clock.
    Clock,
    /// Reset line.
    Reset,
    /// Pin controller state.
    PinCtrl,
    /// Firmware property read.
    Property,
}

// ---------------------------------------------------------------------------
// Device-class roles
// ---------------------------------------------------------------------------

/// Device-class role obligations that the backend must realize.
#[derive(Debug, Clone)]
pub struct DeviceClassRoles {
    /// Device class from the declaration.
    pub device_class: DeviceClass,
    /// Which serial operations are realized (method names).
    pub serial_ops_realized: Vec<String>,
    /// Whether lifecycle callbacks are present.
    pub has_lifecycle: bool,
    /// Whether power management callbacks are present.
    pub has_power_management: bool,
}

// ---------------------------------------------------------------------------
// Register obligations
// ---------------------------------------------------------------------------

/// Register-access and ordering obligations.
#[derive(Debug, Clone)]
pub struct RegisterObligations {
    /// Register block obligations (each block the driver uses).
    pub blocks: Vec<RegisterBlockObligation>,
    /// Register access sites across all callbacks.
    pub access_sites: Vec<RegisterAccessObligation>,
    /// Poll sites across all callbacks.
    pub poll_sites: Vec<PollObligation>,
    /// Register descriptor bitfield definitions.
    pub descriptors: Vec<RegisterDescriptorInfo>,
}

/// A register block the driver declares and uses.
#[derive(Debug, Clone)]
pub struct RegisterBlockObligation {
    /// Block type name.
    pub name: String,
    /// Register field count in this block.
    pub field_count: usize,
    /// Structural register field metadata in declared order.
    ///
    /// Each entry carries the field name, access width, and a computed byte
    /// offset derived from the declared layout order. This is enough for
    /// backend emission to produce per-register byte offsets without a
    /// hardcoded offset table.
    pub register_fields: Vec<RegisterFieldInfo>,
    /// Provenance.
    pub provenance: Provenance,
}

/// Structural metadata for a single register field within a block.
#[derive(Debug, Clone)]
pub struct RegisterFieldInfo {
    /// Field name (e.g. "dr", "fr").
    pub name: String,
    /// Access width in bits, if declared (e.g. 32).
    pub access_width: Option<u32>,
    /// Byte offset within the register block, computed from the declared
    /// field order and access widths.
    pub byte_offset: u32,
}

/// Bitfield descriptor for a single register module.
#[derive(Debug, Clone)]
pub struct RegisterDescriptorInfo {
    /// Register module name (e.g. "DR_REG", "FR_REG").
    pub register_name: String,
    /// Bitfield definitions within this register.
    pub bit_fields: Vec<BitFieldInfo>,
}

/// A single bitfield within a register descriptor.
#[derive(Debug, Clone)]
pub struct BitFieldInfo {
    /// Field name (e.g. "DATA", "TXFF").
    pub name: String,
    /// Bit offset within the register.
    pub offset: u32,
    /// Width in bits.
    pub width: u32,
    /// Named values (enum variants) for this bitfield, if any.
    pub named_values: Vec<NamedValueInfo>,
}

/// A named value (enum variant) within a bitfield.
#[derive(Debug, Clone)]
pub struct NamedValueInfo {
    /// Symbolic name (e.g. "EIGHT_BITS", "ENABLED").
    pub name: String,
    /// Numeric value.
    pub value: u32,
}

/// A register access site that needs lowering.
#[derive(Debug, Clone)]
pub struct RegisterAccessObligation {
    /// Access method (Read, Write, Modify, etc.).
    pub method: WrapperMethod,
    /// Function containing this access.
    pub containing_fn: String,
    /// Provenance.
    pub provenance: Provenance,
}

/// A poll site that needs lowering.
#[derive(Debug, Clone)]
pub struct PollObligation {
    /// Function containing this poll.
    pub containing_fn: String,
    /// Provenance.
    pub provenance: Provenance,
}

// ---------------------------------------------------------------------------
// Synchronization obligations
// ---------------------------------------------------------------------------

/// Synchronization obligations across the driver.
#[derive(Debug, Clone)]
pub struct SynchronizationObligations {
    /// Synchronization primitive fields declared on the driver.
    pub fields: Vec<SyncFieldObligation>,
    /// Lock scope obligations.
    pub lock_scopes: Vec<LockScopeObligation>,
    /// Completion usage obligations.
    pub completion_uses: Vec<CompletionObligation>,
}

/// A synchronization field declared on the driver struct.
#[derive(Debug, Clone)]
pub struct SyncFieldObligation {
    /// Field name on the driver struct.
    pub field_name: String,
    /// Synchronization primitive kind.
    pub kind: SyncFieldKind,
    /// Payload type, if applicable.
    pub payload_type: Option<String>,
    /// Provenance.
    pub provenance: Provenance,
}

/// A lock scope obligation: a function that acquires a lock.
#[derive(Debug, Clone)]
pub struct LockScopeObligation {
    /// Lock primitive kind.
    pub lock_kind: SyncFieldKind,
    /// Function containing this lock scope.
    pub containing_fn: String,
    /// Provenance.
    pub provenance: Provenance,
}

/// A completion usage obligation.
#[derive(Debug, Clone)]
pub struct CompletionObligation {
    /// Completion method (Wait, Signal, Reset).
    pub method: String,
    /// Function containing this completion use.
    pub containing_fn: String,
    /// Provenance.
    pub provenance: Provenance,
}

// ---------------------------------------------------------------------------
// Callback-body lowering actions
// ---------------------------------------------------------------------------

/// Structured plan for a single callback body.
#[derive(Debug, Clone)]
pub struct CallbackBodyPlan {
    /// Callback family this body belongs to.
    pub family: CallbackFamily,
    /// Driver type implementing this callback.
    pub driver_type: String,
    /// Execution context.
    pub context: CallbackContext,
    /// Ordered lowering actions for this callback body.
    pub actions: Vec<LoweringAction>,
}

/// A single lowering action within a callback body.
///
/// These are backend-neutral semantic actions that the portable plan
/// captures so that OS-specific backend planners can translate them
/// into concrete API calls without re-deriving the action sequence.
#[derive(Debug, Clone)]
pub enum LoweringAction {
    /// Acquire a resource (MMIO mapping, IRQ request, clock enable, etc.).
    AcquireResource {
        /// Resource family.
        kind: ResourceKind,
        /// Resource identifier.
        resource_id: String,
        /// Acquisition policy.
        policy: ResourcePolicy,
    },
    /// Release a resource.
    ReleaseResource {
        /// Resource family.
        kind: ResourceKind,
        /// Resource identifier.
        resource_id: String,
    },
    /// Perform a register access.
    RegisterAccess {
        /// Access method.
        method: WrapperMethod,
        /// Containing function for provenance.
        containing_fn: String,
    },
    /// Acquire a lock.
    LockAcquire {
        /// Lock primitive kind.
        kind: SyncFieldKind,
        /// Containing function.
        containing_fn: String,
    },
    /// Release a lock.
    LockRelease {
        /// Lock primitive kind.
        kind: SyncFieldKind,
        /// Containing function.
        containing_fn: String,
    },
    /// Perform a completion operation.
    CompletionOp {
        /// Method name (wait, signal, reset).
        method: String,
        /// Containing function.
        containing_fn: String,
    },
    /// Read a firmware property.
    PropertyRead {
        /// Property identifier.
        property_id: String,
        /// Whether a default fallback exists.
        has_default: bool,
    },
    /// Emit a log message.
    LogEmit {
        /// Log level.
        level: LogLevel,
        /// Message template.
        template: String,
    },
    /// Execute a timed delay.
    TimedDelay {
        /// Delay intrinsic type.
        intrinsic: DelayIntrinsic,
        /// Path kind (canonical or compatibility).
        path_kind: DelayPathKind,
    },
    /// Register an interrupt handler.
    InterruptRegister {
        /// Interrupt identifier.
        interrupt_id: String,
    },
    /// Poll a register condition.
    PollRegister {
        /// Containing function.
        containing_fn: String,
    },
}
