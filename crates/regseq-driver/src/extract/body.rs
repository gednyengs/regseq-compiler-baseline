use rustc_hir::def::Res;
use rustc_hir::{
    BinOpKind, ExprKind, ImplItemKind, ItemKind, MatchSource, PatKind, QPath, StmtKind, UnOp,
};
use rustc_middle::ty::TyCtxt;

// ---------------------------------------------------------------------------
// Record types
// ---------------------------------------------------------------------------

/// Top-level extraction result for authored function bodies.
#[derive(Debug, Clone, PartialEq)]
pub struct BodySurface {
    pub functions: Vec<ExtractedFn>,
}

/// A complete extracted function with metadata and body tree.
#[derive(Debug, Clone, PartialEq)]
pub struct ExtractedFn {
    pub name: String,
    pub kind: FnKind,
    pub receiver: Receiver,
    pub params: Vec<FnParam>,
    pub return_type: Option<String>,
    pub body: BodyExpr,
}

/// Classification of an extracted function.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FnKind {
    /// A trait callback (Lifecycle::init, Serial::tx_byte, etc.).
    Callback {
        trait_name: String,
        method_name: String,
    },
    /// An ISR handler marked with `#[rseq::isr]`.
    Isr,
    /// A helper method on the driver type.
    Helper,
    /// A reachable free function.
    FreeFunction,
}

/// Receiver kind for extracted methods.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Receiver {
    None,
    Ref,
    RefMut,
}

/// A parameter of an extracted function (excluding self).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FnParam {
    pub name: String,
    pub ty: String,
}

/// A binary operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    And,
    Or,
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

/// A unary operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Neg,
    Not,
    Deref,
}

/// A statement in a block body.
#[derive(Debug, Clone, PartialEq)]
pub enum BodyStmt {
    Expr(BodyExpr),
    Semi(BodyExpr),
    Let {
        name: String,
        mutable: bool,
        ty: Option<String>,
        init: Option<BodyExpr>,
    },
}

/// A match arm.
#[derive(Debug, Clone, PartialEq)]
pub struct MatchArm {
    pub pattern: String,
    pub guard: Option<BodyExpr>,
    pub body: BodyExpr,
}

/// An expression in the body IR.
#[derive(Debug, Clone, PartialEq)]
pub enum BodyExpr {
    Block {
        stmts: Vec<BodyStmt>,
        expr: Option<Box<BodyExpr>>,
    },
    MethodCall {
        receiver: Box<BodyExpr>,
        method: String,
        type_args: Vec<String>,
        args: Vec<BodyExpr>,
    },
    Call {
        path: String,
        type_args: Vec<String>,
        args: Vec<BodyExpr>,
    },
    Field {
        base: Box<BodyExpr>,
        name: String,
    },
    Path(String),
    LitInt {
        value: String,
        suffix: Option<String>,
    },
    LitBool(bool),
    LitStr(String),
    Binary {
        op: BinOp,
        lhs: Box<BodyExpr>,
        rhs: Box<BodyExpr>,
    },
    Unary {
        op: UnaryOp,
        operand: Box<BodyExpr>,
    },
    Cast {
        expr: Box<BodyExpr>,
        ty: String,
    },
    If {
        cond: Box<BodyExpr>,
        then_branch: Box<BodyExpr>,
        else_branch: Option<Box<BodyExpr>>,
    },
    Loop {
        body: Box<BodyExpr>,
    },
    Return(Option<Box<BodyExpr>>),
    Try(Box<BodyExpr>),
    Ref {
        mutable: bool,
        expr: Box<BodyExpr>,
    },
    Assign {
        target: Box<BodyExpr>,
        value: Box<BodyExpr>,
    },
    AssignOp {
        op: BinOp,
        target: Box<BodyExpr>,
        value: Box<BodyExpr>,
    },
    Tuple(Vec<BodyExpr>),
    Array(Vec<BodyExpr>),
    Index {
        base: Box<BodyExpr>,
        index: Box<BodyExpr>,
    },
    Struct {
        path: String,
        fields: Vec<(String, BodyExpr)>,
        base: Option<Box<BodyExpr>>,
    },
    Closure {
        params: Vec<String>,
        body: Box<BodyExpr>,
    },
    Match {
        scrutinee: Box<BodyExpr>,
        arms: Vec<MatchArm>,
    },
    Break(Option<Box<BodyExpr>>),
    Continue,
    /// A construct outside the supported extraction subset.
    Unsupported {
        hir_kind: String,
        span_text: Option<String>,
    },
}

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BodyError {
    NoDriverType,
}

// ---------------------------------------------------------------------------
// Main extraction entry point
// ---------------------------------------------------------------------------

/// Extract authored function bodies from the target crate.
pub fn extract_body_surface<'tcx>(tcx: TyCtxt<'tcx>) -> Result<BodySurface, BodyError> {
    let driver_type = find_driver_type_name(tcx).ok_or(BodyError::NoDriverType)?;
    let mut functions = Vec::new();

    for item_id in tcx.hir_free_items() {
        let item = tcx.hir_item(item_id);
        match &item.kind {
            ItemKind::Impl(impl_block) => {
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
                        if let Some(f) = extract_trait_fn(tcx, impl_item, &trait_name) {
                            functions.push(f);
                        }
                    }
                } else {
                    for item_ref in impl_block.items {
                        let impl_item = tcx.hir_impl_item(rustc_hir::ImplItemId {
                            owner_id: item_ref.owner_id,
                        });
                        if let Some(f) = extract_inherent_fn(tcx, impl_item) {
                            functions.push(f);
                        }
                    }
                }
            }
            ItemKind::Fn {
                ident,
                sig,
                body: body_id,
                ..
            } => {
                let name = ident.name.as_str().to_string();
                if name == "main" {
                    continue;
                }
                let body = tcx.hir_body(*body_id);
                let params = extract_params_from_decl(sig.decl, body, false);
                let return_type = render_fn_return(sig.decl);
                let body_expr = extract_expr(tcx, body.value);
                functions.push(ExtractedFn {
                    name,
                    kind: FnKind::FreeFunction,
                    receiver: Receiver::None,
                    params,
                    return_type,
                    body: body_expr,
                });
            }
            _ => {}
        }
    }

    Ok(BodySurface { functions })
}

// ---------------------------------------------------------------------------
// Driver type discovery
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

// ---------------------------------------------------------------------------
// Impl-method extraction
// ---------------------------------------------------------------------------

fn extract_trait_fn<'tcx>(
    tcx: TyCtxt<'tcx>,
    impl_item: &'tcx rustc_hir::ImplItem<'tcx>,
    trait_name: &str,
) -> Option<ExtractedFn> {
    let ImplItemKind::Fn(ref sig, body_id) = impl_item.kind else {
        return None;
    };
    let method_name = impl_item.ident.name.as_str().to_string();
    let body = tcx.hir_body(body_id);
    let has_self = !sig.decl.inputs.is_empty() && is_self_ty(&sig.decl.inputs[0]);
    let receiver = if has_self {
        receiver_from_decl(sig.decl)
    } else {
        Receiver::None
    };
    let params = extract_params_from_decl(sig.decl, body, has_self);
    let return_type = render_fn_return(sig.decl);
    let body_expr = extract_expr(tcx, body.value);

    Some(ExtractedFn {
        name: method_name.clone(),
        kind: FnKind::Callback {
            trait_name: trait_name.to_string(),
            method_name,
        },
        receiver,
        params,
        return_type,
        body: body_expr,
    })
}

fn extract_inherent_fn<'tcx>(
    tcx: TyCtxt<'tcx>,
    impl_item: &'tcx rustc_hir::ImplItem<'tcx>,
) -> Option<ExtractedFn> {
    let ImplItemKind::Fn(ref sig, body_id) = impl_item.kind else {
        return None;
    };
    let name = impl_item.ident.name.as_str().to_string();
    let attrs = tcx.hir_attrs(impl_item.hir_id());
    let is_isr = attrs.iter().any(|a| rseq_attr_name(a) == Some("isr"));
    let has_self = !sig.decl.inputs.is_empty() && is_self_ty(&sig.decl.inputs[0]);
    let receiver = if has_self {
        receiver_from_decl(sig.decl)
    } else {
        Receiver::None
    };
    let body = tcx.hir_body(body_id);
    let params = extract_params_from_decl(sig.decl, body, has_self);
    let return_type = render_fn_return(sig.decl);
    let body_expr = extract_expr(tcx, body.value);

    Some(ExtractedFn {
        name,
        kind: if is_isr { FnKind::Isr } else { FnKind::Helper },
        receiver,
        params,
        return_type,
        body: body_expr,
    })
}

// ---------------------------------------------------------------------------
// Parameter and receiver helpers
// ---------------------------------------------------------------------------

fn is_self_ty(ty: &rustc_hir::Ty<'_>) -> bool {
    matches!(&ty.kind, rustc_hir::TyKind::Ref(_, _))
        || matches!(
            &ty.kind,
            rustc_hir::TyKind::Path(QPath::Resolved(_, path))
            if path.segments.last().map(|s| s.ident.name.as_str() == "Self").unwrap_or(false)
        )
}

fn receiver_from_decl(decl: &rustc_hir::FnDecl<'_>) -> Receiver {
    if decl.inputs.is_empty() {
        return Receiver::None;
    }
    if let rustc_hir::TyKind::Ref(_, mut_ty) = &decl.inputs[0].kind {
        if mut_ty.mutbl.is_mut() {
            Receiver::RefMut
        } else {
            Receiver::Ref
        }
    } else {
        Receiver::None
    }
}

fn extract_params_from_decl<'tcx>(
    decl: &rustc_hir::FnDecl<'tcx>,
    body: &rustc_hir::Body<'tcx>,
    has_self: bool,
) -> Vec<FnParam> {
    let skip = if has_self { 1 } else { 0 };
    body.params
        .iter()
        .skip(skip)
        .zip(decl.inputs.iter().skip(skip))
        .map(|(param, ty)| FnParam {
            name: render_pat(param.pat),
            ty: render_hir_ty(ty),
        })
        .collect()
}

fn render_fn_return(decl: &rustc_hir::FnDecl<'_>) -> Option<String> {
    match &decl.output {
        rustc_hir::FnRetTy::DefaultReturn(_) => None,
        rustc_hir::FnRetTy::Return(ty) => {
            let rendered = render_hir_ty(ty);
            if rendered == "()" {
                None
            } else {
                Some(rendered)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Expression extraction (recursive core)
// ---------------------------------------------------------------------------

fn extract_expr<'tcx>(tcx: TyCtxt<'tcx>, expr: &'tcx rustc_hir::Expr<'tcx>) -> BodyExpr {
    match &expr.kind {
        ExprKind::Block(block, _) => extract_block(tcx, block),
        ExprKind::MethodCall(segment, receiver, args, _) => {
            let type_args = extract_segment_type_args(segment);
            BodyExpr::MethodCall {
                receiver: Box::new(extract_expr(tcx, receiver)),
                method: segment.ident.name.as_str().to_string(),
                type_args,
                args: args.iter().map(|a| extract_expr(tcx, a)).collect(),
            }
        }
        ExprKind::Call(callee, args) => {
            let (path, type_args) = render_callee(tcx, callee);
            BodyExpr::Call {
                path,
                type_args,
                args: args.iter().map(|a| extract_expr(tcx, a)).collect(),
            }
        }
        ExprKind::Field(base, ident) => BodyExpr::Field {
            base: Box::new(extract_expr(tcx, base)),
            name: ident.name.as_str().to_string(),
        },
        ExprKind::Path(qpath) => BodyExpr::Path(render_qpath(qpath)),
        ExprKind::Lit(lit) => extract_lit(lit),
        ExprKind::Binary(op, lhs, rhs) => BodyExpr::Binary {
            op: binop_from_hir(op.node),
            lhs: Box::new(extract_expr(tcx, lhs)),
            rhs: Box::new(extract_expr(tcx, rhs)),
        },
        ExprKind::Unary(op, operand) => BodyExpr::Unary {
            op: unaryop_from_hir(*op),
            operand: Box::new(extract_expr(tcx, operand)),
        },
        ExprKind::Cast(inner, ty) => BodyExpr::Cast {
            expr: Box::new(extract_expr(tcx, inner)),
            ty: render_hir_ty(ty),
        },
        ExprKind::If(cond, then_b, else_b) => BodyExpr::If {
            cond: Box::new(extract_expr(tcx, cond)),
            then_branch: Box::new(extract_expr(tcx, then_b)),
            else_branch: else_b.map(|e| Box::new(extract_expr(tcx, e))),
        },
        ExprKind::Loop(block, _, _source, _) => BodyExpr::Loop {
            body: Box::new(extract_block(tcx, block)),
        },
        ExprKind::Match(scrutinee, arms, source) => {
            if is_try_desugar(source) {
                return BodyExpr::Try(Box::new(extract_expr(tcx, scrutinee)));
            }
            BodyExpr::Match {
                scrutinee: Box::new(extract_expr(tcx, scrutinee)),
                arms: arms.iter().map(|arm| extract_arm(tcx, arm)).collect(),
            }
        }
        ExprKind::Ret(opt) => BodyExpr::Return(opt.map(|e| Box::new(extract_expr(tcx, e)))),
        ExprKind::AddrOf(_, mutbl, inner) => BodyExpr::Ref {
            mutable: mutbl.is_mut(),
            expr: Box::new(extract_expr(tcx, inner)),
        },
        ExprKind::Assign(target, value, _) => BodyExpr::Assign {
            target: Box::new(extract_expr(tcx, target)),
            value: Box::new(extract_expr(tcx, value)),
        },
        ExprKind::AssignOp(op, target, value) => BodyExpr::AssignOp {
            op: binop_from_hir(op.node.into()),
            target: Box::new(extract_expr(tcx, target)),
            value: Box::new(extract_expr(tcx, value)),
        },
        ExprKind::Tup(elems) => {
            BodyExpr::Tuple(elems.iter().map(|e| extract_expr(tcx, e)).collect())
        }
        ExprKind::Array(elems) => {
            BodyExpr::Array(elems.iter().map(|e| extract_expr(tcx, e)).collect())
        }
        ExprKind::Index(base, index, _) => BodyExpr::Index {
            base: Box::new(extract_expr(tcx, base)),
            index: Box::new(extract_expr(tcx, index)),
        },
        ExprKind::Struct(qpath, fields, base) => {
            let base_expr = match base {
                rustc_hir::StructTailExpr::Base(e) => Some(Box::new(extract_expr(tcx, e))),
                _ => None,
            };
            BodyExpr::Struct {
                path: render_qpath(qpath),
                fields: fields
                    .iter()
                    .map(|f| (f.ident.name.as_str().to_string(), extract_expr(tcx, f.expr)))
                    .collect(),
                base: base_expr,
            }
        }
        ExprKind::Closure(closure) => {
            let body = tcx.hir_body(closure.body);
            let params = body.params.iter().map(|p| render_pat(p.pat)).collect();
            BodyExpr::Closure {
                params,
                body: Box::new(extract_expr(tcx, body.value)),
            }
        }
        ExprKind::Break(_, opt) => BodyExpr::Break(opt.map(|e| Box::new(extract_expr(tcx, e)))),
        ExprKind::Continue(_) => BodyExpr::Continue,
        ExprKind::DropTemps(inner) => extract_expr(tcx, inner),
        ExprKind::Let(let_expr) => {
            // `let` in expression position (e.g., `if let Some(x) = ...`)
            BodyExpr::Unsupported {
                hir_kind: "LetExpr".to_string(),
                span_text: Some(format!("let {} = ...", render_pat(let_expr.pat))),
            }
        }
        _ => BodyExpr::Unsupported {
            hir_kind: expr_kind_name(&expr.kind),
            span_text: None,
        },
    }
}

fn extract_block<'tcx>(tcx: TyCtxt<'tcx>, block: &'tcx rustc_hir::Block<'tcx>) -> BodyExpr {
    let stmts = block.stmts.iter().map(|s| extract_stmt(tcx, s)).collect();
    let tail = block.expr.map(|e| Box::new(extract_expr(tcx, e)));
    BodyExpr::Block { stmts, expr: tail }
}

fn extract_stmt<'tcx>(tcx: TyCtxt<'tcx>, stmt: &'tcx rustc_hir::Stmt<'tcx>) -> BodyStmt {
    match &stmt.kind {
        StmtKind::Expr(expr) => BodyStmt::Expr(extract_expr(tcx, expr)),
        StmtKind::Semi(expr) => BodyStmt::Semi(extract_expr(tcx, expr)),
        StmtKind::Let(local) => BodyStmt::Let {
            name: render_pat(local.pat),
            mutable: pat_is_mutable(local.pat),
            ty: local.ty.map(render_hir_ty),
            init: local.init.map(|e| extract_expr(tcx, e)),
        },
        StmtKind::Item(_) => BodyStmt::Semi(BodyExpr::Unsupported {
            hir_kind: "ItemStmt".to_string(),
            span_text: None,
        }),
    }
}

fn extract_arm<'tcx>(tcx: TyCtxt<'tcx>, arm: &'tcx rustc_hir::Arm<'tcx>) -> MatchArm {
    MatchArm {
        pattern: render_pat(arm.pat),
        guard: arm.guard.map(|g| extract_expr(tcx, g)),
        body: extract_expr(tcx, arm.body),
    }
}

fn is_try_desugar(source: &MatchSource) -> bool {
    matches!(source, MatchSource::TryDesugar(_))
}

// ---------------------------------------------------------------------------
// Literal extraction
// ---------------------------------------------------------------------------

fn extract_lit(lit: &rustc_hir::Lit) -> BodyExpr {
    match &lit.node {
        rustc_ast::LitKind::Int(val, suffix) => {
            let suffix_str = match suffix {
                rustc_ast::LitIntType::Signed(s) => Some(format!("{s:?}").to_lowercase()),
                rustc_ast::LitIntType::Unsigned(s) => Some(format!("{s:?}").to_lowercase()),
                rustc_ast::LitIntType::Unsuffixed => None,
            };
            BodyExpr::LitInt {
                value: val.to_string(),
                suffix: suffix_str,
            }
        }
        rustc_ast::LitKind::Bool(b) => BodyExpr::LitBool(*b),
        rustc_ast::LitKind::Str(sym, _) => BodyExpr::LitStr(sym.as_str().to_string()),
        _ => BodyExpr::Unsupported {
            hir_kind: format!("Lit({:?})", lit.node),
            span_text: None,
        },
    }
}

// ---------------------------------------------------------------------------
// Callee and path rendering
// ---------------------------------------------------------------------------

fn render_callee<'tcx>(
    _tcx: TyCtxt<'tcx>,
    callee: &'tcx rustc_hir::Expr<'tcx>,
) -> (String, Vec<String>) {
    if let ExprKind::Path(qpath) = &callee.kind {
        let path = render_qpath(qpath);
        let type_args = extract_qpath_type_args(qpath);
        (path, type_args)
    } else {
        ("<expr>".to_string(), vec![])
    }
}

fn extract_qpath_type_args(qpath: &QPath<'_>) -> Vec<String> {
    let args_opt = match qpath {
        QPath::Resolved(_, path) => path.segments.last().and_then(|s| s.args),
        QPath::TypeRelative(_, seg) => seg.args,
    };
    args_opt
        .map(|args| {
            args.args
                .iter()
                .filter_map(|arg| match arg {
                    rustc_hir::GenericArg::Type(ty) => Some(render_hir_ty(ty)),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default()
}

fn extract_segment_type_args(segment: &rustc_hir::PathSegment<'_>) -> Vec<String> {
    segment
        .args
        .map(|args| {
            args.args
                .iter()
                .filter_map(|arg| match arg {
                    rustc_hir::GenericArg::Type(ty) => Some(render_hir_ty(ty)),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// HIR rendering helpers
// ---------------------------------------------------------------------------

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
        rustc_hir::TyKind::Ptr(mut_ty) => {
            let m = if mut_ty.mutbl.is_mut() {
                "*mut "
            } else {
                "*const "
            };
            format!("{m}{}", render_hir_ty(mut_ty.ty))
        }
        rustc_hir::TyKind::Never => "!".to_string(),
        rustc_hir::TyKind::Infer(_) => "_".to_string(),
        _ => "<type>".to_string(),
    }
}

fn render_pat(pat: &rustc_hir::Pat<'_>) -> String {
    match &pat.kind {
        PatKind::Binding(_, _, ident, _) => ident.name.as_str().to_string(),
        PatKind::Wild => "_".to_string(),
        PatKind::Tuple(pats, _) => {
            let inner: Vec<_> = pats.iter().map(|p| render_pat(p)).collect();
            format!("({})", inner.join(", "))
        }
        PatKind::TupleStruct(qpath, pats, _) => {
            let inner: Vec<_> = pats.iter().map(|p| render_pat(p)).collect();
            format!("{}({})", render_qpath(qpath), inner.join(", "))
        }
        PatKind::Struct(qpath, fields, rest) => {
            let flds: Vec<_> = fields
                .iter()
                .map(|f| format!("{}: {}", f.ident.name, render_pat(f.pat)))
                .collect();
            let rest_str = if rest.is_some() { ", .." } else { "" };
            format!(
                "{} {{ {}{} }}",
                render_qpath(qpath),
                flds.join(", "),
                rest_str
            )
        }
        PatKind::Expr(expr) => format!("<pat_expr:{:?}>", expr.kind),
        PatKind::Ref(inner, _mutbl, _) => {
            format!("&{}", render_pat(inner))
        }
        PatKind::Or(pats) => {
            let inner: Vec<_> = pats.iter().map(|p| render_pat(p)).collect();
            inner.join(" | ")
        }
        _ => "<pattern>".to_string(),
    }
}

fn pat_is_mutable(pat: &rustc_hir::Pat<'_>) -> bool {
    matches!(&pat.kind, PatKind::Binding(rustc_hir::BindingMode::MUT, ..))
}

// ---------------------------------------------------------------------------
// Operator conversion
// ---------------------------------------------------------------------------

fn binop_from_hir(op: BinOpKind) -> BinOp {
    match op {
        BinOpKind::Add => BinOp::Add,
        BinOpKind::Sub => BinOp::Sub,
        BinOpKind::Mul => BinOp::Mul,
        BinOpKind::Div => BinOp::Div,
        BinOpKind::Rem => BinOp::Rem,
        BinOpKind::And => BinOp::And,
        BinOpKind::Or => BinOp::Or,
        BinOpKind::BitAnd => BinOp::BitAnd,
        BinOpKind::BitOr => BinOp::BitOr,
        BinOpKind::BitXor => BinOp::BitXor,
        BinOpKind::Shl => BinOp::Shl,
        BinOpKind::Shr => BinOp::Shr,
        BinOpKind::Eq => BinOp::Eq,
        BinOpKind::Ne => BinOp::Ne,
        BinOpKind::Lt => BinOp::Lt,
        BinOpKind::Le => BinOp::Le,
        BinOpKind::Gt => BinOp::Gt,
        BinOpKind::Ge => BinOp::Ge,
    }
}

fn unaryop_from_hir(op: UnOp) -> UnaryOp {
    match op {
        UnOp::Neg => UnaryOp::Neg,
        UnOp::Not => UnaryOp::Not,
        UnOp::Deref => UnaryOp::Deref,
    }
}

// ---------------------------------------------------------------------------
// Trait name resolution
// ---------------------------------------------------------------------------

fn resolve_trait_name<'tcx>(tcx: TyCtxt<'tcx>, path: &rustc_hir::Path<'tcx>) -> String {
    let def_id = match path.res {
        Res::Def(_, def_id) => def_id,
        _ => {
            return path
                .segments
                .last()
                .map(|s| s.ident.name.as_str().to_string())
                .unwrap_or_default();
        }
    };
    let full_path = tcx.def_path_str(def_id);
    // Extract the short trait name (e.g., "Lifecycle" from "regseq::Lifecycle")
    full_path
        .rsplit("::")
        .next()
        .unwrap_or(&full_path)
        .to_string()
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

// ---------------------------------------------------------------------------
// Attribute helpers
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// ExprKind name for diagnostics
// ---------------------------------------------------------------------------

fn expr_kind_name(kind: &ExprKind<'_>) -> String {
    match kind {
        ExprKind::Block(..) => "Block",
        ExprKind::Call(..) => "Call",
        ExprKind::MethodCall(..) => "MethodCall",
        ExprKind::Tup(..) => "Tup",
        ExprKind::Binary(..) => "Binary",
        ExprKind::Unary(..) => "Unary",
        ExprKind::Lit(..) => "Lit",
        ExprKind::Cast(..) => "Cast",
        ExprKind::If(..) => "If",
        ExprKind::Loop(..) => "Loop",
        ExprKind::Match(..) => "Match",
        ExprKind::Closure(..) => "Closure",
        ExprKind::Assign(..) => "Assign",
        ExprKind::AssignOp(..) => "AssignOp",
        ExprKind::Field(..) => "Field",
        ExprKind::Index(..) => "Index",
        ExprKind::Path(..) => "Path",
        ExprKind::AddrOf(..) => "AddrOf",
        ExprKind::Break(..) => "Break",
        ExprKind::Continue(..) => "Continue",
        ExprKind::Ret(..) => "Ret",
        ExprKind::Struct(..) => "Struct",
        ExprKind::Array(..) => "Array",
        ExprKind::DropTemps(..) => "DropTemps",
        ExprKind::Let(..) => "Let",
        _ => "<unknown>",
    }
    .to_string()
}
