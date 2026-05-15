//! Rustc-to-`RegseqModel` bridge pipeline scaffold.

use std::sync::{Arc, Mutex};

use rustc_middle::mir::mono::MonoItem;
use rustc_middle::ty::TyCtxt;
use rustc_span::def_id::LOCAL_CRATE;
use thiserror::Error;

use regseq_model::{DefPathHash, Item, RegseqModel, StableCrateId, SyntheticItemKind};

mod assign;
mod discovery;
mod handles;
mod sort;

pub use handles::{HandleTable, ItemHandle};

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
        }
    }
}

/// Errors returnable from the bridge.
#[derive(Debug, Error)]
pub enum BridgeError {
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

fn translate_with_interleaved_synthesis(_ctxt: &mut BridgeCtxt<'_>) -> Result<(), BridgeError> {
    Ok(())
}

fn construct_instances_and_resolve_cross_references(
    _ctxt: &mut BridgeCtxt<'_>,
) -> Result<(), BridgeError> {
    Ok(())
}
