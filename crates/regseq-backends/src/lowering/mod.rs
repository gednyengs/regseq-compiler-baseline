//! Portable driver lowering layer.
//!
//! Provides the backend-neutral `DriverLoweringPlan` (in `plan`),
//! the RegseqModel-native entry `build_driver_lowering_plan_from_model`
//! (in `from_model`), and reporting-only diagnostics (`diagnostics`).

pub mod diagnostics;
pub mod from_model;
pub mod plan;
