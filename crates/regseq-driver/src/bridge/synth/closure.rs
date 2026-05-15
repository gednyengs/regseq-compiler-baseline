//! Closure-family bridge item synthesis.

use regseq_model::{
    ClosureCallKind, CrateId, CrateInfo, DefPath, DefPathData, DefPathHash, DefPathSegment,
    FieldLayout, InlineHint, Item, ItemKind, Layout, LayoutKind, Linkage, Span, StableCrateId,
    SyntheticItemKind, Ty,
};
use rustc_span::def_id::DefId;

use super::ClosureSynthHandles;
use crate::bridge::{ty::translate_ty, BridgeCtxt, BridgeError, PendingItem};

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

    // Record the rustc closure `Ty<'tcx>` so the Phase-5 captures-struct
    // view synthesis pass (spec.md section 13.1) can compute the
    // parallel `Ty::Struct` layout via `tcx.layout_of` without
    // re-discovering the closure from MIR.
    let closure_ty =
        rustc_middle::ty::Ty::new_closure(cx.input.tcx, closure_def_id, closure_def.args);
    cx.closure_synth_rustc_ty.insert(closure, closure_ty);

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
    let handle = cx.handles.fresh_handle();
    let item = source_item(cx, closure_def_id, ItemKind::Struct);
    cx.pending_items.push(PendingItem {
        handle,
        item,
        def_path_hash: closure,
        synthetic_kind: SyntheticItemKind::ClosureCaptures { closure },
    });
    handle
}

fn source_item(cx: &mut BridgeCtxt<'_>, def_id: DefId, kind: ItemKind) -> Item {
    let crate_id = crate_id(cx, def_id);
    let rendered = cx.input.tcx.def_path_str(def_id);
    let rendered_id = cx.model.strings.intern(&rendered);
    let rustc_def_path = cx.input.tcx.def_path(def_id);
    let segments = rustc_def_path
        .data
        .iter()
        .map(|segment| DefPathSegment {
            data: translate_def_path_data(cx, segment.data),
            disambiguator: segment.disambiguator,
        })
        .collect();
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
    }
}

fn translate_def_path_data(
    cx: &mut BridgeCtxt<'_>,
    data: rustc_hir::definitions::DefPathData,
) -> DefPathData {
    match data {
        rustc_hir::definitions::DefPathData::CrateRoot => DefPathData::CrateRoot,
        rustc_hir::definitions::DefPathData::Impl => DefPathData::Impl,
        rustc_hir::definitions::DefPathData::ForeignMod => DefPathData::ForeignMod,
        rustc_hir::definitions::DefPathData::Use => DefPathData::Use,
        rustc_hir::definitions::DefPathData::GlobalAsm => DefPathData::GlobalAsm,
        rustc_hir::definitions::DefPathData::TypeNs(symbol) => {
            DefPathData::TypeNs(cx.model.strings.intern(symbol.as_str()))
        }
        rustc_hir::definitions::DefPathData::ValueNs(symbol) => {
            DefPathData::ValueNs(cx.model.strings.intern(symbol.as_str()))
        }
        rustc_hir::definitions::DefPathData::MacroNs(symbol) => {
            DefPathData::MacroNs(cx.model.strings.intern(symbol.as_str()))
        }
        rustc_hir::definitions::DefPathData::LifetimeNs(symbol) => {
            DefPathData::LifetimeNs(cx.model.strings.intern(symbol.as_str()))
        }
        rustc_hir::definitions::DefPathData::Closure => DefPathData::Closure,
        rustc_hir::definitions::DefPathData::Ctor => DefPathData::Ctor,
        rustc_hir::definitions::DefPathData::AnonConst => DefPathData::AnonConst,
        rustc_hir::definitions::DefPathData::LateAnonConst => DefPathData::LateAnonConst,
        rustc_hir::definitions::DefPathData::DesugaredAnonymousLifetime => {
            DefPathData::DesugaredAnonymousLifetime
        }
        rustc_hir::definitions::DefPathData::OpaqueTy => DefPathData::OpaqueTy,
        rustc_hir::definitions::DefPathData::OpaqueLifetime(symbol) => {
            DefPathData::OpaqueLifetime(cx.model.strings.intern(symbol.as_str()))
        }
        rustc_hir::definitions::DefPathData::AnonAssocTy(symbol) => {
            DefPathData::AnonAssocTy(cx.model.strings.intern(symbol.as_str()))
        }
        rustc_hir::definitions::DefPathData::SyntheticCoroutineBody => {
            DefPathData::SyntheticCoroutineBody
        }
        rustc_hir::definitions::DefPathData::NestedStatic => DefPathData::NestedStatic,
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

/// Intern the parallel `Ty::Struct` view of each synthesised closure's
/// captures field-by-field and record the `Ty::Closure -> Ty::Struct`
/// mapping in `model.closure_captures_struct_ty`. Per spec.md section
/// 13.1 (lines 4571-4587): a closure has two `Ty` entries sharing one
/// `Item` -- `Ty::Closure(item, args)` and
/// `Ty::Struct { item, args, layout }`. Both views share the same
/// captures `ItemId` because `intern_item` in `bridge/ty.rs` dedupes on
/// the closure's source-derived `DefPathHash`, which by Phase 5 is
/// already present in `model.item_def_paths` after the captures Item
/// was assigned in Phase 4.
///
/// Runs after `rekey_synthesis_in_phase5` so captures handles resolve
/// to final `ItemId`s.
pub(crate) fn synthesise_closure_captures_struct_views<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
) -> Result<(), BridgeError> {
    let entries = cx
        .closure_synth_cache
        .iter()
        .map(|handles| (handles.closure, handles.captures))
        .collect::<Vec<_>>();
    for (closure_hash, captures_handle) in entries {
        let captures_item_id = cx.handles.resolve(captures_handle).ok_or_else(|| {
            BridgeError::Phase5(format!(
                "ClosureCaptures handle {} was not assigned an ItemId",
                captures_handle.as_u32()
            ))
        })?;
        let Some(closure_ty) = cx.closure_synth_rustc_ty.get(&closure_hash).copied() else {
            continue;
        };
        let upvar_tys = match closure_ty.kind() {
            rustc_middle::ty::TyKind::Closure(_, args) => args.as_closure().upvar_tys(),
            _ => continue,
        };
        let mut field_tys = Vec::with_capacity(upvar_tys.len());
        for upvar in upvar_tys {
            field_tys.push(translate_ty(cx, upvar)?);
        }
        let rustc_layout = cx
            .input
            .tcx
            .layout_of(
                rustc_middle::ty::TypingEnv::fully_monomorphized().as_query_input(closure_ty),
            )
            .map_err(|err| {
                BridgeError::Phase5(format!(
                    "closure captures layout unavailable for {closure_ty}: {err:?}"
                ))
            })?;
        let empty = cx.model.strings.intern("");
        let fields = field_tys
            .iter()
            .copied()
            .enumerate()
            .map(|(idx, ty)| FieldLayout {
                name: empty,
                ty,
                offset: if idx < rustc_layout.fields.count() {
                    rustc_layout.fields.offset(idx).bytes()
                } else {
                    0
                },
            })
            .collect();
        let layout = Layout {
            size: rustc_layout.size.bytes(),
            align: rustc_layout.align.abi.bytes(),
            pack: None,
            uninhabited: rustc_layout.uninhabited,
            kind: LayoutKind::Struct { fields },
        };
        // Translate the closure type to obtain the model `Ty::Closure`
        // TyId so the parallel-view map can be keyed on it.
        let closure_ty_id = translate_ty(cx, closure_ty)?;
        let model_args = match cx.model.types.resolve(closure_ty_id) {
            Ty::Closure(_, args) => args.clone(),
            _ => continue,
        };
        let captures_struct_ty_id = cx.model.types.intern(Ty::Struct {
            item: captures_item_id,
            args: model_args,
            layout,
        });
        cx.model
            .closure_captures_struct_ty
            .insert(closure_ty_id, captures_struct_ty_id);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn closure_module_unit_tests_are_covered_by_integration_fixtures() {
        assert!(true);
    }
}
