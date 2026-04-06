//! Module definition for the item hierarchy.

use serde::{Deserialize, Serialize};

use crate::declaration::Provenance;

use super::identity::{CanonicalPath, ItemId};

/// A Regseq module -- a logical namespace for items.
///
/// Modules are logical groupings, not emitted OS files. Backends are free
/// to choose entirely different physical file layouts.
///
/// In the first implementation slice, modules are flat (one level per
/// driver spec). Recursive module nesting is intentionally deferred
/// until multi-module driver support is needed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Module {
    /// Module name (e.g. "driver", "registers").
    pub name: String,
    pub canonical_path: CanonicalPath,
    /// Items in source order. Resolved through the canonical `ItemStore`.
    pub items: Vec<ItemId>,
    pub provenance: Provenance,
}
