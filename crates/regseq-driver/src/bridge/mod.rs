//! Rustc-to-`RegseqModel` bridge pipeline scaffold.

use std::sync::{Arc, Mutex};

use rustc_middle::mir::{mono::MonoItem, CastKind, Rvalue, StatementKind, TerminatorKind};
use rustc_middle::ty::{TyCtxt, TyKind};
use rustc_span::def_id::{DefId, LOCAL_CRATE};
use std::collections::HashMap;
use thiserror::Error;

use regseq_model::{
    args_key_for_generic_args, canonical_ty_bytes, DefPathHash, FnAbi, FnSig, FunctionInstanceData,
    FunctionInstanceFlavor, GenericArg, Instance, InstanceId, InstanceKind, Item, ItemId, ItemKind,
    Mutability, PrincipalTraitKey, ProjectionBindingsKey, RegseqModel, Safety, Span, StableCrateId,
    SyntheticItemKind, Ty, TyId, UNIT_LAYOUT,
};

pub mod assign;
pub mod body;
pub mod const_;
mod discovery;
pub mod fnsig;
mod handles;
pub mod sort;
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
    /// Forward cache from model `TyId` back to the rustc `Ty<'tcx>` it
    /// was translated from. Populated on the success path of
    /// `translate_erased_ty`; used by Phase-5 body materialisation to
    /// reconstruct rustc `GenericArgs` from model `Instance.generic_args`.
    pub ty_forward_cache: HashMap<TyId, rustc_middle::ty::Ty<'tcx>>,
    /// Cache mapping a Function-kind `ItemId` to the `InstanceId` of
    /// the model `Instance` of `InstanceKind::Function` that
    /// monomorphises it. Populated by the Phase-4 Function-Instance
    /// creation pass and by lazy creation in `translate_call_callee`.
    /// Used by terminator translation to resolve `CallCallee::Direct`
    /// through `model.instances` instead of fabricating an
    /// `InstanceId` from a raw hash (spec.md section 23.7 Direct
    /// callee identity rule).
    pub function_instance_for_item: HashMap<ItemId, InstanceId>,
    /// Phase-2 side cache from closure `DefPathHash` back to its rustc
    /// `Ty<'tcx>`. Populated by closure synthesis; consumed by the
    /// Phase-5 captures-struct view synthesis pass that interns
    /// `Ty::Struct { item, args, layout }` per spec.md section 13.1.
    pub closure_synth_rustc_ty: HashMap<DefPathHash, rustc_middle::ty::Ty<'tcx>>,
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
            ty_forward_cache: HashMap::new(),
            function_instance_for_item: HashMap::new(),
            closure_synth_rustc_ty: HashMap::new(),
        }
    }
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
    let (method_shims, init_meta) =
        vtable_entries_and_init_meta(ctxt, source_ty, principal_ref);
    if let Some(init_meta) = init_meta {
        synth::vtable::synthesise_vtable_with_init_meta(
            ctxt,
            principal,
            impl_ty,
            method_shims,
            init_meta,
        )?;
    } else {
        synth::vtable::synthesise_vtable_with_method_shims(
            ctxt,
            principal,
            impl_ty,
            method_shims,
        )?;
    }
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
    scan_ty_for_synthesis(ctxt, ty)?;
    let Some(principal_ref) = dynamic_principal_ref(ty) else {
        return Ok(());
    };
    let projection_bindings = dynamic_projection_bindings_key(ctxt, ty);
    let principal = principal_trait_key(ctxt, principal_ref, projection_bindings);
    let impl_ty = ctxt.model.types.intern(regseq_model::Ty::Tuple {
        fields: Vec::new(),
        layout: regseq_model::UNIT_LAYOUT,
    });
    synth::synthesise_vtable(ctxt, principal, impl_ty)?;
    Ok(())
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

fn principal_trait_key<'tcx>(
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
    PrincipalTraitKey {
        trait_path: def_path_hash(ctxt.input.tcx, principal.def_id),
        trait_args: args_key_for_generic_args(&ctxt.model, &trait_args),
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

fn projection_bindings_key<'tcx>(
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
                    entry.extend_from_slice(&canonical_ty_bytes(&ctxt.model, ty));
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

/// Compute both the method-shim entry list and (when impl-type layout
/// is available) the `VTableInitMeta` capturing the full slot sequence
/// for Phase 5 init synthesis.
///
/// HYPOTHESIS: this uses `principal.with_self_ty(tcx, source_ty)` plus
/// `erase_and_anonymize_regions` to mirror rustc's
/// `vtable_allocation_provider` (see rustc-src
/// `rustc_middle/src/ty/vtable.rs:88-93`). If the trait_ref cannot be
/// resolved for any reason, both the method-shim list and the init
/// meta come back empty/None.
fn vtable_entries_and_init_meta<'tcx>(
    ctxt: &mut BridgeCtxt<'tcx>,
    source_ty: rustc_middle::ty::Ty<'tcx>,
    principal: rustc_middle::ty::ExistentialTraitRef<'tcx>,
) -> (
    Vec<(u32, Option<DefPathHash>)>,
    Option<synth::VTableInitMeta>,
) {
    let trait_ref = principal.with_self_ty(ctxt.input.tcx, source_ty);
    let trait_ref = ctxt.input.tcx.erase_and_anonymize_regions(trait_ref);
    let entries = ctxt.input.tcx.vtable_entries(trait_ref);

    let mut method_shims = Vec::new();
    let mut slots = Vec::with_capacity(entries.len());
    for (idx, entry) in entries.iter().enumerate() {
        let slot = match entry {
            rustc_middle::ty::VtblEntry::MetadataDropInPlace => synth::VTableSlotKind::DropInPlace,
            rustc_middle::ty::VtblEntry::MetadataSize => synth::VTableSlotKind::Size,
            rustc_middle::ty::VtblEntry::MetadataAlign => synth::VTableSlotKind::Align,
            rustc_middle::ty::VtblEntry::Vacant => synth::VTableSlotKind::Vacant,
            rustc_middle::ty::VtblEntry::TraitVPtr(_) => synth::VTableSlotKind::TraitVPtr,
            rustc_middle::ty::VtblEntry::Method(instance) => {
                method_shims.push((
                    idx as u32,
                    Some(def_path_hash(ctxt.input.tcx, instance.def_id())),
                ));
                synth::VTableSlotKind::Method(None)
            }
        };
        slots.push(slot);
    }

    let layout = ctxt
        .input
        .tcx
        .layout_of(rustc_middle::ty::TypingEnv::fully_monomorphized().as_query_input(source_ty))
        .ok();
    let init_meta = layout.map(|layout| synth::VTableInitMeta {
        slots,
        size: layout.size.bytes(),
        align: layout.align.abi.bytes(),
        pointer_size: ctxt.input.tcx.data_layout.pointer_size().bytes(),
    });

    (method_shims, init_meta)
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
    synth::closure::synthesise_closure_captures_struct_views(ctxt)?;
    const_::construct_const_memory_static_instances(ctxt)?;
    construct_function_instances(ctxt)?;
    body::run_phase5_body_passes(ctxt)?;
    Ok(())
}

/// Phase-4 Function-Instance creation pass. After `ItemId` assignment
/// and synthesis re-keying, ensure every `ItemKind::Function` model
/// `Item` has a matching `Instance` of `InstanceKind::Function` in
/// `model.instances`. The mapping is recorded in
/// `BridgeCtxt::function_instance_for_item` so terminator translation
/// can resolve `CallCallee::Direct` callees through `model.instances`
/// rather than fabricating an `InstanceId` from a `DefPathHash`
/// truncation (spec.md section 23.7 Direct callee identity rule).
///
/// Items that already have an `Instance` (for example, test fixtures
/// that push a Function instance manually) are recorded in the map and
/// not duplicated. Drop-glue Items carry a `DropGlue` flavor; every
/// other Function-kind Item is assigned `Item` flavor by default.
fn construct_function_instances(ctxt: &mut BridgeCtxt<'_>) -> Result<(), BridgeError> {
    for (instance_idx, instance) in ctxt.model.instances.iter().enumerate() {
        if matches!(instance.kind, InstanceKind::Function(_)) {
            ctxt.function_instance_for_item
                .entry(instance.item)
                .or_insert_with(|| InstanceId::from_u32(instance_idx as u32));
        }
    }

    for item_idx in 0..ctxt.model.items.len() {
        let item_id = ItemId::from_u32(item_idx as u32);
        if !matches!(ctxt.model.items[item_id].kind, ItemKind::Function) {
            continue;
        }
        if ctxt.function_instance_for_item.contains_key(&item_id) {
            continue;
        }
        let synthetic_kind = ctxt.model.item_synthetic_kinds[item_id].clone();
        let flavor = match &synthetic_kind {
            SyntheticItemKind::DropGlue { .. } => FunctionInstanceFlavor::DropGlue,
            _ => FunctionInstanceFlavor::Item,
        };
        let (fn_sig, generic_args) = synthesise_function_instance_sig(ctxt, item_id, &synthetic_kind);
        let emission_name = ctxt.model.items[item_id].def_path.rendered;
        let instance_id = ctxt.model.instances.push(Instance {
            item: item_id,
            generic_args,
            emission_name,
            span: Span::dummy(),
            kind: InstanceKind::Function(FunctionInstanceData {
                fn_sig,
                body: None,
                flavor,
                virtual_idx: None,
            }),
        });
        ctxt.function_instance_for_item.insert(item_id, instance_id);
    }

    Ok(())
}

/// Build a `FnSigId` and `generic_args` for a Function-kind Item that
/// has no pre-existing Instance. Drop-glue Items get `fn(*mut T) -> ()`
/// per the spec's drop-glue calling convention; source-derived Items
/// translate the rustc `fn_sig` (with regions erased) so the synthesised
/// Instance carries spec-correct arity. Falls back to `fn() -> ()` for
/// synthetic Items without recoverable rustc signatures.
fn synthesise_function_instance_sig(
    ctxt: &mut BridgeCtxt<'_>,
    item: ItemId,
    synthetic_kind: &SyntheticItemKind,
) -> (regseq_model::FnSigId, Vec<GenericArg>) {
    let unit_ty = ctxt.model.types.intern(Ty::Tuple {
        fields: Vec::new(),
        layout: UNIT_LAYOUT,
    });
    let (inputs, output, generic_args) = match synthetic_kind {
        SyntheticItemKind::DropGlue { self_ty: _ } => {
            let drop_target_ty = ctxt
                .drop_glue_synth_cache
                .iter()
                .find_map(|(ty, handle)| {
                    ctxt.handles
                        .resolve(*handle)
                        .filter(|resolved| *resolved == item)
                        .map(|_| *ty)
                })
                .unwrap_or(unit_ty);
            let ptr_ty = ctxt
                .model
                .types
                .intern(Ty::Ptr(drop_target_ty, Mutability::Mut));
            (
                vec![ptr_ty],
                unit_ty,
                vec![GenericArg::Type(drop_target_ty)],
            )
        }
        SyntheticItemKind::None => match translate_source_fn_signature(ctxt, item) {
            Some((inputs, output)) => (inputs, output, Vec::new()),
            None => (Vec::new(), unit_ty, Vec::new()),
        },
        _ => (Vec::new(), unit_ty, Vec::new()),
    };
    let sig = FnSig {
        inputs,
        output,
        abi: FnAbi::Rust,
        c_variadic: false,
        safety: Safety::Safe,
    };
    let sig_id = ctxt.model.sigs.intern(sig);
    while ctxt.model.sig_c_abi_lowerings.len() < ctxt.model.sigs.len() {
        ctxt.model.sig_c_abi_lowerings.push(None);
    }
    (sig_id, generic_args)
}

/// Translate the rustc `fn_sig` of a source-derived Function Item into
/// model `TyId`s. Erases regions on each input/output before
/// translation so substantive-HRTB binders (`for<'a> fn(&'a T)`) do
/// not block callee-instance synthesis. Returns `None` when the Item's
/// `DefPathHash` does not resolve to a rustc `DefId` or any type fails
/// to translate.
fn translate_source_fn_signature(
    ctxt: &mut BridgeCtxt<'_>,
    item: ItemId,
) -> Option<(Vec<TyId>, TyId)> {
    let wanted = ctxt.model.item_def_paths[item];
    let tcx = ctxt.input.tcx;
    let rustc_hash = rustc_hir::definitions::DefPathHash(
        rustc_data_structures::fingerprint::Fingerprint::new(
            wanted.stable_crate_id.0,
            wanted.local_hash,
        ),
    );
    let def_id = tcx.def_path_hash_to_def_id(rustc_hash)?;
    let poly_sig = tcx.fn_sig(def_id).instantiate_identity();
    let sig = poly_sig.skip_binder();
    let mut inputs = Vec::with_capacity(sig.inputs().len());
    for input in sig.inputs() {
        let erased = tcx.erase_and_anonymize_regions(*input);
        inputs.push(ty::translate_ty(ctxt, erased).ok()?);
    }
    let erased_output = tcx.erase_and_anonymize_regions(sig.output());
    let output = ty::translate_ty(ctxt, erased_output).ok()?;
    Some((inputs, output))
}
