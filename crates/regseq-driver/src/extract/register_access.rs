use rustc_ast::token::{LitKind, TokenKind};
use rustc_ast::tokenstream::TokenTree;
use rustc_hir::intravisit::{self, Visitor};
use rustc_hir::{AttrArgs, Attribute, ExprKind, ImplItemKind, ItemKind};
use rustc_middle::ty::{Ty, TyCtxt};

// ---------------------------------------------------------------------------
// Record types
// ---------------------------------------------------------------------------

/// Top-level extraction result for the Register Access surface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisterAccessSurface {
    pub register_blocks: Vec<RegisterBlockDecl>,
    pub mmio_fields: Vec<MmioFieldDecl>,
    pub wrapper_call_sites: Vec<WrapperCallSite>,
    pub poll_call_sites: Vec<PollCallSite>,
    pub register_descriptors: Vec<RegisterDescriptorFact>,
}

/// A typed value from a register-block attribute property.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlockPropertyValue {
    /// A string literal value, e.g. `endian = "little"`.
    Str(String),
    /// An integer literal value, e.g. `access_width = 32`.
    Int(u32),
    /// A boolean literal value, e.g. `shadow = true`.
    Bool(bool),
}

/// A struct annotated with `#[rseq::register_block]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisterBlockDecl {
    pub name: String,
    /// All authored key=value properties from `#[rseq::register_block(...)]`.
    pub properties: Vec<(String, BlockPropertyValue)>,
    pub fields: Vec<RegisterFieldDecl>,
}

/// A field within a register block, optionally carrying `#[rseq::register(...)]` metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisterFieldDecl {
    pub name: String,
    pub access_width: Option<u32>,
    pub endian: Option<String>,
    pub side_effects: Option<String>,
    pub hw_updates: Option<String>,
    pub register_type_name: Option<String>,
    pub byte_offset: Option<u32>,
    pub access_kind: Option<String>,
    pub array_count: Option<u32>,
}

/// An `Mmio<T>` typed field, with optional `#[rseq::mmio(...)]` metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MmioFieldDecl {
    pub field_name: String,
    pub mmio_id: Option<String>,
    pub register_block_type: Option<String>,
}

/// An accepted wrapper-method name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WrapperMethod {
    ReadRaw,
    Read,
    Write,
    Modify,
    Set,
}

/// A wrapper-method call site detected in a function body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WrapperCallSite {
    pub method: WrapperMethod,
    pub containing_fn: String,
}

/// A `poll::until(...)` call site detected in a function body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PollCallSite {
    pub containing_fn: String,
}

/// A register-descriptor module matching the `register_bitfields!` expansion
/// pattern: a module containing `struct Register` and field sub-modules.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisterDescriptorFact {
    pub register_module: String,
    pub fields: Vec<RegisterDescriptorFieldFact>,
}

/// A field within a register descriptor, recovered from generated macro output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisterDescriptorFieldFact {
    pub field_name: String,
    pub offset: u32,
    pub width: u32,
    pub named_values: Vec<RegisterNamedValueFact>,
}

/// A named value constant generated for a descriptor field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisterNamedValueFact {
    pub name: String,
    pub value: u32,
}

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegisterAccessError {
    MalformedAttribute(String),
    /// `#[rseq::mmio(...)]` was present but did not provide the required `id = "..."`
    /// metadata.
    MissingMmioId,
    InvalidAccessWidth(String),
}

// ---------------------------------------------------------------------------
// Main extraction entry point
// ---------------------------------------------------------------------------

/// Extract the Register Access surface from HIR after analysis.
pub fn extract_register_access_surface<'tcx>(
    tcx: TyCtxt<'tcx>,
) -> Result<RegisterAccessSurface, RegisterAccessError> {
    let mut register_blocks = Vec::new();
    let mut mmio_fields = Vec::new();
    let mut wrapper_call_sites = Vec::new();
    let mut poll_call_sites = Vec::new();
    let mut register_descriptors = Vec::new();

    for item_id in tcx.hir_free_items() {
        let item = tcx.hir_item(item_id);
        let attrs = tcx.hir_attrs(item.hir_id());

        match &item.kind {
            ItemKind::Struct(ident, _generics, variant_data) => {
                if has_rseq_attr(attrs, "register_block") {
                    let block =
                        extract_register_block(tcx, ident.name.as_str(), attrs, variant_data)?;
                    register_blocks.push(block);
                }

                let fields = extract_mmio_fields(tcx, variant_data)?;
                mmio_fields.extend(fields);
            }
            ItemKind::Impl(impl_block) => {
                for impl_item_ref in impl_block.items {
                    let impl_item = tcx.hir_impl_item(rustc_hir::ImplItemId {
                        owner_id: impl_item_ref.owner_id,
                    });
                    collect_impl_item_call_sites(
                        tcx,
                        impl_item,
                        &mut wrapper_call_sites,
                        &mut poll_call_sites,
                    );
                }
            }
            ItemKind::Mod(ident, module) => {
                if let Some(desc) =
                    try_extract_register_descriptor(tcx, ident.name.as_str(), module)
                {
                    register_descriptors.push(desc);
                }
            }
            _ => {}
        }
    }

    Ok(RegisterAccessSurface {
        register_blocks,
        mmio_fields,
        wrapper_call_sites,
        poll_call_sites,
        register_descriptors,
    })
}

// ---------------------------------------------------------------------------
// Attribute helpers
// ---------------------------------------------------------------------------

/// Return the tool name if this is an `#[rseq::*]` tool attribute.
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

fn has_rseq_attr(attrs: &[Attribute], name: &str) -> bool {
    attrs.iter().any(|a| rseq_attr_name(a) == Some(name))
}

// ---------------------------------------------------------------------------
// Register-block extraction
// ---------------------------------------------------------------------------

fn extract_register_block<'tcx>(
    tcx: TyCtxt<'tcx>,
    name: &str,
    block_attrs: &[Attribute],
    variant_data: &rustc_hir::VariantData<'tcx>,
) -> Result<RegisterBlockDecl, RegisterAccessError> {
    // Parse all block-level properties from #[rseq::register_block(...)]
    let block_attr = block_attrs
        .iter()
        .find(|a| rseq_attr_name(a) == Some("register_block"));
    let block_properties = match block_attr {
        Some(attr) => parse_block_properties(attr)?,
        None => Vec::new(),
    };

    // Extract access_width from the property bag for byte-offset computation.
    let block_access_width = block_properties.iter().find_map(|(k, v)| {
        if k == "access_width" {
            if let BlockPropertyValue::Int(n) = v {
                Some(*n)
            } else {
                None
            }
        } else {
            None
        }
    });

    let element_size = block_access_width.map(|w| w / 8);
    let mut current_offset: u32 = 0;
    let mut fields = Vec::new();

    for field in variant_data.fields() {
        let field_name = field.ident.name.as_str().to_string();
        let field_attrs = tcx.hir_attrs(field.hir_id);
        let reg_attr = field_attrs
            .iter()
            .find(|a| rseq_attr_name(a) == Some("register"));

        let (access_width, endian, side_effects, hw_updates) = match reg_attr {
            Some(attr) => parse_register_metadata(attr)?,
            None => (None, None, None, None),
        };

        let wrapper_name = extract_field_wrapper_type_name(field.ty);
        let access_kind = wrapper_name
            .as_deref()
            .and_then(wrapper_to_access_kind)
            .map(|s| s.to_string());
        let register_type_name = extract_register_type_name(field.ty);
        let array_count = extract_field_array_count(tcx, field);
        let byte_offset = element_size.map(|_| current_offset);

        if let Some(es) = element_size {
            current_offset += array_count.unwrap_or(1) * es;
        }

        fields.push(RegisterFieldDecl {
            name: field_name,
            access_width,
            endian,
            side_effects,
            hw_updates,
            register_type_name,
            byte_offset,
            access_kind,
            array_count,
        });
    }

    Ok(RegisterBlockDecl {
        name: name.to_string(),
        properties: block_properties,
        fields,
    })
}

type RegisterMetadata = (Option<u32>, Option<String>, Option<String>, Option<String>);

fn parse_register_metadata(attr: &Attribute) -> Result<RegisterMetadata, RegisterAccessError> {
    let Attribute::Unparsed(item) = attr else {
        return Err(malformed("expected tool attribute"));
    };
    let tokens = match &item.args {
        AttrArgs::Delimited(d) => &d.tokens,
        _ => return Ok((None, None, None, None)),
    };

    let trees: Vec<&TokenTree> = tokens.iter().collect();
    let mut access_width = None;
    let mut endian = None;
    let mut side_effects = None;
    let mut hw_updates = None;
    let mut i = 0;

    while i < trees.len() {
        if is_comma(trees[i]) {
            i += 1;
            continue;
        }

        let key = extract_ident(trees[i])
            .ok_or_else(|| malformed("expected key in register attribute"))?;
        i += 1;

        if i >= trees.len() || !is_eq(trees[i]) {
            return Err(malformed(&format!("expected '=' after '{key}'")));
        }
        i += 1;

        if i >= trees.len() {
            return Err(malformed(&format!("expected value for '{key}'")));
        }

        match key.as_str() {
            "access_width" => {
                let val = extract_integer_literal(trees[i]).ok_or_else(|| {
                    RegisterAccessError::InvalidAccessWidth(
                        "access_width must be an integer literal".to_string(),
                    )
                })?;
                access_width = Some(val);
            }
            "endian" => {
                endian = extract_string_literal(trees[i]);
            }
            "side_effects" => {
                side_effects = extract_string_literal(trees[i]);
            }
            "hw_updates" => {
                hw_updates = extract_string_literal(trees[i]);
            }
            _ => {
                // Unknown keys are ignored for forward compatibility.
            }
        }
        i += 1;
    }

    Ok((access_width, endian, side_effects, hw_updates))
}

/// Parse all key=value properties from a `#[rseq::register_block(...)]` attribute
/// into a generic property bag, preserving every authored property.
fn parse_block_properties(
    attr: &Attribute,
) -> Result<Vec<(String, BlockPropertyValue)>, RegisterAccessError> {
    let Attribute::Unparsed(item) = attr else {
        return Err(malformed("expected tool attribute"));
    };
    let tokens = match &item.args {
        AttrArgs::Delimited(d) => &d.tokens,
        _ => return Ok(Vec::new()),
    };

    let trees: Vec<&TokenTree> = tokens.iter().collect();
    let mut properties = Vec::new();
    let mut i = 0;

    while i < trees.len() {
        if is_comma(trees[i]) {
            i += 1;
            continue;
        }

        let key = extract_ident(trees[i])
            .ok_or_else(|| malformed("expected key in register_block attribute"))?;
        i += 1;

        if i >= trees.len() || !is_eq(trees[i]) {
            return Err(malformed(&format!("expected '=' after '{key}'")));
        }
        i += 1;

        if i >= trees.len() {
            return Err(malformed(&format!("expected value for '{key}'")));
        }

        let value = if let Some(int_val) = extract_integer_literal(trees[i]) {
            BlockPropertyValue::Int(int_val)
        } else if let Some(str_val) = extract_string_literal(trees[i]) {
            BlockPropertyValue::Str(str_val)
        } else if let Some(bool_val) = extract_boolean_literal(trees[i]) {
            BlockPropertyValue::Bool(bool_val)
        } else {
            return Err(malformed(&format!("unsupported value type for '{key}'")));
        };

        properties.push((key, value));
        i += 1;
    }

    Ok(properties)
}

// ---------------------------------------------------------------------------
// Register-block field type helpers
// ---------------------------------------------------------------------------

/// Extract the wrapper type name from a register-block field type.
///
/// For `ReadWrite<u32, DR_REG::Register>` returns `Some("ReadWrite")`.
/// For `[Reserved<u32>; 4]` returns `Some("Reserved")`.
fn extract_field_wrapper_type_name(ty: &rustc_hir::Ty<'_>) -> Option<String> {
    let inner = match &ty.kind {
        rustc_hir::TyKind::Array(elem, _) => *elem,
        _ => ty,
    };
    let rustc_hir::TyKind::Path(qpath) = &inner.kind else {
        return None;
    };
    let path = match qpath {
        rustc_hir::QPath::Resolved(_, path) => path,
        _ => return None,
    };
    path.segments
        .last()
        .map(|s| s.ident.name.as_str().to_string())
}

/// Map a wrapper type name to its access kind abbreviation.
fn wrapper_to_access_kind(wrapper: &str) -> Option<&'static str> {
    match wrapper {
        "ReadWrite" => Some("RW"),
        "ReadOnly" => Some("R"),
        "WriteOnly" => Some("W"),
        _ => None,
    }
}

/// Extract the register descriptor type name from the second generic argument.
///
/// For `ReadWrite<u32, DR_REG::Register>` returns `Some("DR_REG")`.
/// For `Reserved<u32>` or arrays of reserved types, returns `None`.
fn extract_register_type_name(ty: &rustc_hir::Ty<'_>) -> Option<String> {
    let inner = match &ty.kind {
        rustc_hir::TyKind::Array(elem, _) => *elem,
        _ => ty,
    };
    let rustc_hir::TyKind::Path(qpath) = &inner.kind else {
        return None;
    };
    let path = match qpath {
        rustc_hir::QPath::Resolved(_, path) => path,
        _ => return None,
    };
    let last_seg = path.segments.last()?;
    let args = last_seg.args?;
    let mut type_idx = 0;
    for arg in args.args.iter() {
        if let rustc_hir::GenericArg::Type(inner_ty) = arg {
            if type_idx == 1 {
                if let rustc_hir::TyKind::Path(rustc_hir::QPath::Resolved(_, inner_path)) =
                    &inner_ty.kind
                {
                    if !inner_path.segments.is_empty() {
                        return Some(inner_path.segments[0].ident.name.as_str().to_string());
                    }
                }
                return None;
            }
            type_idx += 1;
        }
    }
    None
}

/// Extract the array element count from a struct field's resolved type.
///
/// Returns `Some(n)` for array-typed fields like `[Reserved<u32>; n]`,
/// `None` for non-array fields.
fn extract_field_array_count<'tcx>(
    tcx: TyCtxt<'tcx>,
    field: &rustc_hir::FieldDef<'tcx>,
) -> Option<u32> {
    let field_ty = tcx.type_of(field.def_id).instantiate_identity();
    if let rustc_middle::ty::TyKind::Array(_, len_const) = field_ty.kind() {
        len_const.try_to_target_usize(tcx).map(|v| v as u32)
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// MMIO field extraction
// ---------------------------------------------------------------------------

fn extract_mmio_fields<'tcx>(
    tcx: TyCtxt<'tcx>,
    variant_data: &rustc_hir::VariantData<'tcx>,
) -> Result<Vec<MmioFieldDecl>, RegisterAccessError> {
    let mut mmio_fields = Vec::new();

    for field in variant_data.fields() {
        if !is_mmio_type(field.ty) {
            continue;
        }

        let attrs = tcx.hir_attrs(field.hir_id);
        let mmio_attr = attrs.iter().find(|a| rseq_attr_name(a) == Some("mmio"));

        let decl = parse_mmio_field(mmio_attr, field)?;
        mmio_fields.push(decl);
    }

    Ok(mmio_fields)
}

fn parse_mmio_field(
    attr: Option<&Attribute>,
    field: &rustc_hir::FieldDef<'_>,
) -> Result<MmioFieldDecl, RegisterAccessError> {
    let field_name = field.ident.name.as_str().to_string();
    let register_block_type = extract_first_generic_arg_name(field.ty);

    let mmio_id = if let Some(attr) = attr {
        let Attribute::Unparsed(item) = attr else {
            return Err(malformed("expected tool attribute"));
        };
        let tokens = match &item.args {
            AttrArgs::Delimited(d) => &d.tokens,
            _ => return Err(RegisterAccessError::MissingMmioId),
        };

        let trees: Vec<&TokenTree> = tokens.iter().collect();
        let mut id = None;
        let mut i = 0;

        while i < trees.len() {
            if is_comma(trees[i]) {
                i += 1;
                continue;
            }

            let key = extract_ident(trees[i])
                .ok_or_else(|| malformed("expected key in mmio attribute"))?;
            i += 1;

            if i >= trees.len() || !is_eq(trees[i]) {
                return Err(malformed(&format!("expected '=' after '{key}'")));
            }
            i += 1;

            if i >= trees.len() {
                return Err(malformed(&format!("expected value for '{key}'")));
            }

            if key == "id" {
                id = extract_string_literal(trees[i]);
            }
            i += 1;
        }

        Some(id.ok_or(RegisterAccessError::MissingMmioId)?)
    } else {
        None
    };

    Ok(MmioFieldDecl {
        field_name,
        mmio_id,
        register_block_type,
    })
}

// ---------------------------------------------------------------------------
// HIR type helpers
// ---------------------------------------------------------------------------

/// Extract the first generic type argument name from a HIR type.
///
/// For `Mmio<Pl011Regs>`, returns `Some("Pl011Regs")`.
fn extract_first_generic_arg_name(ty: &rustc_hir::Ty<'_>) -> Option<String> {
    let rustc_hir::TyKind::Path(qpath) = &ty.kind else {
        return None;
    };
    let path = match qpath {
        rustc_hir::QPath::Resolved(_, path) => path,
        _ => return None,
    };
    let last_seg = path.segments.last()?;
    let args = last_seg.args?;
    for arg in args.args.iter() {
        if let rustc_hir::GenericArg::Type(inner_ty) = arg {
            if let rustc_hir::TyKind::Path(rustc_hir::QPath::Resolved(_, inner_path)) =
                &inner_ty.kind
            {
                return Some(
                    inner_path
                        .segments
                        .iter()
                        .map(|s| s.ident.name.as_str().to_string())
                        .collect::<Vec<_>>()
                        .join("::"),
                );
            }
            return None;
        }
    }
    None
}

fn is_mmio_type(ty: &rustc_hir::Ty<'_>) -> bool {
    let rustc_hir::TyKind::Path(qpath) = &ty.kind else {
        return false;
    };
    let path = match qpath {
        rustc_hir::QPath::Resolved(_, path) => path,
        _ => return false,
    };
    path.segments
        .last()
        .map(|seg| seg.ident.name.as_str() == "Mmio")
        .unwrap_or(false)
}

// ---------------------------------------------------------------------------
// Call-site collection
// ---------------------------------------------------------------------------

fn collect_impl_item_call_sites<'tcx>(
    tcx: TyCtxt<'tcx>,
    impl_item: &'tcx rustc_hir::ImplItem<'tcx>,
    wrapper_sites: &mut Vec<WrapperCallSite>,
    poll_sites: &mut Vec<PollCallSite>,
) {
    if let ImplItemKind::Fn(_sig, body_id) = impl_item.kind {
        let fn_name = impl_item.ident.name.as_str().to_string();
        let body = tcx.hir_body(body_id);
        let typeck_results = tcx.typeck(impl_item.owner_id.def_id);
        let mut visitor = CallSiteVisitor {
            tcx,
            typeck_results,
            current_fn: fn_name,
            wrapper_sites: Vec::new(),
            poll_sites: Vec::new(),
        };
        visitor.visit_expr(body.value);
        wrapper_sites.extend(visitor.wrapper_sites);
        poll_sites.extend(visitor.poll_sites);
    }
}

struct CallSiteVisitor<'tcx> {
    tcx: TyCtxt<'tcx>,
    typeck_results: &'tcx rustc_middle::ty::TypeckResults<'tcx>,
    current_fn: String,
    wrapper_sites: Vec<WrapperCallSite>,
    poll_sites: Vec<PollCallSite>,
}

impl<'tcx> Visitor<'tcx> for CallSiteVisitor<'tcx> {
    fn visit_expr(&mut self, expr: &'tcx rustc_hir::Expr<'tcx>) {
        match expr.kind {
            ExprKind::MethodCall(segment, receiver, _, _) => {
                let method = match segment.ident.name.as_str() {
                    "read_raw" => Some(WrapperMethod::ReadRaw),
                    "read" => Some(WrapperMethod::Read),
                    "write" => Some(WrapperMethod::Write),
                    "modify" => Some(WrapperMethod::Modify),
                    "set" => Some(WrapperMethod::Set),
                    _ => None,
                };
                if let Some(method) = method.filter(|m| self.is_known_wrapper_method(receiver, *m))
                {
                    self.wrapper_sites.push(WrapperCallSite {
                        method,
                        containing_fn: self.current_fn.clone(),
                    });
                }
            }
            ExprKind::Call(callee, _) => {
                if is_poll_until_path(callee) {
                    self.poll_sites.push(PollCallSite {
                        containing_fn: self.current_fn.clone(),
                    });
                }
            }
            _ => {}
        }
        intravisit::walk_expr(self, expr);
    }
}

impl<'tcx> CallSiteVisitor<'tcx> {
    fn is_known_wrapper_method(
        &self,
        receiver: &'tcx rustc_hir::Expr<'tcx>,
        method: WrapperMethod,
    ) -> bool {
        let receiver_ty = self.typeck_results.expr_ty(receiver);
        match wrapper_type_name(self.tcx, receiver_ty).as_deref() {
            Some("ReadOnly") => matches!(method, WrapperMethod::ReadRaw | WrapperMethod::Read),
            Some("WriteOnly") => matches!(method, WrapperMethod::Write | WrapperMethod::Set),
            Some("ReadWrite") => true,
            _ => false,
        }
    }
}

fn wrapper_type_name<'tcx>(tcx: TyCtxt<'tcx>, ty: Ty<'tcx>) -> Option<String> {
    ty.peel_refs()
        .ty_adt_def()
        .map(|adt| tcx.item_name(adt.did()).as_str().to_string())
}

fn is_poll_until_path(expr: &rustc_hir::Expr<'_>) -> bool {
    if let ExprKind::Path(rustc_hir::QPath::Resolved(_, path)) = &expr.kind {
        let segs: Vec<&str> = path
            .segments
            .iter()
            .map(|s| s.ident.name.as_str())
            .collect();
        return segs.len() >= 2
            && segs[segs.len() - 2] == "poll"
            && segs[segs.len() - 1] == "until";
    }
    false
}

// ---------------------------------------------------------------------------
// Register-descriptor pattern matching
// ---------------------------------------------------------------------------

/// Check if a module matches the `register_bitfields!` expansion pattern:
/// a module containing `struct Register` with field sub-modules.
fn try_extract_register_descriptor<'tcx>(
    tcx: TyCtxt<'tcx>,
    module_name: &str,
    module: &rustc_hir::Mod<'tcx>,
) -> Option<RegisterDescriptorFact> {
    let has_register_struct = module.item_ids.iter().any(|&child_id| {
        let child = tcx.hir_item(child_id);
        if let ItemKind::Struct(ident, ..) = &child.kind {
            ident.name.as_str() == "Register"
        } else {
            false
        }
    });

    if !has_register_struct {
        return None;
    }

    let fields: Vec<RegisterDescriptorFieldFact> = module
        .item_ids
        .iter()
        .filter_map(|&child_id| {
            let child = tcx.hir_item(child_id);
            if let ItemKind::Mod(ident, field_module) = &child.kind {
                extract_descriptor_field(tcx, ident.name.as_str(), field_module)
            } else {
                None
            }
        })
        .collect();

    Some(RegisterDescriptorFact {
        register_module: module_name.to_string(),
        fields,
    })
}

fn extract_descriptor_field<'tcx>(
    tcx: TyCtxt<'tcx>,
    field_name: &str,
    field_module: &rustc_hir::Mod<'tcx>,
) -> Option<RegisterDescriptorFieldFact> {
    let mut offset = None;
    let mut width = None;
    let mut named_values = Vec::new();

    for &child_id in field_module.item_ids {
        let child = tcx.hir_item(child_id);
        let ItemKind::Const(ident, _, _, rhs) = child.kind else {
            continue;
        };
        let rustc_hir::ConstItemRhs::Body(body_id) = rhs else {
            continue;
        };
        let body = tcx.hir_body(body_id);
        let const_name = ident.name.as_str();

        if const_name == "FIELD" {
            let (field_offset, field_width) = parse_field_const(body.value)?;
            offset = Some(field_offset);
            width = Some(field_width);
            continue;
        }

        let field_offset = offset?;
        if let Some(value) = parse_field_value_const(body.value, field_offset) {
            named_values.push(RegisterNamedValueFact {
                name: const_name.to_string(),
                value,
            });
        }
    }

    Some(RegisterDescriptorFieldFact {
        field_name: field_name.to_string(),
        offset: offset?,
        width: width?,
        named_values,
    })
}

fn parse_field_const(expr: &rustc_hir::Expr<'_>) -> Option<(u32, u32)> {
    let ExprKind::Call(_, args) = expr.kind else {
        return None;
    };
    if args.len() != 2 {
        return None;
    }
    let mask = extract_u32_expr(&args[0])?;
    let shift = extract_u32_expr(&args[1])?;
    let width = (mask >> shift).count_ones();
    Some((shift, width))
}

fn parse_field_value_const(expr: &rustc_hir::Expr<'_>, offset: u32) -> Option<u32> {
    let ExprKind::Call(_, args) = expr.kind else {
        return None;
    };
    if args.len() != 2 {
        return None;
    }
    let value = extract_u32_expr(&args[1])?;
    Some(value >> offset)
}

fn extract_u32_expr(expr: &rustc_hir::Expr<'_>) -> Option<u32> {
    match expr.kind {
        ExprKind::Cast(inner, _) => extract_u32_expr(inner),
        ExprKind::Binary(op, lhs, rhs) => {
            let lhs = extract_u32_expr(lhs)?;
            let rhs = extract_u32_expr(rhs)?;
            match op.node {
                rustc_hir::BinOpKind::Shl => lhs.checked_shl(rhs),
                rustc_hir::BinOpKind::BitOr => Some(lhs | rhs),
                _ => None,
            }
        }
        ExprKind::Lit(lit) => match lit.node {
            rustc_ast::LitKind::Int(v, _) => u32::try_from(v.get()).ok(),
            _ => None,
        },
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Token helpers
// ---------------------------------------------------------------------------

fn malformed(msg: &str) -> RegisterAccessError {
    RegisterAccessError::MalformedAttribute(msg.to_string())
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

fn extract_integer_literal(tree: &TokenTree) -> Option<u32> {
    match tree {
        TokenTree::Token(tok, _) => match tok.kind {
            TokenKind::Literal(lit) if lit.kind == LitKind::Integer => {
                lit.symbol.as_str().parse::<u32>().ok()
            }
            _ => None,
        },
        _ => None,
    }
}

fn extract_boolean_literal(tree: &TokenTree) -> Option<bool> {
    match tree {
        TokenTree::Token(tok, _) => match tok.kind {
            TokenKind::Ident(sym, _) if sym.as_str() == "true" => Some(true),
            TokenKind::Ident(sym, _) if sym.as_str() == "false" => Some(false),
            _ => None,
        },
        _ => None,
    }
}
