//! Forward-declared value types for phase-1a tasks.
//!
//! These stubs keep the root model shape compileable while later
//! P1a-DS tasks replace them with concrete schemas.

use serde::{Deserialize, Serialize};

pub use crate::item::{InlineHint, Item, Linkage};
pub use crate::synthetic::{PrincipalTraitKey, SyntheticItemKind};

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
        let principal = PrincipalTraitKey {
            trait_path: crate::DefPathHash {
                stable_crate_id: crate::StableCrateId(1),
                local_hash: 2,
            },
            trait_args: crate::ArgsKey::EMPTY,
            projection_bindings: crate::ProjectionBindingsKey(0),
        };
        let kind = SyntheticItemKind::VTableType { principal };
        assert!(matches!(kind, SyntheticItemKind::VTableType { .. }));
    }
}
