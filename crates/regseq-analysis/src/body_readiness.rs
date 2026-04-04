//! Body capture readiness and supported-subset validation.
//!
//! Validates that the analysis input contains adequate authored body
//! capture for later lowering, and surfaces any unsupported constructs
//! or coverage gaps explicitly.
//!
//! Design rules:
//! - device-neutral and OS-neutral: no PL011- or Linux-specific logic
//! - no manual body construction: all body data comes from the model
//! - unsupported constructs are surfaced, never silently skipped
//! - deterministic output

use std::collections::BTreeSet;

use crate::input::AnalysisInput;
use crate::readiness::FindingSeverity;

use regseq_model::declaration::bodies::{BodyExpr, BodyStmt, FnKind};
use regseq_model::declaration::callbacks::CallbackFamily;

/// A single body readiness finding.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct BodyFinding {
    /// Severity of this finding.
    pub severity: FindingSeverity,
    /// Category label for grouping.
    pub category: &'static str,
    /// Human-readable finding message.
    pub message: String,
}

/// Result of body capture readiness validation.
///
/// Contains deterministic findings about body coverage, supported-subset
/// compliance, and provenance completeness.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BodyReadinessReport {
    /// All findings, sorted by severity then category then message.
    pub findings: Vec<BodyFinding>,
    /// Number of callback bodies present.
    pub callback_body_count: usize,
    /// Number of helper bodies present.
    pub helper_body_count: usize,
    /// Number of ISR bodies present.
    pub isr_body_count: usize,
    /// Number of helpers transitively reachable from callbacks and ISRs.
    pub reachable_helper_count: usize,
    /// Total unsupported HIR nodes found across all bodies.
    pub unsupported_node_count: usize,
}

impl BodyReadinessReport {
    /// Whether any error-severity findings are present.
    pub fn has_errors(&self) -> bool {
        self.findings
            .iter()
            .any(|f| f.severity == FindingSeverity::Error)
    }

    /// Whether any caution-severity findings are present.
    pub fn has_cautions(&self) -> bool {
        self.findings
            .iter()
            .any(|f| f.severity == FindingSeverity::Caution)
    }

    /// Return only error findings.
    pub fn errors(&self) -> Vec<&BodyFinding> {
        self.findings
            .iter()
            .filter(|f| f.severity == FindingSeverity::Error)
            .collect()
    }

    /// Return only caution findings.
    pub fn cautions(&self) -> Vec<&BodyFinding> {
        self.findings
            .iter()
            .filter(|f| f.severity == FindingSeverity::Caution)
            .collect()
    }
}

/// Validate authored body capture and supported-subset for the analysis input.
///
/// Returns a deterministic report of findings about body coverage,
/// unsupported constructs, and provenance completeness.
pub fn check_body_readiness(input: &AnalysisInput) -> BodyReadinessReport {
    let mut findings = Vec::new();

    let callback_bodies = input.callback_bodies();
    let helper_bodies = input.helper_bodies();
    let isr_bodies = input.isr_bodies();
    let reachable = input.reachable_helper_names();

    check_callback_body_coverage(input, &mut findings);
    check_reachable_helper_coverage(input, &reachable, &mut findings);
    let unsupported_count = check_unsupported_nodes(input, &mut findings);
    check_body_provenance(input, &mut findings);

    findings.sort();

    BodyReadinessReport {
        findings,
        callback_body_count: callback_bodies.len(),
        helper_body_count: helper_bodies.len(),
        isr_body_count: isr_bodies.len(),
        reachable_helper_count: reachable.len(),
        unsupported_node_count: unsupported_count,
    }
}

/// Map a callback family to its expected trait name in body declarations.
fn family_trait_name(family: &CallbackFamily) -> &'static str {
    match family {
        CallbackFamily::Lifecycle => "Lifecycle",
        CallbackFamily::Serial => "Serial",
        CallbackFamily::SerialConsole => "SerialConsole",
        CallbackFamily::PowerManagement => "PowerManagement",
    }
}

/// Check that each declared callback family has at least one captured body.
fn check_callback_body_coverage(input: &AnalysisInput, findings: &mut Vec<BodyFinding>) {
    let families = input.callback_families_present();
    let callback_bodies = input.callback_bodies();

    for family in &families {
        let expected_trait = family_trait_name(family);
        let has_body = callback_bodies.iter().any(|b| {
            matches!(&b.kind, FnKind::Callback { trait_name, .. } if trait_name == expected_trait)
        });

        if !has_body {
            findings.push(BodyFinding {
                severity: FindingSeverity::Error,
                category: "body_coverage",
                message: format!(
                    "callback family '{}' is declared but no captured bodies are present",
                    family.as_str(),
                ),
            });
        }
    }

    // Report callback body count as info.
    if !callback_bodies.is_empty() {
        findings.push(BodyFinding {
            severity: FindingSeverity::Info,
            category: "body_coverage",
            message: format!(
                "{} callback body/bodies captured across {} family/families",
                callback_bodies.len(),
                families.len(),
            ),
        });
    }
}

/// Check that every helper transitively reachable from callbacks/ISRs has a body.
fn check_reachable_helper_coverage(
    input: &AnalysisInput,
    reachable: &BTreeSet<String>,
    findings: &mut Vec<BodyFinding>,
) {
    for name in reachable {
        if input.body_by_name(name).is_none() {
            findings.push(BodyFinding {
                severity: FindingSeverity::Error,
                category: "helper_coverage",
                message: format!("reachable helper '{}' has no captured body", name),
            });
        }
    }
}

/// Walk all body trees and surface any `Unsupported` expression nodes.
fn check_unsupported_nodes(input: &AnalysisInput, findings: &mut Vec<BodyFinding>) -> usize {
    let mut total = 0;

    for func in &input.bodies().functions {
        let unsupported = collect_unsupported_in_expr(&func.body);
        if !unsupported.is_empty() {
            for (hir_kind, span_text) in &unsupported {
                findings.push(BodyFinding {
                    severity: FindingSeverity::Caution,
                    category: "unsupported_construct",
                    message: format!(
                        "function '{}': unsupported HIR node '{}'{}",
                        func.name,
                        hir_kind,
                        span_text
                            .as_ref()
                            .map(|s| format!(" ({})", s))
                            .unwrap_or_default(),
                    ),
                });
            }
            total += unsupported.len();
        }
    }

    total
}

/// Check that all body declarations have non-empty provenance.
fn check_body_provenance(input: &AnalysisInput, findings: &mut Vec<BodyFinding>) {
    for func in &input.bodies().functions {
        if func.provenance.raw_family.is_empty() {
            findings.push(BodyFinding {
                severity: FindingSeverity::Caution,
                category: "body_provenance",
                message: format!("function '{}' has empty provenance raw_family", func.name,),
            });
        }
    }
}

// ----------------------------------------------------------------
// Unsupported-node collection (expression tree walker)
// ----------------------------------------------------------------

/// Collect all `Unsupported` expression nodes in a body expression tree.
fn collect_unsupported_in_expr(expr: &BodyExpr) -> Vec<(String, Option<String>)> {
    let mut out = Vec::new();
    walk_expr_for_unsupported(expr, &mut out);
    out
}

fn walk_expr_for_unsupported(expr: &BodyExpr, out: &mut Vec<(String, Option<String>)>) {
    match expr {
        BodyExpr::Unsupported {
            hir_kind,
            span_text,
        } => {
            out.push((hir_kind.clone(), span_text.clone()));
        }
        BodyExpr::Block { stmts, expr } => {
            for stmt in stmts {
                walk_stmt_for_unsupported(stmt, out);
            }
            if let Some(e) = expr {
                walk_expr_for_unsupported(e, out);
            }
        }
        BodyExpr::MethodCall { receiver, args, .. } => {
            walk_expr_for_unsupported(receiver, out);
            for arg in args {
                walk_expr_for_unsupported(arg, out);
            }
        }
        BodyExpr::Call { args, .. } => {
            for arg in args {
                walk_expr_for_unsupported(arg, out);
            }
        }
        BodyExpr::Field { base, .. } => {
            walk_expr_for_unsupported(base, out);
        }
        BodyExpr::Binary { lhs, rhs, .. } => {
            walk_expr_for_unsupported(lhs, out);
            walk_expr_for_unsupported(rhs, out);
        }
        BodyExpr::Unary { operand, .. } => {
            walk_expr_for_unsupported(operand, out);
        }
        BodyExpr::Cast { expr, .. } | BodyExpr::Try { expr } | BodyExpr::Ref { expr, .. } => {
            walk_expr_for_unsupported(expr, out);
        }
        BodyExpr::If {
            cond,
            then_branch,
            else_branch,
        } => {
            walk_expr_for_unsupported(cond, out);
            walk_expr_for_unsupported(then_branch, out);
            if let Some(e) = else_branch {
                walk_expr_for_unsupported(e, out);
            }
        }
        BodyExpr::Loop { body } => {
            walk_expr_for_unsupported(body, out);
        }
        BodyExpr::Return { expr } | BodyExpr::Break { expr } => {
            if let Some(e) = expr {
                walk_expr_for_unsupported(e, out);
            }
        }
        BodyExpr::Assign { target, value } => {
            walk_expr_for_unsupported(target, out);
            walk_expr_for_unsupported(value, out);
        }
        BodyExpr::AssignOp { target, value, .. } => {
            walk_expr_for_unsupported(target, out);
            walk_expr_for_unsupported(value, out);
        }
        BodyExpr::Tuple { elements } | BodyExpr::Array { elements } => {
            for e in elements {
                walk_expr_for_unsupported(e, out);
            }
        }
        BodyExpr::Index { base, index } => {
            walk_expr_for_unsupported(base, out);
            walk_expr_for_unsupported(index, out);
        }
        BodyExpr::Struct { fields, base, .. } => {
            for f in fields {
                walk_expr_for_unsupported(&f.value, out);
            }
            if let Some(b) = base {
                walk_expr_for_unsupported(b, out);
            }
        }
        BodyExpr::Closure { body, .. } => {
            walk_expr_for_unsupported(body, out);
        }
        BodyExpr::Match { scrutinee, arms } => {
            walk_expr_for_unsupported(scrutinee, out);
            for arm in arms {
                if let Some(g) = &arm.guard {
                    walk_expr_for_unsupported(g, out);
                }
                walk_expr_for_unsupported(&arm.body, out);
            }
        }
        BodyExpr::Path { .. }
        | BodyExpr::LitInt { .. }
        | BodyExpr::LitBool { .. }
        | BodyExpr::LitStr { .. }
        | BodyExpr::Continue => {}
    }
}

fn walk_stmt_for_unsupported(stmt: &BodyStmt, out: &mut Vec<(String, Option<String>)>) {
    match stmt {
        BodyStmt::Expr { expr } | BodyStmt::Semi { expr } => {
            walk_expr_for_unsupported(expr, out);
        }
        BodyStmt::Let { init, .. } => {
            if let Some(e) = init {
                walk_expr_for_unsupported(e, out);
            }
        }
    }
}
