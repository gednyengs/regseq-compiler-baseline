//! Per-entity provenance for compiler-owned model projections.
//!
//! Provenance records keep enough source context to explain where a
//! lowering or analysis obligation came from without depending on a
//! separate intermediate representation.

use serde::{Deserialize, Serialize};

/// Source provenance attached to a model-derived entity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Provenance {
    /// Source file path where the entity was declared, if available.
    pub source_file: Option<String>,
    /// Containing item or method name that originated this entity.
    pub containing_item: Option<String>,
    /// Model family this entity was derived from
    /// (e.g. `"identity"`, `"register_access"`, `"clock"`).
    pub family: String,
}

impl Provenance {
    /// Create a new provenance record.
    pub fn new(family: impl Into<String>) -> Self {
        Self {
            source_file: None,
            containing_item: None,
            family: family.into(),
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
