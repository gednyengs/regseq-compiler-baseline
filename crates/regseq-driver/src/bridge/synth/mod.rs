//! Phase-2 synthesis entry points for bridge-only Items.

use regseq_model::{
    rustc_instance_kind_sort_rank, synthetic_def_path_hash, ArgsKey, Byte, Const, FunctionInstanceFlavor,
    GenericArg, Instance, InstanceKind, InstanceRef, ItemId, PrincipalTraitKey, Span,
    SyntheticItemKind, Ty, TyId, UintKind,
};

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

/// Per-slot classification of a vtable entry as observed at the
/// `tcx.vtable_entries` query result. Used by Phase 5 to build the
/// VTableInstance Static initialiser.
///
/// The `Method` variant carries no payload at the caller boundary;
/// the synthesis function fills in the shim Item handle by matching
/// slot index against the freshly-pushed method shim handles.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VTableSlotKind {
    /// `VtblEntry::MetadataDropInPlace` slot. Resolved to the
    /// drop-shim Item's function pointer at init time.
    DropInPlace,
    /// `VtblEntry::MetadataSize` slot. Encodes the impl-type layout
    /// size as a `usize`-typed Const.
    Size,
    /// `VtblEntry::MetadataAlign` slot. Encodes the impl-type layout
    /// align as a `usize`-typed Const.
    Align,
    /// `VtblEntry::Method(_)` slot. The shim Item handle is filled in
    /// by `synthesise_vtable_with_init_meta` after the shim Items are
    /// pushed; callers pass `Method(None)`.
    Method(Option<ItemHandle>),
    /// `VtblEntry::Vacant` slot. No initialiser; encoded as a null
    /// pointer constant.
    Vacant,
    /// `VtblEntry::TraitVPtr(_)` slot (supertrait upcasting).
    /// Out of scope for v1; encoded as a null pointer constant.
    TraitVPtr,
}

/// Per-vtable init metadata captured at synthesis time so Phase 5 can
/// build the VTableInstance Static's `init` from concrete impl-type
/// layout data. The full slot sequence and the impl-type size/align
/// are needed to mirror rustc's `vtable_allocation_provider`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VTableInitMeta {
    /// Slot sequence in `tcx.vtable_entries` order.
    pub slots: Vec<VTableSlotKind>,
    /// `tcx.layout_of(impl_ty).size` in bytes; encoded in the Size slot.
    pub size: u64,
    /// `tcx.layout_of(impl_ty).align.abi` in bytes; encoded in the Align slot.
    pub align: u64,
    /// `tcx.data_layout.pointer_size()` in bytes; used to size the
    /// Array<u8, N> type for the Static instance.
    pub pointer_size: u64,
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
    /// Slot metadata needed to build the Static instance's init at
    /// Phase 5. `None` when the synthesis path lacked rustc data
    /// (public `synthesise_vtable` entry); Phase 5 falls back to a
    /// 3-slot common-header init in that case.
    pub init_meta: Option<VTableInitMeta>,
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

    // Clone the cache into a sorted snapshot so we can mutate
    // `cx.model` (push consts, types, instances) while iterating.
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
    for (key, handles) in vtable_entries {
        let vtable_type = resolve_handle(cx, handles.vtable_type, "VTableType")?;
        let vtable_instance = resolve_handle(cx, handles.vtable_instance, "VTableInstance")?;
        for method_shim in &handles.method_shims {
            resolve_handle(cx, *method_shim, "VTableMethodShim")?;
        }
        resolve_handle(cx, handles.drop_shim, "VTableDropShim")?;
        cx.model.vtables.insert(key.principal.clone(), vtable_type);
        push_vtable_instance_static(cx, &key, &handles, vtable_instance)?;
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

/// Synthesise an `Instance` of `InstanceKind::Static { ty, init:
/// Some(_) }` for the VTableInstance Item. The init mirrors rustc's
/// `vtable_allocation_provider`: one pointer-sized cell per
/// `tcx.vtable_entries` slot, with function-pointer relocs for
/// DropInPlace / Method slots and zero-extended integer cells for
/// Size / Align slots. Vacant and TraitVPtr slots are encoded as
/// uninitialised pointer cells (matches rustc).
///
/// Per spec.md section 13.2 (line 4743) and the validation rule at
/// spec.md line 1063-1074.
fn push_vtable_instance_static(
    cx: &mut BridgeCtxt<'_>,
    key: &VTableSynthKey,
    handles: &VTableSynthHandles,
    vtable_instance: ItemId,
) -> Result<(), BridgeError> {
    let pointer_size = cx
        .input
        .tcx
        .data_layout
        .pointer_size()
        .bytes();
    // HYPOTHESIS: when no rustc-derived init meta is available
    // (synthetic-principal test paths), fall back to the three-slot
    // common header layout (DropInPlace, Size, Align) per
    // `TyCtxt::COMMON_VTABLE_ENTRIES` with zero size/align values.
    // This keeps the spec's "init: Some(_)" invariant
    // (spec.md:1069-1071) satisfied for every VTableInstance Item.
    let (slots, size, align, ptr_size) = match handles.init_meta.as_ref() {
        Some(meta) => (
            meta.slots.clone(),
            meta.size,
            meta.align,
            meta.pointer_size,
        ),
        None => (
            vec![
                VTableSlotKind::DropInPlace,
                VTableSlotKind::Size,
                VTableSlotKind::Align,
            ],
            0,
            1,
            pointer_size.max(1),
        ),
    };

    let total_bytes = ptr_size * (slots.len() as u64);
    let mut bytes = Vec::with_capacity(total_bytes as usize);
    for slot in &slots {
        match slot {
            VTableSlotKind::DropInPlace => {
                let reloc = vtable_shim_instance_ref(SyntheticItemKind::VTableDropShim {
                    principal: key.principal.clone(),
                    self_ty: key.self_ty,
                });
                push_pointer_reloc(&mut bytes, reloc, ptr_size);
            }
            VTableSlotKind::Size => {
                push_pointer_uint(&mut bytes, size, ptr_size);
            }
            VTableSlotKind::Align => {
                push_pointer_uint(&mut bytes, align, ptr_size);
            }
            VTableSlotKind::Method(handle_slot) => {
                let resolved = handle_slot.and_then(|handle| cx.handles.resolve(handle));
                if let Some(item_id) = resolved {
                    let synthetic_kind = cx.model.item_synthetic_kinds[item_id].clone();
                    let reloc = vtable_shim_instance_ref(synthetic_kind);
                    push_pointer_reloc(&mut bytes, reloc, ptr_size);
                } else {
                    push_pointer_uninit(&mut bytes, ptr_size);
                }
            }
            VTableSlotKind::Vacant | VTableSlotKind::TraitVPtr => {
                push_pointer_uninit(&mut bytes, ptr_size);
            }
        }
    }

    let init_const = cx.model.consts.intern(Const::Bytes(bytes));
    let u8_ty = cx.model.types.intern(Ty::Uint(UintKind::U8));
    let array_ty = cx.model.types.intern(Ty::Array(u8_ty, total_bytes));
    let emission_name = cx.model.items[vtable_instance].def_path.rendered;
    cx.model.instances.push(Instance {
        item: vtable_instance,
        generic_args: vec![GenericArg::Type(array_ty)],
        emission_name,
        span: Span::dummy(),
        kind: InstanceKind::Static {
            ty: array_ty,
            init: Some(init_const),
        },
    });
    Ok(())
}

/// Build an `InstanceRef` for a bridge-synthesised vtable function
/// shim. The reference is identity-only: it points to a Function-rank
/// instance whose stable identity comes from the synthetic item's
/// `synthetic_def_path_hash`. The `VTableShim` flavor rank (2)
/// distinguishes vtable shims from other Function instances per the
/// F4-2 sort key (regseq-model/instance.rs).
fn vtable_shim_instance_ref(synthetic_kind: SyntheticItemKind) -> InstanceRef {
    InstanceRef {
        regseq_kind_rank: 0,
        def_path_hash: synthetic_def_path_hash(&synthetic_kind),
        args_key: ArgsKey::EMPTY,
        fn_shim_disambig: rustc_instance_kind_sort_rank(FunctionInstanceFlavor::VTableShim),
    }
}

fn push_pointer_reloc(bytes: &mut Vec<Byte>, reloc: InstanceRef, ptr_size: u64) {
    for _ in 0..ptr_size {
        bytes.push(Byte::Reloc(reloc));
    }
}

fn push_pointer_uint(bytes: &mut Vec<Byte>, value: u64, ptr_size: u64) {
    let little_endian = value.to_le_bytes();
    for i in 0..ptr_size as usize {
        let byte = little_endian.get(i).copied().unwrap_or(0);
        bytes.push(Byte::Value(byte));
    }
}

fn push_pointer_uninit(bytes: &mut Vec<Byte>, ptr_size: u64) {
    for _ in 0..ptr_size {
        bytes.push(Byte::Uninit);
    }
}
