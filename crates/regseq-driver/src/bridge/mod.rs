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
mod atomic;
mod clock;
pub mod body;
pub mod const_;
pub mod def_path;
mod device_lifecycle;
pub mod diag_log;
mod discovery;
pub mod fnsig;
mod framework_type;
mod handles;
mod interrupt;
pub(crate) mod intrinsic;
mod opaque_handle;
mod pinctrl;
mod serial;
mod sort;
mod synchronization;
pub mod synth;
mod timed_delay;
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
    /// Bridge-only per-Item flags populated by the eager HIR walk
    /// (Changes A, B, C of regseq-driver-58); copied into
    /// `RegseqModel::item_bridge_flags` at Phase-4 assignment time.
    /// Defaults to `BridgeItemFlags::default()` for synth/reactively-
    /// interned Items so bucket-1 enforcement stays correct.
    pub bridge_flags: regseq_model::BridgeItemFlags,
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
    /// Phase-2 cache for recognized `#[rseq::intrinsic]` callees (any family:
    /// diagnostic logging, register-access MMIO/poll).
    pub intrinsics: HashMap<DefPathHash, regseq_model::IntrinsicKind>,
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
    /// Collection-time resolution stash for source-derived
    /// `InstanceKind::Item` functions seen during the Phase-2
    /// `MonoItem::Fn` walk (regseq-driver-74 Change A). Keyed by the
    /// model `DefPathHash` (the converter `def_path_hash`, matching
    /// `model.item_def_paths`) so the orphan post-pass can recover the
    /// rustc `(DefId, GenericArgsRef)` needed to instantiate a
    /// collected-but-uninstantiated item identically to the reactive
    /// direct-call path. The stored args are the rustc
    /// `GenericArgsRef` (not model-translated; translation needs the
    /// post-Phase-4 model). Generic items (bucket 3) may write multiple
    /// monomorphisations to the same key; that is harmless because the
    /// post-pass reads the table only for non-generic orphans, whose
    /// resolution is unique.
    pub collected_item_resolutions:
        HashMap<DefPathHash, (DefId, rustc_middle::ty::GenericArgsRef<'tcx>)>,
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
    /// Transient flag (regseq-driver-82) gating the body-context
    /// reveal of return-position `impl Trait` (RPIT) `Alias(Opaque)`
    /// types in `ty::translate_erased_ty`'s alias folder. Default
    /// `false`; set to `true` only for the duration of a per-Instance
    /// body-derived type translation via the [`RevealOpaqueScope`]
    /// RAII guard (materialise + closure-call construction), and
    /// restored on guard `Drop`. It MUST stay `false` during
    /// identity-signature translation (`construct_function_instances`)
    /// so the RJ-009 bucket-4 `signature_non_rigid` carve-out is
    /// preserved.
    pub reveal_opaque_types: bool,
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
            intrinsics: HashMap::new(),
            dynamic_vtable_ty_pending: HashMap::new(),
            source_item_handles: HashMap::new(),
            source_ty_pending: HashMap::new(),
            function_instance_for_item: HashMap::new(),
            collected_item_resolutions: HashMap::new(),
            ty_forward_cache: HashMap::new(),
            diag_log_peel_residue: DiagLogPeelResidue::default(),
            reveal_opaque_types: false,
        }
    }
}

/// RAII guard (regseq-driver-82) that enables body-context reveal of
/// RPIT `Alias(Opaque)` types for the duration of a per-Instance
/// body-derived type translation, restoring the prior flag value on
/// `Drop`.
///
/// The reveal MUST be scoped to per-Instance body materialisation
/// (`body::materialise::materialise_function_body_inner`) and
/// closure-call instance construction (`construct_closure_call_instances`)
/// and MUST NOT fire during identity-signature translation
/// (`construct_function_instances`), where an uncalled fn's polymorphic
/// RPIT return must keep rejecting `NonRigidTy` so the bucket-4
/// `signature_non_rigid` carve-out (RJ-009) stays covered.
///
/// A guard (rather than a bare set/restore pair) is required because the
/// body walk uses `?` and can lazily create further Instances mid-walk
/// (`body/terminator.rs`); an early return must still restore the flag.
/// The guard exposes the wrapped `BridgeCtxt` via `Deref`/`DerefMut` so
/// the guarded region uses it exactly like the bare `&mut BridgeCtxt`.
pub(crate) struct RevealOpaqueScope<'a, 'tcx> {
    cx: &'a mut BridgeCtxt<'tcx>,
    prev: bool,
}

impl<'a, 'tcx> RevealOpaqueScope<'a, 'tcx> {
    /// Save the current `reveal_opaque_types` value, set it `true`, and
    /// return a guard that restores the saved value on `Drop`.
    pub(crate) fn enter(cx: &'a mut BridgeCtxt<'tcx>) -> Self {
        let prev = cx.reveal_opaque_types;
        cx.reveal_opaque_types = true;
        Self { cx, prev }
    }
}

impl<'tcx> std::ops::Deref for RevealOpaqueScope<'_, 'tcx> {
    type Target = BridgeCtxt<'tcx>;

    fn deref(&self) -> &Self::Target {
        self.cx
    }
}

impl<'tcx> std::ops::DerefMut for RevealOpaqueScope<'_, 'tcx> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.cx
    }
}

impl Drop for RevealOpaqueScope<'_, '_> {
    fn drop(&mut self) {
        self.cx.reveal_opaque_types = self.prev;
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
    /// An enum layout shape is outside the spec's admitted set
    /// (e.g., `Scalar::Union` tag per spec.md section 11.2 lines
    /// 3971-3975, or a `Float` tag primitive per spec.md lines
    /// 3977-3986).
    #[error("unsupported enum layout for {ty_repr}: {reason}")]
    UnsupportedEnumLayout {
        /// Rendered rustc type.
        ty_repr: String,
        /// Human-readable rejection reason.
        reason: String,
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
    /// A Rust `?`-operator MIR cluster could not be normalized by the
    /// same-type `Result`-`?` pre-translation normalizer
    /// (regseq-driver-92). Raised explicitly -- instead of the vague
    /// `NonRigidTy` the un-normalized `Try::Residual` projection would
    /// otherwise produce -- for a cross-type `?` (a real `From`
    /// conversion), an `Option`/other-carrier `?`, a recognized-but-
    /// unexpected cluster shape, or a `needs_drop` `Ok` payload the
    /// rewrite cannot faithfully preserve.
    #[error("unsupported `?` operator: {reason}")]
    UnsupportedQuestionMark {
        /// Author-facing explanation of which `?` shape was rejected.
        reason: String,
    },
    /// A `#[rseq::driver]` match descriptor named a bus family whose match
    /// sugar is not yet supported (`acpi`/`usb`/`i2c`/`spi`). A parse-time
    /// rejection for the Device Lifecycle construct (PLAN CR-5), deliberately
    /// distinct from the D1/D2/D3 validation diagnostics.
    #[error("device-lifecycle: `{bus}` bus match sugar is not yet supported")]
    DeviceLifecycleUnsupportedBus {
        /// The deferred bus family token (`acpi`/`usb`/`i2c`/`spi`).
        bus: String,
    },
    /// A `#[rseq::driver]` failed device-lifecycle validation (PLAN T16, GRILL
    /// I09): D1 (missing-match) / D2 (empty-match) / D3 (non-initializable
    /// user-data). Distinct from the parse-time CR-5 unsupported-bus rejection.
    #[error("device-lifecycle validation: {0}")]
    DeviceLifecycleValidation(DeviceLifecycleDiagnostic),

    /// A device-interrupt frontend check failed (construct #6, contract C06).
    #[error("device-interrupt validation: {0}")]
    InterruptValidation(InterruptDiagnostic),

    /// A peripheral-clock-management frontend (recognition) check failed
    /// (clock construct, contract C06 — the `CLK-*` recognition set). The
    /// binding-side `CLK-L2-*` set lives in `regseq-l2`.
    #[error("peripheral-clock-management validation: {0}")]
    ClockValidation(ClockDiagnostic),

    /// A pin-controller-and-GPIO-configuration frontend (recognition) check
    /// failed (pinctrl state-selection construct, contract C06 — the `PIN-ERR-*`
    /// recognition set). The binding-side `PIN-L2-*` set lives in `regseq-l2`.
    #[error("pin-controller-and-gpio-configuration validation: {0}")]
    PinctrlValidation(PinctrlDiagnostic),

    /// A timed-cpu-delay-and-sleep frontend check failed (TCD construct,
    /// contracts C03/C05).
    #[error("timed-cpu-delay-and-sleep validation: {0}")]
    TimedCpuDelayValidation(TimedCpuDelayDiagnostic),

    /// An atomic-memory-access-and-ordering frontend check failed (atomicity
    /// construct, contracts C01/C02/C07).
    #[error("atomic-memory-access-and-ordering validation: {0}")]
    AtomicValidation(AtomicDiagnostic),

    /// A serial device-class frontend check failed (construct #7).
    #[error("serial validation: {0}")]
    SerialValidation(SerialDiagnostic),

    /// A synchronization frontend check failed (synchronization construct,
    /// contract C03). The §7 context checks land in M3; M2 carries the
    /// deferred-surface rejections (e.g. `AllWaiters` cardinality).
    #[error("synchronization validation: {0}")]
    SynchronizationValidation(SyncDiagnostic),
}

/// Per-construct atomic-memory-access-and-ordering (atomicity) frontend
/// diagnostics (GRILL C02; messages follow the taxonomy organized-spec §9 where
/// an ID exists, generalized per O3 for the degenerate-ordering check;
/// `OrderingNonConst` is the implementer-defined O4 addition). Carried by
/// [`BridgeError::AtomicValidation`].
///
/// `MissingOrdering` is reclassified to the **facade-signature** residual (D15):
/// the canonical typed facade makes a missing-`Ordering` *call* a rustc error
/// before the bridge runs, so the only reachable Regseq check is a *mis-declared*
/// facade (a `#[rseq::intrinsic("atomic_*")]` callee whose signature omits the
/// `Ordering` parameter), mirroring `TimedCpuDelayDiagnostic::ArgTypeMismatch`
/// and `InterruptDiagnostic`'s ISR-signature check.
#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum AtomicDiagnostic {
    /// `ATOMIC-ERR-ELEMENT-TYPE`: an `Atomic<T>` field whose `T` is outside the
    /// admitted set `{u32,i32,u64,i64}` (permissive pointer atomics accepted).
    #[error(
        "ATOMIC-ERR-ELEMENT-TYPE / Atomicity / Atomic Memory Access And Ordering: Atomic<{ty}> declaration on field \
         {field} uses unsupported element type; the locked surface accepts u32/i32/u64/i64 \
         (and permissive pointer atomics per G-ATM-07 anti-narrowing standing item)"
    )]
    ElementType {
        /// The offending field name.
        field: String,
        /// The unsupported element type `T`.
        ty: String,
    },
    /// `ATOMIC-ERR-MISSING-ORDERING` (facade-signature residual, D15): a
    /// recognized `#[rseq::intrinsic("atomic_*")]` callee whose signature omits
    /// the mandatory `Ordering` parameter(s). The canonical-facade call-site
    /// omission is type-system-subsumed (a rustc error before the bridge runs).
    #[error(
        "ATOMIC-ERR-MISSING-ORDERING / Atomicity / Atomic Memory Access And Ordering: atomic operation facade `{facade}` \
         must take an explicit `Ordering` argument; there is no implicit default ({detail})"
    )]
    MissingOrdering {
        /// The recognized facade path/leaf.
        facade: String,
        /// What is wrong with the declared signature.
        detail: String,
    },
    /// `ATOMIC-ERR-BIT-TEST-TYPE`: `bit_test_*` on a non-unsigned atomic
    /// element, or a compile-time-constant out-of-range bit index (folded under
    /// the same ID per C02).
    #[error(
        "ATOMIC-ERR-BIT-TEST-TYPE / Atomicity / Atomic Memory Access And Ordering: bit-test operation {op} {detail}"
    )]
    BitTestType {
        /// The bit-test op (`bit_test_set`/`bit_test_clear`).
        op: String,
        /// The specific violation (non-unsigned element, or out-of-range bit).
        detail: String,
    },
    /// `ATOMIC-ERR-DEGENERATE-ACQREL` (generalized per O3 to "invalid ordering
    /// for operation"): an `Ordering` outside the op's valid set per C01 —
    /// `AcqRel`/`Release` on `load`, `AcqRel`/`Acquire` on `store`, or an
    /// invalid/stronger-than-success `compare_exchange` failure ordering.
    #[error(
        "ATOMIC-ERR-DEGENERATE-ACQREL / Atomicity / Atomic Memory Access And Ordering: Ordering::{ordering} is not valid for \
         {op} (valid: {valid}); the locked per-op ordering matrix rejects it"
    )]
    DegenerateAcqrel {
        /// The operation.
        op: String,
        /// The rejected ordering.
        ordering: String,
        /// The valid ordering set for this op.
        valid: String,
    },
    /// `ATOMIC-ERR-FENCE-RELAXED`: `atomic::fence(Relaxed)` (no cross-CPU effect).
    #[error(
        "ATOMIC-ERR-FENCE-RELAXED / Atomicity / Atomic Memory Access And Ordering: atomic::fence(Relaxed) is degenerate \
         (a Relaxed fence has no cross-CPU effect); use Acquire / Release / AcqRel / SeqCst"
    )]
    FenceRelaxed,
    /// `ATOMIC-ERR-MMIO-FENCE-COLLAPSE` (designed-in, D02/O1): an MMIO-ordering
    /// op smuggled onto the atomic surface. No live trigger until register-access
    /// lands its `io_fence`/`ordered!` ordering surface (M5/T19 placeholder).
    #[error(
        "ATOMIC-ERR-MMIO-FENCE-COLLAPSE / Atomicity / Atomic Memory Access And Ordering: primitive {mmio_op} is excluded from \
         this construct's surface (MMIO ordering belongs to Register Access); use \
         register_access.io_fence for MMIO ordering and atomic_operations.fence for CPU \
         ordering — the two co-exist at MMIO+atomic boundaries"
    )]
    MmioFenceCollapse {
        /// The smuggled MMIO op name.
        mmio_op: String,
    },
    /// `ATOMIC-ERR-ORDERING-NONCONST` (implementer-defined, D14/O4): an
    /// `Ordering` argument that is not a compile-time constant. A runtime
    /// ordering cannot be statically validated (C01) or lowered (C03).
    #[error(
        "ATOMIC-ERR-ORDERING-NONCONST / Atomicity / Atomic Memory Access And Ordering: the Ordering argument to {op} must be a \
         compile-time constant (a literal Ordering::X or a const); a runtime Ordering cannot be \
         statically validated or lowered"
    )]
    OrderingNonConst {
        /// The operation whose ordering argument was non-constant.
        op: String,
    },
    /// `ATOMIC-ERR-NESTED-ATOMIC` (S01; O6 — taxonomy-feedback addition): an
    /// atomic op whose receiver is NOT a direct driver-struct field (it reaches
    /// the `Atomic<T>` through a nested struct field or an array index). The
    /// generic-c oracle, Linux, and Zephyr all require a direct field — this
    /// frontend reject keeps the behavioral oracle and the OS backends in lock-
    /// step (an oracle that accepted nested would predict acceptance the OS
    /// backends then refuse).
    #[error(
        "ATOMIC-ERR-NESTED-ATOMIC / Atomicity / Atomic Memory Access And Ordering: the {op} receiver \
         is not a direct driver-struct field ({detail}); an Atomic<T> must be a direct field (S01)"
    )]
    NestedField {
        /// The operation.
        op: String,
        /// Why the receiver is not a direct field.
        detail: String,
    },
    /// `ATOMIC-ERR-ORDERING-ENUM` (O7 — taxonomy-feedback addition): a recognized
    /// atomic facade whose `Ordering` parameter enum does not declare exactly
    /// `Relaxed, Acquire, Release, AcqRel, SeqCst` in that order. The const
    /// `Ordering` is read as its POSITIONAL discriminant (the C05 ABI:
    /// `Relaxed=0 … SeqCst=4`), so a same-named enum with a different variant
    /// order (e.g. `core::sync::atomic::Ordering`, which swaps Release/Acquire)
    /// would SILENTLY remap orderings. Requiring the canonical order makes the
    /// positional read sound.
    #[error(
        "ATOMIC-ERR-ORDERING-ENUM / Atomicity / Atomic Memory Access And Ordering: the `Ordering` enum \
         used by `{facade}` must declare exactly Relaxed, Acquire, Release, AcqRel, SeqCst in that \
         order ({detail}); the compile-time-constant ordering is read by positional discriminant"
    )]
    OrderingEnum {
        /// The recognized facade.
        facade: String,
        /// What is wrong with the enum's variants.
        detail: String,
    },
}

/// Per-construct serial device-class frontend diagnostics (construct #7). The
/// `SER-ERR-*` family. Carried by [`BridgeError::SerialValidation`]. T03 mints
/// the recognition diagnostics (`MISSING-BASE`, `TYPE-MISMATCH`,
/// `BAD-VARIANTS`); T04 adds the enforcement matrix (dynamic `irq_source_set`
/// call rejection + deferred-member capability gates).
#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum SerialDiagnostic {
    /// `SER-ERR-MISSING-BASE`: a `#[rseq::device_class(serial)]` driver does not
    /// implement the required base `Serial` trait (C04).
    #[error(
        "SER-ERR-MISSING-BASE: serial-class driver `{driver}` does not implement the base \
         `Serial` trait (C04: port_service_set/data_ready_query/data_unit_write/data_unit_read)"
    )]
    MissingBase {
        /// The offending driver's logical name.
        driver: String,
    },
    /// `SER-ERR-TYPE-MISMATCH`: a recognized serial method carries a support
    /// type of the wrong shape (e.g. a non-enum where an enum is required).
    #[error("SER-ERR-TYPE-MISMATCH: driver `{driver}` serial type `{type_name}`: {detail}")]
    TypeMismatch {
        /// The offending driver's logical name.
        driver: String,
        /// The support type that mismatched.
        type_name: String,
        /// What was expected.
        detail: String,
    },
    /// `SER-ERR-BAD-VARIANTS`: a serial support enum is missing a required
    /// variant of the closed vocab (so the compiler cannot mint the literal).
    #[error("SER-ERR-BAD-VARIANTS: driver `{driver}` serial enum `{type_name}`: {detail}")]
    BadVariants {
        /// The offending driver's logical name.
        driver: String,
        /// The offending enum's name.
        type_name: String,
        /// What was missing/wrong.
        detail: String,
    },
    /// `SER-ERR-DYNAMIC-IRQ-SET` (T04, R07/D03/I01): an author method body calls
    /// `irq_source_set` directly. The op is BACKEND-CALLED only — the compiler
    /// owns WHEN each source is armed/disarmed (the per-OS scatter sites); a
    /// dynamic author call breaks the portable placement contract.
    #[error(
        "SER-ERR-DYNAMIC-IRQ-SET: driver `{driver}` method `{caller}` calls `irq_source_set` \
         directly — it is backend-called only (the compiler arms/disarms sources at the per-OS \
         scatter sites; the author owns only the mask bits in the `irq_source_set` match arms)"
    )]
    DynamicIrqSourceSet {
        /// The offending driver's logical name.
        driver: String,
        /// The author method whose body contains the illegal call.
        caller: String,
    },
    /// `SER-ERR-DEFERRED-MEMBER` (T04, R10/C03/C05): an author impl OVERRIDES a
    /// v1-deferred member inside an in-scope trait (`SerialRuntimeLineConfig::`
    /// `{line_format_readback,break_set,flow_auto_set}` or
    /// `SerialQueueControl::fifo_purge_hw`). Deferred members are capability-
    /// gated out of v1; the backend never sees a partially-built surface.
    #[error(
        "SER-ERR-DEFERRED-MEMBER: driver `{driver}` impl of `{trait_name}` overrides the \
         v1-deferred member `{member}` — that capability is not built in v1 (defer it)"
    )]
    DeferredMember {
        /// The offending driver's logical name.
        driver: String,
        /// The in-scope trait carrying the deferred member.
        trait_name: String,
        /// The deferred member that was overridden.
        member: String,
    },
    /// `SER-ERR-DEFERRED-TRAIT` (T04, R10/C06/C07/C08): a serial-class driver
    /// implements a v1-deferred companion trait (`SerialModemControl`,
    /// `SerialConsole`, or `SerialBulkTransfer`). Those planes are out of v1.
    #[error(
        "SER-ERR-DEFERRED-TRAIT: driver `{driver}` implements the v1-deferred serial trait \
         `{trait_name}` — that plane is not built in v1 (defer it)"
    )]
    DeferredTrait {
        /// The offending driver's logical name.
        driver: String,
        /// The deferred trait that was implemented.
        trait_name: String,
    },
    /// `SER-ERR-REPR-CONST` (O2 hardening): an author bit-position const
    /// (`CAUSE_*` for `CauseSetRepr`, `RX_ERR_*` for `RxErrorRepr`) disagrees with
    /// the pinned representation's convention (bit position = the canonical
    /// `SerialCause`/`SerialRxErrorKind` discriminant). The backends decode the
    /// authored bitsets against the pinned convention, so a mismatched const
    /// would make them silently MISCLASSIFY the decoded causes / inline errors —
    /// caught here at the frontend instead.
    #[error(
        "SER-ERR-REPR-CONST: {repr} bit const `{const_name}` = {found:#x}, but the pinned \
         {repr}Repr convention requires {expected:#x} (bit position = the canonical \
         discriminant); the backend would misclassify — fix the const to match"
    )]
    ReprConstMismatch {
        /// Which representation the const belongs to (`CauseSet` / `RxError`).
        repr: String,
        /// The offending const's name.
        const_name: String,
        /// The author's value.
        found: u32,
        /// The pinned-convention value (`1 << discriminant`).
        expected: u32,
    },
    /// `SER-ERR-BITSET-PROVENANCE` (O2 body-provenance hardening): the value
    /// constructed and handed to `<CauseSet|RxError>::from_bits` in the author's
    /// `irq_cause_decode` / `data_unit_read` body carries bits the backend would
    /// MISCLASSIFY — either a constant bit outside the pinned representation's
    /// bit-domain (a raw literal or a wrong-value const), or a non-constant
    /// (device-derived / computed) contribution. The backend decodes the bitset
    /// against the pinned convention, so the bitset must be built ONLY from
    /// convention-domain constant bits. This catches the raw-literal /
    /// differently-named-const / device-bit-leak bypasses of the named-const
    /// check; the device→semantic mapping (which device bit means which cause)
    /// remains irreducibly author knowledge (documented residual).
    #[error(
        "SER-ERR-BITSET-PROVENANCE: the `{repr}` bitset built in `{op}` is not a pure OR of \
         pinned {repr}Repr convention bits — {detail}; the backend would misclassify"
    )]
    BitsetProvenance {
        /// Which representation (`CauseSet` / `RxError`).
        repr: String,
        /// The op whose body builds the bitset.
        op: String,
        /// What was wrong (foreign bits / non-constant contribution).
        detail: String,
    },
}

/// Per-construct device-lifecycle validation diagnostics (PLAN T16; GRILL I09;
/// D06 per-construct numbering). D4 is deferred. Carried by
/// [`BridgeError::DeviceLifecycleValidation`]. M6 constructs only D3
/// ([`Self::UserDataNotInitializable`]); D1/D2 ([`Self::MissingMatch`] /
/// [`Self::EmptyMatch`]) are defined here for the M8 match-validation pass (T17).
#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum DeviceLifecycleDiagnostic {
    /// D1: a `#[rseq::driver]` declared no device-match attribute.
    #[error("driver `{driver}` declares no device-match attribute (expected one of compatible/pci/amba)")]
    MissingMatch {
        /// The offending driver's logical name.
        driver: String,
    },
    /// D2: a `#[rseq::driver]`'s device-match set is present but empty.
    #[error("driver `{driver}` declares an empty device-match set")]
    EmptyMatch {
        /// The offending driver's logical name.
        driver: String,
    },
    /// D3: a user-data field whose type is neither Rule-1 (the scalar zero-init
    /// whitelist bool/u8/u16/u32/u64) nor Rule-2 (`Option<T: Copy>`).
    #[error(
        "driver `{driver}` field `{field}` of type `{rejected_type}` is not a valid user-data \
         field (expected a scalar bool/u8/u16/u32/u64 or Option<T: Copy>)"
    )]
    UserDataNotInitializable {
        /// The offending driver's logical name.
        driver: String,
        /// The offending field name.
        field: String,
        /// The rejected field type, rendered.
        rejected_type: String,
    },
}

/// Per-construct device-interrupt frontend diagnostics (construct #6,
/// contract C06; GRILL D08/D13). Carried by
/// [`BridgeError::InterruptValidation`]. This is the T03 M1-critical subset;
/// T08 completes the C06 family (`IRQ-ERR-ARRAY-BIND-NO-INDEX`,
/// `IRQ-ERR-EXCLUDED-PRIMITIVE`, `IRQ-ERR-DRIVER-CALLABLE-LINE-CONTROL`, the
/// R11 shared-discipline warning). The `IRQ-ERR-MISSING-ID` /
/// `IRQ-ERR-MISSING-HANDLER` / `IRQ-ERR-UNKNOWN-FIELD` / `IRQ-ERR-ISR-SIGNATURE`
/// IDs are minted here within the `IRQ-ERR-*` family (GRILL O3).
#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum InterruptDiagnostic {
    /// `IRQ-ERR-MISSING-ID`: an `Interrupt` field without `id` (R01/D08 —
    /// mandatory, no field-name fallback; intended taxonomy tightening).
    #[error(
        "IRQ-ERR-MISSING-ID: driver `{driver}` interrupt field `{field}` carries no \
         `#[rseq::interrupt(id = \"…\")]` join key (the id is mandatory; there is no \
         field-name fallback)"
    )]
    MissingId {
        /// The offending driver's logical name.
        driver: String,
        /// The offending field name.
        field: String,
    },
    /// `IRQ-ERR-BARE-ISR-AMBIGUOUS`: a bare `#[rseq::isr]` on a driver with
    /// ≠1 `Interrupt` resource.
    #[error(
        "IRQ-ERR-BARE-ISR-AMBIGUOUS: driver `{driver}` method `{method}` is a bare \
         #[rseq::isr] but the driver declares {resource_count} Interrupt resources \
         (bare binding requires exactly one; use `#[rseq::isr(field = \"…\")]`)"
    )]
    BareIsrAmbiguous {
        /// The offending driver's logical name.
        driver: String,
        /// The handler method name.
        method: String,
        /// How many `Interrupt` resources the driver declares.
        resource_count: usize,
    },
    /// `IRQ-ERR-DUPLICATE-HANDLER`: >1 handler bound to one resource.
    #[error(
        "IRQ-ERR-DUPLICATE-HANDLER: driver `{driver}` binds more than one #[rseq::isr] \
         handler to interrupt resource `{resource}` (exactly one per resource)"
    )]
    DuplicateHandler {
        /// The offending driver's logical name.
        driver: String,
        /// The doubly-bound resource's field name.
        resource: String,
    },
    /// `IRQ-ERR-MISSING-HANDLER`: 0 handlers on a non-optional resource (D13).
    #[error(
        "IRQ-ERR-MISSING-HANDLER: driver `{driver}` interrupt resource `{resource}` has \
         no #[rseq::isr] handler (a non-optional Interrupt requires exactly one)"
    )]
    MissingHandler {
        /// The offending driver's logical name.
        driver: String,
        /// The handlerless resource's field name.
        resource: String,
    },
    /// `IRQ-ERR-UNKNOWN-FIELD`: `#[rseq::isr(field = "…")]` naming a field
    /// that is not a recognized `Interrupt` resource.
    #[error(
        "IRQ-ERR-UNKNOWN-FIELD: #[rseq::isr] method `{method}` names field `{field}`, \
         which is not a recognized Interrupt resource on its driver"
    )]
    UnknownField {
        /// The handler method name.
        method: String,
        /// The unknown field name.
        field: String,
    },
    /// `IRQ-ERR-ISR-SIGNATURE`: the handler signature violates C04
    /// (`&mut self -> IrqResult`, `index: usize` for array-bind, never
    /// `Result<_, RegseqError>`).
    #[error("IRQ-ERR-ISR-SIGNATURE: #[rseq::isr] method `{method}`: {detail}")]
    IsrSignature {
        /// The handler method name.
        method: String,
        /// What is wrong with the signature.
        detail: String,
    },
    /// `IRQ-ERR-ARRAY-BIND-NO-INDEX`: a handler bound to an `[Interrupt; N]`
    /// resource without the `index: usize` parameter (C06).
    #[error(
        "IRQ-ERR-ARRAY-BIND-NO-INDEX: #[rseq::isr] method `{method}` binds array \
         interrupt resource `{resource}` but lacks the `index: usize` parameter \
         (the array-bind form is `fn(&mut self, index: usize) -> IrqResult`)"
    )]
    ArrayBindNoIndex {
        /// The handler method name.
        method: String,
        /// The array resource's field name.
        resource: String,
    },
    /// `IRQ-ERR-EXCLUDED-PRIMITIVE`: an excluded-set reintroduction (C06) —
    /// a trait-provided ISR binding (the witnessed exclusion; the
    /// sync-with-handler exclusion becomes representable only when the
    /// Synchronization construct lands), or an `#[rseq::isr]` anchored
    /// outside an inherent `#[rseq::driver]` impl.
    #[error("IRQ-ERR-EXCLUDED-PRIMITIVE: #[rseq::isr] on `{method}`: {detail}")]
    ExcludedPrimitive {
        /// The offending method name.
        method: String,
        /// Which exclusion was reintroduced.
        detail: String,
    },
    /// `IRQ-ERR-DRIVER-CALLABLE-LINE-CONTROL`: the `Interrupt` resource shell
    /// grew driver-callable methods (C06/I02) — line control
    /// (register/enable/disable/deregister) is compiler-managed lifecycle
    /// placement, never an author call.
    #[error(
        "IRQ-ERR-DRIVER-CALLABLE-LINE-CONTROL: the `Interrupt` resource shell declares \
         a driver-callable method `{method}` — interrupt line control is \
         compiler-managed (I02); the shell must stay methodless"
    )]
    DriverCallableLineControl {
        /// The offending method name.
        method: String,
    },
}

/// Per-construct peripheral-clock-management frontend (recognition) diagnostics
/// (clock construct, contract C06; GRILL D04/D13/D17). Carried by
/// [`BridgeError::ClockValidation`]. The `CLK-ERR-*` family is the recognition
/// half (this enum); the binding half is the `regseq-l2` `CLK-L2-*` family.
/// This is the M1/T04 subset (mandatory-id, unsupported-form, duplicate-id);
/// T09 completes the C06 recognition set (forbidden/unknown attr keys, spoofed
/// `clock_*` receiver). Concrete numbering is blessed here within the locked
/// `CLK-ERR-*` family (GRILL D17/O3).
#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum ClockDiagnostic {
    /// `CLK-ERR-MISSING-ID`: a `Clock` field without a usable
    /// `#[rseq::clock(id = "…")]` join key — absent attribute, absent `id`, or
    /// an empty/whitespace id (R01/D13 — mandatory, no field-name fallback;
    /// C06 N1/N12).
    #[error(
        "CLK-ERR-MISSING-ID: driver `{driver}` clock field `{field}` carries no usable \
         `#[rseq::clock(id = \"…\")]` join key (the id is mandatory and non-empty; there is \
         no field-name fallback)"
    )]
    MissingId {
        /// The offending driver's logical name.
        driver: String,
        /// The offending field name.
        field: String,
    },
    /// `CLK-ERR-UNSUPPORTED-FORM`: a recognized clock field in a form v1 does
    /// not build — `Option<Clock>` (optional form, design-in) or `[Clock; N]`
    /// (array form). v1 builds only the required `Clock` form (C06 N2/N14).
    #[error(
        "CLK-ERR-UNSUPPORTED-FORM: driver `{driver}` clock field `{field}` uses the \
         unsupported `{form}` form; v1 builds only the required `Clock` form \
         (`Option<Clock>` and arrays are designed-in, not built)"
    )]
    UnsupportedForm {
        /// The offending driver's logical name.
        driver: String,
        /// The offending field name.
        field: String,
        /// The recognized-but-rejected form (e.g. `Option<Clock>`).
        form: String,
    },
    /// `CLK-ERR-DUPLICATE-ID`: two `Clock` fields on one driver declare the same
    /// L1 `id` (C06 N15; the clock id namespace is construct-local and the L1 id
    /// must be unique within a driver, GRILL D13).
    #[error(
        "CLK-ERR-DUPLICATE-ID: driver `{driver}` declares more than one clock resource with \
         id `{id}` (a clock id must be unique within a driver)"
    )]
    DuplicateId {
        /// The offending driver's logical name.
        driver: String,
        /// The duplicated clock id.
        id: String,
    },
    /// `CLK-ERR-MALFORMED-ATTR`: the `#[rseq::clock(...)]` attribute is
    /// structurally bad — a duplicate `id` key, or an unknown key that is not a
    /// recognized clock attribute (C06 N12). Clock's L1 attribute admits ONLY
    /// `id` (unlike `#[rseq::mmio]`/`#[rseq::interrupt]`, clock rejects unknown
    /// keys rather than ignoring them, GRILL D13).
    #[error(
        "CLK-ERR-MALFORMED-ATTR: driver `{driver}` clock field `{field}`: {detail} \
         (the `#[rseq::clock(...)]` attribute admits only `id = \"…\"`)"
    )]
    MalformedAttr {
        /// The offending driver's logical name.
        driver: String,
        /// The offending field name.
        field: String,
        /// What is malformed (e.g. `duplicate \`id\` key`, `unknown key \`foo\``).
        detail: String,
    },
    /// `CLK-ERR-ATTR-ON-NON-CLOCK`: a `#[rseq::clock(...)]` attribute on a field
    /// whose type is not the `Clock` resource shell (C06 N13).
    #[error(
        "CLK-ERR-ATTR-ON-NON-CLOCK: driver `{driver}` field `{field}` carries \
         `#[rseq::clock(...)]` but is not a `Clock` resource"
    )]
    AttrOnNonClock {
        /// The offending driver's logical name.
        driver: String,
        /// The offending field name.
        field: String,
    },
    /// `CLK-ERR-FORBIDDEN-KEY`: a forbidden L1 attribute key (C06 N21). Keys like
    /// `required`/`mode`/`default_frequency` belong in the `[clock.<id>]` PLATFORM
    /// section, not the L1 attribute; `set_rate`/`clk_bulk_*`/`clock_auto_enable`
    /// are reserved/out-of-scope. Distinct from an unknown key (N12) so the
    /// diagnostic can point the author at the right surface.
    #[error(
        "CLK-ERR-FORBIDDEN-KEY: driver `{driver}` clock field `{field}`: key `{key}` is not \
         permitted on `#[rseq::clock(...)]` (mode/default_frequency/provider belong in the \
         `[clock.<id>]` platform section; set_rate/clk_bulk/clock_auto_enable are reserved)"
    )]
    ForbiddenAttrKey {
        /// The offending driver's logical name.
        driver: String,
        /// The offending field name.
        field: String,
        /// The forbidden key.
        key: String,
    },
    /// `CLK-ERR-SPOOFED-RECEIVER`: a `#[rseq::intrinsic("clock_*")]` tag on a
    /// method whose receiver is not the `Clock` resource shell (C06 N22) — a
    /// spoofed clock op (the `clock_*` intrinsics are compiler-managed on the
    /// `Clock` facade only).
    #[error(
        "CLK-ERR-SPOOFED-RECEIVER: method `{method}` is tagged a `clock_*` intrinsic but its \
         receiver is `{receiver}`, not the `Clock` resource (the clock ops exist only on `Clock`)"
    )]
    SpoofedReceiver {
        /// The offending method name.
        method: String,
        /// The receiver type the method is declared on.
        receiver: String,
    },
}

/// Per-construct pin-controller-and-GPIO-configuration frontend (recognition)
/// diagnostics (pinctrl state-selection construct, contract C06; GRILL
/// D04/D07/D11). Carried by [`BridgeError::PinctrlValidation`]. The `PIN-ERR-*`
/// family is the recognition half (this enum); the binding half is the
/// `regseq-l2` `PIN-L2-*` family. The M1/T04 subset (mandatory-id,
/// malformed/attr-on-non-pinctrl, unsupported-form, duplicate-id, malformed
/// state-attr) is extended here by the M2/T09–T10 tcx-dependent recognition set
/// (non-ZST marker, duplicate state marker, spoofed `apply` receiver). The
/// body-walk SEMANTIC checks that need a materialised model + the L2 join
/// (author-managed field, unresolved generic `S`, invalid state membership) are
/// pure-model and live in `regseq_model::PinctrlSemanticError` (run post-L2 from
/// `callbacks.rs` + the runner). Concrete numbering is blessed here within the
/// locked `PIN-ERR-*` family (GRILL D11).
#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum PinctrlDiagnostic {
    /// `PIN-ERR-MISSING-ID`: a `PinCtrl`/`Option<PinCtrl>` field without a usable
    /// `#[rseq::pinctrl(id = "…")]` join key — absent attribute, absent `id`, or
    /// an empty/whitespace id (R01/D07 — mandatory, no field-name fallback; C06).
    #[error(
        "PIN-ERR-MISSING-ID: driver `{driver}` pinctrl field `{field}` carries no usable \
         `#[rseq::pinctrl(id = \"…\")]` join key (the id is mandatory and non-empty; there is \
         no field-name fallback)"
    )]
    MissingId {
        /// The offending driver's logical name.
        driver: String,
        /// The offending field name.
        field: String,
    },
    /// `PIN-ERR-MALFORMED-ATTR`: the `#[rseq::pinctrl(...)]` attribute is
    /// structurally bad — a duplicate `id` key, a non-string `id`, or an unknown
    /// key (C06). The pinctrl L1 attribute admits ONLY `id = "…"` (the
    /// `pin_states` inventory belongs in the `[pinctrl.<id>]` PLATFORM section).
    #[error(
        "PIN-ERR-MALFORMED-ATTR: driver `{driver}` pinctrl field `{field}`: {detail} \
         (the `#[rseq::pinctrl(...)]` attribute admits only `id = \"…\"`)"
    )]
    MalformedAttr {
        /// The offending driver's logical name.
        driver: String,
        /// The offending field name.
        field: String,
        /// What is malformed (e.g. `duplicate \`id\` key`, `unknown key \`foo\``).
        detail: String,
    },
    /// `PIN-ERR-ATTR-ON-NON-PINCTRL`: a `#[rseq::pinctrl(...)]` attribute on a
    /// field whose type is not the `PinCtrl`/`Option<PinCtrl>` resource (C06).
    #[error(
        "PIN-ERR-ATTR-ON-NON-PINCTRL: driver `{driver}` field `{field}` carries \
         `#[rseq::pinctrl(...)]` but is not a `PinCtrl` resource"
    )]
    AttrOnNonPinctrl {
        /// The offending driver's logical name.
        driver: String,
        /// The offending field name.
        field: String,
    },
    /// `PIN-ERR-UNSUPPORTED-FORM`: a recognized pinctrl field in a form the
    /// construct does not build — `[PinCtrl; N]` (array form) / `&PinCtrl`
    /// (reference) / `Vec<PinCtrl>`. Build-wide v1 builds the required `PinCtrl`
    /// and optional `Option<PinCtrl>` forms only (C06; arrays designed-in).
    #[error(
        "PIN-ERR-UNSUPPORTED-FORM: driver `{driver}` pinctrl field `{field}` uses the \
         unsupported `{form}` form; the construct builds only the required `PinCtrl` and \
         optional `Option<PinCtrl>` forms (arrays/refs are designed-in, not built)"
    )]
    UnsupportedForm {
        /// The offending driver's logical name.
        driver: String,
        /// The offending field name.
        field: String,
        /// The recognized-but-rejected form (e.g. `[PinCtrl; N]`).
        form: String,
    },
    /// `PIN-ERR-DUPLICATE-ID`: two pinctrl fields on one driver declare the same
    /// L1 `id` (C06; the pinctrl id namespace is construct-local and the L1 id
    /// must be unique within a driver, GRILL D07).
    #[error(
        "PIN-ERR-DUPLICATE-ID: driver `{driver}` declares more than one pinctrl resource with \
         id `{id}` (a pinctrl id must be unique within a driver)"
    )]
    DuplicateId {
        /// The offending driver's logical name.
        driver: String,
        /// The duplicated pinctrl id.
        id: String,
    },
    /// `PIN-ERR-MALFORMED-STATE-ATTR`: a `#[rseq::pin_state(...)]` marker
    /// attribute is structurally bad — missing/empty `id`, a non-string `id`, a
    /// duplicate or unknown key (C06). The pin-state marker attribute admits ONLY
    /// `id = "…"`.
    #[error(
        "PIN-ERR-MALFORMED-STATE-ATTR: pin-state marker `{marker}`: {detail} \
         (the `#[rseq::pin_state(...)]` attribute admits only `id = \"…\"`)"
    )]
    MalformedStateAttr {
        /// The offending marker type's name.
        marker: String,
        /// What is malformed (e.g. `id is empty or whitespace`).
        detail: String,
    },
    /// `PIN-ERR-NON-ZST-MARKER`: a `#[rseq::pin_state(...)]` marker type that is
    /// not unit-like — its struct carries one or more fields (C06). Pin-state
    /// markers are author-declared *unit* structs (D04); a payload-carrying
    /// marker is rejected (the field-count check is stricter than ZST-ness, so a
    /// `PhantomData`-bearing marker is also rejected — a marker carries no data).
    #[error(
        "PIN-ERR-NON-ZST-MARKER: pin-state marker `{marker}` is not a unit struct (it carries \
         {fields} field(s)); a `#[rseq::pin_state]` marker must be a fieldless unit struct"
    )]
    NonZstMarker {
        /// The offending marker type's name.
        marker: String,
        /// The number of fields the marker struct declares.
        fields: usize,
    },
    /// `PIN-ERR-DUPLICATE-STATE-MARKER`: two `#[rseq::pin_state(id = "…")]`
    /// markers bind the same state name (C06). A pin-state name maps to exactly
    /// one marker type; two markers claiming one name is ambiguous.
    #[error(
        "PIN-ERR-DUPLICATE-STATE-MARKER: pin-state name `{id}` is claimed by more than one \
         `#[rseq::pin_state]` marker (`{first}` and `{second}`); each state name maps to one marker"
    )]
    DuplicateStateMarker {
        /// The duplicated state name (the `id`).
        id: String,
        /// The first marker type declaring the name.
        first: String,
        /// The second marker type declaring the name.
        second: String,
    },
    /// `PIN-ERR-SPOOFED-RECEIVER`: a `#[rseq::intrinsic("pinctrl_apply")]` tag on
    /// a method whose receiver is neither the `PinCtrl` resource shell nor
    /// `Option<PinCtrl>` (C06 / D14) — a spoofed pinctrl op (the `apply`
    /// intrinsic is compiler-managed on the `PinCtrl` / `Option<PinCtrl>` facades
    /// only). Mirrors clock's `CLK-ERR-SPOOFED-RECEIVER`.
    #[error(
        "PIN-ERR-SPOOFED-RECEIVER: method `{method}` is tagged a `pinctrl_apply` intrinsic but its \
         receiver is `{receiver}`, not `PinCtrl`/`Option<PinCtrl>` (the apply op exists only on the \
         pinctrl facade)"
    )]
    SpoofedReceiver {
        /// The offending method name.
        method: String,
        /// The receiver type the method is declared on.
        receiver: String,
    },
}

/// Per-construct timed-cpu-delay-and-sleep (TCD) frontend diagnostics (GRILL
/// C03/C05/D11). Carried by [`BridgeError::TimedCpuDelayValidation`].
///
/// The first slice mints `TCD-ERR-02` (intrinsic argument-type mismatch). It is
/// largely subsumed by Rust's own typechecking of the typed
/// `regseq::delay::{busy_wait,sleep}` facade (a mistyped *call* fails before
/// the bridge runs); the residual reachable case this catches is a
/// **mis-declared** facade — a function tagged `#[rseq::intrinsic("busy_wait"/
/// "sleep")]` whose parameter is not a `Duration` — mirroring device-interrupt's
/// `IRQ-ERR-ISR-SIGNATURE`. `TCD-ERR-01` (sleep-in-IRQ-context, C05) is minted
/// here when the frontend context check lands (M3/T10).
#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum TimedCpuDelayDiagnostic {
    /// `TCD-ERR-01`: a sleeping intrinsic (`regseq::delay::sleep`) called within
    /// the marked IRQ-context region (the `#[rseq::isr]` handler body, D05
    /// phase-1). The message is the **verbatim** GRILL C05 format; the
    /// reject-fixture test matches it exactly. `busy_wait` is never rejected on
    /// context grounds (I07).
    #[error(
        "TCD / Timed CPU Delay And Sleep: '{intrinsic}' is not legal in IRQ context \
         (call site reachable from {context_kind} per the callback-context model); \
         use 'regseq::delay::busy_wait' instead"
    )]
    SleepInIsr {
        /// The sleeping intrinsic's facade path (`regseq::delay::sleep`).
        intrinsic: String,
        /// The context-marking source token the call site is reachable from
        /// (`rseq::isr`), matching the device-interrupt marking's public shape.
        context_kind: String,
    },
    /// `TCD-ERR-02`: a recognized `busy_wait`/`sleep` intrinsic facade whose
    /// signature is not the contractual single by-value `regseq::Duration`
    /// argument (R13/D11; C03).
    #[error(
        "TCD-ERR-02 / Timed CPU Delay And Sleep: timed-delay intrinsic `{intrinsic}` must take \
         exactly one by-value `regseq::Duration` argument ({detail})"
    )]
    ArgTypeMismatch {
        /// The offending intrinsic's facade path (`regseq::delay::busy_wait`
        /// or `regseq::delay::sleep`).
        intrinsic: String,
        /// What is wrong with the declared signature.
        detail: String,
    },
}

/// Per-construct synchronization validation diagnostics (synchronization
/// construct, contract C03). Carried by
/// [`BridgeError::SynchronizationValidation`]. The §7 context checks (§7.1
/// ISR-legality, §7.2 wait-context, may-sleep-while-held, guard escape /
/// use-after-consume) land in M3; M2 carries only the deferred-surface
/// rejection below.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum SyncDiagnostic {
    /// A `Completion` field declared `#[rseq::sync(cardinality = AllWaiters)]`.
    /// `AllWaiters` is deferred — the Zephyr companion-condvar backing state is
    /// underspecified and there is no corpus witness (GRILL C01/C02); only
    /// `SingleWaiter` (the default) is in the first slice.
    #[error(
        "synchronization: `cardinality = AllWaiters` is not supported in the first slice \
         (only `SingleWaiter`); AllWaiters is deferred (no witness; Zephyr companion-condvar \
         underspecified) on `Completion` field `{field}`"
    )]
    AllWaitersUnsupported {
        /// The offending `Completion` field name.
        field: String,
    },
    /// A `Completion::wait(Duration)` whose `Result<(), RegseqError>` is
    /// CONSUMED (matched / `?`-propagated / bound and read), not discarded. The
    /// first slice does not model the timeout-expiry → `Err` outcome (design
    /// T07-lowering §D4): the wait result is discarded, so a body that branches
    /// on it would read an unset destination — rejected loud rather than emitting
    /// wrong code (I06). Bind it to `_` until the timeout→`Result` lowering lands.
    #[error(
        "synchronization: `Completion::wait` result is consumed on field `{field}`, but the \
         first slice discards the wait result (the timeout→`Result` outcome is not modeled yet); \
         bind it to `_`"
    )]
    CompletionWaitResultConsumed {
        /// The `Completion` field whose `wait` result is consumed.
        field: String,
    },
    /// `#[rseq::sync(cardinality = <X>)]` with an unrecognized value `<X>` — a
    /// typo would otherwise silently bind the conservative `SingleWaiter`
    /// default and mis-state the author's intent. Only `SingleWaiter` is in the
    /// first slice (`AllWaiters` is the separately-diagnosed deferred case).
    #[error(
        "synchronization: unknown `cardinality = {value}` on `Completion` field `{field}` \
         (only `SingleWaiter` is supported in the first slice)"
    )]
    UnknownCardinality {
        /// The offending `Completion` field name.
        field: String,
        /// The unrecognized cardinality token.
        value: String,
    },

    /// `forbidden-isr-blocking-op` (§7.1 ISR-legality + §7.2 wait-context, M3
    /// T11): a sleeping / non-IRQ-safe synchronization op in an `#[rseq::isr]`
    /// IRQ-context body — `Mutex::lock` (sleeps), `SpinLock::lock` (thread-only,
    /// no IRQ save/restore), or `Completion::wait` (sleeps). Only
    /// `IrqSpinLock::lock` and `Completion::signal`/`reset` are ISR-safe. The
    /// check consumes device-interrupt's `IsrBinding` marking and is
    /// **sound-but-incomplete** (intraprocedural/lexical, GRILL `I04`): a
    /// transitively-IRQ-reachable illegal acquisition deeper than the lexical
    /// body escapes until the Open-4 hardening — documented in the conformance
    /// output, never presented as "ISR-safety verified".
    #[error(
        "synchronization: `{op}` is forbidden in `#[{context}]` interrupt context on field \
         `{field}` (only IrqSpinLock acquire and Completion signal/reset are ISR-safe)"
    )]
    ForbiddenIsrBlockingOp {
        /// The offending lock/completion field name.
        field: String,
        /// The forbidden op (`Mutex::lock` / `SpinLock::lock` / `Completion::wait`).
        op: String,
        /// The context attribute (`rseq::isr`).
        context: String,
    },

    /// `sleep-in-atomic-section` (may-sleep-while-held, M3 T12): a may-sleep call
    /// — `regseq::delay::sleep`, `Mutex::lock`, or `Completion::wait` — inside a
    /// `SpinLock`/`IrqSpinLock` held region (the produced `SpinlockHeldRegion`
    /// atomic-section marking). Sleeping while holding a spinlock self-deadlocks.
    /// The may-sleep predicate agrees with timed-delay's. Intraprocedural/lexical
    /// (`I04`).
    #[error(
        "synchronization: may-sleep op `{op}` occurs while the spinlock `{lock_field}` is held \
         (sleeping in a spinlock-held atomic section self-deadlocks)"
    )]
    SleepInAtomicSection {
        /// The may-sleep op (`regseq::delay::sleep` / `Mutex::lock` / `Completion::wait`).
        op: String,
        /// The held spinlock field whose atomic section the op violates.
        lock_field: String,
    },

    /// `forbidden-try-lock-or-unlock` (M3 T12). The reachable form in the first
    /// slice is a **recursive same-lock acquire**: `lock(&self)` takes a shared
    /// borrow, so `let g1 = self.m.lock(); let g2 = self.m.lock();` compiles yet
    /// re-acquires a non-recursive lock while already held — detected statically
    /// as an acquire whose block lies inside the same lock's held region (a clean
    /// consumer of the T10 marking), so the deadlock is rejected at the bridge
    /// rather than lowered to a real OS lock. (The `try_lock` / standalone
    /// `unlock` halves of this catalog entry are unrepresentable in the
    /// first-slice surface — there is no such method — so they are enforced by
    /// the Rust type system, proven by negative fixtures.)
    #[error("synchronization: forbidden lock op on field `{field}`: {detail}")]
    ForbiddenTryLockOrUnlock {
        /// The offending lock field name.
        field: String,
        /// What is wrong (e.g. the recursive-acquire explanation).
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
    // Per regseq-driver-74 Change B: instantiate source bucket-1
    // Function items that rustc's mono-collector interned (mentioned
    // items) but no translated body reached, so they would otherwise
    // trip the Phase-5 `ItemMissingInstance` ratchet. Runs after all
    // reactive instances exist (so item IDs are final and the
    // zero-instance test is accurate) and before the F4-2 sort +
    // completeness check (so the newly created `body: None` instances
    // are ordered and validated like any other).
    instantiate_orphaned_collected_functions(&mut ctxt)?;
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

    // Per regseq-driver-82: a non-generic Function whose identity
    // signature carries an RPIT `Alias(Opaque)` is eagerly flagged
    // `signature_non_rigid` (bucket-4 carve-out) by
    // `construct_function_instances`, which runs before any body is
    // materialised. If such a function is ALSO called, Phase-5 body
    // materialisation reveals the opaque (body-context scope) and
    // creates a real reactive Instance for it. The carve-out flag is
    // then stale: the item is an instantiated bucket-1 Function, not a
    // zero-Instance bucket-4 carve-out. Clear the flag for any flagged
    // Function that ended up with at least one Instance so the
    // `Bucket4HasInstance` invariant holds. An UNCALLED RPIT function
    // (RJ-009's `opaque_adder`) is never reactively instantiated, so
    // its flag and zero-Instance status both stand -- the carve-out
    // (and the `SignatureNonRigid` coverage predicate) is preserved.
    reconcile_revealed_signature_flags(&mut ctxt);

    // Per regseq-backends-24 (IT-14): classify opaque-handle eligibility now that
    // the whole-model use graph (every `Ty`, signature, instance, const) is final.
    // `Item.opaque_handle_eligible` is constructed `false` at every ADT-item site;
    // this post-pass sets it `true` for a non-generic ZST `#[repr(C)]` struct that
    // carries the `[u8; 0]` opaque-handle marker (gc section 2) and is reachable
    // exclusively behind a pointer. The backend then emits such a struct
    // forward-decl-only (no definition / layout asserts).
    opaque_handle::classify_opaque_handle_eligibility(&mut ctxt.model);

    // Per regseq-driver-55 Change G (spec.md section 3.4 Phase 5 +
    // section 8.1 `ItemKind` mirrors `InstanceKind`): after the final
    // F4-2 Instance sort, assert every source-derived non-foreign
    // Item carries at least one covering Instance. This is the
    // bridge-completeness ratchet -- a separate entry point from the
    // model-correctness `RegseqModel::validate` (which the
    // partial-model validator corpus deliberately keeps Instance-free
    // to negative-test other invariants; see
    // `RegseqModel::validate_bridge_completeness` for the rationale).
    ctxt.model
        .validate_bridge_completeness()
        .map_err(|violation| {
            BridgeError::Phase5(format!(
                "bridge-completeness validation failed: {violation:?}"
            ))
        })?;

    Ok(ctxt.model)
}

/// Per regseq-driver-82: clear the `signature_non_rigid` carve-out
/// flag on any Function Item that ended Phase 5 with at least one
/// primary (`Item`-flavored) Instance.
///
/// The flag is set eagerly by `construct_function_instances` when a
/// non-generic Function's identity signature rejects `NonRigidTy`
/// (an RPIT `Alias(Opaque)` return). That eager pass runs before any
/// body is materialised, so it cannot know whether the function is
/// reachable. A reachable (called) such function is reactively
/// instantiated during Phase-5 body materialisation, where the
/// body-context opaque reveal makes its signature translatable. Once
/// the primary Instance exists the carve-out no longer applies: the
/// Item is a normal instantiated bucket-1 Function. Without this
/// reconciliation the stale flag trips the `Bucket4HasInstance`
/// invariant (a bucket-4 Item must have zero Instances).
///
/// An uncalled RPIT function (e.g. RJ-009's private `opaque_adder`) is
/// never reactively instantiated, so its instance count stays zero and
/// its flag is left intact -- the bucket-4 carve-out and the
/// `SignatureNonRigid` coverage predicate are preserved.
///
/// The predicate counts ONLY primary (`Item`-flavored) Function
/// instances, mirroring the validator's `primary_function_counts`
/// (model.rs:3617-3628). This is load-bearing: bucket-1 validation
/// bounds the primary count alone and PERMITS a Function carrying only
/// auxiliary shim-flavored instances (no primary). Counting all
/// instances would clear the carve-out for such a shim-only item too,
/// silently moving it to bucket-1 instead of leaving the (then-correct)
/// `Bucket4HasInstance` rejection in place for a non-rigid-signature
/// item that never gained its covering primary Instance.
fn reconcile_revealed_signature_flags(ctxt: &mut BridgeCtxt<'_>) {
    // Count primary (`Item`-flavored) Function instances per Item,
    // mirroring the validator's `primary_function_counts`
    // (model.rs:3617-3628).
    let mut primary_function_counts: Vec<usize> = vec![0; ctxt.model.items.len()];
    for instance in ctxt.model.instances.iter() {
        let idx = instance.item.index();
        if idx < primary_function_counts.len()
            && matches!(
                &instance.kind,
                regseq_model::InstanceKind::Function(data)
                    if data.flavor == FunctionInstanceFlavor::Item
            )
        {
            primary_function_counts[idx] += 1;
        }
    }
    for (item_id, flags) in ctxt.model.item_bridge_flags.iter_mut() {
        let idx = item_id.index();
        if flags.signature_non_rigid
            && idx < primary_function_counts.len()
            && primary_function_counts[idx] > 0
        {
            flags.signature_non_rigid = false;
        }
    }
}

/// Per regseq-driver-74 Change B: give an `Item`-flavored Function
/// instance to every source bucket-1 Function item that rustc's
/// mono-collector interned but no translated body reached.
///
/// rustc collects mentioned items from pre-optimization MIR
/// (`collect_and_partition_mono_items`), so e.g. a tuple `.clone()`'s
/// field-type library `Clone` impl (`<u32 as Clone>::clone`) is a
/// genuinely emitted mono-item that the bridge interns via the Phase-2
/// `MonoItem::Fn` walk. But the bridge only instantiates the items a
/// translated body reaches, and the field clone's sole call edge lives
/// in the tuple CloneShim body, which the bridge leaves absent and
/// which rustc has anyway optimized to a plain field copy. The item is
/// therefore interned-but-orphaned and trips the Phase-5
/// `ItemMissingInstance` ratchet.
///
/// The targeting predicate mirrors `validate_bridge_completeness`'s
/// bucket-1 classification exactly (`regseq-model/src/model.rs`
/// `:3631-3706`): non-synthetic, non-foreign, non-generic, sig-rigid
/// `ItemKind::Function` with zero instances. Each orphan is
/// instantiated by calling the reactive helper
/// `body::terminator::lookup_or_create_direct_instance` with the same
/// flavor and `callsite_fn_sig` closure the reactive direct-call path
/// uses, so the resulting `(item, args_key, Item)` key and signature
/// are indistinguishable from what a real caller would have produced
/// and the R1 `validate_unique_instance_keys` backstop sees no
/// duplicate.
///
/// This is ~zero-regression by construction: a zero-instance bucket-1
/// source item already fails the build today, so the pass is a no-op
/// on every currently-passing run and can only fire where the build
/// currently fails.
///
/// Stash-miss handling: an orphan absent from
/// `collected_item_resolutions` is a separate pre-existing orphan
/// class (e.g. an uncalled local item eagerly interned but never
/// mono-collected) that is out of regseq-driver-74 scope and already
/// fails the build; it is surfaced as a hard `BridgeError::Phase5`
/// rather than silently skipped.
fn instantiate_orphaned_collected_functions(ctxt: &mut BridgeCtxt<'_>) -> Result<(), BridgeError> {
    // Count instances per item, mirroring the validator's
    // `instance_counts` (model.rs:3618-3629).
    let mut instance_counts: Vec<usize> = vec![0; ctxt.model.items.len()];
    for instance in ctxt.model.instances.iter() {
        let idx = instance.item.index();
        if idx < instance_counts.len() {
            instance_counts[idx] += 1;
        }
    }

    // Collect orphan ItemIds before touching the model: translating an
    // orphan's FnDef can intern fresh nominal items, so snapshot the
    // target set first to avoid observing items appended mid-pass.
    let mut orphans: Vec<ItemId> = Vec::new();
    for (idx, item) in ctxt.model.items.iter().enumerate() {
        let item_id = ItemId::from_u32(idx as u32);
        // Synthetic Items are owned by the synth passes, not this
        // ratchet (model.rs:3636).
        if !matches!(
            ctxt.model.item_synthetic_kinds[item_id],
            SyntheticItemKind::None
        ) {
            continue;
        }
        // Only Function items are reactively orphaned this way; the
        // other source kinds get their Instance in
        // `construct_instances_and_resolve_cross_references`.
        if !matches!(item.kind, ItemKind::Function) {
            continue;
        }
        // Bucket 2 (foreign): excluded (model.rs:3645).
        if item.linkage == regseq_model::Linkage::Foreign {
            continue;
        }
        let flags = ctxt
            .model
            .item_bridge_flags
            .get(&item_id)
            .copied()
            .unwrap_or_default();
        // Bucket 3 (generic): excluded (model.rs:3685).
        if flags.is_generic {
            continue;
        }
        // Bucket 4 (sig-non-rigid Function): excluded (model.rs:3689).
        if flags.signature_non_rigid {
            continue;
        }
        // Bucket 1 orphan: zero instances reference it.
        if instance_counts[idx] != 0 {
            continue;
        }
        orphans.push(item_id);
    }

    for item_id in orphans {
        let def_path_hash = ctxt.model.item_def_paths[item_id];
        let (def_id, rustc_args) = ctxt
            .collected_item_resolutions
            .get(&def_path_hash)
            .copied()
            .ok_or_else(|| {
                let rendered = ctxt
                    .model
                    .strings
                    .resolve(ctxt.model.items[item_id].def_path.rendered);
                BridgeError::Phase5(format!(
                    "orphaned source bucket-1 Function item {} ({}) has no \
                         collection-time resolution: it was interned without being \
                         mono-collected (a separate pre-existing orphan class outside \
                         regseq-driver-74 scope)",
                    item_id.as_u32(),
                    rendered,
                ))
            })?;

        // Translate the FnDef ty to model args via the canonical path
        // (ty.rs:258), exactly as the reactive direct-call path does
        // (terminator.rs:620-621), so the model args match.
        let callee_ty = rustc_middle::ty::Ty::new_fn_def(ctxt.input.tcx, def_id, rustc_args);
        let model_ty = ty::translate_ty(ctxt, callee_ty)?;
        let Ty::FnDef(callee_item, model_args) = ctxt.model.types.resolve(model_ty).clone() else {
            return Err(BridgeError::Phase5(format!(
                "orphan instantiation: translate_ty did not yield Ty::FnDef for item {}",
                item_id.as_u32(),
            )));
        };

        // Create the instance with the same helper, flavor, virtual_idx
        // (None), and sig closure the reactive direct-call path uses
        // (terminator.rs:648-655), so the instance is identical to what
        // a real caller would have produced.
        let _ = body::terminator::lookup_or_create_direct_instance(
            ctxt,
            callee_item,
            model_args,
            FunctionInstanceFlavor::Item,
            None,
            |cx| body::terminator::callsite_fn_sig(cx, def_id, rustc_args),
        )?;
    }
    Ok(())
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
    // Per regseq-driver-55 Change A (spec.md section 8.0 + section
    // 9.4): `MonoItem::Static(def_id)` interns an `ItemKind::Static`
    // Item alongside the existing `ItemKind::Function` interning so
    // the Phase-5 `construct_source_static_instances` pass has a
    // source Item to attach the `InstanceKind::Static` to.
    // `MonoItem::GlobalAsm` is out of scope (spec.md section 15.1
    // rejects it at use sites).
    for mono_item in &discovered_mono_items {
        match mono_item {
            MonoItem::Fn(instance) => {
                let rustc_middle::ty::InstanceKind::Item(def_id) = instance.def else {
                    continue;
                };
                if !is_source_derived_function(ctxt.input.tcx, def_id) {
                    continue;
                }
                // Per regseq-driver-92: the `core` `Try::branch` /
                // `FromResidual::from_residual` methods are mono-collected
                // from the pre-normalization `?` MIR, but the normalizer
                // rewrites every `?` cluster away before translation, so
                // they never appear in a translated body. Interning them
                // here would leave zero-instance bucket-1 orphans whose
                // `Try::Residual` projection signature the orphan-
                // instantiation pass cannot translate (`NonRigidTy`).
                // Skip them; the normalizer is their sole consumer.
                if body::question_mark::is_question_mark_consumed_lang_item(ctxt.input.tcx, def_id)
                {
                    continue;
                }
                let _ = ty::intern_item(ctxt, def_id, ItemKind::Function, ty::SpecialAdt::None);
                // Per regseq-driver-74 Change A: stash the resolved
                // rustc `(DefId, args)` for this collected Item fn,
                // keyed by the model `DefPathHash`. The orphan
                // post-pass (Change B) reads it to instantiate items
                // rustc collected (mentioned-items) but no translated
                // body reaches, recreating the instance identically to
                // the reactive direct-call path. The resolution is
                // already in hand here; only the capture is new.
                let key = def_path_hash(ctxt.input.tcx, def_id);
                ctxt.collected_item_resolutions
                    .insert(key, (def_id, instance.args));
            }
            MonoItem::Static(def_id) => {
                let mutability = source_static_mutability(ctxt.input.tcx, *def_id);
                let _ = ty::intern_item(
                    ctxt,
                    *def_id,
                    ItemKind::Static { mutability },
                    ty::SpecialAdt::None,
                );
            }
            MonoItem::GlobalAsm(_) => continue,
        }
    }

    // Per regseq-driver-55 Change B (spec.md section 8.1, esp. the
    // associated-constant lifting rule): `const` items are NOT
    // surfaced by rustc's mono-item discovery (they are erased at
    // codegen time), so a separate HIR walk interns every top-level
    // `const` AND associated `const` as an `ItemKind::Const` Item.
    // Both forms share the same `ItemKind::Const` shape per section
    // 8.1; `intern_item` dedups by `DefPathHash` so the two walks
    // into the Item table are idempotent by construction.
    let tcx = ctxt.input.tcx;
    let const_def_ids: Vec<DefId> = tcx
        .hir_crate_items(())
        .definitions()
        .filter(|local_def_id| {
            matches!(
                tcx.def_kind(local_def_id.to_def_id()),
                rustc_hir::def::DefKind::Const | rustc_hir::def::DefKind::AssocConst
            )
        })
        .map(|local_def_id| local_def_id.to_def_id())
        .collect();
    for def_id in const_def_ids {
        let _ = ty::intern_item(ctxt, def_id, ItemKind::Const, ty::SpecialAdt::None);
    }

    // Per regseq-driver-58 Changes A, B, C (spec.md section 8.0
    // closed-alphabet ItemKind + section 8.1 impl-lifting): the
    // bridge enumerates every source-defined top-level item EAGERLY
    // via a single HIR walk, regardless of reachability through
    // rustc's mono-item collector. This closes the asymmetry
    // surfaced by manual playground stress-testing where source
    // structs / inherent methods / trait default bodies (which are
    // not reached by `MonoItem::Fn` when nobody calls them and which
    // have no `Ty::Adt(_)` row when nobody references the type) were
    // missing from `model.items`. Foreign decls (Change B) are
    // routed through a separate `foreign_items()` iterator because
    // they have no body owners and `source_item_def_id` cannot find
    // them via `hir_body_owners`.
    eager_hir_enumeration(ctxt)?;

    for mono_item in discovered_mono_items {
        let MonoItem::Fn(instance) = mono_item else {
            continue;
        };
        if instance.def_id().as_local().is_none() {
            continue;
        }
        // Per regseq-driver-62: fetch the polymorphic MIR via
        // `tcx.instance_mir(instance.def)` (matching the regseq-driver-60
        // body materializer at `bridge/body/materialise.rs:287-293`),
        // then substitute `instance.args` so the local_decls /
        // statements / terminators presented to the scan functions are
        // monomorphized. Without the substitution, trait-method call
        // sites whose `Self` is `TyKind::Param(_)` in the polymorphic
        // body trip `canonicalize_call_site_callee` ->
        // `Instance::try_resolve` -> `Ok(None)` -> `BridgeError::Phase4`
        // at `bridge/mod.rs:1042-1046`. `instance_mir` returns the
        // right body for each `InstanceKind` (source body for Item,
        // shim body for DropGlue / VTableShim / etc.), and the
        // substitution is a natural no-op when `instance.args` is the
        // identity, so no defensive branch is needed.
        let polymorphic_body = ctxt.input.tcx.instance_mir(instance.def);
        let mut body = instance.instantiate_mir_and_normalize_erasing_regions(
            ctxt.input.tcx,
            rustc_middle::ty::TypingEnv::fully_monomorphized(),
            rustc_middle::ty::EarlyBinder::bind(polymorphic_body.clone()),
        );
        // Per regseq-driver-92: normalize same-type `Result`-`?` clusters
        // before the synthesis scan walks this body, mirroring the
        // Phase-5 materialiser. The scan resolves each call's callee type
        // (`scan_call_ty_for_vtable`, line below) and every local decl;
        // left intact, the `branch` / `from_residual` callee signatures
        // would hit the `Try::Residual` projection here -- earlier than
        // Phase-5 -- and the `ControlFlow` / `Infallible` locals would be
        // interned. Normalizing first removes both. A no-op for `?`-free
        // bodies. The same scope gate fails loud here for out-of-scope
        // `?` (cross-type / `Option` / `needs_drop`-`Ok`).
        body::question_mark::normalize_question_mark(ctxt.input.tcx, &mut body)?;
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

/// Per regseq-driver-58 Changes A, B, C: enumerate every source-
/// defined top-level item exhaustively from `tcx.hir_crate_items`,
/// regardless of mono-item reachability. Covers ADTs (Struct / Enum
/// / Union), source-defined functions (free fns + inherent / trait-
/// impl methods + default trait method bodies), and foreign decls
/// (`extern "C" { ... }` Fn / Static). `DefKind::Closure` is OUT
/// OF SCOPE -- closures are handled by
/// `synth::closure::synthesise_closure_for_def_id` triggered from
/// the body-scan loop, and re-interning them here would race with
/// the synth path. Trait declarations and associated types are
/// erased per spec.md section 8.1 (`spec.md:2346-2350`); the
/// `is_mir_available` predicate at the `AssocFn` site naturally
/// filters out trait method declarations without bodies.
fn eager_hir_enumeration<'tcx>(ctxt: &mut BridgeCtxt<'tcx>) -> Result<(), BridgeError> {
    use regseq_model::BridgeItemFlags;
    use rustc_hir::def::DefKind;

    let tcx = ctxt.input.tcx;
    let crate_items = tcx.hir_crate_items(());

    // Eager HIR walk for source-defined ADTs and source-defined
    // free / associated functions. Statics are pre-interned via
    // the MonoItem::Static walk; consts via the existing HIR walk
    // above. Foreign decls are routed through `foreign_items()`
    // below because they have no body owners.
    let source_def_ids: Vec<DefId> = crate_items
        .definitions()
        .map(|local_def_id| local_def_id.to_def_id())
        .collect();
    for def_id in source_def_ids {
        if tcx.is_foreign_item(def_id) {
            continue;
        }
        let item_kind = match tcx.def_kind(def_id) {
            DefKind::Struct => ItemKind::Struct,
            DefKind::Enum => ItemKind::Enum,
            DefKind::Union => ItemKind::Union,
            DefKind::Static { mutability, .. } => ItemKind::Static {
                mutability: match mutability {
                    rustc_ast::Mutability::Mut => regseq_model::Mutability::Mut,
                    rustc_ast::Mutability::Not => regseq_model::Mutability::Not,
                },
            },
            DefKind::Fn => ItemKind::Function,
            DefKind::AssocFn => {
                // Per regseq-driver-58 Change C: `DefKind::AssocFn`
                // covers inherent and trait-impl methods (which
                // always have bodies) and trait declarations (which
                // may or may not have a default body). Only the
                // body-bearing cases surface as Items; the
                // `is_mir_available` predicate distinguishes them
                // per spec.md regseq-driver-58 Change C text and the
                // `tcx.is_foreign_item(did) && !tcx.is_mir_available(did)`
                // distinction at spec.md section 9.4. Trait method
                // declarations without bodies do NOT surface (Change H
                // spec amendment).
                if !tcx.is_mir_available(def_id) {
                    continue;
                }
                ItemKind::Function
            }
            // All other DefKinds (Mod, Trait, TyAlias, ForeignTy,
            // ForeignMod, AnonConst, InlineConst, OpaqueTy, Variant,
            // Ctor, Closure, AssocTy, TyParam, ConstParam, LifetimeParam,
            // Use, ExternCrate, GlobalAsm, Macro, Const, AssocConst,
            // Impl, TraitAlias, etc.) are intentionally skipped: they
            // either fall outside the closed six-variant ItemKind
            // alphabet (per spec.md section 8.0 `spec.md:2205-2230`),
            // are handled by a sibling walker (consts via the eager
            // const walk above; closures via the body-scan synth
            // trigger), or are erased per spec.md section 8.1
            // (`spec.md:2346-2350`).
            _ => continue,
        };
        // Per spec.md regseq-driver-58 Change A/C step 1 + regseq-driver-60
        // Change H: bucket classification is computed by the shared
        // `ty::source_bridge_item_flags` helper so this eager intern site and
        // the lazy `intern_source_item_handle` push cannot drift. The helper
        // uses `generics_of(def_id).count() > 0` uniformly, so a trait
        // default method is bucket 3 (generic over its `Self` param), not
        // bucket 1; see its doc comment.
        let flags = ty::source_bridge_item_flags(tcx, def_id);
        let _ = ty::intern_eager_hir_item(
            ctxt,
            def_id,
            item_kind,
            regseq_model::Linkage::Default,
            flags,
        );
    }

    // Per regseq-driver-58 Change B: foreign decls live in
    // `foreign_items()`, NOT in `body_owners()`. The Phase-5
    // `construct_function_instances` / `construct_source_static_instances`
    // passes will push the corresponding `body: None` / `init: None`
    // Instances per spec.md section 9.4 (`spec.md:2642-2648`,
    // `:2668-2677`).
    for foreign_item_id in crate_items.foreign_items() {
        let def_id = foreign_item_id.owner_id.def_id.to_def_id();
        let item_kind = match tcx.def_kind(def_id) {
            DefKind::Fn => ItemKind::Function,
            DefKind::Static { mutability, .. } => ItemKind::Static {
                mutability: match mutability {
                    rustc_ast::Mutability::Mut => regseq_model::Mutability::Mut,
                    rustc_ast::Mutability::Not => regseq_model::Mutability::Not,
                },
            },
            // `DefKind::ForeignTy` (`extern type Foo;`) is out of
            // scope per regseq-driver-58 Non-Goals (foreign-decl
            // visibility semantics beyond `#[foreign]` attribute
            // carriage are excluded).
            _ => continue,
        };
        // Foreign Items can be generic in principle (rustc allows
        // generic foreign fns under unstable flags), but the
        // four-bucket policy treats every foreign Item as bucket 2
        // regardless. The flag is still tracked for completeness.
        // Use `count()` not `own_params` per Change A/C step 1 so
        // generics inherited from an enclosing scope are reflected.
        let is_generic = tcx.generics_of(def_id).count() > 0;
        let flags = BridgeItemFlags {
            is_generic,
            signature_non_rigid: false,
        };
        let _ = ty::intern_eager_hir_item(
            ctxt,
            def_id,
            item_kind,
            regseq_model::Linkage::Foreign,
            flags,
        );
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
            if let TyKind::FnDef(def_id, generic_args) = constant.const_.ty().kind() {
                // Audit finding regseq-driver-42-4: pre-translate
                // every constant FnDef callee so the Phase-5
                // `translate_call_callee` path resolves the callee
                // Item through a real (post-Phase-4) ItemId rather
                // than a Phase-2 placeholder. Reaching translate_ty
                // here interns the source FnDef Item in
                // `cx.pending_items` so Phase-3/4 binds it before
                // body materialisation runs.
                //
                // Per regseq-driver-56 spec section 8.1 the callee
                // is canonicalized through `Instance::try_resolve`
                // BEFORE interning so static-dispatch trait-method
                // calls pre-intern the resolved impl-method DefId
                // rather than the trait-declaration DefId.
                let canonical =
                    canonicalize_call_site_callee(ctxt.input.tcx, *def_id, generic_args).map_err(
                        |err| {
                            BridgeError::Phase4(format!(
                        "call-site Instance::try_resolve failed for {:?} with args {:?}: {err}",
                        *def_id, generic_args,
                    ))
                        },
                    )?;
                // regseq-driver-77: a fn-pointer `Fn*::call` resolves to
                // `InstanceKind::FnPtrShim`. The Phase-5 rewrite
                // (`rewrite_fn_ptr_call_shim`) lowers it to a
                // `CallCallee::Indirect`, so the FnPtrShim callee Item
                // (`core::ops::Fn::call`) is never materialised into an
                // Instance; pre-interning it here would orphan a bucket-1
                // Function with zero Instances and trip the Phase-5
                // completeness check (`model.rs:3599-3607`). Skip ONLY
                // the callee intern -- the arg scans below
                // (`scan_call_ty_for_vtable` / `scan_operand_move_for_drop_glue`)
                // must still run for fn-ptr calls whose args carry
                // vtable / drop-glue needs.
                if !matches!(canonical.flavor, FunctionInstanceFlavor::FnPtrShim) {
                    let _ = ty::translate_ty(ctxt, canonical.callee_ty);
                    if intrinsic::intrinsic_kind_for_def_id(
                        ctxt.input.tcx,
                        canonical.callee_def_id,
                    )
                    .is_some()
                    {
                        let model_ty = ty::translate_ty(ctxt, canonical.callee_ty)?;
                        if let Ty::FnDef(callee_item, _) =
                            ctxt.model.types.resolve(model_ty).clone()
                        {
                            // Per audit-finding F4 source-derived Items
                            // carry placeholder ItemIds during Phase 2;
                            // look the callee's DefPath up through
                            // `pending_items` when the ItemId hasn't yet
                            // been bound to `model.items`.
                            let callee_def_path = ctxt
                                .lookup_item_def_path(callee_item)
                                .expect("FnDef callee Item must be interned")
                                .clone();
                            intrinsic::recognise_intrinsic_def_id(
                                ctxt,
                                ItemHandle::from_u32(callee_item.as_u32()),
                                &callee_def_path,
                                canonical.callee_def_id,
                            )?;
                        }
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
    // Per regseq-driver-55 Change F (spec.md section 3.4 Phase 5 +
    // section 8.1): source `static` / `const` Items get their
    // symmetric Instance rows constructed here, between the
    // Function-instance pass and the closure-family pass. Their
    // parent Items are interned in Phase 2 (Change A's
    // `MonoItem::Static` walk; Change B's `hir_crate_items` const
    // walk) and assigned final `ItemId`s by Phase 4, so
    // `source_item_def_id` resolves them and the pushed
    // `Instance.item` is a valid index that the closing
    // `sort_items_by_f4` remap (and the final
    // `sort_instances_by_f4_2`) carry through correctly.
    //
    // INTERPRETATION CHOICE (regseq-driver-55): Change F's
    // pseudocode also slots `construct_nominal_adt_instances` here,
    // on the premise (Change E step 3) that "Phase-2's
    // mono-item-driven type translation populates one Ty entry per
    // reachable (item, args) pair" with a resolvable `ItemId`. That
    // premise does not hold in this driver: source `struct` / `enum`
    // / `union` Items are NOT mono-items and are first interned
    // during Phase-5 fn-sig / body translation (after Phase 4), so
    // their `Ty::Struct/Enum/Union` entries carry the high-end
    // placeholder `ItemId` (per `placeholder_item_id_for_handle`)
    // until `assign_late_pending_items` + `patch_resolved_source_ty_pending`
    // run at the END of this orchestrator. Reading the Ty's `item`
    // field at this slot yields an unpatched placeholder, and
    // pushing `Instance.item = <placeholder>` would panic the
    // closing `sort_items_by_f4` remap (`remap[item.index()]`
    // indexes past `items.len()`). Additionally, running before
    // `eliminate_diag_log_peel_residue` would push an
    // `InstanceKind::Enum` for the DiagLogPeel residue `LogArg`
    // enum, pinning it in the model against the spec.md section
    // 26.6 contract. The spec-INTENT (one Instance per reachable
    // monomorphisation via the `model.types` walk, no rustc DefId
    // resolution) is preserved verbatim by running the SAME pass as
    // the final construction step (after every ItemId/TyId
    // finalisation barrier AND the residue DCE) -- mirroring the
    // existing `const_::construct_const_memory_static_instances`
    // precedent for late-resolved Instances. Only the slot moves;
    // the algorithm is unchanged. See the call site at the end of
    // this function for the full rationale.
    construct_source_static_instances(ctxt)?;
    construct_source_const_instances(ctxt)?;
    // Per regseq-driver-51 (spec.md:4531-4546 + 778-781): bridge-
    // synthesised ClosureCall Function Items have no rustc DefId, so
    // `construct_function_instances` (`mod.rs:1114-1121`) skips them.
    // This parallel walk constructs one `Function` `Instance` per
    // synthesised ClosureCall Item BEFORE `run_phase5_body_passes` so
    // the Phase-5 materialiser at `body/materialise.rs:303-338` can
    // fill in `body` via the existing closure-body resolution at
    // `body/materialise.rs:87-100`.
    construct_closure_call_instances(ctxt)?;
    // Device-lifecycle (T09/T10): override-check each bound driver's callbacks
    // and force-materialize the overriding instances BEFORE the Phase-5 body
    // drain, so those synthesized callbacks get bodies the backend can emit.
    device_lifecycle::resolve_drivers(ctxt)?;
    // Serial device-class (construct #7): recognize each serial-class driver's
    // op-set + capture the support-enum variant maps, force-materializing the op
    // bodies before the drain (like the lifecycle callbacks). Runs after the
    // lifecycle pass (device class known) and before `resolve_isrs` so the
    // serial-class interrupt-handler relaxation (a serial driver's line handler
    // is the compiler-synthesized serial ISR, not an authored `#[rseq::isr]`) is
    // in effect when the interrupt pass validates handler presence.
    serial::resolve_serial(ctxt)?;
    // Device-interrupt (construct #6): discover + validate + bind the
    // #[rseq::isr] handlers and force-materialize their instances before the
    // drain, exactly like the lifecycle callbacks. The pending attribute
    // records are finalised after the late-item barriers below.
    let pending_isr_attrs = interrupt::resolve_isrs(ctxt)?;
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
    // Per regseq-driver-55 Change E (slotted here per the
    // INTERPRETATION CHOICE documented at the Change-F site above).
    // This is the LAST construction step, after every ItemId/TyId
    // finalisation barrier this driver requires:
    //
    //  * `assign_late_pending_items` + `patch_resolved_source_ty_pending`
    //    + `sort_items_by_f4`/`remap_item_ids` have given every
    //    source `struct`/`enum`/`union` and synth `VTableType` Item
    //    a final `ItemId` and patched its `Ty::Struct/Enum/Union`
    //    view (this driver interns source ADT Items late, during
    //    Phase-5 fn-sig / body translation -- NOT in Phase 2 as
    //    Change E step 3 assumes -- so the Ty's `item` field is an
    //    unpatched placeholder until this barrier);
    //  * `eliminate_diag_log_peel_residue` has removed the
    //    DiagLogPeel residue `Ty::Enum { item: <LogArg> }` entry.
    //    Running BEFORE the DCE would push an `InstanceKind::Enum`
    //    for `LogArg`, which keeps the `LogArg` Ty/Item reachable
    //    and violates the spec.md section 26.6 contract that
    //    `LogArg` does NOT appear in RegseqModel.
    //
    // The `model.types` walk is therefore the authoritative,
    // fully-resolved, residue-free monomorphisation enumeration --
    // identical mechanism to Change E step 3, just after the
    // barriers the spec's premise does not account for. The closing
    // `model.sort_instances_by_f4_2()` in `run_bridge` restores
    // F4-2 canonical ordering across the expanded Instance set.
    construct_nominal_adt_instances(ctxt)?;
    // Device-interrupt finalisation (construct #6): every barrier above has
    // given the handler function Items their final `ItemId`s and the drain
    // has materialized their bodies — record the `AttrData::IsrBinding`
    // markers and compute the `IrqResult` arm-reachability facts.
    interrupt::finalise_isrs(ctxt, pending_isr_attrs)?;
    // Timed-cpu-delay-and-sleep frontend context legality (TCD construct,
    // C05/D05): reject a `sleep` intrinsic call lexically within a marked
    // `#[rseq::isr]` handler body. Runs last — after the handler bodies are
    // materialized AND `finalise_isrs` has recorded the `IsrBinding` markers
    // this pass consults.
    timed_delay::validate_isr_no_sleep(&ctxt.model)?;
    // Atomic-memory-access-and-ordering frontend op legality (atomicity
    // construct, C01/C02): the per-op valid-ordering matrix, the const-`Ordering`
    // discipline (D14), and the bit-test element/bounds rule (C07/D08). Runs
    // last — after every body is materialized, so it walks the recognized atomic
    // calls.
    atomic::validate_atomic_ops(&ctxt.model)?;
    // Synchronization op / acquire-site recognition: walk the materialized
    // bodies and record each `lock()` acquire + its paired guard release into
    // the `SynchronizationTables` sibling table. Runs here — after the bodies
    // are materialized and the item sort has re-keyed `synchronization.drivers`
    // (so the lock fields recognized in Phase 4 are looked up under their final
    // `ItemId`s) — the same placement discipline as `validate_isr_no_sleep`.
    synchronization::recognise_acquire_sites(&mut ctxt.model)?;
    // Synchronization context-marking analysis (M3 — the thesis component):
    // produce the spinlock-held atomic-section markings over each spinning-kind
    // guard's (approximated) live range, and run the §7.1 ISR-legality / §7.2
    // wait-context / may-sleep-while-held / recursive-acquire checks over the
    // joined IsrBinding + may-sleep markings. Runs after the acquire sites are
    // recorded — the same placement discipline, and the first pass that both
    // consumes and produces context markings (all intraprocedural/lexical, I04).
    synchronization::analyse_sync_contexts(&mut ctxt.model)?;
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
    use regseq_model::{
        BridgeItemFlags, FunctionInstanceData, Instance, InstanceKind, ItemKind, Linkage,
    };
    // Per regseq-driver-58 Changes E + F: walk every NON-synthetic
    // `ItemKind::Function` Item and push exactly one Function
    // Instance keyed by the four-bucket policy:
    //
    //   * Bucket 1 (non-generic + sig-rigid + non-foreign): translate
    //     the identity-instantiated signature and push an Instance
    //     with `body: None` (body materialisation runs later).
    //   * Bucket 2 (foreign): translate the identity-instantiated
    //     signature and push an Instance with `body: None` /
    //     `flavor: Item` per spec.md section 9.4
    //     (`spec.md:2642-2648`). Foreign Items have no MIR body, so
    //     `body` stays `None` through Phase-5 materialisation as
    //     well.
    //   * Bucket 3 (generic): skip; monomorphisations reached by the
    //     `MonoItem::Fn` walk are interned through
    //     `lookup_or_create_direct_instance`, and unreached generics
    //     stay Item-only.
    //   * Bucket 4 (non-generic + sig-non-rigid): record
    //     `signature_non_rigid = true` on `item_bridge_flags`; push
    //     no Instance. Validator's bucket-4 carve-out keys on this
    //     flag.
    //
    // Synthetic Items (closure call shims, vtable shims, drop glue)
    // are filtered out; their Instance lifecycle is owned by sibling
    // passes (`construct_closure_call_instances`, the synth/vtable
    // passes, etc.).
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
            if !matches!(
                ctxt.model.item_synthetic_kinds[item_id],
                SyntheticItemKind::None
            ) {
                return None;
            }
            Some(item_id)
        })
        .collect();

    for item_id in function_items {
        let linkage = ctxt.model.items[item_id].linkage;
        let is_foreign = linkage == Linkage::Foreign;
        let def_path_hash = ctxt.model.item_def_paths[item_id];
        let def_id = if is_foreign {
            // Per Change B (Codex new issue #1): foreign Items are
            // resolved through `tcx.hir_crate_items().foreign_items()`,
            // NOT `tcx.hir_body_owners()` (foreign decls have no
            // body owners). Per the no-defensive-fallback rule a
            // foreign Function Item interned by the eager HIR walk
            // (Change B) MUST resolve here; an unresolved hash is a
            // bridge-pipeline invariant violation, not a recoverable
            // shape.
            match foreign_item_def_id(ctxt.input.tcx, def_path_hash) {
                Some(def_id) => def_id,
                None => {
                    return Err(BridgeError::Phase5(format!(
                        "foreign ItemKind::Function {item_id:?} (def_path_hash={def_path_hash:?}) \
                         has no rustc DefId via hir_crate_items().foreign_items(); \
                         the eager HIR walk (Change B) must intern only locally-defined \
                         foreign decls"
                    )));
                }
            }
        } else {
            match source_item_def_id(ctxt.input.tcx, def_path_hash) {
                Some(def_id) => def_id,
                // Item without a rustc DefId in this compilation
                // (e.g., cross-crate or shim Items that slipped past
                // the synthetic_kind filter); the call site that
                // needs the Instance will lazy-construct one through
                // `lookup_or_create_direct_instance`.
                None => continue,
            }
        };
        if !is_rustc_function(ctxt.input.tcx, def_id) {
            continue;
        }
        let bridge_flags = ctxt
            .model
            .item_bridge_flags
            .get(&item_id)
            .copied()
            .unwrap_or_default();

        if is_foreign {
            // Bucket 2.
            if duplicate_function_identity_instance(ctxt, item_id) {
                continue;
            }
            let poly_fn_sig = ctxt.input.tcx.fn_sig(def_id).instantiate_identity();
            // Per Change F + the no-defensive-fallback rule: foreign
            // signatures must be rigid (C ABI-bound) so any
            // translation failure here is a real input invariant
            // violation rather than a recoverable shape. Propagate
            // the error so callers see the actual `BridgeError`
            // (UnsupportedCAbiType, UnsupportedAbi, etc.) rather
            // than a silent skip.
            let fn_sig = fnsig::translate_fn_sig(ctxt, poly_fn_sig)?;
            // Per regseq-driver-84: a foreign `body: None` function is an
            // ABI import. Its `emission_name` must be the real link name
            // rustc would call it by (`abs`), NOT the rendered DefPath
            // (`input::{extern#0}::abs`); the latter compiles but fails at
            // link because libc exports `abs`. `foreign_fn_abi_symbol`
            // mirrors rustc's `compute_symbol_name` for foreign items.
            let abi_symbol = foreign_fn_abi_symbol(ctxt.input.tcx, def_id);
            let emission_name = ctxt.model.strings.intern(&abi_symbol);
            let span = ctxt.model.items[item_id].span;
            let flavor = FunctionInstanceFlavor::Item;
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
                    virtual_idx: None,
                }),
            });
            ctxt.function_instance_for_item
                .insert((item_id, args_key, flavor), instance_id);
            continue;
        }

        // Bucket 3: skip generic Items. Monomorphisations reached
        // through the MonoItem walk are handled by
        // `lookup_or_create_direct_instance`; bucket-3 Items with
        // zero monomorphisations stay Item-only.
        if bridge_flags.is_generic {
            continue;
        }

        // Trait default method Items live at the trait's DefPath. They are now
        // bucket 3 (generic over the trait's `Self` param), so the `is_generic`
        // guard above already skips them; this explicit check is a defensive
        // belt-and-suspenders guard. Their identity args include `Self` (and
        // any trait-level generics), so `Instance::try_resolve` with
        // identity_args cannot pin them to a concrete impl. The concrete
        // Function Instance for a resolved receiver is created reactively by
        // Phase 5's `lookup_or_create_direct_instance` at the call site (or,
        // for device-lifecycle callbacks, force-materialized in
        // `device_lifecycle::resolve_drivers`).
        if matches!(
            ctxt.input.tcx.def_kind(def_id),
            rustc_hir::def::DefKind::AssocFn
        ) && ctxt.input.tcx.trait_of_assoc(def_id).is_some()
        {
            continue;
        }

        if duplicate_function_identity_instance(ctxt, item_id) {
            // The reactive `MonoItem::Fn` walk + `lookup_or_create_direct_instance`
            // pre-created an identity-args Instance during Phase 2 /
            // Phase 5 body materialisation; do not push a second
            // Instance for the same `(Item, identity-args,
            // FunctionInstanceFlavor::Item)` tuple.
            continue;
        }

        let identity_args =
            rustc_middle::ty::GenericArgs::identity_for_item(ctxt.input.tcx, def_id);
        let rustc_instance = match rustc_middle::ty::Instance::try_resolve(
            ctxt.input.tcx,
            rustc_middle::ty::TypingEnv::fully_monomorphized(),
            def_id,
            identity_args,
        ) {
            Ok(Some(instance)) => instance,
            Ok(None) | Err(_) => {
                return Err(BridgeError::Phase5(format!(
                    "non-generic non-foreign fn {def_id:?} failed Instance::try_resolve"
                )));
            }
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
        // For a concrete (non-generic) Function, associated-type projections in
        // the signature (e.g. a `BitOr::Output` operator-overload return) are
        // rigid-after-normalization; `translate_fn_sig_monomorphic` resolves
        // them so the instance is created rather than falsely shunted to the
        // bucket-4 signature-non-rigid path. RPIT `Alias(Opaque)` is left
        // intact, preserving that carve-out.
        let fn_sig = match fnsig::translate_fn_sig_monomorphic(ctxt, poly_fn_sig) {
            Ok(sig) => sig,
            Err(BridgeError::NonRigidTy { .. }) => {
                // Bucket 4: record the carve-out flag and push no
                // Instance. The validator's bucket-4 branch (Change
                // G) reads this flag to accept zero-Instance Function
                // Items whose declared signature carries an
                // `Alias(Opaque)` (RPIT) or other non-rigid `TyKind`.
                ctxt.model.item_bridge_flags.insert(
                    item_id,
                    BridgeItemFlags {
                        is_generic: false,
                        signature_non_rigid: true,
                    },
                );
                continue;
            }
            Err(e) => return Err(e),
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

/// True iff `model.instances` already carries a Function Instance
/// for `item_id` with identity `generic_args` (empty vec). Used by
/// `construct_function_instances` to dedupe Bucket-1 / Bucket-2 push
/// against a reactive MonoItem-walk-driven Instance that may already
/// exist for the same Item.
fn duplicate_function_identity_instance(ctxt: &BridgeCtxt<'_>, item_id: ItemId) -> bool {
    use regseq_model::InstanceKind;
    ctxt.model.instances.iter().any(|inst| {
        inst.item == item_id
            && inst.generic_args.is_empty()
            && matches!(inst.kind, InstanceKind::Function(_))
    })
}

/// Per regseq-driver-55 Change C: construct one
/// `InstanceKind::Static { ty, init }` Instance per source-derived
/// `ItemKind::Static` Item. Per spec.md section 8.0 statics cannot
/// be generic, so the relationship is always 1:1 (one Instance per
/// Item, empty `generic_args`).
///
/// Synth statics (`SyntheticItemKind::VTableInstance` /
/// `ConstMemoryStatic`) are filtered out: those carry their own
/// Phase-5 construction passes
/// (`synth/vtable.rs::push_vtable_instance_statics`,
/// `const_::construct_const_memory_static_instances`) that own the
/// init-aggregate shape. Foreign source statics
/// (`extern "C" { static FOO: u32; }`) are out of scope per
/// regseq-driver-55 Non-Goals; `const_::translate_static_init`
/// returns `init: None` for them, but the bridge does not
/// currently surface them as Items.
fn construct_source_static_instances(ctxt: &mut BridgeCtxt<'_>) -> Result<(), BridgeError> {
    use regseq_model::{Instance, InstanceKind, Linkage};

    // Per regseq-driver-58 Change F: this pass owns BOTH source-
    // derived and foreign-derived `ItemKind::Static` Items.
    //
    //   * Source-derived statics push `InstanceKind::Static { ty,
    //     init: Some(_) }` -- `translate_static_init` produces the
    //     initialiser const.
    //   * Foreign statics push `InstanceKind::Static { ty, init:
    //     None }` per spec.md section 9.4 (`spec.md:2668-2677`).
    //
    // Snapshot the target Items before iterating: the construction
    // loop interns `Ty`s / `Const`s and pushes Instances, any of
    // which would invalidate a live borrow of `model.items`.
    let static_items: Vec<ItemId> = ctxt
        .model
        .items
        .iter()
        .enumerate()
        .filter_map(|(idx, item)| {
            let item_id = ItemId::from_u32(idx as u32);
            if !matches!(item.kind, ItemKind::Static { .. }) {
                return None;
            }
            if !matches!(
                ctxt.model.item_synthetic_kinds[item_id],
                SyntheticItemKind::None
            ) {
                return None;
            }
            Some(item_id)
        })
        .collect();

    for item_id in static_items {
        // Skip Items that already carry an `InstanceKind::Static`
        // entry (defensive: today the bridge only enters this pass
        // once per run, but per the no-defensive-fallback rule we
        // still avoid double-push if a future caller orders the
        // passes differently).
        if ctxt
            .model
            .instances
            .iter()
            .any(|inst| inst.item == item_id && matches!(inst.kind, InstanceKind::Static { .. }))
        {
            continue;
        }
        let linkage = ctxt.model.items[item_id].linkage;
        let is_foreign = linkage == Linkage::Foreign;
        let def_path_hash = ctxt.model.item_def_paths[item_id];
        let def_id = if is_foreign {
            // Per Change B: foreign Items live in
            // `tcx.hir_crate_items().foreign_items()` rather than
            // `hir_body_owners`.
            foreign_item_def_id(ctxt.input.tcx, def_path_hash).unwrap_or_else(|| {
                panic!(
                    "construct_source_static_instances: foreign ItemKind::Static \
                     {item_id:?} (def_path_hash={def_path_hash:?}) has no local-crate \
                     DefId via foreign_items()",
                )
            })
        } else {
            source_item_def_id(ctxt.input.tcx, def_path_hash).unwrap_or_else(|| {
                panic!(
                    "construct_source_static_instances: source ItemKind::Static {item_id:?} \
                     (def_path_hash={def_path_hash:?}) has no local-crate DefId; \
                     the MonoItem::Static and HIR statics walkers must intern only local \
                     statics"
                )
            })
        };
        let rustc_ty = ctxt.input.tcx.type_of(def_id).instantiate_identity();
        let ty = ty::translate_ty(ctxt, rustc_ty)?;
        // `translate_static_init` already returns `Ok(None)` for
        // foreign items per its own `is_foreign_item` guard, so the
        // bucket-2 `init: None` shape per spec.md section 9.4
        // (`spec.md:2668-2677`) is produced without an extra branch
        // here. Errors propagate per the no-defensive-fallback rule.
        let init = const_::translate_static_init(ctxt, def_id)?;
        let emission_name = ctxt.model.items[item_id].def_path.rendered;
        let span = ctxt.model.items[item_id].span;
        ctxt.model.instances.push(Instance {
            item: item_id,
            generic_args: Vec::new(),
            emission_name,
            span,
            kind: InstanceKind::Static { ty, init },
        });
    }
    Ok(())
}

/// Per regseq-driver-55 Change D: construct one
/// `InstanceKind::Const { ty, value }` Instance per source-derived
/// `ItemKind::Const` Item (top-level `const` and lifted associated
/// `const` per spec.md section 8.1). Consts cannot be generic, so
/// the relationship is always 1:1.
fn construct_source_const_instances(ctxt: &mut BridgeCtxt<'_>) -> Result<(), BridgeError> {
    use regseq_model::{Instance, InstanceKind, Linkage};

    let const_items: Vec<ItemId> = ctxt
        .model
        .items
        .iter()
        .enumerate()
        .filter_map(|(idx, item)| {
            let item_id = ItemId::from_u32(idx as u32);
            if !matches!(item.kind, ItemKind::Const) {
                return None;
            }
            if !matches!(
                ctxt.model.item_synthetic_kinds[item_id],
                SyntheticItemKind::None
            ) {
                return None;
            }
            // Foreign source const declarations have no body / init
            // per spec.md section 3.1 invariant 10 + section 9.4 and
            // are SPEC-AMBIGUOUS / out of scope per regseq-driver-55
            // Non-Goals 1-2. Change B's `hir_crate_items` walker
            // already filters to `DefKind::{Const, AssocConst}` and
            // would not surface foreign decls, but this guard pins
            // the contract for any future Phase-2 broadening.
            if item.linkage == Linkage::Foreign {
                return None;
            }
            Some(item_id)
        })
        .collect();

    for item_id in const_items {
        let def_path_hash = ctxt.model.item_def_paths[item_id];
        // Source const Items are interned from `tcx.hir_crate_items`
        // (Change B), which only enumerates local-crate definitions;
        // `source_item_def_id` must therefore resolve. Per spec.md
        // section 2 "No defensive fallbacks", a precondition that must
        // hold per the spec panics when violated rather than silently
        // skipping.
        let def_id = source_item_def_id(ctxt.input.tcx, def_path_hash).unwrap_or_else(|| {
            panic!(
                "construct_source_const_instances: source ItemKind::Const {item_id:?} \
                 (def_path_hash={def_path_hash:?}) has no local-crate DefId; \
                 Change B's hir_crate_items walker must only intern local consts"
            )
        });
        let (ty, value) = const_::translate_const_item(ctxt, def_id)?;
        let emission_name = ctxt.model.items[item_id].def_path.rendered;
        let span = ctxt.model.items[item_id].span;
        ctxt.model.instances.push(Instance {
            item: item_id,
            generic_args: Vec::new(),
            emission_name,
            span,
            kind: InstanceKind::Const { ty, value },
        });
    }
    Ok(())
}

/// Per regseq-driver-55 Change E: construct one
/// `InstanceKind::{Struct, Enum, Union} { layout }` Instance per
/// reachable monomorphisation of every source-derived ADT Item AND
/// every synth `VTableType` Item.
///
/// The monomorphisations are enumerated by walking `model.types`:
/// Phase-2's mono-item-driven type translation interns one nominal
/// `Ty` per reachable `(item, args)` pair (spec.md section 3.4
/// Phase 5 cardinality: one Instance per `(Item, GenericArgs, rustc
/// shim flavor)` tuple). For synth `VTableType` ADTs the parallel
/// `Ty::Struct` view is interned by
/// `ty.rs::synthesize_dynamic_vtable_ty` (and refined by
/// `synth/vtable.rs::push_vtable_instance_statics`); its placeholder
/// `ItemId` is patched to the final `ItemId` by
/// `synth::rekey_synthesis_in_phase5`, which runs first in the
/// Phase-5 orchestrator -- so the same `model.types` walk covers
/// both source and synth ADT origins with real `ItemId`s.
///
/// `SyntheticItemKind::ClosureCaptures` ADTs are filtered OUT: the
/// `Struct` Instance for a closure's captures struct is owned by
/// `construct_closure_call_instances` (which keys on the per-closure
/// `DefPathHash` and would conflict with a second push here).
///
/// This pass does NOT call `source_item_def_id`: that helper scans
/// `tcx.hir_body_owners()`, which covers HIR body owners
/// (functions, statics, consts) but NOT ADT definitions
/// (`struct`/`enum`/`union` have no HIR body). The `model.types`
/// walk is the authoritative enumeration and needs no rustc DefId.
fn construct_nominal_adt_instances(ctxt: &mut BridgeCtxt<'_>) -> Result<(), BridgeError> {
    // Per regseq-driver-58 Change D: extend the existing
    // `model.types`-driven nominal-ADT Instance construction so it
    // ALSO fires on bucket-1 source-derived ADT Items that the
    // reactive `translate_adt` path did not reach (e.g., an unused
    // top-level `pub struct Reg(u32)` whose type was never referenced
    // by a translated callee body). For each such Item, eagerly
    // intern the identity-instantiated `Ty::Struct/Enum/Union` view
    // so the subsequent `nominal_adt_instances_pass` walks it.
    //
    // Filtering rules:
    //   * `synthetic_kind == None`: bridge-synth ADTs (closure
    //     captures, vtable types) are owned by sibling passes.
    //   * `linkage != Foreign`: ADTs are never foreign in Rust, so
    //     this is a contract-pin rather than a live trigger.
    //   * `!is_generic`: bucket-3 generic ADTs with zero
    //     monomorphisations get zero Instances per spec.
    let adt_items: Vec<ItemId> = ctxt
        .model
        .items
        .iter()
        .enumerate()
        .filter_map(|(idx, item)| {
            let item_id = ItemId::from_u32(idx as u32);
            if !matches!(
                item.kind,
                ItemKind::Struct | ItemKind::Enum | ItemKind::Union
            ) {
                return None;
            }
            if !matches!(
                ctxt.model.item_synthetic_kinds[item_id],
                SyntheticItemKind::None
            ) {
                return None;
            }
            if item.linkage == regseq_model::Linkage::Foreign {
                return None;
            }
            Some(item_id)
        })
        .collect();
    for item_id in adt_items {
        let bridge_flags = ctxt
            .model
            .item_bridge_flags
            .get(&item_id)
            .copied()
            .unwrap_or_default();
        if bridge_flags.is_generic {
            // Bucket 3: skip eager interning. Reactive monomorphisations
            // (if any) are interned via the existing
            // `translate_adt` path.
            continue;
        }
        let already_has_ty = ctxt
            .model
            .types
            .iter()
            .any(|(_, ty)| matches!(ty, Ty::Struct { item, .. } | Ty::Enum { item, .. } | Ty::Union { item, .. } if *item == item_id));
        if already_has_ty {
            continue;
        }
        let def_path_hash = ctxt.model.item_def_paths[item_id];
        // ADT definitions are NOT body owners, so `source_item_def_id`
        // (which scans `hir_body_owners`) cannot find them. Resolve
        // through `tcx.def_path_hash_to_def_id` instead, matching
        // the pattern used by `bridge::fnsig::adt_def`. The
        // synthetic-kind filter above guards against the
        // "uninterned StableCrateId(0)" panic in
        // `def_path_hash_to_def_id` for synthetic Items.
        let rustc_hash = rustc_hir::definitions::DefPathHash(
            rustc_data_structures::fingerprint::Fingerprint::new(
                def_path_hash.stable_crate_id.0,
                def_path_hash.local_hash,
            ),
        );
        // Per the no-defensive-fallback rule a non-synthetic non-
        // foreign non-generic ADT Item interned by the eager HIR
        // walk MUST resolve to a rustc DefId here. An unresolved
        // hash is a bridge-pipeline invariant violation.
        let def_id = ctxt
            .input
            .tcx
            .def_path_hash_to_def_id(rustc_hash)
            .ok_or_else(|| {
                BridgeError::Phase5(format!(
                    "construct_nominal_adt_instances: source ADT Item {item_id:?} \
                 (def_path_hash={def_path_hash:?}) has no rustc DefId via \
                 def_path_hash_to_def_id; the eager HIR walk (Change A) must \
                 intern only locally-defined ADTs"
                ))
            })?;
        let rustc_ty = ctxt.input.tcx.type_of(def_id).instantiate_identity();
        // Propagate any translation failure. Per Change D + the
        // no-defensive-fallback rule, this branch must not silently
        // skip: a `translate_ty` rejection on the identity-substituted
        // type of a non-generic non-foreign ADT is a real bridge-input
        // invariant violation.
        ty::translate_ty(ctxt, rustc_ty)?;
    }

    let pending = ctxt.pending_def_path_hashes();
    nominal_adt_instances_pass(&mut ctxt.model, &pending)
}

/// Loop body shared by the bridge-side `construct_nominal_adt_instances`
/// pass and the Test Plan A9 idempotency re-invocation helper
/// `replay_construct_nominal_adt_instances_for_tests`. Idempotent by
/// construction: the `existing` `(item, args_key)` set is rebuilt from
/// the current `model.instances` on entry, so a second call against an
/// already-populated model adds no Instance rows.
fn nominal_adt_instances_pass(
    model: &mut RegseqModel,
    pending: &HashMap<ItemId, DefPathHash>,
) -> Result<(), BridgeError> {
    use regseq_model::{GenericArgs, Instance, InstanceKind};

    // Existing `(item, args_key)` coverage for nominal Instances so
    // a re-entry (Test Plan A9 idempotency) or an Instance already
    // produced by another Phase-5 pass is not duplicated (spec.md
    // regseq-driver-55 Change E step 4).
    let mut existing: std::collections::HashSet<(ItemId, ArgsKey)> = model
        .instances
        .iter()
        .filter_map(|inst| match &inst.kind {
            InstanceKind::Struct { .. }
            | InstanceKind::Enum { .. }
            | InstanceKind::Union { .. } => Some((
                inst.item,
                args_key_for_generic_args_with_pending(model, &inst.generic_args, pending),
            )),
            _ => None,
        })
        .collect();

    // Snapshot the nominal `Ty` rows before pushing Instances (the
    // push borrows `model.instances` mutably while `model.types` is
    // immutably walked).
    let rows: Vec<(ItemId, GenericArgs, TyId)> = model
        .types
        .iter()
        .filter_map(|(ty_id, ty)| match ty {
            Ty::Struct { item, args, .. }
            | Ty::Enum { item, args, .. }
            | Ty::Union { item, args, .. } => Some((*item, args.clone(), ty_id)),
            _ => None,
        })
        .collect();

    for (item_id, args, layout_ty) in rows {
        // After `rekey_synthesis_in_phase5` every nominal `Ty`
        // carries a real (patched) `ItemId`. A placeholder here is
        // a Phase-4/5 ordering bug, not a recoverable shape: panic
        // per the README section 2 "No defensive fallbacks" rule.
        assert!(
            item_id.index() < model.items.len(),
            "construct_nominal_adt_instances: nominal Ty carries unpatched \
             placeholder ItemId {} (items.len() = {}); \
             rekey_synthesis_in_phase5 must patch every nominal Ty first",
            item_id.as_u32(),
            model.items.len()
        );
        let item_kind = model.items[item_id].kind.clone();
        if !matches!(
            item_kind,
            ItemKind::Struct | ItemKind::Enum | ItemKind::Union
        ) {
            continue;
        }
        match &model.item_synthetic_kinds[item_id] {
            SyntheticItemKind::None | SyntheticItemKind::VTableType { .. } => {}
            // ClosureCaptures Struct Instances are owned by
            // `construct_closure_call_instances`; any other synth
            // ADT kind is not a definition-site nominal.
            _ => continue,
        }
        let args_key = args_key_for_generic_args_with_pending(model, &args, pending);
        if !existing.insert((item_id, args_key)) {
            continue;
        }
        let emission_name = model.items[item_id].def_path.rendered;
        let span = model.items[item_id].span;
        let kind = match item_kind {
            ItemKind::Struct => InstanceKind::Struct { layout: layout_ty },
            ItemKind::Enum => InstanceKind::Enum { layout: layout_ty },
            ItemKind::Union => InstanceKind::Union { layout: layout_ty },
            _ => unreachable!("filtered to Struct/Enum/Union above"),
        };
        model.instances.push(Instance {
            item: item_id,
            generic_args: args,
            emission_name,
            span,
            kind,
        });
    }
    Ok(())
}

/// Re-invoke the nominal-ADT Instance construction loop on an
/// already-bridged `RegseqModel` for the Test Plan A9 idempotency
/// check. Operates with an empty pending-item map (the model is
/// post-bridge; `BridgeCtxt::pending_items` was drained by the
/// orchestrator). A second call must add no new Instance rows -- the
/// `(item, args_key)` dedup set is rebuilt from the live
/// `model.instances` on entry, so any row produced by the first pass
/// suppresses a duplicate push here.
pub fn replay_construct_nominal_adt_instances_for_tests(
    model: &mut RegseqModel,
) -> Result<(), BridgeError> {
    nominal_adt_instances_pass(model, &HashMap::new())
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

    // Per regseq-driver-82: closure-call instance construction is
    // body-derived but runs BEFORE `run_phase5_body_passes`, and it
    // translates closure signature input/output types outside the
    // materialise guard's window. A closure whose signature carries an
    // RPIT opaque (e.g. `let c = || mk();`) would still reject without
    // a guard here, so reveal body-context opaques for the duration of
    // this pass. The RAII guard restores the prior flag on every exit
    // path (including `?`), and this pass never reaches the
    // identity-signature path, so RJ-009 is unaffected.
    let mut reveal_guard = RevealOpaqueScope::enter(ctxt);
    let ctxt = &mut *reveal_guard;

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

/// Canonical-form output of `canonicalize_call_site_callee`. Per
/// spec section 8.1 (`spec.md:2341-2362`, regseq-driver-56 amendment),
/// a direct call-site `Operand::Constant(FnDef(def_id, generic_args))`
/// resolves through `ty::Instance::try_resolve` to a canonical
/// `(callee_def_id, callee_generic_args)` pair keyed by the resolved
/// `InstanceKind`: `Item(resolved_def_id)` rekeys on the resolved impl
/// DefId; `Virtual(_, idx)` retains the original trait DefId and
/// records `virtual_idx = Some(idx)`; all other shim kinds retain the
/// original MIR operand `(def_id, generic_args)` per the bridge's
/// pre-fix behavior.
pub(crate) struct CanonicalCallSiteCallee<'tcx> {
    /// Canonical `Ty::FnDef` view ready for `translate_ty`.
    pub(crate) callee_ty: rustc_middle::ty::Ty<'tcx>,
    /// Canonical callee `DefId` (resolved impl DefId for `Item`-kind;
    /// original MIR DefId for `Virtual` and shim kinds).
    pub(crate) callee_def_id: DefId,
    /// Canonical callee `GenericArgsRef` (resolved `instance.args` for
    /// `Item`-kind; original MIR `generic_args` for `Virtual` and shim
    /// kinds).
    pub(crate) callee_generic_args: rustc_middle::ty::GenericArgsRef<'tcx>,
    /// `FunctionInstanceFlavor` tag for the resulting Function Instance.
    pub(crate) flavor: FunctionInstanceFlavor,
    /// `virtual_idx = Some(idx)` exactly when `flavor == Virtual`.
    pub(crate) virtual_idx: Option<u32>,
}

/// Resolve a direct call-site `(def_id, generic_args)` through
/// `ty::Instance::try_resolve` and return the canonical
/// `(callee_def_id, callee_generic_args, flavor, virtual_idx)` plus a
/// synthesised `Ty::FnDef` view. Shared by Phase-2 pre-translation
/// (`scan_terminator_for_synthesis`) and Phase-5 callee translation
/// (`translate_call_callee`) so the call-site canonicalization
/// contract lives in one place per spec section 8.1.
///
/// Returns `Err(String)` when `Instance::try_resolve` returns
/// `Ok(None)` or `Err(_)`. The bridge operates exclusively in
/// `TypingEnv::fully_monomorphized`; in that context the resolver
/// never legitimately fails for a `TyKind::FnDef` operand of a
/// successfully monomorphised body, so a `None`/`Err` here is a
/// rustc-invariant violation worth surfacing as a hard error per the
/// no-defensive-fallbacks rule.
pub(crate) fn canonicalize_call_site_callee<'tcx>(
    tcx: TyCtxt<'tcx>,
    def_id: DefId,
    generic_args: rustc_middle::ty::GenericArgsRef<'tcx>,
) -> Result<CanonicalCallSiteCallee<'tcx>, String> {
    let typing_env = rustc_middle::ty::TypingEnv::fully_monomorphized();
    match rustc_middle::ty::Instance::try_resolve(tcx, typing_env, def_id, generic_args) {
        Ok(Some(instance)) => {
            let (callee_def_id, callee_generic_args, flavor, virtual_idx) = match instance.def {
                rustc_middle::ty::InstanceKind::Virtual(_, idx) => (
                    def_id,
                    generic_args,
                    FunctionInstanceFlavor::Virtual,
                    Some(idx as u32),
                ),
                rustc_middle::ty::InstanceKind::Item(_) => (
                    instance.def.def_id(),
                    instance.args,
                    FunctionInstanceFlavor::Item,
                    None,
                ),
                other => (
                    def_id,
                    generic_args,
                    function_instance_flavor_for_kind(other),
                    None,
                ),
            };
            let callee_ty =
                rustc_middle::ty::Ty::new_fn_def(tcx, callee_def_id, callee_generic_args);
            Ok(CanonicalCallSiteCallee {
                callee_ty,
                callee_def_id,
                callee_generic_args,
                flavor,
                virtual_idx,
            })
        }
        Ok(None) => Err("Instance::try_resolve returned Ok(None)".to_string()),
        Err(err) => Err(format!("Instance::try_resolve returned Err({err:?})")),
    }
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

/// Per regseq-driver-58 Change B: resolve a foreign Item's
/// `DefPathHash` to its rustc `DefId` by scanning
/// `tcx.hir_crate_items().foreign_items()`. Foreign decls have no
/// MIR body, so `source_item_def_id` (which scans
/// `hir_body_owners`) cannot find them; this sibling helper is the
/// dedicated resolver for foreign Function / Static Items.
fn foreign_item_def_id(tcx: TyCtxt<'_>, wanted: DefPathHash) -> Option<DefId> {
    for foreign_item_id in tcx.hir_crate_items(()).foreign_items() {
        let def_id = foreign_item_id.owner_id.def_id.to_def_id();
        let actual = tcx.def_path_hash(def_id);
        if actual.stable_crate_id().as_u64() == wanted.stable_crate_id.0
            && actual.local_hash().as_u64() == wanted.local_hash
        {
            return Some(def_id);
        }
    }
    None
}

/// Per regseq-driver-84: the unmangled ABI link name a foreign
/// function (`extern "C" { fn abs(...); }`) is imported under, used as
/// its `emission_name` so the backend's `extern` prototype + call site
/// reference the symbol the linker actually resolves (`abs`, exported
/// by libc) rather than the rendered DefPath.
///
/// Mirrors rustc's `compute_symbol_name` for foreign items: an explicit
/// `#[link_name = "..."]` (surfaced as `codegen_fn_attrs.symbol_name`)
/// wins; otherwise the bare item identifier (`tcx.item_name`). Foreign
/// items are implicitly no-mangle, so the identifier IS the link name.
fn foreign_fn_abi_symbol(tcx: TyCtxt<'_>, def_id: DefId) -> String {
    if let Some(symbol_name) = tcx.codegen_fn_attrs(def_id).symbol_name {
        return symbol_name.to_string();
    }
    tcx.item_name(def_id).to_string()
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

/// Map rustc's `static_mutability` for a `MonoItem::Static` `DefId`
/// onto the RegseqModel `Mutability` enum carried by
/// `ItemKind::Static { mutability }` per spec.md section 8.0
/// (source-level keyword choice; statics cannot be generic so the
/// Item:Instance relationship is always 1:1).
///
/// Per the README section 2 "No defensive fallbacks" rule this
/// panics rather than substituting a default: rustc's mono-item
/// collection only ever yields a `MonoItem::Static` for an actual
/// `static` definition, so `static_mutability` is `Some` by
/// construction; a `None` here is a toolchain-contract violation,
/// not a recoverable shape.
fn source_static_mutability(tcx: TyCtxt<'_>, def_id: DefId) -> regseq_model::Mutability {
    match tcx.static_mutability(def_id) {
        Some(rustc_hir::Mutability::Mut) => regseq_model::Mutability::Mut,
        Some(rustc_hir::Mutability::Not) => regseq_model::Mutability::Not,
        None => panic!(
            "source_static_mutability: MonoItem::Static def_id {def_id:?} \
             has no static_mutability (rustc collects MonoItem::Static \
             only for defined statics)"
        ),
    }
}
