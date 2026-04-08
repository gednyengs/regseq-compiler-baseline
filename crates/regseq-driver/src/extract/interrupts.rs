use rustc_ast::token::{LitKind, TokenKind};
use rustc_ast::tokenstream::TokenTree;
use rustc_hir::{AttrArgs, Attribute, FnRetTy, ImplItemKind, ItemKind};
use rustc_middle::ty::TyCtxt;

use super::callback_context::CallbackContext;

// ---------------------------------------------------------------------------
// Record types
// ---------------------------------------------------------------------------

/// Top-level extraction result for the interrupt surface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterruptSurface {
    pub resources: Vec<InterruptResourceDecl>,
    pub isr_bindings: Vec<IsrBinding>,
}

/// An interrupt resource field declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterruptResourceDecl {
    pub field_name: String,
    pub interrupt_id: String,
    pub shape: InterruptFieldShape,
    /// Exact rendered Rust type of this field (e.g. `"Interrupt"`, `"Option<Interrupt>"`).
    pub field_type: String,
    pub layer2: InterruptLayer2Properties,
}

/// The shape of an interrupt resource field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterruptFieldShape {
    Single,
    Optional,
    Array(usize),
}

/// Layer 2 interrupt binding properties.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct InterruptLayer2Properties {
    pub trigger_type: Option<String>,
    pub trigger_polarity: Option<String>,
    pub priority: Option<String>,
    pub shared: Option<bool>,
}

/// A bound ISR handler extracted from an impl block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IsrBinding {
    pub handler_name: String,
    pub bind_target: IsrBindTarget,
    pub has_index_param: bool,
    pub context: CallbackContext,
}

/// How an ISR handler binds to an interrupt resource.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IsrBindTarget {
    /// Bare `#[rseq::isr]` -- auto-binds when exactly one interrupt resource.
    Auto,
    /// Explicit `#[rseq::isr(field_name)]`.
    ExplicitField(String),
}

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InterruptError {
    MalformedAttribute(String),
    AmbiguousBareIsr,
    MissingIndexParam(String),
    InvalidIsrReturnType(String),
    DuplicateHandler(String),
}

// ---------------------------------------------------------------------------
// Main extraction entry point
// ---------------------------------------------------------------------------

/// Extract the interrupt surface from HIR after analysis.
pub fn extract_interrupt_surface<'tcx>(
    tcx: TyCtxt<'tcx>,
) -> Result<InterruptSurface, InterruptError> {
    let mut resources = Vec::new();
    let mut isr_bindings = Vec::new();

    for item_id in tcx.hir_free_items() {
        let item = tcx.hir_item(item_id);

        match &item.kind {
            ItemKind::Struct(_ident, _generics, variant_data) => {
                let fields = extract_interrupt_fields(tcx, variant_data)?;
                resources.extend(fields);
            }
            ItemKind::Impl(impl_block) => {
                for impl_item_ref in impl_block.items {
                    let impl_item = tcx.hir_impl_item(rustc_hir::ImplItemId {
                        owner_id: impl_item_ref.owner_id,
                    });
                    if let Some(binding) = extract_isr_binding(tcx, impl_item)? {
                        isr_bindings.push(binding);
                    }
                }
            }
            _ => {}
        }
    }

    validate_bindings(&resources, &isr_bindings)?;

    Ok(InterruptSurface {
        resources,
        isr_bindings,
    })
}

// ---------------------------------------------------------------------------
// Interrupt resource field extraction
// ---------------------------------------------------------------------------

fn extract_interrupt_fields<'tcx>(
    tcx: TyCtxt<'tcx>,
    variant_data: &rustc_hir::VariantData<'tcx>,
) -> Result<Vec<InterruptResourceDecl>, InterruptError> {
    let mut fields = Vec::new();

    for field in variant_data.fields() {
        let shape = match interrupt_field_shape(tcx, field.ty) {
            Some(s) => s,
            None => continue,
        };

        let attrs = tcx.hir_attrs(field.hir_id);
        let interrupt_attr = attrs
            .iter()
            .find(|a| rseq_attr_name(a) == Some("interrupt"));

        let Some(attr) = interrupt_attr else {
            continue;
        };

        let (interrupt_id, layer2) = parse_interrupt_attr(attr)?;
        let interrupt_id = interrupt_id.ok_or_else(|| {
            InterruptError::MalformedAttribute("interrupt attribute requires id = \"...\"".into())
        })?;

        fields.push(InterruptResourceDecl {
            field_name: field.ident.name.as_str().to_string(),
            interrupt_id,
            shape,
            field_type: hir_ty_to_string(field.ty),
            layer2,
        });
    }

    Ok(fields)
}

fn interrupt_field_shape<'tcx>(
    tcx: TyCtxt<'tcx>,
    ty: &rustc_hir::Ty<'tcx>,
) -> Option<InterruptFieldShape> {
    match &ty.kind {
        rustc_hir::TyKind::Path(qpath) => {
            let path = match qpath {
                rustc_hir::QPath::Resolved(_, path) => path,
                _ => return None,
            };
            let last_seg = path.segments.last()?;
            let name = last_seg.ident.name.as_str();

            if name == "Interrupt" {
                return Some(InterruptFieldShape::Single);
            }

            if name == "Option" {
                let args = last_seg.args?;
                for arg in args.args.iter() {
                    if let rustc_hir::GenericArg::Type(inner_ty) = arg {
                        if let rustc_hir::TyKind::Path(rustc_hir::QPath::Resolved(_, p)) =
                            &inner_ty.kind
                        {
                            if p.segments.last().map(|s| s.ident.name.as_str()) == Some("Interrupt")
                            {
                                return Some(InterruptFieldShape::Optional);
                            }
                        }
                    }
                }
            }

            None
        }
        rustc_hir::TyKind::Array(inner_ty, array_len) => {
            if !path_ends_with_interrupt(&inner_ty.kind) {
                return None;
            }
            let len = extract_array_len(tcx, array_len).unwrap_or(0);
            Some(InterruptFieldShape::Array(len))
        }
        _ => None,
    }
}

fn path_ends_with_interrupt(kind: &rustc_hir::TyKind<'_>) -> bool {
    if let rustc_hir::TyKind::Path(rustc_hir::QPath::Resolved(_, path)) = kind {
        path.segments
            .last()
            .map(|s| s.ident.name.as_str() == "Interrupt")
            .unwrap_or(false)
    } else {
        false
    }
}

fn extract_array_len<'tcx>(
    tcx: TyCtxt<'tcx>,
    array_len: &rustc_hir::ConstArg<'tcx>,
) -> Option<usize> {
    if let rustc_hir::ConstArgKind::Anon(anon_const) = &array_len.kind {
        let body = tcx.hir_body(anon_const.body);
        extract_usize_literal(body.value)
    } else {
        None
    }
}

fn extract_usize_literal(expr: &rustc_hir::Expr<'_>) -> Option<usize> {
    if let rustc_hir::ExprKind::Lit(lit) = &expr.kind {
        if let rustc_ast::LitKind::Int(v, _) = lit.node {
            return usize::try_from(v.get()).ok();
        }
    }
    None
}

// ---------------------------------------------------------------------------
// ISR binding extraction
// ---------------------------------------------------------------------------

fn extract_isr_binding<'tcx>(
    tcx: TyCtxt<'tcx>,
    impl_item: &'tcx rustc_hir::ImplItem<'tcx>,
) -> Result<Option<IsrBinding>, InterruptError> {
    let attrs = tcx.hir_attrs(impl_item.hir_id());
    let isr_attr = attrs.iter().find(|a| rseq_attr_name(a) == Some("isr"));

    let Some(attr) = isr_attr else {
        return Ok(None);
    };

    let handler_name = impl_item.ident.name.as_str().to_string();

    let ImplItemKind::Fn(ref sig, _body_id) = impl_item.kind else {
        return Err(InterruptError::MalformedAttribute(
            "#[rseq::isr] must be on a method".into(),
        ));
    };

    if !is_irq_result_return_type(&sig.decl.output) {
        return Err(InterruptError::InvalidIsrReturnType(handler_name));
    }

    let bind_target = parse_isr_bind_target(attr)?;
    let has_index_param = has_index_usize_param(sig.decl);

    Ok(Some(IsrBinding {
        handler_name,
        bind_target,
        has_index_param,
        context: CallbackContext::HandlerManaged,
    }))
}

fn is_irq_result_return_type(output: &FnRetTy<'_>) -> bool {
    match output {
        FnRetTy::Return(ty) => {
            if let rustc_hir::TyKind::Path(rustc_hir::QPath::Resolved(_, path)) = &ty.kind {
                path.segments
                    .last()
                    .map(|s| s.ident.name.as_str() == "IrqResult")
                    .unwrap_or(false)
            } else {
                false
            }
        }
        FnRetTy::DefaultReturn(_) => false,
    }
}

fn has_index_usize_param(decl: &rustc_hir::FnDecl<'_>) -> bool {
    decl.inputs.iter().skip(1).any(|ty| {
        if let rustc_hir::TyKind::Path(rustc_hir::QPath::Resolved(_, path)) = &ty.kind {
            path.segments
                .last()
                .map(|s| s.ident.name.as_str() == "usize")
                .unwrap_or(false)
        } else {
            false
        }
    })
}

// ---------------------------------------------------------------------------
// Structural validation
// ---------------------------------------------------------------------------

fn validate_bindings(
    resources: &[InterruptResourceDecl],
    bindings: &[IsrBinding],
) -> Result<(), InterruptError> {
    for binding in bindings {
        if binding.bind_target == IsrBindTarget::Auto && resources.len() != 1 {
            return Err(InterruptError::AmbiguousBareIsr);
        }

        if let IsrBindTarget::ExplicitField(ref field_name) = binding.bind_target {
            if let Some(resource) = resources.iter().find(|r| r.field_name == *field_name) {
                if matches!(resource.shape, InterruptFieldShape::Array(_))
                    && !binding.has_index_param
                {
                    return Err(InterruptError::MissingIndexParam(
                        binding.handler_name.clone(),
                    ));
                }
            }
        }
    }

    let mut seen_targets: Vec<String> = Vec::new();
    for binding in bindings {
        let target_key = match &binding.bind_target {
            IsrBindTarget::Auto => {
                if resources.len() == 1 {
                    resources[0].field_name.clone()
                } else {
                    continue;
                }
            }
            IsrBindTarget::ExplicitField(name) => name.clone(),
        };

        if seen_targets.contains(&target_key) {
            return Err(InterruptError::DuplicateHandler(target_key));
        }
        seen_targets.push(target_key);
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Attribute parsing
// ---------------------------------------------------------------------------

fn parse_interrupt_attr(
    attr: &Attribute,
) -> Result<(Option<String>, InterruptLayer2Properties), InterruptError> {
    let Attribute::Unparsed(item) = attr else {
        return Err(malformed("expected tool attribute"));
    };
    let tokens = match &item.args {
        AttrArgs::Delimited(d) => &d.tokens,
        _ => return Ok((None, InterruptLayer2Properties::default())),
    };

    let trees: Vec<&TokenTree> = tokens.iter().collect();
    let mut interrupt_id = None;
    let mut layer2 = InterruptLayer2Properties::default();
    let mut i = 0;

    while i < trees.len() {
        if is_comma(trees[i]) {
            i += 1;
            continue;
        }

        let key = extract_ident(trees[i])
            .ok_or_else(|| malformed("expected key in interrupt attribute"))?;
        i += 1;

        if i >= trees.len() || !is_eq(trees[i]) {
            return Err(malformed(&format!("expected '=' after '{key}'")));
        }
        i += 1;

        if i >= trees.len() {
            return Err(malformed(&format!("expected value for '{key}'")));
        }

        match key.as_str() {
            "id" => {
                interrupt_id = extract_string_literal(trees[i]);
            }
            "trigger_type" => {
                layer2.trigger_type = extract_string_literal(trees[i]);
            }
            "trigger_polarity" => {
                layer2.trigger_polarity = extract_string_literal(trees[i]);
            }
            "priority" => {
                layer2.priority = extract_string_literal(trees[i]);
            }
            "shared" => {
                layer2.shared = extract_bool_ident(trees[i]);
            }
            _ => {
                // Unknown keys ignored for forward compatibility.
            }
        }
        i += 1;
    }

    Ok((interrupt_id, layer2))
}

fn parse_isr_bind_target(attr: &Attribute) -> Result<IsrBindTarget, InterruptError> {
    let Attribute::Unparsed(item) = attr else {
        return Err(malformed("expected tool attribute"));
    };
    match &item.args {
        AttrArgs::Delimited(d) => {
            let trees: Vec<&TokenTree> = d.tokens.iter().collect();
            if trees.is_empty() {
                return Ok(IsrBindTarget::Auto);
            }
            if trees.len() == 1 {
                if let Some(ident) = extract_ident(trees[0]) {
                    return Ok(IsrBindTarget::ExplicitField(ident));
                }
            }
            Err(malformed(
                "expected field name identifier in #[rseq::isr(...)]",
            ))
        }
        _ => Ok(IsrBindTarget::Auto),
    }
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

fn malformed(msg: &str) -> InterruptError {
    InterruptError::MalformedAttribute(msg.to_string())
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

fn extract_bool_ident(tree: &TokenTree) -> Option<bool> {
    match tree {
        TokenTree::Token(tok, _) => match tok.kind {
            TokenKind::Ident(sym, _) => match sym.as_str() {
                "true" => Some(true),
                "false" => Some(false),
                _ => None,
            },
            _ => None,
        },
        _ => None,
    }
}
