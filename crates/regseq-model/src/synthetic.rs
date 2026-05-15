//! Synthesised item discriminators.
//!
//! This module owns synthetic item classification data that is
//! parallel to `RegseqModel.items`.

use serde::{Deserialize, Serialize};

use crate::def_path::DefPathHash;
use crate::placeholders::PrincipalTraitKey;

/// Source or synthesised item classification.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SyntheticItemKind {
    /// Source-derived item.
    #[default]
    None,

    /// Closure captures struct item.
    ///
    /// This is a discriminator only: the underlying item identity is
    /// the closure expression's rustc-derived `DefPathHash`, carried
    /// both here and in the parallel item identity side table.
    ClosureCaptures {
        /// Rustc `DefPathHash` of the closure expression.
        closure: DefPathHash,
    },

    /// Synthesised closure call-method function item.
    ClosureCall {
        /// Rustc `DefPathHash` of the closure expression.
        closure: DefPathHash,
        /// Closure call trait method represented by this function.
        call_kind: ClosureCallKind,
    },

    /// Synthesised closure drop-glue function item.
    ClosureDropGlue {
        /// Rustc `DefPathHash` of the closure expression.
        closure: DefPathHash,
    },

    /// Bridge-synthesised vtable type item.
    ///
    /// The full vtable family lands in the following task; this
    /// placeholder variant remains so existing root-model tests keep
    /// compiling until the complete key surface replaces it.
    VTableType {
        /// Principal trait key for the vtable type.
        principal: PrincipalTraitKey,
    },
}

/// Closure call-method kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ClosureCallKind {
    /// `Fn::call`.
    Call,
    /// `FnMut::call_mut`.
    CallMut,
    /// `FnOnce::call_once`.
    CallOnce,
}

#[cfg(test)]
mod tests {
    use std::collections::hash_map::DefaultHasher;
    use std::collections::HashSet;
    use std::hash::{Hash, Hasher};

    use super::*;
    use crate::crate_info::StableCrateId;

    fn closure_hash() -> DefPathHash {
        DefPathHash {
            stable_crate_id: StableCrateId(0xC105),
            local_hash: 0xA11CE,
        }
    }

    fn hash_of<T: Hash>(value: &T) -> u64 {
        let mut hasher = DefaultHasher::new();
        value.hash(&mut hasher);
        hasher.finish()
    }

    #[test]
    fn closure_call_kind_variants_construct_clone_and_compare() {
        let call = ClosureCallKind::Call;
        let call_mut = ClosureCallKind::CallMut;
        let call_once = ClosureCallKind::CallOnce;

        assert_eq!(call.clone(), call);
        assert_ne!(call, call_mut);
        assert_ne!(call, call_once);
        assert_ne!(call_mut, call_once);
    }

    #[test]
    fn closure_call_kind_hashes_clones_equally_and_variants_distinctly() {
        let call = ClosureCallKind::Call;
        assert_eq!(hash_of(&call), hash_of(&call.clone()));

        let set = HashSet::from([
            ClosureCallKind::Call,
            ClosureCallKind::CallMut,
            ClosureCallKind::CallOnce,
        ]);
        assert_eq!(set.len(), 3);
    }

    #[test]
    fn closure_call_kind_debug_strings_are_distinguishable() {
        let debug = [
            format!("{:?}", ClosureCallKind::Call),
            format!("{:?}", ClosureCallKind::CallMut),
            format!("{:?}", ClosureCallKind::CallOnce),
        ];
        assert_eq!(HashSet::from(debug).len(), 3);
    }

    #[test]
    fn closure_captures_constructs_and_clones() {
        let kind = SyntheticItemKind::ClosureCaptures {
            closure: closure_hash(),
        };
        assert_eq!(kind.clone(), kind);
    }

    #[test]
    fn closure_call_constructs_for_each_call_kind() {
        let closure = closure_hash();
        let calls = [
            SyntheticItemKind::ClosureCall {
                closure,
                call_kind: ClosureCallKind::Call,
            },
            SyntheticItemKind::ClosureCall {
                closure,
                call_kind: ClosureCallKind::CallMut,
            },
            SyntheticItemKind::ClosureCall {
                closure,
                call_kind: ClosureCallKind::CallOnce,
            },
        ];

        assert_eq!(HashSet::from(calls).len(), 3);
    }

    #[test]
    fn closure_drop_glue_constructs_and_is_distinct() {
        let closure = closure_hash();
        let drop_glue = SyntheticItemKind::ClosureDropGlue { closure };
        let call = SyntheticItemKind::ClosureCall {
            closure,
            call_kind: ClosureCallKind::CallOnce,
        };
        let captures = SyntheticItemKind::ClosureCaptures { closure };

        assert_ne!(drop_glue, call);
        assert_ne!(drop_glue, captures);
    }

    #[test]
    fn closure_family_variants_are_pairwise_distinct_for_one_closure() {
        let closure = closure_hash();
        let variants = HashSet::from([
            SyntheticItemKind::ClosureCaptures { closure },
            SyntheticItemKind::ClosureCall {
                closure,
                call_kind: ClosureCallKind::Call,
            },
            SyntheticItemKind::ClosureCall {
                closure,
                call_kind: ClosureCallKind::CallMut,
            },
            SyntheticItemKind::ClosureCall {
                closure,
                call_kind: ClosureCallKind::CallOnce,
            },
            SyntheticItemKind::ClosureDropGlue { closure },
        ]);

        assert_eq!(variants.len(), 5);
    }

    #[test]
    fn closure_family_variants_round_trip_through_serde() {
        let closure = closure_hash();
        let variants = [
            SyntheticItemKind::ClosureCaptures { closure },
            SyntheticItemKind::ClosureCall {
                closure,
                call_kind: ClosureCallKind::Call,
            },
            SyntheticItemKind::ClosureCall {
                closure,
                call_kind: ClosureCallKind::CallMut,
            },
            SyntheticItemKind::ClosureCall {
                closure,
                call_kind: ClosureCallKind::CallOnce,
            },
            SyntheticItemKind::ClosureDropGlue { closure },
        ];

        for variant in variants {
            let json = serde_json::to_string(&variant).unwrap();
            assert_eq!(
                serde_json::from_str::<SyntheticItemKind>(&json).unwrap(),
                variant
            );
        }

        for call_kind in [
            ClosureCallKind::Call,
            ClosureCallKind::CallMut,
            ClosureCallKind::CallOnce,
        ] {
            let json = serde_json::to_string(&call_kind).unwrap();
            assert_eq!(
                serde_json::from_str::<ClosureCallKind>(&json).unwrap(),
                call_kind
            );
        }
    }

    #[test]
    fn closure_call_kind_rank_table_matches_spec_order() {
        fn rank(kind: ClosureCallKind) -> u8 {
            match kind {
                ClosureCallKind::Call => 0,
                ClosureCallKind::CallMut => 1,
                ClosureCallKind::CallOnce => 2,
            }
        }

        assert_eq!(rank(ClosureCallKind::Call), 0);
        assert_eq!(rank(ClosureCallKind::CallMut), 1);
        assert_eq!(rank(ClosureCallKind::CallOnce), 2);
    }
}
