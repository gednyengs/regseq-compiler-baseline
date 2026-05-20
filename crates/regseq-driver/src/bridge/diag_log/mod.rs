//! Diagnostic-logging intrinsic recognition for bridge Call translation.

pub mod dead_item_elim;
pub mod format;
pub mod peel;
mod recognition;
pub mod validation;

pub use format::{parse_format_template, FormatSpecifier, ParsedFormatTemplate};
pub use peel::diag_log_peel_pass;
pub use validation::validate_log_call;

use regseq_model::{DefPath, DefPathData, InstanceId, InstanceKind, Ty};
use rustc_span::def_id::DefId;

use crate::bridge::{BridgeCtxt, BridgeError, ItemHandle};

/// Structural recognition of `<LogArg<'_> as From<T>>::from` Call
/// callees per spec.md section 26.4 + section 26.5.
///
/// Per the regseq-driver-56 amendment at spec.md:2354-2394, at a
/// direct call site the bridge resolves
/// `ty::Instance::try_resolve(.., fully_monomorphized, def_id,
/// generic_args)` before interning the callee Item, so
/// `InstanceKind::Item` keys on the resolved impl-method DefId
/// rather than the trait-declaration DefId. The recognizer therefore
/// matches a single canonical impl-method shape:
///
/// - the callee Item's def_path terminal segment is the value
///   `from`,
/// - the path contains a `DefPathData::Impl` ancestor segment, and
/// - the Function Instance's `FnSig.output` resolves to a
///   `Ty::Enum`, `Ty::Struct`, or `Ty::Union` whose Item's def_path
///   terminal segment renders as `LogArg`.
///
/// Shared between `peel.rs::peel_log_arg_from_wrapper` (per Phase-5
/// sub-pass) and `dead_item_elim::eliminate_diag_log_peel_residue`
/// (per regseq-driver-48 Change B) so the recognition logic stays
/// in one place.
pub(crate) fn is_from_logarg_instance(
    model: &regseq_model::RegseqModel,
    instance_id: InstanceId,
) -> bool {
    if instance_id.index() >= model.instances.len() {
        return false;
    }
    let instance = &model.instances[instance_id];
    let item = instance.item;
    if item.index() >= model.items.len() {
        return false;
    }
    let def_path = &model.items[item].def_path;

    // Canonical impl-method recognizer: terminal `ValueNs("from")`
    // segment, some `DefPathData::Impl` ancestor, and the Function
    // Instance's `FnSig.output` resolves to a `LogArg`-named ADT.
    let Some(last) = def_path.segments.last() else {
        return false;
    };
    let DefPathData::ValueNs(name_id) = last.data else {
        return false;
    };
    if model.strings.resolve(name_id) != "from" {
        return false;
    }
    if !def_path
        .segments
        .iter()
        .any(|s| matches!(s.data, DefPathData::Impl))
    {
        return false;
    }
    let InstanceKind::Function(function) = &instance.kind else {
        return false;
    };
    if function.fn_sig.index() >= model.sigs.len() {
        return false;
    }
    let sig = model.sigs.resolve(function.fn_sig);
    ty_terminal_segment_is_logarg(model, sig.output)
}

fn ty_terminal_segment_is_logarg(
    model: &regseq_model::RegseqModel,
    ty_id: regseq_model::TyId,
) -> bool {
    if ty_id.index() >= model.types.len() {
        return false;
    }
    let nominal_item = match model.types.resolve(ty_id) {
        Ty::Enum { item, .. } | Ty::Struct { item, .. } | Ty::Union { item, .. } => *item,
        _ => return false,
    };
    if nominal_item.index() >= model.items.len() {
        return false;
    }
    let path_id = model.items[nominal_item].def_path.rendered;
    let path = model.strings.resolve(path_id);
    path.rsplit("::")
        .next()
        .map(|seg| seg == "LogArg")
        .unwrap_or(false)
}

/// Phase-2 recognition hook invoked from the Call Terminator translator.
///
/// If the callee has already been recognized as a
/// `regseq::log::<sev>!` macro emission per spec.md sections 3.6.4
/// and 26.1-26.5, validates the Phase-2 cache entry that Phase 4
/// materializes into `RegseqModel.domain_attrs.item_attrs`.
pub fn recognise_log_intrinsic_call<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    callee_handle: ItemHandle,
    callee_def_path: &DefPath,
) -> Result<(), BridgeError> {
    let item = regseq_model::ItemId::from_u32(callee_handle.as_u32());
    if item.as_u32() as usize >= cx.model.item_def_paths.len() {
        return Ok(());
    }
    let Some(kind) = cx.diag_log_intrinsics.get(&cx.model.item_def_paths[item]) else {
        return Ok(());
    };
    if !recognition::callee_name_matches_kind(&cx.model, callee_def_path, kind) {
        return Ok(());
    }
    Ok(())
}

pub(crate) fn recognise_log_intrinsic_def_id<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    callee_handle: ItemHandle,
    callee_def_path: &DefPath,
    def_id: DefId,
) -> Result<(), BridgeError> {
    if let Some(kind) = recognition::recognise_log_intrinsic_attr(cx.input.tcx, def_id) {
        if recognition::callee_name_matches_kind(&cx.model, callee_def_path, &kind) {
            cx.diag_log_intrinsics
                .entry(def_path_hash(cx.input.tcx, def_id))
                .or_insert(kind);
        }
    }
    recognise_log_intrinsic_call(cx, callee_handle, callee_def_path)
}

pub(crate) fn intrinsic_kind_for_def_id(
    tcx: rustc_middle::ty::TyCtxt<'_>,
    def_id: DefId,
) -> Option<regseq_model::IntrinsicKind> {
    recognition::recognise_log_intrinsic_attr(tcx, def_id)
}

pub(crate) fn materialise_log_intrinsics(cx: &mut BridgeCtxt<'_>) {
    let recognised = cx
        .model
        .item_def_paths
        .iter()
        .copied()
        .enumerate()
        .filter_map(|(idx, def_path_hash)| {
            let item = regseq_model::ItemId::from_u32(idx as u32);
            let kind = cx.diag_log_intrinsics.get(&def_path_hash)?;
            recognition::callee_name_matches_kind(&cx.model, &cx.model.items[item].def_path, kind)
                .then(|| (item, kind.clone()))
        })
        .collect::<Vec<_>>();

    for (item, kind) in recognised {
        recognition::record_log_intrinsic(&mut cx.model, item, kind);
    }
}

fn def_path_hash(tcx: rustc_middle::ty::TyCtxt<'_>, def_id: DefId) -> regseq_model::DefPathHash {
    let hash = tcx.def_path_hash(def_id);
    regseq_model::DefPathHash {
        stable_crate_id: regseq_model::StableCrateId(hash.stable_crate_id().as_u64()),
        local_hash: hash.local_hash().as_u64(),
    }
}
