//! Static analysis pass infrastructure for the Regseq compiler.
//!
//! Provides an analysis-facing bootstrap input layer over rustc-independent
//! models from `regseq-model`, and will host validation passes over those
//! models before lowering begins.

pub mod input;

pub use input::{AnalysisInput, ResourceJoinSummary};
