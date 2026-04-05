//! BehaviorNode-to-C emission engine.
//!
//! Translates portable `BehaviorNode` trees into Linux-specific C code
//! lines. All device-specific content comes from the plan data; this
//! module contains only Linux-specific API mappings that are device-neutral.

use crate::lowering::behavior::{
    AssignValue, BehaviorNode, BehaviorPlan, BinOp, BranchCondition, FieldWrite, FieldWriteValue,
    FunctionRole, ResourceOperation, ReturnSemantic, ValueExpr,
};
use crate::lowering::plan::ResourceKind;

use super::plan::LinuxRegisterLayout;

// ---------------------------------------------------------------------------
// Emission context
// ---------------------------------------------------------------------------

/// Context for translating BehaviorNode trees to C code.
pub(super) struct EmitContext<'a> {
    /// Snake_case driver name (e.g., `"my_device"`).
    pub snake: &'a str,
    /// SCREAMING_SNAKE driver name (e.g., `"MY_DEVICE"`).
    pub upper: String,
    /// Register define prefix (e.g., `"DEV"`).
    pub reg_prefix: &'a str,
    /// Portable behavior plan (for interrupt class lookup).
    #[allow(dead_code)]
    pub behavior: &'a BehaviorPlan,
    /// Linux register layout (for named value resolution).
    pub register_layout: &'a LinuxRegisterLayout,
    /// Current function role (affects return semantics).
    pub current_role: Option<FunctionRole>,
}

// ---------------------------------------------------------------------------
// Naming helpers — shared between emit_regs and emit_behavior
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
///
/// e.g., descriptor `"CTRL_REG"`, field `"MODE"`, value `"EIGHT_BITS"`,
/// prefix `"DEV"` → `"DEV_CTRL_MODE_EIGHT_BITS"`.
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
///
/// e.g., class `"rx"`, upper `"MY_DEVICE"` →
/// `"MY_DEVICE_RX_IRQ_MASK"`.
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
// BehaviorNode tree emission
// ---------------------------------------------------------------------------

/// Emit a sequence of behavior nodes as C code lines.
pub(super) fn emit_behavior_nodes(
    nodes: &[BehaviorNode],
    ctx: &EmitContext<'_>,
    indent: usize,
) -> Vec<String> {
    let mut lines = Vec::new();
    for node in nodes {
        lines.extend(emit_behavior_node(node, ctx, indent));
    }
    lines
}

/// Emit a single behavior node as C code lines.
fn emit_behavior_node(node: &BehaviorNode, ctx: &EmitContext<'_>, indent: usize) -> Vec<String> {
    let ind = "\t".repeat(indent);
    match node {
        BehaviorNode::RegWrite { register, fields } => {
            let reg_define = register_offset_define(ctx.reg_prefix, register);
            let value = emit_combined_write_value(fields, register, ctx);
            vec![format!(
                "{ind}{}_reg_write(d, {reg_define}, {value});",
                ctx.snake
            )]
        }

        BehaviorNode::RegModify { register, fields } => {
            let reg_define = register_offset_define(ctx.reg_prefix, register);
            let (clr, set) = emit_modify_masks(fields, register, ctx);
            vec![format!(
                "{ind}{}_reg_modify(d, {reg_define}, {clr}, {set});",
                ctx.snake
            )]
        }

        BehaviorNode::RegReadRaw { register } => {
            let reg_define = register_offset_define(ctx.reg_prefix, register);
            vec![format!("{ind}{}_reg_read(d, {reg_define})", ctx.snake)]
        }

        BehaviorNode::RegFieldRead { register, field } => {
            let reg_define = register_offset_define(ctx.reg_prefix, register);
            // Look up the bitfield mask from the register layout.
            let mask = find_bitfield_mask_define(ctx, register, field);
            vec![format!(
                "{ind}({}_reg_read(d, {reg_define}) & {mask})",
                ctx.snake
            )]
        }

        BehaviorNode::Branch {
            condition,
            then_body,
            else_body,
        } => {
            let mut lines = Vec::new();
            let cond_str = emit_condition(condition, ctx);
            lines.push(format!("{ind}if ({cond_str}) {{"));
            lines.extend(emit_behavior_nodes(then_body, ctx, indent + 1));
            if !else_body.is_empty() {
                lines.push(format!("{ind}}} else {{"));
                lines.extend(emit_behavior_nodes(else_body, ctx, indent + 1));
            }
            lines.push(format!("{ind}}}"));
            lines
        }

        BehaviorNode::Loop { condition, body } => {
            let mut lines = Vec::new();
            let cond_str = emit_condition(condition, ctx);
            lines.push(format!("{ind}while ({cond_str}) {{"));
            lines.extend(emit_behavior_nodes(body, ctx, indent + 1));
            lines.push(format!("{ind}}}"));
            lines
        }

        BehaviorNode::OrderedSequence { steps } => emit_behavior_nodes(steps, ctx, indent),

        BehaviorNode::Return { semantic } => {
            vec![format!("{ind}{}", emit_return(semantic, ctx))]
        }

        BehaviorNode::CallHelper { callee, args } => {
            let arg_strs: Vec<String> = args.iter().map(|a| emit_value_expr(a, ctx)).collect();
            let all_args = if arg_strs.is_empty() {
                "d".to_string()
            } else {
                format!("d, {}", arg_strs.join(", "))
            };
            vec![format!("{ind}{}_{callee}({all_args});", ctx.snake)]
        }

        BehaviorNode::FieldAssign { field, value } => {
            let val = emit_assign_value(value);
            vec![format!("{ind}d->{field} = {val};")]
        }

        BehaviorNode::ResourceOp {
            resource_kind,
            field_name,
            operation,
        } => emit_resource_op(&ind, ctx, resource_kind, field_name, operation),

        BehaviorNode::Delay { .. } => {
            vec![format!("{ind}/* TODO: timed delay */")]
        }

        BehaviorNode::InterruptClassTest {
            status_source,
            class_name,
        } => {
            let mask = interrupt_class_define(&ctx.upper, class_name);
            vec![format!("{ind}if ({status_source} & {mask})")]
        }

        BehaviorNode::InterruptClassClear {
            status_source,
            class_name,
        } => {
            let mask = interrupt_class_define(&ctx.upper, class_name);
            // Write to the ICR register to clear the matched interrupt sources.
            // The ICR register is identified by convention from the register layout.
            let icr_define = find_icr_register_define(ctx);
            vec![format!(
                "{ind}{}_reg_write(d, {icr_define}, {status_source} & {mask});",
                ctx.snake
            )]
        }

        BehaviorNode::RegisterFlagCheck {
            flag_name,
            register,
            field,
        } => {
            let mask = find_bitfield_mask_define(ctx, register, field);
            // Pattern: if (raw & mask) accumulate_flag;
            vec![format!("{ind}if (dr & {mask}) /* {flag_name} flag */",)]
        }

        BehaviorNode::LocalBinding {
            name,
            mutable: _,
            value,
        } => {
            let val_str = emit_value_expr(value, ctx);
            // Infer a reasonable C type from the expression.
            let c_type = infer_c_type(value);
            vec![format!("{ind}{c_type} {name} = {val_str};")]
        }

        BehaviorNode::FallibleOp {
            body,
            cleanup_on_error,
        } => {
            let mut lines = emit_behavior_nodes(body, ctx, indent);
            if !cleanup_on_error.is_empty() {
                lines.push(format!("{ind}if (ret) {{"));
                lines.extend(emit_behavior_nodes(cleanup_on_error, ctx, indent + 1));
                lines.push(format!("{ind}\treturn ret;"));
                lines.push(format!("{ind}}}"));
            }
            lines
        }

        BehaviorNode::CleanupRegion { body, cleanup } => {
            let mut lines = emit_behavior_nodes(body, ctx, indent);
            if !cleanup.is_empty() {
                lines.push(String::new());
                lines.extend(emit_behavior_nodes(cleanup, ctx, indent));
            }
            lines
        }

        BehaviorNode::SubsystemAction { subsystem, action } => {
            emit_subsystem_action(&ind, ctx, subsystem, action)
        }

        BehaviorNode::CompoundAssign { target, op, value } => {
            let val = emit_value_expr(value, ctx);
            vec![format!("{ind}{target} {op}= {val};")]
        }

        BehaviorNode::Eval { expr } => {
            let val = emit_value_expr(expr, ctx);
            vec![format!("{ind}{val};")]
        }
    }
}

// ---------------------------------------------------------------------------
// Condition emission
// ---------------------------------------------------------------------------

/// Translate a BranchCondition to a C expression string.
fn emit_condition(cond: &BranchCondition, ctx: &EmitContext<'_>) -> String {
    match cond {
        BranchCondition::RegFieldEq {
            register,
            field,
            value,
        } => {
            let mask = find_bitfield_mask_define(ctx, register, field);
            let reg_define = register_offset_define(ctx.reg_prefix, register);
            format!(
                "({}_reg_read(d, {reg_define}) & {mask}) == {value}",
                ctx.snake
            )
        }

        BranchCondition::RegFieldNonZero { register, field } => {
            let mask = find_bitfield_mask_define(ctx, register, field);
            let reg_define = register_offset_define(ctx.reg_prefix, register);
            format!("{}_reg_read(d, {reg_define}) & {mask}", ctx.snake)
        }

        BranchCondition::RawEqZero { source } => {
            format!("{source} == 0")
        }

        BranchCondition::FieldIsNone { field } => {
            format!("!d->{field}")
        }

        BranchCondition::And(lhs, rhs) => {
            format!(
                "({}) && ({})",
                emit_condition(lhs, ctx),
                emit_condition(rhs, ctx)
            )
        }

        BranchCondition::BoolParam { name } => name.clone(),

        BranchCondition::HelperPredicate { callee } => {
            format!("{}_{}(d)", ctx.snake, callee)
        }

        BranchCondition::InterruptClassActive {
            status_source,
            class_name,
        } => {
            let mask = interrupt_class_define(&ctx.upper, class_name);
            format!("{status_source} & {mask}")
        }

        BranchCondition::ValueTest { expr } => emit_value_expr(expr, ctx),
    }
}

// ---------------------------------------------------------------------------
// Value expression emission
// ---------------------------------------------------------------------------

/// Translate a ValueExpr to a C expression string.
pub(super) fn emit_value_expr(expr: &ValueExpr, ctx: &EmitContext<'_>) -> String {
    match expr {
        ValueExpr::IntLit(v) => {
            if *v > 9 {
                format!("{v:#x}U")
            } else {
                format!("{v}")
            }
        }
        ValueExpr::BoolLit(b) => {
            if *b {
                "1".into()
            } else {
                "0".into()
            }
        }
        ValueExpr::Param(name) => name.clone(),
        ValueExpr::Local(name) => name.clone(),
        ValueExpr::FieldRef(name) => format!("d->{name}"),
        ValueExpr::BinOp { op, lhs, rhs } => {
            let l = emit_value_expr(lhs, ctx);
            let r = emit_value_expr(rhs, ctx);
            let op_str = match op {
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
            };
            format!("({l} {op_str} {r})")
        }
        ValueExpr::Cast { expr, ty } => {
            let inner = emit_value_expr(expr, ctx);
            let c_type = rust_ty_to_c(ty);
            format!("({c_type})({inner})")
        }
        ValueExpr::MethodResult { callee, args } => {
            let arg_strs: Vec<String> = args.iter().map(|a| emit_value_expr(a, ctx)).collect();
            format!("{}({})", callee, arg_strs.join(", "))
        }
        ValueExpr::FieldExtract {
            register,
            field,
            raw_value,
        } => {
            let mask = find_bitfield_mask_define(ctx, register, field);
            let raw = emit_value_expr(raw_value, ctx);
            format!("({raw} & {mask})")
        }
        ValueExpr::None => "0".into(),
        ValueExpr::Some(inner) => emit_value_expr(inner, ctx),
    }
}

// ---------------------------------------------------------------------------
// Field write helpers
// ---------------------------------------------------------------------------

/// Emit a combined value for a register write from multiple field assignments.
fn emit_combined_write_value(
    fields: &[FieldWrite],
    register: &str,
    ctx: &EmitContext<'_>,
) -> String {
    if fields.is_empty() {
        return "0".into();
    }
    if fields.len() == 1 && fields[0].field == "__raw__" {
        return emit_fwv(&fields[0].value, register, &fields[0].field, ctx);
    }
    let parts: Vec<String> = fields
        .iter()
        .filter_map(|fw| {
            let val = emit_fwv(&fw.value, register, &fw.field, ctx);
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
    fields: &[FieldWrite],
    register: &str,
    ctx: &EmitContext<'_>,
) -> (String, String) {
    let mut clr_parts = Vec::new();
    let mut set_parts = Vec::new();

    for fw in fields {
        let mask = find_bitfield_mask_define(ctx, register, &fw.field);
        clr_parts.push(mask.clone());
        let val = emit_fwv(&fw.value, register, &fw.field, ctx);
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
fn emit_fwv(value: &FieldWriteValue, register: &str, field: &str, ctx: &EmitContext<'_>) -> String {
    match value {
        FieldWriteValue::Named(name) => {
            // Look up the named value define from the register layout.
            resolve_named_value_define(ctx, register, field, name)
        }
        FieldWriteValue::Variable(var) => var.clone(),
        FieldWriteValue::Raw(val) => {
            if *val > 9 {
                format!("{val:#x}U")
            } else {
                format!("{val}")
            }
        }
        FieldWriteValue::Zero => "0".into(),
    }
}

// ---------------------------------------------------------------------------
// Return emission
// ---------------------------------------------------------------------------

/// Emit a C return statement from a ReturnSemantic.
fn emit_return(semantic: &ReturnSemantic, ctx: &EmitContext<'_>) -> String {
    match semantic {
        ReturnSemantic::Ok => "return 0;".into(),
        ReturnSemantic::None => "return;".into(),
        ReturnSemantic::SomeValue => "return ret;".into(),
        ReturnSemantic::IrqHandled => "return IRQ_HANDLED;".into(),
        ReturnSemantic::IrqNotHandled => "return IRQ_NONE;".into(),
        ReturnSemantic::ConditionalResult => "return ret;".into(),
        ReturnSemantic::Bool(true) => {
            // In tx_empty context, true means TEMT.
            if ctx.current_role == Some(FunctionRole::SerialTxEmpty) {
                "return TIOCSER_TEMT;".into()
            } else {
                "return 1;".into()
            }
        }
        ReturnSemantic::Bool(false) => "return 0;".into(),
        ReturnSemantic::Void => "return;".into(),
    }
}

// ---------------------------------------------------------------------------
// Assign value emission
// ---------------------------------------------------------------------------

fn emit_assign_value(value: &AssignValue) -> String {
    match value {
        AssignValue::None => "NULL".into(),
        AssignValue::SomeByte { source } => source.clone(),
        AssignValue::Literal(lit) => lit.clone(),
    }
}

// ---------------------------------------------------------------------------
// Resource operation emission
// ---------------------------------------------------------------------------

fn emit_resource_op(
    ind: &str,
    _ctx: &EmitContext<'_>,
    resource_kind: &ResourceKind,
    field_name: &str,
    operation: &ResourceOperation,
) -> Vec<String> {
    match (resource_kind, operation) {
        (ResourceKind::Clock, ResourceOperation::Enable) => {
            vec![
                format!("{ind}ret = clk_prepare_enable(d->{field_name});"),
                format!("{ind}if (ret)"),
                format!("{ind}\treturn ret;"),
            ]
        }
        (ResourceKind::Clock, ResourceOperation::Disable) => {
            vec![format!("{ind}clk_disable_unprepare(d->{field_name});")]
        }
        (ResourceKind::Reset, ResourceOperation::Toggle) => {
            vec![format!("{ind}/* TODO: reset toggle for {field_name} */")]
        }
        (ResourceKind::PinCtrl, ResourceOperation::ApplyState { state }) => {
            vec![format!(
                "{ind}/* TODO: pinctrl select state \"{state}\" for {field_name} */"
            )]
        }
        _ => {
            vec![format!("{ind}/* TODO: resource op on {field_name} */",)]
        }
    }
}

// ---------------------------------------------------------------------------
// Subsystem action emission
// ---------------------------------------------------------------------------

/// Emit Linux serial-core API calls from portable subsystem actions.
fn emit_subsystem_action(
    ind: &str,
    _ctx: &EmitContext<'_>,
    subsystem: &str,
    action: &str,
) -> Vec<String> {
    if subsystem == "serial" {
        match action {
            "push_rx_char" => {
                vec![format!("{ind}uart_insert_char(&d->port, dr, 0, ch, flag);")]
            }
            "flush_rx_buffer" => {
                vec![format!("{ind}tty_flip_buffer_push(&d->port.state->port);")]
            }
            "signal_tx_ready" => {
                vec![format!("{ind}uart_write_wakeup(&d->port);")]
            }
            "increment_rx_count" => {
                vec![format!("{ind}d->port.icount.rx++;")]
            }
            "increment_tx_count" => {
                vec![format!("{ind}d->port.icount.tx++;")]
            }
            _ => {
                vec![format!(
                    "{ind}/* TODO: serial subsystem action \"{action}\" */"
                )]
            }
        }
    } else {
        vec![format!(
            "{ind}/* TODO: subsystem \"{subsystem}\" action \"{action}\" */"
        )]
    }
}

// ---------------------------------------------------------------------------
// Register layout helpers
// ---------------------------------------------------------------------------

/// Find the C define name for a bitfield mask, looking up the register
/// layout to match register/field names.
fn find_bitfield_mask_define(ctx: &EmitContext<'_>, register: &str, field: &str) -> String {
    // Try to find a matching descriptor for the register.
    for desc in &ctx.register_layout.descriptors {
        let desc_prefix = normalize_desc_prefix(&desc.register_name);
        // Match if the register name (e.g., "dr") matches the descriptor
        // prefix (e.g., "DR") case-insensitively.
        if desc_prefix.eq_ignore_ascii_case(register)
            || desc_prefix.eq_ignore_ascii_case(&register.replace('_', ""))
        {
            // Look for the specific bitfield.
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

/// Find the ICR (interrupt clear) register define from the layout.
fn find_icr_register_define(ctx: &EmitContext<'_>) -> String {
    // Look for a register field named "icr" in the layout.
    for block in &ctx.register_layout.blocks {
        for field in &block.fields {
            if field.name.eq_ignore_ascii_case("icr") {
                return register_offset_define(ctx.reg_prefix, &field.name);
            }
        }
    }
    // Fallback: conventional name.
    format!("{}_ICR", ctx.reg_prefix)
}

/// Resolve a named value define from the register layout.
fn resolve_named_value_define(
    ctx: &EmitContext<'_>,
    register: &str,
    field: &str,
    value_name: &str,
) -> String {
    // Search descriptors for the named value.
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
// Type inference helpers
// ---------------------------------------------------------------------------

/// Infer a C type from a ValueExpr for local bindings.
fn infer_c_type(value: &ValueExpr) -> &'static str {
    match value {
        ValueExpr::IntLit(v) => {
            if *v > u32::MAX as u64 {
                "u64"
            } else {
                "u32"
            }
        }
        ValueExpr::BoolLit(_) => "bool",
        ValueExpr::Cast { ty, .. } => {
            if ty.contains("u64") || ty.contains("i64") {
                "u64"
            } else if ty.contains("u8") || ty.contains("i8") {
                "u8"
            } else if ty.contains("u16") || ty.contains("i16") {
                "u16"
            } else {
                "u32"
            }
        }
        _ => "u32",
    }
}

/// Convert a Rust type name to a C type name.
fn rust_ty_to_c(ty: &str) -> &str {
    match ty {
        "u8" => "u8",
        "u16" => "u16",
        "u32" => "u32",
        "u64" => "u64",
        "i8" => "s8",
        "i16" => "s16",
        "i32" => "s32",
        "i64" => "s64",
        "bool" => "bool",
        _ => "u32",
    }
}
