//! Per-entity source provenance for binding-model nodes.
//!
//! Every normalized binding entity carries provenance tracing back to
//! the parsed Layer 2 section and key that originated it.

use serde::{Deserialize, Serialize};

/// Source provenance attached to a normalized binding entity.
///
/// Tracks the origin of each entity back to the parsed Layer 2 platform
/// file so that later join and analysis passes can audit the normalization
/// path without relying on top-level metadata alone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BindingProvenance {
    /// The Layer 2 section kind that originated this entity
    /// (e.g. `"platform"`, `"properties"`, `"mmio"`, `"interrupt"`).
    pub section_kind: String,
    /// The section key within the family, if applicable
    /// (e.g. `"uart_regs"` for `[mmio.uart_regs]`).
    pub section_key: Option<String>,
    /// Source file path of the platform file, if available.
    pub source_file: Option<String>,
}

impl BindingProvenance {
    /// Create a new provenance record for a given section kind.
    pub fn new(section_kind: impl Into<String>) -> Self {
        Self {
            section_kind: section_kind.into(),
            section_key: None,
            source_file: None,
        }
    }

    /// Set the section key.
    pub fn with_section_key(mut self, key: impl Into<String>) -> Self {
        self.section_key = Some(key.into());
        self
    }

    /// Set the source file path.
    pub fn with_source_file(mut self, path: impl Into<String>) -> Self {
        self.source_file = Some(path.into());
        self
    }
}
