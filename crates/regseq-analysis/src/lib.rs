//! Static analysis pass infrastructure for the Regseq compiler.
//!
//! Provides an analysis-facing bootstrap input layer over rustc-independent
//! models from `regseq-model`, and will host validation passes over those
//! models before lowering begins.

pub mod body_readiness;
pub mod input;
pub mod readiness;

pub use body_readiness::{check_body_readiness, BodyFinding, BodyReadinessReport};
pub use input::{AnalysisInput, ResourceJoinSummary};
pub use readiness::{check_readiness, FindingSeverity, ReadinessFinding, ReadinessReport};
