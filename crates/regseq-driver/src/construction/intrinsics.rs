//! Rule 1 Regseq intrinsic recognition for Pass 2.
//!
//! Implemented by task regseq-model-45.
//!
//! At Pass 2 setup, the [`IntrinsicLookup`] is built once by parsing every
//! canonical-name entry in the Phase 4 [`IntrinsicRegistry`] through
//! [`parse_canonical_intrinsic_name`]. During terminator translation,
//! a `Call` whose callee `DefId` resolves in the lookup becomes an
//! `IntrinsicCall`; unregistered callees fall through to `Call`.

use std::collections::HashMap;

use rustc_hir::def_id::DefId;
use rustc_middle::ty::TyCtxt;

use regseq_model::ir::{parse_canonical_intrinsic_name, RegseqIntrinsic};

use crate::extract::intrinsic_registry::IntrinsicRegistry;

use super::{ConstructionError, ErrorProvenance};

/// Pass 2 lookup from rustc [`DefId`] to [`RegseqIntrinsic`].
///
/// Built at Pass 2 setup by parsing every canonical-name entry in the
/// Phase 4 [`IntrinsicRegistry`] through
/// [`parse_canonical_intrinsic_name`]. Once constructed, a `Call`
/// terminator whose callee `DefId` resolves through [`Self::lookup`]
/// becomes an `IntrinsicCall`; unresolved callees fall through to
/// `Call`.
#[derive(Debug, Default, Clone)]
pub struct IntrinsicLookup {
    entries: HashMap<DefId, RegseqIntrinsic>,
}

impl IntrinsicLookup {
    /// Build the lookup from the Phase 4 [`IntrinsicRegistry`].
    ///
    /// Delegates to [`Self::from_entries`] after projecting the registry
    /// to its `(DefId, &str)` entries.
    pub fn build(tcx: TyCtxt<'_>, registry: &IntrinsicRegistry) -> Result<Self, ConstructionError> {
        Self::from_entries(tcx, registry.iter())
    }

    /// Build the lookup from an explicit iterator of `(DefId, canonical)`
    /// pairs.
    ///
    /// Every canonical-name string must parse through
    /// [`parse_canonical_intrinsic_name`]; any unknown name yields
    /// [`ConstructionError::UnknownIntrinsicCanonicalName`] at setup
    /// rather than silently falling through per call site. This
    /// variant is used directly by tests that construct synthetic
    /// registries without going through the HIR walker.
    pub fn from_entries<'a, I>(tcx: TyCtxt<'_>, entries: I) -> Result<Self, ConstructionError>
    where
        I: IntoIterator<Item = (DefId, &'a str)>,
    {
        let iter = entries.into_iter();
        let mut out: HashMap<DefId, RegseqIntrinsic> = HashMap::new();
        for (def_id, canonical) in iter {
            match parse_canonical_intrinsic_name(canonical) {
                Some(variant) => {
                    out.insert(def_id, variant);
                }
                None => {
                    return Err(ConstructionError::UnknownIntrinsicCanonicalName {
                        provenance: ErrorProvenance::for_def_id(tcx, def_id),
                        canonical: canonical.to_string(),
                    });
                }
            }
        }
        Ok(Self { entries: out })
    }

    /// Resolve a callee `DefId` to its [`RegseqIntrinsic`] variant, if
    /// the callee is a registered intrinsic.
    pub fn lookup(&self, def_id: DefId) -> Option<RegseqIntrinsic> {
        self.entries.get(&def_id).copied()
    }

    /// Number of registered intrinsics.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the lookup contains no entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

// All tests in this module exercise post-HIR/MIR output (constructed model,
// JSON serialization, snapshots). Voided pending MonoRust + RegseqModel
// redesign. See docs/discussions/compiler-refactoring/test-audit.md.
#[cfg(all(test, feature = "post-monorust-tests"))]
mod tests {
    use super::*;

    #[test]
    fn empty_lookup_has_no_entries() {
        let lookup = IntrinsicLookup::default();
        assert!(lookup.is_empty());
        assert_eq!(lookup.len(), 0);
    }
}
