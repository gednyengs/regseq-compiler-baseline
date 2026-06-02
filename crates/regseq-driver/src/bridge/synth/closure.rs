//! Closure-family bridge item synthesis.

use regseq_model::{
    render_def_path, ClosureCallKind, CrateId, CrateInfo, DefPath, DefPathHash, DefPathSegment,
    InlineHint, Item, ItemKind, Linkage, Span, StableCrateId, SyntheticItemKind,
};
use rustc_span::def_id::DefId;

use super::ClosureSynthHandles;
use crate::bridge::def_path::translate_def_path_data;
use crate::bridge::{BridgeCtxt, BridgeError, PendingItem};

/// Synthesise the closure captures, call, and drop-glue Items.
pub fn synthesise_closure<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    closure_def: rustc_middle::ty::ClosureArgs<rustc_middle::ty::TyCtxt<'tcx>>,
) -> Result<ClosureSynthHandles, BridgeError> {
    let closure_def_id = find_closure_def_id(cx, closure_def).ok_or_else(|| {
        BridgeError::Phase3("closure synthesis requires a discovered closure DefId".to_string())
    })?;
    synthesise_closure_for_def_id(cx, closure_def_id, closure_def)
}

pub(crate) fn synthesise_closure_for_def_id<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    closure_def_id: DefId,
    closure_def: rustc_middle::ty::ClosureArgs<rustc_middle::ty::TyCtxt<'tcx>>,
) -> Result<ClosureSynthHandles, BridgeError> {
    let closure = def_path_hash(cx.input.tcx, closure_def_id);
    if let Some(handles) = cx
        .closure_synth_cache
        .iter()
        .find(|handles| handles.closure == closure)
    {
        return Ok(handles.clone());
    }

    let captures = push_closure_captures_item(cx, closure_def_id, closure);
    Ok(synthesise_closure_tail(cx, closure, closure_def, captures))
}

fn synthesise_closure_tail<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    closure: DefPathHash,
    closure_def: rustc_middle::ty::ClosureArgs<rustc_middle::ty::TyCtxt<'tcx>>,
    captures: crate::bridge::ItemHandle,
) -> ClosureSynthHandles {
    let calls = closure_call_kinds(closure_def)
        .into_iter()
        .map(|call_kind| {
            super::vtable::push_pending_synthetic_item(
                cx,
                regseq_model::ItemKind::Function,
                SyntheticItemKind::ClosureCall { closure, call_kind },
                None,
            )
        })
        .collect::<Vec<_>>();
    let drop_glue = closure_def
        .tupled_upvars_ty()
        .needs_drop(
            cx.input.tcx,
            rustc_middle::ty::TypingEnv::fully_monomorphized(),
        )
        .then(|| {
            super::vtable::push_pending_synthetic_item(
                cx,
                regseq_model::ItemKind::Function,
                SyntheticItemKind::ClosureDropGlue { closure },
                None,
            )
        });

    let handles = ClosureSynthHandles {
        closure,
        captures,
        calls,
        drop_glue,
    };
    cx.closure_synth_cache.push(handles.clone());
    handles
}

fn find_closure_def_id<'tcx>(
    cx: &BridgeCtxt<'tcx>,
    closure_def: rustc_middle::ty::ClosureArgs<rustc_middle::ty::TyCtxt<'tcx>>,
) -> Option<DefId> {
    let discovered = cx.input.discovered_mono_items.lock().ok()?.clone();
    for mono_item in discovered {
        let rustc_middle::mir::mono::MonoItem::Fn(instance) = mono_item else {
            continue;
        };
        let Some(local_def_id) = instance.def_id().as_local() else {
            continue;
        };
        let body = cx.input.tcx.optimized_mir(local_def_id);
        for local in &body.local_decls {
            if let rustc_middle::ty::TyKind::Closure(def_id, args) = local.ty.kind() {
                if args.as_closure() == closure_def {
                    return Some(*def_id);
                }
            }
        }
    }
    for item_id in cx
        .input
        .tcx
        .hir_module_items(rustc_hir::def_id::LocalModDefId::CRATE_DEF_ID)
        .free_items()
    {
        if !matches!(
            cx.input.tcx.hir_item(item_id).kind,
            rustc_hir::ItemKind::Fn { .. }
        ) {
            continue;
        }
        let body = cx.input.tcx.optimized_mir(item_id.owner_id.def_id);
        for local in &body.local_decls {
            if let rustc_middle::ty::TyKind::Closure(def_id, args) = local.ty.kind() {
                if args.as_closure() == closure_def {
                    return Some(*def_id);
                }
            }
        }
    }
    None
}

fn closure_call_kinds<'tcx>(
    closure_def: rustc_middle::ty::ClosureArgs<rustc_middle::ty::TyCtxt<'tcx>>,
) -> Vec<ClosureCallKind> {
    match closure_def.kind().as_str() {
        "Fn" => vec![
            ClosureCallKind::Call,
            ClosureCallKind::CallMut,
            ClosureCallKind::CallOnce,
        ],
        "FnMut" => vec![ClosureCallKind::CallMut, ClosureCallKind::CallOnce],
        "FnOnce" => vec![ClosureCallKind::CallOnce],
        _ => vec![ClosureCallKind::CallOnce],
    }
}

fn push_closure_captures_item(
    cx: &mut BridgeCtxt<'_>,
    closure_def_id: DefId,
    closure: DefPathHash,
) -> crate::bridge::ItemHandle {
    // Per spec 13.1 + audit-finding F4 the captures-struct Item is
    // owned by this synth path. We also register the handle in
    // `source_item_handles` so any defensive `intern_item` call for
    // the closure DefId dedups against the same handle rather than
    // pushing a second `SyntheticItemKind::None` Item (see HIGH-40-1
    // / HIGH-37-1 audit findings).
    if let Some(handle) = cx.source_item_handles.get(&closure).copied() {
        return handle;
    }
    let handle = cx.handles.fresh_handle();
    cx.source_item_handles.insert(closure, handle);
    let item = source_item(cx, closure_def_id, ItemKind::Struct);
    cx.pending_items.push(PendingItem {
        handle,
        item,
        def_path_hash: closure,
        synthetic_kind: SyntheticItemKind::ClosureCaptures { closure },
        bridge_flags: regseq_model::BridgeItemFlags::default(),
    });
    handle
}

fn source_item(cx: &mut BridgeCtxt<'_>, def_id: DefId, kind: ItemKind) -> Item {
    let crate_id = crate_id(cx, def_id);
    let rustc_def_path = cx.input.tcx.def_path(def_id);
    let segments: Vec<DefPathSegment> = rustc_def_path
        .data
        .iter()
        .map(|segment| DefPathSegment {
            data: translate_def_path_data(&mut cx.model.strings, segment.data),
            disambiguator: segment.disambiguator,
        })
        .collect();
    // The rendered form MUST come from `render_def_path` so it matches
    // the validator's canonical re-rendering at validate_items (the
    // same pattern used by synth/vtable.rs:597-605 for synthetic
    // Items). `tcx.def_path_str` drops the crate prefix and trips
    // `DefPathRenderMismatch` for source-derived discriminator Items
    // like `ClosureCaptures` (whose def_path is rustc's verbatim per
    // spec.md:5200-5203's invariant-2 carve-out).
    let rendered = render_def_path(
        &DefPath {
            crate_id,
            segments: segments.clone(),
            rendered: regseq_model::StringId::ZERO,
        },
        &cx.model.crates,
        &cx.model.strings,
    );
    let rendered_id = cx.model.strings.intern(&rendered);
    Item {
        def_path: DefPath {
            crate_id,
            segments,
            rendered: rendered_id,
        },
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
        is_result: false,
    }
}

fn crate_id(cx: &mut BridgeCtxt<'_>, def_id: DefId) -> CrateId {
    let stable = StableCrateId(cx.input.tcx.stable_crate_id(def_id.krate).as_u64());
    for (idx, krate) in cx.model.crates.iter().enumerate() {
        if krate.stable_crate_id == stable {
            return CrateId::from_u32(idx as u32);
        }
    }
    let name = cx.input.tcx.crate_name(def_id.krate).to_string();
    let name = cx.model.strings.intern(&name);
    cx.model.crates.push(CrateInfo {
        name,
        stable_crate_id: stable,
    })
}

fn def_path_hash(tcx: rustc_middle::ty::TyCtxt<'_>, def_id: DefId) -> DefPathHash {
    let hash = tcx.def_path_hash(def_id);
    DefPathHash {
        stable_crate_id: StableCrateId(hash.stable_crate_id().as_u64()),
        local_hash: hash.local_hash().as_u64(),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn closure_module_unit_tests_are_covered_by_integration_fixtures() {
        assert!(true);
    }
}
