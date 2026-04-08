//! SemanticModel projection from populated item hierarchy.
//!
//! Phase 2C: projects the backend-facing `SemanticModel` from a populated
//! `ItemHierarchy`. The projection is deterministic and explicit:
//!
//! - **Functions**: exactly the functions in the lowered set (Phase 2B output).
//! - **Constants**: only constants referenced by `Expr::Constant` in the
//!   projected function bodies.
//! - **Interrupt classes**: copied from the hierarchy verbatim.

use std::collections::BTreeSet;

use regseq_model::item::constant::ConstantDef;
use regseq_model::item::function::FunctionDef;
use regseq_model::item::identity::{CanonicalPath, ItemId};
use regseq_model::item::types::Item;
use regseq_model::semantic::body::{Block, SemanticBody};
use regseq_model::semantic::expr::Expr;
use regseq_model::semantic::model::SemanticModel;
use regseq_model::semantic::ops::FieldWriteValue;
use regseq_model::semantic::stmt::Stmt;

use super::hierarchy::ItemHierarchy;

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Project a `SemanticModel` from a populated `ItemHierarchy`.
///
/// **Function selection**: includes exactly the functions whose `ItemId` is
/// in `lowered_functions` (the set returned by Phase 2B lowering).
///
/// **Constant selection**: includes only constants whose canonical paths are
/// referenced by `Expr::Constant` in the projected function bodies.
///
/// **Interrupt classes**: copied from the hierarchy.
///
/// Deterministic: same inputs produce the same model.
pub fn project_semantic_model(
    hierarchy: &ItemHierarchy,
    lowered_functions: &BTreeSet<ItemId>,
) -> SemanticModel {
    // Step 1: Collect projected functions from the lowered set.
    let functions: Vec<FunctionDef> = (0..hierarchy.store.len())
        .filter_map(|i| {
            let id = ItemId(i as u32);
            if !lowered_functions.contains(&id) {
                return None;
            }
            match hierarchy.store.get(id) {
                Some(Item::Function(f)) => Some(f.clone()),
                _ => None,
            }
        })
        .collect();

    // Step 2: Collect referenced constant paths from projected function bodies.
    let referenced_constants = collect_referenced_constants(&functions);

    // Step 3: Collect constants that are actually referenced.
    let constants: Vec<ConstantDef> = (0..hierarchy.store.len())
        .filter_map(|i| {
            let id = ItemId(i as u32);
            match hierarchy.store.get(id) {
                Some(Item::Constant(c)) if referenced_constants.contains(&c.canonical_path) => {
                    Some(c.clone())
                }
                _ => None,
            }
        })
        .collect();

    // Step 4: Copy interrupt classes verbatim.
    let interrupt_classes = hierarchy.interrupt_classes.clone();

    SemanticModel {
        functions,
        constants,
        interrupt_classes,
    }
}

// ---------------------------------------------------------------------------
// Constant reference collector
// ---------------------------------------------------------------------------

/// Collect all `ConstantRef.canonical_path` values referenced in the given
/// function bodies via `Expr::Constant`.
fn collect_referenced_constants(functions: &[FunctionDef]) -> BTreeSet<CanonicalPath> {
    let mut refs = BTreeSet::new();
    for func in functions {
        collect_from_body(&func.body, &mut refs);
    }
    refs
}

fn collect_from_body(body: &SemanticBody, refs: &mut BTreeSet<CanonicalPath>) {
    collect_from_block(&body.body, refs);
}

fn collect_from_block(block: &Block, refs: &mut BTreeSet<CanonicalPath>) {
    for stmt in &block.stmts {
        collect_from_stmt(stmt, refs);
    }
}

fn collect_from_stmt(stmt: &Stmt, refs: &mut BTreeSet<CanonicalPath>) {
    match stmt {
        // Binding and assignment
        Stmt::Let { value, .. } => collect_from_expr(value, refs),
        Stmt::Assign { value, .. } => collect_from_expr(value, refs),
        Stmt::CompoundAssign { value, .. } => collect_from_expr(value, refs),
        Stmt::Eval { expr } => collect_from_expr(expr, refs),

        // Control flow
        Stmt::Branch {
            condition,
            then_body,
            else_body,
        } => {
            collect_from_expr(condition, refs);
            collect_from_block(then_body, refs);
            collect_from_block(else_body, refs);
        }
        Stmt::Loop { condition, body } => {
            collect_from_expr(condition, refs);
            collect_from_block(body, refs);
        }
        Stmt::OrderedRegion { body } => {
            collect_from_block(body, refs);
        }
        Stmt::FallibleOp {
            body,
            cleanup_on_error,
            ..
        } => {
            collect_from_block(body, refs);
            collect_from_block(cleanup_on_error, refs);
        }
        Stmt::Return { value } => {
            if let Some(expr) = value {
                collect_from_expr(expr, refs);
            }
        }

        // Device-domain side effects
        Stmt::RegWrite { fields, .. } | Stmt::RegModify { fields, .. } => {
            for fw in fields {
                collect_from_field_write_value(&fw.value, refs);
            }
        }
        Stmt::ResourceOp { .. } => {}
        Stmt::FieldAssign { value, .. } => collect_from_expr(value, refs),
        Stmt::SubsystemAction { args, .. } => {
            for arg in args {
                collect_from_expr(arg, refs);
            }
        }
    }
}

fn collect_from_expr(expr: &Expr, refs: &mut BTreeSet<CanonicalPath>) {
    match expr {
        // The target: collect constant references.
        Expr::Constant { constant } => {
            refs.insert(constant.canonical_path.clone());
        }

        // Recursive children
        Expr::BinaryOp { lhs, rhs, .. } => {
            collect_from_expr(lhs, refs);
            collect_from_expr(rhs, refs);
        }
        Expr::UnaryOp { operand, .. } => collect_from_expr(operand, refs),
        Expr::Cast { expr, .. } => collect_from_expr(expr, refs),
        Expr::FieldExtract { raw_value, .. } => collect_from_expr(raw_value, refs),
        Expr::HelperCall { args, .. } => {
            for arg in args {
                collect_from_expr(arg, refs);
            }
        }
        Expr::WithDefault { value, default } => {
            collect_from_expr(value, refs);
            collect_from_expr(default, refs);
        }
        Expr::InterruptClassTest { status, .. } => collect_from_expr(status, refs),
        Expr::IsSome { inner } => collect_from_expr(inner, refs),
        Expr::OptionSome { inner } => collect_from_expr(inner, refs),
        Expr::OptionUnwrap { inner } => collect_from_expr(inner, refs),

        // Leaf nodes -- no recursion.
        Expr::IntLit { .. }
        | Expr::BoolLit { .. }
        | Expr::LocalRef { .. }
        | Expr::DriverFieldRead { .. }
        | Expr::RegFieldRead { .. }
        | Expr::RegReadRaw { .. }
        | Expr::ResourcePropertyRead { .. }
        | Expr::ResourcePresent { .. }
        | Expr::OptionNone => {}
    }
}

fn collect_from_field_write_value(value: &FieldWriteValue, refs: &mut BTreeSet<CanonicalPath>) {
    match value {
        FieldWriteValue::Computed { expr } => collect_from_expr(expr, refs),
        FieldWriteValue::Named { .. }
        | FieldWriteValue::Raw { .. }
        | FieldWriteValue::Zero
        | FieldWriteValue::Local { .. } => {}
    }
}
