//! Portable driver lowering layer.
//!
//! Provides the backend-neutral `DriverLoweringPlan` (in `plan`),
//! the RegseqModel-native entry `build_driver_lowering_plan_from_model`
//! (in `from_model`), portable behavior types (`behavior`), and
//! reporting-only diagnostics (`diagnostics`).
//!
//! The legacy planner (`planner.rs`), body interpreter, behavior
//! planner, hierarchy, projection, MIR interpreter, and semantic
//! lowering modules were removed in Wave A of the legacy-removal
//! refactor. See
//! `docs/discussions/models/regseq-model/legacy-removal-plan.md` and
//! the audit at `regseq-backends-audit.md`.

pub mod behavior;
pub mod diagnostics;
pub mod from_model;
pub mod plan;
