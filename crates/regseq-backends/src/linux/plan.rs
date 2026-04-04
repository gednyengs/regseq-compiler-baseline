//! Linux-specific backend plan types.
//!
//! `LinuxBackendPlan` captures Linux realization decisions derived from
//! the portable `DriverLoweringPlan`. This includes Linux driver
//! registration, serial-core integration, probe/remove orchestration,
//! resource-acquisition sequencing, and emitted-file layout.
//!
//! No portable lowering types are redefined here; they are consumed
//! from the `lowering::plan` module.

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
    /// Linux-specific resource acquisition sequencing.
    pub resource_acquisition: LinuxResourceSequencing,
    /// Emitted-file layout decisions.
    pub artifact_layout: LinuxArtifactLayout,
    /// Register layout for register-helper emission.
    pub register_layout: LinuxRegisterLayout,
    /// Per-property realization details for emission.
    pub property_realization: Vec<LinuxPropertyRealization>,
    /// Deployment metadata for optional resource families.
    pub deployment_metadata: LinuxDeploymentMetadata,
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
    /// Port type constant name.
    pub port_type: String,
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
// Resource acquisition sequencing
// ---------------------------------------------------------------------------

/// Linux-specific resource acquisition sequencing.
#[derive(Debug, Clone)]
pub struct LinuxResourceSequencing {
    /// Ordered acquisition entries.
    pub acquisition_order: Vec<LinuxAcquisitionEntry>,
    /// Ordered release entries (reverse of acquisition).
    pub release_order: Vec<LinuxReleaseEntry>,
}

/// A single Linux resource acquisition entry.
#[derive(Debug, Clone)]
pub struct LinuxAcquisitionEntry {
    /// Position in the sequence.
    pub order: usize,
    /// Linux API family (e.g. "devm_ioremap", "devm_clk_get").
    pub api_family: String,
    /// Resource identifier.
    pub resource_id: String,
    /// Whether failure is fatal.
    pub required: bool,
}

/// A single Linux resource release entry.
#[derive(Debug, Clone)]
pub struct LinuxReleaseEntry {
    /// Position in the sequence.
    pub order: usize,
    /// Linux API family for release (e.g. "clk_disable_unprepare").
    pub api_family: String,
    /// Resource identifier.
    pub resource_id: String,
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
/// without a hardcoded PL011 offset table.
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
    /// Block type name (e.g. "Pl011Registers").
    pub block_name: String,
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
// Deployment metadata
// ---------------------------------------------------------------------------

/// Deployment metadata for optional resource families.
///
/// Carries clock, reset, and pinctrl facts from the joined/binding
/// layer so that emission does not need to re-read raw frontend models.
#[derive(Debug, Clone)]
pub struct LinuxDeploymentMetadata {
    /// Clock deployment entries.
    pub clocks: Vec<LinuxClockEntry>,
    /// Reset deployment entries.
    pub resets: Vec<LinuxResetEntry>,
    /// Pin controller deployment entries.
    pub pinctrl: Vec<LinuxPinCtrlEntry>,
}

/// Clock deployment metadata for Linux emission.
#[derive(Debug, Clone)]
pub struct LinuxClockEntry {
    /// Clock identifier.
    pub clock_id: String,
    /// Whether the clock is required.
    pub required: bool,
    /// Default frequency in Hz from the deployment binding, if supplied.
    pub default_frequency_hz: Option<i64>,
}

/// Reset deployment metadata for Linux emission.
#[derive(Debug, Clone)]
pub struct LinuxResetEntry {
    /// Field name on the driver struct.
    pub field_name: String,
    /// Whether the reset is required.
    pub required: bool,
    /// Reset line identifier from the deployment binding.
    pub reset_line_id: Option<String>,
    /// Whether the reset line is shared.
    pub shared: Option<bool>,
}

/// Pin controller deployment metadata for Linux emission.
#[derive(Debug, Clone)]
pub struct LinuxPinCtrlEntry {
    /// Field name on the driver struct.
    pub field_name: String,
    /// Whether the pinctrl is required.
    pub required: bool,
    /// Available pin states from the deployment binding.
    pub states: Option<Vec<String>>,
}
