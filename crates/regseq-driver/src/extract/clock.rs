use rustc_ast::token::{LitKind, TokenKind};
use rustc_ast::tokenstream::TokenTree;
use rustc_hir::intravisit::{self, Visitor};
use rustc_hir::{AttrArgs, Attribute, ExprKind, ImplItemKind, ItemKind};
use rustc_middle::ty::{Ty, TyCtxt};

// ---------------------------------------------------------------------------
// Record types
// ---------------------------------------------------------------------------

/// Top-level extraction result for the Clock resource surface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClockSurface {
    pub fields: Vec<ClockField>,
    pub call_sites: Vec<ClockCallSite>,
}

/// A `Clock` or `Option<Clock>` field with `#[rseq::clock(id = "...")]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClockField {
    pub field_name: String,
    pub clock_id: String,
    pub required: bool,
}

/// An accepted clock wrapper-method call site.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClockMethod {
    Enable,
    Disable,
    GetRate,
}

/// A wrapper-method call site detected in a function body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClockCallSite {
    pub method: ClockMethod,
    pub containing_fn: String,
}

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClockError {
    MalformedAttribute(String),
    MissingClockId(String),
}

// ---------------------------------------------------------------------------
// Main extraction entry point
// ---------------------------------------------------------------------------

/// Extract the Clock resource surface from HIR after analysis.
pub fn extract_clock_surface<'tcx>(tcx: TyCtxt<'tcx>) -> Result<ClockSurface, ClockError> {
    let mut fields = Vec::new();
    let mut call_sites = Vec::new();

    for item_id in tcx.hir_free_items() {
        let item = tcx.hir_item(item_id);

        match &item.kind {
            ItemKind::Struct(_ident, _generics, variant_data) => {
                let extracted = extract_clock_fields(tcx, variant_data)?;
                fields.extend(extracted);
            }
            ItemKind::Impl(impl_block) => {
                for impl_item_ref in impl_block.items {
                    let impl_item = tcx.hir_impl_item(rustc_hir::ImplItemId {
                        owner_id: impl_item_ref.owner_id,
                    });
                    collect_clock_call_sites(tcx, impl_item, &mut call_sites);
                }
            }
            _ => {}
        }
    }

    Ok(ClockSurface { fields, call_sites })
}

// ---------------------------------------------------------------------------
// Clock field extraction
// ---------------------------------------------------------------------------

fn extract_clock_fields<'tcx>(
    tcx: TyCtxt<'tcx>,
    variant_data: &rustc_hir::VariantData<'tcx>,
) -> Result<Vec<ClockField>, ClockError> {
    let mut fields = Vec::new();

    for field in variant_data.fields() {
        let (is_clock, required) = match clock_field_shape(field.ty) {
            Some(r) => (true, r),
            None => continue,
        };

        if !is_clock {
            continue;
        }

        let attrs = tcx.hir_attrs(field.hir_id);
        let clock_attr = attrs.iter().find(|a| rseq_attr_name(a) == Some("clock"));

        let field_name = field.ident.name.as_str().to_string();

        let Some(attr) = clock_attr else {
            return Err(ClockError::MissingClockId(field_name));
        };

        let clock_id = parse_clock_attr(attr)?;
        let clock_id = clock_id.ok_or_else(|| ClockError::MissingClockId(field_name.clone()))?;

        fields.push(ClockField {
            field_name,
            clock_id,
            required,
        });
    }

    Ok(fields)
}

/// Returns `Some(required)` if this is a `Clock` or `Option<Clock>` type.
fn clock_field_shape(ty: &rustc_hir::Ty<'_>) -> Option<bool> {
    let rustc_hir::TyKind::Path(qpath) = &ty.kind else {
        return None;
    };
    let path = match qpath {
        rustc_hir::QPath::Resolved(_, path) => path,
        _ => return None,
    };
    let last_seg = path.segments.last()?;
    let name = last_seg.ident.name.as_str();

    if name == "Clock" {
        return Some(true);
    }

    if name == "Option" {
        let args = last_seg.args?;
        for arg in args.args.iter() {
            if let rustc_hir::GenericArg::Type(inner_ty) = arg {
                if let rustc_hir::TyKind::Path(rustc_hir::QPath::Resolved(_, p)) = &inner_ty.kind {
                    if p.segments.last().map(|s| s.ident.name.as_str()) == Some("Clock") {
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

fn parse_clock_attr(attr: &Attribute) -> Result<Option<String>, ClockError> {
    let Attribute::Unparsed(item) = attr else {
        return Err(malformed("expected tool attribute"));
    };
    let tokens = match &item.args {
        AttrArgs::Delimited(d) => &d.tokens,
        _ => return Ok(None),
    };

    let trees: Vec<&TokenTree> = tokens.iter().collect();
    let mut clock_id = None;
    let mut i = 0;

    while i < trees.len() {
        if is_comma(trees[i]) {
            i += 1;
            continue;
        }

        let key =
            extract_ident(trees[i]).ok_or_else(|| malformed("expected key in clock attribute"))?;
        i += 1;

        if i >= trees.len() || !is_eq(trees[i]) {
            return Err(malformed(&format!("expected '=' after '{key}'")));
        }
        i += 1;

        if i >= trees.len() {
            return Err(malformed(&format!("expected value for '{key}'")));
        }

        if key == "id" {
            clock_id = extract_string_literal(trees[i]);
        }
        i += 1;
    }

    Ok(clock_id)
}

// ---------------------------------------------------------------------------
// Call-site collection
// ---------------------------------------------------------------------------

fn collect_clock_call_sites<'tcx>(
    tcx: TyCtxt<'tcx>,
    impl_item: &'tcx rustc_hir::ImplItem<'tcx>,
    sites: &mut Vec<ClockCallSite>,
) {
    if let ImplItemKind::Fn(_sig, body_id) = impl_item.kind {
        let fn_name = impl_item.ident.name.as_str().to_string();
        let body = tcx.hir_body(body_id);
        let typeck_results = tcx.typeck(impl_item.owner_id.def_id);
        let mut visitor = ClockCallSiteVisitor {
            tcx,
            typeck_results,
            current_fn: fn_name,
            sites: Vec::new(),
        };
        visitor.visit_expr(body.value);
        sites.extend(visitor.sites);
    }
}

struct ClockCallSiteVisitor<'tcx> {
    tcx: TyCtxt<'tcx>,
    typeck_results: &'tcx rustc_middle::ty::TypeckResults<'tcx>,
    current_fn: String,
    sites: Vec<ClockCallSite>,
}

impl<'tcx> Visitor<'tcx> for ClockCallSiteVisitor<'tcx> {
    fn visit_expr(&mut self, expr: &'tcx rustc_hir::Expr<'tcx>) {
        if let ExprKind::MethodCall(segment, receiver, _, _) = expr.kind {
            let method = match segment.ident.name.as_str() {
                "enable" => Some(ClockMethod::Enable),
                "disable" => Some(ClockMethod::Disable),
                "get_rate" => Some(ClockMethod::GetRate),
                _ => None,
            };

            if let Some(method) = method {
                if self.is_clock_receiver(receiver) {
                    self.sites.push(ClockCallSite {
                        method,
                        containing_fn: self.current_fn.clone(),
                    });
                }
            }
        }
        intravisit::walk_expr(self, expr);
    }
}

impl<'tcx> ClockCallSiteVisitor<'tcx> {
    fn is_clock_receiver(&self, receiver: &'tcx rustc_hir::Expr<'tcx>) -> bool {
        let receiver_ty = self.typeck_results.expr_ty(receiver);
        is_clock_type(self.tcx, receiver_ty)
    }
}

fn is_clock_type<'tcx>(tcx: TyCtxt<'tcx>, ty: Ty<'tcx>) -> bool {
    ty.peel_refs()
        .ty_adt_def()
        .map(|adt| tcx.item_name(adt.did()).as_str() == "Clock")
        .unwrap_or(false)
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

fn malformed(msg: &str) -> ClockError {
    ClockError::MalformedAttribute(msg.to_string())
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
