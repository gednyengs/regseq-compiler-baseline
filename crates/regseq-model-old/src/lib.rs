//! Rustc-independent internal model for the Regseq compiler.
//!
//! Owns the target [`model::RegseqModel`] IR, its typed identity scheme,
//! the CFG-based body IR (`ir`), Phase-1 normalization, platform Phase-2
//! join, verification, query helpers, and JSON serialization. This crate
//! has no rustc dependency.

pub mod identity;
pub mod ir;
pub mod join;
pub mod model;
pub mod platform;
pub mod platform_parser;
pub mod properties;
pub mod query;
pub mod serialize;
pub mod verify;

pub use join::{populate_bindings, JoinError, JoinErrorKind, JoinProvenance};
