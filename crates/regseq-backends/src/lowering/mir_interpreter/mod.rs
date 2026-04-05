//! MIR-based body interpreter.
//!
//! Walks captured MIR CFG bodies from `AnalysisInput.mir_bodies()` and
//! produces structured `BehaviorPlan` output. This is the canonical
//! executable-body interpretation path. The legacy HIR-based
//! `body_interpreter` module is not used by this path.
//!
//! The interpreter has two conceptual phases:
//! 1. MIR interpretation — walk captured MIR, identify register/resource/
//!    control-flow semantics, preserve cleanup-bearing paths
//! 2. Portable semantic lowering — transform recognized semantics into
//!    the portable behavior plan for backend consumption

pub mod call_classifier;
pub mod cfg_helpers;
pub mod cfg_walker;
pub mod context;
pub mod place_resolver;
pub mod value_lowering;

use std::collections::BTreeSet;

use regseq_analysis::AnalysisInput;
use regseq_model::declaration::mir_body::{FnKind, MirFnBodyDecl};

use super::behavior::{
    BehaviorNode, BehaviorPlan, BranchCondition, FunctionBehavior, FunctionParam, FunctionRole,
    InterruptClassDef, PortableType,
};

use context::{FnContext, MirInterpreterContext};
use value_lowering::{
    derive_class_name_from_constant_path, extract_constant_path, extract_constant_value,
};

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Build a `BehaviorPlan` by interpreting MIR function bodies.
///
/// Uses `AnalysisInput.mir_bodies()` exclusively. Does not use the
/// legacy HIR body path.
pub fn interpret_mir_bodies(input: &AnalysisInput) -> BehaviorPlan {
    let ctx = MirInterpreterContext::from_analysis(input);

    let mut functions = Vec::new();
    let mut interrupt_classes = Vec::new();

    // Process callbacks.
    for func in input.mir_callback_bodies() {
        functions.push(interpret_mir_function(&ctx, func));
    }

    // Process ISR handlers.
    for func in input.mir_isr_bodies() {
        let behavior = interpret_mir_function(&ctx, func);
        let classes = derive_interrupt_classes_from_mir(&behavior.body, &func.body);
        interrupt_classes.extend(classes);
        functions.push(behavior);
    }

    // Process reachable helpers.
    let reachable = input.mir_reachable_helper_names();
    for func in input.mir_helper_bodies() {
        if reachable.contains(&func.name) {
            functions.push(interpret_mir_function(&ctx, func));
        }
    }

    // Process reachable free functions.
    for func in input.mir_free_function_bodies() {
        if reachable.contains(&func.name) {
            functions.push(interpret_mir_function(&ctx, func));
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

/// Post-process a behavior plan to resolve interrupt class mask values.
///
/// When MIR bodies carry literal mask values as scalar constants, class
/// definitions already have correct mask values and this is a no-op.
/// This pass exists for backward compatibility with test fixtures that
/// use symbolic constant names requiring external resolution.
pub fn resolve_interrupt_class_masks(plan: &mut BehaviorPlan, constant_values: &[(String, u32)]) {
    for class in &mut plan.interrupt_classes {
        if class.mask_value == 0 && class.name != "all" {
            for (const_name, value) in constant_values {
                let expected = const_name
                    .strip_prefix("Self::")
                    .unwrap_or(const_name)
                    .strip_suffix("_INTERRUPT_MASK")
                    .or_else(|| {
                        const_name
                            .strip_prefix("Self::")
                            .unwrap_or(const_name)
                            .strip_suffix("_MASK")
                    })
                    .unwrap_or(const_name)
                    .to_lowercase();
                if class.name.contains(&expected) || expected.contains(&class.name) {
                    class.mask_value = *value;
                    break;
                }
            }
        }
    }

    // Recompute the "all" class mask.
    let combined = plan
        .interrupt_classes
        .iter()
        .filter(|c| c.name != "all")
        .fold(0u32, |acc, c| acc | c.mask_value);
    if let Some(all_class) = plan.interrupt_classes.iter_mut().find(|c| c.name == "all") {
        all_class.mask_value = combined;
    }
}

// ---------------------------------------------------------------------------
// Per-function interpretation
// ---------------------------------------------------------------------------

/// Interpret a single MIR function body into a `FunctionBehavior`.
fn interpret_mir_function(
    shared_ctx: &MirInterpreterContext,
    func: &MirFnBodyDecl,
) -> FunctionBehavior {
    let fn_ctx = FnContext::new(
        shared_ctx,
        &func.body.local_decls,
        func.body.arg_count,
        &func.receiver,
        &func.params,
        &func.body,
    );

    let role = determine_role(func);
    let body = cfg_walker::walk_region(&fn_ctx, &func.body, 0, &BTreeSet::new());

    let params = func
        .params
        .iter()
        .map(|p| FunctionParam {
            name: p.name.clone(),
            ty: parse_portable_type(&p.ty),
        })
        .collect();
    let return_type = func.return_type.as_ref().map(|t| parse_portable_type(t));

    FunctionBehavior {
        name: func.name.clone(),
        role,
        params,
        return_type,
        body,
    }
}

/// Parse a Rust type string into a `PortableType`.
fn parse_portable_type(ty: &str) -> PortableType {
    match ty {
        "u8" => PortableType::U8,
        "u16" => PortableType::U16,
        "u32" => PortableType::U32,
        "u64" => PortableType::U64,
        "i8" => PortableType::I8,
        "i16" => PortableType::I16,
        "i32" => PortableType::I32,
        "i64" => PortableType::I64,
        "bool" => PortableType::Bool,
        other => PortableType::Opaque(other.to_string()),
    }
}

/// Determine the behavioral role from function kind and name.
fn determine_role(func: &MirFnBodyDecl) -> FunctionRole {
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
// Interrupt class derivation from MIR
// ---------------------------------------------------------------------------

/// Derive interrupt class definitions from ISR behavior and MIR body.
///
/// Scans for `InterruptClassActive` branch conditions and extracts mask
/// values directly from MIR scalar constants where available.
fn derive_interrupt_classes_from_mir(
    behavior_body: &[BehaviorNode],
    mir_body: &regseq_model::declaration::mir_body::MirBody,
) -> Vec<InterruptClassDef> {
    let mut classes = Vec::new();
    let mut seen_names = BTreeSet::new();

    // Collect all InterruptClassActive conditions from the behavior tree.
    collect_interrupt_classes(behavior_body, &mut classes, &mut seen_names);

    // Try to resolve mask values from MIR constants.
    resolve_classes_from_mir_constants(&mut classes, mir_body);

    classes
}

/// Recursively collect interrupt class definitions from behavior nodes.
fn collect_interrupt_classes(
    nodes: &[BehaviorNode],
    classes: &mut Vec<InterruptClassDef>,
    seen: &mut BTreeSet<String>,
) {
    for node in nodes {
        match node {
            BehaviorNode::Branch {
                condition: BranchCondition::InterruptClassActive { class_name, .. },
                then_body,
                else_body,
            } => {
                if seen.insert(class_name.clone()) {
                    classes.push(InterruptClassDef {
                        name: class_name.clone(),
                        mask_value: extract_mask_from_class_name(class_name),
                    });
                }
                collect_interrupt_classes(then_body, classes, seen);
                collect_interrupt_classes(else_body, classes, seen);
            }
            BehaviorNode::Branch {
                then_body,
                else_body,
                ..
            } => {
                collect_interrupt_classes(then_body, classes, seen);
                collect_interrupt_classes(else_body, classes, seen);
            }
            BehaviorNode::Loop { body, .. } => {
                collect_interrupt_classes(body, classes, seen);
            }
            BehaviorNode::OrderedSequence { steps } => {
                collect_interrupt_classes(steps, classes, seen);
            }
            BehaviorNode::FallibleOp {
                body,
                cleanup_on_error,
            } => {
                collect_interrupt_classes(body, classes, seen);
                collect_interrupt_classes(cleanup_on_error, classes, seen);
            }
            BehaviorNode::CleanupRegion { body, cleanup } => {
                collect_interrupt_classes(body, classes, seen);
                collect_interrupt_classes(cleanup, classes, seen);
            }
            _ => {}
        }
    }
}

/// Extract mask value from a class name that encodes it (e.g., "class_0x0050").
fn extract_mask_from_class_name(name: &str) -> u32 {
    if let Some(hex_part) = name.strip_prefix("class_0x") {
        if let Ok(v) = u32::from_str_radix(hex_part, 16) {
            return v;
        }
    }
    // Try the format from the value lowering: "class_{mask:#06x}".
    if let Some(rest) = name.strip_prefix("class_") {
        if let Some(hex) = rest.strip_prefix("0x") {
            if let Ok(v) = u32::from_str_radix(hex, 16) {
                return v;
            }
        }
    }
    0
}

/// Try to resolve class mask values from MIR constant operands.
///
/// Uses symbolic-name-based matching: extracts constant paths from
/// MIR BitAnd operands, derives class names, and matches them against
/// unresolved class definitions. Scalar constants are already handled
/// by the class-name encoding (e.g., "class_0x0050"). This function
/// handles the symbolic/unevaluated case.
fn resolve_classes_from_mir_constants(
    classes: &mut [InterruptClassDef],
    mir_body: &regseq_model::declaration::mir_body::MirBody,
) {
    use regseq_model::declaration::mir_body::{MirBinOp, MirRvalue, MirStatementKind};

    // Collect (class_name, mask_value) pairs from MIR BitAnd operations.
    // For scalar constants, class names already encode the value and
    // extract_mask_from_class_name handles them. For symbolic paths,
    // we try to find a companion scalar value in the same basic block.
    let mut name_to_mask: Vec<(String, Option<u32>)> = Vec::new();

    for bb in &mir_body.basic_blocks {
        for stmt in &bb.statements {
            if let MirStatementKind::Assign { rvalue, .. } = &stmt.kind {
                if let MirRvalue::BinaryOp {
                    op: MirBinOp::BitAnd,
                    rhs,
                    ..
                } = rvalue.as_ref()
                {
                    // Skip scalar constants — already resolved via class name encoding.
                    if extract_constant_value(rhs).is_some() {
                        continue;
                    }
                    // Symbolic/unevaluated constant: extract path and derive class name.
                    if let Some(path) = extract_constant_path(rhs) {
                        let class_name = derive_class_name_from_constant_path(&path);
                        // The scalar value is not available in the current MIR surface
                        // for unevaluated constants.
                        name_to_mask.push((class_name, None));
                    }
                }
            }
        }
    }

    // For unresolved classes, check if we found a name match with a value.
    for class in classes.iter_mut() {
        if class.mask_value == 0 && class.name != "all" {
            for (name, value) in &name_to_mask {
                if class.name == *name {
                    if let Some(v) = value {
                        class.mask_value = *v;
                    }
                    break;
                }
            }
        }
    }
}
