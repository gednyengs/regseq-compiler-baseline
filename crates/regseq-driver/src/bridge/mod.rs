//! Rustc-to-`RegseqModel` bridge pipeline scaffold.

use std::sync::{Arc, Mutex};

use rustc_middle::mir::{mono::MonoItem, CastKind, Rvalue, StatementKind, TerminatorKind};
use rustc_middle::ty::{TyCtxt, TyKind};
use rustc_span::def_id::{DefId, LOCAL_CRATE};
use std::collections::HashMap;
use thiserror::Error;

use regseq_model::{
    args_key_for_generic_args_with_pending, canonical_ty_bytes_with_pending, DefPathHash,
    GenericArg, Item, PrincipalTraitKey, ProjectionBindingsKey, RegseqModel, StableCrateId,
    SyntheticItemKind, Ty, TyId,
};

mod assign;
pub mod body;
pub mod const_;
pub mod diag_log;
mod discovery;
pub mod fnsig;
mod handles;
mod sort;
pub mod synth;
pub mod ty;

pub use const_::ConstMemoryStaticHandle;
pub use handles::{HandleTable, ItemHandle};
pub use synth::{ClosureSynthHandles, VTableSynthHandles};

/// Inputs the bridge consumes from the rustc host.
pub struct BridgeInput<'tcx> {
    /// Rustc type context for the compilation being bridged.
    pub tcx: TyCtxt<'tcx>,
    /// Phase-1 mono-item closure, populated by discovery.
    pub discovered_mono_items: Arc<Mutex<Vec<MonoItem<'tcx>>>>,
}

impl<'tcx> BridgeInput<'tcx> {
    /// Creates bridge input for a rustc compilation.
    pub fn new(tcx: TyCtxt<'tcx>) -> Self {
        Self {
            tcx,
            discovered_mono_items: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

/// Item record accumulated before final `ItemId` assignment.
#[derive(Clone, Debug)]
pub struct PendingItem {
    /// Phase-2 handle used for cross-references before Phase 4.
    pub handle: ItemHandle,
    /// Model item payload.
    pub item: Item,
    /// Stable DefPathHash side-table payload for this item.
    pub def_path_hash: DefPathHash,
    /// Synthetic classification side-table payload for this item.
    pub synthetic_kind: SyntheticItemKind,
}

/// Mutable working state threaded through every bridge phase.
pub struct BridgeCtxt<'tcx> {
    /// Rustc-side bridge input and Phase-1 discovery output.
    pub input: BridgeInput<'tcx>,
    /// Model under construction.
    pub model: RegseqModel,
    /// Phase-2 handle table remapped to final `ItemId`s in Phase 4.
    pub handles: HandleTable,
    /// Item payloads accumulated before F4 sort and final assignment.
    pub pending_items: Vec<PendingItem>,
    /// Phase-2 cache for bridge-synthesised ConstMemoryStatic items.
    pub const_memory_static_cache: Vec<ConstMemoryStaticHandle>,
    /// Phase-2 cache for bridge-synthesised closure-family items.
    pub closure_synth_cache: Vec<ClosureSynthHandles>,
    /// Phase-2 cache for bridge-synthesised vtable type items.
    pub vtable_type_synth_cache: HashMap<PrincipalTraitKey, ItemHandle>,
    /// Phase-2 cache for bridge-synthesised vtable-family items.
    pub vtable_synth_cache: HashMap<synth::VTableSynthKey, VTableSynthHandles>,
    /// Phase-2 cache for bridge-synthesised drop-glue items.
    pub drop_glue_synth_cache: HashMap<TyId, ItemHandle>,
    /// Phase-2 cache for diagnostic-logging intrinsic callees.
    pub diag_log_intrinsics: HashMap<DefPathHash, regseq_model::IntrinsicKind>,
    /// Deferred resolution side-table for `Ty::Dynamic.vtable_ty`
    /// `Ty::Struct` views interned during Phase 2 with placeholder
    /// `ItemId`s. Per spec.md section 3.4 D-12-5.3, final `ItemId`s
    /// are not assigned until Phase 4; this map records each
    /// placeholder `Ty::Struct` so the bridge can patch it to the
    /// real `ItemId` once Phase 4 binds the synth/vtable.rs handle.
    pub dynamic_vtable_ty_pending: HashMap<ItemHandle, TyId>,
    /// Dedup cache for source-derived items (`Struct`, `Enum`,
    /// `Union`, `FnDef`, closure captures). One `ItemHandle` per
    /// `DefPathHash` regardless of how many monomorphisations the
    /// bridge sees. Per spec.md section 3.4 D-12-5.3 the final
    /// `ItemId` is assigned at Phase 4; Phase 2 uses placeholder
    /// `ItemId`s derived from these handles and the
    /// `source_ty_pending` side-table records the placeholder
    /// `TyId`s so Phase 5 can patch them to the real `ItemId`s.
    pub source_item_handles: HashMap<DefPathHash, ItemHandle>,
    /// Deferred-resolution side-table for source-derived nominal
    /// `Ty` entries (`Ty::Struct`, `Ty::Enum`, `Ty::Union`,
    /// `Ty::FnDef`, `Ty::Closure`) interned during Phase 2 with
    /// placeholder `ItemId`s. Phase 5 walks this map and calls
    /// `TyInterner::patch_nominal_item_id` for each entry once the
    /// handles bind their final `ItemId`s.
    pub source_ty_pending: HashMap<ItemHandle, Vec<TyId>>,
}

impl<'tcx> BridgeCtxt<'tcx> {
    /// Creates working state for the five-phase bridge pipeline.
    pub fn new(input: BridgeInput<'tcx>) -> Self {
        let crate_name = input.tcx.crate_name(LOCAL_CRATE).to_string();
        let stable_crate_id = StableCrateId(input.tcx.stable_crate_id(LOCAL_CRATE).as_u64());
        let model = RegseqModel::with_primary(&crate_name, stable_crate_id);

        Self {
            input,
            model,
            handles: HandleTable::new(),
            pending_items: Vec::new(),
            const_memory_static_cache: Vec::new(),
            closure_synth_cache: Vec::new(),
            vtable_type_synth_cache: HashMap::new(),
            vtable_synth_cache: HashMap::new(),
            drop_glue_synth_cache: HashMap::new(),
            diag_log_intrinsics: HashMap::new(),
            dynamic_vtable_ty_pending: HashMap::new(),
            source_item_handles: HashMap::new(),
            source_ty_pending: HashMap::new(),
        }
    }
}

impl<'tcx> BridgeCtxt<'tcx> {
    /// Resolve an `ItemId` (real OR Phase-2 placeholder) to its
    /// `DefPath`. Used by Phase-2 callers that need a callee's
    /// rendered path before Phase-4 binds the final `ItemId`. Returns
    /// `None` if the `ItemId` is out of bounds for both
    /// `model.items` and `pending_items`.
    pub(crate) fn lookup_item_def_path(
        &self,
        item: regseq_model::ItemId,
    ) -> Option<&regseq_model::DefPath> {
        let idx = item.as_u32() as usize;
        if idx < self.model.items.len() {
            return Some(&self.model.items[item].def_path);
        }
        let handle_idx = regseq_model::ItemId::MAX_AS_U32 - item.as_u32();
        let handle = ItemHandle::from_u32(handle_idx);
        self.pending_items
            .iter()
            .find(|pending| pending.handle == handle)
            .map(|pending| &pending.item.def_path)
    }

    /// Resolve an `ItemId` to its `ItemKind`.
    pub(crate) fn lookup_item_kind(
        &self,
        item: regseq_model::ItemId,
    ) -> Option<regseq_model::ItemKind> {
        let idx = item.as_u32() as usize;
        if idx < self.model.items.len() {
            return Some(self.model.items[item].kind.clone());
        }
        let handle_idx = regseq_model::ItemId::MAX_AS_U32 - item.as_u32();
        let handle = ItemHandle::from_u32(handle_idx);
        self.pending_items
            .iter()
            .find(|pending| pending.handle == handle)
            .map(|pending| pending.item.kind.clone())
    }

    /// Resolve an `ItemId` to its `DefPathHash`.
    pub(crate) fn lookup_item_def_path_hash(
        &self,
        item: regseq_model::ItemId,
    ) -> Option<DefPathHash> {
        let idx = item.as_u32() as usize;
        if idx < self.model.item_def_paths.len() {
            return Some(self.model.item_def_paths[item]);
        }
        let handle_idx = regseq_model::ItemId::MAX_AS_U32 - item.as_u32();
        let handle = ItemHandle::from_u32(handle_idx);
        self.pending_items
            .iter()
            .find(|pending| pending.handle == handle)
            .map(|pending| pending.def_path_hash)
    }

    /// Resolve an `ItemId` to its `SyntheticItemKind`.
    pub(crate) fn lookup_item_synthetic_kind(
        &self,
        item: regseq_model::ItemId,
    ) -> Option<regseq_model::SyntheticItemKind> {
        let idx = item.as_u32() as usize;
        if idx < self.model.item_synthetic_kinds.len() {
            return Some(self.model.item_synthetic_kinds[item].clone());
        }
        let handle_idx = regseq_model::ItemId::MAX_AS_U32 - item.as_u32();
        let handle = ItemHandle::from_u32(handle_idx);
        self.pending_items
            .iter()
            .find(|pending| pending.handle == handle)
            .map(|pending| pending.synthetic_kind.clone())
    }

    /// Build the placeholder-`ItemId` → `DefPathHash` overlay used by
    /// `regseq_model::args_key_for_generic_args_with_pending` and
    /// `canonical_ty_bytes_with_pending`. Walks `pending_items` and
    /// keys every pending handle by its placeholder `ItemId` so that
    /// nominal `Ty` encoding during Phase 2 (e.g.,
    /// `principal_trait_key` for a vtable whose trait args include a
    /// source-derived ADT) resolves to the rustc-stable `DefPathHash`
    /// instead of panicking on an out-of-bounds index into
    /// `model.item_def_paths`. Post-Phase-4 the map is empty
    /// (`pending_items` has been drained into `model.items` /
    /// `model.item_def_paths`), so callers can use this helper
    /// regardless of phase.
    pub(crate) fn pending_def_path_hashes(&self) -> HashMap<regseq_model::ItemId, DefPathHash> {
        self.pending_items
            .iter()
            .map(|pending| {
                (
                    placeholder_item_id_for_handle(pending.handle),
                    pending.def_path_hash,
                )
            })
            .collect()
    }
}

/// Drive Phase 3 (F4 sort) and Phase 4 (final `ItemId` assignment +
/// side-table population) for tests that need real `ItemId`s. After
/// this returns, every `Item` recorded in `pending_items` has been
/// pushed to `model.items` in F4-deterministic order and its
/// `ItemHandle` resolves to a final `ItemId`. Subsequent
/// [`synth::rekey_synthesis_in_phase5`] calls patch placeholder
/// `Ty` entries (recorded in `source_ty_pending` /
/// `dynamic_vtable_ty_pending`) to the bound `ItemId`s, mirroring
/// what `run_bridge` does in production.
pub fn flush_pending_items_for_tests(ctxt: &mut BridgeCtxt<'_>) -> Result<(), BridgeError> {
    sort::f4_sort(&ctxt.model, &mut ctxt.pending_items)?;
    assign::assign_item_ids_and_populate_sidetables(ctxt)?;
    Ok(())
}

/// Derive the Phase-2 placeholder `ItemId` for a Phase-2 `ItemHandle`.
/// Final `ItemId`s are assigned at Phase 4 per spec.md section 3.4
/// D-12-5.3; the placeholder occupies the high end of `ItemId` space
/// (mirroring `synthesize_dynamic_vtable_ty`'s pattern at
/// `bridge/ty.rs`) so it cannot collide with any real Phase-4
/// assignment which fills `ItemId` densely from zero.
pub(crate) fn placeholder_item_id_for_handle(handle: ItemHandle) -> regseq_model::ItemId {
    regseq_model::ItemId::from_u32(regseq_model::ItemId::MAX_AS_U32 - handle.as_u32())
}

/// Position within an `extern "C"` function signature where an
/// unsupported type was encountered. Spec section 14.5 treats Input,
/// Output, and Field-of-aggregate admissibility separately, so the
/// diagnostic must report which kind of position triggered the
/// rejection.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum CAbiTypePosition {
    /// Argument at the given positional index.
    Input(usize),
    /// Return type position.
    Output,
    /// Field of an aggregate that was being inspected for
    /// field-position admissibility.
    Field,
}

/// Errors returnable from the bridge.
#[derive(Debug, Error)]
pub enum BridgeError {
    /// A type is rejected by the driver DSL policy.
    #[error("DSL rejects {kind:?} type: {ty_repr}")]
    DslRejection {
        /// Rejection family.
        kind: DslRejectionKind,
        /// Rendered rustc type.
        ty_repr: String,
    },
    /// A non-rigid type survived normalization.
    #[error("non-rigid TyKind survived normalization: {kind:?} in {ty_repr}")]
    NonRigidTy {
        /// Non-rigid TyKind family.
        kind: NonRigidTyKind,
        /// Rendered rustc type.
        ty_repr: String,
    },
    /// A foreign type reached type translation.
    #[error("foreign type is unsupported: {ty_repr}")]
    Foreign {
        /// Rendered rustc type.
        ty_repr: String,
    },
    /// A coroutine-family type reached type translation.
    #[error("coroutine type is unsupported: {ty_repr}")]
    Coroutine {
        /// Rendered rustc type.
        ty_repr: String,
    },
    /// A pattern/refinement type reached type translation.
    #[error("pattern type is unsupported: {ty_repr}")]
    Pat {
        /// Rendered rustc type.
        ty_repr: String,
    },
    /// An unsafe binder reached type translation.
    #[error("unsafe binder type is unsupported: {ty_repr}")]
    UnsafeBinder {
        /// Rendered rustc type.
        ty_repr: String,
    },
    /// A dynamic type has no principal trait.
    #[error("auto-only dyn type is unsupported: {ty_repr}")]
    DynAutoOnly {
        /// Rendered rustc type.
        ty_repr: String,
    },
    /// A dyn-star type reached type translation.
    #[error("dyn-star type is unsupported: {ty_repr}")]
    DynStar {
        /// Rendered rustc type.
        ty_repr: String,
    },
    /// A custom auto trait reached dynamic type translation.
    #[error("custom auto trait is unsupported: {trait_repr} in {ty_repr}")]
    CustomAutoTrait {
        /// Rendered custom auto trait.
        trait_repr: String,
        /// Rendered rustc type.
        ty_repr: String,
    },
    /// A substantive higher-ranked binder would lose type identity when erased.
    #[error("substantive HRTB is unsupported: {binder_repr}")]
    SubstantiveHrtb {
        /// Rendered binder.
        binder_repr: String,
    },
    /// A type layout could not be represented.
    #[error("type layout is unavailable: {ty_repr}")]
    Layout {
        /// Rendered rustc type.
        ty_repr: String,
    },
    /// A type kind is outside the current bridge task's translation surface.
    #[error("unsupported type: {ty_repr}")]
    UnsupportedTy {
        /// Rendered rustc type.
        ty_repr: String,
    },
    /// A function signature uses an ABI outside the v1 bridge surface.
    #[error("unsupported ABI: {abi_repr}")]
    UnsupportedAbi {
        /// Rendered rustc ABI.
        abi_repr: String,
    },
    /// A function signature has an uninhabited parameter.
    #[error("unsupported never input at argument {input_idx}")]
    UnsupportedNeverInput {
        /// Positional argument index.
        input_idx: usize,
    },
    /// A type has no v1 C ABI lowering at an extern C boundary.
    #[error("unsupported C ABI type at {position:?}: {ty_repr}")]
    UnsupportedCAbiType {
        /// Rendered model type.
        ty_repr: String,
        /// Where the offending type was encountered (spec section
        /// 14.5 surfaces Input / Output / Field admissibility
        /// separately, so the diagnostic must carry the position).
        position: CAbiTypePosition,
    },
    /// A const shape is outside the current bridge task's translation surface.
    #[error("unsupported const: {const_repr}")]
    UnsupportedConst {
        /// Rendered rustc const.
        const_repr: String,
    },
    /// Generic opaque handles are rejected before model construction.
    #[error("generic opaque handle is unsupported: {0}")]
    GenericOpaqueHandle(String),
    /// Phase 1 discovery failed.
    #[error("phase 1 discovery failed: {0}")]
    Phase1(String),
    /// Phase 3 F4 sorting failed.
    #[error("phase 3 F4 sort failed: {0}")]
    Phase3(String),
    /// Phase 4 ItemId assignment failed.
    #[error("phase 4 assignment failed: {0}")]
    Phase4(String),
    /// Phase 5 body materialisation or validation failed.
    #[error("phase 5 body pass failed: {0}")]
    Phase5(String),
}

/// DSL-policy type rejection family.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DslRejectionKind {
    /// `Box<T>` is outside the no-alloc driver DSL.
    Box,
    /// `core::ffi::c_void` must be replaced with an opaque struct.
    CVoid,
    /// `core::sync::atomic::Atomic*` must use the Regseq atomic DSL.
    Atomic,
    /// A MIR place projection is outside the Regseq body vocabulary.
    PlaceProjection,
    /// A MIR rvalue is outside the Regseq body vocabulary.
    Rvalue,
    /// A MIR statement is outside the Regseq body vocabulary.
    Statement,
    /// A MIR terminator is outside the Regseq body vocabulary.
    Terminator,
    /// A MIR basic block is outside the Regseq body vocabulary.
    BasicBlock,
}

/// Non-rigid `TyKind` family rejected after normalization.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NonRigidTyKind {
    /// `TyKind::Param`.
    Param,
    /// `TyKind::Bound`.
    Bound,
    /// `TyKind::Placeholder`.
    Placeholder,
    /// `TyKind::Infer`.
    Infer,
    /// `TyKind::Alias`.
    Alias,
    /// `TyKind::Error`.
    Error,
}

/// Orchestrates the five-phase bridge pipeline.
pub fn run_bridge<'tcx>(input: BridgeInput<'tcx>) -> Result<RegseqModel, BridgeError> {
    let mut ctxt = BridgeCtxt::new(input);

    let discovered_mono_items = discovery::collect_crate_mono_items(ctxt.input.tcx)?;
    *ctxt
        .input
        .discovered_mono_items
        .lock()
        .map_err(|_| BridgeError::Phase1("discovery set lock poisoned".to_string()))? =
        discovered_mono_items;
    translate_with_interleaved_synthesis(&mut ctxt)?;
    sort::f4_sort(&ctxt.model, &mut ctxt.pending_items)?;
    assign::assign_item_ids_and_populate_sidetables(&mut ctxt)?;
    construct_instances_and_resolve_cross_references(&mut ctxt)?;

    Ok(ctxt.model)
}

fn translate_with_interleaved_synthesis(ctxt: &mut BridgeCtxt<'_>) -> Result<(), BridgeError> {
    let discovered_mono_items = ctxt
        .input
        .discovered_mono_items
        .lock()
        .map_err(|_| BridgeError::Phase1("discovery set lock poisoned".to_string()))?
        .clone();
    for mono_item in discovered_mono_items {
        let MonoItem::Fn(instance) = mono_item else {
            continue;
        };
        let Some(local_def_id) = instance.def_id().as_local() else {
            continue;
        };
        let body = ctxt.input.tcx.optimized_mir(local_def_id);
        for local in &body.local_decls {
            scan_ty_for_synthesis(ctxt, local.ty)?;
        }
        for block in body.basic_blocks.iter() {
            for statement in &block.statements {
                scan_statement_for_synthesis(ctxt, &body.local_decls, &statement.kind)?;
            }
            if let Some(terminator) = &block.terminator {
                scan_terminator_for_synthesis(ctxt, &body.local_decls, &terminator.kind)?;
            }
        }
    }
    Ok(())
}

fn scan_ty_for_synthesis<'tcx>(
    ctxt: &mut BridgeCtxt<'tcx>,
    ty: rustc_middle::ty::Ty<'tcx>,
) -> Result<(), BridgeError> {
    match ty.kind() {
        TyKind::Closure(def_id, args) => {
            synth::closure::synthesise_closure_for_def_id(ctxt, *def_id, args.as_closure())?;
        }
        TyKind::Dynamic(predicates, _) => {
            if let Some(principal_ref) = predicates
                .principal()
                .map(|principal| principal.skip_binder())
            {
                let projection_bindings = projection_bindings_key(ctxt, predicates);
                let principal = principal_trait_key(ctxt, principal_ref, projection_bindings);
                synth::vtable::synthesise_vtable_type(ctxt, principal);
            }
        }
        TyKind::Ref(_, pointee, _) | TyKind::RawPtr(pointee, _) => {
            scan_ty_for_synthesis(ctxt, *pointee)?;
        }
        TyKind::Array(elem, _) | TyKind::Slice(elem) => {
            scan_ty_for_synthesis(ctxt, *elem)?;
        }
        TyKind::Tuple(fields) => {
            for field in fields.iter() {
                scan_ty_for_synthesis(ctxt, field)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn scan_statement_for_synthesis<'tcx>(
    ctxt: &mut BridgeCtxt<'tcx>,
    local_decls: &rustc_middle::mir::LocalDecls<'tcx>,
    statement: &StatementKind<'tcx>,
) -> Result<(), BridgeError> {
    let StatementKind::Assign(assignment) = statement else {
        return Ok(());
    };
    let (_, rvalue) = &**assignment;
    scan_rvalue_moves_for_drop_glue(ctxt, local_decls, rvalue)?;
    let Rvalue::Cast(
        CastKind::PointerCoercion(rustc_middle::ty::adjustment::PointerCoercion::Unsize, _),
        operand,
        target_ty,
    ) = rvalue
    else {
        return Ok(());
    };

    let Some(principal_ref) = dynamic_principal_ref(*target_ty) else {
        return Ok(());
    };
    let source_ty = concrete_impl_ty(operand.ty(local_decls, ctxt.input.tcx));
    let Ok(impl_ty) = ty::translate_ty(ctxt, source_ty) else {
        return Ok(());
    };
    let projection_bindings = dynamic_projection_bindings_key(ctxt, *target_ty);
    let principal = principal_trait_key(ctxt, principal_ref, projection_bindings);
    synth::vtable::synthesise_vtable_with_rustc_entries(
        ctxt,
        principal,
        principal_ref,
        impl_ty,
        source_ty,
    )?;
    Ok(())
}

fn scan_terminator_for_synthesis<'tcx>(
    ctxt: &mut BridgeCtxt<'tcx>,
    local_decls: &rustc_middle::mir::LocalDecls<'tcx>,
    terminator: &TerminatorKind<'tcx>,
) -> Result<(), BridgeError> {
    if let TerminatorKind::Call { func, args, .. } = terminator {
        scan_call_ty_for_vtable(ctxt, func.ty(local_decls, ctxt.input.tcx))?;
        scan_operand_move_for_drop_glue(ctxt, local_decls, func)?;
        if let rustc_middle::mir::Operand::Constant(constant) = func {
            if let TyKind::FnDef(def_id, _) = constant.const_.ty().kind() {
                if diag_log::intrinsic_kind_for_def_id(ctxt.input.tcx, *def_id).is_some() {
                    let model_ty = ty::translate_ty(ctxt, constant.const_.ty())?;
                    if let Ty::FnDef(callee_item, _) = ctxt.model.types.resolve(model_ty).clone() {
                        // Per audit-finding F4 source-derived Items
                        // carry placeholder ItemIds during Phase 2;
                        // look the callee's DefPath up through
                        // `pending_items` when the ItemId hasn't yet
                        // been bound to `model.items`.
                        let callee_def_path = ctxt
                            .lookup_item_def_path(callee_item)
                            .expect("FnDef callee Item must be interned")
                            .clone();
                        diag_log::recognise_log_intrinsic_def_id(
                            ctxt,
                            ItemHandle::from_u32(callee_item.as_u32()),
                            &callee_def_path,
                            *def_id,
                        )?;
                    }
                }
            }
        }
        for arg in args.iter() {
            scan_call_ty_for_vtable(ctxt, arg.node.ty(local_decls, ctxt.input.tcx))?;
            scan_operand_move_for_drop_glue(ctxt, local_decls, &arg.node)?;
        }
    }

    if let TerminatorKind::Drop { place, .. } = terminator {
        let drop_ty = place.ty(local_decls, ctxt.input.tcx).ty;
        if !drop_ty.needs_drop(
            ctxt.input.tcx,
            rustc_middle::ty::TypingEnv::fully_monomorphized(),
        ) {
            return Ok(());
        }
        if let Ok(model_ty) = ty::translate_ty(ctxt, drop_ty) {
            synth::synthesise_drop_glue(ctxt, model_ty)?;
        }
    }
    Ok(())
}

fn scan_call_ty_for_vtable<'tcx>(
    ctxt: &mut BridgeCtxt<'tcx>,
    ty: rustc_middle::ty::Ty<'tcx>,
) -> Result<(), BridgeError> {
    // Per spec.md sections 13.2 and 3.4, `Ty::Dynamic` translation
    // triggers VTableType synthesis only; the (instance + method
    // shims + drop shim) trio is keyed on the concrete impl_ty and
    // is synthesised at `Cast::Unsize` sites in
    // `scan_statement_for_synthesis`. Call sites with `dyn Trait`
    // arguments do not carry a concrete impl_ty so they only need
    // to register the principal's vtable type.
    scan_ty_for_synthesis(ctxt, ty)
}

fn scan_rvalue_moves_for_drop_glue<'tcx>(
    ctxt: &mut BridgeCtxt<'tcx>,
    local_decls: &rustc_middle::mir::LocalDecls<'tcx>,
    rvalue: &Rvalue<'tcx>,
) -> Result<(), BridgeError> {
    match rvalue {
        Rvalue::Use(operand)
        | Rvalue::Repeat(operand, _)
        | Rvalue::Cast(_, operand, _)
        | Rvalue::UnaryOp(_, operand) => {
            scan_operand_move_for_drop_glue(ctxt, local_decls, operand)
        }
        Rvalue::BinaryOp(_, operands) => {
            scan_operand_move_for_drop_glue(ctxt, local_decls, &operands.0)?;
            scan_operand_move_for_drop_glue(ctxt, local_decls, &operands.1)
        }
        Rvalue::Aggregate(_, operands) => {
            for operand in operands.iter() {
                scan_operand_move_for_drop_glue(ctxt, local_decls, operand)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn scan_operand_move_for_drop_glue<'tcx>(
    ctxt: &mut BridgeCtxt<'tcx>,
    local_decls: &rustc_middle::mir::LocalDecls<'tcx>,
    operand: &rustc_middle::mir::Operand<'tcx>,
) -> Result<(), BridgeError> {
    let rustc_middle::mir::Operand::Move(place) = operand else {
        return Ok(());
    };
    let move_ty = place.ty(local_decls, ctxt.input.tcx).ty;
    let typing_env = rustc_middle::ty::TypingEnv::fully_monomorphized();
    if ctxt
        .input
        .tcx
        .type_is_copy_modulo_regions(typing_env, move_ty)
    {
        return Ok(());
    }
    if let Ok(model_ty) = ty::translate_ty(ctxt, move_ty) {
        synth::synthesise_drop_glue(ctxt, model_ty)?;
    }
    Ok(())
}

fn dynamic_principal_ref<'tcx>(
    ty: rustc_middle::ty::Ty<'tcx>,
) -> Option<rustc_middle::ty::ExistentialTraitRef<'tcx>> {
    match ty.kind() {
        TyKind::Dynamic(predicates, _) => Some(predicates.principal()?.skip_binder()),
        TyKind::Ref(_, pointee, _) | TyKind::RawPtr(pointee, _) => dynamic_principal_ref(*pointee),
        _ => None,
    }
}

fn concrete_impl_ty<'tcx>(ty: rustc_middle::ty::Ty<'tcx>) -> rustc_middle::ty::Ty<'tcx> {
    match ty.kind() {
        TyKind::Ref(_, pointee, _) | TyKind::RawPtr(pointee, _) => *pointee,
        _ => ty,
    }
}

pub(crate) fn principal_trait_key<'tcx>(
    ctxt: &mut BridgeCtxt<'tcx>,
    principal: rustc_middle::ty::ExistentialTraitRef<'tcx>,
    projection_bindings: ProjectionBindingsKey,
) -> PrincipalTraitKey {
    let trait_args = principal
        .args
        .iter()
        .filter_map(|arg| match arg.kind() {
            rustc_middle::ty::GenericArgKind::Type(ty) => {
                ty::translate_ty(ctxt, ty).ok().map(GenericArg::Type)
            }
            rustc_middle::ty::GenericArgKind::Const(_)
            | rustc_middle::ty::GenericArgKind::Lifetime(_) => None,
        })
        .collect::<Vec<_>>();
    // Per audit-finding F4 + spec.md section 3.4 D-12-5.3, source
    // -derived ADT Items interned during Phase 2 carry placeholder
    // ItemIds until Phase 4. The pending overlay resolves them
    // through their rustc DefPathHashes so trait_args canonical bytes
    // are stable across the placeholder→final ItemId remap.
    let pending = ctxt.pending_def_path_hashes();
    PrincipalTraitKey {
        trait_path: def_path_hash(ctxt.input.tcx, principal.def_id),
        trait_args: args_key_for_generic_args_with_pending(&ctxt.model, &trait_args, &pending),
        projection_bindings,
    }
}

fn dynamic_projection_bindings_key<'tcx>(
    ctxt: &mut BridgeCtxt<'tcx>,
    ty: rustc_middle::ty::Ty<'tcx>,
) -> ProjectionBindingsKey {
    match ty.kind() {
        TyKind::Dynamic(predicates, _) => projection_bindings_key(ctxt, predicates),
        TyKind::Ref(_, pointee, _) | TyKind::RawPtr(pointee, _) => {
            dynamic_projection_bindings_key(ctxt, *pointee)
        }
        _ => ProjectionBindingsKey(0),
    }
}

pub(crate) fn projection_bindings_key<'tcx>(
    ctxt: &mut BridgeCtxt<'tcx>,
    predicates: &'tcx rustc_middle::ty::List<
        rustc_middle::ty::Binder<'tcx, rustc_middle::ty::ExistentialPredicate<'tcx>>,
    >,
) -> ProjectionBindingsKey {
    use sha2::{Digest, Sha256};

    let mut entries = Vec::new();
    for predicate in predicates.iter() {
        let rustc_middle::ty::ExistentialPredicate::Projection(projection) =
            predicate.skip_binder()
        else {
            continue;
        };
        let hash = def_path_hash(ctxt.input.tcx, projection.def_id);
        let mut entry = Vec::new();
        encode_def_path_hash(&mut entry, hash);
        match projection.term.kind() {
            rustc_middle::ty::TermKind::Ty(ty) => {
                entry.push(0);
                if let Ok(ty) = ty::translate_ty(ctxt, ty) {
                    let pending = ctxt.pending_def_path_hashes();
                    entry.extend_from_slice(&canonical_ty_bytes_with_pending(
                        &ctxt.model,
                        ty,
                        &pending,
                    ));
                }
            }
            rustc_middle::ty::TermKind::Const(konst) => {
                entry.push(1);
                entry.extend_from_slice(format!("{:?}", konst.kind()).as_bytes());
            }
        }
        entries.push((hash, entry));
    }
    entries.sort_by_key(|(hash, _)| (hash.stable_crate_id, hash.local_hash));

    let mut bytes = Vec::new();
    bytes.extend_from_slice(&(entries.len() as u32).to_le_bytes());
    for (_, entry) in entries {
        bytes.extend_from_slice(&entry);
    }
    let digest = Sha256::digest(&bytes);
    let mut head = [0u8; 16];
    head.copy_from_slice(&digest[..16]);
    ProjectionBindingsKey(u128::from_be_bytes(head))
}

fn encode_def_path_hash(bytes: &mut Vec<u8>, hash: DefPathHash) {
    bytes.extend_from_slice(&hash.stable_crate_id.0.to_le_bytes());
    bytes.extend_from_slice(&hash.local_hash.to_le_bytes());
}

fn def_path_hash(tcx: TyCtxt<'_>, def_id: DefId) -> DefPathHash {
    let hash = tcx.def_path_hash(def_id);
    DefPathHash {
        stable_crate_id: StableCrateId(hash.stable_crate_id().as_u64()),
        local_hash: hash.local_hash().as_u64(),
    }
}

fn construct_instances_and_resolve_cross_references(
    ctxt: &mut BridgeCtxt<'_>,
) -> Result<(), BridgeError> {
    synth::rekey_synthesis_in_phase5(ctxt)?;
    body::run_phase5_body_passes(ctxt)?;
    // Body materialisation calls `translate_const`, which can synthesise
    // late `ConstMemoryStatic` pending items for reference-typed const
    // operands. Drain those through the same Phase-4 ItemId assignment
    // path before constructing the corresponding Static instances per
    // spec section 3.4.
    if !ctxt.pending_items.is_empty() {
        sort::f4_sort(&ctxt.model, &mut ctxt.pending_items)?;
        assign::assign_late_pending_items(ctxt);
    }
    const_::construct_const_memory_static_instances(ctxt)?;
    Ok(())
}
