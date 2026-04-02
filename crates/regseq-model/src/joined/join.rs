//! Join logic combining declaration IR and binding IR into a joined artifact.
//!
//! Uses explicit-id join and field-name join as the only accepted
//! mechanisms. The join is deterministic and local: it does not infer
//! defaults, perform lowering, or widen the contract.

use std::collections::BTreeMap;
use std::fmt;

use crate::binding::BindingModel;
use crate::declaration::DeclarationModel;

use super::artifact::*;

/// Errors produced during the join.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JoinError {
    /// A structural error that prevents the join from proceeding.
    StructuralError(String),
}

impl fmt::Display for JoinError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StructuralError(msg) => write!(f, "join error: {msg}"),
        }
    }
}

impl std::error::Error for JoinError {}

/// Perform the Layer 1 + Layer 2 join.
///
/// Combines a normalized `DeclarationModel` with a normalized
/// `BindingModel` into a `JoinedArtifact`. The join preserves
/// provenance to both sides and makes unmatched entries explicit.
pub fn perform_join(
    decl: &DeclarationModel,
    binding: &BindingModel,
) -> Result<JoinedArtifact, JoinError> {
    let header = JoinedHeader::from(binding.header.clone());

    let (properties_joined, properties_decl_only, properties_binding_only) =
        join_properties(decl, binding);
    let (mmio_joined, mmio_decl_only, mmio_binding_only) = join_mmio(decl, binding);
    let (interrupts_joined, interrupts_decl_only, interrupts_binding_only) =
        join_interrupts(decl, binding);
    let (clocks_joined, clocks_decl_only, clocks_binding_only) = join_clocks(decl, binding);
    let (resets_joined, resets_decl_only, resets_binding_only) = join_resets(decl, binding);
    let (pinctrl_joined, pinctrl_decl_only, pinctrl_binding_only) = join_pinctrl(decl, binding);

    Ok(JoinedArtifact {
        header,
        properties_joined,
        properties_decl_only,
        properties_binding_only,
        mmio_joined,
        mmio_decl_only,
        mmio_binding_only,
        interrupts_joined,
        interrupts_decl_only,
        interrupts_binding_only,
        clocks_joined,
        clocks_decl_only,
        clocks_binding_only,
        resets_joined,
        resets_decl_only,
        resets_binding_only,
        pinctrl_joined,
        pinctrl_decl_only,
        pinctrl_binding_only,
    })
}

// ---- Property join ----

fn join_properties(
    decl: &DeclarationModel,
    binding: &BindingModel,
) -> (
    Vec<PropertyJoinEntry>,
    Vec<PropertyDeclOnly>,
    Vec<crate::binding::PropertyBinding>,
) {
    let binding_by_key: BTreeMap<&str, &crate::binding::PropertyBinding> = binding
        .properties
        .iter()
        .map(|p| (p.key.as_str(), p))
        .collect();

    let mut joined = Vec::new();
    let mut decl_only = Vec::new();
    let mut matched_keys = Vec::new();

    for field in &decl.properties.fields {
        if let Some(bp) = binding_by_key.get(field.property_id.as_str()) {
            matched_keys.push(field.property_id.as_str());
            joined.push(PropertyJoinEntry {
                property_id: field.property_id.clone(),
                field_name: field.field_name.clone(),
                field_type: field.field_type.clone(),
                required: field.required,
                default_literal: field.default_literal.clone(),
                binding_value: Some(bp.value.clone()),
                outcome: JoinOutcome::Matched,
                decl_provenance: field.provenance.clone(),
                binding_provenance: Some(bp.provenance.clone()),
            });
        } else {
            decl_only.push(PropertyDeclOnly {
                property_id: field.property_id.clone(),
                field_name: field.field_name.clone(),
                field_type: field.field_type.clone(),
                required: field.required,
                default_literal: field.default_literal.clone(),
                provenance: field.provenance.clone(),
            });
        }
    }

    let binding_only: Vec<_> = binding
        .properties
        .iter()
        .filter(|bp| !matched_keys.contains(&bp.key.as_str()))
        .cloned()
        .collect();

    (joined, decl_only, binding_only)
}

// ---- MMIO join (explicit mmio_id) ----

fn join_mmio(
    decl: &DeclarationModel,
    binding: &BindingModel,
) -> (
    Vec<MmioJoinEntry>,
    Vec<crate::declaration::resources::MmioFieldDecl>,
    Vec<crate::binding::MmioBinding>,
) {
    let binding_by_id: BTreeMap<&str, &crate::binding::MmioBinding> = binding
        .mmio
        .iter()
        .map(|b| (b.join_key.value(), b))
        .collect();

    let mut joined = Vec::new();
    let mut decl_only = Vec::new();
    let mut matched_ids = Vec::new();

    for field in &decl.resources.mmio {
        let join_id = field.mmio_id.as_deref();
        if let Some(id) = join_id {
            if let Some(bp) = binding_by_id.get(id) {
                matched_ids.push(id);
                let outcome = if bp.base.is_some() && bp.size.is_some() {
                    JoinOutcome::Matched
                } else {
                    JoinOutcome::MatchedIncomplete
                };
                joined.push(MmioJoinEntry {
                    field_name: field.field_name.clone(),
                    mmio_id: id.to_string(),
                    register_block_type: field.register_block_type.clone(),
                    base: bp.base,
                    size: bp.size,
                    outcome,
                    decl_provenance: field.provenance.clone(),
                    binding_provenance: bp.provenance.clone(),
                });
                continue;
            }
        }
        decl_only.push(field.clone());
    }

    let binding_only: Vec<_> = binding
        .mmio
        .iter()
        .filter(|b| !matched_ids.contains(&b.join_key.value()))
        .cloned()
        .collect();

    (joined, decl_only, binding_only)
}

// ---- Interrupt join (explicit interrupt_id) ----

fn join_interrupts(
    decl: &DeclarationModel,
    binding: &BindingModel,
) -> (
    Vec<InterruptJoinEntry>,
    Vec<InterruptDeclOnly>,
    Vec<crate::binding::InterruptBinding>,
) {
    let binding_by_id: BTreeMap<&str, &crate::binding::InterruptBinding> = binding
        .interrupts
        .iter()
        .map(|b| (b.join_key.value(), b))
        .collect();

    let mut joined = Vec::new();
    let mut decl_only = Vec::new();
    let mut matched_ids = Vec::new();

    for res in &decl.resources.interrupts.resources {
        if let Some(bp) = binding_by_id.get(res.interrupt_id.as_str()) {
            matched_ids.push(res.interrupt_id.as_str());
            let has_all_binding_fields =
                bp.trigger_type.is_some() && bp.trigger_polarity.is_some() && bp.shared.is_some();
            let outcome = if has_all_binding_fields {
                JoinOutcome::Matched
            } else {
                JoinOutcome::MatchedIncomplete
            };
            joined.push(InterruptJoinEntry {
                field_name: res.field_name.clone(),
                interrupt_id: res.interrupt_id.clone(),
                shape: res.shape.clone(),
                // Authoritative binding values only.
                trigger_type: bp.trigger_type.clone(),
                trigger_polarity: bp.trigger_polarity.clone(),
                priority: bp.priority.clone(),
                shared: bp.shared,
                // Declaration hints preserved as diagnostic annotation.
                decl_binding_hints: res.binding_hints.clone(),
                outcome,
                decl_provenance: res.provenance.clone(),
                binding_provenance: bp.provenance.clone(),
            });
        } else {
            decl_only.push(InterruptDeclOnly {
                field_name: res.field_name.clone(),
                interrupt_id: res.interrupt_id.clone(),
                shape: res.shape.clone(),
                binding_hints: res.binding_hints.clone(),
                provenance: res.provenance.clone(),
            });
        }
    }

    let binding_only: Vec<_> = binding
        .interrupts
        .iter()
        .filter(|b| !matched_ids.contains(&b.join_key.value()))
        .cloned()
        .collect();

    (joined, decl_only, binding_only)
}

// ---- Clock join (explicit clock_id) ----

fn join_clocks(
    decl: &DeclarationModel,
    binding: &BindingModel,
) -> (
    Vec<ClockJoinEntry>,
    Vec<crate::declaration::resources::ClockFieldDecl>,
    Vec<crate::binding::ClockBinding>,
) {
    let binding_by_id: BTreeMap<&str, &crate::binding::ClockBinding> = binding
        .clocks
        .iter()
        .map(|b| (b.join_key.value(), b))
        .collect();

    let mut joined = Vec::new();
    let mut decl_only = Vec::new();
    let mut matched_ids = Vec::new();

    for field in &decl.resources.clock.fields {
        if let Some(bp) = binding_by_id.get(field.clock_id.as_str()) {
            matched_ids.push(field.clock_id.as_str());
            let outcome = if bp.default_frequency.is_some() {
                JoinOutcome::Matched
            } else {
                JoinOutcome::MatchedIncomplete
            };
            joined.push(ClockJoinEntry {
                field_name: field.field_name.clone(),
                clock_id: field.clock_id.clone(),
                required: field.required,
                default_frequency: bp.default_frequency,
                outcome,
                decl_provenance: field.provenance.clone(),
                binding_provenance: bp.provenance.clone(),
            });
        } else {
            decl_only.push(field.clone());
        }
    }

    let binding_only: Vec<_> = binding
        .clocks
        .iter()
        .filter(|b| !matched_ids.contains(&b.join_key.value()))
        .cloned()
        .collect();

    (joined, decl_only, binding_only)
}

// ---- Reset join (field-name) ----

fn join_resets(
    decl: &DeclarationModel,
    binding: &BindingModel,
) -> (
    Vec<ResetJoinEntry>,
    Vec<crate::declaration::resources::ResetFieldDecl>,
    Vec<crate::binding::ResetBinding>,
) {
    let binding_by_name: BTreeMap<&str, &crate::binding::ResetBinding> = binding
        .resets
        .iter()
        .map(|b| (b.join_key.value(), b))
        .collect();

    let mut joined = Vec::new();
    let mut decl_only = Vec::new();
    let mut matched_names = Vec::new();

    for field in &decl.resources.reset.fields {
        if let Some(bp) = binding_by_name.get(field.field_name.as_str()) {
            matched_names.push(field.field_name.as_str());
            let has_all = bp.reset_line_id.is_some() && bp.reset_shared.is_some();
            let outcome = if has_all {
                JoinOutcome::Matched
            } else {
                JoinOutcome::MatchedIncomplete
            };
            joined.push(ResetJoinEntry {
                field_name: field.field_name.clone(),
                required: field.required,
                reset_line_id: bp.reset_line_id.clone(),
                reset_shared: bp.reset_shared,
                outcome,
                decl_provenance: field.provenance.clone(),
                binding_provenance: bp.provenance.clone(),
            });
        } else {
            decl_only.push(field.clone());
        }
    }

    let binding_only: Vec<_> = binding
        .resets
        .iter()
        .filter(|b| !matched_names.contains(&b.join_key.value()))
        .cloned()
        .collect();

    (joined, decl_only, binding_only)
}

// ---- PinCtrl join (explicit pinctrl_id or field-name) ----

fn join_pinctrl(
    decl: &DeclarationModel,
    binding: &BindingModel,
) -> (
    Vec<PinCtrlJoinEntry>,
    Vec<crate::declaration::resources::PinCtrlFieldDecl>,
    Vec<crate::binding::PinCtrlBinding>,
) {
    // Build a lookup by binding join key value.
    let binding_by_key: BTreeMap<&str, &crate::binding::PinCtrlBinding> = binding
        .pinctrl
        .iter()
        .map(|b| (b.join_key.value(), b))
        .collect();

    let mut joined = Vec::new();
    let mut decl_only = Vec::new();
    let mut matched_keys = Vec::new();

    for field in &decl.resources.pinctrl.fields {
        // Try explicit pinctrl_id first, then fall back to field_name.
        let lookup_key = field
            .pinctrl_id
            .as_deref()
            .unwrap_or(field.field_name.as_str());

        if let Some(bp) = binding_by_key.get(lookup_key) {
            matched_keys.push(bp.join_key.value());
            let outcome = if bp.pin_states.is_some() {
                JoinOutcome::Matched
            } else {
                JoinOutcome::MatchedIncomplete
            };
            joined.push(PinCtrlJoinEntry {
                field_name: field.field_name.clone(),
                join_key: bp.join_key.clone(),
                required: field.required,
                pin_states: bp.pin_states.clone(),
                outcome,
                decl_provenance: field.provenance.clone(),
                binding_provenance: bp.provenance.clone(),
            });
        } else {
            decl_only.push(field.clone());
        }
    }

    let binding_only: Vec<_> = binding
        .pinctrl
        .iter()
        .filter(|b| !matched_keys.contains(&b.join_key.value()))
        .cloned()
        .collect();

    (joined, decl_only, binding_only)
}
