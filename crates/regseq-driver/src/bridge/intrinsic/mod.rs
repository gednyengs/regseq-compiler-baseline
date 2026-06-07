//! Construct-neutral `#[rseq::intrinsic("...")]` recognition + materialisation.
//!
//! Shared by every intrinsic family (diagnostic logging, register-access
//! MMIO/poll). The Phase-2 Call-translator
//! caches recognized callees by `DefPathHash` in `BridgeCtxt.intrinsics`;
//! Phase-4's `materialise_intrinsics` records each as an
//! `AttrData::Intrinsic(kind)` on the callee Item. Family-specific modelling
//! (e.g. device-lifecycle descriptor interning) lives in the construct's own
//! bridge module; this module only handles the generic intrinsic marker.

mod recognition;

use regseq_model::DefPath;
use rustc_span::def_id::DefId;

use crate::bridge::{BridgeCtxt, BridgeError, ItemHandle};

/// Phase-2 recognition hook invoked from the Call Terminator translator.
///
/// If the callee was already recognized as an `#[rseq::intrinsic]` emission,
/// validates the Phase-2 cache entry that Phase 4 materializes into
/// `RegseqModel.domain_attrs.item_attrs`.
pub fn recognise_intrinsic_call<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    callee_handle: ItemHandle,
    callee_def_path: &DefPath,
) -> Result<(), BridgeError> {
    let item = regseq_model::ItemId::from_u32(callee_handle.as_u32());
    if item.as_u32() as usize >= cx.model.item_def_paths.len() {
        return Ok(());
    }
    let Some(kind) = cx.intrinsics.get(&cx.model.item_def_paths[item]) else {
        return Ok(());
    };
    if !recognition::callee_name_matches_kind(&cx.model, callee_def_path, kind) {
        return Ok(());
    }
    Ok(())
}

pub(crate) fn recognise_intrinsic_def_id<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    callee_handle: ItemHandle,
    callee_def_path: &DefPath,
    def_id: DefId,
) -> Result<(), BridgeError> {
    if let Some(kind) = recognition::recognise_intrinsic_attr(cx.input.tcx, def_id) {
        if recognition::callee_name_matches_kind(&cx.model, callee_def_path, &kind) {
            cx.intrinsics
                .entry(def_path_hash(cx.input.tcx, def_id))
                .or_insert(kind);
        }
    }
    recognise_intrinsic_call(cx, callee_handle, callee_def_path)
}

pub(crate) fn intrinsic_kind_for_def_id(
    tcx: rustc_middle::ty::TyCtxt<'_>,
    def_id: DefId,
) -> Option<regseq_model::IntrinsicKind> {
    recognition::recognise_intrinsic_attr(tcx, def_id)
}

pub(crate) fn materialise_intrinsics(cx: &mut BridgeCtxt<'_>) {
    let recognised = cx
        .model
        .item_def_paths
        .iter()
        .copied()
        .enumerate()
        .filter_map(|(idx, def_path_hash)| {
            let item = regseq_model::ItemId::from_u32(idx as u32);
            let kind = cx.intrinsics.get(&def_path_hash)?;
            recognition::callee_name_matches_kind(&cx.model, &cx.model.items[item].def_path, kind)
                .then(|| (item, kind.clone()))
        })
        .collect::<Vec<_>>();

    for (item, kind) in recognised {
        recognition::record_intrinsic(&mut cx.model, item, kind);
    }
}

fn def_path_hash(tcx: rustc_middle::ty::TyCtxt<'_>, def_id: DefId) -> regseq_model::DefPathHash {
    let hash = tcx.def_path_hash(def_id);
    regseq_model::DefPathHash {
        stable_crate_id: regseq_model::StableCrateId(hash.stable_crate_id().as_u64()),
        local_hash: hash.local_hash().as_u64(),
    }
}
