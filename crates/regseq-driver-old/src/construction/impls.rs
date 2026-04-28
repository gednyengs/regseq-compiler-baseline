//! Impl-block construction for Pass 1.
//!
//! Categorizes each impl-block as inherent vs trait, and driver vs
//! non-driver, then builds the corresponding [`ImplBlock`] with method
//! stubs and associated constants. Method bodies are always placeholder
//! [`regseq_model_old::ir::Body`] values -- Pass 2 populates them.

use std::collections::HashMap;

use rustc_hir::def_id::DefId;
use rustc_hir::ImplItemKind;

use regseq_model_old::identity::TypeDefId;
use regseq_model_old::model::ImplBlock;

use super::walk::{empty_body, function_signature, BuildContext};

/// Where an [`ImplBlock`] should be attached in the owning module.
pub enum ImplBlockAttachment {
    NonDriverInherent(ImplBlock),
    NonDriverTrait(ImplBlock),
    DriverInherent(ImplBlock),
    DriverTrait(ImplBlock),
    Skipped,
}

pub fn build_impl_block(
    cx: &mut BuildContext<'_, '_>,
    impl_def: DefId,
    target_def: DefId,
    local_type_ids: &HashMap<DefId, TypeDefId>,
) -> Result<ImplBlockAttachment, super::ConstructionError> {
    let target_type = match local_type_ids.get(&target_def) {
        Some(id) => *id,
        None => return Ok(ImplBlockAttachment::Skipped),
    };

    let is_driver = super::walk::def_has_rseq_attr(cx.tcx, target_def, "driver");
    let trait_name = trait_name_for_impl(cx, impl_def);

    let impl_local = impl_def.as_local().expect("impl must be local");
    let hir_item = cx.tcx.hir_item(rustc_hir::ItemId {
        owner_id: rustc_hir::OwnerId { def_id: impl_local },
    });
    let impl_data = match &hir_item.kind {
        rustc_hir::ItemKind::Impl(block) => block,
        _ => return Ok(ImplBlockAttachment::Skipped),
    };

    let mut methods = Vec::new();
    let mut associated_constants = Vec::new();

    for impl_item_ref in impl_data.items {
        let impl_item = cx.tcx.hir_impl_item(rustc_hir::ImplItemId {
            owner_id: impl_item_ref.owner_id,
        });
        let member_def = impl_item_ref.owner_id.def_id.to_def_id();
        let name = impl_item.ident.name.as_str().to_owned();
        match &impl_item.kind {
            ImplItemKind::Fn(..) => {
                methods.push(build_method(cx, member_def, &name, impl_item)?);
            }
            ImplItemKind::Const(..) => {
                associated_constants.push(super::walk::build_associated_constant(
                    cx, member_def, &name,
                ));
            }
            _ => {}
        }
    }

    let block = ImplBlock {
        target_type,
        trait_name,
        methods,
        associated_constants,
    };

    Ok(match (is_driver, block.trait_name.is_some()) {
        (true, true) => ImplBlockAttachment::DriverTrait(block),
        (true, false) => ImplBlockAttachment::DriverInherent(block),
        (false, true) => ImplBlockAttachment::NonDriverTrait(block),
        (false, false) => ImplBlockAttachment::NonDriverInherent(block),
    })
}

fn trait_name_for_impl(
    cx: &mut BuildContext<'_, '_>,
    impl_def: DefId,
) -> Option<regseq_model_old::identity::Symbol> {
    let trait_ref = cx.tcx.impl_opt_trait_ref(impl_def)?;
    let trait_def = trait_ref.skip_binder().def_id;
    let name = cx.tcx.item_name(trait_def).as_str().to_owned();
    Some(cx.symbols.intern(&name))
}

fn build_method(
    cx: &mut BuildContext<'_, '_>,
    method_def: DefId,
    name: &str,
    impl_item: &rustc_hir::ImplItem<'_>,
) -> Result<regseq_model_old::model::Function, super::ConstructionError> {
    use regseq_model_old::identity::FunctionId;

    let id = FunctionId::from(cx.alloc_and_record_public(method_def));
    cx.function_map.insert(method_def, id);
    let name_sym = cx.intern_public(name);
    let (params, return_type) = function_signature(cx, method_def);
    let arg_count = params.len();
    let body = empty_body(cx.tcx, cx.symbols, method_def, arg_count);

    let attrs = super::attrs::collect_rseq_attrs(cx.tcx, method_def)?;
    let _ = impl_item;

    Ok(regseq_model_old::model::Function {
        id,
        name: name_sym,
        params,
        return_type,
        body,
        attrs,
    })
}
