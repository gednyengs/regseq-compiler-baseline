//! Binding-model header facts from the Layer 2 platform file.
//!
//! Captures normalized platform deployment metadata (version, os, arch,
//! board) separately from resource and property binding entries.

use serde::{Deserialize, Serialize};

use super::provenance::BindingProvenance;

/// Normalized platform header from the Layer 2 binding file.
///
/// Contains deployment-level metadata that is not a resource or property
/// binding. This data informs backend selection and build configuration
/// but does not participate in per-resource join logic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BindingHeader {
    /// Schema version from the platform file (e.g. `"1"`).
    pub version: String,
    /// Target operating system (e.g. `"linux"`, `"zephyr"`).
    pub os: String,
    /// Target architecture (e.g. `"aarch64"`, `"x86_64"`).
    pub arch: String,
    /// Board or deployment label (e.g. `"raspberry-pi-4"`).
    pub board: String,
    /// Provenance tracing back to the Layer 2 source.
    pub provenance: BindingProvenance,
}
