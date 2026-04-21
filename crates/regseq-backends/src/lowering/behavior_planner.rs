//! Behavior planner.
//!
//! Builds a `BehaviorPlan` from an `AnalysisInput` by interpreting the
//! captured MIR function bodies via the MIR interpreter.
//!
//! This module uses `AnalysisInput.mir_bodies()` exclusively. The legacy
//! HIR-based `body_interpreter` module is not used by this path.
//!
//! The transitional `regseq_analysis::legacy_input::AnalysisInput` shim
//! is deprecated at its source; `#![allow(deprecated)]` applies here
//! until `regseq-lowering-10` rewrites this path against `&RegseqModel`.
#![allow(deprecated)]

use regseq_analysis::legacy_input::AnalysisInput;
use regseq_model::declaration::callbacks::CallbackFamily;

use super::behavior::BehaviorPlan;
use super::mir_interpreter;

/// Build a `BehaviorPlan` from an `AnalysisInput`.
///
/// Uses the MIR-based interpreter as the canonical executable-body path.
pub fn build_behavior_plan(input: &AnalysisInput) -> BehaviorPlan {
    build_behavior_plan_with_constants(input, &[])
}

/// Build a `BehaviorPlan` with explicit authored constant values.
///
/// The `constant_values` map provides `(constant_path, value)` pairs
/// for resolving interrupt class mask values when the MIR body data
/// uses symbolic constant names rather than resolved scalar values.
///
/// When MIR bodies carry literal mask values as scalar constants (the
/// normal pipeline path), class definitions already have correct mask
/// values and the post-pass is a no-op.
pub fn build_behavior_plan_with_constants(
    input: &AnalysisInput,
    constant_values: &[(String, u32)],
) -> BehaviorPlan {
    validate_mir_body_coverage(input);

    // MIR is the canonical path. No legacy fallback.
    let mut plan = mir_interpreter::interpret_mir_bodies(input);
    mir_interpreter::resolve_interrupt_class_masks(&mut plan, constant_values);
    plan
}

fn validate_mir_body_coverage(input: &AnalysisInput) {
    let present_families = input.callback_families_present();
    let callback_bodies = input.mir_callback_bodies();

    for family in &present_families {
        let expected_trait = match family {
            CallbackFamily::Lifecycle => "Lifecycle",
            CallbackFamily::Serial => "Serial",
            CallbackFamily::SerialConsole => "SerialConsole",
            CallbackFamily::PowerManagement => "PowerManagement",
        };

        let has_body = callback_bodies.iter().any(|b| {
            matches!(
                &b.kind,
                regseq_model::declaration::mir_body::FnKind::Callback { trait_name, .. }
                    if trait_name == expected_trait
            )
        });

        assert!(
            has_body,
            "missing MIR callback bodies for declared callback family '{expected_trait}'"
        );
    }

    if !input
        .declaration()
        .resources
        .interrupts
        .isr_bindings
        .is_empty()
    {
        assert!(
            !input.mir_isr_bodies().is_empty(),
            "missing MIR ISR bodies for declared interrupt bindings"
        );
    }

    let reachable_helpers = input.mir_reachable_helper_names();
    for helper in reachable_helpers {
        assert!(
            input.mir_body_by_name(&helper).is_some(),
            "missing MIR body for reachable helper '{helper}'"
        );
    }
}
