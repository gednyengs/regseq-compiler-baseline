//! Portable lowering diagnostics.
//!
//! Reviewer-facing reports that document Linux-oriented sequencing
//! decisions derived from the portable plan. These are **reporting
//! only** -- they are not consumed by any emitter pass. Codex
//! round-3 verification (see task spec) confirmed that the types
//! relocated here were previously populated on [`crate::linux::plan::LinuxBackendPlan`]
//! and test-asserted but never read by the Linux emitter.
//!
//! Design choice (option 2 of `regseq-lowering-10` Implementation
//! Notes): the Linux-flavored strings on these structs (API family
//! names, deployment fields) are not backend-neutral, so folding
//! them directly into [`crate::lowering::plan::DriverLoweringPlan`]
//! as portable obligations would misrepresent them. Keeping them
//! behind a [`DriverLoweringDiagnostics`] handle documents the
//! reviewer-facing intent while still surfacing the information to
//! tests that previously asserted on the Linux plan.

// ---------------------------------------------------------------------------
// Diagnostics container
// ---------------------------------------------------------------------------

/// Reviewer-facing diagnostics produced alongside a
/// [`crate::lowering::plan::DriverLoweringPlan`].
///
/// None of the data here is consumed by emission. Tests that used to
/// assert on the Linux plan's `resource_acquisition` and
/// `deployment_metadata` fields now assert on this diagnostics
/// bundle.
#[derive(Debug, Clone, Default)]
pub struct DriverLoweringDiagnostics {
    /// Linux resource-acquisition sequencing, reporting-only.
    pub linux_resource_sequencing: LinuxResourceSequencing,
    /// Linux deployment metadata for optional resource families,
    /// reporting-only.
    pub linux_deployment_metadata: LinuxDeploymentMetadata,
}

// ---------------------------------------------------------------------------
// Linux resource sequencing (reporting-only)
// ---------------------------------------------------------------------------

/// Linux-specific resource acquisition sequencing.
///
/// Relocated from `LinuxBackendPlan::resource_acquisition` per
/// `regseq-lowering-10`. Reporting-only; not consumed by emitters.
#[derive(Debug, Clone, Default)]
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
// Linux deployment metadata (reporting-only)
// ---------------------------------------------------------------------------

/// Deployment metadata for optional resource families.
///
/// Relocated from `LinuxBackendPlan::deployment_metadata` per
/// `regseq-lowering-10`. Reporting-only; not consumed by emitters.
#[derive(Debug, Clone, Default)]
pub struct LinuxDeploymentMetadata {
    /// Clock deployment entries.
    pub clocks: Vec<LinuxClockEntry>,
    /// Reset deployment entries.
    pub resets: Vec<LinuxResetEntry>,
    /// Pin controller deployment entries.
    pub pinctrl: Vec<LinuxPinCtrlEntry>,
}

/// Clock deployment metadata.
#[derive(Debug, Clone)]
pub struct LinuxClockEntry {
    /// Clock identifier.
    pub clock_id: String,
    /// Whether the clock is required.
    pub required: bool,
    /// Default frequency in Hz from the deployment binding, if supplied.
    pub default_frequency_hz: Option<i64>,
}

/// Reset deployment metadata.
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

/// Pin controller deployment metadata.
#[derive(Debug, Clone)]
pub struct LinuxPinCtrlEntry {
    /// Field name on the driver struct.
    pub field_name: String,
    /// Whether the pinctrl is required.
    pub required: bool,
    /// Available pin states from the deployment binding.
    pub states: Option<Vec<String>>,
}
