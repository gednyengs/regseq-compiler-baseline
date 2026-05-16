//! Phase-2 synthesis entry points for bridge-only Items.

use regseq_model::{DefPathHash, Layout, PrincipalTraitKey, TyId};

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
    /// Vtable method shim function item handles, in vtable-entry order.
    pub method_shims: Vec<ItemHandle>,
    /// Vtable drop shim function item handle. `None` when `impl_ty`
    /// has trivial Drop (the MetadataDropInPlace slot becomes a null
    /// pointer per spec.md section 13.2 and the
    /// `vtable_allocation_provider` rule in
    /// `rustc_middle/src/ty/vtable.rs`).
    pub drop_shim: Option<ItemHandle>,
    /// Per-slot composition for the Phase-5 VTableInstance Static
    /// init aggregate. Empty for the synthetic test path; populated
    /// from `tcx.vtable_entries` for the rustc-aware Cast::Unsize path.
    pub(crate) slots: Vec<VTableSlot>,
    /// Cached vtable struct layout used to intern the `Ty::Struct`
    /// view of the vtable type during Phase 5.
    pub(crate) layout: Option<Layout>,
}

/// Cache key for vtable instance and shim synthesis.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct VTableSynthKey {
    /// Principal trait key for the trait-object type.
    pub principal: PrincipalTraitKey,
    /// Canonical self type key for the concrete implementation.
    pub self_ty: regseq_model::ArgsKey,
}

/// Per-slot composition for a vtable instance Static. Each VTableSlot
/// encodes one rustc `VtblEntry` variant per spec.md section 13.2 and
/// the `vtable_allocation_provider` rule in
/// `rustc_middle/src/ty/vtable.rs`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum VTableSlot {
    /// MetadataDropInPlace slot. `Some(handle)` when the impl_ty has
    /// non-trivial Drop; `None` otherwise (the slot becomes a null
    /// pointer cell).
    Drop {
        /// Drop shim handle when impl_ty needs drop.
        shim_handle: Option<ItemHandle>,
    },
    /// MetadataSize slot (target-endian usize bytes of impl_ty).
    Size {
        /// Size in bytes.
        bytes: u64,
    },
    /// MetadataAlign slot (target-endian usize bytes of impl_ty).
    Align {
        /// Alignment in bytes.
        bytes: u64,
    },
    /// Method shim slot.
    Method {
        /// Method shim function-item handle.
        handle: ItemHandle,
        /// Source method DefPathHash from the rustc Instance.
        method_def_path: DefPathHash,
        /// Slot index in `tcx.vtable_entries` order.
        vtable_entry_index: u32,
        /// Source method name for vtable struct field naming.
        method_name: String,
    },
    /// Vacant slot (non-dispatchable method).
    Vacant,
    /// Supertrait vtable pointer slot.
    TraitVPtr {
        /// Principal of the supertrait.
        supertrait_principal: PrincipalTraitKey,
        /// Source supertrait short name for vtable struct field naming.
        supertrait_name: String,
    },
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

/// Synthesise the vtable family (type + instance + drop shim) for a
/// (Trait, ImplTy) pair using the synthetic test path. Records
/// handles in the vtable working cache and dedups on
/// `(PrincipalTraitKey, ImplTy)` per spec.md section 13.3. No method
/// shims and no slot composition are recorded; the rustc-aware path
/// via `Cast::Unsize` is responsible for those.
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
/// resolves ItemHandle to ItemId via the HandleTable, populates
/// `RegseqModel.vtables` for vtable type Items per spec.md section
/// 13.3, and pushes VTableInstance Static instances per spec.md
/// section 13.2. The Static instances land at the end of the existing
/// `model.instances` table; per spec.md F4-2 (section 16.2) they
/// outrank Function instances pushed by regseq-driver-42, so the
/// final ordering after that step is naturally F4-2 conformant.
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

    // Per spec.md section 13.3, RegseqModel.vtables maps every
    // PrincipalTraitKey to its VTableType ItemId, regardless of
    // whether a VTableInstance was synthesised. Populate from the
    // standalone vtable-type cache; the vtable-instance pass below
    // cross-references the same map.
    let mut vtable_type_entries = cx
        .vtable_type_synth_cache
        .iter()
        .map(|(principal, handle)| (principal.clone(), *handle))
        .collect::<Vec<_>>();
    vtable_type_entries.sort_by_key(|(principal, _)| {
        (
            principal.trait_path.stable_crate_id,
            principal.trait_path.local_hash,
            principal.trait_args,
            principal.projection_bindings.0,
        )
    });
    for (principal, handle) in &vtable_type_entries {
        let item_id = resolve_handle(cx, *handle, "VTableType")?;
        cx.model.vtables.insert(principal.clone(), item_id);
    }

    // Phase-4 deferred-Ty resolution: patch every placeholder
    // `Ty::Struct` view recorded by
    // `bridge::ty::synthesize_dynamic_vtable_ty` to its real
    // `ItemId` per spec.md section 3.4 D-12-5.3.
    let pending = cx.dynamic_vtable_ty_pending.drain().collect::<Vec<_>>();
    for (handle, ty_id) in pending {
        let item_id = resolve_handle(cx, handle, "VTableType")?;
        cx.model.types.patch_nominal_item_id(ty_id, item_id);
    }

    let mut vtable_entries = cx
        .vtable_synth_cache
        .iter()
        .map(|(key, handles)| (key.clone(), handles.clone()))
        .collect::<Vec<_>>();
    vtable_entries.sort_by_key(|(key, _)| {
        (
            key.principal.trait_path.stable_crate_id,
            key.principal.trait_path.local_hash,
            key.principal.trait_args,
            key.principal.projection_bindings.0,
            key.self_ty,
        )
    });
    for (_, handles) in &vtable_entries {
        resolve_handle(cx, handles.vtable_type, "VTableType")?;
        resolve_handle(cx, handles.vtable_instance, "VTableInstance")?;
        for method_shim in &handles.method_shims {
            resolve_handle(cx, *method_shim, "VTableMethodShim")?;
        }
        if let Some(drop_shim) = handles.drop_shim {
            resolve_handle(cx, drop_shim, "VTableDropShim")?;
        }
    }

    for handle in cx.drop_glue_synth_cache.values().copied() {
        resolve_handle(cx, handle, "DropGlue")?;
    }

    // Phase-4 deferred-Ty resolution for source-derived nominal
    // entries (`Ty::Struct`, `Ty::Enum`, `Ty::Union`, `Ty::FnDef`,
    // `Ty::Closure`) interned during Phase 2 with placeholder
    // `ItemId`s. Per spec.md section 3.4 D-12-5.3 the final
    // `ItemId`s are bound at Phase 4; this pass patches every
    // placeholder `Ty` recorded in `cx.source_ty_pending`. Runs
    // before `vtable::push_vtable_instance_statics` and before
    // `body::run_phase5_body_passes` so subsequent `translate_ty`
    // calls see real `ItemId`s and the `TyInterner` lookup hits
    // existing entries instead of producing duplicates.
    let source_pending = cx.source_ty_pending.drain().collect::<Vec<_>>();
    for (handle, ty_ids) in source_pending {
        let item_id = resolve_handle(cx, handle, "SourceItem")?;
        for ty_id in ty_ids {
            cx.model.types.patch_nominal_item_id(ty_id, item_id);
        }
    }

    vtable::push_vtable_instance_statics(cx, &vtable_entries)?;

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
