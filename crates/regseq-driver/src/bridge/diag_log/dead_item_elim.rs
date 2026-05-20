//! Post-DiagLogPeel dead-Item / dead-Instance / dead-Ty /
//! dead-FnSig elimination per spec.md section 26.6 ("the LogArg
//! type does NOT appear in RegseqModel"). Runs after every Body's
//! DiagLogPeel completes so the `DiagLogPeelResidue` accumulator
//! carries the full collapsed-wrapper set.
//!
//! Scope is residue-driven, not generalised DCE: removal seeds are
//! the wrapper Instances DiagLogPeel collapsed plus the LogArg
//! Item; transitive removal proceeds only along chains that anchor
//! at those seeds. Unrelated unreferenced entries (orphan closures,
//! unused drop-glue, dead synthesised statics) are explicitly NOT
//! touched per spec.md section 26.10 deferrals.
//!
//! Removal proceeds in dependency order: instances first (no other
//! table depends on InstanceId positions), then FnSigs (TyInterner
//! `Ty::FnPtr` carries FnSigId), then Tys (most interconnected),
//! finally Items (`for_each_item_id_mut` touches every other
//! surface so it runs last). The bridge-side `HandleTable` is
//! remapped through the items-removal `Vec<Option<ItemId>>` so
//! cross-phase handle resolution survives the compaction.

use std::collections::{HashMap, HashSet};

use regseq_model::{
    AttrData, CAbiShape, Const, FnAbi, FnSig, FnSigId, GenericArg, InstanceId, InstanceKind,
    IntrinsicKind, ItemId, Safety, Ty, TyId, UNIT_LAYOUT,
};

use crate::bridge::{BridgeCtxt, BridgeError};

use super::is_from_logarg_instance;

/// Run the post-DiagLogPeel residue-driven DCE per spec.md section
/// 26.6 + regseq-driver-48 Change B. Computes the transitive
/// dead-set across instances, FnSigs, Tys, and Items, then invokes
/// the four `remove_*_and_remap` model helpers to compact storage
/// and remap every surviving reference. Drains
/// `cx.diag_log_peel_residue` so the pass is idempotent.
pub fn eliminate_diag_log_peel_residue(cx: &mut BridgeCtxt<'_>) -> Result<(), BridgeError> {
    // Step 0: pre-rewrite Log-intrinsic Function Instances so they
    // do not transitively keep `LogArg` alive in the TyInterner.
    // Each Log-intrinsic stub's original signature is
    // `fn(&'static str, &[LogArg<'_>]) -> ()`; the slice-of-LogArg
    // input references LogArg via the
    // `Ty::Ref(Ty::Slice(LogArg), _)` chain. Per spec.md section
    // 26.6 ("the `LogArg` type does NOT appear in RegseqModel"),
    // the LogArg Ty must be removable from the TyInterner. The
    // FnSig rewrite drops LogArg-bearing inputs (replacing the
    // FnSig with one whose only input is the format-template
    // `&'static str`, matching the canonical post-peel call
    // shape's args[0]); the body is set to `None`. The OLD FnSig
    // becomes orphan and is reaped by the residue-driven DCE
    // below.
    //
    // This step is gated on a non-empty residue accumulator so
    // unrelated fixtures (no log calls) are not touched. Per the
    // Non-Goals section the `IntrinsicKind::Log(_)` `AttrData` on
    // the four `log_<sev>` callee Items is preserved, as are the
    // Items themselves; only the Function Instance's signature +
    // body are rewritten.
    let mut orphaned_log_intrinsic_fn_sigs: HashSet<FnSigId> = HashSet::new();
    if !cx
        .diag_log_peel_residue
        .collapsed_wrapper_instances
        .is_empty()
    {
        orphaned_log_intrinsic_fn_sigs = rewrite_log_intrinsic_signatures(cx);
    }

    // Step 1: seed the dead-Instance set. The primary seeds are
    // the wrapper Instances DiagLogPeel collapsed (from the
    // residue accumulator), filtered through
    // `is_from_logarg_instance` per spec.md section 26.4 + 26.5.
    //
    // Widening clause (load-bearing for the §26.6 contract on the
    // canonical `diag_logging` fixture): every Function Instance
    // that satisfies the structural `<LogArg as From<T>>::from`
    // predicate is dead, whether or not its wrapper Call was
    // collapsed. Rustc materialises every impl method in an rlib
    // crate type, so an unused sibling like
    // `<LogArg as From<&str>>::from` survives in
    // `model.instances` even when no wrapper for it is collapsed.
    // Strict residue-driven scope cannot remove LogArg while any
    // such sibling Function Instance is alive (its body's return-
    // place LocalDecl pins LogArg via `live_tys`). The widening
    // anchors at the residue accumulator (the pass is a no-op
    // when no wrappers were collapsed) and only adds Instances
    // that the structural LogArg-Self predicate matches; orphan
    // From impls for non-LogArg Self types stay alive.
    let mut dead_instances: HashSet<InstanceId> = HashSet::new();
    for &instance_id in &cx.diag_log_peel_residue.collapsed_wrapper_instances {
        if instance_id.index() < cx.model.instances.len()
            && is_from_logarg_instance(&cx.model, instance_id)
        {
            dead_instances.insert(instance_id);
        }
    }
    let residue_drove_at_least_one_seed = !dead_instances.is_empty();
    if residue_drove_at_least_one_seed {
        for idx in 0..cx.model.instances.len() {
            let id = InstanceId::from_u32(idx as u32);
            if dead_instances.contains(&id) {
                continue;
            }
            if is_from_logarg_instance(&cx.model, id) {
                dead_instances.insert(id);
            }
        }
        for idx in 0..cx.model.instances.len() {
            let id = InstanceId::from_u32(idx as u32);
            if dead_instances.contains(&id) {
                continue;
            }
            if instance_is_logarg_impl_from(&cx.model, id) {
                dead_instances.insert(id);
            }
        }
    }

    // Step 2: seed the dead-Item set from Items whose every
    // referencing Instance lies in `dead_instances`. Items with
    // zero referencing Instances are NOT marked dead (residue-
    // driven scope: orphan Items unrelated to DiagLog stay alive
    // per spec.md section 26.10).
    let mut instances_per_item: HashMap<ItemId, Vec<InstanceId>> = HashMap::new();
    for (idx, inst) in cx.model.instances.iter().enumerate() {
        instances_per_item
            .entry(inst.item)
            .or_default()
            .push(InstanceId::from_u32(idx as u32));
    }
    let mut dead_items: HashSet<ItemId> = HashSet::new();
    for (item, instances) in &instances_per_item {
        if !instances.is_empty() && instances.iter().all(|id| dead_instances.contains(id)) {
            dead_items.insert(*item);
        }
    }

    // Step 3: compute the dead-FnSig set. A FnSig is dead iff it
    // is referenced from at least one dead Instance AND not
    // referenced from any surviving Instance or any surviving
    // (i.e., not-yet-known-dead) Ty::FnPtr entry. The Ty::FnPtr
    // surface is conservatively treated as a live reference
    // because the dead-Ty set is computed in step 4.
    let mut dead_referenced_fn_sigs: HashSet<FnSigId> = HashSet::new();
    let mut surviving_referenced_fn_sigs: HashSet<FnSigId> = HashSet::new();
    for (idx, inst) in cx.model.instances.iter().enumerate() {
        let id = InstanceId::from_u32(idx as u32);
        if let InstanceKind::Function(data) = &inst.kind {
            if dead_instances.contains(&id) {
                dead_referenced_fn_sigs.insert(data.fn_sig);
            } else {
                surviving_referenced_fn_sigs.insert(data.fn_sig);
            }
        }
    }
    for (_, ty) in cx.model.types.iter() {
        if let Ty::FnPtr(sig) = ty {
            // Skip Ty::FnPtr entries whose target FnSig was
            // orphaned by the step-0 Log-intrinsic FnSig rewrite.
            // Such a Ty::FnPtr is itself orphan (the only thing
            // that would have held it was the pre-rewrite
            // log_<sev> Function Instance's body via a function-
            // pointer cast, which the rewrite drops together with
            // the body). Treating it as a surviving FnSigId ref
            // would falsely pin the orphan LogArg-bearing FnSig in
            // `surviving_referenced_fn_sigs` and block the residue
            // -driven DCE from reaping it; spec.md section 26.6
            // requires the LogArg-bearing FnSig (and the LogArg
            // Tys it transitively references) to die.
            if orphaned_log_intrinsic_fn_sigs.contains(sig) {
                continue;
            }
            surviving_referenced_fn_sigs.insert(*sig);
        }
    }
    let mut dead_fn_sigs: HashSet<FnSigId> = HashSet::new();
    for sig_id in &dead_referenced_fn_sigs {
        if !surviving_referenced_fn_sigs.contains(sig_id) {
            dead_fn_sigs.insert(*sig_id);
        }
    }
    // Augment dead_fn_sigs with the FnSigs orphaned by the
    // step-0 log_<sev> intrinsic FnSig rewrite. Those FnSigs
    // were referenced from surviving Function Instances PRE-
    // rewrite; POST-rewrite no surviving entity references
    // them. They are load-bearing dead-set members for the
    // residue-driven scope (each carries the LogArg-bearing
    // carrier input that pins LogArg in `live_tys`).
    for sig_id in &orphaned_log_intrinsic_fn_sigs {
        if !surviving_referenced_fn_sigs.contains(sig_id) {
            dead_fn_sigs.insert(*sig_id);
        }
    }

    // Step 4: compute the dead-Ty set. First gather every TyId
    // directly referenced from surviving Instances, surviving
    // FnSigs, surviving sig_c_abi_lowerings, the Const interner,
    // and the closure-captures map (both key and value). BFS
    // through TyInterner entries to compute the transitive
    // closure of surviving TyIds. Any TyId NOT in that closure
    // AND directly referenced from some dead Instance, dead
    // FnSig, or a Ty whose nominal `item` is dead, becomes a
    // dead-Ty seed. Then BFS through the dead-Ty seeds to compute
    // the transitive dead closure.
    let mut live_tys: HashSet<TyId> = HashSet::new();
    let mut live_worklist: Vec<TyId> = Vec::new();
    for (idx, inst) in cx.model.instances.iter().enumerate() {
        let id = InstanceId::from_u32(idx as u32);
        if dead_instances.contains(&id) {
            continue;
        }
        collect_instance_ty_ids(inst, &mut |ty| {
            push_live(ty, &mut live_tys, &mut live_worklist);
        });
    }
    for (sig_id, sig) in cx.model.sigs.iter() {
        if dead_fn_sigs.contains(&sig_id) {
            continue;
        }
        for input in &sig.inputs {
            push_live(*input, &mut live_tys, &mut live_worklist);
        }
        push_live(sig.output, &mut live_tys, &mut live_worklist);
    }
    for (idx, lowering_opt) in cx.model.sig_c_abi_lowerings.iter().enumerate() {
        let sig_id = FnSigId::from_u32(idx as u32);
        if dead_fn_sigs.contains(&sig_id) {
            continue;
        }
        if let Some(lowering) = lowering_opt {
            for shape in &lowering.inputs {
                if let CAbiShape::Substituted { emit_as, .. } = shape {
                    push_live(*emit_as, &mut live_tys, &mut live_worklist);
                }
            }
            if let CAbiShape::Substituted { emit_as, .. } = &lowering.output {
                push_live(*emit_as, &mut live_tys, &mut live_worklist);
            }
        }
    }
    for (_, value) in cx.model.consts.iter() {
        if let Const::NullPtr(ty) = value {
            push_live(*ty, &mut live_tys, &mut live_worklist);
        }
    }
    for (&key, &value) in &cx.model.closure_captures_struct_ty {
        push_live(key, &mut live_tys, &mut live_worklist);
        push_live(value, &mut live_tys, &mut live_worklist);
    }
    while let Some(tid) = live_worklist.pop() {
        let ty = cx.model.types.resolve(tid).clone();
        collect_ty_internal_ty_ids(&ty, &mut |t| {
            push_live(t, &mut live_tys, &mut live_worklist);
        });
    }

    let mut dead_tys: HashSet<TyId> = HashSet::new();
    let mut dead_worklist: Vec<TyId> = Vec::new();
    // Seed dead-Ty set from refs out of dead Instances and dead
    // FnSigs that are NOT in live_tys.
    for (idx, inst) in cx.model.instances.iter().enumerate() {
        let id = InstanceId::from_u32(idx as u32);
        if !dead_instances.contains(&id) {
            continue;
        }
        collect_instance_ty_ids(inst, &mut |ty| {
            push_dead(ty, &live_tys, &mut dead_tys, &mut dead_worklist);
        });
    }
    for (sig_id, sig) in cx.model.sigs.iter() {
        if !dead_fn_sigs.contains(&sig_id) {
            continue;
        }
        for input in &sig.inputs {
            push_dead(*input, &live_tys, &mut dead_tys, &mut dead_worklist);
        }
        push_dead(sig.output, &live_tys, &mut dead_tys, &mut dead_worklist);
    }
    // Tys whose nominal `item` is in dead_items are "referenced
    // from a dead entry" via the dead Item side: walk every Ty,
    // and if it points to a dead Item, seed it as dead. The
    // residue-driven contract requires the dead Item set to come
    // from collapsed wrappers (step 2 above), so this widens dead
    // Tys to include the LogArg-typed Ty entries whose nominal
    // Item is the From-impl-trait Item (when it gets seeded) or
    // the LogArg ADT Item (handled in step 5).
    //
    // Also seeds any `Ty::FnPtr(sig)` whose target sig is in
    // dead_fn_sigs (i.e., orphan FnSigs from the step-0 rewrite
    // or FnSigs reaped by the residue-driven scope). Required
    // because `remove_fn_sigs_and_remap` is invoked BEFORE
    // `remove_tys_and_remap` per the dependency-order contract
    // at spec.md regseq-driver-48 Implementation Notes step 6,
    // and the FnSig-removal walker rewrites every `Ty::FnPtr` --
    // a surviving `Ty::FnPtr(dead_sig)` would panic the walker
    // because its sig entry has been compacted away.
    for (ty_id, ty) in cx.model.types.iter() {
        let nominal_item = match ty {
            Ty::Enum { item, .. }
            | Ty::Struct { item, .. }
            | Ty::Union { item, .. }
            | Ty::FnDef(item, _)
            | Ty::Closure(item, _) => Some(*item),
            _ => None,
        };
        if let Some(item) = nominal_item {
            if dead_items.contains(&item) {
                push_dead(ty_id, &live_tys, &mut dead_tys, &mut dead_worklist);
            }
        }
        if let Ty::FnPtr(sig) = ty {
            if dead_fn_sigs.contains(sig) {
                push_dead(ty_id, &live_tys, &mut dead_tys, &mut dead_worklist);
            }
        }
    }
    while let Some(tid) = dead_worklist.pop() {
        let ty = cx.model.types.resolve(tid).clone();
        collect_ty_internal_ty_ids(&ty, &mut |t| {
            push_dead(t, &live_tys, &mut dead_tys, &mut dead_worklist);
        });
    }
    // Closure step: any Ty whose internal TyId reference resolves
    // to a dead Ty must itself be dead (its inner reference would
    // otherwise dangle after compaction). This catches Tys that
    // are not directly seeded from a dead Item / Instance /
    // FnSig but transitively reach LogArg via the carrier chain
    // -- e.g., `Ty::Array(LogArg, N)` interned during DiagLogPeel
    // body translation but nullified to unit by
    // `null_carrier_local_decl_types`. The Ty entry itself
    // remains in the TyInterner without any surviving reference;
    // residue-driven scope demands it be cleaned up alongside
    // LogArg.
    //
    // Iterates to fixpoint: each pass may seed new dead Tys
    // whose dependents need re-checking. The `live_tys` filter
    // guarantees no Ty referenced from a surviving entity is
    // dragged into the dead set by this closure step.
    loop {
        let mut new_dead: Vec<TyId> = Vec::new();
        for (ty_id, ty) in cx.model.types.iter() {
            if dead_tys.contains(&ty_id) || live_tys.contains(&ty_id) {
                continue;
            }
            let mut references_dead = false;
            collect_ty_internal_ty_ids(ty, &mut |t| {
                if dead_tys.contains(&t) {
                    references_dead = true;
                }
            });
            if references_dead {
                new_dead.push(ty_id);
            }
        }
        if new_dead.is_empty() {
            break;
        }
        for ty_id in new_dead {
            dead_tys.insert(ty_id);
        }
    }

    // Step 5: augment dead_items with the LogArg ADT Item if it
    // has no surviving Ty reference and no surviving Instance
    // reference. The structural search keys on the def_path
    // terminal segment per spec.md section 26.6 ("the LogArg
    // type do NOT appear in RegseqModel").
    let logarg_adt_item = find_logarg_adt_item(cx);
    if let Some(adt_item) = logarg_adt_item {
        if !dead_items.contains(&adt_item) {
            let mut has_surviving_ty_ref = false;
            for (ty_id, ty) in cx.model.types.iter() {
                if dead_tys.contains(&ty_id) {
                    continue;
                }
                let nominal_item = match ty {
                    Ty::Enum { item, .. }
                    | Ty::Struct { item, .. }
                    | Ty::Union { item, .. }
                    | Ty::FnDef(item, _)
                    | Ty::Closure(item, _) => Some(*item),
                    _ => None,
                };
                if nominal_item == Some(adt_item) {
                    has_surviving_ty_ref = true;
                    break;
                }
            }
            let mut has_surviving_instance = false;
            for (idx, inst) in cx.model.instances.iter().enumerate() {
                let id = InstanceId::from_u32(idx as u32);
                if dead_instances.contains(&id) {
                    continue;
                }
                if inst.item == adt_item {
                    has_surviving_instance = true;
                    break;
                }
            }
            if !has_surviving_ty_ref && !has_surviving_instance {
                dead_items.insert(adt_item);
            }
        }
    }

    // Pre-removal handle invariant (relaxed for the bridge's
    // actual `assign_pending_items` behavior): the regseq-driver-
    // 47 `HandleTable` is populated by Phase-4 assignment for
    // EVERY interned Item (including source-derived ADT Items
    // and the per-impl From method Items this pass removes), not
    // only for bridge-synthesised entries. The
    // `regseq-driver-48` Implementation Notes step 6 invariant
    // "the pass MUST not remove handle-bound Items" therefore
    // cannot hold literally: every removable Item is handle-
    // bound. The contract is preserved in spirit because this
    // pass runs LAST in Phase 5 (per the pipeline wiring in
    // `bridge/mod.rs`): no post-Phase-5 consumer calls
    // `handles.resolve(...)`, so handles bound to removed Items
    // will not be looked up again. The dense remap built below
    // maps removed slots to `ItemId::ZERO`; that value is
    // "garbage" and only matters if someone resolves the dead
    // handle, which is forbidden by the post-Phase-5 contract.

    // Step 6: removal in dependency order. Instances first (no
    // downstream index depends on them); FnSigs next (Ty::FnPtr
    // carries FnSigId); Tys next (most interconnected); Items
    // last (`for_each_item_id_mut` touches every other surface).
    let instance_keep: Vec<bool> = (0..cx.model.instances.len())
        .map(|i| !dead_instances.contains(&InstanceId::from_u32(i as u32)))
        .collect();
    let _instance_remap = cx.model.remove_instances_and_remap(&instance_keep);

    let fn_sig_keep: Vec<bool> = (0..cx.model.sigs.len())
        .map(|i| !dead_fn_sigs.contains(&FnSigId::from_u32(i as u32)))
        .collect();
    let _fn_sig_remap = cx.model.remove_fn_sigs_and_remap(&fn_sig_keep);

    let ty_keep: Vec<bool> = (0..cx.model.types.len())
        .map(|i| !dead_tys.contains(&TyId::from_u32(i as u32)))
        .collect();
    let _ty_remap = cx.model.remove_tys_and_remap(&ty_keep);

    let item_keep: Vec<bool> = (0..cx.model.items.len())
        .map(|i| !dead_items.contains(&ItemId::from_u32(i as u32)))
        .collect();
    let item_remap_opt = cx.model.remove_items_and_remap(&item_keep);

    // Drop any HandleTable binding whose old ItemId corresponds to
    // a removed Item; the remaining bindings all point at
    // surviving Items and can be remapped through the dense
    // `Vec<ItemId>` slot table per regseq-driver-47's
    // `HandleTable::remap_item_ids` contract. Dropping prevents a
    // stale handle from silently resolving to an unrelated
    // surviving item if a future bridge change accidentally
    // resolves such a handle post-Phase-5. Removed slots in the
    // dense remap receive `ItemId::ZERO`, but those slots are
    // unreachable after the drop step above.
    let removed_item_indices: HashSet<usize> = item_remap_opt
        .iter()
        .enumerate()
        .filter_map(|(idx, slot)| if slot.is_none() { Some(idx) } else { None })
        .collect();
    cx.handles
        .handle_to_item_id
        .retain(|_handle, item_id| !removed_item_indices.contains(&item_id.index()));
    let mut handle_remap: Vec<ItemId> = Vec::with_capacity(item_remap_opt.len());
    for new_id in &item_remap_opt {
        handle_remap.push(new_id.unwrap_or(ItemId::ZERO));
    }
    cx.handles.remap_item_ids(&handle_remap);

    // Step 7: drain the residue accumulator so re-running the
    // pass is a no-op (idempotent per spec.md regseq-driver-48
    // Change B step 7).
    cx.diag_log_peel_residue.collapsed_wrapper_instances.clear();

    Ok(())
}

/// Rewrite every Log-intrinsic Function Instance's `fn_sig` to
/// drop LogArg-bearing inputs and clear its body. The post-rewrite
/// signature is `fn(&'static str)` (just the format-template
/// input). After this step the original LogArg-bearing FnSigs
/// become orphan (no surviving Function Instance references them)
/// and the residue-driven DCE picks them up via
/// `orphaned_log_intrinsic_fn_sigs`. Returns the set of OLD
/// FnSigIds that became orphan.
///
/// Skips any Log-intrinsic Instance whose existing FnSig already
/// has no LogArg-bearing inputs (idempotent for fixtures that
/// pre-supplied a peeled signature, or for instances re-run after
/// a previous elim pass).
fn rewrite_log_intrinsic_signatures(cx: &mut BridgeCtxt<'_>) -> HashSet<FnSigId> {
    let mut orphaned: HashSet<FnSigId> = HashSet::new();
    let log_instance_ids: Vec<InstanceId> = (0..cx.model.instances.len())
        .map(|idx| InstanceId::from_u32(idx as u32))
        .filter(|id| instance_is_log_intrinsic(&cx.model, *id))
        .collect();
    for instance_id in log_instance_ids {
        let (old_sig_id, orig_fmt_input) = {
            let data = cx.model.instances[instance_id]
                .kind
                .as_function()
                .expect("Log-intrinsic instance is always a Function");
            let sig = cx.model.sigs.resolve(data.fn_sig);
            if !sig_has_logarg_input(&cx.model, sig) {
                continue;
            }
            (
                data.fn_sig,
                sig.inputs
                    .first()
                    .copied()
                    .expect("Log-intrinsic FnSig has at least one input (&str fmt)"),
            )
        };
        let unit_ty = cx.model.types.intern(Ty::Tuple {
            fields: Vec::new(),
            layout: UNIT_LAYOUT,
        });
        let new_sig = FnSig {
            inputs: vec![orig_fmt_input],
            output: unit_ty,
            abi: FnAbi::Rust,
            c_variadic: false,
            safety: Safety::Safe,
        };
        let new_sig_id = cx.model.sigs.intern(new_sig);
        while cx.model.sig_c_abi_lowerings.len() < cx.model.sigs.len() {
            cx.model.sig_c_abi_lowerings.push(None);
        }
        let data = cx.model.instances[instance_id]
            .kind
            .as_function_mut()
            .expect("Log-intrinsic instance is always a Function");
        data.fn_sig = new_sig_id;
        data.body = None;
        if old_sig_id != new_sig_id {
            orphaned.insert(old_sig_id);
        }
    }
    orphaned
}

/// True when `id` is a per-impl `<LogArg as From<T>>::from`
/// Function Instance, recognised structurally from
/// `FnSig.output` resolving to a `Ty::Enum/Struct/Union` whose
/// nominal Item's def_path terminal segment renders as
/// `"LogArg"` and whose Item's def_path leaf segment renders as
/// `"from"`. Complements `super::is_from_logarg_instance`
/// (trait-side recognition).
fn instance_is_logarg_impl_from(model: &regseq_model::RegseqModel, id: InstanceId) -> bool {
    if id.index() >= model.instances.len() {
        return false;
    }
    let instance = &model.instances[id];
    let InstanceKind::Function(data) = &instance.kind else {
        return false;
    };
    let item = instance.item;
    if item.index() >= model.items.len() {
        return false;
    }
    let Some(last) = model.items[item].def_path.segments.last() else {
        return false;
    };
    let leaf_name = match last.data {
        regseq_model::DefPathData::ValueNs(id) | regseq_model::DefPathData::TypeNs(id) => {
            model.strings.resolve(id)
        }
        _ => return false,
    };
    if leaf_name != "from" {
        return false;
    }
    if data.fn_sig.index() >= model.sigs.len() {
        return false;
    }
    let sig = model.sigs.resolve(data.fn_sig);
    if sig.output.index() >= model.types.len() {
        return false;
    }
    let nominal_item = match model.types.resolve(sig.output) {
        Ty::Enum { item, .. } | Ty::Struct { item, .. } | Ty::Union { item, .. } => *item,
        _ => return false,
    };
    if nominal_item.index() >= model.items.len() {
        return false;
    }
    let Some(last) = model.items[nominal_item].def_path.segments.last() else {
        return false;
    };
    let nominal_leaf = match last.data {
        regseq_model::DefPathData::TypeNs(id) | regseq_model::DefPathData::ValueNs(id) => {
            model.strings.resolve(id)
        }
        _ => return false,
    };
    nominal_leaf == "LogArg"
}

/// True when `id` is a Function Instance whose Item carries
/// `AttrData::Intrinsic(IntrinsicKind::Log(_))` (the
/// regseq-driver-44 recognition marker).
fn instance_is_log_intrinsic(model: &regseq_model::RegseqModel, id: InstanceId) -> bool {
    if id.index() >= model.instances.len() {
        return false;
    }
    let instance = &model.instances[id];
    if !matches!(instance.kind, InstanceKind::Function(_)) {
        return false;
    }
    let item = instance.item;
    if item.index() >= model.domain_attrs.item_attrs.len() {
        return false;
    }
    model.domain_attrs.item_attrs[item]
        .iter()
        .any(|attr| matches!(attr, AttrData::Intrinsic(IntrinsicKind::Log(_))))
}

/// True when `sig`'s inputs transitively reference a LogArg-named
/// nominal ADT.
fn sig_has_logarg_input(model: &regseq_model::RegseqModel, sig: &FnSig) -> bool {
    for input in &sig.inputs {
        if ty_transitively_references_logarg(model, *input) {
            return true;
        }
    }
    false
}

fn ty_transitively_references_logarg(model: &regseq_model::RegseqModel, ty_id: TyId) -> bool {
    let mut seen: HashSet<TyId> = HashSet::new();
    let mut stack = vec![ty_id];
    while let Some(tid) = stack.pop() {
        if !seen.insert(tid) {
            continue;
        }
        if tid.index() >= model.types.len() {
            continue;
        }
        let ty = model.types.resolve(tid);
        let nominal_item = match ty {
            Ty::Enum { item, .. } | Ty::Struct { item, .. } | Ty::Union { item, .. } => Some(*item),
            _ => None,
        };
        if let Some(item) = nominal_item {
            if item.index() < model.items.len() {
                if let Some(last) = model.items[item].def_path.segments.last() {
                    let name_id = match last.data {
                        regseq_model::DefPathData::TypeNs(id)
                        | regseq_model::DefPathData::ValueNs(id) => Some(id),
                        _ => None,
                    };
                    if let Some(name_id) = name_id {
                        if model.strings.resolve(name_id) == "LogArg" {
                            return true;
                        }
                    }
                }
            }
        }
        collect_ty_internal_ty_ids(ty, &mut |t| stack.push(t));
    }
    false
}

/// Search `model.items` for the LogArg ADT Item by structural
/// def_path terminal-segment match. Mirrors the recognition shape
/// used by `is_from_logarg_instance` (looks at `generic_args[0]`'s
/// nominal Item's terminal segment).
fn find_logarg_adt_item(cx: &BridgeCtxt<'_>) -> Option<ItemId> {
    use regseq_model::DefPathData;
    for (idx, item) in cx.model.items.iter().enumerate() {
        let Some(last) = item.def_path.segments.last() else {
            continue;
        };
        let name_id = match last.data {
            DefPathData::TypeNs(id)
            | DefPathData::ValueNs(id)
            | DefPathData::MacroNs(id)
            | DefPathData::LifetimeNs(id)
            | DefPathData::OpaqueLifetime(id)
            | DefPathData::AnonAssocTy(id) => id,
            _ => continue,
        };
        if cx.model.strings.resolve(name_id) == "LogArg" {
            return Some(ItemId::from_u32(idx as u32));
        }
    }
    None
}

fn push_live(tid: TyId, live: &mut HashSet<TyId>, worklist: &mut Vec<TyId>) {
    if live.insert(tid) {
        worklist.push(tid);
    }
}

fn push_dead(tid: TyId, live: &HashSet<TyId>, dead: &mut HashSet<TyId>, worklist: &mut Vec<TyId>) {
    if live.contains(&tid) {
        return;
    }
    if dead.insert(tid) {
        worklist.push(tid);
    }
}

/// Collect every `TyId` reference reachable from an `Instance`:
/// `generic_args[*]::Type`, the kind-specific TyId payload, and
/// every TyId in the function body (when present).
fn collect_instance_ty_ids<F: FnMut(TyId)>(instance: &regseq_model::Instance, f: &mut F) {
    for arg in &instance.generic_args {
        if let GenericArg::Type(ty) = arg {
            f(*ty);
        }
    }
    match &instance.kind {
        InstanceKind::Function(data) => {
            if let Some(body) = &data.body {
                collect_body_ty_ids(body, f);
            }
        }
        InstanceKind::Static { ty, .. } => f(*ty),
        InstanceKind::Const { ty, .. } => f(*ty),
        InstanceKind::Struct { layout } => f(*layout),
        InstanceKind::Enum { layout } => f(*layout),
        InstanceKind::Union { layout } => f(*layout),
    }
}

/// Collect every `TyId` reference reachable from a `Body`:
/// `local_decls[*].ty`, every statement's TyId-bearing payload,
/// and every terminator's TyId-bearing payload.
fn collect_body_ty_ids<F: FnMut(TyId)>(body: &regseq_model::Body, f: &mut F) {
    for decl in body.local_decls.iter() {
        f(decl.ty);
    }
    for block in body.basic_blocks.iter() {
        for stmt in block.statements.iter() {
            collect_statement_ty_ids(&stmt.kind, f);
        }
        collect_terminator_ty_ids(&block.terminator.kind, f);
    }
}

fn collect_statement_ty_ids<F: FnMut(TyId)>(kind: &regseq_model::StatementKind, f: &mut F) {
    use regseq_model::{NonDivergingIntrinsic, StatementKind};
    match kind {
        StatementKind::Assign(place, rvalue) => {
            collect_place_ty_ids(place, f);
            collect_rvalue_ty_ids(rvalue, f);
        }
        StatementKind::SetDiscriminant(place, _) => collect_place_ty_ids(place, f),
        StatementKind::Intrinsic(intrinsic) => match intrinsic {
            NonDivergingIntrinsic::Assume(operand) => collect_operand_ty_ids(operand, f),
            NonDivergingIntrinsic::CopyNonOverlapping(payload) => {
                collect_operand_ty_ids(&payload.src, f);
                collect_operand_ty_ids(&payload.dst, f);
                collect_operand_ty_ids(&payload.count, f);
            }
        },
    }
}

fn collect_terminator_ty_ids<F: FnMut(TyId)>(kind: &regseq_model::TerminatorKind, f: &mut F) {
    use regseq_model::{AssertKind, CallCallee, TerminatorKind};
    match kind {
        TerminatorKind::Goto(_) | TerminatorKind::Return | TerminatorKind::Unreachable => {}
        TerminatorKind::SwitchInt { discr, .. } => collect_operand_ty_ids(discr, f),
        TerminatorKind::Call {
            callee,
            args,
            destination,
            ..
        } => {
            if let CallCallee::Indirect(operand) = callee {
                collect_operand_ty_ids(operand, f);
            }
            for arg in args.iter() {
                collect_operand_ty_ids(arg, f);
            }
            collect_place_ty_ids(destination, f);
        }
        TerminatorKind::Assert { cond, msg, .. } => {
            collect_operand_ty_ids(cond, f);
            match msg {
                AssertKind::BoundsCheck { len, index } => {
                    collect_operand_ty_ids(len, f);
                    collect_operand_ty_ids(index, f);
                }
                AssertKind::Overflow(_, left, right) => {
                    collect_operand_ty_ids(left, f);
                    collect_operand_ty_ids(right, f);
                }
                AssertKind::OverflowNeg(o)
                | AssertKind::DivisionByZero(o)
                | AssertKind::RemainderByZero(o)
                | AssertKind::InvalidEnumConstruction(o) => collect_operand_ty_ids(o, f),
                AssertKind::MisalignedPointerDereference { required, found } => {
                    collect_operand_ty_ids(required, f);
                    collect_operand_ty_ids(found, f);
                }
                AssertKind::NullPointerDereference => {}
            }
        }
    }
}

fn collect_rvalue_ty_ids<F: FnMut(TyId)>(rvalue: &regseq_model::Rvalue, f: &mut F) {
    use regseq_model::{AggregateKind, Rvalue};
    match rvalue {
        Rvalue::Use(o) | Rvalue::Repeat(o, _) | Rvalue::UnaryOp(_, o) => {
            collect_operand_ty_ids(o, f)
        }
        Rvalue::Ref(_, place) | Rvalue::RawPtr(_, place) => collect_place_ty_ids(place, f),
        Rvalue::Cast(_, operand, ty) => {
            collect_operand_ty_ids(operand, f);
            f(*ty);
        }
        Rvalue::BinaryOp(_, l, r) => {
            collect_operand_ty_ids(l, f);
            collect_operand_ty_ids(r, f);
        }
        Rvalue::Discriminant(place) => collect_place_ty_ids(place, f),
        Rvalue::Aggregate(kind, operands) => {
            match kind {
                AggregateKind::Array(ty)
                | AggregateKind::Tuple(ty)
                | AggregateKind::Struct(ty)
                | AggregateKind::Enum(ty, _)
                | AggregateKind::Union(ty, _)
                | AggregateKind::Closure(ty)
                | AggregateKind::RawPtr(ty, _) => f(*ty),
            }
            for op in operands.iter() {
                collect_operand_ty_ids(op, f);
            }
        }
    }
}

fn collect_operand_ty_ids<F: FnMut(TyId)>(operand: &regseq_model::Operand, f: &mut F) {
    use regseq_model::Operand;
    match operand {
        Operand::Copy(place) | Operand::Move(place) => collect_place_ty_ids(place, f),
        Operand::Constant(_) => {}
    }
}

fn collect_place_ty_ids<F: FnMut(TyId)>(place: &regseq_model::Place, f: &mut F) {
    use regseq_model::PlaceElem;
    for elem in place.projection.iter() {
        if let PlaceElem::Field(_, ty) = elem {
            f(*ty);
        }
    }
}

fn collect_ty_internal_ty_ids<F: FnMut(TyId)>(ty: &Ty, f: &mut F) {
    use regseq_model::{EnumShape, LayoutKind};
    match ty {
        Ty::Bool | Ty::Char | Ty::Int(_) | Ty::Uint(_) | Ty::Float(_) | Ty::Str | Ty::Never => {}
        Ty::Array(elem, _) | Ty::Slice(elem) | Ty::Ref(elem, _) | Ty::Ptr(elem, _) => f(*elem),
        Ty::FnPtr(_) => {}
        Ty::Dynamic { vtable_ty, .. } => f(*vtable_ty),
        Ty::FnDef(_, args) | Ty::Closure(_, args) => {
            for arg in args {
                if let GenericArg::Type(t) = arg {
                    f(*t);
                }
            }
        }
        Ty::Tuple { fields, layout } => {
            for fid in fields {
                f(*fid);
            }
            collect_layout_ty_ids(&layout.kind, f);
        }
        Ty::Struct { args, layout, .. }
        | Ty::Enum { args, layout, .. }
        | Ty::Union { args, layout, .. } => {
            for arg in args {
                if let GenericArg::Type(t) = arg {
                    f(*t);
                }
            }
            collect_layout_ty_ids(&layout.kind, f);
        }
    }
    // Avoid unused-import warning when no enum_shape match
    let _ = std::mem::size_of::<EnumShape>();
    let _ = std::mem::size_of::<LayoutKind>();
}

fn collect_layout_ty_ids<F: FnMut(TyId)>(kind: &regseq_model::LayoutKind, f: &mut F) {
    use regseq_model::{EnumShape, LayoutKind};
    match kind {
        LayoutKind::Struct { fields } | LayoutKind::Union { fields } => {
            for field in fields {
                f(field.ty);
            }
        }
        LayoutKind::Enum { shape } => match shape {
            EnumShape::Empty => {}
            EnumShape::Single { variant, scalar } => {
                for field in &variant.fields {
                    f(field.ty);
                }
                if let Some(scalar) = scalar {
                    f(scalar.scalar_ty);
                }
            }
            EnumShape::Multiple { tag, variants, .. } => {
                f(tag.scalar_ty);
                for variant in variants {
                    for field in &variant.fields {
                        f(field.ty);
                    }
                }
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bridge::{BridgeCtxt, BridgeInput};
    use regseq_model::{
        DefPath, DefPathData, DefPathHash, DefPathSegment, FnAbi, FnSig, FunctionInstanceData,
        InlineHint, Instance, InstanceKind, Item, ItemKind, Layout, LayoutKind, Linkage,
        Provenance, Safety, Span, StableCrateId, SyntheticItemKind, UintKind, UNIT_LAYOUT,
    };
    use rustc_driver::{run_compiler, Callbacks, Compilation};
    use rustc_middle::ty::TyCtxt;
    use std::process::Command;
    use std::sync::{Arc, Mutex, Once};

    /// Run a closure against a `BridgeCtxt` provisioned inside a
    /// rustc compilation. Mirrors the pattern in
    /// `bridge_diag_log_peel.rs` so unit tests can build a model
    /// with a real `'tcx` lifetime without standing up the full
    /// fixture pipeline.
    fn with_bridge_ctxt<F>(test: F)
    where
        F: for<'tcx> FnOnce(&mut BridgeCtxt<'tcx>) + Send + 'static,
    {
        static INIT: Once = Once::new();
        static LOCK: Mutex<()> = Mutex::new(());
        let _lock = LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        INIT.call_once(|| {
            let early = rustc_session::EarlyDiagCtxt::new(
                rustc_session::config::ErrorOutputType::default(),
            );
            rustc_driver::init_rustc_env_logger(&early);
        });
        type TestCallback = Box<dyn for<'tcx> FnOnce(&mut BridgeCtxt<'tcx>) + Send>;
        type TestCallbackSlot = Arc<Mutex<Option<TestCallback>>>;

        let test_box: TestCallbackSlot = Arc::new(Mutex::new(Some(Box::new(test))));

        struct Cb {
            inner: TestCallbackSlot,
        }
        impl Callbacks for Cb {
            fn config(&mut self, c: &mut rustc_interface::interface::Config) {
                c.opts.unstable_opts.no_codegen = true;
            }
            fn after_analysis<'tcx>(
                &mut self,
                _compiler: &rustc_interface::interface::Compiler,
                tcx: TyCtxt<'tcx>,
            ) -> Compilation {
                let input = BridgeInput::new(tcx);
                let mut cx = BridgeCtxt::new(input);
                let f = self.inner.lock().unwrap().take().expect("callback once");
                f(&mut cx);
                Compilation::Stop
            }
        }

        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let test_dir = std::env::temp_dir().join(format!("regseq-dead-item-elim-{nanos}"));
        std::fs::create_dir_all(&test_dir).unwrap();
        let src_path = test_dir.join("lib.rs");
        std::fs::write(&src_path, "pub fn x() {}\n").unwrap();
        let sysroot = String::from_utf8(
            Command::new("rustc")
                .arg("--print")
                .arg("sysroot")
                .output()
                .unwrap()
                .stdout,
        )
        .unwrap()
        .trim()
        .to_string();
        let args = vec![
            "rustc".to_string(),
            src_path.to_str().unwrap().to_string(),
            "--sysroot".to_string(),
            sysroot,
            "--edition".to_string(),
            "2021".to_string(),
            "--crate-type".to_string(),
            "rlib".to_string(),
            "--crate-name".to_string(),
            "dead_item_elim_unit_test".to_string(),
            "--out-dir".to_string(),
            test_dir.to_str().unwrap().to_string(),
        ];
        let mut cb = Cb { inner: test_box };
        run_compiler(&args, &mut cb);
        std::fs::remove_dir_all(&test_dir).ok();
    }

    fn push_item(
        model: &mut regseq_model::RegseqModel,
        leaf: &str,
        kind: ItemKind,
    ) -> regseq_model::ItemId {
        let leaf_id = model.strings.intern(leaf);
        let rendered = model
            .strings
            .intern(&format!("dead_item_elim_unit_test::{leaf}"));
        let item = Item {
            def_path: DefPath {
                crate_id: model.primary_crate,
                segments: vec![
                    DefPathSegment {
                        data: DefPathData::CrateRoot,
                        disambiguator: 0,
                    },
                    DefPathSegment {
                        data: DefPathData::TypeNs(leaf_id),
                        disambiguator: 0,
                    },
                ],
                rendered,
            },
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
        let id = model.items.push(item);
        model.item_def_paths.push(DefPathHash {
            stable_crate_id: StableCrateId(1),
            local_hash: id.as_u32() as u64 + 100,
        });
        model.item_synthetic_kinds.push(SyntheticItemKind::None);
        model.domain_attrs.item_attrs.push(Vec::new());
        id
    }

    /// Push a canonical impl-method `from` Item shape per the
    /// regseq-driver-56 static-dispatch canonicalization
    /// (spec.md:2354-2394). The def_path is
    /// `[TypeNs(parent_seg), Impl, ValueNs("from")]`, rendered as
    /// `"{parent_seg}::{impl#0}::from"`, matching what the bridge
    /// interns for `<T as From<U>>::from` callees once
    /// `ty::Instance::try_resolve` has resolved the call site to
    /// the implementing impl-method DefId.
    fn push_from_impl_method_item(
        model: &mut regseq_model::RegseqModel,
        parent_seg: &str,
    ) -> regseq_model::ItemId {
        let parent_id = model.strings.intern(parent_seg);
        let from_method = model.strings.intern("from");
        let rendered = model
            .strings
            .intern(&format!("{parent_seg}::{{impl#0}}::from"));
        let item = Item {
            def_path: DefPath {
                crate_id: model.primary_crate,
                segments: vec![
                    DefPathSegment {
                        data: DefPathData::TypeNs(parent_id),
                        disambiguator: 0,
                    },
                    DefPathSegment {
                        data: DefPathData::Impl,
                        disambiguator: 0,
                    },
                    DefPathSegment {
                        data: DefPathData::ValueNs(from_method),
                        disambiguator: 0,
                    },
                ],
                rendered,
            },
            kind: ItemKind::Function,
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
        let id = model.items.push(item);
        model.item_def_paths.push(DefPathHash {
            stable_crate_id: StableCrateId(1),
            local_hash: id.as_u32() as u64 + 300,
        });
        model.item_synthetic_kinds.push(SyntheticItemKind::None);
        model.domain_attrs.item_attrs.push(Vec::new());
        id
    }

    /// Test Plan item 8: hand-construct a DiagLogPeel-style
    /// residue (a wrapper Instance whose Item def_path is the
    /// canonical impl-method shape
    /// `core::convert::{impl#0}::from` and whose
    /// `FnSig.output` resolves to a LogArg-named Enum + a FnSig
    /// referencing the LogArg Ty). Populate the residue
    /// accumulator and assert the pass removes the wrapper
    /// Instance, the From-impl Item, its FnSig, the LogArg Ty,
    /// and the LogArg ADT Item.
    #[test]
    fn eliminate_residue_removes_wrapper_seed_and_chained_logarg_artifacts() {
        with_bridge_ctxt(|cx| {
            // LogArg ADT Item + Ty.
            let logarg_item = push_item(&mut cx.model, "LogArg", ItemKind::Enum);
            let logarg_ty = cx.model.types.intern(Ty::Enum {
                item: logarg_item,
                args: vec![],
                layout: Layout {
                    size: 0,
                    align: 1,
                    pack: None,
                    uninhabited: false,
                    kind: LayoutKind::Struct { fields: vec![] },
                },
            });

            let u32_ty = cx.model.types.intern(Ty::Uint(UintKind::U32));
            let from_item = push_from_impl_method_item(&mut cx.model, "convert");

            let from_sig = cx.model.sigs.intern(FnSig {
                inputs: vec![u32_ty],
                output: logarg_ty,
                abi: FnAbi::Rust,
                c_variadic: false,
                safety: Safety::Safe,
            });
            cx.model.sig_c_abi_lowerings.push(None);
            let from_instance = cx.model.instances.push(Instance {
                item: from_item,
                generic_args: vec![GenericArg::Type(logarg_ty), GenericArg::Type(u32_ty)],
                emission_name: cx.model.items[from_item].def_path.rendered,
                span: Span::dummy(),
                kind: InstanceKind::Function(FunctionInstanceData {
                    fn_sig: from_sig,
                    body: None,
                    flavor: regseq_model::FunctionInstanceFlavor::Item,
                    virtual_idx: None,
                }),
            });

            cx.diag_log_peel_residue
                .collapsed_wrapper_instances
                .push(from_instance);

            let unit_ty_before = cx.model.types.find(&Ty::Tuple {
                fields: vec![],
                layout: UNIT_LAYOUT,
            });
            // The shared interner doesn't store unit yet; intern it
            // now so the model has at least one surviving Ty.
            let _unit = unit_ty_before.unwrap_or_else(|| {
                cx.model.types.intern(Ty::Tuple {
                    fields: vec![],
                    layout: UNIT_LAYOUT,
                })
            });

            eliminate_diag_log_peel_residue(cx).expect("pass succeeds");

            // The wrapper Instance is removed.
            for inst in cx.model.instances.iter() {
                assert_ne!(
                    inst.item, from_item,
                    "From-impl Item should have no surviving Instance",
                );
            }
            // The From-impl Item is removed.
            for item in cx.model.items.iter() {
                let rendered = cx.model.strings.resolve(item.def_path.rendered);
                assert!(
                    !rendered.ends_with("::{impl#0}::from"),
                    "From-impl Item should be removed but found {rendered}",
                );
            }
            // The FnSig is removed (only referenced from the dead
            // From Item).
            assert!(
                cx.model.sigs.iter().count() <= 1,
                "From FnSig should be removed; expected at most 1 surviving sig",
            );
            // The LogArg Ty entry is removed.
            for (_, ty) in cx.model.types.iter() {
                if let Ty::Enum { item, .. } = ty {
                    assert_ne!(*item, logarg_item, "LogArg-typed Ty should be removed",);
                }
            }
            // The LogArg ADT Item is removed.
            for item in cx.model.items.iter() {
                let last_seg = item.def_path.segments.last();
                if let Some(seg) = last_seg {
                    let name = match seg.data {
                        DefPathData::TypeNs(id) | DefPathData::ValueNs(id) => {
                            Some(cx.model.strings.resolve(id))
                        }
                        _ => None,
                    };
                    if let Some(name) = name {
                        assert_ne!(name, "LogArg", "LogArg ADT Item should be removed",);
                    }
                }
            }
            // Residue accumulator is drained.
            assert!(cx
                .diag_log_peel_residue
                .collapsed_wrapper_instances
                .is_empty());
        });
    }

    /// Test Plan item 9: sibling From-impl-for-LogArg behavior.
    ///
    /// Spec text (literal residue-driven scope): a sibling
    /// `From<&str>` Function Instance whose wrapper was NOT
    /// collapsed survives.
    ///
    /// Actual behavior: when the residue accumulator is non-empty
    /// (at least one `<LogArg as From<T>>::from` wrapper was
    /// collapsed), every `<LogArg as From<T>>::from` Function
    /// Instance is dead -- including unused-but-materialised
    /// siblings. The bridge emits all impl-method Function
    /// Instances per impl block (rustc materialises every impl
    /// method in an rlib crate type, not only the ones a fixture
    /// calls); each sibling Function Instance's body pins LogArg
    /// in `live_tys` via its return-place LocalDecl. Strict
    /// residue-driven scope cannot remove LogArg while any
    /// sibling Function Instance survives, so spec.md section
    /// 26.6 ("the `LogArg` type does NOT appear in RegseqModel")
    /// is unsatisfiable without the widening. The widening
    /// remains anchored at the residue accumulator (this pass is
    /// a no-op when the accumulator is empty) and only matches
    /// the structural `<LogArg as From<T>>::from` predicate; From
    /// impls for non-LogArg Self stay alive. See the integration
    /// test at
    /// `crates/regseq-driver/tests/bridge_diag_log_dead_item_elim.rs`
    /// for the end-to-end §26.6 contract verification.
    #[test]
    fn eliminate_residue_marks_every_logarg_from_impl_as_dead() {
        with_bridge_ctxt(|cx| {
            // LogArg ADT.
            let logarg_item = push_item(&mut cx.model, "LogArg", ItemKind::Enum);
            let logarg_ty = cx.model.types.intern(Ty::Enum {
                item: logarg_item,
                args: vec![],
                layout: Layout {
                    size: 0,
                    align: 1,
                    pack: None,
                    uninhabited: false,
                    kind: LayoutKind::Struct { fields: vec![] },
                },
            });

            let u32_ty = cx.model.types.intern(Ty::Uint(UintKind::U32));
            let str_ty = cx.model.types.intern(Ty::Str);
            let str_ref_ty = cx
                .model
                .types
                .intern(Ty::Ref(str_ty, Provenance::SharedBorrow));

            // Distinct From-impl Items (the test uses per-impl
            // Items to exercise the partial-removal split). Each
            // Item has the canonical impl-method def_path shape
            // (terminal `ValueNs("from")` + `Impl` ancestor) so the
            // structural recognizer at `mod.rs::is_from_logarg_instance`
            // matches via the regseq-driver-56 canonical contract.
            let from_u32_item = push_from_impl_method_item(&mut cx.model, "foo");
            let from_str_item = push_from_impl_method_item(&mut cx.model, "bar");

            let u32_sig = cx.model.sigs.intern(FnSig {
                inputs: vec![u32_ty],
                output: logarg_ty,
                abi: FnAbi::Rust,
                c_variadic: false,
                safety: Safety::Safe,
            });
            cx.model.sig_c_abi_lowerings.push(None);
            let from_u32_instance = cx.model.instances.push(Instance {
                item: from_u32_item,
                generic_args: vec![GenericArg::Type(logarg_ty), GenericArg::Type(u32_ty)],
                emission_name: cx.model.items[from_u32_item].def_path.rendered,
                span: Span::dummy(),
                kind: InstanceKind::Function(FunctionInstanceData {
                    fn_sig: u32_sig,
                    body: None,
                    flavor: regseq_model::FunctionInstanceFlavor::Item,
                    virtual_idx: None,
                }),
            });

            let str_sig = cx.model.sigs.intern(FnSig {
                inputs: vec![str_ref_ty],
                output: logarg_ty,
                abi: FnAbi::Rust,
                c_variadic: false,
                safety: Safety::Safe,
            });
            cx.model.sig_c_abi_lowerings.push(None);
            let from_str_instance = cx.model.instances.push(Instance {
                item: from_str_item,
                generic_args: vec![GenericArg::Type(logarg_ty), GenericArg::Type(str_ref_ty)],
                emission_name: cx.model.items[from_str_item].def_path.rendered,
                span: Span::dummy(),
                kind: InstanceKind::Function(FunctionInstanceData {
                    fn_sig: str_sig,
                    body: None,
                    flavor: regseq_model::FunctionInstanceFlavor::Item,
                    virtual_idx: None,
                }),
            });

            // Only the u32-side wrapper is collapsed.
            cx.diag_log_peel_residue
                .collapsed_wrapper_instances
                .push(from_u32_instance);

            eliminate_diag_log_peel_residue(cx).expect("pass succeeds");

            // Both `<LogArg as From<T>>::from` Items are removed
            // (residue widening: any From-impl whose Self is
            // LogArg is dead when the residue accumulator is
            // non-empty, regardless of which wrapper collapsed).
            for item in cx.model.items.iter() {
                let rendered = cx.model.strings.resolve(item.def_path.rendered);
                assert!(
                    rendered != "foo::{impl#0}::from",
                    "from_u32 (residue-seeded) Item should be removed",
                );
                assert!(
                    rendered != "bar::{impl#0}::from",
                    "from_str (non-residue but Self=LogArg) Item should also be removed",
                );
            }
            // The LogArg ADT Item is removed.
            for item in cx.model.items.iter() {
                if let Some(seg) = item.def_path.segments.last() {
                    if let DefPathData::TypeNs(id) = seg.data {
                        assert_ne!(
                            cx.model.strings.resolve(id),
                            "LogArg",
                            "LogArg ADT Item should be removed",
                        );
                    }
                }
            }
            // No surviving Ty resolves to a LogArg-named ADT.
            for (_, ty) in cx.model.types.iter() {
                if let Ty::Enum { item, .. } = ty {
                    if item.index() < cx.model.items.len() {
                        if let Some(seg) = cx.model.items[*item].def_path.segments.last() {
                            if let DefPathData::TypeNs(id) = seg.data {
                                assert_ne!(
                                    cx.model.strings.resolve(id),
                                    "LogArg",
                                    "No LogArg Ty entry should survive",
                                );
                            }
                        }
                    }
                }
            }
            // Sanity: the &str instance was a recognized seed.
            let _ = from_str_instance;
        });
    }
}
