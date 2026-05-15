//! Phase 4 final `ItemId` assignment and side-table population.

use super::{BridgeCtxt, BridgeError};

/// Assigns dense `ItemId`s and populates item side tables in lock-step.
pub(crate) fn assign_item_ids_and_populate_sidetables(
    ctxt: &mut BridgeCtxt<'_>,
) -> Result<(), BridgeError> {
    assign_pending_items(&mut ctxt.model, &mut ctxt.handles, &mut ctxt.pending_items);
    Ok(())
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
            },
            PendingItem {
                handle: second,
                item: item(&mut model, "beta"),
                def_path_hash: DefPathHash {
                    stable_crate_id: StableCrateId(1),
                    local_hash: 2,
                },
                synthetic_kind: SyntheticItemKind::None,
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
