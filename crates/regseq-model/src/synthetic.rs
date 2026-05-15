//! Synthesised item discriminators.
//!
//! This module owns synthetic item classification data that is
//! parallel to `RegseqModel.items`.

use serde::{Deserialize, Serialize};

use crate::args_key::ArgsKey;
use crate::def_path::DefPathHash;

/// Full identity for a trait object principal at the vtable boundary.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PrincipalTraitKey {
    /// Rustc `DefPathHash` for the trait `DefId`.
    ///
    /// The trait declaration need not correspond to an `ItemId` in
    /// `RegseqModel.items`; trait declarations are erased from the
    /// source-level item alphabet.
    pub trait_path: DefPathHash,
    /// Canonical generic arguments to the trait itself.
    pub trait_args: ArgsKey,
    /// Named associated item equality payload for the trait object.
    pub projection_bindings: ProjectionBindingsKey,
}

/// Stable 128-bit content fingerprint for projection bindings.
///
/// This is not a proof of equality. The model stores only the
/// truncated hash, not the canonical projection-binding bytes, and
/// treats truncated SHA-256 collision as outside the v1 threat model.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProjectionBindingsKey(pub u128);

/// Stable 128-bit content fingerprint for anonymous const memory.
///
/// This is not a proof of equality. The model stores only the
/// truncated hash, not the canonical allocation bytes, and treats
/// truncated SHA-256 collision as outside the v1 threat model.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ContentHash(pub u128);

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

    /// Bridge-synthesised drop-glue function item for a concrete type.
    DropGlue {
        /// Canonical self type key for the dropped type.
        self_ty: ArgsKey,
    },

    /// Bridge-synthesised vtable type item.
    ///
    /// Identity is the full principal trait key and is independent of
    /// the crate or site that first triggered synthesis.
    VTableType {
        /// Principal trait key for the vtable type.
        principal: PrincipalTraitKey,
    },

    /// Bridge-synthesised vtable instance static item.
    VTableInstance {
        /// Principal trait key for the vtable instance.
        principal: PrincipalTraitKey,
        /// Canonical self type key for the concrete implementation.
        self_ty: ArgsKey,
    },

    /// Bridge-synthesised vtable method shim function item.
    VTableMethodShim {
        /// Principal trait key for the method shim.
        principal: PrincipalTraitKey,
        /// Canonical self type key for the concrete implementation.
        self_ty: ArgsKey,
        /// Slot index in rustc's `tcx.vtable_entries` sequence.
        vtable_entry_index: u32,
        /// Rustc `DefPathHash` of the source trait method.
        method_def_path: DefPathHash,
    },

    /// Bridge-synthesised vtable drop-in-place shim function item.
    VTableDropShim {
        /// Principal trait key for the drop shim.
        principal: PrincipalTraitKey,
        /// Canonical self type key for the concrete implementation.
        self_ty: ArgsKey,
    },

    /// Bridge-synthesised immutable static for anonymous const memory.
    ConstMemoryStatic {
        /// Canonical content fingerprint for the memory allocation.
        content_hash: ContentHash,
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

    fn def_hash(stable_crate_id: u64, local_hash: u64) -> DefPathHash {
        DefPathHash {
            stable_crate_id: StableCrateId(stable_crate_id),
            local_hash,
        }
    }

    fn closure_hash() -> DefPathHash {
        def_hash(0xC105, 0xA11CE)
    }

    fn principal_with_path(local_hash: u64) -> PrincipalTraitKey {
        PrincipalTraitKey {
            trait_path: def_hash(0x711A17, local_hash),
            trait_args: ArgsKey(0x1111_2222_3333_4444_5555_6666_7777_8888),
            projection_bindings: ProjectionBindingsKey(0x9999_aaaa_bbbb_cccc_dddd_eeee_ffff_0000),
        }
    }

    fn principal() -> PrincipalTraitKey {
        principal_with_path(0x7A17)
    }

    fn self_ty() -> ArgsKey {
        ArgsKey(0x1234_5678_9abc_def0_0102_0304_0506_0708)
    }

    fn content_hash(value: u128) -> ContentHash {
        ContentHash(value)
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

    #[test]
    fn none_constructs_and_clones() {
        let kind = SyntheticItemKind::None;
        assert_eq!(kind.clone(), kind);
    }

    #[test]
    fn vtable_type_uses_full_principal_identity() {
        let first = SyntheticItemKind::VTableType {
            principal: principal(),
        };
        let same = SyntheticItemKind::VTableType {
            principal: principal(),
        };
        let different = SyntheticItemKind::VTableType {
            principal: principal_with_path(0x7A18),
        };

        assert_eq!(first, same);
        assert_ne!(first, different);
    }

    #[test]
    fn vtable_instance_is_distinct_from_vtable_type() {
        let principal = principal();
        let ty = SyntheticItemKind::VTableType {
            principal: principal.clone(),
        };
        let instance = SyntheticItemKind::VTableInstance {
            principal,
            self_ty: self_ty(),
        };

        assert_ne!(ty, instance);
    }

    #[test]
    fn vtable_method_shim_identity_includes_entry_index() {
        let first = SyntheticItemKind::VTableMethodShim {
            principal: principal(),
            self_ty: self_ty(),
            vtable_entry_index: 4,
            method_def_path: def_hash(0xCA11, 0xA),
        };
        let second = SyntheticItemKind::VTableMethodShim {
            principal: principal(),
            self_ty: self_ty(),
            vtable_entry_index: 5,
            method_def_path: def_hash(0xCA11, 0xA),
        };

        assert_ne!(first, second);
    }

    #[test]
    fn vtable_drop_shim_is_distinct_from_vtable_instance() {
        let principal = principal();
        let instance = SyntheticItemKind::VTableInstance {
            principal: principal.clone(),
            self_ty: self_ty(),
        };
        let drop_shim = SyntheticItemKind::VTableDropShim {
            principal,
            self_ty: self_ty(),
        };

        assert_ne!(drop_shim, instance);
    }

    #[test]
    fn const_memory_static_identity_uses_content_hash() {
        let first = SyntheticItemKind::ConstMemoryStatic {
            content_hash: content_hash(0xdead_beef_dead_beef_dead_beef_dead_beef),
        };
        let second = SyntheticItemKind::ConstMemoryStatic {
            content_hash: content_hash(0xdead_beef_dead_beef_dead_beef_dead_beee),
        };

        assert_ne!(first, second);
    }

    #[test]
    fn all_synthetic_item_kind_variants_are_pairwise_distinct() {
        let principal = principal();
        let variants = [
            SyntheticItemKind::None,
            SyntheticItemKind::ClosureCaptures {
                closure: closure_hash(),
            },
            SyntheticItemKind::ClosureCall {
                closure: closure_hash(),
                call_kind: ClosureCallKind::Call,
            },
            SyntheticItemKind::ClosureDropGlue {
                closure: closure_hash(),
            },
            SyntheticItemKind::VTableType {
                principal: principal.clone(),
            },
            SyntheticItemKind::VTableInstance {
                principal: principal.clone(),
                self_ty: self_ty(),
            },
            SyntheticItemKind::VTableMethodShim {
                principal: principal.clone(),
                self_ty: self_ty(),
                vtable_entry_index: 4,
                method_def_path: def_hash(0xCA11, 0xA),
            },
            SyntheticItemKind::VTableDropShim {
                principal,
                self_ty: self_ty(),
            },
            SyntheticItemKind::ConstMemoryStatic {
                content_hash: content_hash(0xCAFE),
            },
        ];

        for (idx, left) in variants.iter().enumerate() {
            for right in variants.iter().skip(idx + 1) {
                assert_ne!(left, right);
            }
        }
    }

    #[test]
    fn principal_trait_key_equality_and_hash_use_all_fields() {
        let base = principal();
        let same = principal();
        assert_eq!(base, same);
        assert_eq!(hash_of(&base), hash_of(&same));

        let different_path = principal_with_path(0x7A18);
        let different_args = PrincipalTraitKey {
            trait_args: ArgsKey(1),
            ..principal()
        };
        let different_bindings = PrincipalTraitKey {
            projection_bindings: ProjectionBindingsKey(1),
            ..principal()
        };

        assert_ne!(base, different_path);
        assert_ne!(base, different_args);
        assert_ne!(base, different_bindings);
    }

    #[test]
    fn projection_bindings_key_round_trips_raw_value() {
        let key = ProjectionBindingsKey(0x1234);
        assert_eq!(key.0, 0x1234);
        assert_eq!(key.clone(), key);
    }

    #[test]
    fn content_hash_round_trips_raw_value() {
        let hash = ContentHash(0x5678);
        assert_eq!(hash.0, 0x5678);
        assert_eq!(hash.clone(), hash);
    }

    #[test]
    fn every_synthetic_item_kind_variant_round_trips_through_serde() {
        let principal = principal();
        let variants = [
            SyntheticItemKind::None,
            SyntheticItemKind::ClosureCaptures {
                closure: closure_hash(),
            },
            SyntheticItemKind::ClosureCall {
                closure: closure_hash(),
                call_kind: ClosureCallKind::CallMut,
            },
            SyntheticItemKind::ClosureDropGlue {
                closure: closure_hash(),
            },
            SyntheticItemKind::VTableType {
                principal: principal.clone(),
            },
            SyntheticItemKind::VTableInstance {
                principal: principal.clone(),
                self_ty: self_ty(),
            },
            SyntheticItemKind::VTableMethodShim {
                principal: principal.clone(),
                self_ty: self_ty(),
                vtable_entry_index: 4,
                method_def_path: def_hash(0xCA11, 0xA),
            },
            SyntheticItemKind::VTableDropShim {
                principal,
                self_ty: self_ty(),
            },
            SyntheticItemKind::ConstMemoryStatic {
                content_hash: content_hash(0xCAFE),
            },
        ];

        for variant in variants {
            let json = serde_json::to_string(&variant).unwrap();
            assert_eq!(
                serde_json::from_str::<SyntheticItemKind>(&json).unwrap(),
                variant
            );
        }
    }

    #[test]
    fn synthetic_key_types_round_trip_through_serde() {
        let key = principal();
        let bindings = ProjectionBindingsKey(0x42);
        let content = ContentHash(0x43);

        let json = serde_json::to_string(&key).unwrap();
        assert_eq!(
            serde_json::from_str::<PrincipalTraitKey>(&json).unwrap(),
            key
        );
        let json = serde_json::to_string(&bindings).unwrap();
        assert_eq!(
            serde_json::from_str::<ProjectionBindingsKey>(&json).unwrap(),
            bindings
        );
        let json = serde_json::to_string(&content).unwrap();
        assert_eq!(serde_json::from_str::<ContentHash>(&json).unwrap(), content);
    }
}
