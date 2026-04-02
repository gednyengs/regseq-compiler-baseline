//! Join-key types for binding-model entities.
//!
//! Provides an explicit home for the two accepted join mechanisms:
//! explicit attribute/id join and field-name join. Each binding entity
//! carries a `JoinKey` that identifies how it should be matched against
//! the corresponding declaration-side entity during the join phase.

use serde::{Deserialize, Serialize};

/// A join-relevant identifier carried by a binding entity.
///
/// Distinguishes between the two accepted Layer 1 / Layer 2 join
/// patterns so that the join phase can apply the correct matching
/// strategy without inspecting binding-entry internals.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum JoinKey {
    /// Explicit attribute/id join: the binding entry carries a named
    /// identifier that matches a Layer 1 `#[rseq::...(id = "...")]`
    /// attribute.
    ExplicitId(String),
    /// Field-name join: the binding entry carries a field name that
    /// matches a Layer 1 struct field declaration by name.
    FieldName(String),
}

impl JoinKey {
    /// Return the string value of the join key regardless of variant.
    pub fn value(&self) -> &str {
        match self {
            Self::ExplicitId(s) | Self::FieldName(s) => s,
        }
    }
}
