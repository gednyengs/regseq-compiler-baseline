//! Binding IR: normalized Layer 2 binding types.
//!
//! This module defines the normalized binding-model types for the Regseq
//! compiler. It coexists with declaration, join, and lowering-oriented IR
//! families and preserves section ownership, typed per-family entries,
//! and per-entity provenance.
//!
//! The binding model starts from the accepted parser output shape as-is
//! and does not assume declaration-aware inference, join results, or
//! backend lowering strategies. It is well-formed independently of the
//! declaration IR.

pub mod bus;
pub mod header;
pub mod join_key;
pub mod normalize;
pub mod properties;
pub mod provenance;
pub mod resources;

// Re-export top-level types for convenience.
pub use bus::BusBinding;
pub use header::BindingHeader;
pub use join_key::JoinKey;
pub use normalize::{normalize, BindingNormalizeError};
pub use properties::PropertyBinding;
pub use provenance::BindingProvenance;
pub use resources::{ClockBinding, InterruptBinding, MmioBinding, PinCtrlBinding, ResetBinding};

use serde::{Deserialize, Serialize};

/// Top-level normalized binding model.
///
/// Contains all Layer 2 binding-side facts for a single platform file,
/// organized by section family. Header/build metadata is kept separate
/// from resource and property binding entries.
///
/// The binding model does not depend on declaration IR to be well-formed.
/// It preserves section ownership rather than flattening all Layer 2 data
/// into one generic property map.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BindingModel {
    /// Platform deployment header (version, os, arch, board).
    pub header: BindingHeader,
    /// Normalized property binding entries from `[properties]`.
    pub properties: Vec<PropertyBinding>,
    /// Normalized MMIO binding entries from `[mmio.{id}]` sections.
    pub mmio: Vec<MmioBinding>,
    /// Normalized interrupt binding entries from `[interrupt.{id}]` sections.
    pub interrupts: Vec<InterruptBinding>,
    /// Normalized clock binding entries from `[clock.{id}]` sections.
    pub clocks: Vec<ClockBinding>,
    /// Normalized reset binding entries from `[reset.{field_name}]` sections.
    pub resets: Vec<ResetBinding>,
    /// Normalized pin controller binding entries from `[pinctrl.{id}]` sections.
    pub pinctrl: Vec<PinCtrlBinding>,
    /// Normalized bus binding from the `[bus]` section, if present.
    pub bus: Option<BusBinding>,
}
