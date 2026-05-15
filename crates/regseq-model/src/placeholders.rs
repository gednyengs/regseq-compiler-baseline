//! Forward-declared value types for phase-1a tasks.
//!
//! These stubs keep the root model shape compileable while later
//! P1a-DS tasks replace them with concrete schemas.

use serde::{Deserialize, Serialize};

use crate::ids::{ConstId, FnSigId};
use crate::index_vec::IndexVec;

/// Forward-declared `Const`.
// FORWARD: P1a-DS-05
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Const {}

/// Forward-declared const interner.
// FORWARD: P1a-DS-05
pub type ConstInterner = IndexVec<ConstId, Const>;

/// Forward-declared `Item`.
// FORWARD: P1a-DS-07
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Item {}

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

/// Forward-declared `FnSig`.
// FORWARD: P1a-DS-06
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FnSig {}

/// Forward-declared function signature interner.
// FORWARD: P1a-DS-06
pub type FnSigInterner = IndexVec<FnSigId, FnSig>;

/// Forward-declared C ABI lowering record.
// FORWARD: P1a-DS-06
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CAbiLowering {}

/// Forward-declared linkage classification.
// FORWARD: P1a-DS-07
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Linkage {
    /// Default linkage placeholder.
    #[default]
    Default,
}

/// Forward-declared inline hint.
// FORWARD: P1a-DS-07
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum InlineHint {
    /// No inline hint placeholder.
    #[default]
    None,
}

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
        assert_eq!(Const::default(), Const {});
        assert_eq!(Item::default(), Item {});
        assert_eq!(FnSig::default(), FnSig {});
        assert_eq!(Body::default(), Body {});
    }

    #[test]
    fn forward_interner_aliases_construct() {
        let consts: ConstInterner = IndexVec::new();
        let sigs: FnSigInterner = IndexVec::new();
        assert!(consts.is_empty());
        assert!(sigs.is_empty());
    }

    #[test]
    fn synthetic_item_kind_and_principal_key_construct() {
        let principal = PrincipalTraitKey::default();
        let kind = SyntheticItemKind::VTableType { principal };
        assert!(matches!(kind, SyntheticItemKind::VTableType { .. }));
    }

    #[test]
    fn remaining_forward_stubs_construct() {
        assert_eq!(CAbiLowering::default(), CAbiLowering {});
        assert_eq!(Linkage::default(), Linkage::Default);
        assert_eq!(InlineHint::default(), InlineHint::None);
        assert_eq!(IntrinsicKind::default(), IntrinsicKind::Placeholder);
    }
}
