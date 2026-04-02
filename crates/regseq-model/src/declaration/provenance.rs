//! Per-entity source provenance for declaration-model nodes.
//!
//! Every normalized declaration entity should carry enough provenance to
//! answer: where did this come from, which item or method originated it,
//! and which raw artifact family did it normalize from.

use serde::{Deserialize, Serialize};

/// Source provenance attached to a normalized declaration entity.
///
/// Tracks the origin of each entity back to the raw frontend artifact so
/// that later analysis can audit the normalization path without relying on
/// top-level metadata alone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Provenance {
    /// Source file path where the entity was declared, if available.
    pub source_file: Option<String>,
    /// Containing item or method name that originated this entity.
    pub containing_item: Option<String>,
    /// Raw artifact family this entity was normalized from
    /// (e.g. `"identity"`, `"register_access"`, `"clock"`).
    pub raw_family: String,
}

impl Provenance {
    /// Create a new provenance record.
    pub fn new(raw_family: impl Into<String>) -> Self {
        Self {
            source_file: None,
            containing_item: None,
            raw_family: raw_family.into(),
        }
    }

    /// Set the source file path.
    pub fn with_source_file(mut self, path: impl Into<String>) -> Self {
        self.source_file = Some(path.into());
        self
    }

    /// Set the containing item name.
    pub fn with_containing_item(mut self, item: impl Into<String>) -> Self {
        self.containing_item = Some(item.into());
        self
    }
}
