//! Behavior planner.
//!
//! Builds a `BehaviorPlan` from an `AnalysisInput` by interpreting the
//! captured authored function bodies.
//!
//! When body data is available (the normal compiler path), the planner
//! delegates to `body_interpreter::interpret_bodies` which walks the
//! captured `BodyExpr` trees and produces structured behavioral plans.
//!
//! When body data is absent (legacy test fixtures), the planner produces
//! an empty behavior plan. This fallback exists only for backward
//! compatibility during the transition.

use regseq_analysis::AnalysisInput;

use super::behavior::BehaviorPlan;
use super::body_interpreter;

/// Build a `BehaviorPlan` from an `AnalysisInput`.
///
/// Prefers body-derived interpretation when captured bodies are available.
/// Returns an empty plan for legacy fixtures with no body data.
pub fn build_behavior_plan(input: &AnalysisInput) -> BehaviorPlan {
    build_behavior_plan_with_constants(input, &[])
}

/// Build a `BehaviorPlan` with explicit authored constant values.
///
/// The `constant_values` map provides `(constant_path, value)` pairs
/// for resolving interrupt class mask values when the body data uses
/// authored constant references rather than resolved literals.
pub fn build_behavior_plan_with_constants(
    input: &AnalysisInput,
    constant_values: &[(String, u32)],
) -> BehaviorPlan {
    let has_bodies = !input.bodies().functions.is_empty();

    if has_bodies {
        let mut plan = body_interpreter::interpret_bodies(input);
        body_interpreter::resolve_interrupt_class_masks(&mut plan, constant_values);
        plan
    } else {
        BehaviorPlan {
            functions: Vec::new(),
            interrupt_classes: Vec::new(),
        }
    }
}
