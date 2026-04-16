//! Unified RegseqModel types.
//!
//! This module hosts the rustc-independent data model that consolidates
//! declaration, binding, and lowering facts produced by the extraction
//! pipeline. Types are added incrementally as Phase 2 of the regseq-model
//! execution plan progresses.

pub mod framework;

pub use framework::{FrameworkTypeKind, TypeDefClassification};
