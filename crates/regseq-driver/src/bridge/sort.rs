//! Phase 3 F4 sorting.

use regseq_model::RegseqModel;

use super::{BridgeError, PendingItem};

/// Sorts Phase-2 pending items by the model F4 comparator contract.
pub(crate) fn f4_sort(
    model: &RegseqModel,
    pending_items: &mut [PendingItem],
) -> Result<(), BridgeError> {
    pending_items.sort_by(|first, second| model.compare_items_f4(&first.item, &second.item));
    Ok(())
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

    #[test]
    fn f4_sort_orders_pending_items_by_model_comparator_contract() {
        let mut model = regseq_model::RegseqModel::with_primary("test", StableCrateId(1));
        let mut handles = HandleTable::new();
        let beta = PendingItem {
            handle: handles.fresh_handle(),
            item: item(&mut model, "beta"),
            def_path_hash: DefPathHash {
                stable_crate_id: StableCrateId(1),
                local_hash: 2,
            },
            synthetic_kind: SyntheticItemKind::None,
        };
        let alpha = PendingItem {
            handle: handles.fresh_handle(),
            item: item(&mut model, "alpha"),
            def_path_hash: DefPathHash {
                stable_crate_id: StableCrateId(1),
                local_hash: 1,
            },
            synthetic_kind: SyntheticItemKind::None,
        };
        let mut pending_items = vec![beta, alpha];

        f4_sort(&model, &mut pending_items).expect("sort should succeed");

        let ordered = pending_items
            .iter()
            .map(|pending| model.strings.resolve(pending.item.def_path.rendered))
            .collect::<Vec<_>>();
        assert_eq!(ordered, vec!["test::alpha", "test::beta"]);
    }
}
