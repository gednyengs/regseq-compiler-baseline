//! Portable driver lowering layer.
//!
//! Provides the backend-neutral `DriverLoweringPlan` (in `plan`),
//! the RegseqModel-native entry `build_driver_lowering_plan_from_model`
//! (in `from_model`), and reporting-only diagnostics (`diagnostics`).
//!
//! The legacy planner, body interpreter, behavior planner, hierarchy,
//! projection, MIR interpreter, and semantic lowering modules were
//! removed in Wave A of the legacy-removal refactor. The portable
//! `behavior` vocabulary went in Wave B alongside the
//! `regseq_model::semantic` substrate it re-exported `BinOp` from.
//! See `docs/discussions/models/regseq-model/legacy-removal-plan.md`
//! and `regseq-backends-audit.md`.

pub mod diagnostics;
pub mod from_model;
pub mod plan;
