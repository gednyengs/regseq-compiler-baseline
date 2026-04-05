//! Portable driver lowering layer.
//!
//! This module provides the backend-neutral `DriverLoweringPlan` and the
//! planner entry point that converts an `AnalysisInput` into a portable
//! plan. OS-specific backend planners consume the portable plan rather
//! than re-reading `AnalysisInput` ad hoc.

pub mod behavior;
pub mod behavior_planner;
pub mod body_interpreter;
pub mod plan;
pub mod planner;
