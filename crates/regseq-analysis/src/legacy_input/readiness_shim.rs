//! Legacy readiness reporter targeted at [`AnalysisInput`].
//!
//! Preserves the pre-RegseqModel [`check_readiness`] entry point and
//! its associated finding / report vocabulary (`FindingSeverity`,
//! `ReadinessFinding`, `ReadinessReport`) for callers that have not
//! yet migrated to [`crate::readiness::check_readiness`] against
//! `&RegseqModel`. See [`crate::legacy_input`] for the retirement
//! plan (target: `regseq-lowering-10`).

use std::collections::BTreeSet;

use regseq_model::compat::{BusType, CallbackFamily, DeviceClass, PropertyLintDecl};

use super::input_shim::{AnalysisInput, ResourceJoinSummary};

/// Expected callback families per device class.
fn expected_callback_families(device_class: &DeviceClass) -> BTreeSet<CallbackFamily> {
    let mut expected = BTreeSet::new();
    expected.insert(CallbackFamily::Lifecycle);
    match device_class {
        DeviceClass::SerialUart => {
            expected.insert(CallbackFamily::Serial);
        }
        DeviceClass::Network => {}
        DeviceClass::Other(_) => {}
    }
    expected
}

/// A single readiness finding.
#[deprecated(
    note = "Shim type; migrate to crate::readiness::ReadinessReport via regseq-lowering-10"
)]
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ReadinessFinding {
    /// Severity of this finding.
    pub severity: FindingSeverity,
    /// Category label for grouping.
    pub category: &'static str,
    /// Human-readable finding message.
    pub message: String,
}

/// Severity levels for legacy readiness findings.
#[deprecated(
    note = "Shim type; migrate to crate::readiness::ReadinessReport via regseq-lowering-10"
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum FindingSeverity {
    /// An expected-complete requirement is missing.
    Error,
    /// A caution point is visible.
    Caution,
    /// An informational observation.
    Info,
}

/// Deterministic legacy readiness report for the lowering-input contract.
#[deprecated(
    note = "Shim type; migrate to crate::readiness::ReadinessReport via regseq-lowering-10"
)]
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

/// Run all legacy readiness checks against the given analysis input.
#[deprecated(
    note = "Shim entry point; migrate to crate::readiness::check_readiness(&RegseqModel, &VerifyReport) via regseq-lowering-10"
)]
pub fn check_readiness(input: &AnalysisInput) -> ReadinessReport {
    let mut findings = Vec::new();

    check_callback_family_integrity(input, &mut findings);
    check_property_lints(input, &mut findings);
    check_resource_join_completeness(input, &mut findings);
    check_interrupt_authority(input, &mut findings);
    check_resource_optionality(input, &mut findings);
    check_bus_consistency(input, &mut findings);

    findings.sort();

    let join_summaries = input.all_join_summaries();

    ReadinessReport {
        findings,
        join_summaries,
    }
}

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

fn check_resource_optionality(input: &AnalysisInput, findings: &mut Vec<ReadinessFinding>) {
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

fn check_bus_consistency(input: &AnalysisInput, findings: &mut Vec<ReadinessFinding>) {
    let decl_bus = input.bus_identity();
    let binding_bus = input.bus_binding();

    match (decl_bus, binding_bus) {
        (Some(l1), Some(l2)) => {
            if *l1 != l2.bus_type {
                findings.push(ReadinessFinding {
                    severity: FindingSeverity::Error,
                    category: "bus_consistency",
                    message: format!(
                        "Layer 1 declares bus '{}' but Layer 2 specifies '{}'",
                        l1.as_str(),
                        l2.bus_type.as_str(),
                    ),
                });
            } else {
                findings.push(ReadinessFinding {
                    severity: FindingSeverity::Info,
                    category: "bus_consistency",
                    message: format!(
                        "bus '{}' consistent across Layer 1 and Layer 2",
                        l1.as_str(),
                    ),
                });
            }

            check_bus_required_metadata(&l2.bus_type, l2.amba_id, findings);
        }
        (Some(l1), None) => {
            findings.push(ReadinessFinding {
                severity: FindingSeverity::Caution,
                category: "bus_consistency",
                message: format!(
                    "Layer 1 declares bus '{}' but no [bus] section in Layer 2",
                    l1.as_str(),
                ),
            });
        }
        (None, Some(l2)) => {
            findings.push(ReadinessFinding {
                severity: FindingSeverity::Caution,
                category: "bus_consistency",
                message: format!(
                    "Layer 2 specifies bus '{}' but no bus declared in Layer 1",
                    l2.bus_type.as_str(),
                ),
            });
        }
        (None, None) => {}
    }
}

fn check_bus_required_metadata(
    bus_type: &BusType,
    amba_id: Option<i64>,
    findings: &mut Vec<ReadinessFinding>,
) {
    match bus_type {
        BusType::Amba => {
            if amba_id.is_none() {
                findings.push(ReadinessFinding {
                    severity: FindingSeverity::Caution,
                    category: "bus_consistency",
                    message: "AMBA bus family present but amba_id is missing in Layer 2".into(),
                });
            }
        }
        BusType::Pci | BusType::Integrated => {}
    }
}
