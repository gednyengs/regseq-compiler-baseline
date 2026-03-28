//! Expression folding pass.
//!
//! Cleans up the structured C AST by:
//! 1. Removing unit assignments (`_N = ()`).
//! 2. Collapsing checked arithmetic patterns (temp + assert + field → direct op).
//! 3. Inlining single-use temporaries into their sole consumer.
//! 4. Removing dead stores (assignments whose result is never read).

use super::types::{CExpr, CStmt};

/// Run all folding passes on a structured AST until stable.
pub fn fold(stmts: &mut Vec<CStmt>) {
    for _ in 0..20 {
        let changed = fold_recursive(stmts);
        if !changed {
            break;
        }
    }
}

fn fold_recursive(stmts: &mut Vec<CStmt>) -> bool {
    let mut changed = false;

    // Recurse into nested blocks first.
    for stmt in stmts.iter_mut() {
        match stmt {
            CStmt::If {
                then_body,
                else_body,
                ..
            } => {
                changed |= fold_recursive(then_body);
                changed |= fold_recursive(else_body);
            }
            CStmt::While { body, .. } => {
                changed |= fold_recursive(body);
            }
            CStmt::Switch {
                cases, default, ..
            } => {
                for (_, body) in cases.iter_mut() {
                    changed |= fold_recursive(body);
                }
                changed |= fold_recursive(default);
            }
            _ => {}
        }
    }

    // Apply flat transforms at this level.
    changed |= remove_unit_assignments(stmts);
    changed |= collapse_checked_arithmetic(stmts);
    changed |= inline_single_use_temps(stmts);
    changed |= remove_dead_stores(stmts);

    changed
}

// ---------------------------------------------------------------------------
// Pass 1: Remove `_N = ()`
// ---------------------------------------------------------------------------

fn remove_unit_assignments(stmts: &mut Vec<CStmt>) -> bool {
    let before = stmts.len();
    stmts.retain(|s| !matches!(s, CStmt::Assign { rhs: CExpr::Unit, .. }));
    stmts.len() != before
}

// ---------------------------------------------------------------------------
// Pass 2: Collapse checked arithmetic
// ---------------------------------------------------------------------------
//
// MIR pattern for checked add:
//   _t = a + b          (actually AddWithOverflow, produces tuple)
//   if (!(_t.1)) { ... } (assert overflow flag is false)
//   _r = _t.0           (extract result)
//
// Collapsed form:
//   _r = a + b

fn collapse_checked_arithmetic(stmts: &mut Vec<CStmt>) -> bool {
    let mut changed = false;
    let mut i = 0;
    while i + 2 < stmts.len() {
        if let (true, new_assign) = try_collapse_checked(stmts, i) {
            stmts[i + 2] = new_assign;
            stmts.remove(i + 1); // remove assert
            stmts.remove(i); // remove temp assignment
            changed = true;
            continue; // re-examine at same index
        }
        i += 1;
    }
    changed
}

fn try_collapse_checked(stmts: &[CStmt], i: usize) -> (bool, CStmt) {
    let dummy = CStmt::Comment(String::new());

    // stmt[i]: _t = a OP b
    let (t, op, a, b) = match &stmts[i] {
        CStmt::Assign {
            lhs,
            rhs: CExpr::BinOp { op, lhs: a, rhs: b },
        } if is_arithmetic_op(op) => match lhs.as_local() {
            Some(t) => (t, op.clone(), a.clone(), b.clone()),
            None => return (false, dummy),
        },
        _ => return (false, dummy),
    };

    // stmt[i+1]: if (!(_t.1)) { ... }  (the assert)
    // We accept any If whose condition references _t (loose match).
    let is_assert = match &stmts[i + 1] {
        CStmt::If { cond, .. } => expr_references_local(cond, t),
        _ => false,
    };
    if !is_assert {
        return (false, dummy);
    }

    // stmt[i+2]: _r = _t.0
    match &stmts[i + 2] {
        CStmt::Assign { lhs, rhs } => {
            if let CExpr::Field { base, field } = rhs {
                if base.as_local() == Some(t) && field == "0" {
                    let new_rhs = CExpr::BinOp {
                        op,
                        lhs: Box::new(*a),
                        rhs: Box::new(*b),
                    };
                    return (
                        true,
                        CStmt::Assign {
                            lhs: lhs.clone(),
                            rhs: new_rhs,
                        },
                    );
                }
            }
            (false, dummy)
        }
        _ => (false, dummy),
    }
}

fn is_arithmetic_op(op: &str) -> bool {
    matches!(op, "+" | "-" | "*")
}

fn expr_references_local(expr: &CExpr, target: usize) -> bool {
    let mut locals = Vec::new();
    expr.locals_read(&mut locals);
    locals.contains(&target)
}

// ---------------------------------------------------------------------------
// Pass 3: Inline single-use temporaries
// ---------------------------------------------------------------------------

fn inline_single_use_temps(stmts: &mut Vec<CStmt>) -> bool {
    let mut changed = false;
    let mut i = 0;
    while i + 1 < stmts.len() {
        if let CStmt::Assign { lhs, rhs } = &stmts[i] {
            if let Some(target) = lhs.as_local() {
                // Don't inline _0 (return place).
                if target == 0 {
                    i += 1;
                    continue;
                }
                // Don't inline calls (side effects).
                if has_side_effects(rhs) {
                    i += 1;
                    continue;
                }
                let rhs_clone = rhs.clone();
                // Count uses in the rest of the block.
                let total_uses = count_local_uses(&stmts[i + 1..], target);
                if total_uses == 1 {
                    // Verify the single use is in the very next statement.
                    let next_uses = count_in_stmt(&stmts[i + 1], target);
                    if next_uses == 1 {
                        substitute_in_stmt(&mut stmts[i + 1], target, &rhs_clone);
                        stmts.remove(i);
                        changed = true;
                        continue;
                    }
                }
            }
        }
        i += 1;
    }
    changed
}

// ---------------------------------------------------------------------------
// Pass 4: Remove dead stores
// ---------------------------------------------------------------------------

fn remove_dead_stores(stmts: &mut Vec<CStmt>) -> bool {
    let mut changed = false;
    let mut i = 0;
    while i < stmts.len() {
        if let CStmt::Assign { lhs, rhs } = &stmts[i] {
            if let Some(target) = lhs.as_local() {
                // Keep _0 (return place).
                if target == 0 {
                    i += 1;
                    continue;
                }
                // Keep assignments with side effects.
                if has_side_effects(rhs) {
                    i += 1;
                    continue;
                }
                let uses_after = count_local_uses(&stmts[i + 1..], target);
                if uses_after == 0 {
                    stmts.remove(i);
                    changed = true;
                    continue;
                }
            }
        }
        i += 1;
    }
    changed
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn has_side_effects(expr: &CExpr) -> bool {
    match expr {
        CExpr::Call { .. } => true,
        CExpr::BinOp { lhs, rhs, .. } => has_side_effects(lhs) || has_side_effects(rhs),
        CExpr::UnaryOp { operand, .. } => has_side_effects(operand),
        CExpr::Field { base, .. } => has_side_effects(base),
        CExpr::AddrOf(inner) | CExpr::Deref(inner) => has_side_effects(inner),
        CExpr::Cast { operand, .. } => has_side_effects(operand),
        CExpr::Aggregate { fields, .. } => fields.iter().any(has_side_effects),
        _ => false,
    }
}

fn count_local_uses(stmts: &[CStmt], target: usize) -> usize {
    stmts.iter().map(|s| count_in_stmt(s, target)).sum()
}

fn count_in_stmt(stmt: &CStmt, target: usize) -> usize {
    match stmt {
        CStmt::Assign { lhs, rhs } => {
            // If lhs is Local(target), that's a write, not a read.
            let lhs_reads = if lhs.as_local() == Some(target) {
                0
            } else {
                count_in_expr(lhs, target)
            };
            lhs_reads + count_in_expr(rhs, target)
        }
        CStmt::If {
            cond,
            then_body,
            else_body,
        } => {
            count_in_expr(cond, target)
                + count_local_uses(then_body, target)
                + count_local_uses(else_body, target)
        }
        CStmt::While { cond, body } => {
            count_in_expr(cond, target) + count_local_uses(body, target)
        }
        CStmt::Switch {
            scrutinee,
            cases,
            default,
        } => {
            let mut c = count_in_expr(scrutinee, target);
            for (_, body) in cases {
                c += count_local_uses(body, target);
            }
            c += count_local_uses(default, target);
            c
        }
        CStmt::Return(Some(expr)) | CStmt::Expr(expr) => count_in_expr(expr, target),
        CStmt::Drop { place } => count_in_expr(place, target),
        CStmt::VarDecl { init: Some(e), .. } => count_in_expr(e, target),
        _ => 0,
    }
}

fn count_in_expr(expr: &CExpr, target: usize) -> usize {
    let mut locals = Vec::new();
    expr.locals_read(&mut locals);
    locals.iter().filter(|&&l| l == target).count()
}

fn substitute_in_stmt(stmt: &mut CStmt, target: usize, replacement: &CExpr) {
    match stmt {
        CStmt::Assign { lhs, rhs } => {
            // Don't substitute the lhs if it's a simple Local(target) write.
            if lhs.as_local() != Some(target) {
                lhs.substitute(target, replacement);
            }
            rhs.substitute(target, replacement);
        }
        CStmt::If {
            cond,
            then_body,
            else_body,
        } => {
            cond.substitute(target, replacement);
            for s in then_body {
                substitute_in_stmt(s, target, replacement);
            }
            for s in else_body {
                substitute_in_stmt(s, target, replacement);
            }
        }
        CStmt::While { cond, body } => {
            cond.substitute(target, replacement);
            for s in body {
                substitute_in_stmt(s, target, replacement);
            }
        }
        CStmt::Switch {
            scrutinee,
            cases,
            default,
        } => {
            scrutinee.substitute(target, replacement);
            for (_, body) in cases {
                for s in body {
                    substitute_in_stmt(s, target, replacement);
                }
            }
            for s in default {
                substitute_in_stmt(s, target, replacement);
            }
        }
        CStmt::Return(Some(expr)) | CStmt::Expr(expr) => {
            expr.substitute(target, replacement);
        }
        CStmt::Drop { place } => {
            place.substitute(target, replacement);
        }
        CStmt::VarDecl { init: Some(e), .. } => {
            e.substitute(target, replacement);
        }
        _ => {}
    }
}
