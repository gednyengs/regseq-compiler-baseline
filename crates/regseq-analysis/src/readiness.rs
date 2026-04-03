//! Lowering-input readiness and consistency checks.
//!
//! Assesses whether the current declaration-plus-joined artifact is
//! complete, consistent, and reviewable enough to act as the
//! lowering-input contract for Phase D.
//!
//! Design rules:
//! - analysis-facing only: no backend lowering logic
//! - deterministic output
//! - partial joins and unmatched facts remain explicit (not collapsed)
//! - caution points remain visible

use std::collections::BTreeSet;

use crate::input::{AnalysisInput, ResourceJoinSummary};
use regseq_model::declaration::callbacks::CallbackFamily;
use regseq_model::declaration::driver::DeviceClass;
use regseq_model::declaration::properties::PropertyLintDecl;

/// Expected callback families per device class.
///
/// Returns the set of families that a device class is expected to
/// have for the accepted PL011/E1000 surfaces. This is the current
/// accepted baseline, not a backend policy decision.
fn expected_callback_families(device_class: &DeviceClass) -> BTreeSet<CallbackFamily> {
    let mut expected = BTreeSet::new();
    expected.insert(CallbackFamily::Lifecycle);
    match device_class {
        DeviceClass::SerialUart => {
            expected.insert(CallbackFamily::Serial);
            // SerialConsole and PowerManagement are optional.
        }
        DeviceClass::Network => {
            // Network has no serial, but lifecycle is required.
        }
        DeviceClass::Other(_) => {}
    }
    expected
}

/// A single readiness finding.
///
/// Findings are deterministic, sorted, and carry enough context for
/// review. Each finding has a severity level and a structured message.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ReadinessFinding {
    /// Severity of this finding.
    pub severity: FindingSeverity,
    /// Category label for grouping.
    pub category: &'static str,
    /// Human-readable finding message.
    pub message: String,
}

/// Severity levels for readiness findings.
///
/// These do NOT map to backend pass/fail. They are reviewer-facing
/// indicators of analysis-layer concern.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum FindingSeverity {
    /// An expected-complete requirement is missing. Phase D may not proceed.
    Error,
    /// A caution point is visible. Phase D can proceed with awareness.
    Caution,
    /// An informational observation.
    Info,
}

/// Deterministic readiness report for the lowering-input contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadinessReport {
    /// All findings, sorted by severity then category then message.
    pub findings: Vec<ReadinessFinding>,
    /// Per-family join summaries (carried through for review).
    pub join_summaries: Vec<ResourceJoinSummary>,
}

impl ReadinessReport {
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
    pub fn errors(&self) -> Vec<&ReadinessFinding> {
        self.findings
            .iter()
            .filter(|f| f.severity == FindingSeverity::Error)
            .collect()
    }

    /// Return only caution findings.
    pub fn cautions(&self) -> Vec<&ReadinessFinding> {
        self.findings
            .iter()
            .filter(|f| f.severity == FindingSeverity::Caution)
            .collect()
    }

    /// Return only info findings.
    pub fn infos(&self) -> Vec<&ReadinessFinding> {
        self.findings
            .iter()
            .filter(|f| f.severity == FindingSeverity::Info)
            .collect()
    }
}

/// Run all readiness checks against the given analysis input.
///
/// Returns a deterministic report of findings. The report preserves
/// partial join outcomes and caution points rather than collapsing
/// them into pass/fail.
pub fn check_readiness(input: &AnalysisInput) -> ReadinessReport {
    let mut findings = Vec::new();

    check_callback_family_integrity(input, &mut findings);
    check_property_lints(input, &mut findings);
    check_resource_join_completeness(input, &mut findings);
    check_interrupt_authority(input, &mut findings);
    check_resource_optionality(input, &mut findings);

    findings.sort();

    let join_summaries = input.all_join_summaries();

    ReadinessReport {
        findings,
        join_summaries,
    }
}

/// Check callback-family integrity for the current device class.
fn check_callback_family_integrity(input: &AnalysisInput, findings: &mut Vec<ReadinessFinding>) {
    let device_class = &input.declaration().driver.device_class;
    let expected = expected_callback_families(device_class);
    let present = input.callback_families_present();

    for family in &expected {
        if !present.contains(family) {
            findings.push(ReadinessFinding {
                severity: FindingSeverity::Error,
                category: "callback_integrity",
                message: format!(
                    "expected callback family '{}' for device class '{}' is missing",
                    family.as_str(),
                    device_class.as_str(),
                ),
            });
        }
    }

    // Report optional families that are present (informational).
    for family in &present {
        if !expected.contains(family) {
            findings.push(ReadinessFinding {
                severity: FindingSeverity::Info,
                category: "callback_integrity",
                message: format!(
                    "optional callback family '{}' is present for device class '{}'",
                    family.as_str(),
                    device_class.as_str(),
                ),
            });
        }
    }
}

/// Check property-lint carry-through.
fn check_property_lints(input: &AnalysisInput, findings: &mut Vec<ReadinessFinding>) {
    for lint in input.property_lints() {
        match lint {
            PropertyLintDecl::RequiredWithDefault {
                property_id,
                field_name,
                ..
            } => {
                findings.push(ReadinessFinding {
                    severity: FindingSeverity::Caution,
                    category: "property_lint",
                    message: format!(
                        "property '{}' (field '{}') is required but has a default value",
                        property_id, field_name,
                    ),
                });
            }
        }
    }
}

/// Check resource join completeness across all families.
fn check_resource_join_completeness(input: &AnalysisInput, findings: &mut Vec<ReadinessFinding>) {
    for summary in input.all_join_summaries() {
        if summary.joined_incomplete > 0 {
            findings.push(ReadinessFinding {
                severity: FindingSeverity::Caution,
                category: "join_completeness",
                message: format!(
                    "{}: {} join(s) matched but incomplete",
                    summary.family, summary.joined_incomplete,
                ),
            });
        }
        if summary.decl_only > 0 {
            findings.push(ReadinessFinding {
                severity: FindingSeverity::Caution,
                category: "join_completeness",
                message: format!(
                    "{}: {} declaration-only entry/entries (no matching binding)",
                    summary.family, summary.decl_only,
                ),
            });
        }
        if summary.binding_only > 0 {
            findings.push(ReadinessFinding {
                severity: FindingSeverity::Caution,
                category: "join_completeness",
                message: format!(
                    "{}: {} binding-only entry/entries (no matching declaration)",
                    summary.family, summary.binding_only,
                ),
            });
        }
    }
}

/// Check interrupt binding-side authority.
fn check_interrupt_authority(input: &AnalysisInput, findings: &mut Vec<ReadinessFinding>) {
    for entry in input.interrupts_joined() {
        let has_decl_hints = entry.decl_binding_hints.trigger_type.is_some()
            || entry.decl_binding_hints.trigger_polarity.is_some()
            || entry.decl_binding_hints.priority.is_some()
            || entry.decl_binding_hints.shared.is_some();

        if has_decl_hints {
            findings.push(ReadinessFinding {
                severity: FindingSeverity::Info,
                category: "interrupt_authority",
                message: format!(
                    "interrupt '{}': declaration-side binding hints present (diagnostic-only, \
                     not authoritative)",
                    entry.interrupt_id,
                ),
            });
        }
    }
}

/// Check optional-resource presence versus required-resource absence.
fn check_resource_optionality(input: &AnalysisInput, findings: &mut Vec<ReadinessFinding>) {
    // Check clock fields.
    for field in input.declaration().clock_fields() {
        if field.required {
            let is_joined = input
                .clocks_joined()
                .iter()
                .any(|e| e.clock_id == field.clock_id);
            if !is_joined {
                findings.push(ReadinessFinding {
                    severity: FindingSeverity::Error,
                    category: "resource_optionality",
                    message: format!(
                        "required clock '{}' has no matching binding",
                        field.clock_id,
                    ),
                });
            }
        }
    }

    // Check reset fields.
    for field in input.declaration().reset_fields() {
        if field.required {
            let is_joined = input
                .resets_joined()
                .iter()
                .any(|e| e.field_name == field.field_name);
            if !is_joined {
                findings.push(ReadinessFinding {
                    severity: FindingSeverity::Error,
                    category: "resource_optionality",
                    message: format!(
                        "required reset '{}' has no matching binding",
                        field.field_name,
                    ),
                });
            }
        }
    }

    // Check pinctrl fields.
    for field in input.declaration().pinctrl_fields() {
        if field.required {
            let is_joined = input
                .pinctrl_joined()
                .iter()
                .any(|e| e.field_name == field.field_name);
            if !is_joined {
                findings.push(ReadinessFinding {
                    severity: FindingSeverity::Error,
                    category: "resource_optionality",
                    message: format!(
                        "required pinctrl '{}' has no matching binding",
                        field.field_name,
                    ),
                });
            }
        }
    }

    // Check required properties.
    for field in &input.declaration().properties.fields {
        if field.required {
            let is_joined = input
                .properties_joined()
                .iter()
                .any(|e| e.property_id == field.property_id);
            if !is_joined {
                findings.push(ReadinessFinding {
                    severity: FindingSeverity::Error,
                    category: "resource_optionality",
                    message: format!(
                        "required property '{}' has no matching binding",
                        field.property_id,
                    ),
                });
            }
        }
    }
}
