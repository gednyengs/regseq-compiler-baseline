//! Body expression interpreter.
//!
//! Walks captured `BodyExpr` trees from analysis and produces structured
//! `BehaviorNode` lists. This is the core bridge between MIR-derived body
//! capture and the portable behavioral plan.
//!
//! The interpreter recognizes patterns for:
//! - register operations (write, modify, read, read_raw)
//! - optional resource operations (clock, reset, pinctrl)
//! - helper/method calls
//! - control flow (if/else, while, return)
//! - fallible operations (`?` / Try desugaring)
//! - ordered regions (barrier-bracketed sequences from `ordered!` expansion)
//! - local variable bindings and intermediate computations
//! - field assignments on the driver struct
//!
//! `regseq_analysis::legacy_input::AnalysisInput` is a transitional shim
//! retired by `regseq-lowering-10`; `#![allow(deprecated)]` is applied
//! here until then.
#![allow(deprecated)]

use std::collections::BTreeSet;

use regseq_analysis::legacy_input::AnalysisInput;
use regseq_model::declaration::bodies::{BodyExpr, BodyStmt, FnBodyDecl, FnKind};

use super::behavior::{
    AssignValue, BehaviorNode, BehaviorPlan, BinOp, BranchCondition, FieldWrite, FieldWriteValue,
    FunctionBehavior, FunctionRole, InterruptClassDef, ResourceOperation, ReturnSemantic,
    ValueExpr,
};
use super::plan::ResourceKind;

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Build a `BehaviorPlan` by interpreting captured function bodies.
///
/// Walks the `BodyExpr` trees from analysis for each callback, helper,
/// ISR, and free function. Produces per-function `FunctionBehavior`
/// entries and derives interrupt class definitions from ISR body patterns.
pub fn interpret_bodies(input: &AnalysisInput) -> BehaviorPlan {
    let ctx = InterpreterContext::from_analysis(input);

    let mut functions = Vec::new();
    let mut interrupt_classes = Vec::new();

    // Process callbacks.
    for func in input.callback_bodies() {
        functions.push(interpret_function(&ctx, func));
    }

    // Process ISR handlers.
    for func in input.isr_bodies() {
        let behavior = interpret_function(&ctx, func);
        // Derive interrupt classes from ISR body patterns.
        let classes = derive_interrupt_classes(&behavior.body);
        interrupt_classes.extend(classes);
        functions.push(behavior);
    }

    // Process reachable helpers.
    let reachable = input.reachable_helper_names();
    for func in input.helper_bodies() {
        if reachable.contains(&func.name) {
            functions.push(interpret_function(&ctx, func));
        }
    }

    // Process reachable free functions.
    for func in input.free_function_bodies() {
        if reachable.contains(&func.name) {
            functions.push(interpret_function(&ctx, func));
        }
    }

    // Add the "all" class if we have any interrupt classes.
    if !interrupt_classes.is_empty() {
        let all_mask = interrupt_classes
            .iter()
            .fold(0u32, |acc, c| acc | c.mask_value);
        interrupt_classes.push(InterruptClassDef {
            name: "all".into(),
            mask_value: all_mask,
        });
    }

    BehaviorPlan {
        functions,
        interrupt_classes,
    }
}

// ---------------------------------------------------------------------------
// Interpreter context
// ---------------------------------------------------------------------------

/// Context for body interpretation, derived from analysis metadata.
struct InterpreterContext {
    /// MMIO field names on the driver struct (e.g. "regs").
    mmio_fields: BTreeSet<String>,
    /// Known register field names within MMIO blocks.
    register_fields: BTreeSet<String>,
    /// Optional resource fields with their resource kind.
    resource_fields: Vec<(String, ResourceKind)>,
    /// Names of known helper functions.
    helper_names: BTreeSet<String>,
    /// Names of driver struct fields that are not resources.
    _driver_fields: BTreeSet<String>,
}

impl InterpreterContext {
    fn from_analysis(input: &AnalysisInput) -> Self {
        let mut mmio_fields = BTreeSet::new();
        let mut register_fields = BTreeSet::new();
        let mut resource_fields = Vec::new();
        let mut driver_fields = BTreeSet::new();

        // MMIO fields.
        for mmio in &input.declaration().resources.mmio {
            mmio_fields.insert(mmio.field_name.clone());
        }

        // Register fields within blocks.
        for block in input.register_blocks() {
            for field in &block.fields {
                register_fields.insert(field.name.clone());
            }
        }

        // Optional resource fields.
        for clock in &input.declaration().resources.clock.fields {
            resource_fields.push((clock.field_name.clone(), ResourceKind::Clock));
        }
        for reset in &input.declaration().resources.reset.fields {
            resource_fields.push((reset.field_name.clone(), ResourceKind::Reset));
        }
        for pinctrl in &input.declaration().resources.pinctrl.fields {
            resource_fields.push((pinctrl.field_name.clone(), ResourceKind::PinCtrl));
        }

        // Helper function names.
        let helper_names = input.reachable_helper_names();

        // Driver fields (properties and state fields).
        for prop in &input.declaration().properties.fields {
            driver_fields.insert(prop.field_name.clone());
        }

        InterpreterContext {
            mmio_fields,
            register_fields,
            resource_fields,
            helper_names,
            _driver_fields: driver_fields,
        }
    }

    /// Check if a field name refers to an MMIO resource.
    fn is_mmio_field(&self, name: &str) -> bool {
        self.mmio_fields.contains(name)
    }

    /// Check if a field name refers to a register within an MMIO block.
    fn is_register_field(&self, name: &str) -> bool {
        self.register_fields.contains(name)
    }

    /// Return the resource kind for a field name, if it's a resource.
    fn resource_kind(&self, field_name: &str) -> Option<ResourceKind> {
        self.resource_fields
            .iter()
            .find(|(name, _)| name == field_name)
            .map(|(_, kind)| *kind)
    }

    /// Check if a name is a known helper function.
    fn is_helper(&self, name: &str) -> bool {
        self.helper_names.contains(name)
    }
}

// ---------------------------------------------------------------------------
// Per-function interpretation
// ---------------------------------------------------------------------------

/// Interpret a single function body into a `FunctionBehavior`.
fn interpret_function(ctx: &InterpreterContext, func: &FnBodyDecl) -> FunctionBehavior {
    let role = determine_role(func);
    let body = interpret_body_expr(ctx, &func.body);

    FunctionBehavior {
        name: func.name.clone(),
        role,
        params: vec![],
        return_type: None,
        body,
    }
}

/// Determine the behavioral role from function kind and name.
fn determine_role(func: &FnBodyDecl) -> FunctionRole {
    match &func.kind {
        FnKind::Callback {
            trait_name,
            method_name,
        } => match (trait_name.as_str(), method_name.as_str()) {
            ("Lifecycle", "init") => FunctionRole::LifecycleInit,
            ("Lifecycle", "deinit") => FunctionRole::LifecycleDeinit,
            ("Serial", "port_open") => FunctionRole::SerialPortOpen,
            ("Serial", "port_close") => FunctionRole::SerialPortClose,
            ("Serial", "configure") => FunctionRole::SerialConfigure,
            ("Serial", "tx_byte") => FunctionRole::SerialTxByte,
            ("Serial", "rx_char") => FunctionRole::SerialRxChar,
            ("Serial", "tx_ready") => FunctionRole::SerialTxReady,
            ("Serial", "tx_empty") => FunctionRole::SerialTxEmpty,
            ("Serial", "flush_rx_fifo") => FunctionRole::SerialFlushRxFifo,
            ("Serial", "flush_tx_fifo") => FunctionRole::SerialFlushTxFifo,
            ("Serial", "send_break") => FunctionRole::SerialSendBreak,
            _ => FunctionRole::Helper,
        },
        FnKind::Isr => FunctionRole::Isr,
        FnKind::Helper | FnKind::FreeFunction => FunctionRole::Helper,
    }
}

// ---------------------------------------------------------------------------
// Expression interpretation
// ---------------------------------------------------------------------------

/// Interpret a `BodyExpr` into a list of `BehaviorNode`s.
fn interpret_body_expr(ctx: &InterpreterContext, expr: &BodyExpr) -> Vec<BehaviorNode> {
    match expr {
        BodyExpr::Block { stmts, expr } => {
            // Check for barrier-bracketed ordered sequence (expanded ordered! macro).
            if let Some(ordered) = try_interpret_ordered_block(ctx, stmts, expr) {
                return ordered;
            }

            let mut nodes = Vec::new();
            for stmt in stmts {
                nodes.extend(interpret_stmt(ctx, stmt));
            }
            if let Some(trailing) = expr {
                nodes.extend(interpret_trailing_expr(ctx, trailing));
            }
            nodes
        }
        other => interpret_single_expr(ctx, other),
    }
}

/// Detect and interpret an `ordered!` macro expansion block.
///
/// The `ordered!` macro expands to:
/// ```text
/// {
///     memory_barrier(BarrierKind::Full);
///     let __regseq_ordered_result = { body };
///     memory_barrier(BarrierKind::Full);
///     __regseq_ordered_result
/// }
/// ```
///
/// Returns an `OrderedSequence` node wrapping the inner body steps.
fn try_interpret_ordered_block(
    ctx: &InterpreterContext,
    stmts: &[BodyStmt],
    _trailing: &Option<Box<BodyExpr>>,
) -> Option<Vec<BehaviorNode>> {
    if stmts.len() != 3 {
        return None;
    }
    // stmts[0]: Semi { Call("memory_barrier", ...) }
    if !is_barrier_call_stmt(&stmts[0]) {
        return None;
    }
    // stmts[2]: Semi { Call("memory_barrier", ...) }
    if !is_barrier_call_stmt(&stmts[2]) {
        return None;
    }
    // stmts[1]: Let { name: "__regseq_ordered_result", init: Some(Block { ... }) }
    if let BodyStmt::Let {
        name,
        init: Some(init_expr),
        ..
    } = &stmts[1]
    {
        if name == "__regseq_ordered_result" {
            let inner_steps = interpret_body_expr(ctx, init_expr);
            return Some(vec![BehaviorNode::OrderedSequence { steps: inner_steps }]);
        }
    }
    None
}

/// Check if a statement is a barrier intrinsic call from `ordered!` expansion.
fn is_barrier_call_stmt(stmt: &BodyStmt) -> bool {
    let expr = match stmt {
        BodyStmt::Semi { expr } | BodyStmt::Expr { expr } => expr,
        _ => return false,
    };
    match expr {
        BodyExpr::Call { path, .. } => path.contains("memory_barrier"),
        BodyExpr::MethodCall { method, .. } => method.contains("memory_barrier"),
        _ => false,
    }
}

/// Interpret a statement into behavior nodes.
fn interpret_stmt(ctx: &InterpreterContext, stmt: &BodyStmt) -> Vec<BehaviorNode> {
    match stmt {
        BodyStmt::Semi { expr } | BodyStmt::Expr { expr } => interpret_single_expr(ctx, expr),
        BodyStmt::Let {
            name,
            mutable,
            init,
            ..
        } => {
            if let Some(init_expr) = init {
                vec![BehaviorNode::LocalBinding {
                    name: name.clone(),
                    mutable: *mutable,
                    value: interpret_value_expr(ctx, init_expr),
                }]
            } else {
                vec![BehaviorNode::LocalBinding {
                    name: name.clone(),
                    mutable: *mutable,
                    value: ValueExpr::None,
                }]
            }
        }
    }
}

/// Interpret a single expression as behavior nodes.
fn interpret_single_expr(ctx: &InterpreterContext, expr: &BodyExpr) -> Vec<BehaviorNode> {
    // Try to match known patterns first.
    if let Some(node) = try_match_register_op(ctx, expr) {
        return vec![node];
    }
    if let Some(node) = try_match_resource_op(ctx, expr) {
        return vec![node];
    }
    if let Some(node) = try_match_helper_call(ctx, expr) {
        return vec![node];
    }
    if let Some(node) = try_match_field_assign(ctx, expr) {
        return vec![node];
    }

    match expr {
        BodyExpr::Block { .. } => interpret_body_expr(ctx, expr),

        BodyExpr::If {
            cond,
            then_branch,
            else_branch,
        } => {
            let condition = interpret_condition(ctx, cond);
            let then_body = interpret_body_expr(ctx, then_branch);
            let else_body = else_branch
                .as_ref()
                .map(|e| interpret_body_expr(ctx, e))
                .unwrap_or_default();
            vec![BehaviorNode::Branch {
                condition,
                then_body,
                else_body,
            }]
        }

        BodyExpr::Loop { body } => {
            // Try to recognize while-loop pattern: loop { if !cond { break } body }
            if let Some((condition, loop_body)) = try_extract_while_loop(ctx, body) {
                vec![BehaviorNode::Loop {
                    condition,
                    body: loop_body,
                }]
            } else {
                // Plain loop with break detection.
                let body_nodes = interpret_body_expr(ctx, body);
                vec![BehaviorNode::Loop {
                    condition: BranchCondition::ValueTest {
                        expr: ValueExpr::BoolLit(true),
                    },
                    body: body_nodes,
                }]
            }
        }

        BodyExpr::Return { expr } => {
            let semantic = expr
                .as_ref()
                .map(|e| interpret_return_semantic(ctx, e))
                .unwrap_or(ReturnSemantic::Void);
            vec![BehaviorNode::Return { semantic }]
        }

        BodyExpr::Try { expr } => {
            let inner = interpret_single_expr(ctx, expr);
            vec![BehaviorNode::FallibleOp {
                body: inner,
                cleanup_on_error: vec![],
            }]
        }

        BodyExpr::Assign { target, value } => {
            if let Some(node) = try_match_field_assign_inner(ctx, target, value) {
                vec![node]
            } else {
                vec![BehaviorNode::Eval {
                    expr: interpret_value_expr(ctx, expr),
                }]
            }
        }

        BodyExpr::AssignOp { op, target, value } => {
            if let Some(field_name) = extract_self_field(target) {
                vec![BehaviorNode::CompoundAssign {
                    target: field_name,
                    op: op.clone(),
                    value: interpret_value_expr(ctx, value),
                }]
            } else {
                let target_name = match target.as_ref() {
                    BodyExpr::Path { path } => path.clone(),
                    _ => "unknown".into(),
                };
                vec![BehaviorNode::CompoundAssign {
                    target: target_name,
                    op: op.clone(),
                    value: interpret_value_expr(ctx, value),
                }]
            }
        }

        BodyExpr::MethodCall {
            receiver, method, ..
        } => {
            // Generic method call that didn't match specific patterns.
            let callee = method.clone();
            if is_self_receiver(receiver) && ctx.is_helper(&callee) {
                vec![BehaviorNode::CallHelper {
                    callee,
                    args: vec![],
                }]
            } else {
                vec![BehaviorNode::Eval {
                    expr: interpret_value_expr(ctx, expr),
                }]
            }
        }

        BodyExpr::Call { path, args, .. } => {
            if is_ok_unit_call(path, args) {
                vec![BehaviorNode::Return {
                    semantic: ReturnSemantic::Ok,
                }]
            } else {
                vec![BehaviorNode::Eval {
                    expr: interpret_value_expr(ctx, expr),
                }]
            }
        }

        _ => {
            // For other expressions, produce an Eval node.
            vec![BehaviorNode::Eval {
                expr: interpret_value_expr(ctx, expr),
            }]
        }
    }
}

/// Interpret a trailing block expression (typically a return value).
fn interpret_trailing_expr(ctx: &InterpreterContext, expr: &BodyExpr) -> Vec<BehaviorNode> {
    match expr {
        BodyExpr::Call { path, args, .. } if is_ok_unit_call(path, args) => {
            vec![BehaviorNode::Return {
                semantic: ReturnSemantic::Ok,
            }]
        }
        BodyExpr::Path { path } if path == "IrqResult::Handled" => {
            vec![BehaviorNode::Return {
                semantic: ReturnSemantic::IrqHandled,
            }]
        }
        BodyExpr::Path { path } if path == "IrqResult::NotHandled" => {
            vec![BehaviorNode::Return {
                semantic: ReturnSemantic::IrqNotHandled,
            }]
        }
        BodyExpr::Path { path } if path == "None" => {
            vec![BehaviorNode::Return {
                semantic: ReturnSemantic::None,
            }]
        }
        _ => interpret_single_expr(ctx, expr),
    }
}

// ---------------------------------------------------------------------------
// Pattern matchers
// ---------------------------------------------------------------------------

/// Try to match a register operation: `self.regs.<reg>.<method>(args)`.
fn try_match_register_op(ctx: &InterpreterContext, expr: &BodyExpr) -> Option<BehaviorNode> {
    if let BodyExpr::MethodCall {
        receiver,
        method,
        args,
        ..
    } = expr
    {
        // Pattern: self.regs.<register>.<method>(args)
        if let BodyExpr::Field {
            base: mmio_access,
            name: reg_name,
        } = receiver.as_ref()
        {
            if is_mmio_receiver(ctx, mmio_access) && ctx.is_register_field(reg_name) {
                return match method.as_str() {
                    "write" => {
                        let fields = interpret_write_args(ctx, args);
                        Some(BehaviorNode::RegWrite {
                            register: reg_name.clone(),
                            fields,
                        })
                    }
                    "modify" => {
                        let fields = interpret_write_args(ctx, args);
                        Some(BehaviorNode::RegModify {
                            register: reg_name.clone(),
                            fields,
                        })
                    }
                    "read" => {
                        let field = extract_field_arg(args);
                        Some(BehaviorNode::RegFieldRead {
                            register: reg_name.clone(),
                            field,
                        })
                    }
                    "read_raw" => Some(BehaviorNode::RegReadRaw {
                        register: reg_name.clone(),
                    }),
                    _ => None,
                };
            }
        }
    }
    None
}

/// Try to match an optional resource operation: `self.<resource>.<method>(...)`.
fn try_match_resource_op(ctx: &InterpreterContext, expr: &BodyExpr) -> Option<BehaviorNode> {
    // Pattern: if let Some(res) = &self.<resource> { res.<method>(...) }
    // This shows up as method calls on optional resources.
    // Also pattern: self.<helper_that_wraps_resource>()
    // For now, recognize direct method calls on known resource helper patterns.

    if let BodyExpr::MethodCall {
        receiver, method, ..
    } = expr
    {
        // Check for patterns like clock.enable(), reset.toggle(), pinctrl.apply()
        if let Some(field_name) = extract_self_field(receiver) {
            if let Some(kind) = ctx.resource_kind(&field_name) {
                let operation = match method.as_str() {
                    "enable" => Some(ResourceOperation::Enable),
                    "disable" => Some(ResourceOperation::Disable),
                    "toggle" => Some(ResourceOperation::Toggle),
                    "apply" => Some(ResourceOperation::ApplyState {
                        state: "default".into(),
                    }),
                    _ => None,
                };
                if let Some(op) = operation {
                    return Some(BehaviorNode::ResourceOp {
                        resource_kind: kind,
                        field_name,
                        operation: op,
                    });
                }
            }
        }
    }
    None
}

/// Try to match a helper call: `self.<method>()`.
fn try_match_helper_call(ctx: &InterpreterContext, expr: &BodyExpr) -> Option<BehaviorNode> {
    if let BodyExpr::MethodCall {
        receiver, method, ..
    } = expr
    {
        if is_self_receiver(receiver) && ctx.is_helper(method) {
            return Some(BehaviorNode::CallHelper {
                callee: method.clone(),
                args: vec![],
            });
        }
    }
    None
}

/// Try to match a field assignment: `self.<field> = value`.
fn try_match_field_assign(ctx: &InterpreterContext, expr: &BodyExpr) -> Option<BehaviorNode> {
    if let BodyExpr::Assign { target, value } = expr {
        try_match_field_assign_inner(ctx, target, value)
    } else {
        None
    }
}

fn try_match_field_assign_inner(
    _ctx: &InterpreterContext,
    target: &BodyExpr,
    value: &BodyExpr,
) -> Option<BehaviorNode> {
    if let Some(field_name) = extract_self_field(target) {
        let assign_value = match value {
            BodyExpr::Path { path } if path == "None" => AssignValue::None,
            BodyExpr::Call { path, args, .. } if path == "Some" && args.len() == 1 => {
                if let BodyExpr::Path { path: inner } = &args[0] {
                    AssignValue::SomeByte {
                        source: inner.clone(),
                    }
                } else {
                    AssignValue::Literal(format!("{value:?}"))
                }
            }
            BodyExpr::LitInt { value: v, .. } => AssignValue::Literal(v.clone()),
            BodyExpr::Path { path } => AssignValue::Literal(path.clone()),
            _ => AssignValue::Literal(format!("{value:?}")),
        };
        Some(BehaviorNode::FieldAssign {
            field: field_name,
            value: assign_value,
        })
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Condition interpretation
// ---------------------------------------------------------------------------

/// Interpret a condition expression for Branch/Loop nodes.
fn interpret_condition(ctx: &InterpreterContext, expr: &BodyExpr) -> BranchCondition {
    match expr {
        // self.regs.fr.read(FIELD) != 0  →  RegFieldNonZero
        BodyExpr::Binary { op, lhs, rhs } if op == "!=" || op == "==" => {
            if let (Some(reg_read), Some(0)) = (try_extract_reg_read(ctx, lhs), extract_int(rhs)) {
                if op == "!=" {
                    return BranchCondition::RegFieldNonZero {
                        register: reg_read.0,
                        field: reg_read.1,
                    };
                } else {
                    return BranchCondition::RegFieldEq {
                        register: reg_read.0,
                        field: reg_read.1,
                        value: 0,
                    };
                }
            }
            // status == 0
            if op == "==" {
                if let (BodyExpr::Path { path }, Some(0)) = (lhs.as_ref(), extract_int(rhs)) {
                    return BranchCondition::RawEqZero {
                        source: path.clone(),
                    };
                }
            }
            // status & MASK != 0 → InterruptClassActive
            // The class name is derived from the authored constant name
            // (e.g. Self::RX_INTERRUPT_MASK → "rx") or from the literal
            // mask value if no named constant is referenced.
            if op == "!=" {
                if let (
                    BodyExpr::Binary {
                        op: inner_op,
                        lhs: status,
                        rhs: mask,
                    },
                    Some(0),
                ) = (lhs.as_ref(), extract_int(rhs))
                {
                    if inner_op == "&" {
                        if let Some(src) = extract_path(status) {
                            let class_name = derive_class_name_from_constant(mask);
                            return BranchCondition::InterruptClassActive {
                                status_source: src,
                                class_name,
                            };
                        }
                    }
                }
            }
            BranchCondition::ValueTest {
                expr: interpret_value_expr(ctx, expr),
            }
        }

        // self.pending_tx.is_none()
        BodyExpr::MethodCall {
            receiver, method, ..
        } if method == "is_none" => {
            if let Some(field) = extract_self_field(receiver) {
                BranchCondition::FieldIsNone { field }
            } else {
                BranchCondition::ValueTest {
                    expr: interpret_value_expr(ctx, expr),
                }
            }
        }

        // self.tx_ready() used as predicate
        BodyExpr::MethodCall {
            receiver, method, ..
        } if is_self_receiver(receiver) && ctx.is_helper(method) => {
            BranchCondition::HelperPredicate {
                callee: method.clone(),
            }
        }

        // Bool parameter
        BodyExpr::Path { path } if !path.contains("::") && !path.contains('.') => {
            BranchCondition::BoolParam { name: path.clone() }
        }

        // Conjunction
        BodyExpr::Binary { op, lhs, rhs } if op == "&&" => {
            let left = interpret_condition(ctx, lhs);
            let right = interpret_condition(ctx, rhs);
            BranchCondition::And(Box::new(left), Box::new(right))
        }

        _ => BranchCondition::ValueTest {
            expr: interpret_value_expr(ctx, expr),
        },
    }
}

// ---------------------------------------------------------------------------
// Value expression interpretation
// ---------------------------------------------------------------------------

/// Interpret a `BodyExpr` as a `ValueExpr`.
fn interpret_value_expr(ctx: &InterpreterContext, expr: &BodyExpr) -> ValueExpr {
    match expr {
        BodyExpr::LitInt { value, .. } => ValueExpr::IntLit(value.parse::<u64>().unwrap_or(0)),
        BodyExpr::LitBool { value } => ValueExpr::BoolLit(*value),
        BodyExpr::Path { path } if path == "self" => ValueExpr::FieldRef("self".into()),
        BodyExpr::Path { path } if path == "None" => ValueExpr::None,
        BodyExpr::Path { path } => {
            // Try to determine if this is a param, local, or field ref.
            if path.starts_with("self.") || path.starts_with("Self::") {
                ValueExpr::FieldRef(path.clone())
            } else {
                ValueExpr::Param(path.clone())
            }
        }
        BodyExpr::Field { base, name } => {
            if is_self_receiver(base) {
                ValueExpr::FieldRef(name.clone())
            } else {
                ValueExpr::FieldRef(format!("{:?}.{name}", base))
            }
        }
        BodyExpr::Binary { op, lhs, rhs } => {
            let bin_op = match op.as_str() {
                "+" => BinOp::Add,
                "-" => BinOp::Sub,
                "*" => BinOp::Mul,
                "/" => BinOp::Div,
                "<<" => BinOp::Shl,
                ">>" => BinOp::Shr,
                "&" => BinOp::BitAnd,
                "|" => BinOp::BitOr,
                "==" => BinOp::Eq,
                "!=" => BinOp::Ne,
                "&&" => BinOp::And,
                "||" => BinOp::Or,
                _ => BinOp::Add, // fallback
            };
            ValueExpr::BinOp {
                op: bin_op,
                lhs: Box::new(interpret_value_expr(ctx, lhs)),
                rhs: Box::new(interpret_value_expr(ctx, rhs)),
            }
        }
        BodyExpr::Cast { expr, ty } => ValueExpr::Cast {
            expr: Box::new(interpret_value_expr(ctx, expr)),
            ty: ty.clone(),
        },
        BodyExpr::MethodCall {
            receiver,
            method,
            args,
            ..
        } => {
            // Check for register field read as value.
            if let BodyExpr::Field {
                base: mmio_access,
                name: reg_name,
            } = receiver.as_ref()
            {
                if is_mmio_receiver(ctx, mmio_access) && ctx.is_register_field(reg_name) {
                    match method.as_str() {
                        "read" => {
                            let field = extract_field_arg(args);
                            return ValueExpr::FieldExtract {
                                register: reg_name.clone(),
                                field,
                                raw_value: Box::new(ValueExpr::FieldRef(reg_name.clone())),
                            };
                        }
                        "read_raw" => {
                            return ValueExpr::MethodResult {
                                callee: format!("{reg_name}.read_raw"),
                                args: vec![],
                            };
                        }
                        _ => {}
                    }
                }
            }
            let callee_args = args.iter().map(|a| interpret_value_expr(ctx, a)).collect();
            ValueExpr::MethodResult {
                callee: method.clone(),
                args: callee_args,
            }
        }
        BodyExpr::Call { path, args, .. } => {
            if path == "Some" && args.len() == 1 {
                ValueExpr::Some(Box::new(interpret_value_expr(ctx, &args[0])))
            } else {
                let callee_args = args.iter().map(|a| interpret_value_expr(ctx, a)).collect();
                ValueExpr::MethodResult {
                    callee: path.clone(),
                    args: callee_args,
                }
            }
        }
        _ => ValueExpr::IntLit(0), // fallback for unrecognized patterns
    }
}

// ---------------------------------------------------------------------------
// Return semantic interpretation
// ---------------------------------------------------------------------------

/// Determine the return semantic from a return value expression.
fn interpret_return_semantic(_ctx: &InterpreterContext, expr: &BodyExpr) -> ReturnSemantic {
    match expr {
        BodyExpr::Call { path, args, .. } => {
            if is_ok_unit_call(path, args) {
                ReturnSemantic::Ok
            } else if path == "Some" {
                ReturnSemantic::SomeValue
            } else {
                ReturnSemantic::Void
            }
        }
        BodyExpr::Path { path } => match path.as_str() {
            "None" => ReturnSemantic::None,
            "IrqResult::Handled" => ReturnSemantic::IrqHandled,
            "IrqResult::NotHandled" => ReturnSemantic::IrqNotHandled,
            "true" => ReturnSemantic::Bool(true),
            "false" => ReturnSemantic::Bool(false),
            _ => ReturnSemantic::Void,
        },
        BodyExpr::LitBool { value } => ReturnSemantic::Bool(*value),
        BodyExpr::If { .. } => ReturnSemantic::ConditionalResult,
        _ => ReturnSemantic::Void,
    }
}

// ---------------------------------------------------------------------------
// Register argument interpretation
// ---------------------------------------------------------------------------

/// Interpret register write/modify arguments into field writes.
fn interpret_write_args(_ctx: &InterpreterContext, args: &[BodyExpr]) -> Vec<FieldWrite> {
    let mut fields = Vec::new();

    for arg in args {
        match arg {
            // Named value: REG::FIELD::VALUE
            BodyExpr::Path { path } => {
                if let Some((field, value)) = parse_field_value_path(path) {
                    fields.push(FieldWrite {
                        field,
                        value: FieldWriteValue::Named(value),
                    });
                }
            }
            // Sum of named values: REG::A::V1 + REG::B::V2
            BodyExpr::Binary { op, lhs, rhs } if op == "+" => {
                fields.extend(collect_sum_fields(lhs));
                fields.extend(collect_sum_fields(rhs));
            }
            // Variable
            BodyExpr::Call {
                path,
                args: call_args,
                ..
            } => {
                // field_val(FIELD, value) or raw_reg_val(mask, value)
                if path == "field_val" && call_args.len() == 2 {
                    if let Some(field_name) = extract_field_descriptor_name(&call_args[0]) {
                        let value = match &call_args[1] {
                            BodyExpr::Path { path } => FieldWriteValue::Variable(path.clone()),
                            BodyExpr::LitInt { value, .. } => {
                                if let Ok(v) = value.parse::<u32>() {
                                    FieldWriteValue::Raw(v)
                                } else {
                                    FieldWriteValue::Variable(value.clone())
                                }
                            }
                            BodyExpr::Cast { expr, .. } => match expr.as_ref() {
                                BodyExpr::Path { path } => FieldWriteValue::Variable(path.clone()),
                                _ => FieldWriteValue::Variable("expr".into()),
                            },
                            _ => FieldWriteValue::Variable("expr".into()),
                        };
                        fields.push(FieldWrite {
                            field: field_name,
                            value,
                        });
                    }
                } else if path == "raw_reg_val" && call_args.len() == 2 {
                    // Raw mask/value pair.
                    let mask_val = extract_int(&call_args[0]).unwrap_or(0) as u32;
                    let write_val = extract_int(&call_args[1]).unwrap_or(0) as u32;
                    fields.push(FieldWrite {
                        field: format!("raw_{mask_val:#x}"),
                        value: if write_val == 0 {
                            FieldWriteValue::Zero
                        } else {
                            FieldWriteValue::Raw(write_val)
                        },
                    });
                }
            }
            _ => {}
        }
    }

    fields
}

/// Collect field writes from a sum expression (A + B + C).
fn collect_sum_fields(expr: &BodyExpr) -> Vec<FieldWrite> {
    match expr {
        BodyExpr::Path { path } => {
            if let Some((field, value)) = parse_field_value_path(path) {
                vec![FieldWrite {
                    field,
                    value: FieldWriteValue::Named(value),
                }]
            } else {
                vec![]
            }
        }
        BodyExpr::Binary { op, lhs, rhs } if op == "+" => {
            let mut fields = collect_sum_fields(lhs);
            fields.extend(collect_sum_fields(rhs));
            fields
        }
        _ => vec![],
    }
}

/// Parse a path like `REG::FIELD::VALUE` into (field, value).
fn parse_field_value_path(path: &str) -> Option<(String, String)> {
    let parts: Vec<&str> = path.split("::").collect();
    if parts.len() >= 3 {
        let field = parts[parts.len() - 2].to_string();
        let value = parts[parts.len() - 1].to_string();
        Some((field, value))
    } else {
        None
    }
}

/// Extract a field descriptor name from an expression like `REG::FIELD::FIELD`.
fn extract_field_descriptor_name(expr: &BodyExpr) -> Option<String> {
    if let BodyExpr::Path { path } = expr {
        let parts: Vec<&str> = path.split("::").collect();
        if parts.len() >= 2 {
            return Some(parts[parts.len() - 2].to_string());
        }
    }
    None
}

/// Extract the field name from a register read argument.
fn extract_field_arg(args: &[BodyExpr]) -> String {
    for arg in args {
        if let BodyExpr::Path { path } = arg {
            let parts: Vec<&str> = path.split("::").collect();
            if parts.len() >= 2 {
                return parts[parts.len() - 2].to_string();
            }
        }
    }
    "unknown".into()
}

// ---------------------------------------------------------------------------
// While-loop extraction
// ---------------------------------------------------------------------------

/// Try to extract a while-loop pattern from a loop body.
///
/// Recognizes: `loop { if !condition { break; } body }` as a while loop.
fn try_extract_while_loop(
    ctx: &InterpreterContext,
    body: &BodyExpr,
) -> Option<(BranchCondition, Vec<BehaviorNode>)> {
    if let BodyExpr::Block { stmts, .. } = body {
        // Look for: first stmt is `if condition { break }` → while(!condition)
        if stmts.is_empty() {
            return None;
        }
        if let BodyStmt::Semi { expr } | BodyStmt::Expr { expr } = &stmts[0] {
            if let BodyExpr::If {
                cond,
                then_branch,
                else_branch,
            } = expr
            {
                // Check if then_branch is a break.
                if is_break_expr(then_branch) && else_branch.is_none() {
                    let condition = interpret_condition(ctx, cond);
                    // The remaining stmts form the loop body.
                    let mut loop_body = Vec::new();
                    for stmt in &stmts[1..] {
                        loop_body.extend(interpret_stmt(ctx, stmt));
                    }
                    // Invert the condition (break-on-true → continue-while-not).
                    return Some((condition, loop_body));
                }
            }
        }
    }
    None
}

fn is_break_expr(expr: &BodyExpr) -> bool {
    matches!(expr, BodyExpr::Break { .. })
        || matches!(expr, BodyExpr::Block { stmts, .. }
            if stmts.len() == 1 && matches!(&stmts[0],
                BodyStmt::Semi { expr } | BodyStmt::Expr { expr }
                if matches!(expr, BodyExpr::Break { .. })))
}

// ---------------------------------------------------------------------------
// Interrupt class derivation
// ---------------------------------------------------------------------------

/// Derive interrupt class definitions from ISR body patterns.
///
/// Recognizes patterns like:
/// ```text
/// if status & MASK != 0 { ... }
/// ```
/// Class names are derived from the authored constant names referenced
/// in branch conditions (device-agnostic structural convention), not
/// from device-specific heuristics.
fn derive_interrupt_classes(body: &[BehaviorNode]) -> Vec<InterruptClassDef> {
    let mut classes = Vec::new();
    let mut seen_names = BTreeSet::new();

    for node in body {
        if let BehaviorNode::Branch {
            condition: BranchCondition::InterruptClassActive { class_name, .. },
            ..
        } = node
        {
            if seen_names.insert(class_name.clone()) {
                // Resolve mask value from the class_name metadata.
                // Class names from the condition interpreter carry
                // the mask value when derived from literal constants.
                let mask_value = extract_mask_from_class_name(class_name);
                classes.push(InterruptClassDef {
                    name: class_name.clone(),
                    mask_value,
                });
            }
        }
    }

    classes
}

/// Extract a mask value from a class name that may encode it.
///
/// Class names from literal mask values carry the value in a suffix
/// (e.g., "rx_0x50" → 0x50). Names from authored constants don't
/// embed the value; returns 0 for those.
fn extract_mask_from_class_name(name: &str) -> u32 {
    if let Some(hex_part) = name.rsplit('_').next() {
        if let Some(digits) = hex_part.strip_prefix("0x") {
            if let Ok(v) = u32::from_str_radix(digits, 16) {
                return v;
            }
        }
    }
    0
}

/// Derive a device-agnostic interrupt class name from an authored
/// constant reference or literal mask value.
///
/// For named constants like `Self::RX_INTERRUPT_MASK`, strips the
/// `Self::` prefix and common suffixes (`_INTERRUPT_MASK`, `_MASK`,
/// `_IRQ_MASK`) then lowercases the result.
///
/// For literal integer masks, produces a neutral name encoding the
/// mask value so the class is identifiable without device knowledge.
fn derive_class_name_from_constant(mask_expr: &BodyExpr) -> String {
    match mask_expr {
        BodyExpr::Path { path } => {
            let name = path.strip_prefix("Self::").unwrap_or(path);
            // Strip common interrupt mask suffixes.
            let stripped = name
                .strip_suffix("_INTERRUPT_MASK")
                .or_else(|| name.strip_suffix("_IRQ_MASK"))
                .or_else(|| name.strip_suffix("_MASK"))
                .unwrap_or(name);
            stripped.to_lowercase()
        }
        BodyExpr::LitInt { value, .. } => {
            // Literal mask value: produce a neutral name with the value
            // so the class is identifiable.
            if let Ok(v) = value.parse::<u32>() {
                format!("class_{v:#x}")
            } else {
                format!("class_{value}")
            }
        }
        _ => "unknown_class".into(),
    }
}

// ---------------------------------------------------------------------------
// Utility helpers
// ---------------------------------------------------------------------------

/// Try to extract a register field read from a method call expression.
///
/// Matches: `self.regs.<reg>.read(FIELD)` and returns (register, field).
fn try_extract_reg_read(ctx: &InterpreterContext, expr: &BodyExpr) -> Option<(String, String)> {
    if let BodyExpr::MethodCall {
        receiver,
        method,
        args,
        ..
    } = expr
    {
        if method == "read" {
            if let BodyExpr::Field {
                base: mmio_access,
                name: reg_name,
            } = receiver.as_ref()
            {
                if is_mmio_receiver(ctx, mmio_access) && ctx.is_register_field(reg_name) {
                    let field = extract_field_arg(args);
                    return Some((reg_name.clone(), field));
                }
            }
        }
    }
    None
}

fn is_self_receiver(expr: &BodyExpr) -> bool {
    matches!(expr, BodyExpr::Path { path } if path == "self")
}

fn is_mmio_receiver(ctx: &InterpreterContext, expr: &BodyExpr) -> bool {
    if let BodyExpr::Field { base, name } = expr {
        if is_self_receiver(base) && ctx.is_mmio_field(name) {
            return true;
        }
    }
    false
}

fn extract_self_field(expr: &BodyExpr) -> Option<String> {
    if let BodyExpr::Field { base, name } = expr {
        if is_self_receiver(base) {
            return Some(name.clone());
        }
    }
    None
}

fn extract_int(expr: &BodyExpr) -> Option<u64> {
    if let BodyExpr::LitInt { value, .. } = expr {
        return value.parse().ok();
    }
    None
}

fn extract_path(expr: &BodyExpr) -> Option<String> {
    if let BodyExpr::Path { path } = expr {
        return Some(path.clone());
    }
    None
}

fn is_ok_unit_call(path: &str, args: &[BodyExpr]) -> bool {
    path == "Ok"
        && args.len() == 1
        && matches!(&args[0], BodyExpr::Tuple { elements } if elements.is_empty())
}

/// Post-process a behavior plan to resolve interrupt class mask values.
///
/// When body data uses authored constant references (`Self::RX_INTERRUPT_MASK`),
/// class names are derived at interpretation time but mask values may be
/// zero (unknown from the Path alone). This pass populates mask values
/// from a caller-provided constant-value map.
///
/// When body data uses literal mask values, class definitions already have
/// correct mask values and this pass is a no-op.
pub fn resolve_interrupt_class_masks(plan: &mut BehaviorPlan, constant_values: &[(String, u32)]) {
    for class in &mut plan.interrupt_classes {
        if class.mask_value == 0 && class.name != "all" {
            // Try to resolve from the constant map.
            for (const_name, value) in constant_values {
                let expected = derive_class_name_from_constant(&BodyExpr::Path {
                    path: const_name.clone(),
                });
                if expected == class.name {
                    class.mask_value = *value;
                    break;
                }
            }
        }
    }

    // Recompute the "all" class mask as the OR of all non-"all" classes.
    let combined = plan
        .interrupt_classes
        .iter()
        .filter(|c| c.name != "all")
        .fold(0u32, |acc, c| acc | c.mask_value);
    if let Some(all_class) = plan.interrupt_classes.iter_mut().find(|c| c.name == "all") {
        all_class.mask_value = combined;
    }
}
