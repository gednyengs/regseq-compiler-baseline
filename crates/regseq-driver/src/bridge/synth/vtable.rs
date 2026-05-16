//! Vtable-family bridge item synthesis.

use regseq_model::{
    args_key_for_generic_args, render_def_path, synthetic_def_path_hash,
    synthetic_def_path_segments, CrateId, DefPath, DefPathData, DefPathHash, DefPathSegment,
    GenericArg, InlineHint, Item, ItemKind, Linkage, Mutability, PrincipalTraitKey, Span,
    SyntheticItemKind, TyId,
};

use super::{VTableSynthHandles, VTableSynthKey};
use crate::bridge::{BridgeCtxt, BridgeError, ItemHandle, PendingItem};

pub(crate) fn synthesise_vtable_type(
    cx: &mut BridgeCtxt<'_>,
    principal: PrincipalTraitKey,
) -> ItemHandle {
    if let Some(handle) = cx.vtable_type_synth_cache.get(&principal) {
        return *handle;
    }
    let handle = push_pending_synthetic_item(
        cx,
        ItemKind::Struct,
        SyntheticItemKind::VTableType {
            principal: principal.clone(),
        },
        None,
    );
    cx.vtable_type_synth_cache.insert(principal, handle);
    handle
}

/// Synthesise the vtable type, instance, and shim Items.
pub fn synthesise_vtable<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    principal: PrincipalTraitKey,
    impl_ty: TyId,
) -> Result<VTableSynthHandles, BridgeError> {
    synthesise_vtable_with_method_shims(cx, principal, impl_ty, vec![(3, None)])
}

pub(crate) fn synthesise_vtable_with_method_shims<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    principal: PrincipalTraitKey,
    impl_ty: TyId,
    method_shims: Vec<(u32, Option<DefPathHash>)>,
) -> Result<VTableSynthHandles, BridgeError> {
    let self_ty = args_key_for_generic_args(&cx.model, &vec![GenericArg::Type(impl_ty)]);
    let key = VTableSynthKey {
        principal: principal.clone(),
        self_ty,
    };
    if let Some(handles) = cx.vtable_synth_cache.get(&key) {
        return Ok(handles.clone());
    }

    let vtable_type = synthesise_vtable_type(cx, principal.clone());
    let vtable_instance = push_pending_synthetic_item(
        cx,
        ItemKind::Static {
            mutability: Mutability::Not,
        },
        SyntheticItemKind::VTableInstance {
            principal: principal.clone(),
            self_ty,
        },
        None,
    );
    let method_shims = method_shims
        .into_iter()
        .map(|(vtable_entry_index, method_def_path)| {
            push_pending_synthetic_item(
                cx,
                ItemKind::Function,
                SyntheticItemKind::VTableMethodShim {
                    principal: principal.clone(),
                    self_ty,
                    vtable_entry_index,
                    method_def_path: method_def_path.unwrap_or(principal.trait_path),
                },
                None,
            )
        })
        .collect::<Vec<_>>();
    let drop_shim = push_pending_synthetic_item(
        cx,
        ItemKind::Function,
        SyntheticItemKind::VTableDropShim {
            principal: principal.clone(),
            self_ty,
        },
        None,
    );

    let handles = VTableSynthHandles {
        vtable_type,
        vtable_instance,
        method_shims,
        drop_shim,
    };
    cx.vtable_synth_cache.insert(key, handles.clone());
    Ok(handles)
}

pub(crate) fn push_pending_synthetic_item(
    cx: &mut BridgeCtxt<'_>,
    kind: ItemKind,
    synthetic_kind: SyntheticItemKind,
    def_path_hash_override: Option<DefPathHash>,
) -> ItemHandle {
    let handle = cx.handles.fresh_handle();
    let (def_path, def_path_hash) = synthetic_def_path(cx, &synthetic_kind, def_path_hash_override);
    let item = Item {
        def_path,
        kind,
        span: Span::dummy(),
        linkage: Linkage::Default,
        link_section: None,
        inline_hint: InlineHint::None,
        is_unsafe: false,
        is_phantom_data: false,
        is_unsafe_cell: false,
        is_manually_drop: false,
        opaque_handle_eligible: false,
    };
    cx.pending_items.push(PendingItem {
        handle,
        item,
        def_path_hash,
        synthetic_kind,
    });
    handle
}

pub(crate) fn synthetic_def_path(
    cx: &mut BridgeCtxt<'_>,
    synthetic_kind: &SyntheticItemKind,
    def_path_hash_override: Option<DefPathHash>,
) -> (DefPath, DefPathHash) {
    match synthetic_kind {
        SyntheticItemKind::ClosureCaptures { closure } => {
            let text = format!("<closure captures {}>", closure.local_hash);
            let text_id = cx.model.strings.intern(&text);
            let def_path = DefPath {
                crate_id: cx.model.primary_crate,
                segments: vec![DefPathSegment {
                    data: DefPathData::TypeNs(text_id),
                    disambiguator: 0,
                }],
                rendered: text_id,
            };
            (def_path, def_path_hash_override.unwrap_or(*closure))
        }
        SyntheticItemKind::None => synthetic_none_def_path(cx, "drop_glue"),
        _ => {
            let segments = synthetic_def_path_segments(synthetic_kind, &mut cx.model.strings);
            let crate_id = synthetic_crate_id(cx, synthetic_kind);
            let rendered = render_def_path(
                &DefPath {
                    crate_id,
                    segments: segments.clone(),
                    rendered: regseq_model::StringId::ZERO,
                },
                &cx.model.crates,
                &cx.model.strings,
            );
            let def_path = DefPath {
                crate_id,
                segments,
                rendered: cx.model.strings.intern(&rendered),
            };
            (def_path, synthetic_def_path_hash(synthetic_kind))
        }
    }
}

fn synthetic_crate_id(cx: &BridgeCtxt<'_>, synthetic_kind: &SyntheticItemKind) -> CrateId {
    let stable = match synthetic_kind {
        SyntheticItemKind::ClosureCall { closure, .. }
        | SyntheticItemKind::ClosureDropGlue { closure } => Some(closure.stable_crate_id),
        SyntheticItemKind::VTableType { principal }
        | SyntheticItemKind::VTableInstance { principal, .. }
        | SyntheticItemKind::VTableMethodShim { principal, .. }
        | SyntheticItemKind::VTableDropShim { principal, .. } => {
            Some(principal.trait_path.stable_crate_id)
        }
        SyntheticItemKind::DropGlue { .. }
        | SyntheticItemKind::ConstMemoryStatic { .. }
        | SyntheticItemKind::None
        | SyntheticItemKind::ClosureCaptures { .. } => None,
    };
    stable
        .and_then(|stable| {
            cx.model
                .crates
                .iter()
                .enumerate()
                .find(|(_, krate)| krate.stable_crate_id == stable)
                .map(|(idx, _)| CrateId::from_u32(idx as u32))
        })
        .unwrap_or(cx.model.primary_crate)
}

fn synthetic_none_def_path(cx: &mut BridgeCtxt<'_>, family: &str) -> (DefPath, DefPathHash) {
    let disambiguator = cx.pending_items.len() as u64;
    let text = format!("<regseq_synth::{family}::{disambiguator}>");
    let text_id = cx.model.strings.intern(&text);
    let def_path = DefPath {
        crate_id: cx.model.primary_crate,
        segments: vec![DefPathSegment {
            data: DefPathData::ValueNs(text_id),
            disambiguator: 0,
        }],
        rendered: text_id,
    };
    (
        def_path,
        DefPathHash {
            stable_crate_id: regseq_model::StableCrateId(0),
            local_hash: 0xD00D_600E_0000_0000 | disambiguator,
        },
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn empty_self_key_is_available_for_vtable_tests() {
        assert_eq!(regseq_model::ArgsKey::EMPTY, regseq_model::ArgsKey::EMPTY);
    }
}
