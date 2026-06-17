//! Phase 4 final `ItemId` assignment and side-table population.

use super::{
    device_lifecycle, framework_type, interrupt, intrinsic, timed_delay, BridgeCtxt, BridgeError,
};

/// Assigns dense `ItemId`s and populates item side tables in lock-step.
pub(crate) fn assign_item_ids_and_populate_sidetables(
    ctxt: &mut BridgeCtxt<'_>,
) -> Result<(), BridgeError> {
    assign_pending_items(&mut ctxt.model, &mut ctxt.handles, &mut ctxt.pending_items);
    intrinsic::materialise_intrinsics(ctxt);
    framework_type::materialise_framework_types(ctxt);
    // Timed-cpu-delay-and-sleep (TCD construct): validate the signature of
    // every recognized busy_wait/sleep facade (TCD-ERR-02). Runs after
    // `materialise_intrinsics` records the markers it walks.
    timed_delay::validate_delay_signatures(ctxt)?;
    device_lifecycle::materialise_drivers(ctxt)?;
    // Device-interrupt (construct #6): walks the driver map materialised by
    // the device-lifecycle pass above, so it must run after it.
    interrupt::materialise_interrupts(ctxt)?;
    Ok(())
}

/// Drains any pending items added after the initial Phase-4 assignment,
/// running them through the same ItemId + side-table population path.
/// Used to absorb late synthesis triggers fired during Phase-5 body
/// materialisation (e.g., reference-typed const operands that
/// synthesise `ConstMemoryStatic` items per spec section 3.4).
pub(crate) fn assign_late_pending_items(ctxt: &mut BridgeCtxt<'_>) {
    assign_pending_items(&mut ctxt.model, &mut ctxt.handles, &mut ctxt.pending_items);
}

fn assign_pending_items(
    model: &mut regseq_model::RegseqModel,
    handles: &mut super::HandleTable,
    pending_items: &mut Vec<super::PendingItem>,
) {
    for pending in pending_items.drain(..) {
        let item_id = model.items.push(pending.item);
        model.item_def_paths.push(pending.def_path_hash);
        model.item_synthetic_kinds.push(pending.synthetic_kind);
        model.domain_attrs.item_attrs.push(Vec::new());
        // Populate the bridge-only flag side-table per regseq-driver-58.
        // Entries with default `BridgeItemFlags { is_generic: false,
        // signature_non_rigid: false }` are skipped to keep the map
        // sparse; the four-bucket validator falls back to the same
        // default when looking up a missing entry.
        if pending.bridge_flags != regseq_model::BridgeItemFlags::default() {
            model
                .item_bridge_flags
                .insert(item_id, pending.bridge_flags);
        }
        handles.bind_final_item_id(pending.handle, item_id);
    }
}

#[cfg(test)]
mod tests {
    use regseq_model::{
        DefPath, DefPathData, DefPathHash, DefPathSegment, InlineHint, Item, ItemKind, Linkage,
        Span, StableCrateId, SyntheticItemKind,
    };

    use super::*;
    use crate::bridge::{HandleTable, PendingItem};

    fn item(model: &mut regseq_model::RegseqModel, name: &str) -> Item {
        let name_id = model.strings.intern(name);
        let rendered = model.strings.intern(&format!("test::{name}"));
        Item {
            def_path: DefPath {
                crate_id: model.primary_crate,
                segments: vec![DefPathSegment {
                    data: DefPathData::ValueNs(name_id),
                    disambiguator: 0,
                }],
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
            is_result: false,
        }
    }

    fn run_assignment() -> Vec<u32> {
        let mut model = regseq_model::RegseqModel::with_primary("test", StableCrateId(1));
        let mut handles = HandleTable::new();
        let first = handles.fresh_handle();
        let second = handles.fresh_handle();
        let mut pending_items = vec![
            PendingItem {
                handle: first,
                item: item(&mut model, "alpha"),
                def_path_hash: DefPathHash {
                    stable_crate_id: StableCrateId(1),
                    local_hash: 1,
                },
                synthetic_kind: SyntheticItemKind::None,
                bridge_flags: regseq_model::BridgeItemFlags::default(),
            },
            PendingItem {
                handle: second,
                item: item(&mut model, "beta"),
                def_path_hash: DefPathHash {
                    stable_crate_id: StableCrateId(1),
                    local_hash: 2,
                },
                synthetic_kind: SyntheticItemKind::None,
                bridge_flags: regseq_model::BridgeItemFlags::default(),
            },
        ];
        assign_pending_items(&mut model, &mut handles, &mut pending_items);

        vec![
            handles.resolve(first).unwrap().as_u32(),
            handles.resolve(second).unwrap().as_u32(),
        ]
    }

    #[test]
    fn assignment_is_deterministic_for_same_pending_item_order() {
        let expected = run_assignment();
        for _ in 0..10 {
            assert_eq!(run_assignment(), expected);
        }
    }
}
