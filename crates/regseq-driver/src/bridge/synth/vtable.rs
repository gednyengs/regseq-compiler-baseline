//! Vtable-family bridge item synthesis.

use std::collections::HashMap;

use regseq_model::{
    args_key_for_generic_args_with_pending, render_def_path, synthetic_def_path_hash,
    synthetic_def_path_segments, ArgsKey, Const, ConstId, CrateId, DefPath, DefPathData,
    DefPathHash, DefPathSegment, FieldLayout, FnAbi, FnSigId, FunctionInstanceFlavor, GenericArg,
    InlineHint, Instance, InstanceKind, InstanceRef, Item, ItemId, ItemKind, Layout, LayoutKind,
    Linkage, Mutability, PrincipalTraitKey, ProjectionBindingsKey, Safety, Span, StableCrateId,
    SyntheticItemKind, Ty, TyId, UintKind, UNIT_LAYOUT,
};
use rustc_middle::ty::{self as rustc_ty};
use rustc_span::def_id::DefId;

use super::{VTableSlot, VTableSynthHandles, VTableSynthKey};
use crate::bridge::{BridgeCtxt, BridgeError, ItemHandle, PendingItem};

/// Synthesise the vtable type Item for a principal trait. Idempotent
/// per `PrincipalTraitKey` per spec.md section 13.3. Returns the
/// Phase-2 `ItemHandle` that gets remapped to the final `ItemId` in
/// Phase 4.
pub(crate) fn synthesise_vtable_type(
    cx: &mut BridgeCtxt<'_>,
    principal: PrincipalTraitKey,
) -> ItemHandle {
    if let Some(handle) = cx.vtable_type_synth_cache.get(&principal) {
        return *handle;
    }
    let handle = push_pending_synthetic_item(
        cx,
        ItemKind::Struct,
        SyntheticItemKind::VTableType {
            principal: principal.clone(),
        },
        None,
    );
    cx.vtable_type_synth_cache.insert(principal, handle);
    handle
}

/// Synthesise the vtable family for the synthetic test path: vtable
/// type + vtable instance + drop shim, no method shims, no slot
/// composition. Production code uses
/// `synthesise_vtable_with_rustc_entries` instead so that the slot
/// composition reflects `tcx.vtable_entries`.
pub fn synthesise_vtable<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    principal: PrincipalTraitKey,
    impl_ty: TyId,
) -> Result<VTableSynthHandles, BridgeError> {
    synthesise_vtable_inner(cx, principal, impl_ty, Vec::new(), Vec::new(), true, None)
}

/// Synthesise the vtable family from a Cast::Unsize site backed by a
/// rustc impl_ty. Per spec.md sections 13.2 and 13.3 plus the audit
/// findings, this consults
/// `tcx.vtable_entries(principal_ref.with_self_ty(tcx, impl_ty))` to
/// derive the full slot-by-slot composition (drop, size, align,
/// methods, supertrait vptrs, vacant). One method shim Function Item
/// is synthesised per `VtblEntry::Method`. The drop shim is
/// synthesised only if `impl_ty` has non-trivial Drop.
pub(crate) fn synthesise_vtable_with_rustc_entries<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    principal: PrincipalTraitKey,
    principal_ref: rustc_ty::ExistentialTraitRef<'tcx>,
    impl_ty_model: TyId,
    impl_ty_rustc: rustc_ty::Ty<'tcx>,
) -> Result<VTableSynthHandles, BridgeError> {
    let trait_ref = principal_ref.with_self_ty(cx.input.tcx, impl_ty_rustc);
    let trait_ref = cx.input.tcx.erase_and_anonymize_regions(trait_ref);
    let entries = cx.input.tcx.vtable_entries(trait_ref).to_vec();

    let needs_drop =
        impl_ty_rustc.needs_drop(cx.input.tcx, rustc_ty::TypingEnv::fully_monomorphized());

    let (size_bytes, align_bytes) = layout_size_align(cx, impl_ty_rustc)?;

    let mut method_entries: Vec<(u32, DefPathHash)> = Vec::new();
    let mut slot_seeds: Vec<VTableSlot> = Vec::with_capacity(entries.len());
    for (idx, entry) in entries.iter().enumerate() {
        let slot_idx = idx as u32;
        match entry {
            rustc_ty::VtblEntry::MetadataDropInPlace => {
                slot_seeds.push(VTableSlot::Drop { shim_handle: None });
            }
            rustc_ty::VtblEntry::MetadataSize => {
                slot_seeds.push(VTableSlot::Size { bytes: size_bytes });
            }
            rustc_ty::VtblEntry::MetadataAlign => {
                slot_seeds.push(VTableSlot::Align { bytes: align_bytes });
            }
            rustc_ty::VtblEntry::Method(instance) => {
                let method_def_path = def_path_hash(cx.input.tcx, instance.def_id());
                let method_name = cx.input.tcx.item_name(instance.def_id()).to_string();
                let shim_fn_sig = compute_method_shim_fn_sig(cx, *instance)?;
                method_entries.push((slot_idx, method_def_path));
                slot_seeds.push(VTableSlot::Method {
                    handle: ItemHandle::from_u32(0),
                    method_def_path,
                    vtable_entry_index: slot_idx,
                    method_name,
                    shim_fn_sig,
                });
            }
            rustc_ty::VtblEntry::Vacant => {
                slot_seeds.push(VTableSlot::Vacant);
            }
            rustc_ty::VtblEntry::TraitVPtr(supertrait_ref) => {
                // Per spec.md section 13.2 plus the audit-finding
                // refinement on this task: TraitVPtr slots reference
                // the supertrait's VTableInstance Static, not just
                // its VTableType. Recursively synthesize the full
                // supertrait vtable family (type + instance + method
                // shims + drop shim) so the relocation in the slot
                // resolves to a real Instance.
                let supertrait_existential =
                    rustc_ty::ExistentialTraitRef::erase_self_ty(cx.input.tcx, *supertrait_ref);
                let supertrait_name = cx.input.tcx.item_name(supertrait_ref.def_id).to_string();
                let supertrait_principal = crate::bridge::principal_trait_key(
                    cx,
                    supertrait_existential,
                    ProjectionBindingsKey(0),
                );
                synthesise_vtable_with_rustc_entries(
                    cx,
                    supertrait_principal.clone(),
                    supertrait_existential,
                    impl_ty_model,
                    impl_ty_rustc,
                )?;
                slot_seeds.push(VTableSlot::TraitVPtr {
                    supertrait_principal,
                    supertrait_name,
                });
            }
        }
    }

    let layout = vtable_layout_for_slots(cx, &slot_seeds);

    synthesise_vtable_inner(
        cx,
        principal,
        impl_ty_model,
        method_entries,
        slot_seeds,
        needs_drop,
        Some(layout),
    )
}

/// Compute the per-method vtable-shim signature from a rustc method
/// `Instance` per spec.md section 13.2 "Method shim and drop shim ABI
/// is `FnAbi::C` uniformly... Their erased signatures are checked
/// with CAbiPositionAdmissible (§14.5) after receiver erasure to
/// `*mut ()`". Translates the source method's per-monomorphisation
/// `PolyFnSig`, replaces the receiver (`inputs[0]`) with
/// `Ty::Ptr(unit, Mutability::Mut)`, forces `abi = FnAbi::C` /
/// `safety = Safety::Safe`, interns the resulting `FnSig`, and
/// populates the `sig_c_abi_lowerings` side-table entry so spec.md
/// section 3.3 step 5 stays satisfied. The vtable struct's per-method
/// slot field type uses `Ty::FnPtr(shim_fn_sig)` so the post-rewrite
/// Indirect Call preserves the source return type per spec.md section
/// 23.6.2 B-3.
fn compute_method_shim_fn_sig<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    instance: rustc_ty::Instance<'tcx>,
) -> Result<FnSigId, BridgeError> {
    let poly_sig = cx
        .input
        .tcx
        .fn_sig(instance.def_id())
        .instantiate(cx.input.tcx, instance.args);
    let base_sig_id = crate::bridge::fnsig::translate_fn_sig(cx, poly_sig)?;
    let unit_ty = cx.model.types.intern(Ty::Tuple {
        fields: Vec::new(),
        layout: UNIT_LAYOUT,
    });
    let unit_ptr = cx.model.types.intern(Ty::Ptr(unit_ty, Mutability::Mut));
    let mut sig = cx.model.sigs.resolve(base_sig_id).clone();
    if sig.inputs.is_empty() {
        return Err(BridgeError::Phase5(
            "method shim signature must have at least one input (receiver)".to_string(),
        ));
    }
    sig.inputs[0] = unit_ptr;
    sig.abi = FnAbi::C;
    sig.safety = Safety::Safe;
    let erased_sig_id = cx.model.sigs.intern(sig);
    let _ = crate::bridge::fnsig::compute_cabi_lowering(cx, erased_sig_id);
    Ok(erased_sig_id)
}

fn synthesise_vtable_inner(
    cx: &mut BridgeCtxt<'_>,
    principal: PrincipalTraitKey,
    impl_ty: TyId,
    method_entries: Vec<(u32, DefPathHash)>,
    slot_seeds: Vec<VTableSlot>,
    needs_drop: bool,
    layout: Option<Layout>,
) -> Result<VTableSynthHandles, BridgeError> {
    let pending = cx.pending_def_path_hashes();
    let self_ty = args_key_for_generic_args_with_pending(
        &cx.model,
        &vec![GenericArg::Type(impl_ty)],
        &pending,
    );
    let key = VTableSynthKey {
        principal: principal.clone(),
        self_ty,
    };
    if let Some(handles) = cx.vtable_synth_cache.get(&key) {
        return Ok(handles.clone());
    }

    let vtable_type = synthesise_vtable_type(cx, principal.clone());
    let vtable_instance = push_pending_synthetic_item(
        cx,
        ItemKind::Static {
            mutability: Mutability::Not,
        },
        SyntheticItemKind::VTableInstance {
            principal: principal.clone(),
            self_ty,
        },
        None,
    );

    let mut method_shims_by_slot: HashMap<u32, ItemHandle> = HashMap::new();
    let mut method_shim_handles: Vec<ItemHandle> = Vec::with_capacity(method_entries.len());
    for (vtable_entry_index, method_def_path) in &method_entries {
        let handle = push_pending_synthetic_item(
            cx,
            ItemKind::Function,
            SyntheticItemKind::VTableMethodShim {
                principal: principal.clone(),
                self_ty,
                vtable_entry_index: *vtable_entry_index,
                method_def_path: *method_def_path,
            },
            None,
        );
        method_shim_handles.push(handle);
        method_shims_by_slot.insert(*vtable_entry_index, handle);
    }

    let drop_shim = if needs_drop {
        Some(push_pending_synthetic_item(
            cx,
            ItemKind::Function,
            SyntheticItemKind::VTableDropShim {
                principal: principal.clone(),
                self_ty,
            },
            None,
        ))
    } else {
        None
    };

    let mut slots = slot_seeds;
    for slot in &mut slots {
        match slot {
            VTableSlot::Drop { shim_handle } => {
                *shim_handle = drop_shim;
            }
            VTableSlot::Method {
                handle,
                vtable_entry_index,
                ..
            } => {
                if let Some(found) = method_shims_by_slot.get(vtable_entry_index) {
                    *handle = *found;
                }
            }
            _ => {}
        }
    }

    let handles = VTableSynthHandles {
        vtable_type,
        vtable_instance,
        method_shims: method_shim_handles,
        drop_shim,
        slots,
        layout,
    };
    cx.vtable_synth_cache.insert(key, handles.clone());
    Ok(handles)
}

/// Phase-5 step that pushes a `Static` Instance for every cached
/// VTableInstance Item, with a `Const::Aggregate` initializer whose
/// fields mirror the `tcx.vtable_entries` slot composition recorded
/// during Phase-2 synthesis. Per spec.md sections 13.2 and 13.3 plus
/// the audit findings on slot semantics. The Static's `ty` references
/// the `Ty::Struct` view of the vtable type Item; that struct view is
/// interned here using the cached vtable layout so callers do not
/// need to first translate a `dyn Trait` use of the principal.
pub(crate) fn push_vtable_instance_statics(
    cx: &mut BridgeCtxt<'_>,
    entries: &[(VTableSynthKey, VTableSynthHandles)],
) -> Result<(), BridgeError> {
    for (key, handles) in entries {
        let vtable_type_item = super::resolve_handle(cx, handles.vtable_type, "VTableType")?;
        let vtable_instance_item =
            super::resolve_handle(cx, handles.vtable_instance, "VTableInstance")?;

        // Intern (or reuse) the `Ty::Struct` view of the vtable type
        // so the Static instance's `ty` field is well-typed.
        let vtable_struct_ty =
            intern_vtable_struct_ty(cx, vtable_type_item, handles.layout.clone());

        // Build the per-slot Const values that make up the vtable
        // instance aggregate per spec.md section 13.2.
        let init_const = build_vtable_init_aggregate(cx, key, handles)?;

        let emission_name = cx.model.items[vtable_instance_item].def_path.rendered;
        cx.model.instances.push(Instance {
            item: vtable_instance_item,
            generic_args: Vec::new(),
            emission_name,
            span: Span::dummy(),
            kind: InstanceKind::Static {
                ty: vtable_struct_ty,
                init: Some(init_const),
            },
        });

        // The Static's init aggregate above carries `Const::Reloc`
        // entries that reference each method-shim Item and the
        // drop-shim Item (when present) by `InstanceRef`. For the
        // joint-validation walk's instance-closure check (spec.md
        // section 23.6.2 B-7) to succeed, the bridge must materialise
        // a corresponding Function Instance for every cited shim
        // Item. Without this push, the relocation dangles because
        // the only path that creates shim Instances today is a
        // Direct Call from a source body -- but for trait-object
        // dispatch the source body Calls the `Virtual` flavor, not
        // the shim, so the shim Instance would otherwise never be
        // created. Match the InstanceRef encoding in
        // `build_vtable_init_aggregate`: `regseq_kind_rank: 0`
        // (Function), `args_key: EMPTY` (empty `generic_args`),
        // `fn_shim_disambig: VTableShim`.
        push_vtable_shim_function_instances(cx, handles)?;
    }
    Ok(())
}

fn push_vtable_shim_function_instances(
    cx: &mut BridgeCtxt<'_>,
    handles: &VTableSynthHandles,
) -> Result<(), BridgeError> {
    use regseq_model::FunctionInstanceData;

    for shim_handle in &handles.method_shims {
        let shim_item = super::resolve_handle(cx, *shim_handle, "VTableMethodShim")?;
        let shim_fn_sig = handles
            .slots
            .iter()
            .find_map(|slot| match slot {
                super::VTableSlot::Method {
                    handle,
                    shim_fn_sig,
                    ..
                } if *handle == *shim_handle => Some(*shim_fn_sig),
                _ => None,
            })
            .ok_or_else(|| {
                BridgeError::Phase5(format!(
                    "VTableMethodShim handle {} has no matching VTableSlot::Method",
                    shim_handle.as_u32()
                ))
            })?;
        let emission_name = cx.model.items[shim_item].def_path.rendered;
        cx.model.instances.push(Instance {
            item: shim_item,
            generic_args: Vec::new(),
            emission_name,
            span: Span::dummy(),
            kind: InstanceKind::Function(FunctionInstanceData {
                fn_sig: shim_fn_sig,
                body: None,
                flavor: FunctionInstanceFlavor::VTableShim,
                virtual_idx: None,
            }),
        });
    }

    if let Some(drop_handle) = handles.drop_shim {
        let drop_item = super::resolve_handle(cx, drop_handle, "VTableDropShim")?;
        // Drop shims have a fixed canonical-vacant erased signature
        // (`extern "C" fn(*mut ()) -> ()`) per spec.md section 13.2
        // "Method shim and drop shim ABI is FnAbi::C uniformly".
        let drop_fn_sig_ty = canonical_vacant_fn_ptr_ty(cx);
        let Ty::FnPtr(drop_fn_sig) = cx.model.types.resolve(drop_fn_sig_ty) else {
            return Err(BridgeError::Phase5(
                "canonical_vacant_fn_ptr_ty must return Ty::FnPtr".to_string(),
            ));
        };
        let drop_fn_sig = *drop_fn_sig;
        let emission_name = cx.model.items[drop_item].def_path.rendered;
        cx.model.instances.push(Instance {
            item: drop_item,
            generic_args: Vec::new(),
            emission_name,
            span: Span::dummy(),
            kind: InstanceKind::Function(FunctionInstanceData {
                fn_sig: drop_fn_sig,
                body: None,
                flavor: FunctionInstanceFlavor::VTableShim,
                virtual_idx: None,
            }),
        });
    }
    Ok(())
}

fn intern_vtable_struct_ty(
    cx: &mut BridgeCtxt<'_>,
    vtable_type_item: ItemId,
    cached_layout: Option<Layout>,
) -> TyId {
    if let Some(ty_id) = cx.model.types.find_nominal(
        regseq_model::NominalKind::Struct,
        vtable_type_item,
        &Vec::new(),
    ) {
        // The shared `Ty::Struct` view may have been interned earlier
        // by `bridge::ty::synthesize_dynamic_vtable_ty` with a
        // pointer-only approximation of the layout. Patch it here to
        // the slot-precise layout cached by the rustc-aware
        // synthesis path so the Static and the `Ty::Dynamic.vtable_ty`
        // share an identical, spec-conformant layout.
        if let Some(layout) = cached_layout {
            cx.model.types.patch_layout(ty_id, layout);
        }
        return ty_id;
    }
    let layout = cached_layout.unwrap_or_else(empty_struct_layout);
    cx.model.types.intern(Ty::Struct {
        item: vtable_type_item,
        args: Vec::new(),
        layout,
    })
}

fn empty_struct_layout() -> Layout {
    Layout {
        size: 0,
        align: 1,
        pack: None,
        uninhabited: false,
        kind: LayoutKind::Struct { fields: vec![] },
    }
}

fn build_vtable_init_aggregate(
    cx: &mut BridgeCtxt<'_>,
    key: &VTableSynthKey,
    handles: &VTableSynthHandles,
) -> Result<ConstId, BridgeError> {
    let unit_ty = cx.model.types.intern(Ty::Tuple {
        fields: Vec::new(),
        layout: UNIT_LAYOUT,
    });
    let unit_ptr = cx.model.types.intern(Ty::Ptr(unit_ty, Mutability::Mut));
    let null_ptr = cx.model.consts.intern(Const::NullPtr(unit_ptr));
    let pointer_size = (cx.model.target.pointer_width / 8) as usize;
    let uninit_ptr_cell = cx
        .model
        .consts
        .intern(Const::Bytes(vec![regseq_model::Byte::Uninit; pointer_size]));

    let mut field_consts: Vec<ConstId> = Vec::with_capacity(handles.slots.len());
    for slot in &handles.slots {
        let value = match slot {
            VTableSlot::Drop { shim_handle: None } => null_ptr,
            VTableSlot::Drop {
                shim_handle: Some(handle),
            } => {
                let drop_shim_def_path = pending_def_path_hash_for(cx, *handle, "VTableDropShim")?;
                cx.model.consts.intern(Const::Reloc(InstanceRef {
                    regseq_kind_rank: 0,
                    def_path_hash: drop_shim_def_path,
                    args_key: ArgsKey::EMPTY,
                    fn_shim_disambig: vtable_shim_disambig(),
                }))
            }
            VTableSlot::Size { bytes } => cx
                .model
                .consts
                .intern(Const::Uint(UintKind::Usize, *bytes as u128)),
            VTableSlot::Align { bytes } => cx
                .model
                .consts
                .intern(Const::Uint(UintKind::Usize, *bytes as u128)),
            VTableSlot::Method {
                handle,
                method_def_path: _,
                ..
            } => {
                let shim_def_path = pending_def_path_hash_for(cx, *handle, "VTableMethodShim")?;
                cx.model.consts.intern(Const::Reloc(InstanceRef {
                    regseq_kind_rank: 0,
                    def_path_hash: shim_def_path,
                    args_key: ArgsKey::EMPTY,
                    fn_shim_disambig: vtable_shim_disambig(),
                }))
            }
            // Per the audit-finding refinement and rustc's
            // `vtable_allocation_provider`, vacant slots hold an
            // uninit pointer cell, not a null pointer.
            VTableSlot::Vacant => uninit_ptr_cell,
            VTableSlot::TraitVPtr {
                supertrait_principal,
                ..
            } => {
                let supertrait_synth_kind = SyntheticItemKind::VTableInstance {
                    principal: supertrait_principal.clone(),
                    self_ty: key.self_ty,
                };
                let supertrait_def_path = synthetic_def_path_hash(&supertrait_synth_kind);
                cx.model.consts.intern(Const::Reloc(InstanceRef {
                    regseq_kind_rank: 1,
                    def_path_hash: supertrait_def_path,
                    args_key: ArgsKey::EMPTY,
                    fn_shim_disambig: 0,
                }))
            }
        };
        field_consts.push(value);
    }

    Ok(cx.model.consts.intern(Const::Aggregate(field_consts)))
}

fn vtable_shim_disambig() -> u8 {
    use regseq_model::rustc_instance_kind_sort_rank;
    rustc_instance_kind_sort_rank(FunctionInstanceFlavor::VTableShim)
}

fn pending_def_path_hash_for(
    cx: &BridgeCtxt<'_>,
    handle: ItemHandle,
    family: &'static str,
) -> Result<DefPathHash, BridgeError> {
    let item_id = super::resolve_handle(cx, handle, family)?;
    Ok(cx.model.item_def_paths[item_id])
}

fn def_path_hash(tcx: rustc_middle::ty::TyCtxt<'_>, def_id: DefId) -> DefPathHash {
    let hash = tcx.def_path_hash(def_id);
    DefPathHash {
        stable_crate_id: StableCrateId(hash.stable_crate_id().as_u64()),
        local_hash: hash.local_hash().as_u64(),
    }
}

fn layout_size_align<'tcx>(
    cx: &BridgeCtxt<'tcx>,
    impl_ty: rustc_ty::Ty<'tcx>,
) -> Result<(u64, u64), BridgeError> {
    let typing_env = rustc_ty::TypingEnv::fully_monomorphized();
    let layout = cx
        .input
        .tcx
        .layout_of(typing_env.as_query_input(impl_ty))
        .map_err(|_| BridgeError::Layout {
            ty_repr: impl_ty.to_string(),
        })?;
    Ok((layout.size.bytes(), layout.align.abi.bytes()))
}

/// Build the vtable struct `Layout` per the spec.md section 13.2
/// "Synthetic vtable layout algorithm": one slot per
/// `tcx.vtable_entries` entry, each slot pointer-width, with stable
/// per-slot field names (`__drop_in_place`, `__size`, `__align`,
/// the source method's name with `#N` disambiguation when collision,
/// `__super_TraitName` with `#N` disambiguation, or `__vacant_N`).
/// Size/align slots use `Ty::Uint(Usize)` field types; vacant and
/// method slots use the canonical vacant `Ty::FnPtr` signature
/// `extern "C" fn(*mut ()) -> ()` per spec section 13.2's
/// canonical-vacant-signature rule; supertrait vptr and drop-in-place
/// slots use `Ty::Ptr(unit, Mut)` since they carry whole-pointer
/// relocations rather than fn-shaped values.
fn vtable_layout_for_slots(cx: &mut BridgeCtxt<'_>, slots: &[VTableSlot]) -> Layout {
    let pointer_size = (cx.model.target.pointer_width / 8) as u64;
    let pointer_align = pointer_size.max(1);
    let unit_ty = cx.model.types.intern(Ty::Tuple {
        fields: Vec::new(),
        layout: UNIT_LAYOUT,
    });
    let unit_ptr = cx.model.types.intern(Ty::Ptr(unit_ty, Mutability::Mut));
    let usize_ty = cx.model.types.intern(Ty::Uint(UintKind::Usize));
    let canonical_vacant_fn_ptr = canonical_vacant_fn_ptr_ty(cx);

    // Method-name and supertrait-name collision disambiguator: the
    // first occurrence keeps the source name; later occurrences get
    // `#N` appended where N is the slot index per spec.md section
    // 13.2.
    let mut method_name_seen = std::collections::HashSet::new();
    let mut super_name_seen = std::collections::HashSet::new();

    let mut fields: Vec<FieldLayout> = Vec::with_capacity(slots.len());
    for (idx, slot) in slots.iter().enumerate() {
        let (name_text, ty) = match slot {
            VTableSlot::Drop { .. } => ("__drop_in_place".to_owned(), unit_ptr),
            VTableSlot::Size { .. } => ("__size".to_owned(), usize_ty),
            VTableSlot::Align { .. } => ("__align".to_owned(), usize_ty),
            VTableSlot::Method {
                vtable_entry_index,
                method_name,
                shim_fn_sig,
                ..
            } => {
                let name = if method_name_seen.insert(method_name.clone()) {
                    method_name.clone()
                } else {
                    format!("{method_name}#{vtable_entry_index}")
                };
                // Per spec.md section 13.2 + section 23.6.2 B-3, the
                // method slot field type is the per-method shim
                // signature (receiver erased to `*mut ()`, return
                // type preserved) so the post-rewrite Indirect Call
                // satisfies "destination_ty == operand_ty(callee_
                // operand).output()". Vacant slots below still use
                // the canonical vacant FnPtr per spec.md section
                // 13.2 "Vacant slot canonical type".
                let shim_fn_ptr = cx.model.types.intern(Ty::FnPtr(*shim_fn_sig));
                (name, shim_fn_ptr)
            }
            VTableSlot::Vacant => (format!("__vacant_{idx}"), canonical_vacant_fn_ptr),
            VTableSlot::TraitVPtr {
                supertrait_name, ..
            } => {
                let base = format!("__super_{supertrait_name}");
                let name = if super_name_seen.insert(base.clone()) {
                    base
                } else {
                    format!("{base}#{idx}")
                };
                (name, unit_ptr)
            }
        };
        let name = cx.model.strings.intern(&name_text);
        fields.push(FieldLayout {
            name,
            ty,
            offset: idx as u64 * pointer_size,
        });
    }
    Layout {
        size: slots.len() as u64 * pointer_size,
        align: pointer_align,
        pack: None,
        uninhabited: false,
        kind: LayoutKind::Struct { fields },
    }
}

/// Intern the canonical vacant FnPtr signature
/// `extern "C" fn(*mut ()) -> ()` per spec.md section 13.2. The
/// interned FnSig has `FnAbi::C`, so spec.md section 3.3 step 5
/// requires a matching `Some(CAbiLowering)` in the parallel
/// `sig_c_abi_lowerings` side table; populate it via the canonical
/// `bridge::fnsig::compute_cabi_lowering` helper.
pub fn canonical_vacant_fn_ptr_ty(cx: &mut BridgeCtxt<'_>) -> TyId {
    use regseq_model::{FnAbi, FnSig, Safety};
    let unit_ty = cx.model.types.intern(Ty::Tuple {
        fields: Vec::new(),
        layout: UNIT_LAYOUT,
    });
    let unit_ptr = cx.model.types.intern(Ty::Ptr(unit_ty, Mutability::Mut));
    let sig = FnSig {
        inputs: vec![unit_ptr],
        output: unit_ty,
        abi: FnAbi::C,
        c_variadic: false,
        safety: Safety::Safe,
    };
    let sig_id = cx.model.sigs.intern(sig);
    let _ = crate::bridge::fnsig::compute_cabi_lowering(cx, sig_id);
    cx.model.types.intern(Ty::FnPtr(sig_id))
}

pub(crate) fn push_pending_synthetic_item(
    cx: &mut BridgeCtxt<'_>,
    kind: ItemKind,
    synthetic_kind: SyntheticItemKind,
    def_path_hash_override: Option<DefPathHash>,
) -> ItemHandle {
    let handle = cx.handles.fresh_handle();
    let (def_path, def_path_hash) = synthetic_def_path(cx, &synthetic_kind, def_path_hash_override);
    let item = Item {
        def_path,
        kind,
        span: Span::dummy(),
        linkage: Linkage::Default,
        link_section: None,
        inline_hint: InlineHint::None,
        is_unsafe: false,
        is_phantom_data: false,
        is_unsafe_cell: false,
        is_manually_drop: false,
        opaque_handle_eligible: false,
    };
    cx.pending_items.push(PendingItem {
        handle,
        item,
        def_path_hash,
        synthetic_kind,
    });
    handle
}

pub(crate) fn synthetic_def_path(
    cx: &mut BridgeCtxt<'_>,
    synthetic_kind: &SyntheticItemKind,
    def_path_hash_override: Option<DefPathHash>,
) -> (DefPath, DefPathHash) {
    match synthetic_kind {
        SyntheticItemKind::ClosureCaptures { closure } => {
            let text = format!("<closure captures {}>", closure.local_hash);
            let text_id = cx.model.strings.intern(&text);
            let def_path = DefPath {
                crate_id: cx.model.primary_crate,
                segments: vec![DefPathSegment {
                    data: DefPathData::TypeNs(text_id),
                    disambiguator: 0,
                }],
                rendered: text_id,
            };
            (def_path, def_path_hash_override.unwrap_or(*closure))
        }
        SyntheticItemKind::None => synthetic_none_def_path(cx, "drop_glue"),
        _ => {
            let segments = synthetic_def_path_segments(synthetic_kind, &mut cx.model.strings);
            let crate_id = synthetic_crate_id(cx, synthetic_kind);
            let rendered = render_def_path(
                &DefPath {
                    crate_id,
                    segments: segments.clone(),
                    rendered: regseq_model::StringId::ZERO,
                },
                &cx.model.crates,
                &cx.model.strings,
            );
            let def_path = DefPath {
                crate_id,
                segments,
                rendered: cx.model.strings.intern(&rendered),
            };
            (def_path, synthetic_def_path_hash(synthetic_kind))
        }
    }
}

fn synthetic_crate_id(cx: &BridgeCtxt<'_>, synthetic_kind: &SyntheticItemKind) -> CrateId {
    let stable = match synthetic_kind {
        SyntheticItemKind::ClosureCall { closure, .. }
        | SyntheticItemKind::ClosureDropGlue { closure } => Some(closure.stable_crate_id),
        SyntheticItemKind::VTableType { principal }
        | SyntheticItemKind::VTableInstance { principal, .. }
        | SyntheticItemKind::VTableMethodShim { principal, .. }
        | SyntheticItemKind::VTableDropShim { principal, .. } => {
            Some(principal.trait_path.stable_crate_id)
        }
        SyntheticItemKind::DropGlue { .. }
        | SyntheticItemKind::ConstMemoryStatic { .. }
        | SyntheticItemKind::None
        | SyntheticItemKind::ClosureCaptures { .. } => None,
    };
    stable
        .and_then(|stable| {
            cx.model
                .crates
                .iter()
                .enumerate()
                .find(|(_, krate)| krate.stable_crate_id == stable)
                .map(|(idx, _)| CrateId::from_u32(idx as u32))
        })
        .unwrap_or(cx.model.primary_crate)
}

fn synthetic_none_def_path(cx: &mut BridgeCtxt<'_>, family: &str) -> (DefPath, DefPathHash) {
    let disambiguator = cx.pending_items.len() as u64;
    let text = format!("<regseq_synth::{family}::{disambiguator}>");
    let text_id = cx.model.strings.intern(&text);
    let def_path = DefPath {
        crate_id: cx.model.primary_crate,
        segments: vec![DefPathSegment {
            data: DefPathData::ValueNs(text_id),
            disambiguator: 0,
        }],
        rendered: text_id,
    };
    (
        def_path,
        DefPathHash {
            stable_crate_id: regseq_model::StableCrateId(0),
            local_hash: 0xD00D_600E_0000_0000 | disambiguator,
        },
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn empty_self_key_is_available_for_vtable_tests() {
        assert_eq!(regseq_model::ArgsKey::EMPTY, regseq_model::ArgsKey::EMPTY);
    }
}
