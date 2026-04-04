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
