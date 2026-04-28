//! PinCtrl intrinsic metadata extraction for Pass 2 (task
//! regseq-model-52).
//!
//! At Pass 1 finalization, [`PinStateLookup::build`] walks the
//! populated root module, finds every TypeDef whose `Attrs` bag
//! carries a well-formed `#[rseq::pin_state(id = "...")]` entry (the
//! bag is populated by [`super::attrs::collect_rseq_attrs`] during
//! the HIR walk), and interns each `id` string into the Pass 1
//! [`SymbolInterner`] so Pass 2 can attach
//! [`IntrinsicMetadata::PinCtrl`](regseq_model_old::ir::IntrinsicMetadata::PinCtrl)
//! to every `PinCtrlApply` `IntrinsicCall` without re-reading the
//! attribute at each call site.
//!
//! At Pass 2 terminator translation, [`resolve_pinctrl_metadata`]
//! recovers the monomorphized `S` type parameter from the call's
//! `Instance::args`, looks it up in the [`PinStateLookup`] produced
//! by Pass 1, and returns the resolved `state_id` [`Symbol`].
//! Well-formed source always resolves; any call that reaches a
//! marker whose TypeDef has no `pin_state` entry trips
//! [`ConstructionError::MissingPinStateAttribute`] as a defensive
//! guard against extraction surprises (Phase 4.2 already rejects
//! malformed attributes earlier, so a `None` result here implies
//! the driver supplied a type that was not annotated).

use std::collections::HashMap;

use rustc_hir::def_id::DefId;
use rustc_middle::ty::{self, TyCtxt};
use rustc_span::Span;

use regseq_model_old::identity::{Symbol, SymbolInterner, TypeDefId};
use regseq_model_old::ir::{IntrinsicMetadata, RegseqIntrinsic};
use regseq_model_old::model::{AttrValue, Attrs, Module};

use super::{ConstructionError, ErrorProvenance};

// ---------------------------------------------------------------------------
// PinStateLookup
// ---------------------------------------------------------------------------

/// Pass 1 cross-reference table mapping each pin-state marker
/// type's rustc [`DefId`] to its interned `state_id` [`Symbol`].
///
/// Built once at Pass 1 finalization and consumed by Pass 2's
/// [`resolve_pinctrl_metadata`]. The symbol is interned in Pass 1's
/// [`SymbolInterner`]; callers must resolve it through the SAME
/// interner Pass 1 used to build the lookup (mirrors the
/// convention used by
/// [`RegisterFieldMeta::name`](regseq_model_old::ir::RegisterFieldMeta)).
#[derive(Debug, Default, Clone)]
pub struct PinStateLookup {
    entries: HashMap<DefId, Symbol>,
}

impl PinStateLookup {
    /// Build the lookup by walking `root` (recursively into
    /// submodules) and interning every TypeDef's
    /// `#[rseq::pin_state(id = "...")]` entry. `type_defs` is the
    /// Pass 1 `DefId -> TypeDefId` cross-reference map used to
    /// resolve each TypeDef back to its rustc `DefId`.
    pub fn build(
        root: &Module,
        type_defs: &HashMap<DefId, TypeDefId>,
        interner: &mut SymbolInterner,
    ) -> Self {
        let reverse: HashMap<TypeDefId, DefId> =
            type_defs.iter().map(|(&did, &tid)| (tid, did)).collect();
        let mut entries: HashMap<DefId, Symbol> = HashMap::new();
        collect_entries(root, &reverse, interner, &mut entries);
        Self { entries }
    }

    /// Insert a pre-built `(DefId, Symbol)` entry. Used internally by
    /// [`Self::build`] and by tests that synthesize lookups from
    /// hand-built entries.
    pub fn insert(&mut self, marker: DefId, state_id: Symbol) {
        self.entries.insert(marker, state_id);
    }

    /// Resolve a pin-state marker type's [`DefId`] to its interned
    /// `state_id` [`Symbol`]. Returns [`None`] when the marker has
    /// no `#[rseq::pin_state(id = "...")]` entry in its TypeDef's
    /// attrs bag.
    pub fn lookup(&self, marker: DefId) -> Option<Symbol> {
        self.entries.get(&marker).copied()
    }

    /// Number of registered pin-state markers.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the lookup carries no entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

fn collect_entries(
    module: &Module,
    reverse: &HashMap<TypeDefId, DefId>,
    interner: &mut SymbolInterner,
    out: &mut HashMap<DefId, Symbol>,
) {
    for td in &module.type_defs {
        let Some(&def_id) = reverse.get(&td.id) else {
            continue;
        };
        let Some(id) = extract_state_id(&td.attrs) else {
            continue;
        };
        let sym = interner.intern(&id);
        out.insert(def_id, sym);
    }
    for sub in &module.submodules {
        collect_entries(sub, reverse, interner, out);
    }
}

/// Pull the `id` string out of a TypeDef's attrs bag if it carries a
/// well-formed `#[rseq::pin_state(id = "...")]` entry. Returns
/// [`None`] when the attribute is absent or its payload is not the
/// expected `KeyValues -> id: String(...)` shape.
fn extract_state_id(attrs: &Attrs) -> Option<String> {
    for entry in &attrs.entries {
        if entry.key != "rseq::pin_state" {
            continue;
        }
        let AttrValue::KeyValues(kvs) = &entry.value else {
            return None;
        };
        for kv in kvs {
            if kv.key == "id" {
                if let AttrValue::String(s) = &kv.value {
                    return Some(s.clone());
                }
            }
        }
        return None;
    }
    None
}

// ---------------------------------------------------------------------------
// Pass 2 metadata resolution
// ---------------------------------------------------------------------------

/// Outcome of [`resolve_pinctrl_metadata`]: the structured
/// metadata to attach to the [`IntrinsicCall`] terminator.
pub(crate) struct PinCtrlMetadataResolution {
    pub metadata: IntrinsicMetadata,
}

/// Whether `intrinsic` is the single pin-control apply intrinsic.
pub(crate) fn is_pinctrl_apply_intrinsic(intrinsic: RegseqIntrinsic) -> bool {
    matches!(intrinsic, RegseqIntrinsic::PinCtrlApply)
}

/// Run PinCtrl metadata extraction for a `PinCtrlApply` intrinsic
/// call. `generic_args` is the callee's monomorphized generic
/// argument list (see `callee_generic_args` in [`super::pass2`]).
/// `PinCtrl::apply<S>(&self)`'s own generics contain one type
/// parameter `S`, so the concrete `S` is recovered by reading the
/// substitution slot that corresponds to the callee's first own
/// type parameter (skipping any parent-impl substs). This avoids
/// the generality loss of "first ADT" heuristics when future
/// signatures add parent-impl type parameters.
///
/// Returns [`ConstructionError::MissingPinStateAttribute`] when
/// either `S` cannot be recovered as an ADT or its `DefId` has no
/// entry in `pin_state_lookup`.
pub(crate) fn resolve_pinctrl_metadata<'tcx>(
    tcx: TyCtxt<'tcx>,
    callee_def_id: DefId,
    generic_args: ty::GenericArgsRef<'tcx>,
    pin_state_lookup: &PinStateLookup,
    span: Span,
) -> Result<PinCtrlMetadataResolution, ConstructionError> {
    let marker_def_id =
        resolve_own_type_param_adt(tcx, callee_def_id, generic_args).ok_or_else(|| {
            ConstructionError::MissingPinStateAttribute {
                provenance: ErrorProvenance {
                    item_path: tcx.def_path_str(callee_def_id),
                    span,
                },
                marker: "<unresolved>".to_string(),
            }
        })?;

    let state_id = pin_state_lookup.lookup(marker_def_id).ok_or_else(|| {
        ConstructionError::MissingPinStateAttribute {
            provenance: ErrorProvenance::for_def_id(tcx, marker_def_id),
            marker: tcx.def_path_str(marker_def_id),
        }
    })?;

    Ok(PinCtrlMetadataResolution {
        metadata: IntrinsicMetadata::PinCtrl { state_id },
    })
}

/// Resolve the callee's first OWN type parameter to the ADT
/// [`DefId`] that monomorphization substituted for it. "Own"
/// meaning the parameter is declared on the callee itself (skip
/// parent-impl substitutions via `generics.parent_count`). For
/// `PinCtrl::apply<S>`, this yields `S`'s concrete marker
/// [`DefId`]. Returns [`None`] when the callee has no own type
/// parameter, when the corresponding substitution is not a `Type`
/// arg, or when the substituted type is not an ADT (pin-state
/// markers are always authored as struct/enum ADTs).
fn resolve_own_type_param_adt<'tcx>(
    tcx: TyCtxt<'tcx>,
    callee_def_id: DefId,
    args: ty::GenericArgsRef<'tcx>,
) -> Option<DefId> {
    let generics = tcx.generics_of(callee_def_id);
    for (i, param) in generics.own_params.iter().enumerate() {
        if !matches!(param.kind, ty::GenericParamDefKind::Type { .. }) {
            continue;
        }
        let abs_idx = generics.parent_count + i;
        let arg = args.get(abs_idx)?;
        let ty::GenericArgKind::Type(ty) = arg.kind() else {
            return None;
        };
        let ty::TyKind::Adt(adt_def, _) = ty.kind() else {
            return None;
        };
        return Some(adt_def.did());
    }
    None
}
