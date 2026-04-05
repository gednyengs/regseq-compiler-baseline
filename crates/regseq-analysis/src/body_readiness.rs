//! MIR body capture readiness and supported-subset validation.
//!
//! Validates that the analysis input contains adequate MIR-derived body
//! capture for later lowering, and surfaces any unsupported constructs
//! or coverage gaps explicitly.
//!
//! Design rules:
//! - device-neutral and OS-neutral: no PL011- or Linux-specific logic
//! - no manual body construction: all body data comes from the model
//! - unsupported constructs are surfaced, never silently skipped
//! - deterministic output
//! - uses MIR bodies (mir_bodies) exclusively, not the legacy HIR path

use std::collections::{BTreeMap, BTreeSet};

use crate::input::AnalysisInput;
use crate::readiness::FindingSeverity;

use regseq_model::declaration::callbacks::CallbackFamily;
use regseq_model::declaration::mir_body::{
    FnKind, MirBody, MirOperand, MirProjectionElem, MirQueryFlavor, MirRvalue, MirStatementKind,
    MirTerminatorKind,
};

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

/// Aggregate counts of MIR forms encountered across all bodies.
///
/// Makes the supported-subset explicit rather than implicit: each
/// form kind is counted so reviewers can see exactly which MIR
/// constructs appear in the current slice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MirFormSummary {
    /// Whether the query flavor is the required `DropsElaborated`.
    pub query_flavor_valid: bool,
    /// Number of basic blocks marked as cleanup (unwind) blocks.
    pub cleanup_block_count: usize,
    /// Number of `Drop` terminators across all bodies.
    pub drop_terminator_count: usize,
    /// Number of detected fallible control-flow patterns (`?`-style
    /// SwitchInt result discrimination).
    pub fallible_pattern_count: usize,
    /// Per-kind count of statements encountered.
    pub statement_kinds: BTreeMap<String, usize>,
    /// Per-kind count of terminators encountered.
    pub terminator_kinds: BTreeMap<String, usize>,
    /// Per-kind count of rvalues encountered.
    pub rvalue_kinds: BTreeMap<String, usize>,
}

/// Result of MIR body capture readiness validation.
///
/// Contains deterministic findings about body coverage, supported-subset
/// compliance, cleanup visibility, and provenance completeness.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BodyReadinessReport {
    /// All findings, sorted by severity then category then message.
    pub findings: Vec<BodyFinding>,
    /// Number of MIR callback bodies present.
    pub callback_body_count: usize,
    /// Number of MIR helper bodies present.
    pub helper_body_count: usize,
    /// Number of MIR ISR bodies present.
    pub isr_body_count: usize,
    /// Number of helpers transitively reachable from callbacks and ISRs.
    pub reachable_helper_count: usize,
    /// Total unsupported MIR nodes found across all bodies.
    pub unsupported_node_count: usize,
    /// Aggregate MIR form summary for explicit subset enumeration.
    pub mir_form_summary: MirFormSummary,
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

/// Validate MIR body capture and supported-subset for the analysis input.
///
/// Returns a deterministic report of findings about body coverage,
/// unsupported constructs, cleanup visibility, and provenance completeness.
/// Uses the canonical `mir_bodies` path, not the legacy HIR body set.
pub fn check_body_readiness(input: &AnalysisInput) -> BodyReadinessReport {
    let mut findings = Vec::new();

    let callback_bodies = input.mir_callback_bodies();
    let helper_bodies = input.mir_helper_bodies();
    let isr_bodies = input.mir_isr_bodies();
    let reachable = input.mir_reachable_helper_names();

    check_query_flavor(input, &mut findings);
    check_callback_body_coverage(input, &mut findings);
    check_reachable_helper_coverage(input, &reachable, &mut findings);
    let unsupported_count = check_unsupported_nodes(input, &mut findings);
    check_cleanup_visibility(input, &mut findings);
    check_fallible_patterns(input, &mut findings);
    check_body_provenance(input, &mut findings);

    let mir_form_summary = build_mir_form_summary(input);

    findings.sort();

    BodyReadinessReport {
        findings,
        callback_body_count: callback_bodies.len(),
        helper_body_count: helper_bodies.len(),
        isr_body_count: isr_bodies.len(),
        reachable_helper_count: reachable.len(),
        unsupported_node_count: unsupported_count,
        mir_form_summary,
    }
}

// ----------------------------------------------------------------
// Query flavor validation
// ----------------------------------------------------------------

fn check_query_flavor(input: &AnalysisInput, findings: &mut Vec<BodyFinding>) {
    match input.mir_query_flavor() {
        MirQueryFlavor::DropsElaborated => {
            findings.push(BodyFinding {
                severity: FindingSeverity::Info,
                category: "query_flavor",
                message: "MIR query flavor is DropsElaborated (required)".into(),
            });
        }
    }
}

// ----------------------------------------------------------------
// Callback body coverage
// ----------------------------------------------------------------

/// Map a callback family to its expected trait name in body declarations.
fn family_trait_name(family: &CallbackFamily) -> &'static str {
    match family {
        CallbackFamily::Lifecycle => "Lifecycle",
        CallbackFamily::Serial => "Serial",
        CallbackFamily::SerialConsole => "SerialConsole",
        CallbackFamily::PowerManagement => "PowerManagement",
    }
}

fn check_callback_body_coverage(input: &AnalysisInput, findings: &mut Vec<BodyFinding>) {
    let families = input.callback_families_present();
    let callback_bodies = input.mir_callback_bodies();

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
                    "callback family '{}' is declared but no MIR bodies are captured",
                    family.as_str(),
                ),
            });
        }
    }

    if !callback_bodies.is_empty() {
        findings.push(BodyFinding {
            severity: FindingSeverity::Info,
            category: "body_coverage",
            message: format!(
                "{} MIR callback body/bodies captured across {} family/families",
                callback_bodies.len(),
                families.len(),
            ),
        });
    }
}

// ----------------------------------------------------------------
// Reachable helper coverage
// ----------------------------------------------------------------

fn check_reachable_helper_coverage(
    input: &AnalysisInput,
    reachable: &BTreeSet<String>,
    findings: &mut Vec<BodyFinding>,
) {
    for name in reachable {
        if input.mir_body_by_name(name).is_none() {
            findings.push(BodyFinding {
                severity: FindingSeverity::Error,
                category: "helper_coverage",
                message: format!("reachable helper '{}' has no captured MIR body", name),
            });
        }
    }
}

// ----------------------------------------------------------------
// Unsupported MIR construct detection
// ----------------------------------------------------------------

fn check_unsupported_nodes(input: &AnalysisInput, findings: &mut Vec<BodyFinding>) -> usize {
    let mut total = 0;

    for func in &input.mir_bodies().functions {
        let unsupported = collect_unsupported_in_mir(&func.body);
        if !unsupported.is_empty() {
            for (form_kind, description) in &unsupported {
                findings.push(BodyFinding {
                    severity: FindingSeverity::Caution,
                    category: "unsupported_construct",
                    message: format!(
                        "function '{}': unsupported MIR {} '{}'",
                        func.name, form_kind, description,
                    ),
                });
            }
            total += unsupported.len();
        }
    }

    total
}

/// Collect all unsupported MIR constructs in a body.
///
/// Returns `(form_kind, description)` pairs where `form_kind` is one of
/// "statement", "terminator", "rvalue", "projection".
fn collect_unsupported_in_mir(body: &MirBody) -> Vec<(&'static str, String)> {
    let mut out = Vec::new();

    for bb in &body.basic_blocks {
        for stmt in &bb.statements {
            if let MirStatementKind::Unsupported { description } = &stmt.kind {
                out.push(("statement", description.clone()));
            }
            // Check rvalues inside Assign statements.
            if let MirStatementKind::Assign { rvalue, .. } = &stmt.kind {
                collect_unsupported_in_rvalue(rvalue, &mut out);
            }
        }
        if let Some(ref term) = bb.terminator {
            match &term.kind {
                MirTerminatorKind::Unsupported { description } => {
                    out.push(("terminator", description.clone()));
                }
                // Check operands inside Call/SwitchInt/Assert for unsupported constants.
                MirTerminatorKind::Call { func, args, .. } => {
                    collect_unsupported_in_operand(func, &mut out);
                    for arg in args {
                        collect_unsupported_in_operand(arg, &mut out);
                    }
                }
                MirTerminatorKind::SwitchInt { discr, .. } => {
                    collect_unsupported_in_operand(discr, &mut out);
                }
                MirTerminatorKind::Assert { cond, .. } => {
                    collect_unsupported_in_operand(cond, &mut out);
                }
                _ => {}
            }
        }
    }

    out
}

fn collect_unsupported_in_rvalue(rvalue: &MirRvalue, out: &mut Vec<(&'static str, String)>) {
    match rvalue {
        MirRvalue::Unsupported { description } => {
            out.push(("rvalue", description.clone()));
        }
        MirRvalue::Use { operand }
        | MirRvalue::UnaryOp { operand, .. }
        | MirRvalue::Cast { operand, .. } => {
            collect_unsupported_in_operand(operand, out);
        }
        MirRvalue::BinaryOp { lhs, rhs, .. } => {
            collect_unsupported_in_operand(lhs, out);
            collect_unsupported_in_operand(rhs, out);
        }
        MirRvalue::Aggregate { operands, .. } => {
            for op in operands {
                collect_unsupported_in_operand(op, out);
            }
        }
        MirRvalue::Ref { place, .. }
        | MirRvalue::Discriminant { place }
        | MirRvalue::CopyForDeref { place } => {
            collect_unsupported_in_place_projections(place, out);
        }
    }
}

fn collect_unsupported_in_operand(operand: &MirOperand, out: &mut Vec<(&'static str, String)>) {
    match operand {
        MirOperand::Copy { place } | MirOperand::Move { place } => {
            collect_unsupported_in_place_projections(place, out);
        }
        MirOperand::Constant { .. } => {}
    }
}

fn collect_unsupported_in_place_projections(
    place: &regseq_model::declaration::mir_body::MirPlace,
    out: &mut Vec<(&'static str, String)>,
) {
    for proj in &place.projection {
        if let MirProjectionElem::Unsupported { description } = proj {
            out.push(("projection", description.clone()));
        }
    }
}

// ----------------------------------------------------------------
// Cleanup / drop visibility
// ----------------------------------------------------------------

fn check_cleanup_visibility(input: &AnalysisInput, findings: &mut Vec<BodyFinding>) {
    let mut total_cleanup_blocks = 0usize;
    let mut total_drop_terminators = 0usize;
    let mut functions_with_cleanup = 0usize;

    for func in &input.mir_bodies().functions {
        let mut fn_cleanup = 0usize;
        let mut fn_drops = 0usize;

        for bb in &func.body.basic_blocks {
            if bb.is_cleanup {
                fn_cleanup += 1;
            }
            if let Some(ref term) = bb.terminator {
                if matches!(term.kind, MirTerminatorKind::Drop { .. }) {
                    fn_drops += 1;
                }
            }
        }

        if fn_cleanup > 0 {
            functions_with_cleanup += 1;
        }
        total_cleanup_blocks += fn_cleanup;
        total_drop_terminators += fn_drops;
    }

    if total_cleanup_blocks > 0 || total_drop_terminators > 0 {
        findings.push(BodyFinding {
            severity: FindingSeverity::Info,
            category: "cleanup_visibility",
            message: format!(
                "{} cleanup block(s) and {} Drop terminator(s) across {} function(s)",
                total_cleanup_blocks, total_drop_terminators, functions_with_cleanup,
            ),
        });
    }
}

// ----------------------------------------------------------------
// Fallible control-flow pattern detection
// ----------------------------------------------------------------

/// Detect `?`-style fallible control-flow patterns in MIR.
///
/// In post-drop-elaboration MIR, the `?` operator desugars to a
/// `SwitchInt` on the discriminant of a `Result` value, with one
/// branch continuing on the success path and another running cleanup
/// and returning the error. We detect this pattern by looking for
/// `SwitchInt` terminators that follow a `Discriminant` rvalue
/// assignment in the same basic block.
fn check_fallible_patterns(input: &AnalysisInput, findings: &mut Vec<BodyFinding>) {
    for func in &input.mir_bodies().functions {
        let count = count_fallible_patterns(&func.body);
        if count > 0 {
            findings.push(BodyFinding {
                severity: FindingSeverity::Info,
                category: "fallible_pattern",
                message: format!(
                    "function '{}': {} fallible control-flow pattern(s) detected",
                    func.name, count,
                ),
            });
        }
    }
}

fn count_fallible_patterns(body: &MirBody) -> usize {
    let mut count = 0;
    for bb in &body.basic_blocks {
        // Pattern: block has a Discriminant rvalue assignment followed
        // by a SwitchInt terminator on the same local.
        let has_discriminant_assign = bb.statements.iter().any(|stmt| {
            matches!(
                &stmt.kind,
                MirStatementKind::Assign { rvalue, .. }
                    if matches!(**rvalue, MirRvalue::Discriminant { .. })
            )
        });
        let has_switch = bb
            .terminator
            .as_ref()
            .is_some_and(|t| matches!(t.kind, MirTerminatorKind::SwitchInt { .. }));

        if has_discriminant_assign && has_switch {
            count += 1;
        }
    }
    count
}

// ----------------------------------------------------------------
// Provenance validation
// ----------------------------------------------------------------

fn check_body_provenance(input: &AnalysisInput, findings: &mut Vec<BodyFinding>) {
    for func in &input.mir_bodies().functions {
        if func.provenance.raw_family.is_empty() {
            findings.push(BodyFinding {
                severity: FindingSeverity::Caution,
                category: "body_provenance",
                message: format!("function '{}' has empty provenance raw_family", func.name),
            });
        }
        if func.def_path.is_empty() {
            findings.push(BodyFinding {
                severity: FindingSeverity::Caution,
                category: "body_provenance",
                message: format!("function '{}' has empty def_path", func.name),
            });
        }
    }
}

// ----------------------------------------------------------------
// MIR form summary builder
// ----------------------------------------------------------------

fn build_mir_form_summary(input: &AnalysisInput) -> MirFormSummary {
    let query_flavor_valid = matches!(input.mir_query_flavor(), MirQueryFlavor::DropsElaborated);

    let mut cleanup_block_count = 0usize;
    let mut drop_terminator_count = 0usize;
    let mut fallible_pattern_count = 0usize;
    let mut statement_kinds: BTreeMap<String, usize> = BTreeMap::new();
    let mut terminator_kinds: BTreeMap<String, usize> = BTreeMap::new();
    let mut rvalue_kinds: BTreeMap<String, usize> = BTreeMap::new();

    for func in &input.mir_bodies().functions {
        fallible_pattern_count += count_fallible_patterns(&func.body);
        for bb in &func.body.basic_blocks {
            if bb.is_cleanup {
                cleanup_block_count += 1;
            }
            for stmt in &bb.statements {
                let kind_name = stmt_kind_name(&stmt.kind);
                *statement_kinds.entry(kind_name.into()).or_insert(0) += 1;

                // Count rvalue kinds inside Assign.
                if let MirStatementKind::Assign { rvalue, .. } = &stmt.kind {
                    let rv_name = rvalue_kind_name(rvalue);
                    *rvalue_kinds.entry(rv_name.into()).or_insert(0) += 1;
                }
            }
            if let Some(ref term) = bb.terminator {
                let kind_name = term_kind_name(&term.kind);
                *terminator_kinds.entry(kind_name.into()).or_insert(0) += 1;

                if matches!(term.kind, MirTerminatorKind::Drop { .. }) {
                    drop_terminator_count += 1;
                }
            }
        }
    }

    MirFormSummary {
        query_flavor_valid,
        cleanup_block_count,
        drop_terminator_count,
        fallible_pattern_count,
        statement_kinds,
        terminator_kinds,
        rvalue_kinds,
    }
}

fn stmt_kind_name(kind: &MirStatementKind) -> &'static str {
    match kind {
        MirStatementKind::Assign { .. } => "Assign",
        MirStatementKind::StorageLive { .. } => "StorageLive",
        MirStatementKind::StorageDead { .. } => "StorageDead",
        MirStatementKind::SetDiscriminant { .. } => "SetDiscriminant",
        MirStatementKind::Nop => "Nop",
        MirStatementKind::Unsupported { .. } => "Unsupported",
    }
}

fn term_kind_name(kind: &MirTerminatorKind) -> &'static str {
    match kind {
        MirTerminatorKind::Goto { .. } => "Goto",
        MirTerminatorKind::SwitchInt { .. } => "SwitchInt",
        MirTerminatorKind::Return => "Return",
        MirTerminatorKind::Unreachable => "Unreachable",
        MirTerminatorKind::UnwindResume => "UnwindResume",
        MirTerminatorKind::UnwindTerminate { .. } => "UnwindTerminate",
        MirTerminatorKind::Call { .. } => "Call",
        MirTerminatorKind::Drop { .. } => "Drop",
        MirTerminatorKind::Assert { .. } => "Assert",
        MirTerminatorKind::Unsupported { .. } => "Unsupported",
    }
}

fn rvalue_kind_name(rvalue: &MirRvalue) -> &'static str {
    match rvalue {
        MirRvalue::Use { .. } => "Use",
        MirRvalue::Ref { .. } => "Ref",
        MirRvalue::BinaryOp { .. } => "BinaryOp",
        MirRvalue::UnaryOp { .. } => "UnaryOp",
        MirRvalue::Discriminant { .. } => "Discriminant",
        MirRvalue::Aggregate { .. } => "Aggregate",
        MirRvalue::Cast { .. } => "Cast",
        MirRvalue::CopyForDeref { .. } => "CopyForDeref",
        MirRvalue::Unsupported { .. } => "Unsupported",
    }
}
