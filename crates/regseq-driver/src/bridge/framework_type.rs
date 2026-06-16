//! Item/type-driven `framework_type` recognition for the bridge.
//!
//! Unlike diagnostic-logging recognition (which is call-driven and
//! fires at the Call terminator, see `bridge::diag_log`), a framework
//! type such as `RegseqError` is a *type*: recognition is therefore
//! item/type-driven. [`materialise_framework_types`] walks the
//! interned enum Items, reads the `#[rseq::framework_type("...")]`
//! tool attribute carried by each enum's source definition, and
//! records `AttrData::FrameworkType(..)` onto the matching `ItemId`.
//!
//! The split mirrors the diag-log precedent: a pure recognizer
//! ([`recognise_framework_type_attr`] / [`framework_type_for_name`])
//! plus a materialization pass ([`materialise_framework_types`]).

use regseq_model::{AttrData, FrameworkType, ItemId, ItemKind, RegseqModel, SyntheticItemKind};
use rustc_span::{def_id::DefId, Symbol as RustcSymbol};

use super::BridgeCtxt;

/// Read the `#[rseq::framework_type("...")]` tool attribute on
/// `def_id` and map it to a [`FrameworkType`].
///
/// Mirrors `intrinsic::recognition::recognise_intrinsic_attr`: the
/// tool-attribute path is `[rseq, framework_type]` (fixtures enable it
/// with `#![register_tool(rseq)]`). Returns `None` when the attribute
/// is absent, appears more than once, is not a single string literal,
/// or carries a NAME outside the closed framework-type vocabulary.
pub(crate) fn recognise_framework_type_attr(
    tcx: rustc_middle::ty::TyCtxt<'_>,
    def_id: DefId,
) -> Option<FrameworkType> {
    let path = [
        RustcSymbol::intern("rseq"),
        RustcSymbol::intern("framework_type"),
    ];
    let mut attrs = tcx.get_attrs_by_path(def_id, &path);
    let attr = attrs.next()?;
    if attrs.next().is_some() {
        return None;
    }
    let list = attr.meta_item_list()?;
    let [nested] = list.as_slice() else {
        return None;
    };
    let lit = nested.lit()?;
    let rustc_ast::LitKind::Str(name, _) = lit.kind else {
        return None;
    };
    framework_type_for_name(name.as_str())
}

/// Closed-vocabulary lookup from dash-form attribute NAME to a
/// [`FrameworkType`].
///
/// The NAME string uses dash form (`regseq-error`), matching the
/// `#[rseq::framework_type("regseq-error")]` spelling. Unknown names
/// return `None`. The mapping lives here in the driver rather than on
/// the model type per the task Non-Goals (no regseq-model change).
fn framework_type_for_name(name: &str) -> Option<FrameworkType> {
    match name {
        "regseq-error" => Some(FrameworkType::RegseqError),
        // The ISR dispatch-result type (device-interrupt construct #6):
        // `#[rseq::framework_type("irq-result")]`.
        "irq-result" => Some(FrameworkType::IrqResult),
        _ => None,
    }
}

/// Record `AttrData::FrameworkType(ty)` onto `item`'s attribute list,
/// idempotently: a second call for an Item that already carries a
/// `FrameworkType` marker is a no-op.
fn record_framework_type(model: &mut RegseqModel, item: ItemId, ty: FrameworkType) {
    let attrs = &mut model.domain_attrs.item_attrs[item];
    if !attrs
        .iter()
        .any(|attr| matches!(attr, AttrData::FrameworkType(_)))
    {
        attrs.push(AttrData::FrameworkType(ty));
    }
}

/// Recover the rustc `DefId` for a source-derived Item from its
/// `DefPathHash` side-table entry.
///
/// Mirrors the existing ADT-resolution path in
/// `bridge/mod.rs::construct_nominal_adt_instances`. Synthetic Items
/// use the reserved `StableCrateId(0)`, whose hash panics inside
/// `def_path_hash_to_def_id`; callers MUST filter those out before
/// calling this helper.
pub(crate) fn recover_source_def_id(
    tcx: rustc_middle::ty::TyCtxt<'_>,
    def_path_hash: regseq_model::DefPathHash,
) -> Option<DefId> {
    let rustc_hash =
        rustc_hir::definitions::DefPathHash(rustc_data_structures::fingerprint::Fingerprint::new(
            def_path_hash.stable_crate_id.0,
            def_path_hash.local_hash,
        ));
    tcx.def_path_hash_to_def_id(rustc_hash)
}

/// Item/type-driven materialization pass: walk every enum Item, read
/// its `#[rseq::framework_type("...")]` tool attribute, and record the
/// recognized [`FrameworkType`] onto its `ItemId`.
///
/// Mirrors `intrinsic::materialise_intrinsics` in shape, but is
/// item-driven (walking `model.items` enum entries) rather than
/// call-driven. Recognition fires for any marked enum regardless of
/// how it is used -- including an enum that appears only as a
/// function return type -- so the authored-internal boundary
/// distinction (R02/I01) is preserved.
///
/// Synthetic and non-source Items are skipped per the PLAN's T04
/// rule. Synthetic Items carry the reserved `StableCrateId(0)`, whose
/// hash panics inside `def_path_hash_to_def_id`, so they are filtered
/// before recovery. The recovered `DefId` is then required to be local
/// (`is_local`): the framework type is identified purely by the
/// `#[rseq::framework_type("...")]` attribute on its source definition
/// in the crate being compiled, so a dependency-crate enum (a
/// non-source Item) is never attributed.
pub(crate) fn materialise_framework_types(cx: &mut BridgeCtxt<'_>) {
    let tcx = cx.input.tcx;
    let recognised = cx
        .model
        .items
        .iter()
        .enumerate()
        .filter_map(|(idx, item)| {
            let item_id = ItemId::from_u32(idx as u32);
            if !matches!(item.kind, ItemKind::Enum) {
                return None;
            }
            if !matches!(
                cx.model.item_synthetic_kinds[item_id],
                SyntheticItemKind::None
            ) {
                return None;
            }
            let def_id = recover_source_def_id(tcx, cx.model.item_def_paths[item_id])?;
            // Skip non-source (dependency-crate) Items: only enums
            // defined in the crate under compilation are eligible.
            if !def_id.is_local() {
                return None;
            }
            let framework_type = recognise_framework_type_attr(tcx, def_id)?;
            Some((item_id, framework_type))
        })
        .collect::<Vec<_>>();

    for (item_id, framework_type) in recognised {
        record_framework_type(&mut cx.model, item_id, framework_type);
    }
}

#[cfg(test)]
mod tests {
    use regseq_model::{AttrData, FrameworkType, IndexVec, ItemId, RegseqModel, StableCrateId};

    use super::*;

    fn model_with_item_attrs(count: usize) -> RegseqModel {
        let mut model = RegseqModel::with_primary("test", StableCrateId(1));
        model.domain_attrs.item_attrs = IndexVec::from_elem(Vec::new(), count);
        model
    }

    #[test]
    fn recognises_regseq_error_name() {
        assert_eq!(
            framework_type_for_name("regseq-error"),
            Some(FrameworkType::RegseqError)
        );
    }

    #[test]
    fn recognises_irq_result_name() {
        assert_eq!(
            framework_type_for_name("irq-result"),
            Some(FrameworkType::IrqResult)
        );
        // Underscore typo stays outside the closed vocabulary.
        assert_eq!(framework_type_for_name("irq_result"), None);
    }

    #[test]
    fn rejects_typo_wrong_and_empty_names() {
        // Underscore typo, wrong string, and the empty NAME all fall
        // outside the closed vocabulary.
        assert_eq!(framework_type_for_name("regseq_error"), None);
        assert_eq!(framework_type_for_name("not-an-error"), None);
        assert_eq!(framework_type_for_name(""), None);
    }

    #[test]
    fn record_is_idempotent() {
        let mut model = model_with_item_attrs(1);
        let item = ItemId::from_u32(0);

        record_framework_type(&mut model, item, FrameworkType::RegseqError);
        record_framework_type(&mut model, item, FrameworkType::RegseqError);

        assert_eq!(
            model.domain_attrs.item_attrs[item],
            vec![AttrData::FrameworkType(FrameworkType::RegseqError)]
        );
    }

    #[test]
    fn unmarked_item_carries_no_framework_attr() {
        let model = model_with_item_attrs(1);
        assert!(model.domain_attrs.item_attrs[ItemId::from_u32(0)].is_empty());
    }
}
