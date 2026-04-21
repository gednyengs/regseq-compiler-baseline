//! Linux-specific backend plan types.
//!
//! `LinuxBackendPlan` captures Linux realization decisions derived from
//! the portable `DriverLoweringPlan`. This includes Linux driver
//! registration, serial-core integration, probe/remove orchestration,
//! resource-acquisition sequencing, and emitted-file layout.
//!
//! No portable lowering types are redefined here; they are consumed
//! from the `lowering::plan` module.

use regseq_model::compat::{FunctionRole, SemanticModel};

// ---------------------------------------------------------------------------
// Top-level Linux plan
// ---------------------------------------------------------------------------

/// Linux-specific backend plan.
///
/// Derived from `DriverLoweringPlan` by the Linux planner. Contains all
/// Linux-specific realization decisions needed to emit a buildable
/// out-of-tree kernel module.
#[derive(Debug, Clone)]
pub struct LinuxBackendPlan {
    /// Linux driver registration shape.
    pub registration: LinuxRegistrationShape,
    /// Serial-core realization, if the driver is a serial device.
    pub serial_core: Option<LinuxSerialCoreRealization>,
    /// Probe function orchestration.
    pub probe: LinuxProbeOrchestration,
    /// Remove function orchestration.
    pub remove: LinuxRemoveOrchestration,
    /// Emitted-file layout decisions.
    pub artifact_layout: LinuxArtifactLayout,
    /// Register layout for register-helper emission.
    pub register_layout: LinuxRegisterLayout,
    /// Per-property realization details for emission.
    pub property_realization: Vec<LinuxPropertyRealization>,
    /// Mapping from portable function behaviors to Linux callbacks.
    pub behavior_mapping: LinuxBehaviorMapping,
    /// uart_ops realization entries for each Linux callback.
    pub uart_ops_realization: Vec<LinuxUartOpsEntry>,
    /// Transitional semantic-model handle.
    ///
    /// Carries the projected [`SemanticModel`] that the Linux emitter
    /// still string-formats from. After `regseq-lowering-10` trimmed
    /// the `behavior` field off [`crate::lowering::plan::DriverLoweringPlan`],
    /// the Linux plan temporarily owns this behavioral IR until
    /// `regseq-linux-backend-04` / `-05` retarget the emitters to the
    /// CFG `ir::Body` handles carried by
    /// [`crate::linux::body_index::FunctionBodyIndex`].
    pub behavior: SemanticModel,
    /// Raw driver-identity name from the declaration.
    ///
    /// Distinct from `registration.module_name` (which is the
    /// snake-cased C identifier). Used for generated-file banners and
    /// `MODULE_DESCRIPTION`.
    pub driver_name: String,
    /// Driver-state struct field layout for the shared header.
    ///
    /// Pre-computed by the Linux planner so the emitter does not
    /// re-derive Linux C types from portable resource obligations.
    pub shared_struct_fields: Vec<LinuxSharedStructField>,
    /// Whether the driver declares at least one optional clock.
    ///
    /// Controls whether the probe's `uartclk` initializer emits a
    /// ternary fallback (`d->clock ? clk_get_rate(...) : ...`) or
    /// calls `clk_get_rate` unconditionally. Pre-computed by the
    /// planner from the portable clock obligations.
    pub has_optional_clock: bool,
}

/// A single field in the generated driver-state struct.
///
/// The emitter renders one `type_name field_name;` declaration per
/// entry, with the documentation string attached as a field-level
/// comment. Ordering is significant: the planner emits MMIO, IRQ,
/// clock, and property fields in declaration order, followed by
/// `struct uart_port` when serial-core is realized.
#[derive(Debug, Clone)]
pub struct LinuxSharedStructField {
    /// Full C type spelling (e.g. `"void __iomem *"`, `"int"`,
    /// `"u32"`, `"struct uart_port"`).
    pub c_type: String,
    /// C identifier for the field.
    pub field_name: String,
    /// Documentation string for a field-level comment.
    pub doc: String,
}

// ---------------------------------------------------------------------------
// Registration shape
// ---------------------------------------------------------------------------

/// Linux platform_driver (or amba_driver) registration shape.
#[derive(Debug, Clone)]
pub struct LinuxRegistrationShape {
    /// Platform driver struct name in the generated C.
    pub driver_struct_name: String,
    /// Match table name.
    pub match_table_name: String,
    /// Compatible entries for the of_device_id table.
    pub compatible_entries: Vec<String>,
    /// Generated module name.
    pub module_name: String,
    /// Module license string.
    pub module_license: String,
    /// Whether this is an AMBA driver (vs platform_driver).
    pub is_amba: bool,
    /// AMBA peripheral ID for the match table (only set when `is_amba` is true).
    pub amba_id: Option<u32>,
    /// AMBA mask for the match table (only set when `is_amba` is true).
    pub amba_mask: Option<u32>,
}

// ---------------------------------------------------------------------------
// Serial-core realization
// ---------------------------------------------------------------------------

/// Linux serial-core (uart_driver / uart_ops) realization.
#[derive(Debug, Clone)]
pub struct LinuxSerialCoreRealization {
    /// uart_driver struct name.
    pub uart_driver_name: String,
    /// uart_ops struct name.
    pub uart_ops_name: String,
    /// Which uart_ops callbacks are realized.
    pub realized_ops: Vec<String>,
    /// Port type constant for Linux serial-core registration.
    pub port_type: LinuxPortType,
    /// TTY device name prefix for `uart_driver.dev_name`.
    ///
    /// This is an explicit Linux backend naming policy decision,
    /// not derived from device-specific identifiers.
    pub dev_name: String,
}

/// Typed Linux port-type representation.
///
/// Carries both the symbolic define name and the numeric value for
/// the `PORT_*` constant used in serial-core registration. The
/// numeric value is a deterministic backend allocation, not a
/// device-specific hardware fact.
#[derive(Debug, Clone)]
pub struct LinuxPortType {
    /// Symbolic define name (e.g., `"PORT_MY_DEVICE"`).
    pub define_name: String,
    /// Numeric value for the define.
    pub value: u32,
}

// ---------------------------------------------------------------------------
// Probe/remove orchestration
// ---------------------------------------------------------------------------

/// Linux probe function orchestration.
#[derive(Debug, Clone)]
pub struct LinuxProbeOrchestration {
    /// Ordered probe steps.
    pub steps: Vec<LinuxProbeStep>,
}

/// A single step in the Linux probe sequence.
#[derive(Debug, Clone)]
pub enum LinuxProbeStep {
    /// Map an MMIO region via devm_ioremap_resource.
    MapMmio {
        /// MMIO identifier.
        mmio_id: String,
    },
    /// Request an IRQ via devm_request_irq.
    RequestIrq {
        /// Interrupt identifier.
        interrupt_id: String,
    },
    /// Get and enable a clock via devm_clk_get / clk_prepare_enable.
    GetClock {
        /// Clock identifier.
        clock_id: String,
        /// Whether this clock is optional.
        optional: bool,
    },
    /// Deassert a reset line via devm_reset_control_get / reset_control_deassert.
    DeassertReset {
        /// Reset field name.
        field_name: String,
        /// Whether this reset is optional.
        optional: bool,
    },
    /// Select a pinctrl state via devm_pinctrl_get / pinctrl_select_state.
    SelectPinCtrl {
        /// Pinctrl field name.
        field_name: String,
        /// Whether this pinctrl is optional.
        optional: bool,
    },
    /// Read a firmware property via device_property_read_*.
    ReadProperty {
        /// Property identifier.
        property_id: String,
        /// Whether a default fallback exists.
        has_default: bool,
    },
    /// Register a UART port with the serial core.
    RegisterUartPort,
}

/// Linux remove function orchestration.
#[derive(Debug, Clone)]
pub struct LinuxRemoveOrchestration {
    /// Ordered remove steps.
    pub steps: Vec<LinuxRemoveStep>,
}

/// A single step in the Linux remove sequence.
#[derive(Debug, Clone)]
pub enum LinuxRemoveStep {
    /// Unregister the UART port.
    UnregisterUartPort,
    /// Free an IRQ.
    FreeIrq {
        /// Interrupt identifier.
        interrupt_id: String,
    },
    /// Disable and put a clock.
    PutClock {
        /// Clock identifier.
        clock_id: String,
    },
    /// Assert a reset line.
    AssertReset {
        /// Reset field name.
        field_name: String,
    },
    /// Unmap an MMIO region (typically handled by devm).
    UnmapMmio {
        /// MMIO identifier.
        mmio_id: String,
    },
}

// ---------------------------------------------------------------------------
// Artifact layout
// ---------------------------------------------------------------------------

/// Linux emitted-file layout decisions.
#[derive(Debug, Clone)]
pub struct LinuxArtifactLayout {
    /// Files to be emitted.
    pub emitted_files: Vec<LinuxEmittedFile>,
}

/// A single emitted file in the Linux artifact layout.
#[derive(Debug, Clone)]
pub struct LinuxEmittedFile {
    /// Output filename.
    pub filename: String,
    /// Role of this file in the generated module.
    pub role: LinuxFileRole,
}

/// Role of an emitted file in the Linux out-of-tree module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinuxFileRole {
    /// Main driver/module integration source file.
    DriverMain,
    /// Shared header with driver state struct and forward declarations.
    SharedHeader,
    /// Register helper definitions.
    RegisterHelpers,
    /// Makefile for out-of-tree build.
    Makefile,
    /// Kbuild fragment.
    KbuildFragment,
}

// ---------------------------------------------------------------------------
// Register layout for emission
// ---------------------------------------------------------------------------

/// Linux register layout data for register-helper emission.
///
/// Carries enough structural metadata that the emitter can produce
/// per-register byte-offset defines and per-bitfield mask/shift macros
/// without a hardcoded device-specific offset table.
#[derive(Debug, Clone)]
pub struct LinuxRegisterLayout {
    /// Register blocks with per-field byte offsets.
    pub blocks: Vec<LinuxRegisterBlock>,
    /// Register descriptors with bitfield and named-value details.
    pub descriptors: Vec<LinuxRegisterDescriptor>,
}

/// A register block with per-field structural metadata.
#[derive(Debug, Clone)]
pub struct LinuxRegisterBlock {
    /// Block type name (e.g. "FooRegisters").
    pub block_name: String,
    /// C identifier prefix for generated register defines.
    ///
    /// This is a formatting policy for generated C identifiers, not a
    /// semantic fact. Derived from the authored block name by the planner
    /// (e.g., `"FooRegisters"` → `"FOO"`).
    pub reg_prefix: String,
    /// Register fields with computed byte offsets.
    pub fields: Vec<LinuxRegisterFieldEntry>,
}

/// A single register field with its computed byte offset.
#[derive(Debug, Clone)]
pub struct LinuxRegisterFieldEntry {
    /// Register name (e.g. "dr", "fr").
    pub name: String,
    /// Byte offset within the register block.
    pub byte_offset: u32,
    /// Access width in bytes (e.g. 4 for a 32-bit register).
    pub access_width_bytes: u32,
}

/// A register descriptor with bitfield details for emission.
#[derive(Debug, Clone)]
pub struct LinuxRegisterDescriptor {
    /// Register module name (e.g. "DR_REG").
    pub register_name: String,
    /// Bitfield entries.
    pub bit_fields: Vec<LinuxBitFieldEntry>,
}

/// A single bitfield entry for Linux emission.
#[derive(Debug, Clone)]
pub struct LinuxBitFieldEntry {
    /// Field name (e.g. "DATA", "TXFF").
    pub name: String,
    /// Bit offset within the register.
    pub bit_offset: u32,
    /// Bit width.
    pub bit_width: u32,
    /// Named values (enum variants), if any.
    pub named_values: Vec<LinuxNamedValue>,
}

/// A named value (enum variant) for a bitfield.
#[derive(Debug, Clone)]
pub struct LinuxNamedValue {
    /// Symbolic name (e.g. "EIGHT_BITS").
    pub name: String,
    /// Numeric value.
    pub value: u32,
}

// ---------------------------------------------------------------------------
// Property realization
// ---------------------------------------------------------------------------

/// Linux-specific property realization details.
///
/// Carries enough information for the emitter to generate
/// `device_property_read_*` calls with correct types and defaults.
#[derive(Debug, Clone)]
pub struct LinuxPropertyRealization {
    /// Property identifier (e.g. "current-speed").
    pub property_id: String,
    /// Destination field name on the driver struct.
    pub field_name: String,
    /// C type string for the emitted read call (e.g. "u32").
    pub c_type: String,
    /// Whether the property is required.
    pub required: bool,
    /// Default literal from the declaration, if any.
    pub default_literal: Option<String>,
    /// Linux API for reading this property (e.g. "device_property_read_u32").
    pub linux_read_api: String,
}

// ---------------------------------------------------------------------------
// uart_ops realization
// ---------------------------------------------------------------------------

/// How a Linux uart_ops callback body should be realized.
#[derive(Debug, Clone)]
pub enum LinuxUartOpsBody {
    /// Emit the semantic body directly (self-only params, direct return).
    DirectBody { role: FunctionRole },
    /// Emit the semantic body with parameter bindings and/or return adaptation.
    WrappedBody {
        role: FunctionRole,
        param_bindings: Vec<ParamBinding>,
        return_adaptation: Option<ReturnAdaptation>,
    },
    /// Minimal stub body (e.g., modem control, or callbacks not yet
    /// realized from lowered plan data).
    Stub {
        /// C code lines for the stub body.
        lines: Vec<String>,
    },
}

/// A parameter binding that maps a Linux callback parameter to a semantic name.
#[derive(Debug, Clone)]
pub struct ParamBinding {
    /// Semantic parameter name expected by the body.
    pub semantic_name: String,
    /// C type for the binding declaration.
    pub c_type: String,
    /// C expression that computes the semantic value from Linux params.
    pub c_expr: String,
}

/// How the return value is adapted between semantic body and Linux callback.
#[derive(Debug, Clone)]
pub enum ReturnAdaptation {
    /// Adapt bool return to unsigned int with symbolic true/false constants.
    BoolToUint {
        true_value: String,
        false_value: String,
    },
    /// Adapt Result return to int (error paths pass through, success → 0).
    ResultToInt,
    /// Drop Result return (Linux callback returns void).
    DropResult,
}

/// Realization entry for a single Linux uart_ops callback.
#[derive(Debug, Clone)]
pub struct LinuxUartOpsEntry {
    /// Linux callback field name (e.g., "start_tx").
    pub op_name: String,
    /// Full C function signature.
    pub signature: String,
    /// How to realize the body.
    pub body_source: LinuxUartOpsBody,
    /// Whether to emit `container_of` preamble.
    pub needs_container_of: bool,
    /// Default trailing lines (e.g., "return 0;").
    pub trailing: Vec<String>,
}

// ---------------------------------------------------------------------------
// Behavior mapping
// ---------------------------------------------------------------------------

/// Mapping from portable function behaviors to Linux callbacks.
///
/// Tells the emitter which portable function maps to which Linux
/// callback without the emitter needing device-specific knowledge.
#[derive(Debug, Clone)]
pub struct LinuxBehaviorMapping {
    /// Per-function callback mappings.
    pub callback_mappings: Vec<LinuxCallbackMapping>,
    /// Number of functions in the portable behavior plan.
    pub portable_function_count: usize,
    /// Number of interrupt classes available from the portable plan.
    pub interrupt_class_count: usize,
}

/// Mapping for a single portable function to its Linux callback.
#[derive(Debug, Clone)]
pub struct LinuxCallbackMapping {
    /// Function name from the portable behavior plan.
    pub portable_function: String,
    /// Linux callback name for emission.
    pub linux_callback: String,
    /// Which Linux callback group this belongs to.
    pub group: LinuxCallbackGroup,
}

/// Group of Linux callbacks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinuxCallbackGroup {
    /// Probe/remove callbacks.
    ProbeRemove,
    /// uart_ops callbacks.
    UartOps,
    /// ISR handler.
    IsrHandler,
    /// Internal helper (not a direct Linux callback).
    Helper,
}
