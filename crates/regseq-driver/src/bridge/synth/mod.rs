//! Phase-2 synthesis entry points for bridge-only Items.

use regseq_model::{PrincipalTraitKey, TyId};

use super::{BridgeCtxt, BridgeError, ItemHandle};

pub mod closure;
pub mod drop_glue;
pub mod vtable;

/// Phase-2 handles for the closure synthetic item family.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClosureSynthHandles {
    /// Rustc DefPathHash of the closure expression.
    pub closure: regseq_model::DefPathHash,
    /// Captures-struct item handle.
    pub captures: ItemHandle,
    /// Closure call-method function item handles.
    pub calls: Vec<ItemHandle>,
    /// Closure drop-glue function item handle when captures need drop.
    pub drop_glue: Option<ItemHandle>,
}

/// Phase-2 handles for the vtable synthetic item family.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VTableSynthHandles {
    /// Vtable type item handle.
    pub vtable_type: ItemHandle,
    /// Vtable instance static item handle.
    pub vtable_instance: ItemHandle,
    /// Vtable method shim function item handles.
    pub method_shims: Vec<ItemHandle>,
    /// Vtable drop shim function item handle.
    pub drop_shim: ItemHandle,
}

/// Cache key for vtable instance and shim synthesis.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct VTableSynthKey {
    /// Principal trait key for the trait-object type.
    pub principal: PrincipalTraitKey,
    /// Canonical self type key for the concrete implementation.
    pub self_ty: regseq_model::ArgsKey,
}

/// Synthesise the closure family for a closure mono-item. Records
/// three handles (ClosureCaptures + ClosureCall + ClosureDropGlue) in
/// the closure working cache. Per spec.md section 13.1.
pub fn synthesise_closure<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    closure_def: rustc_middle::ty::ClosureArgs<rustc_middle::ty::TyCtxt<'tcx>>,
) -> Result<ClosureSynthHandles, BridgeError> {
    closure::synthesise_closure(cx, closure_def)
}

/// Synthesise the vtable family for a (Trait, ImplTy) pair. Records
/// three handles (VTableType + VTableInstance + VTableShim) in the
/// vtable working cache and dedups on PrincipalTraitKey per spec.md
/// section 13.3. Per spec.md section 13.2.
pub fn synthesise_vtable<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    principal: PrincipalTraitKey,
    impl_ty: TyId,
) -> Result<VTableSynthHandles, BridgeError> {
    vtable::synthesise_vtable(cx, principal, impl_ty)
}

/// Synthesise a drop-glue Function Item for a non-Copy Ty. Records a
/// handle in the drop-glue working cache. Consumed by
/// regseq-driver-42's Drop-as-Call collapse.
pub fn synthesise_drop_glue<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    drop_target_ty: TyId,
) -> Result<ItemHandle, BridgeError> {
    drop_glue::synthesise_drop_glue(cx, drop_target_ty)
}

/// Phase-5 re-key pass. Walks every per-family working cache,
/// resolves ItemHandle to ItemId via the HandleTable, and populates
/// RegseqModel.vtables for vtable instance Items.
pub fn rekey_synthesis_in_phase5(cx: &mut BridgeCtxt<'_>) -> Result<(), BridgeError> {
    for handles in &cx.closure_synth_cache {
        resolve_handle(cx, handles.captures, "ClosureCaptures")?;
        for call in &handles.calls {
            resolve_handle(cx, *call, "ClosureCall")?;
        }
        if let Some(drop_glue) = handles.drop_glue {
            resolve_handle(cx, drop_glue, "ClosureDropGlue")?;
        }
    }

    for handle in cx.vtable_type_synth_cache.values().copied() {
        resolve_handle(cx, handle, "VTableType")?;
    }

    let mut vtable_entries = cx.vtable_synth_cache.iter().collect::<Vec<_>>();
    vtable_entries.sort_by_key(|(key, _)| {
        (
            key.principal.trait_path.stable_crate_id,
            key.principal.trait_path.local_hash,
            key.principal.trait_args,
            key.principal.projection_bindings.0,
            key.self_ty,
        )
    });
    for (key, handles) in vtable_entries {
        let vtable_type = resolve_handle(cx, handles.vtable_type, "VTableType")?;
        resolve_handle(cx, handles.vtable_instance, "VTableInstance")?;
        for method_shim in &handles.method_shims {
            resolve_handle(cx, *method_shim, "VTableMethodShim")?;
        }
        resolve_handle(cx, handles.drop_shim, "VTableDropShim")?;
        cx.model.vtables.insert(key.principal.clone(), vtable_type);
    }

    for handle in cx.drop_glue_synth_cache.values().copied() {
        resolve_handle(cx, handle, "DropGlue")?;
    }

    Ok(())
}

pub(crate) fn resolve_handle(
    cx: &BridgeCtxt<'_>,
    handle: ItemHandle,
    family: &'static str,
) -> Result<regseq_model::ItemId, BridgeError> {
    cx.handles.resolve(handle).ok_or_else(|| {
        BridgeError::Phase4(format!(
            "{family} handle {} was not assigned an ItemId",
            handle.as_u32()
        ))
    })
}
