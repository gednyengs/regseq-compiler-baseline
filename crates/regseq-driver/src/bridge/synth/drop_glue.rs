//! Drop-glue bridge item synthesis.

use regseq_model::{args_key_for_generic_args_with_pending, GenericArg, SyntheticItemKind};

use crate::bridge::{BridgeCtxt, BridgeError, ItemHandle};

/// Synthesise a drop-glue Function Item for a non-Copy type.
pub fn synthesise_drop_glue<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    drop_target_ty: regseq_model::TyId,
) -> Result<ItemHandle, BridgeError> {
    if let Some(handle) = cx.drop_glue_synth_cache.get(&drop_target_ty) {
        return Ok(*handle);
    }

    // Per audit-finding F4 + spec.md section 3.4 D-12-5.3, the
    // drop_target_ty may reference a source-derived ADT whose ItemId
    // is still a Phase-2 placeholder. Resolve through the pending
    // overlay so the synthetic-item self_ty key is stable across the
    // placeholder→final remap at Phase 4.
    let pending = cx.pending_def_path_hashes();
    let self_ty = args_key_for_generic_args_with_pending(
        &cx.model,
        &vec![GenericArg::Type(drop_target_ty)],
        &pending,
    );
    let handle = super::vtable::push_pending_synthetic_item(
        cx,
        regseq_model::ItemKind::Function,
        SyntheticItemKind::DropGlue { self_ty },
        None,
    );
    cx.drop_glue_synth_cache.insert(drop_target_ty, handle);
    Ok(handle)
}

#[cfg(test)]
mod tests {
    use regseq_model::TyId;

    #[test]
    fn ty_id_key_shape_supports_drop_glue_dedup() {
        assert_eq!(TyId::from_u32(3), TyId::from_u32(3));
        assert_ne!(TyId::from_u32(3), TyId::from_u32(4));
    }
}
