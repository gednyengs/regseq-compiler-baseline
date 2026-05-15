//! C ABI lowering result shapes.

use serde::{Deserialize, Serialize};

use crate::ids::TyId;

/// C ABI shape for one function-signature position.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CAbiShape {
    /// Use the position's own `TyId` in its natural C representation.
    Direct,

    /// Substitute a different `TyId` at the C boundary.
    Substituted {
        /// Final terminal type to emit at the C boundary.
        emit_as: TyId,
        /// Reason why substitution occurred.
        reason: SubstReason,
    },
}

/// Reason for a C ABI substitution.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SubstReason {
    /// One or more transparent unwraps.
    TransparentChain,

    /// Fieldless integer-representation enum.
    TagScalar,

    /// Transparent unwraps that ultimately resolve to a tag scalar.
    TransparentChainAndTagScalar,
}

/// C ABI lowering result for every position in a function signature.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CAbiLowering {
    /// One shape per `FnSig.inputs` entry.
    pub inputs: Vec<CAbiShape>,
    /// Return-position shape.
    pub output: CAbiShape,
}

#[allow(dead_code)]
fn _c_abi_shape_exhaustiveness_check(shape: &CAbiShape) {
    match shape {
        CAbiShape::Direct => (),
        CAbiShape::Substituted {
            emit_as: _,
            reason: _,
        } => (),
    }
}

#[allow(dead_code)]
fn _subst_reason_exhaustiveness_check(reason: &SubstReason) {
    match reason {
        SubstReason::TransparentChain => (),
        SubstReason::TagScalar => (),
        SubstReason::TransparentChainAndTagScalar => (),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    fn ty(value: u32) -> TyId {
        TyId::from_u32(value)
    }

    fn hash<T: Hash>(value: &T) -> u64 {
        let mut hasher = DefaultHasher::new();
        value.hash(&mut hasher);
        hasher.finish()
    }

    #[test]
    fn c_abi_shape_variants_construct() {
        let direct = CAbiShape::Direct;
        let transparent = CAbiShape::Substituted {
            emit_as: ty(1),
            reason: SubstReason::TransparentChain,
        };
        let tag = CAbiShape::Substituted {
            emit_as: ty(2),
            reason: SubstReason::TagScalar,
        };
        let both = CAbiShape::Substituted {
            emit_as: ty(3),
            reason: SubstReason::TransparentChainAndTagScalar,
        };

        assert_eq!(direct, CAbiShape::Direct);
        assert!(matches!(
            transparent,
            CAbiShape::Substituted {
                reason: SubstReason::TransparentChain,
                ..
            }
        ));
        assert!(matches!(
            tag,
            CAbiShape::Substituted {
                reason: SubstReason::TagScalar,
                ..
            }
        ));
        assert!(matches!(
            both,
            CAbiShape::Substituted {
                reason: SubstReason::TransparentChainAndTagScalar,
                ..
            }
        ));
    }

    #[test]
    fn c_abi_lowering_constructs_and_round_trips_through_serde() {
        let lowering = CAbiLowering {
            inputs: vec![CAbiShape::Direct, CAbiShape::Direct],
            output: CAbiShape::Direct,
        };
        let json = serde_json::to_string(&lowering).unwrap();
        assert_eq!(
            serde_json::from_str::<CAbiLowering>(&json).unwrap(),
            lowering
        );
    }

    #[test]
    fn c_abi_lowering_equality_and_hash_cover_fields() {
        let a = CAbiLowering {
            inputs: vec![CAbiShape::Direct],
            output: CAbiShape::Substituted {
                emit_as: ty(1),
                reason: SubstReason::TransparentChain,
            },
        };
        let b = a.clone();
        assert_eq!(a, b);
        assert_eq!(hash(&a), hash(&b));

        let different_emit_as = CAbiShape::Substituted {
            emit_as: ty(2),
            reason: SubstReason::TransparentChain,
        };
        let different_reason = CAbiShape::Substituted {
            emit_as: ty(1),
            reason: SubstReason::TagScalar,
        };

        assert_ne!(a.output, different_emit_as);
        assert_ne!(a.output, different_reason);
    }
}
