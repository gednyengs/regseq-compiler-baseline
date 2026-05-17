//! Content-keyed function signature interner.

use std::collections::HashMap;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::fn_sig::FnSig;
use crate::ids::FnSigId;
use crate::index_vec::IndexVec;

/// Content-keyed function signature interner.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FnSigInterner {
    storage: IndexVec<FnSigId, FnSig>,
    lookup: HashMap<FnSig, FnSigId>,
}

impl FnSigInterner {
    /// Returns an empty function signature interner.
    pub fn new() -> Self {
        Self {
            storage: IndexVec::new(),
            lookup: HashMap::new(),
        }
    }

    /// Builds an interner from raw storage without rebuilding its lookup map.
    ///
    /// This exists for validator corruption fixtures that must model a
    /// tampered deserialized table. Normal construction must use
    /// `FnSigInterner::new` and `FnSigInterner::intern`.
    pub fn from_storage_unchecked_for_validation(storage: IndexVec<FnSigId, FnSig>) -> Self {
        Self {
            storage,
            lookup: HashMap::new(),
        }
    }

    /// Interns `sig` and returns the content identity's `FnSigId`.
    pub fn intern(&mut self, sig: FnSig) -> FnSigId {
        if let Some(id) = self.lookup.get(&sig) {
            return *id;
        }
        let id = self.storage.push(sig.clone());
        self.lookup.insert(sig, id);
        id
    }

    /// Resolves `id` to its stored function signature.
    pub fn resolve(&self, id: FnSigId) -> &FnSig {
        &self.storage[id]
    }

    /// Finds an interned function signature without mutating.
    pub fn find(&self, sig: &FnSig) -> Option<FnSigId> {
        self.lookup.get(sig).copied()
    }

    /// Number of stored function signature identities.
    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        self.storage.len()
    }

    /// Iterator over `(FnSigId, &FnSig)` in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = (FnSigId, &FnSig)> {
        self.storage
            .iter()
            .enumerate()
            .map(|(idx, sig)| (FnSigId::from_u32(idx as u32), sig))
    }

    /// Apply `f` to every stored `FnSig` entry by mutable reference,
    /// without touching the content-keyed lookup map. Callers must
    /// invoke `rebuild_lookup` after the walk if mutations change
    /// any entry's identity key (e.g., a `TyId` substitution inside
    /// `FnSig.inputs` or `FnSig.output`).
    pub(crate) fn for_each_fn_sig_mut<F: FnMut(&mut FnSig)>(&mut self, mut f: F) {
        for sig in self.storage.iter_mut() {
            f(sig);
        }
    }

    /// Rebuild the content-keyed lookup map from the current
    /// storage. Used after mutations that change one or more
    /// entries' identity keys (`for_each_fn_sig_mut`,
    /// `compact_by_keep`). Asserts identity-key uniqueness in debug
    /// builds.
    pub(crate) fn rebuild_lookup(&mut self) {
        self.lookup.clear();
        for (idx, sig) in self.storage.iter().enumerate() {
            let prev = self
                .lookup
                .insert(sig.clone(), FnSigId::from_u32(idx as u32));
            debug_assert!(
                prev.is_none(),
                "FnSigInterner identity collision at FnSigId {}",
                idx,
            );
        }
    }

    /// Compact storage in place, retaining only entries where
    /// `keep[idx]` is `true`. Clears the content-keyed lookup map
    /// (callers MUST invoke `rebuild_lookup` after remapping every
    /// surviving entry's nested `TyId` references through the
    /// returned remap).
    ///
    /// Returns the old-to-new `FnSigId` remap as
    /// `Vec<Option<FnSigId>>` indexed by the old `FnSigId`'s
    /// `index()`. `None` marks a removed entry; `Some(new_id)`
    /// marks a surviving entry's new index.
    ///
    /// Panics if `keep.len() != storage.len()`.
    pub(crate) fn compact_by_keep(&mut self, keep: &[bool]) -> Vec<Option<FnSigId>> {
        assert_eq!(
            keep.len(),
            self.storage.len(),
            "FnSigInterner::compact_by_keep keep slice length must match storage length",
        );
        let old_storage = std::mem::take(&mut self.storage).into_inner();
        let mut new_storage: Vec<FnSig> = Vec::with_capacity(old_storage.len());
        let mut remap: Vec<Option<FnSigId>> = Vec::with_capacity(old_storage.len());
        for (idx, sig) in old_storage.into_iter().enumerate() {
            if keep[idx] {
                let new_idx = FnSigId::from_u32(new_storage.len() as u32);
                new_storage.push(sig);
                remap.push(Some(new_idx));
            } else {
                remap.push(None);
            }
        }
        self.storage.replace_inner(new_storage);
        self.lookup.clear();
        remap
    }
}

impl Default for FnSigInterner {
    fn default() -> Self {
        Self::new()
    }
}

impl Serialize for FnSigInterner {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.storage.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for FnSigInterner {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let storage = IndexVec::<FnSigId, FnSig>::deserialize(deserializer)?;
        let mut lookup = HashMap::new();
        for (idx, sig) in storage.iter().enumerate() {
            let id = FnSigId::from_u32(idx as u32);
            if let Some(first) = lookup.insert(sig.clone(), id) {
                return Err(serde::de::Error::custom(format!(
                    "duplicate FnSig identity at FnSigId {} and FnSigId {}",
                    first.as_u32(),
                    id.as_u32()
                )));
            }
        }

        Ok(Self { storage, lookup })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cabi::{CAbiLowering, CAbiShape, SubstReason};
    use crate::ty::{FnAbi, Safety, Ty, UintKind};
    use crate::{RegseqModel, StableCrateId, TyId};

    fn ty(value: u32) -> TyId {
        TyId::from_u32(value)
    }

    fn sig(inputs: Vec<TyId>, output: TyId) -> FnSig {
        FnSig {
            inputs,
            output,
            abi: FnAbi::Rust,
            c_variadic: false,
            safety: Safety::Safe,
        }
    }

    #[test]
    fn new_returns_empty_interner() {
        let interner = FnSigInterner::new();
        assert_eq!(interner.len(), 0);
        assert_eq!(interner.iter().count(), 0);
    }

    #[test]
    fn intern_deduplicates_resolves_finds_and_iterates() {
        let mut interner = FnSigInterner::new();
        let value = sig(vec![ty(0)], ty(1));

        let first = interner.intern(value.clone());
        let second = interner.intern(value.clone());

        assert_eq!(first, FnSigId::ZERO);
        assert_eq!(first, second);
        assert_eq!(interner.resolve(first), &value);
        assert_eq!(interner.find(&value), Some(first));
        assert_eq!(interner.len(), 1);
        assert_eq!(interner.iter().collect::<Vec<_>>(), vec![(first, &value)]);
    }

    #[test]
    fn intern_keeps_each_schema_distinction() {
        let mut interner = FnSigInterner::new();
        let base = sig(vec![ty(0)], ty(1));
        let base_id = interner.intern(base.clone());

        let mut abi = base.clone();
        abi.abi = FnAbi::C;
        let abi_id = interner.intern(abi);

        let mut variadic = base.clone();
        variadic.c_variadic = true;
        let variadic_id = interner.intern(variadic);

        let mut safety = base.clone();
        safety.safety = Safety::Unsafe;
        let safety_id = interner.intern(safety);

        let input_order_id = interner.intern(sig(vec![ty(1), ty(0)], ty(1)));
        let output_id = interner.intern(sig(vec![ty(0)], ty(0)));

        assert_eq!(base_id, FnSigId::ZERO);
        assert_ne!(base_id, abi_id);
        assert_ne!(base_id, variadic_id);
        assert_ne!(base_id, safety_id);
        assert_ne!(base_id, input_order_id);
        assert_ne!(base_id, output_id);
        assert_eq!(interner.len(), 6);
    }

    #[test]
    fn serde_round_trip_preserves_storage_order_and_lookup() {
        let mut interner = FnSigInterner::new();
        let values = vec![
            sig(vec![], ty(0)),
            sig(vec![ty(0)], ty(0)),
            FnSig {
                abi: FnAbi::C,
                ..sig(vec![ty(0)], ty(0))
            },
            FnSig {
                c_variadic: true,
                ..sig(vec![ty(0)], ty(0))
            },
            FnSig {
                safety: Safety::Unsafe,
                ..sig(vec![], ty(0))
            },
        ];
        for value in &values {
            interner.intern(value.clone());
        }

        let json = serde_json::to_string(&interner).unwrap();
        let round_trip = serde_json::from_str::<FnSigInterner>(&json).unwrap();

        assert_eq!(round_trip.len(), values.len());
        for (idx, value) in values.iter().enumerate() {
            let id = FnSigId::from_u32(idx as u32);
            assert_eq!(round_trip.resolve(id), value);
            assert_eq!(round_trip.find(value), Some(id));
        }
        assert_eq!(
            round_trip.iter().map(|(id, _)| id).collect::<Vec<_>>(),
            vec![
                FnSigId::from_u32(0),
                FnSigId::from_u32(1),
                FnSigId::from_u32(2),
                FnSigId::from_u32(3),
                FnSigId::from_u32(4)
            ]
        );
    }

    #[test]
    fn serde_rejects_duplicate_canonical_content() {
        let value = sig(vec![ty(0)], ty(1));
        let json = serde_json::to_string(&vec![value.clone(), value]).unwrap();
        let err = serde_json::from_str::<FnSigInterner>(&json).unwrap_err();
        assert!(err.to_string().contains("duplicate FnSig identity"));
    }

    #[test]
    fn serde_accepts_dangling_ty_ids_for_later_model_validation() {
        let value = sig(vec![ty(99)], ty(0));
        let json = serde_json::to_string(&vec![value.clone()]).unwrap();
        let interner = serde_json::from_str::<FnSigInterner>(&json).unwrap();
        assert_eq!(interner.resolve(FnSigId::ZERO), &value);
    }

    #[test]
    fn handle_lowering_preserves_signature_identity() {
        let mut model = RegseqModel::with_primary("primary", StableCrateId(1));
        let handle_ty = model.types.intern(Ty::Struct {
            item: crate::ItemId::ZERO,
            args: vec![],
            layout: crate::UNIT_LAYOUT,
        });
        let u32_ty = model.types.intern(Ty::Uint(UintKind::U32));

        let handle_sig = FnSig {
            inputs: vec![handle_ty],
            output: handle_ty,
            abi: FnAbi::C,
            c_variadic: false,
            safety: Safety::Safe,
        };
        let u32_sig = FnSig {
            inputs: vec![u32_ty],
            output: u32_ty,
            abi: FnAbi::C,
            c_variadic: false,
            safety: Safety::Safe,
        };

        let handle_id = model.sigs.intern(handle_sig);
        model.sig_c_abi_lowerings.push(Some(CAbiLowering {
            inputs: vec![CAbiShape::Substituted {
                emit_as: u32_ty,
                reason: SubstReason::TransparentChain,
            }],
            output: CAbiShape::Substituted {
                emit_as: u32_ty,
                reason: SubstReason::TransparentChain,
            },
        }));
        let u32_id = model.sigs.intern(u32_sig);
        model.sig_c_abi_lowerings.push(Some(CAbiLowering {
            inputs: vec![CAbiShape::Direct],
            output: CAbiShape::Direct,
        }));

        assert_eq!(model.sigs.resolve(handle_id).inputs, vec![handle_ty]);
        assert_eq!(model.sigs.resolve(handle_id).output, handle_ty);
        assert_eq!(
            model.sig_c_abi_lowerings[handle_id],
            Some(CAbiLowering {
                inputs: vec![CAbiShape::Substituted {
                    emit_as: u32_ty,
                    reason: SubstReason::TransparentChain,
                }],
                output: CAbiShape::Substituted {
                    emit_as: u32_ty,
                    reason: SubstReason::TransparentChain,
                },
            })
        );
        assert_ne!(handle_id, u32_id);
        assert_eq!(
            model.sig_c_abi_lowerings[u32_id],
            Some(CAbiLowering {
                inputs: vec![CAbiShape::Direct],
                output: CAbiShape::Direct,
            })
        );
    }
}
