use rustc_hir::intravisit::{self, Visitor};
use rustc_hir::{ExprKind, ImplItemKind, ItemKind};
use rustc_middle::ty::{Ty, TyCtxt};

// ---------------------------------------------------------------------------
// Record types
// ---------------------------------------------------------------------------

/// Top-level extraction result for the ResetLine resource surface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResetSurface {
    pub fields: Vec<ResetField>,
    pub call_sites: Vec<ResetCallSite>,
}

/// A `ResetLine` or `Option<ResetLine>` field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResetField {
    pub field_name: String,
    pub required: bool,
}

/// An accepted reset wrapper-method identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResetMethod {
    Assert,
    Deassert,
    Toggle,
}

/// A wrapper-method call site detected in a function body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResetCallSite {
    pub method: ResetMethod,
    pub containing_fn: String,
}

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResetError {
    ToggleArgNotDuration(String),
}

// ---------------------------------------------------------------------------
// Main extraction entry point
// ---------------------------------------------------------------------------

/// Extract the ResetLine resource surface from HIR after analysis.
pub fn extract_reset_surface<'tcx>(tcx: TyCtxt<'tcx>) -> Result<ResetSurface, ResetError> {
    let mut fields = Vec::new();
    let mut call_sites = Vec::new();

    for item_id in tcx.hir_free_items() {
        let item = tcx.hir_item(item_id);

        match &item.kind {
            ItemKind::Struct(_ident, _generics, variant_data) => {
                let extracted = extract_reset_fields(variant_data);
                fields.extend(extracted);
            }
            ItemKind::Impl(impl_block) => {
                for impl_item_ref in impl_block.items {
                    let impl_item = tcx.hir_impl_item(rustc_hir::ImplItemId {
                        owner_id: impl_item_ref.owner_id,
                    });
                    collect_reset_call_sites(tcx, impl_item, &mut call_sites)?;
                }
            }
            _ => {}
        }
    }

    Ok(ResetSurface { fields, call_sites })
}

// ---------------------------------------------------------------------------
// ResetLine field extraction
// ---------------------------------------------------------------------------

fn extract_reset_fields<'tcx>(variant_data: &rustc_hir::VariantData<'tcx>) -> Vec<ResetField> {
    let mut fields = Vec::new();

    for field in variant_data.fields() {
        let required = match reset_field_shape(field.ty) {
            Some(r) => r,
            None => continue,
        };

        fields.push(ResetField {
            field_name: field.ident.name.as_str().to_string(),
            required,
        });
    }

    fields
}

/// Returns `Some(required)` if this is a `ResetLine` or `Option<ResetLine>` type.
fn reset_field_shape(ty: &rustc_hir::Ty<'_>) -> Option<bool> {
    let rustc_hir::TyKind::Path(qpath) = &ty.kind else {
        return None;
    };
    let path = match qpath {
        rustc_hir::QPath::Resolved(_, path) => path,
        _ => return None,
    };
    let last_seg = path.segments.last()?;
    let name = last_seg.ident.name.as_str();

    if name == "ResetLine" {
        return Some(true);
    }

    if name == "Option" {
        let args = last_seg.args?;
        for arg in args.args.iter() {
            if let rustc_hir::GenericArg::Type(inner_ty) = arg {
                if let rustc_hir::TyKind::Path(rustc_hir::QPath::Resolved(_, p)) = &inner_ty.kind {
                    if p.segments.last().map(|s| s.ident.name.as_str()) == Some("ResetLine") {
                        return Some(false);
                    }
                }
            }
        }
    }

    None
}

// ---------------------------------------------------------------------------
// Call-site collection
// ---------------------------------------------------------------------------

fn collect_reset_call_sites<'tcx>(
    tcx: TyCtxt<'tcx>,
    impl_item: &'tcx rustc_hir::ImplItem<'tcx>,
    sites: &mut Vec<ResetCallSite>,
) -> Result<(), ResetError> {
    if let ImplItemKind::Fn(_sig, body_id) = impl_item.kind {
        let fn_name = impl_item.ident.name.as_str().to_string();
        let body = tcx.hir_body(body_id);
        let typeck_results = tcx.typeck(impl_item.owner_id.def_id);
        let mut visitor = ResetCallSiteVisitor {
            tcx,
            typeck_results,
            current_fn: fn_name,
            sites: Vec::new(),
            error: None,
        };
        visitor.visit_expr(body.value);
        if let Some(err) = visitor.error {
            return Err(err);
        }
        sites.extend(visitor.sites);
    }
    Ok(())
}

struct ResetCallSiteVisitor<'tcx> {
    tcx: TyCtxt<'tcx>,
    typeck_results: &'tcx rustc_middle::ty::TypeckResults<'tcx>,
    current_fn: String,
    sites: Vec<ResetCallSite>,
    error: Option<ResetError>,
}

impl<'tcx> Visitor<'tcx> for ResetCallSiteVisitor<'tcx> {
    fn visit_expr(&mut self, expr: &'tcx rustc_hir::Expr<'tcx>) {
        if self.error.is_some() {
            return;
        }

        if let ExprKind::MethodCall(segment, receiver, args, _) = expr.kind {
            let method_name = segment.ident.name.as_str();
            let method = match method_name {
                "assert" => Some(ResetMethod::Assert),
                "deassert" => Some(ResetMethod::Deassert),
                "toggle" => Some(ResetMethod::Toggle),
                _ => None,
            };

            if let Some(method) = method {
                if is_resetline_type(self.tcx, self.typeck_results.expr_ty(receiver)) {
                    if method == ResetMethod::Toggle {
                        if let Some(arg) = args.first() {
                            let arg_ty = self.typeck_results.expr_ty(arg);
                            if !is_duration_type(self.tcx, arg_ty) {
                                self.error =
                                    Some(ResetError::ToggleArgNotDuration(self.current_fn.clone()));
                                return;
                            }
                        }
                    }
                    self.sites.push(ResetCallSite {
                        method,
                        containing_fn: self.current_fn.clone(),
                    });
                }
            }
        }
        intravisit::walk_expr(self, expr);
    }
}

fn is_resetline_type<'tcx>(tcx: TyCtxt<'tcx>, ty: Ty<'tcx>) -> bool {
    ty.peel_refs()
        .ty_adt_def()
        .map(|adt| tcx.item_name(adt.did()).as_str() == "ResetLine")
        .unwrap_or(false)
}

fn is_duration_type<'tcx>(tcx: TyCtxt<'tcx>, ty: Ty<'tcx>) -> bool {
    ty.peel_refs()
        .ty_adt_def()
        .map(|adt| tcx.item_name(adt.did()).as_str() == "Duration")
        .unwrap_or(false)
}
