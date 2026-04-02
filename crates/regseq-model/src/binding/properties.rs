//! Property binding entries from the Layer 2 `[properties]` section.
//!
//! Each property binding carries its join-relevant key, a typed value,
//! and per-entity provenance.

use serde::{Deserialize, Serialize};

use super::provenance::BindingProvenance;
use crate::platform::PlatformValue;

/// A normalized property binding entry from the `[properties]` section.
///
/// The `key` serves as the join identifier, matching the Layer 1
/// `#[rseq::property(id = "...")]` declaration. Values preserve the
/// accepted scalar/aggregate typing from the parser output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PropertyBinding {
    /// Property identifier matching the Layer 1 declaration id.
    /// This is the join key for property bindings.
    pub key: String,
    /// Typed property value from the Layer 2 file.
    pub value: PlatformValue,
    /// Provenance tracing back to the Layer 2 source.
    pub provenance: BindingProvenance,
}
