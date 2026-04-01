//! Collect DSL-level information from the HIR.
//!
//! Walks the HIR to find items annotated with `#[rseq::*]` attributes and
//! extracts function signatures, struct definitions, and attribute metadata
//! into rustc-independent [`HirSummary`] types.

use rustc_hir as hir;
use rustc_middle::ty::TyCtxt;

use super::types::{HirFuncInfo, HirStructInfo, HirSummary, RseqAttr};

/// Walk the HIR and collect all regseq-relevant items.
pub fn collect_hir(tcx: TyCtxt<'_>) -> HirSummary {
    let mut summary = HirSummary::default();

    for owner_id in tcx.hir_free_items() {
        let item = tcx.hir_item(owner_id);
        match &item.kind {
            hir::ItemKind::Fn { sig, .. } => {
                let def_id = item.owner_id.def_id;
                let name = tcx.def_path_str(def_id);
                let short_name = tcx.item_name(def_id.to_def_id()).to_string();
                let rseq_attrs = extract_rseq_attrs(tcx, item.hir_id());

                // Param/return types will come from MIR's LocalDecl, which
                // has resolved type information.  Store placeholder names
                // here; the decorated emitter merges them with MIR types.
                let params: Vec<(String, String)> = sig
                    .decl
                    .inputs
                    .iter()
                    .enumerate()
                    .map(|(i, _ty)| {
                        let pname = format!("arg{i}");
                        // Type will be filled from MIR locals.
                        (pname, String::new())
                    })
                    .collect();

                let return_ty = match sig.decl.output {
                    hir::FnRetTy::DefaultReturn(_) => "()".to_string(),
                    // Will be overridden from MIR local _0 type.
                    hir::FnRetTy::Return(_) => String::new(),
                };

                summary.functions.push(HirFuncInfo {
                    name,
                    short_name,
                    rseq_attrs,
                    params,
                    return_ty,
                });
            }
            hir::ItemKind::Struct(_, _, _) => {
                let def_id = item.owner_id.def_id;
                let name = tcx.item_name(def_id.to_def_id()).to_string();
                let rseq_attrs = extract_rseq_attrs(tcx, item.hir_id());

                // Get fields from the type-checked ADT definition.
                let adt_def = tcx.adt_def(def_id);
                let fields = if let Some(variant) = adt_def.variants().iter().next() {
                    variant
                        .fields
                        .iter()
                        .map(|f| {
                            let fname = f.name.to_string();
                            let fty =
                                tcx.type_of(f.did).instantiate_identity().to_string();
                            (fname, fty)
                        })
                        .collect()
                } else {
                    Vec::new()
                };

                summary.structs.push(HirStructInfo {
                    name,
                    rseq_attrs,
                    fields,
                });
            }
            _ => {}
        }
    }

    summary
}

/// Extract `#[rseq::*]` tool attributes from an item's attribute list.
fn extract_rseq_attrs(tcx: TyCtxt<'_>, hir_id: hir::HirId) -> Vec<RseqAttr> {
    let mut result = Vec::new();
    for attr in tcx.hir_attrs(hir_id) {
        // Tool attributes are `Unparsed` (not built-in parsed attributes).
        let hir::Attribute::Unparsed(unparsed) = attr else {
            continue;
        };
        let segments: Vec<String> = unparsed
            .path
            .segments
            .iter()
            .map(|s| s.to_string())
            .collect();
        if segments.first().map(String::as_str) == Some("rseq") {
            let kind = segments
                .get(1)
                .cloned()
                .unwrap_or_else(|| "unknown".to_string());
            let args = match &unparsed.args {
                hir::AttrArgs::Empty => None,
                hir::AttrArgs::Delimited(delim) => {
                    Some(format!("{:?}", delim.tokens))
                }
                hir::AttrArgs::Eq { expr, .. } => {
                    Some(format!("= {expr:?}"))
                }
            };
            result.push(RseqAttr { kind, args });
        }
    }
    result
}
