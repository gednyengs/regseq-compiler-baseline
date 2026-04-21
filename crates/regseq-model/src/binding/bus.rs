//! Bus binding entry from the Layer 2 `[bus]` section.
//!
//! Captures the typed bus family and family-specific deployment metadata.
//! This is driver-level binding data that participates in lowering decisions
//! rather than per-resource joins.

use serde::{Deserialize, Serialize};

use crate::declaration::driver::BusType;

use super::provenance::BindingProvenance;

/// A normalized bus binding from the `[bus]` section.
///
/// Contains the typed bus family identity and any family-specific metadata
/// validated during normalization. AMBA-specific fields are only populated
/// when `bus_type` is `BusType::Amba`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BusBinding {
    /// Typed portable bus family.
    pub bus_type: BusType,
    /// AMBA peripheral ID (only valid for AMBA bus family).
    pub amba_id: Option<i64>,
    /// AMBA peripheral revision (only valid for AMBA bus family).
    pub revision: Option<i64>,
    /// AMBA peripheral-ID match-table mask (only valid for AMBA bus
    /// family). When `None`, the Linux planner applies the PrimeCell
    /// default (`0x000f_ffff`) and the portable plan records an
    /// `amba_mask_default_applied` diagnostic.
    pub amba_mask: Option<i64>,
    /// Provenance tracing back to the Layer 2 source.
    pub provenance: BindingProvenance,
}
