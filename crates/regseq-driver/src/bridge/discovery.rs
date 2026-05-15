//! Phase 1 mono-item discovery.

use rustc_middle::mir::mono::MonoItem;
use rustc_middle::ty::TyCtxt;

use super::BridgeError;

/// Collects the crate mono-item closure from rustc.
pub(crate) fn collect_crate_mono_items<'tcx>(
    tcx: TyCtxt<'tcx>,
) -> Result<Vec<MonoItem<'tcx>>, BridgeError> {
    let partitions = tcx.collect_and_partition_mono_items(());
    let mut mono_items = Vec::new();
    for codegen_unit in partitions.codegen_units {
        mono_items.extend(
            codegen_unit
                .items_in_deterministic_order(tcx)
                .into_iter()
                .map(|(item, _data)| item),
        );
    }
    Ok(mono_items)
}
