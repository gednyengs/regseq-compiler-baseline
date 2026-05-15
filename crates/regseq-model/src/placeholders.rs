//! Forward-declared value types for phase-1a tasks.
//!
//! These stubs keep the root model shape compileable while later
//! P1a-DS tasks replace them with concrete schemas.

use serde::{Deserialize, Serialize};

pub use crate::item::{InlineHint, Item, Linkage};

/// Forward-declared source item synthetic-kind tag.
// FORWARD: P1a-DS-08b
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SyntheticItemKind {
    /// Source-derived item.
    #[default]
    None,
    /// Bridge-synthesized vtable type item.
    VTableType {
        /// Principal trait key for the vtable type.
        principal: PrincipalTraitKey,
    },
}

/// Forward-declared principal trait identity.
// FORWARD: P1a-DS-08b
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PrincipalTraitKey {}

/// Forward-declared intrinsic classification.
// FORWARD: P1a-DS-09
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum IntrinsicKind {
    /// Placeholder intrinsic kind.
    #[default]
    Placeholder,
}

/// Forward-declared `Body`.
// FORWARD: P1a-DS-10
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Body {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forward_value_stubs_construct() {
        assert_eq!(Body::default(), Body {});
    }

    #[test]
    fn synthetic_item_kind_and_principal_key_construct() {
        let principal = PrincipalTraitKey::default();
        let kind = SyntheticItemKind::VTableType { principal };
        assert!(matches!(kind, SyntheticItemKind::VTableType { .. }));
    }

    #[test]
    fn remaining_forward_stubs_construct() {
        assert_eq!(IntrinsicKind::default(), IntrinsicKind::Placeholder);
    }
}
