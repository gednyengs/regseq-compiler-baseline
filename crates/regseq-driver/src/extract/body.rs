//! Authored callback and helper body extraction via MIR.
//!
//! Uses HIR only for root discovery (driver type, callback methods, ISR
//! handlers, candidate inherent helpers). Executable body semantics are
//! extracted from rustc's `mir_drops_elaborated_and_const_checked` query
//! and converted to the owned [`crate::mir_repr`] types.

use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};

use rustc_hir::{ImplItemKind, ItemKind, QPath};
use rustc_middle::mir;
use rustc_middle::ty::TyCtxt;

use crate::mir_repr::*;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors from body extraction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BodyError {
    /// No `#[rseq::driver]`-annotated struct found.
    NoDriverType,
}

// ---------------------------------------------------------------------------
// Main extraction entry point
// ---------------------------------------------------------------------------

/// Extract MIR bodies for callbacks, ISRs, and reachable helpers.
///
/// Uses HIR to discover authored roots (callback methods, ISR handlers,
/// candidate inherent helpers), then extracts post-drop-elaboration MIR
/// from `mir_drops_elaborated_and_const_checked` for each root and
/// transitively reachable helpers.
pub fn extract_body_surface<'tcx>(tcx: TyCtxt<'tcx>) -> Result<MirBodySurface, BodyError> {
    let driver_type = find_driver_type_name(tcx).ok_or(BodyError::NoDriverType)?;
    let roots = discover_roots(tcx, &driver_type);

    // Build a map of all candidate helpers by name for transitive discovery.
    let candidate_helpers: HashMap<String, rustc_hir::OwnerId> = roots
        .helper_candidates
        .iter()
        .map(|(name, owner_id)| (name.clone(), *owner_id))
        .collect();

    // Start with callback and ISR roots.
    let mut work: VecDeque<(String, rustc_hir::OwnerId, MirFnKind)> = VecDeque::new();
    for root in &roots.callbacks {
        work.push_back((root.name.clone(), root.owner_id, root.kind.clone()));
    }
    for root in &roots.isrs {
        work.push_back((root.name.clone(), root.owner_id, MirFnKind::Isr));
    }

    let mut visited: HashSet<String> = HashSet::new();
    let mut functions: Vec<MirExtractedFn> = Vec::new();

    while let Some((name, owner_id, kind)) = work.pop_front() {
        if !visited.insert(name.clone()) {
            continue;
        }

        let local_def_id = owner_id.def_id;
        let def_path = tcx.def_path_str(local_def_id.to_def_id());
        let stolen = tcx.mir_drops_elaborated_and_const_checked(local_def_id);
        let body_ref = stolen.borrow();

        let mir_body = super::mir_extract::convert_body(tcx, &body_ref);
        let helper_calls = discover_called_helpers(tcx, &body_ref, &candidate_helpers);

        // Enqueue newly discovered helpers.
        for helper_name in &helper_calls {
            if !visited.contains(helper_name) {
                if let Some(helper_owner) = candidate_helpers.get(helper_name) {
                    work.push_back((helper_name.clone(), *helper_owner, MirFnKind::Helper));
                }
            }
        }

        let (receiver, params, return_type) = extract_fn_signature(tcx, owner_id);

        functions.push(MirExtractedFn {
            name,
            def_path,
            kind,
            receiver,
            params,
            return_type,
            body: mir_body,
            helper_calls,
        });
    }

    Ok(MirBodySurface { functions })
}

// ---------------------------------------------------------------------------
// HIR root discovery
// ---------------------------------------------------------------------------

struct DiscoveredRoots {
    callbacks: Vec<FnRoot>,
    isrs: Vec<FnRoot>,
    helper_candidates: Vec<(String, rustc_hir::OwnerId)>,
}

struct FnRoot {
    name: String,
    owner_id: rustc_hir::OwnerId,
    kind: MirFnKind,
}

fn discover_roots(tcx: TyCtxt<'_>, driver_type: &str) -> DiscoveredRoots {
    let mut callbacks = Vec::new();
    let mut isrs = Vec::new();
    let mut helper_candidates = Vec::new();

    for item_id in tcx.hir_free_items() {
        let item = tcx.hir_item(item_id);
        let ItemKind::Impl(impl_block) = &item.kind else {
            continue;
        };
        let Some(self_name) = self_type_name(impl_block.self_ty) else {
            continue;
        };
        if self_name != driver_type {
            continue;
        }

        if let Some(trait_ref) = &impl_block.of_trait {
            let trait_name = resolve_trait_name(tcx, trait_ref.trait_ref.path);
            for item_ref in impl_block.items {
                let impl_item = tcx.hir_impl_item(rustc_hir::ImplItemId {
                    owner_id: item_ref.owner_id,
                });
                if let ImplItemKind::Fn(..) = &impl_item.kind {
                    let method_name = impl_item.ident.name.as_str().to_string();
                    callbacks.push(FnRoot {
                        name: method_name.clone(),
                        owner_id: item_ref.owner_id,
                        kind: MirFnKind::Callback {
                            trait_name: trait_name.clone(),
                            method_name,
                        },
                    });
                }
            }
        } else {
            for item_ref in impl_block.items {
                let impl_item = tcx.hir_impl_item(rustc_hir::ImplItemId {
                    owner_id: item_ref.owner_id,
                });
                if let ImplItemKind::Fn(..) = &impl_item.kind {
                    let name = impl_item.ident.name.as_str().to_string();
                    let attrs = tcx.hir_attrs(impl_item.hir_id());
                    let is_isr = attrs.iter().any(|a| rseq_attr_name(a) == Some("isr"));
                    if is_isr {
                        isrs.push(FnRoot {
                            name: name.clone(),
                            owner_id: item_ref.owner_id,
                            kind: MirFnKind::Isr,
                        });
                    }
                    // All inherent methods are helper candidates.
                    helper_candidates.push((name, item_ref.owner_id));
                }
            }
        }
    }

    DiscoveredRoots {
        callbacks,
        isrs,
        helper_candidates,
    }
}

// ---------------------------------------------------------------------------
// MIR-based helper reachability
// ---------------------------------------------------------------------------

/// Discover names of locally-defined helpers called from a MIR body.
fn discover_called_helpers<'tcx>(
    tcx: TyCtxt<'tcx>,
    body: &mir::Body<'tcx>,
    candidates: &HashMap<String, rustc_hir::OwnerId>,
) -> Vec<String> {
    let mut called: BTreeSet<String> = BTreeSet::new();

    for bb in body.basic_blocks.iter() {
        let Some(term) = &bb.terminator else { continue };
        if let mir::TerminatorKind::Call {
            func: mir::Operand::Constant(box const_op),
            ..
        } = &term.kind
        {
            if let mir::Const::Val(_, ty) = const_op.const_ {
                if let rustc_middle::ty::TyKind::FnDef(def_id, _) = ty.kind() {
                    if let Some(local_def_id) = def_id.as_local() {
                        let fn_name = tcx.item_name(local_def_id.to_def_id());
                        let name = fn_name.to_string();
                        if candidates.contains_key(&name) {
                            called.insert(name);
                        }
                    }
                }
            }
        }
    }

    called.into_iter().collect()
}

// ---------------------------------------------------------------------------
// Signature extraction from HIR
// ---------------------------------------------------------------------------

fn extract_fn_signature(
    tcx: TyCtxt<'_>,
    owner_id: rustc_hir::OwnerId,
) -> (String, Vec<MirFnParam>, Option<String>) {
    let impl_item = tcx.hir_impl_item(rustc_hir::ImplItemId { owner_id });
    let ImplItemKind::Fn(ref sig, body_id) = impl_item.kind else {
        return ("none".to_string(), vec![], None);
    };
    let body = tcx.hir_body(body_id);
    let has_self = !sig.decl.inputs.is_empty() && is_self_ty(&sig.decl.inputs[0]);
    let receiver = if has_self {
        receiver_label(sig.decl)
    } else {
        "none".to_string()
    };
    let skip = if has_self { 1 } else { 0 };
    let params: Vec<MirFnParam> = body
        .params
        .iter()
        .skip(skip)
        .zip(sig.decl.inputs.iter().skip(skip))
        .map(|(param, ty)| MirFnParam {
            name: render_pat(param.pat),
            ty: render_hir_ty(ty),
        })
        .collect();
    let return_type = match &sig.decl.output {
        rustc_hir::FnRetTy::DefaultReturn(_) => None,
        rustc_hir::FnRetTy::Return(ty) => {
            let r = render_hir_ty(ty);
            if r == "()" {
                None
            } else {
                Some(r)
            }
        }
    };
    (receiver, params, return_type)
}

fn receiver_label(decl: &rustc_hir::FnDecl<'_>) -> String {
    if decl.inputs.is_empty() {
        return "none".to_string();
    }
    if let rustc_hir::TyKind::Ref(_, mut_ty) = &decl.inputs[0].kind {
        if mut_ty.mutbl.is_mut() {
            "ref_mut"
        } else {
            "ref"
        }
        .to_string()
    } else {
        "none".to_string()
    }
}

// ---------------------------------------------------------------------------
// Driver type and trait name helpers
// ---------------------------------------------------------------------------

fn find_driver_type_name(tcx: TyCtxt<'_>) -> Option<String> {
    for item_id in tcx.hir_free_items() {
        let item = tcx.hir_item(item_id);
        let attrs = tcx.hir_attrs(item.hir_id());
        if attrs.iter().any(|a| rseq_attr_name(a) == Some("driver")) {
            if let ItemKind::Struct(ident, ..) = &item.kind {
                return Some(ident.name.as_str().to_string());
            }
        }
    }
    None
}

fn resolve_trait_name<'tcx>(tcx: TyCtxt<'tcx>, path: &rustc_hir::Path<'tcx>) -> String {
    let def_id = match path.res {
        rustc_hir::def::Res::Def(_, def_id) => def_id,
        _ => {
            return path
                .segments
                .last()
                .map(|s| s.ident.name.as_str().to_string())
                .unwrap_or_default();
        }
    };
    let full = tcx.def_path_str(def_id);
    full.rsplit("::").next().unwrap_or(&full).to_string()
}

fn self_type_name(ty: &rustc_hir::Ty<'_>) -> Option<String> {
    if let rustc_hir::TyKind::Path(QPath::Resolved(_, path)) = &ty.kind {
        path.segments
            .last()
            .map(|s| s.ident.name.as_str().to_string())
    } else {
        None
    }
}

fn rseq_attr_name(attr: &rustc_hir::Attribute) -> Option<&str> {
    let rustc_hir::Attribute::Unparsed(item) = attr else {
        return None;
    };
    let segs = &item.path.segments;
    if segs.len() == 2 && segs[0].as_str() == "rseq" {
        Some(segs[1].as_str())
    } else {
        None
    }
}

fn is_self_ty(ty: &rustc_hir::Ty<'_>) -> bool {
    matches!(&ty.kind, rustc_hir::TyKind::Ref(_, _))
        || matches!(
            &ty.kind,
            rustc_hir::TyKind::Path(QPath::Resolved(_, path))
            if path.segments.last().map(|s| s.ident.name.as_str() == "Self").unwrap_or(false)
        )
}

fn render_pat(pat: &rustc_hir::Pat<'_>) -> String {
    match &pat.kind {
        rustc_hir::PatKind::Binding(_, _, ident, _) => ident.name.as_str().to_string(),
        rustc_hir::PatKind::Wild => "_".to_string(),
        _ => "<pattern>".to_string(),
    }
}

fn render_hir_ty<A>(ty: &rustc_hir::Ty<'_, A>) -> String {
    match &ty.kind {
        rustc_hir::TyKind::Path(qpath) => render_qpath(qpath),
        rustc_hir::TyKind::Ref(_, mut_ty) => {
            let m = if mut_ty.mutbl.is_mut() { "mut " } else { "" };
            format!("&{m}{}", render_hir_ty(mut_ty.ty))
        }
        rustc_hir::TyKind::Slice(elem) => format!("[{}]", render_hir_ty(elem)),
        rustc_hir::TyKind::Tup([]) => "()".to_string(),
        rustc_hir::TyKind::Tup(elems) => {
            let inner: Vec<_> = elems.iter().map(|e| render_hir_ty(e)).collect();
            format!("({})", inner.join(", "))
        }
        rustc_hir::TyKind::Never => "!".to_string(),
        _ => "<type>".to_string(),
    }
}

fn render_qpath(qpath: &QPath<'_>) -> String {
    match qpath {
        QPath::Resolved(_, path) => path
            .segments
            .iter()
            .map(|s| s.ident.name.as_str())
            .collect::<Vec<_>>()
            .join("::"),
        QPath::TypeRelative(ty, segment) => {
            format!("{}::{}", render_hir_ty(ty), segment.ident.name.as_str())
        }
    }
}
