//! Semantic-model-to-C emission engine.
//!
//! Translates `SemanticBody` (Stmt/Expr trees with typed locals) into
//! Linux-specific C code. All device-specific content comes from plan data
//! and the SemanticModel; this module contains only Linux-specific API
//! mappings that are device-neutral.
//!
//! Replaces the legacy `emit_behavior.rs` BehaviorNode-to-C engine.

use std::collections::HashSet;

use regseq_model::semantic::body::{Block, SemanticBody};
use regseq_model::semantic::expr::Expr;
use regseq_model::semantic::local::{LocalId, LocalOrigin, SemanticLocal};
use regseq_model::semantic::model::SemanticModel;
use regseq_model::semantic::ops::{
    AssignTarget, BinOp, FieldWriteValue, PinCtrlState, ResourceOperation, ResourceProperty,
    UnaryOp,
};
use regseq_model::semantic::refs::{SubsystemActionKind, SubsystemKind};
use regseq_model::semantic::stmt::Stmt;
use regseq_model::semantic::types::{ResourceKind, SemanticType};

use super::plan::LinuxRegisterLayout;

// ---------------------------------------------------------------------------
// Emission context
// ---------------------------------------------------------------------------

/// Context for translating SemanticBody to C code.
pub(super) struct SemanticEmitContext<'a> {
    /// Snake_case driver name (e.g., `"pl011_full_spec"`).
    pub snake: &'a str,
    /// SCREAMING_SNAKE driver name (e.g., `"PL011_FULL_SPEC"`).
    pub upper: String,
    /// Register define prefix (e.g., `"PL011"`).
    pub reg_prefix: &'a str,
    /// Linux register layout (for named value resolution).
    pub register_layout: &'a LinuxRegisterLayout,
    /// Semantic model (for constant/interrupt class lookup).
    #[allow(dead_code)]
    pub model: &'a SemanticModel,
    /// Current function's local environment.
    pub locals: &'a [SemanticLocal],
}

// ---------------------------------------------------------------------------
// Return mode — how Stmt::Return is lowered to C
// ---------------------------------------------------------------------------

/// Controls how `Stmt::Return` is lowered to C.
///
/// Passed through `emit_body_stmts` so that every return site is adapted
/// inline, not as a post-body wrapper step.
#[derive(Clone)]
pub(super) enum ReturnMode {
    /// Emit returns directly.
    Direct,
    /// Adapt bool returns to unsigned int with true/false mapping.
    BoolToUint {
        true_value: String,
        false_value: String,
    },
    /// Semantic function returns Result. Error-path returns pass through.
    /// Success path `Return { None }` → `return 0;`.
    ResultToInt,
    /// Semantic function returns Result but Linux callback returns void.
    /// All returns become bare `return;`.
    DropResult,
}

// ---------------------------------------------------------------------------
// Naming helpers — carried over from emit_behavior.rs
// ---------------------------------------------------------------------------

/// Generate the C define name for a register offset.
///
/// e.g., field `"dr"` with prefix `"DEV"` → `"DEV_DR"`.
pub(super) fn register_offset_define(reg_prefix: &str, field_name: &str) -> String {
    format!("{}_{}", reg_prefix, field_name.to_uppercase())
}

/// Generate the C define name for a bitfield mask.
///
/// e.g., descriptor `"DR_REG"`, field `"OE"`, prefix `"DEV"` → `"DEV_DR_OE"`.
pub(super) fn bitfield_mask_define(reg_prefix: &str, desc_name: &str, bf_name: &str) -> String {
    let desc_prefix = normalize_desc_prefix(desc_name);
    format!("{}_{}_{}", reg_prefix, desc_prefix, bf_name)
}

/// Generate the C define name for a named value.
pub(super) fn named_value_define(
    reg_prefix: &str,
    desc_name: &str,
    bf_name: &str,
    nv_name: &str,
) -> String {
    let desc_prefix = normalize_desc_prefix(desc_name);
    format!("{}_{}_{}_{}", reg_prefix, desc_prefix, bf_name, nv_name)
}

/// Generate the C define name for an interrupt class mask.
pub(super) fn interrupt_class_define(upper: &str, class_name: &str) -> String {
    format!("{}_{}_IRQ_MASK", upper, class_name.to_uppercase())
}

/// Normalize a descriptor name to a C define prefix.
///
/// Strips trailing `"_REG"` if present: `"DR_REG"` → `"DR"`.
pub(super) fn normalize_desc_prefix(name: &str) -> &str {
    name.strip_suffix("_REG").unwrap_or(name)
}

// ---------------------------------------------------------------------------
// Register layout helpers
// ---------------------------------------------------------------------------

/// Find the C define name for a bitfield mask from the register layout.
fn find_bitfield_mask_define(ctx: &SemanticEmitContext<'_>, register: &str, field: &str) -> String {
    for desc in &ctx.register_layout.descriptors {
        let desc_prefix = normalize_desc_prefix(&desc.register_name);
        if desc_prefix.eq_ignore_ascii_case(register)
            || desc_prefix.eq_ignore_ascii_case(&register.replace('_', ""))
        {
            for bf in &desc.bit_fields {
                if bf.name.eq_ignore_ascii_case(field) {
                    return format!(
                        "{}_{}",
                        ctx.reg_prefix,
                        bitfield_mask_define("", desc_prefix, &bf.name)
                    )
                    .trim_start_matches('_')
                    .to_string();
                }
            }
        }
    }
    // Fallback: construct from convention.
    format!(
        "{}_{}_{}",
        ctx.reg_prefix,
        register.to_uppercase(),
        field.to_uppercase()
    )
}

/// Resolve a named value define from the register layout.
fn resolve_named_value_define(
    ctx: &SemanticEmitContext<'_>,
    register: &str,
    field: &str,
    value_name: &str,
) -> String {
    for desc in &ctx.register_layout.descriptors {
        let desc_prefix = normalize_desc_prefix(&desc.register_name);
        if desc_prefix.eq_ignore_ascii_case(register)
            || desc_prefix.eq_ignore_ascii_case(&register.replace('_', ""))
        {
            for bf in &desc.bit_fields {
                if bf.name.eq_ignore_ascii_case(field) {
                    for nv in &bf.named_values {
                        if nv.name.eq_ignore_ascii_case(value_name) {
                            return named_value_define(
                                ctx.reg_prefix,
                                desc_prefix,
                                &bf.name,
                                &nv.name,
                            );
                        }
                    }
                }
            }
        }
    }
    // Fallback: construct from convention.
    format!(
        "{}_{}_{}_{}",
        ctx.reg_prefix,
        register.to_uppercase().replace('_', ""),
        field.to_uppercase(),
        value_name.to_uppercase()
    )
}

// ---------------------------------------------------------------------------
// SemanticType → C type mapping
// ---------------------------------------------------------------------------

/// Map a SemanticType to its C type string.
///
/// The external type/enum realization tables are current-slice-bounded
/// conventions keyed on the `name` field of `SemanticType::Struct/Enum`.
pub(super) fn semantic_type_to_c(ty: &SemanticType, snake: &str) -> String {
    match ty {
        SemanticType::Bool => "bool".into(),
        SemanticType::U8 => "u8".into(),
        SemanticType::U16 => "u16".into(),
        SemanticType::U32 => "u32".into(),
        SemanticType::U64 => "u64".into(),
        SemanticType::I8 => "s8".into(),
        SemanticType::I16 => "s16".into(),
        SemanticType::I32 => "s32".into(),
        SemanticType::I64 => "s64".into(),
        SemanticType::Void => "void".into(),
        SemanticType::DriverData => format!("struct {snake}_data *"),
        SemanticType::RegisterValue { width } => format!("u{width}"),
        SemanticType::RegisterBlock { .. } => "void __iomem *".into(),
        SemanticType::ResourceHandle { kind } => match kind {
            ResourceKind::Clock => "struct clk *".into(),
            ResourceKind::Reset => "struct reset_control *".into(),
            ResourceKind::PinCtrl => "struct pinctrl *".into(),
            ResourceKind::Interrupt => "int".into(),
            ResourceKind::Property => "u32".into(),
            ResourceKind::Mmio => "void __iomem *".into(),
        },
        SemanticType::Struct { name, .. } => match name.as_str() {
            "SerialErrors" => "unsigned int".into(),
            "SerialConfig" => "const struct ktermios *".into(),
            // RxChar flows inline — local type depends on context.
            "RxChar" => "u32".into(),
            other => panic!("unrecognized external struct type for C emission: {other}"),
        },
        SemanticType::Enum { name, .. } => match name.as_str() {
            "IrqResult" => "irqreturn_t".into(),
            "RegseqError" => "int".into(),
            other => panic!("unrecognized external enum type for C emission: {other}"),
        },
        SemanticType::Ref { inner } => {
            let inner_c = semantic_type_to_c(inner, snake);
            if inner_c.ends_with('*') {
                format!("const {inner_c}")
            } else {
                format!("const {inner_c} *")
            }
        }
        SemanticType::RefMut { inner } => {
            let inner_c = semantic_type_to_c(inner, snake);
            if inner_c.ends_with('*') {
                inner_c
            } else {
                format!("{inner_c} *")
            }
        }
        SemanticType::Array { element, len } => {
            let elem_c = semantic_type_to_c(element, snake);
            format!("{elem_c}[{len}]")
        }
        SemanticType::Option { inner } => {
            if is_pointer_type(inner) {
                // Nullable pointer — same as inner type.
                semantic_type_to_c(inner, snake)
            } else {
                // Presence-flagged struct for non-pointer options.
                let inner_c = semantic_type_to_c(inner, snake);
                format!("struct {{ bool has; {inner_c} val; }}")
            }
        }
        SemanticType::Result { .. } => "int".into(),
        SemanticType::Opaque { description } => {
            panic!("opaque type reached C emission: {description}")
        }
    }
}

/// Check whether a SemanticType is a pointer type in C (uses NULL sentinel for Option).
fn is_pointer_type(ty: &SemanticType) -> bool {
    matches!(
        ty,
        SemanticType::DriverData
            | SemanticType::RegisterBlock { .. }
            | SemanticType::ResourceHandle {
                kind: ResourceKind::Clock
                    | ResourceKind::Reset
                    | ResourceKind::PinCtrl
                    | ResourceKind::Mmio
            }
    )
}

// ---------------------------------------------------------------------------
// Enum-typed IntLit → Linux constant mapping
// ---------------------------------------------------------------------------

/// Resolve an enum-typed integer literal to its Linux C constant.
///
/// Current-slice-bounded convention: keyed on (short_name, variant_index).
fn resolve_enum_variant(name: &str, value: i128) -> String {
    match (name, value) {
        ("IrqResult", 0) => "IRQ_HANDLED".into(),
        ("IrqResult", 1) => "IRQ_NONE".into(),
        ("RegseqError", 0) => "-EIO".into(),
        ("RegseqError", 1) => "-EINVAL".into(),
        ("RegseqError", 2) => "-ETIMEDOUT".into(),
        _ => panic!("unrecognized external enum variant: {name}::{value}"),
    }
}

// ---------------------------------------------------------------------------
// External callable realization
// ---------------------------------------------------------------------------

/// Realize an external callable to C.
///
/// Current-slice-bounded: keyed on canonical_path from FunctionRef.
fn realize_external_callable(canonical_path: &str, args: &[String]) -> String {
    match canonical_path {
        "regseq::RxChar::new" => {
            // Inline: args are (byte, flag). The result flows through a local
            // and is consumed at subsystem action boundaries.
            // For now, emit as the first arg (byte value) since RxChar is
            // destructured at the uart_insert_char call site.
            if args.len() >= 2 {
                // Emit as compound literal — byte is the primary value
                args[0].clone()
            } else if args.len() == 1 {
                args[0].clone()
            } else {
                "0".into()
            }
        }
        "regseq::SerialErrors::empty" => "0".into(),
        _ => panic!("unrecognized external callable in C emission: {canonical_path}"),
    }
}

// ---------------------------------------------------------------------------
// Fallible resource operation table
// ---------------------------------------------------------------------------

/// Check if a ResourceOp is fallible in the Linux API.
fn is_fallible_resource_op(kind: ResourceKind, op: &ResourceOperation) -> bool {
    matches!(
        (kind, op),
        (ResourceKind::Clock, ResourceOperation::Enable)
            | (ResourceKind::PinCtrl, ResourceOperation::ApplyState { .. })
    )
}

/// Linux API call for a resource operation.
fn resource_op_api(kind: ResourceKind, op: &ResourceOperation, field_name: &str) -> String {
    match (kind, op) {
        (ResourceKind::Clock, ResourceOperation::Enable) => {
            format!("clk_prepare_enable(d->{field_name})")
        }
        (ResourceKind::Clock, ResourceOperation::Disable) => {
            format!("clk_disable_unprepare(d->{field_name})")
        }
        (ResourceKind::Reset, ResourceOperation::Toggle { .. }) => {
            format!("reset_control_reset(d->{field_name})")
        }
        (ResourceKind::PinCtrl, ResourceOperation::ApplyState { state }) => match state {
            PinCtrlState::Default => "pinctrl_pm_select_default_state(dev)".into(),
            PinCtrlState::Sleep => "pinctrl_pm_select_sleep_state(dev)".into(),
        },
        _ => format!("/* TODO: resource op on {field_name} */"),
    }
}

// ---------------------------------------------------------------------------
// Expression emission
// ---------------------------------------------------------------------------

/// Emit a semantic expression as a C expression string.
pub(super) fn emit_expr(expr: &Expr, ctx: &SemanticEmitContext<'_>) -> String {
    match expr {
        Expr::IntLit { value, ty } => {
            if let SemanticType::Enum { name, .. } = ty {
                return resolve_enum_variant(name, *value);
            }
            if *value > 9 || *value < 0 {
                format!("{value:#x}U")
            } else {
                format!("{value}")
            }
        }
        Expr::BoolLit { value } => {
            if *value {
                "true".into()
            } else {
                "false".into()
            }
        }
        Expr::LocalRef { local } => resolve_local_name(ctx.locals, *local),
        Expr::DriverFieldRead { field } => format!("d->{}", field.name),
        Expr::Constant { constant } => constant.stable_name.clone(),
        Expr::BinaryOp { op, lhs, rhs } => {
            let l = emit_expr(lhs, ctx);
            let r = emit_expr(rhs, ctx);
            let op_str = binop_to_c(*op);
            format!("({l} {op_str} {r})")
        }
        Expr::UnaryOp { op, operand } => {
            let inner = emit_expr(operand, ctx);
            match op {
                UnaryOp::Not => format!("!({inner})"),
                UnaryOp::Neg => format!("-({inner})"),
                UnaryOp::BitNot => format!("~({inner})"),
            }
        }
        Expr::Cast { expr: inner, ty } => {
            let c_type = semantic_type_to_c(ty, ctx.snake);
            let inner_c = emit_expr(inner, ctx);
            format!("({c_type})({inner_c})")
        }
        Expr::RegReadRaw { register } => {
            let reg_define = register_offset_define(ctx.reg_prefix, &register.name);
            format!("{}_reg_read(d, {reg_define})", ctx.snake)
        }
        Expr::RegFieldRead { register, field } => {
            let reg_define = register_offset_define(ctx.reg_prefix, &register.name);
            let mask = find_bitfield_mask_define(ctx, &register.name, &field.name);
            format!("({}_reg_read(d, {reg_define}) & {mask})", ctx.snake)
        }
        Expr::FieldExtract {
            register: _,
            field,
            raw_value,
        } => {
            let raw = emit_expr(raw_value, ctx);
            let mask = find_bitfield_mask_define(ctx, &field.register.name, &field.name);
            format!("({raw} & {mask})")
        }
        Expr::HelperCall { callee, args } => {
            let arg_strs: Vec<String> = args.iter().map(|a| emit_expr(a, ctx)).collect();
            // Dispatch: authored helper vs external callable.
            let path_str = callee.canonical_path.as_str();
            if path_str.starts_with("regseq::") {
                realize_external_callable(path_str, &arg_strs)
            } else {
                let all_args = if arg_strs.is_empty() {
                    "d".to_string()
                } else {
                    format!("d, {}", arg_strs.join(", "))
                };
                format!(
                    "{}_{callee_name}({all_args})",
                    ctx.snake,
                    callee_name = callee.name
                )
            }
        }
        Expr::ResourcePropertyRead { resource, property } => match property {
            ResourceProperty::ClockRate => {
                format!("clk_get_rate(d->{})", resource.field.name)
            }
            _ => panic!("unsupported resource property read: {property:?}"),
        },
        Expr::WithDefault { value, default } => {
            let default_c = emit_expr(default, ctx);
            // Extract resource field from value to derive the presence condition.
            if let Expr::ResourcePropertyRead { resource, .. } = value.as_ref() {
                let field = &resource.field.name;
                let val_c = emit_expr(value, ctx);
                format!("(d->{field} ? {val_c} : {default_c})")
            } else {
                // Fallback: emit as-is (shouldn't happen in current slice).
                let val_c = emit_expr(value, ctx);
                format!("({val_c} ? {val_c} : {default_c})")
            }
        }
        Expr::InterruptClassTest { status, class_ref } => {
            let status_c = emit_expr(status, ctx);
            let mask = interrupt_class_define(&ctx.upper, &class_ref.name);
            format!("({status_c} & {mask})")
        }
        Expr::ResourcePresent { resource } => {
            format!("(d->{} != NULL)", resource.field.name)
        }
        Expr::IsSome { inner } => {
            // Dispatch on inner type for carrier-appropriate test.
            let inner_c = emit_expr(inner, ctx);
            if is_option_pointer_context(inner, ctx) {
                format!("({inner_c} != NULL)")
            } else {
                format!("{inner_c}.has")
            }
        }
        Expr::OptionNone => {
            // Context-dependent: caller must handle based on type.
            "0".into()
        }
        Expr::OptionSome { inner } => {
            // For pointer types, just emit the inner value.
            // For non-pointer types, the caller (FieldAssign) handles carrier wrapping.
            emit_expr(inner, ctx)
        }
        Expr::OptionUnwrap { inner } => {
            let inner_c = emit_expr(inner, ctx);
            if is_option_pointer_context(inner, ctx) {
                inner_c
            } else {
                format!("{inner_c}.val")
            }
        }
    }
}

/// Determine if an option expression is over a pointer type (uses NULL sentinel).
fn is_option_pointer_context(expr: &Expr, ctx: &SemanticEmitContext<'_>) -> bool {
    match expr {
        Expr::DriverFieldRead { field } => {
            // Check if the field name corresponds to a known resource handle.
            // Resource handles (clock, reset, pinctrl) are pointer-typed.
            let name = &field.name;
            name == "clock" || name == "reset" || name == "pinctrl" || name == "regs"
        }
        Expr::LocalRef { local } => {
            if let Some(l) = ctx.locals.iter().find(|l| l.id == *local) {
                if let SemanticType::Option { inner } = &l.ty {
                    return is_pointer_type(inner);
                }
            }
            false
        }
        Expr::ResourcePresent { .. } => true,
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// Statement emission
// ---------------------------------------------------------------------------

/// Emit a block of statements.
fn emit_block(
    block: &Block,
    ctx: &SemanticEmitContext<'_>,
    indent: usize,
    return_mode: &ReturnMode,
) -> Vec<String> {
    let mut lines = Vec::new();
    for stmt in &block.stmts {
        lines.extend(emit_stmt(stmt, ctx, indent, return_mode));
    }
    lines
}

/// Emit a single statement as C code lines.
fn emit_stmt(
    stmt: &Stmt,
    ctx: &SemanticEmitContext<'_>,
    indent: usize,
    return_mode: &ReturnMode,
) -> Vec<String> {
    let ind = "\t".repeat(indent);
    match stmt {
        Stmt::Let { local, value } => {
            let name = resolve_local_name(ctx.locals, *local);
            let val = emit_expr(value, ctx);
            vec![format!("{ind}{name} = {val};")]
        }
        Stmt::Assign { target, value } => {
            let val = emit_expr(value, ctx);
            match target {
                AssignTarget::Local { local } => {
                    let name = resolve_local_name(ctx.locals, *local);
                    vec![format!("{ind}{name} = {val};")]
                }
                AssignTarget::DriverField { field } => {
                    vec![format!("{ind}d->{} = {val};", field.name)]
                }
            }
        }
        Stmt::CompoundAssign { target, op, value } => {
            let val = emit_expr(value, ctx);
            let op_str = binop_to_c(*op);
            match target {
                AssignTarget::Local { local } => {
                    let name = resolve_local_name(ctx.locals, *local);
                    vec![format!("{ind}{name} {op_str}= {val};")]
                }
                AssignTarget::DriverField { field } => {
                    vec![format!("{ind}d->{} {op_str}= {val};", field.name)]
                }
            }
        }
        Stmt::Eval { expr } => {
            let val = emit_expr(expr, ctx);
            vec![format!("{ind}{val};")]
        }
        Stmt::Branch {
            condition,
            then_body,
            else_body,
        } => {
            let mut lines = Vec::new();
            let cond = emit_expr(condition, ctx);
            lines.push(format!("{ind}if ({cond}) {{"));
            lines.extend(emit_block(then_body, ctx, indent + 1, return_mode));
            if !else_body.stmts.is_empty() {
                lines.push(format!("{ind}}} else {{"));
                lines.extend(emit_block(else_body, ctx, indent + 1, return_mode));
            }
            lines.push(format!("{ind}}}"));
            lines
        }
        Stmt::Loop { condition, body } => {
            let mut lines = Vec::new();
            let cond = emit_expr(condition, ctx);
            lines.push(format!("{ind}while ({cond}) {{"));
            lines.extend(emit_block(body, ctx, indent + 1, return_mode));
            lines.push(format!("{ind}}}"));
            lines
        }
        Stmt::OrderedRegion { body } => emit_block(body, ctx, indent, return_mode),
        Stmt::FallibleOp {
            body,
            success_local: _,
            error_local,
            cleanup_on_error,
        } => {
            let mut lines = Vec::new();
            // Emit body (contains the fallible call).
            lines.extend(emit_block(body, ctx, indent, return_mode));
            // Error check using the error local.
            let err_name = resolve_local_name(ctx.locals, *error_local);
            lines.push(format!("{ind}if ({err_name}) {{"));
            lines.extend(emit_block(cleanup_on_error, ctx, indent + 1, return_mode));
            // Error-path return, lowered by ReturnMode.
            let return_line = match return_mode {
                ReturnMode::Direct | ReturnMode::ResultToInt => {
                    format!("{ind}\treturn {err_name};")
                }
                ReturnMode::DropResult => format!("{ind}\treturn;"),
                ReturnMode::BoolToUint { .. } => {
                    // Should not happen in a fallible context.
                    format!("{ind}\treturn {err_name};")
                }
            };
            lines.push(return_line);
            lines.push(format!("{ind}}}"));
            lines
        }
        Stmt::Return { value } => {
            vec![format!("{ind}{}", emit_return(value, ctx, return_mode))]
        }
        Stmt::RegWrite { register, fields } => {
            let reg_define = register_offset_define(ctx.reg_prefix, &register.name);
            let value = emit_combined_write_value(fields, &register.name, ctx);
            vec![format!(
                "{ind}{}_reg_write(d, {reg_define}, {value});",
                ctx.snake
            )]
        }
        Stmt::RegModify { register, fields } => {
            let reg_define = register_offset_define(ctx.reg_prefix, &register.name);
            let (clr, set) = emit_modify_masks(fields, &register.name, ctx);
            vec![format!(
                "{ind}{}_reg_modify(d, {reg_define}, {clr}, {set});",
                ctx.snake
            )]
        }
        Stmt::ResourceOp {
            resource,
            operation,
        } => {
            let field_name = &resource.field.name;
            let kind = resource.kind;
            if is_fallible_resource_op(kind, operation) {
                // Backend-side fallible resource emission workaround.
                let api = resource_op_api(kind, operation, field_name);
                let return_line = match return_mode {
                    ReturnMode::DropResult => format!("{ind}\treturn;"),
                    _ => format!("{ind}\treturn ret;"),
                };
                vec![
                    format!("{ind}ret = {api};"),
                    format!("{ind}if (ret)"),
                    return_line,
                ]
            } else {
                let api = resource_op_api(kind, operation, field_name);
                vec![format!("{ind}{api};")]
            }
        }
        Stmt::FieldAssign { field, value } => {
            let val = emit_expr(value, ctx);
            vec![format!("{ind}d->{} = {val};", field.name)]
        }
        Stmt::SubsystemAction { action, args } => {
            let ind = &ind;
            let arg_strs: Vec<String> = args.iter().map(|a| emit_expr(a, ctx)).collect();
            match (&action.subsystem, &action.action) {
                (SubsystemKind::DeviceClassIo, SubsystemActionKind::IngestRxUnit) => {
                    // uart_insert_char expects: port, status, overrun_bit, ch, flag
                    // Args from the semantic IR carry the values needed.
                    if arg_strs.len() >= 4 {
                        vec![format!(
                            "{ind}uart_insert_char(&d->port, {}, {}, {}, {});",
                            arg_strs[0], arg_strs[1], arg_strs[2], arg_strs[3]
                        )]
                    } else {
                        vec![format!(
                            "{ind}uart_insert_char(&d->port, {});",
                            arg_strs.join(", ")
                        )]
                    }
                }
                (SubsystemKind::Buffering, SubsystemActionKind::PublishRxBatch) => {
                    vec![format!("{ind}tty_flip_buffer_push(&d->port.state->port);")]
                }
                _ => {
                    panic!(
                        "unrecognized subsystem action: {:?}::{:?}",
                        action.subsystem, action.action
                    );
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Return emission with ReturnMode dispatch
// ---------------------------------------------------------------------------

fn emit_return(
    value: &Option<Expr>,
    ctx: &SemanticEmitContext<'_>,
    return_mode: &ReturnMode,
) -> String {
    match return_mode {
        ReturnMode::Direct => match value {
            Some(expr) => format!("return {};", emit_expr(expr, ctx)),
            None => "return;".into(),
        },
        ReturnMode::BoolToUint {
            true_value,
            false_value,
        } => match value {
            Some(expr) => {
                let val = emit_expr(expr, ctx);
                format!("return ({val}) ? {true_value} : {false_value};")
            }
            None => panic!("BoolToUint return mode with no value"),
        },
        ReturnMode::ResultToInt => match value {
            Some(expr) => format!("return {};", emit_expr(expr, ctx)),
            None => "return 0;".into(),
        },
        ReturnMode::DropResult => "return;".into(),
    }
}

// ---------------------------------------------------------------------------
// Body emission (top-level)
// ---------------------------------------------------------------------------

/// Emit a function body with local declarations and statement emission.
///
/// `exclude_params` contains the LocalIds of params that are already bound
/// (via C function signature, container_of, or wrapper param bindings).
/// These are excluded from the local declaration block.
pub(super) fn emit_body_stmts(
    body: &SemanticBody,
    ctx: &SemanticEmitContext<'_>,
    exclude_params: &HashSet<LocalId>,
    return_mode: &ReturnMode,
) -> Vec<String> {
    let mut lines = Vec::new();

    // Check if any fallible ResourceOp exists in the body (needs `int ret;`).
    let needs_ret = body_has_fallible_resource_op(&body.body);

    // Emit local declarations (C89 style, top of function).
    let mut decl_count = 0;
    if needs_ret {
        lines.push("\tint ret;".into());
        decl_count += 1;
    }
    for local in &body.locals {
        if exclude_params.contains(&local.id) {
            continue;
        }
        if local.origin == LocalOrigin::ReturnPlace {
            continue;
        }
        let c_type = semantic_type_to_c(&local.ty, ctx.snake);
        lines.push(format!("\t{c_type} {};", local.name));
        decl_count += 1;
    }
    if decl_count > 0 {
        lines.push(String::new());
    }

    // Emit body statements.
    lines.extend(emit_block(&body.body, ctx, 0, return_mode));

    lines
}

/// Check if a block contains any fallible ResourceOp.
fn body_has_fallible_resource_op(block: &Block) -> bool {
    for stmt in &block.stmts {
        match stmt {
            Stmt::ResourceOp {
                resource,
                operation,
            } => {
                if is_fallible_resource_op(resource.kind, operation) {
                    return true;
                }
            }
            Stmt::Branch {
                then_body,
                else_body,
                ..
            } => {
                if body_has_fallible_resource_op(then_body)
                    || body_has_fallible_resource_op(else_body)
                {
                    return true;
                }
            }
            Stmt::Loop { body, .. } | Stmt::OrderedRegion { body } => {
                if body_has_fallible_resource_op(body) {
                    return true;
                }
            }
            Stmt::FallibleOp {
                body,
                cleanup_on_error,
                ..
            } => {
                if body_has_fallible_resource_op(body)
                    || body_has_fallible_resource_op(cleanup_on_error)
                {
                    return true;
                }
            }
            _ => {}
        }
    }
    false
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Resolve a LocalId to its C name from the local environment.
fn resolve_local_name(locals: &[SemanticLocal], id: LocalId) -> String {
    locals
        .iter()
        .find(|l| l.id == id)
        .map(|l| l.name.clone())
        .unwrap_or_else(|| format!("__unknown_local_{}", id.0))
}

/// Map a BinOp to its C operator string.
fn binop_to_c(op: BinOp) -> &'static str {
    match op {
        BinOp::Add => "+",
        BinOp::Sub => "-",
        BinOp::Mul => "*",
        BinOp::Div => "/",
        BinOp::Shl => "<<",
        BinOp::Shr => ">>",
        BinOp::BitAnd => "&",
        BinOp::BitOr => "|",
        BinOp::Eq => "==",
        BinOp::Ne => "!=",
        BinOp::And => "&&",
        BinOp::Or => "||",
    }
}

/// Emit a combined value for a register write from semantic field assignments.
fn emit_combined_write_value(
    fields: &[regseq_model::semantic::ops::FieldWrite],
    register: &str,
    ctx: &SemanticEmitContext<'_>,
) -> String {
    if fields.is_empty() {
        return "0".into();
    }
    // Check for raw write (single field with __raw__ name).
    if fields.len() == 1 && fields[0].field.name == "__raw__" {
        return emit_field_write_value(&fields[0].value, register, &fields[0].field.name, ctx);
    }
    let parts: Vec<String> = fields
        .iter()
        .filter_map(|fw| {
            let val = emit_field_write_value(&fw.value, register, &fw.field.name, ctx);
            if val == "0" {
                None
            } else {
                Some(val)
            }
        })
        .collect();
    if parts.is_empty() {
        "0".into()
    } else {
        parts.join(" | ")
    }
}

/// Emit clear and set masks for a register modify operation.
fn emit_modify_masks(
    fields: &[regseq_model::semantic::ops::FieldWrite],
    register: &str,
    ctx: &SemanticEmitContext<'_>,
) -> (String, String) {
    let mut clr_parts = Vec::new();
    let mut set_parts = Vec::new();

    for fw in fields {
        let mask = find_bitfield_mask_define(ctx, register, &fw.field.name);
        clr_parts.push(mask.clone());
        let val = emit_field_write_value(&fw.value, register, &fw.field.name, ctx);
        if val != "0" {
            set_parts.push(val);
        }
    }

    let clr = if clr_parts.is_empty() {
        "0".into()
    } else {
        clr_parts.join(" | ")
    };
    let set = if set_parts.is_empty() {
        "0".into()
    } else {
        set_parts.join(" | ")
    };
    (clr, set)
}

/// Emit a single field write value.
fn emit_field_write_value(
    value: &FieldWriteValue,
    register: &str,
    field: &str,
    ctx: &SemanticEmitContext<'_>,
) -> String {
    match value {
        FieldWriteValue::Named { name } => resolve_named_value_define(ctx, register, field, name),
        FieldWriteValue::Raw { value } => {
            if *value > 9 {
                format!("{value:#x}U")
            } else {
                format!("{value}")
            }
        }
        FieldWriteValue::Zero => "0".into(),
        FieldWriteValue::Local { local } => resolve_local_name(ctx.locals, *local),
        FieldWriteValue::Computed { expr } => emit_expr(expr, ctx),
    }
}
