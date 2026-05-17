//! Rustc-to-`RegseqModel` bridge pipeline scaffold.

use std::sync::{Arc, Mutex};

use rustc_middle::mir::{mono::MonoItem, CastKind, Rvalue, StatementKind, TerminatorKind};
use rustc_middle::ty::{TyCtxt, TyKind};
use rustc_span::def_id::{DefId, LOCAL_CRATE};
use std::collections::HashMap;
use thiserror::Error;

use regseq_model::{
    args_key_for_generic_args_with_pending, canonical_ty_bytes_with_pending, ArgsKey, DefPathHash,
    FunctionInstanceFlavor, GenericArg, InstanceId, Item, ItemId, ItemKind, PrincipalTraitKey,
    ProjectionBindingsKey, RegseqModel, StableCrateId, SyntheticItemKind, Ty, TyId,
};

mod assign;
pub mod body;
pub mod const_;
pub mod def_path;
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

/// Bridge-side accumulator for DiagLogPeel residue per spec.md
/// section 26.6 (`spec.md:12705-12707`). Populated during
/// `peel_body_against_model` per collapsed wrapper Call; drained
/// by `eliminate_diag_log_peel_residue` after every Body has been
/// peeled in the Phase-5 sub-pass order. The accumulator carries
/// the seed set for the post-DiagLogPeel residue-driven DCE: the
/// wrapper Call's `CallCallee::Direct(instance_id)` captured BEFORE
/// `collapse_wrapper_calls` rewrites the terminator, so the seeds
/// survive the collapse.
#[derive(Clone, Debug, Default)]
pub struct DiagLogPeelResidue {
    /// `InstanceId`s of the `<LogArg as From<T>>::from(...)` wrapper
    /// Calls that DiagLogPeel collapsed to `Goto`. Each is a seed
    /// for the transitive dead-set computation in
    /// `eliminate_diag_log_peel_residue`.
    pub collapsed_wrapper_instances: Vec<InstanceId>,
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
    /// Direct-callee resolution table per audit finding
    /// regseq-driver-42-4. Keyed by `(ItemId, ArgsKey,
    /// FunctionInstanceFlavor)` so generic monomorphisations and shim
    /// flavors do NOT collapse to a single Instance. Populated by
    /// `construct_function_instances` in Phase 4 and extended lazily
    /// at terminator-translation time when a `(ItemId, args, flavor)`
    /// triple is observed at a call site that the Phase-4 walk had no
    /// way to reach in advance.
    pub function_instance_for_item: HashMap<(ItemId, ArgsKey, FunctionInstanceFlavor), InstanceId>,
    /// Reverse lookup from model `TyId` to its originating
    /// `rustc_middle::ty::Ty<'tcx>` (after region erasure). Populated
    /// on every successful return from `ty::translate_erased_ty` per
    /// audit-finding regseq-driver-43-2: Phase-5 body materialisation
    /// reconstitutes per-Instance rustc generic args from the model's
    /// `Instance.generic_args` by mapping each `GenericArg::Type(TyId)`
    /// back through this cache.
    pub ty_forward_cache: HashMap<TyId, rustc_middle::ty::Ty<'tcx>>,
    /// Bridge-side accumulator for DiagLogPeel residue per
    /// regseq-driver-48 Change A. Populated by
    /// `peel_body_against_model` per collapsed wrapper Call;
    /// drained by `diag_log::dead_item_elim::eliminate_diag_log_peel_residue`
    /// at the end of Phase 5. See `DiagLogPeelResidue`.
    pub diag_log_peel_residue: DiagLogPeelResidue,
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
            function_instance_for_item: HashMap::new(),
            ty_forward_cache: HashMap::new(),
            diag_log_peel_residue: DiagLogPeelResidue::default(),
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
    pub fn lookup_item_synthetic_kind(
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

/// Drain any late pending items appended during a Phase-5 terminator
/// translation (e.g., the eager drop-glue synthesis fallback for a
/// drop site whose `place_ty` was not reached during the Phase-2
/// walk). Used by `bridge::body::terminator::translate_drop_as_call`
/// to bind freshly-minted synthetic-Item handles before they are
/// resolved at the call emission site. Phase-5 placement is fine
/// because the F4 sort over the late additions is bridge-internal
/// (post-Phase-4 ItemIds are dense and append-only); the production
/// path also relies on this via the post-body-passes flush in
/// `construct_instances_and_resolve_cross_references`.
///
/// Per audit-finding regseq-driver-43-2 (body materialisation
/// triggers translate_ty on local types, which can intern fresh
/// source-derived items mid-translation): after assigning ItemIds we
/// also drain `source_ty_pending` so any nominal `Ty` views that
/// embed the freshly-bound placeholder are patched to the real
/// `ItemId`. Without this, subsequent `args_key_for_generic_args_*`
/// calls during the same Phase-5 sub-step panic on an unresolved
/// placeholder.
pub(crate) fn assign_late_pending_items_for_terminator(ctxt: &mut BridgeCtxt<'_>) {
    if ctxt.pending_items.is_empty() && ctxt.source_ty_pending.is_empty() {
        return;
    }
    if !ctxt.pending_items.is_empty() {
        assign::assign_late_pending_items(ctxt);
    }
    patch_resolved_source_ty_pending(ctxt);
}

/// Patch every `source_ty_pending` entry whose handle now resolves to
/// a final `ItemId`. Entries whose handle is still unbound are left
/// in place so a later drain can pick them up. Per audit-finding
/// regseq-driver-43-2.
pub(crate) fn patch_resolved_source_ty_pending(ctxt: &mut BridgeCtxt<'_>) {
    let snapshot: Vec<(ItemHandle, Vec<TyId>)> = ctxt.source_ty_pending.drain().collect();
    for (handle, ty_ids) in snapshot {
        if let Some(item_id) = ctxt.handles.resolve(handle) {
            for ty_id in ty_ids {
                ctxt.model.types.patch_nominal_item_id(ty_id, item_id);
            }
        } else {
            // Handle still unbound (its pending item has not been
            // drained yet); reinstate the entry so the next drain
            // can pick it up.
            ctxt.source_ty_pending.insert(handle, ty_ids);
        }
    }
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
    /// A diagnostic-logging format-template parse rejected by the
    /// closed-subset grammar at spec.md section 26.7 (e.g., `{:?}`
    /// Debug formatter rejection per D3 at spec.md line 12840).
    #[error("diagnostic-logging format error ({reason:?}) in template `{template}`")]
    DiagLogFormat {
        /// Rule-specific rejection family.
        reason: DiagLogFormatReason,
        /// Original (or offending) template text for diagnostics.
        template: String,
    },
    /// A diagnostic-logging bridge-side validation diagnostic per
    /// spec.md section 26.8 (D1, D2, D4, D6, D7, D8). D3 is
    /// surface-formatted as `DiagLogFormat` since the closed-subset
    /// grammar parser enforces it at parse time.
    #[error("diagnostic-logging validation failed ({diagnostic:?}): {detail}")]
    DiagLogValidation {
        /// The spec.md section 26.8 rule that failed.
        diagnostic: DiagLogValidationDiagnostic,
        /// Author-facing detail for the failing rule.
        detail: String,
    },
}

/// Format-template parser rejection family per spec.md section 26.7
/// and D3 at spec.md line 12840.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiagLogFormatReason {
    /// `{:?}` Debug formatter: parsed only to reject per D3 at
    /// spec.md line 12840.
    DebugFormatterRejected,
    /// Positional argument like `{0}`: outside the closed subset
    /// (spec.md section 26.7).
    PositionalArgRejected,
    /// Named argument like `{name}`: outside the closed subset
    /// (spec.md section 26.7).
    NamedArgRejected,
    /// Modifier outside the closed Hex / formatted-Hex subset (e.g.,
    /// `{:b}`, `{:o}`, `{:.5}`, `{:p}`).
    NonSubsetModifier,
    /// Unclosed `{` brace.
    UnclosedBrace,
    /// Stray `}` brace not part of the `}}` escape.
    UnmatchedClosingBrace,
}

/// Bridge-side validation diagnostics per spec.md section 26.8.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiagLogValidationDiagnostic {
    /// D1: specifier count differs from peeled argument count
    /// (template / arg arity parity).
    D1,
    /// D2: per-position modifier incompatible with operand type
    /// (e.g., hex modifier on a non-integer operand).
    D2,
    /// D4: an operand's source type is outside the `LogArg`
    /// admissible set (spec.md section 26.4).
    D4,
    /// D6: `args[0]` is not a `&'static str` literal at the MIR
    /// shape (spec.md section 26.8 D6).
    D6,
    /// D7: format template ends in a newline; backends append the
    /// target-appropriate terminator (spec.md section 26.7 +
    /// section 26.8 D7).
    D7,
    /// D8: `&str` operand count in args[1..] exceeds the selected
    /// binding target's declared `max_str_args_per_call`.
    D8,
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
    // Audit-finding HIGH-40-2 (spec.md section 16.2 / 16.3): the
    // bridge pushes Function and Static instances during distinct
    // Phase-5 sub-passes (`body::run_phase5_body_passes` for
    // Function bodies, `synth::rekey_synthesis_in_phase5` for
    // vtable Static initialisers, `const_::construct_const_memory_static_instances`
    // for reference-typed const Statics). Push order does not by
    // itself satisfy F4-2 (Function rank 0 must precede Static rank
    // 1). A final F4-2 sort guarantees the ordering regardless of
    // push interleaving and remaps every `CallCallee::Direct`
    // InstanceId so terminator references survive the permutation.
    ctxt.model.sort_instances_by_f4_2();

    Ok(ctxt.model)
}

fn translate_with_interleaved_synthesis(ctxt: &mut BridgeCtxt<'_>) -> Result<(), BridgeError> {
    let discovered_mono_items = ctxt
        .input
        .discovered_mono_items
        .lock()
        .map_err(|_| BridgeError::Phase1("discovery set lock poisoned".to_string()))?
        .clone();

    // Per spec.md section 3.4 Phase 2 (`spec.md:1142`): "Bridge walks
    // the discovered mono-item set. For each MonoItem, translates
    // rustc state into RegseqModel content." This pre-pass interns
    // each source-derived `MonoItem::Fn(instance)` whose
    // `instance.def` is `InstanceKind::Item(def_id)` and whose
    // `DefKind` is `Fn` or `AssocFn`. Shim flavors (DropGlue,
    // VTableShim, ClosureOnceShim, ...) and `DefKind::Closure` are
    // skipped here -- those route through the synthesis triggers in
    // the body-scan loop below per spec.md section 3.4 Phase-2
    // trigger table (`spec.md:1148-1158`) and spec.md section 13.1
    // (closure synthesis at `spec.md:4511-4554`).
    //
    // No `as_local()` gate: spec.md section 3.4 Phase 1
    // (`spec.md:1133-1139`) discovers items across "primary crate +
    // all regseq-aware dependency crates per Q6", and the new
    // Item-interning pass does not require MIR access (only the
    // `DefId`). `ty::intern_item` dispatches to
    // `intern_source_item_handle` which dedups via
    // `source_item_handles`, making the call idempotent if the
    // body-scan loop later re-discovers the same FnDef via a callee
    // constant.
    for mono_item in &discovered_mono_items {
        let MonoItem::Fn(instance) = mono_item else {
            continue;
        };
        let rustc_middle::ty::InstanceKind::Item(def_id) = instance.def else {
            continue;
        };
        if !is_source_derived_function(ctxt.input.tcx, def_id) {
            continue;
        }
        let _ = ty::intern_item(ctxt, def_id, ItemKind::Function, ty::SpecialAdt::None);
    }

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
            if let TyKind::FnDef(_, _) = constant.const_.ty().kind() {
                // Audit finding regseq-driver-42-4: pre-translate
                // every constant FnDef callee so the Phase-5
                // `translate_call_callee` path resolves the callee
                // Item through a real (post-Phase-4) ItemId rather
                // than a Phase-2 placeholder. Reaching translate_ty
                // here interns the source FnDef Item in
                // `cx.pending_items` so Phase-3/4 binds it before
                // body materialisation runs.
                let _ = ty::translate_ty(ctxt, constant.const_.ty());
            }
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
    // Audit-finding regseq-driver-42-4 + 42-5: construct one Function
    // Instance per (ItemKind::Function ItemId, identity-args,
    // FunctionInstanceFlavor::Item) tuple before body materialisation
    // so terminator translation can resolve `CallCallee::Direct`
    // targets through `function_instance_for_item`. Lazy expansion at
    // call sites (different `(item, args, flavor)` triples) extends
    // the map during terminator translation.
    construct_function_instances(ctxt)?;
    // Per regseq-driver-51 (spec.md:4531-4546 + 778-781): bridge-
    // synthesised ClosureCall Function Items have no rustc DefId, so
    // `construct_function_instances` (`mod.rs:1114-1121`) skips them.
    // This parallel walk constructs one `Function` `Instance` per
    // synthesised ClosureCall Item BEFORE `run_phase5_body_passes` so
    // the Phase-5 materialiser at `body/materialise.rs:303-338` can
    // fill in `body` via the existing closure-body resolution at
    // `body/materialise.rs:87-100`.
    construct_closure_call_instances(ctxt)?;
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
    // Body materialisation can have left placeholder `ItemId`s in
    // the type interner (`source_ty_pending` entries whose handles
    // bound to real `ItemId`s in the drain above). Patch them
    // before `sort_items_by_f4` walks the interner, otherwise the
    // remap step would index past `model.items.len()` per
    // `placeholder_item_id_for_handle`'s high-end encoding.
    patch_resolved_source_ty_pending(ctxt);
    // Audit-finding regseq-driver-47 (spec.md sections 3.4 + 16.1):
    // `assign_late_pending_items` appends late synthesis items past
    // the original F4-sorted batch, which can interleave them out
    // of order with the existing items per spec section 16.1. A
    // global items-side F4 sort + ItemId remap restores invariant
    // 3 (`ItemsSortedAtConstruction`) before the next Phase-5 step
    // (`construct_const_memory_static_instances`) consults
    // `handles.resolve` for the ConstMemoryStatic Item handles
    // assigned in the late-pending drain. The unconditional call
    // is idempotent for fixtures with no late items and provides
    // forward safety for any future bridge addition that introduces
    // another late-item path.
    let item_remap = ctxt.model.sort_items_by_f4();
    ctxt.handles.remap_item_ids(&item_remap);
    const_::construct_const_memory_static_instances(ctxt)?;
    // Audit-finding regseq-driver-48 (spec.md section 26.6
    // `I-DiagLog-CallArity` carrier-survival contract): the
    // DiagLogPeel body pass collapses every `<LogArg as From<T>>::
    // from(...)` wrapper Call to a Goto but the wrapper Items,
    // their Instances, their per-Item FnSig entries, and the
    // `Ty::Enum { item: <LogArg-item> }` entries in the TyInterner
    // all survive. Section 26.6 pins "the `LogArg::from(...)`
    // wrappers, the slice aggregate, and the `LogArg` type do NOT
    // appear in RegseqModel". This pass runs the residue-driven
    // DCE that removes those entries (instances -> fn_sigs -> tys
    // -> items in dependency order) and remaps every surviving
    // reference. Scope is residue-driven, not generalised DCE:
    // removal seeds come from the DiagLogPeelResidue accumulator
    // populated during `run_phase5_body_passes` above.
    diag_log::dead_item_elim::eliminate_diag_log_peel_residue(ctxt)?;
    Ok(())
}

/// Construct one `InstanceKind::Function` Instance per `ItemKind::Function`
/// model Item per audit finding regseq-driver-42-4. Each Instance is
/// keyed in `function_instance_for_item` by `(ItemId, ArgsKey,
/// FunctionInstanceFlavor)` so generic monomorphisations and shim
/// flavors do NOT collapse. Per spec section 16.2 (F4-2) the table is
/// built in `(F4-2 rank, item, generic_args_key, shim_flavor)` order;
/// Function (rank 0) instances precede Static (rank 1) instances
/// (which the existing vtable / const_memory_static passes push
/// afterwards). The final `model.sort_instances_by_f4_2()` call at the
/// end of `run_bridge` is what guarantees F4-2 ordering across the
/// whole table; this pass just produces the Function-side rows.
fn construct_function_instances(ctxt: &mut BridgeCtxt<'_>) -> Result<(), BridgeError> {
    use regseq_model::{FunctionInstanceData, Instance, InstanceKind, ItemKind};
    // Walk every `ItemKind::Function` model Item per audit finding
    // regseq-driver-42-4 (Codex review item 2). Synthetic Items (drop
    // glue, vtable shims, closure call shims) are NOT filtered here;
    // for those, the rustc resolution via `source_item_def_id` will
    // return `None` (synthetic DefPathHashes do not correspond to
    // rustc DefIds) and we skip Instance creation in this pass. The
    // direct-callee resolution path in `translate_call_callee` and
    // `translate_drop_as_call` then lazy-creates Instances for them
    // through `lookup_or_create_direct_instance`, which records the
    // synthesis flavor on the Instance.
    let function_items: Vec<ItemId> = ctxt
        .model
        .items
        .iter()
        .enumerate()
        .filter_map(|(idx, item)| {
            let item_id = ItemId::from_u32(idx as u32);
            if !matches!(item.kind, ItemKind::Function) {
                return None;
            }
            Some(item_id)
        })
        .collect();

    for item_id in function_items {
        let def_path_hash = ctxt.model.item_def_paths[item_id];
        let Some(def_id) = source_item_def_id(ctxt.input.tcx, def_path_hash) else {
            // Item without a rustc DefId in this compilation (e.g.,
            // cross-crate references); the call site that needs
            // its Instance will lazy-construct one through
            // `lookup_or_create_direct_instance`.
            continue;
        };
        // Skip items that are not actually rustc functions; the
        // bridge's `ItemKind::Function` covers both FnDef and
        // closure-call shims, but the latter carry synthetic kinds
        // and were filtered above.
        if !is_rustc_function(ctxt.input.tcx, def_id) {
            continue;
        }
        let identity_args =
            rustc_middle::ty::GenericArgs::identity_for_item(ctxt.input.tcx, def_id);
        let Ok(rustc_instance) = rustc_middle::ty::Instance::try_resolve(
            ctxt.input.tcx,
            rustc_middle::ty::TypingEnv::fully_monomorphized(),
            def_id,
            identity_args,
        ) else {
            continue;
        };
        let Some(rustc_instance) = rustc_instance else {
            continue;
        };
        let flavor = function_instance_flavor_for_kind(rustc_instance.def);
        let virtual_idx = match rustc_instance.def {
            rustc_middle::ty::InstanceKind::Virtual(_, idx) => Some(idx as u32),
            _ => None,
        };
        let poly_fn_sig = ctxt
            .input
            .tcx
            .fn_sig(def_id)
            .instantiate(ctxt.input.tcx, identity_args);
        let Ok(fn_sig) = fnsig::translate_fn_sig(ctxt, poly_fn_sig) else {
            continue;
        };
        let emission_name = ctxt.model.items[item_id].def_path.rendered;
        let span = ctxt.model.items[item_id].span;
        let args_key =
            args_key_for_generic_args_with_pending(&ctxt.model, &Vec::new(), &HashMap::new());
        let instance_id = ctxt.model.instances.push(Instance {
            item: item_id,
            generic_args: Vec::new(),
            emission_name,
            span,
            kind: InstanceKind::Function(FunctionInstanceData {
                fn_sig,
                body: None,
                flavor,
                virtual_idx,
            }),
        });
        ctxt.function_instance_for_item
            .insert((item_id, args_key, flavor), instance_id);
    }
    Ok(())
}

/// Per regseq-driver-51: bridge-side eager construction of the
/// closure family's `model.instances` entries. Runs AFTER Phase-4
/// ItemId assignment (because `Instance.item: ItemId` requires final
/// IDs, per `regseq-model/src/instance.rs:134-144`) and BEFORE
/// `run_phase5_body_passes` so the materialiser loop at
/// `bridge/body/materialise.rs:303-338` can fill in `body` via the
/// closure-body resolution at `bridge/body/materialise.rs:87-100`.
///
/// Two Instance shapes per discovered closure:
///
/// - One `InstanceKind::Function` per synthesised
///   `SyntheticItemKind::ClosureCall { closure, call_kind }` Item.
///   `fn_sig` carries the substituted signature derived from the
///   closure's `closure_sig_as_fn_ptr_ty`, with a per-
///   `ClosureCallKind` receiver shape per spec.md:4542-4546:
///   - `Call`     -> `Ref(captures_struct, SharedBorrow)` (`&Captures`).
///   - `CallMut`  -> `Ref(captures_struct, UniqueBorrow)` (`&mut Captures`).
///   - `CallOnce` -> `captures_struct` by value (no `Ref`).
///
/// - One `InstanceKind::Struct { layout }` per
///   `SyntheticItemKind::ClosureCaptures { closure }` Item, where
///   `layout` is the parallel `Ty::Struct(captures_item, args, layout)`
///   TyId per stage-3 D6.3. This satisfies the spec.md:778-781
///   `body.is_some()` / `init.is_some()` invariant for non-Foreign
///   Items at the Instance side and is required by the
///   `pretty-printer.md` §10.2.4 ADT-definition rendering rule the
///   `regseq-model-95` snapshot exercises.
///
/// Instance-level idempotency uses the
/// `(ItemId, ArgsKey, FunctionInstanceFlavor)` lookup pattern shared
/// with `bridge/body/terminator.rs::lookup_or_create_direct_instance`
/// (lines 513-534) for the Function arm and a per-captures
/// "first-seen" guard for the Struct arm (the closure's captures Item
/// is unique per closure DefPathHash via the existing
/// `synth/closure.rs::closure_synth_cache` dedup). The lazy
/// closure-call path at `terminator.rs:303` keys on the same
/// translated closure args, so post-Phase-4 the eager and lazy paths
/// converge on one Instance per ClosureCall Item.
fn construct_closure_call_instances(ctxt: &mut BridgeCtxt<'_>) -> Result<(), BridgeError> {
    use regseq_model::{
        ClosureCallKind, FnAbi, FnSig, FunctionInstanceData, Instance, InstanceKind, Provenance,
        Safety, SyntheticItemKind, Ty,
    };

    // Snapshot every closure-family synthetic Item before iterating;
    // the loop body interns new `TyId`s, `FnSigId`s, and `Instance`
    // rows, any of which would invalidate a live borrow of
    // `model.item_synthetic_kinds`.
    #[derive(Clone)]
    enum ClosureArm {
        Captures(ItemId, DefPathHash),
        Call(ItemId, DefPathHash, ClosureCallKind),
    }
    let closure_arms: Vec<ClosureArm> = ctxt
        .model
        .item_synthetic_kinds
        .iter()
        .enumerate()
        .filter_map(|(idx, kind)| {
            let item_id = ItemId::from_u32(idx as u32);
            match kind {
                SyntheticItemKind::ClosureCaptures { closure } => {
                    Some(ClosureArm::Captures(item_id, *closure))
                }
                SyntheticItemKind::ClosureCall { closure, call_kind } => {
                    Some(ClosureArm::Call(item_id, *closure, *call_kind))
                }
                _ => None,
            }
        })
        .collect();

    // Per-closure shared scratch: rustc def_id, model TyId for the
    // closure (Ty::Closure view), translated identity args, captures
    // struct TyId. Built once per closure on first arm encountered.
    let mut closure_cache: std::collections::HashMap<
        DefPathHash,
        Option<(
            rustc_span::def_id::DefId,
            regseq_model::TyId,
            GenericArg,
            TyId,
        )>,
    > = std::collections::HashMap::new();
    // Track captures Items already populated with a Struct Instance so
    // a second arm referencing the same closure does not push a
    // duplicate (the captures Item is unique per closure, but this
    // guard remains correct under any future fixture that re-enters
    // the helper).
    let mut captures_instance_pushed: std::collections::HashSet<ItemId> =
        std::collections::HashSet::new();

    for arm in closure_arms {
        let closure_hash = match &arm {
            ClosureArm::Captures(_, hash) => *hash,
            ClosureArm::Call(_, hash, _) => *hash,
        };

        // Resolve / cache the per-closure scratch state on first hit.
        // The cache distinguishes "tried and failed to resolve" (None
        // entry, encoded by a sentinel below) from "not yet tried";
        // a None-entry caller falls through with `continue`.
        let resolved = if let Some(slot) = closure_cache.get(&closure_hash) {
            slot.clone()
        } else {
            let resolved = resolve_closure_scratch(ctxt, closure_hash)?;
            closure_cache.insert(closure_hash, resolved.clone());
            resolved
        };
        let Some((_closure_def_id, closure_ty_id, closure_model_args_arg, captures_struct_ty_id)) =
            resolved
        else {
            continue;
        };
        // `closure_model_args_arg` is encoded as a single
        // `GenericArg::Type(_)` slot so the cache can stay `Copy`-able.
        // Real Instance keying / construction below uses the full
        // translated args vec recovered from the `Ty::Closure` view
        // (a `Clone`).
        let closure_model_args = match ctxt.model.types.resolve(closure_ty_id) {
            Ty::Closure(_, args) => args.clone(),
            _ => continue,
        };
        let _ = closure_model_args_arg;

        match arm {
            ClosureArm::Captures(item_id, _) => {
                if !captures_instance_pushed.insert(item_id) {
                    continue;
                }
                // Pretty-printer §10.2.4 dispatches on
                // `InstanceKind::Struct { layout }` whose `layout`
                // TyId resolves to a `Ty::Struct { item, args, layout }`
                // entry. `closure_captures_struct_ty` already maps the
                // closure's `Ty::Closure` view to that parallel
                // `Ty::Struct` view per stage-3 D6.3.
                let emission_name = ctxt.model.items[item_id].def_path.rendered;
                let span = ctxt.model.items[item_id].span;
                ctxt.model.instances.push(Instance {
                    item: item_id,
                    generic_args: closure_model_args.clone(),
                    emission_name,
                    span,
                    kind: InstanceKind::Struct {
                        layout: captures_struct_ty_id,
                    },
                });
            }
            ClosureArm::Call(item_id, _, call_kind) => {
                // Resolve the closure's rustc `DefId` so we can pull
                // the closure's `ClosureArgs` and rebuild the call
                // signature. The cached `_closure_def_id` is already
                // bound; re-grab it via the closure type for clarity.
                let closure_rustc_ty = *ctxt
                    .ty_forward_cache
                    .get(&closure_ty_id)
                    .expect("Ty::Closure forward-cache entry exists after translate_ty");
                let TyKind::Closure(_, closure_generic_args) = closure_rustc_ty.kind() else {
                    continue;
                };
                let closure_args = closure_generic_args.as_closure();

                // Per spec.md:4542-4546 receiver shape per `ClosureCallKind`.
                let receiver_ty = match call_kind {
                    ClosureCallKind::Call => ctxt
                        .model
                        .types
                        .intern(Ty::Ref(captures_struct_ty_id, Provenance::SharedBorrow)),
                    ClosureCallKind::CallMut => ctxt
                        .model
                        .types
                        .intern(Ty::Ref(captures_struct_ty_id, Provenance::UniqueBorrow)),
                    ClosureCallKind::CallOnce => captures_struct_ty_id,
                };

                // Untuple `closure_sig_as_fn_ptr_ty.inputs[0]` (the
                // tupled argument struct) into flat positional inputs
                // per spec.md:4531-4541 and the existing untupling
                // pattern at `bridge/body/terminator.rs:219-249`.
                let closure_fn_sig = closure_args.sig().skip_binder();
                let tupled_input_rustc =
                    ty::erase_regions_or_reject(ctxt.input.tcx, closure_fn_sig.inputs()[0])?;
                let output_rustc =
                    ty::erase_regions_or_reject(ctxt.input.tcx, closure_fn_sig.output())?;
                let TyKind::Tuple(tuple_fields) = tupled_input_rustc.kind() else {
                    continue;
                };
                let mut inputs = Vec::with_capacity(1 + tuple_fields.len());
                inputs.push(receiver_ty);
                for field_ty in tuple_fields.iter() {
                    inputs.push(ty::translate_ty(ctxt, field_ty)?);
                }
                let output = ty::translate_ty(ctxt, output_rustc)?;

                // Closure-call FnSigs are `FnAbi::Rust` per
                // spec.md:4561-4566 (the synth rewrite normalises away
                // `RustCall`); the C-ABI lowering side table receives a
                // `None` entry to stay dense per spec.md section 3.3
                // step 5.
                let fn_sig_id = ctxt.model.sigs.intern(FnSig {
                    inputs,
                    output,
                    abi: FnAbi::Rust,
                    c_variadic: false,
                    safety: Safety::Safe,
                });
                while ctxt.model.sig_c_abi_lowerings.len() < ctxt.model.sigs.len() {
                    ctxt.model.sig_c_abi_lowerings.push(None);
                }

                // Instance-level idempotency per the
                // `(ItemId, ArgsKey, FunctionInstanceFlavor)` lookup at
                // `bridge/body/terminator.rs:513-534`. The Instance's
                // `generic_args` are the closure's translated identity
                // args so `bridge/body/materialise.rs::translate_model_generic_args`
                // can rebuild the rustc closure args for the body-fetch.
                let flavor = FunctionInstanceFlavor::Item;
                let generic_args: regseq_model::GenericArgs = closure_model_args.clone();
                let pending = ctxt.pending_def_path_hashes();
                let args_key =
                    args_key_for_generic_args_with_pending(&ctxt.model, &generic_args, &pending);
                let key = (item_id, args_key, flavor);
                if ctxt.function_instance_for_item.contains_key(&key) {
                    continue;
                }

                let emission_name = ctxt.model.items[item_id].def_path.rendered;
                let span = ctxt.model.items[item_id].span;
                let instance_id = ctxt.model.instances.push(Instance {
                    item: item_id,
                    generic_args,
                    emission_name,
                    span,
                    kind: InstanceKind::Function(FunctionInstanceData {
                        fn_sig: fn_sig_id,
                        body: None,
                        flavor,
                        virtual_idx: None,
                    }),
                });
                ctxt.function_instance_for_item.insert(key, instance_id);
            }
        }
    }
    Ok(())
}

/// Resolve per-closure scratch shared across the captures-Item and
/// call-Item passes of `construct_closure_call_instances`. Returns
/// `Ok(None)` when the closure DefPathHash does not resolve to a
/// rustc local DefId (cross-crate closures, currently out of scope)
/// or when type translation produces an unexpected shape; the caller
/// skips the arm in that case.
fn resolve_closure_scratch(
    ctxt: &mut BridgeCtxt<'_>,
    closure_hash: DefPathHash,
) -> Result<Option<(rustc_span::def_id::DefId, TyId, GenericArg, TyId)>, BridgeError> {
    use regseq_model::Ty;

    let Some(closure_local_def_id) = ctxt.input.tcx.hir_body_owners().find(|local_def_id| {
        let actual = ctxt.input.tcx.def_path_hash(local_def_id.to_def_id());
        actual.stable_crate_id().as_u64() == closure_hash.stable_crate_id.0
            && actual.local_hash().as_u64() == closure_hash.local_hash
    }) else {
        return Ok(None);
    };
    let closure_def_id = closure_local_def_id.to_def_id();
    let closure_rustc_ty = ctxt
        .input
        .tcx
        .type_of(closure_def_id)
        .instantiate_identity();
    if !matches!(closure_rustc_ty.kind(), TyKind::Closure(..)) {
        return Ok(None);
    }
    let closure_ty_id = ty::translate_ty(ctxt, closure_rustc_ty)?;
    let captures_struct_ty_id = match ctxt
        .model
        .closure_captures_struct_ty
        .get(&closure_ty_id)
        .copied()
    {
        Some(ty_id) => ty_id,
        None => return Ok(None),
    };
    // Confirm the Ty::Closure view is reachable; the explicit resolve
    // here is also a structural sanity check before the caller pushes
    // an Instance keyed off `closure_ty_id`.
    if !matches!(ctxt.model.types.resolve(closure_ty_id), Ty::Closure(..)) {
        return Ok(None);
    }
    // Sentinel `GenericArg` payload kept solely so the cache value
    // stays `Clone` without re-cloning the (potentially large)
    // closure args slice on every cache hit. The caller recovers the
    // real args from the `Ty::Closure` interner entry.
    Ok(Some((
        closure_def_id,
        closure_ty_id,
        GenericArg::Type(closure_ty_id),
        captures_struct_ty_id,
    )))
}

/// Map a `rustc_middle::ty::InstanceKind` to the closed
/// `FunctionInstanceFlavor` tag. Per spec section 9.3 +
/// `rustc_instance_kind_sort_rank`.
pub(crate) fn function_instance_flavor_for_kind(
    kind: rustc_middle::ty::InstanceKind<'_>,
) -> FunctionInstanceFlavor {
    use rustc_middle::ty::InstanceKind as RK;
    match kind {
        RK::Item(_) => FunctionInstanceFlavor::Item,
        RK::Intrinsic(_) => FunctionInstanceFlavor::Intrinsic,
        RK::VTableShim(_) => FunctionInstanceFlavor::VTableShim,
        RK::ReifyShim(_, _) => FunctionInstanceFlavor::ReifyShim,
        RK::FnPtrShim(_, _) => FunctionInstanceFlavor::FnPtrShim,
        RK::Virtual(_, _) => FunctionInstanceFlavor::Virtual,
        RK::ClosureOnceShim { .. } => FunctionInstanceFlavor::ClosureOnceShim,
        RK::ConstructCoroutineInClosureShim { .. } => {
            FunctionInstanceFlavor::ConstructCoroutineInClosureShim
        }
        RK::ThreadLocalShim(_) => FunctionInstanceFlavor::ThreadLocalShim,
        RK::FutureDropPollShim(_, _, _) => FunctionInstanceFlavor::FutureDropPollShim,
        RK::DropGlue(_, _) => FunctionInstanceFlavor::DropGlue,
        RK::CloneShim(_, _) => FunctionInstanceFlavor::CloneShim,
        RK::FnPtrAddrShim(_, _) => FunctionInstanceFlavor::FnPtrAddrShim,
        RK::AsyncDropGlueCtorShim(_, _) => FunctionInstanceFlavor::AsyncDropGlueCtorShim,
        RK::AsyncDropGlue(_, _) => FunctionInstanceFlavor::AsyncDropGlue,
    }
}

fn source_item_def_id(tcx: TyCtxt<'_>, wanted: DefPathHash) -> Option<DefId> {
    for local_def_id in tcx.hir_body_owners() {
        let actual = tcx.def_path_hash(local_def_id.to_def_id());
        if actual.stable_crate_id().as_u64() == wanted.stable_crate_id.0
            && actual.local_hash().as_u64() == wanted.local_hash
        {
            return Some(local_def_id.to_def_id());
        }
    }
    None
}

fn is_rustc_function(tcx: TyCtxt<'_>, def_id: DefId) -> bool {
    matches!(
        tcx.def_kind(def_id),
        rustc_hir::def::DefKind::Fn
            | rustc_hir::def::DefKind::AssocFn
            | rustc_hir::def::DefKind::Closure
    )
}

/// True iff `def_id` names a top-level source-derived function
/// definition that the Phase-2 baseline pass should intern as an
/// `ItemKind::Function` Item.
///
/// Excludes `DefKind::Closure`: closures are handled by the spec.md
/// section 13.1 synthesis path (ClosureCaptures + ClosureCall +
/// ClosureDropGlue) via `synth::closure::synthesise_closure_for_def_id`
/// and would otherwise be interned by both paths.
///
/// This is intentionally narrower than `is_rustc_function`, which
/// also includes `DefKind::Closure` for Instance resolution in
/// `construct_function_instances`.
pub fn is_source_derived_function(tcx: TyCtxt<'_>, def_id: DefId) -> bool {
    matches!(
        tcx.def_kind(def_id),
        rustc_hir::def::DefKind::Fn | rustc_hir::def::DefKind::AssocFn
    )
}
