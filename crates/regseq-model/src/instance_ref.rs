//! Canonical reference to an instance from a const relocation.

use serde::{Deserialize, Serialize};

use crate::args_key::ArgsKey;
use crate::def_path::DefPathHash;

/// Canonical reference to a monomorphized instance.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct InstanceRef {
    /// Stable rank of the Regseq instance kind.
    pub regseq_kind_rank: u8,
    /// Cross-build identity of the source or synthesized item.
    pub def_path_hash: DefPathHash,
    /// Deep canonical key for the instance generic arguments.
    pub args_key: ArgsKey,
    /// Stable discriminator for function shims.
    pub fn_shim_disambig: u8,
}

impl InstanceRef {
    /// Returns the 34-byte canonical encoding for this reference.
    pub fn canonical_bytes(self) -> [u8; 34] {
        let mut out = [0u8; 34];
        out[0] = self.regseq_kind_rank;
        out[1..9].copy_from_slice(&self.def_path_hash.stable_crate_id.0.to_le_bytes());
        out[9..17].copy_from_slice(&self.def_path_hash.local_hash.to_le_bytes());
        out[17..33].copy_from_slice(&self.args_key.0.to_be_bytes());
        out[33] = self.fn_shim_disambig;
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{regseq_instance_kind_rank, InstanceKind, StableCrateId};

    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    fn instance_ref(rank: u8) -> InstanceRef {
        InstanceRef {
            regseq_kind_rank: rank,
            def_path_hash: DefPathHash {
                stable_crate_id: StableCrateId(0xAABB),
                local_hash: 0xCCDD,
            },
            args_key: ArgsKey(0xEEFF),
            fn_shim_disambig: 5,
        }
    }

    fn hash<T: Hash>(value: &T) -> u64 {
        let mut hasher = DefaultHasher::new();
        value.hash(&mut hasher);
        hasher.finish()
    }

    #[test]
    fn instance_ref_constructs_for_each_regseq_rank() {
        let ranks = [
            regseq_instance_kind_rank(&InstanceKind::Function {
                sig: crate::FnSigId::ZERO,
                body: None,
            }),
            regseq_instance_kind_rank(&InstanceKind::Static {
                ty: crate::TyId::ZERO,
                init: crate::ConstId::ZERO,
            }),
            regseq_instance_kind_rank(&InstanceKind::Const {
                ty: crate::TyId::ZERO,
                value: crate::ConstId::ZERO,
            }),
            regseq_instance_kind_rank(&InstanceKind::Struct {
                layout: crate::TyId::ZERO,
            }),
            regseq_instance_kind_rank(&InstanceKind::Enum {
                layout: crate::TyId::ZERO,
            }),
            regseq_instance_kind_rank(&InstanceKind::Union {
                layout: crate::TyId::ZERO,
            }),
        ];

        assert_eq!(ranks, [0, 1, 2, 3, 4, 5]);
        for rank in ranks {
            assert_eq!(instance_ref(rank).regseq_kind_rank, rank);
        }
    }

    #[test]
    fn instance_ref_is_copy_and_hashes_by_value() {
        let a = instance_ref(0);
        let b = a;
        let c = a;
        assert_eq!(a, b);
        assert_eq!(hash(&a), hash(&c));
    }

    #[test]
    fn instance_ref_round_trips_through_serde() {
        let value = instance_ref(2);
        let json = serde_json::to_string(&value).unwrap();
        assert_eq!(serde_json::from_str::<InstanceRef>(&json).unwrap(), value);
    }

    #[test]
    fn instance_ref_orders_lexicographically() {
        let mut values = vec![
            InstanceRef {
                regseq_kind_rank: 1,
                ..instance_ref(0)
            },
            InstanceRef {
                args_key: ArgsKey(0),
                ..instance_ref(0)
            },
            instance_ref(0),
        ];
        values.sort();
        assert_eq!(values[0].args_key, ArgsKey(0));
        assert_eq!(values[1], instance_ref(0));
        assert_eq!(values[2].regseq_kind_rank, 1);
    }

    #[test]
    fn canonical_bytes_match_34_byte_spec_shape() {
        let value = instance_ref(0);
        let bytes = value.canonical_bytes();
        let mut expected = vec![0x00];
        expected.extend_from_slice(&0xAABB_u64.to_le_bytes());
        expected.extend_from_slice(&0xCCDD_u64.to_le_bytes());
        expected.extend_from_slice(&0xEEFF_u128.to_be_bytes());
        expected.push(0x05);

        assert_eq!(bytes.len(), 34);
        assert_eq!(bytes.as_slice(), expected.as_slice());
    }

    #[test]
    fn canonical_bytes_encode_args_key_big_endian() {
        let value = InstanceRef {
            args_key: ArgsKey(0x0123_4567_89AB_CDEF_FEDC_BA98_7654_3210),
            ..instance_ref(0)
        };
        assert_eq!(
            &value.canonical_bytes()[17..33],
            &[
                0x01, 0x23, 0x45, 0x67, 0x89, 0xAB, 0xCD, 0xEF, 0xFE, 0xDC, 0xBA, 0x98, 0x76, 0x54,
                0x32, 0x10,
            ]
        );
    }
}
