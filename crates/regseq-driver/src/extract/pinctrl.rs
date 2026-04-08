use rustc_ast::token::{LitKind, TokenKind};
use rustc_ast::tokenstream::TokenTree;
use rustc_hir::intravisit::{self, Visitor};
use rustc_hir::{AttrArgs, Attribute, ExprKind, ImplItemKind, ItemKind};
use rustc_middle::ty::{Ty, TyCtxt};

// ---------------------------------------------------------------------------
// Record types
// ---------------------------------------------------------------------------

/// Top-level extraction result for the PinCtrl resource surface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PinCtrlSurface {
    pub fields: Vec<PinCtrlField>,
    pub call_sites: Vec<PinCtrlCallSite>,
}

/// A `PinCtrl` or `Option<PinCtrl>` field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PinCtrlField {
    pub field_name: String,
    pub required: bool,
    pub pinctrl_id: Option<String>,
    /// Exact rendered Rust type of this field (e.g. `"PinCtrl"`, `"Option<PinCtrl>"`).
    pub field_type: String,
}

/// A recognized `apply::<State>()` call site.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PinCtrlCallSite {
    pub state_type: String,
    pub containing_fn: String,
}

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PinCtrlError {
    MalformedAttribute(String),
}

// ---------------------------------------------------------------------------
// Main extraction entry point
// ---------------------------------------------------------------------------

/// Extract the PinCtrl resource surface from HIR after analysis.
pub fn extract_pinctrl_surface<'tcx>(tcx: TyCtxt<'tcx>) -> Result<PinCtrlSurface, PinCtrlError> {
    let mut fields = Vec::new();
    let mut call_sites = Vec::new();

    for item_id in tcx.hir_free_items() {
        let item = tcx.hir_item(item_id);

        match &item.kind {
            ItemKind::Struct(_ident, _generics, variant_data) => {
                let extracted = extract_pinctrl_fields(tcx, variant_data)?;
                fields.extend(extracted);
            }
            ItemKind::Impl(impl_block) => {
                for impl_item_ref in impl_block.items {
                    let impl_item = tcx.hir_impl_item(rustc_hir::ImplItemId {
                        owner_id: impl_item_ref.owner_id,
                    });
                    collect_pinctrl_call_sites(tcx, impl_item, &mut call_sites);
                }
            }
            _ => {}
        }
    }

    Ok(PinCtrlSurface { fields, call_sites })
}

// ---------------------------------------------------------------------------
// PinCtrl field extraction
// ---------------------------------------------------------------------------

fn extract_pinctrl_fields<'tcx>(
    tcx: TyCtxt<'tcx>,
    variant_data: &rustc_hir::VariantData<'tcx>,
) -> Result<Vec<PinCtrlField>, PinCtrlError> {
    let mut fields = Vec::new();

    for field in variant_data.fields() {
        let required = match pinctrl_field_shape(field.ty) {
            Some(r) => r,
            None => continue,
        };

        let attrs = tcx.hir_attrs(field.hir_id);
        let pinctrl_attr = attrs.iter().find(|a| rseq_attr_name(a) == Some("pinctrl"));

        let pinctrl_id = match pinctrl_attr {
            Some(attr) => parse_pinctrl_attr(attr)?,
            None => None,
        };

        fields.push(PinCtrlField {
            field_name: field.ident.name.as_str().to_string(),
            required,
            pinctrl_id,
            field_type: hir_ty_to_string(field.ty),
        });
    }

    Ok(fields)
}

/// Returns `Some(required)` if this is a `PinCtrl` or `Option<PinCtrl>` type.
fn pinctrl_field_shape(ty: &rustc_hir::Ty<'_>) -> Option<bool> {
    let rustc_hir::TyKind::Path(qpath) = &ty.kind else {
        return None;
    };
    let path = match qpath {
        rustc_hir::QPath::Resolved(_, path) => path,
        _ => return None,
    };
    let last_seg = path.segments.last()?;
    let name = last_seg.ident.name.as_str();

    if name == "PinCtrl" {
        return Some(true);
    }

    if name == "Option" {
        let args = last_seg.args?;
        for arg in args.args.iter() {
            if let rustc_hir::GenericArg::Type(inner_ty) = arg {
                if let rustc_hir::TyKind::Path(rustc_hir::QPath::Resolved(_, p)) = &inner_ty.kind {
                    if p.segments.last().map(|s| s.ident.name.as_str()) == Some("PinCtrl") {
                        return Some(false);
                    }
                }
            }
        }
    }

    None
}

// ---------------------------------------------------------------------------
// Attribute parsing
// ---------------------------------------------------------------------------

fn parse_pinctrl_attr(attr: &Attribute) -> Result<Option<String>, PinCtrlError> {
    let Attribute::Unparsed(item) = attr else {
        return Err(malformed("expected tool attribute"));
    };
    let tokens = match &item.args {
        AttrArgs::Delimited(d) => &d.tokens,
        _ => return Ok(None),
    };

    let trees: Vec<&TokenTree> = tokens.iter().collect();
    let mut pinctrl_id = None;
    let mut i = 0;

    while i < trees.len() {
        if is_comma(trees[i]) {
            i += 1;
            continue;
        }

        let key = extract_ident(trees[i])
            .ok_or_else(|| malformed("expected key in pinctrl attribute"))?;
        i += 1;

        if i >= trees.len() || !is_eq(trees[i]) {
            return Err(malformed(&format!("expected '=' after '{key}'")));
        }
        i += 1;

        if i >= trees.len() {
            return Err(malformed(&format!("expected value for '{key}'")));
        }

        if key == "id" {
            pinctrl_id = extract_string_literal(trees[i]);
        }
        i += 1;
    }

    Ok(pinctrl_id)
}

// ---------------------------------------------------------------------------
// Call-site collection
// ---------------------------------------------------------------------------

fn collect_pinctrl_call_sites<'tcx>(
    tcx: TyCtxt<'tcx>,
    impl_item: &'tcx rustc_hir::ImplItem<'tcx>,
    sites: &mut Vec<PinCtrlCallSite>,
) {
    if let ImplItemKind::Fn(_sig, body_id) = impl_item.kind {
        let fn_name = impl_item.ident.name.as_str().to_string();
        let body = tcx.hir_body(body_id);
        let typeck_results = tcx.typeck(impl_item.owner_id.def_id);
        let mut visitor = PinCtrlCallSiteVisitor {
            tcx,
            typeck_results,
            current_fn: fn_name,
            sites: Vec::new(),
        };
        visitor.visit_expr(body.value);
        sites.extend(visitor.sites);
    }
}

struct PinCtrlCallSiteVisitor<'tcx> {
    tcx: TyCtxt<'tcx>,
    typeck_results: &'tcx rustc_middle::ty::TypeckResults<'tcx>,
    current_fn: String,
    sites: Vec<PinCtrlCallSite>,
}

impl<'tcx> Visitor<'tcx> for PinCtrlCallSiteVisitor<'tcx> {
    fn visit_expr(&mut self, expr: &'tcx rustc_hir::Expr<'tcx>) {
        if let ExprKind::MethodCall(segment, receiver, _, _) = expr.kind {
            if segment.ident.name.as_str() == "apply"
                && is_pinctrl_type(self.tcx, self.typeck_results.expr_ty(receiver))
            {
                if let Some(state_name) = extract_turbofish_type_name(segment) {
                    self.sites.push(PinCtrlCallSite {
                        state_type: state_name,
                        containing_fn: self.current_fn.clone(),
                    });
                }
            }
        }
        intravisit::walk_expr(self, expr);
    }
}

/// Extract the type name from a turbofish generic argument on a method call
/// segment, e.g. `apply::<DefaultState>` -> `"DefaultState"`.
fn extract_turbofish_type_name(segment: &rustc_hir::PathSegment<'_>) -> Option<String> {
    let args = segment.args?;
    for arg in args.args.iter() {
        if let rustc_hir::GenericArg::Type(ty) = arg {
            if let rustc_hir::TyKind::Path(rustc_hir::QPath::Resolved(_, path)) = &ty.kind {
                return Some(
                    path.segments
                        .iter()
                        .map(|s| s.ident.name.as_str().to_string())
                        .collect::<Vec<_>>()
                        .join("::"),
                );
            }
        }
    }
    None
}

fn is_pinctrl_type<'tcx>(tcx: TyCtxt<'tcx>, ty: Ty<'tcx>) -> bool {
    ty.peel_refs()
        .ty_adt_def()
        .map(|adt| tcx.item_name(adt.did()).as_str() == "PinCtrl")
        .unwrap_or(false)
}

// ---------------------------------------------------------------------------
// HIR type rendering
// ---------------------------------------------------------------------------

/// Produce a recoverable string representation of a HIR type.
fn hir_ty_to_string(ty: &rustc_hir::Ty<'_>) -> String {
    let rustc_hir::TyKind::Path(qpath) = &ty.kind else {
        return "<unsupported>".to_string();
    };
    let path = match qpath {
        rustc_hir::QPath::Resolved(_, path) => path,
        _ => return "<unsupported>".to_string(),
    };

    let mut parts = Vec::new();
    for seg in path.segments.iter() {
        let mut s = seg.ident.name.as_str().to_string();
        if let Some(args) = seg.args {
            let inner: Vec<String> = args
                .args
                .iter()
                .filter_map(|arg| {
                    if let rustc_hir::GenericArg::Type(inner_ty) = arg {
                        if let rustc_hir::TyKind::Path(rustc_hir::QPath::Resolved(_, p)) =
                            &inner_ty.kind
                        {
                            Some(
                                p.segments
                                    .iter()
                                    .map(|s| s.ident.name.as_str().to_string())
                                    .collect::<Vec<_>>()
                                    .join("::"),
                            )
                        } else {
                            Some("<unsupported>".to_string())
                        }
                    } else {
                        None
                    }
                })
                .collect();
            if !inner.is_empty() {
                s.push('<');
                s.push_str(&inner.join(", "));
                s.push('>');
            }
        }
        parts.push(s);
    }

    parts.join("::")
}

// ---------------------------------------------------------------------------
// Token helpers
// ---------------------------------------------------------------------------

fn rseq_attr_name(attr: &Attribute) -> Option<&str> {
    let Attribute::Unparsed(item) = attr else {
        return None;
    };
    let segs = &item.path.segments;
    if segs.len() == 2 && segs[0].as_str() == "rseq" {
        Some(segs[1].as_str())
    } else {
        None
    }
}

fn malformed(msg: &str) -> PinCtrlError {
    PinCtrlError::MalformedAttribute(msg.to_string())
}

fn is_comma(tree: &TokenTree) -> bool {
    matches!(tree, TokenTree::Token(tok, _) if matches!(tok.kind, TokenKind::Comma))
}

fn is_eq(tree: &TokenTree) -> bool {
    matches!(tree, TokenTree::Token(tok, _) if matches!(tok.kind, TokenKind::Eq))
}

fn extract_ident(tree: &TokenTree) -> Option<String> {
    match tree {
        TokenTree::Token(tok, _) => match tok.kind {
            TokenKind::Ident(sym, _) => Some(sym.as_str().to_string()),
            _ => None,
        },
        _ => None,
    }
}

fn extract_string_literal(tree: &TokenTree) -> Option<String> {
    match tree {
        TokenTree::Token(tok, _) => match tok.kind {
            TokenKind::Literal(lit) if lit.kind == LitKind::Str => {
                Some(lit.symbol.as_str().to_string())
            }
            _ => None,
        },
        _ => None,
    }
}
