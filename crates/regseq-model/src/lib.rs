//! Rustc-independent internal model for the Regseq compiler.
//!
//! Owns the target [`model::RegseqModel`] IR, its typed identity scheme,
//! the CFG-based body IR (`ir`), Phase-1 normalization, platform Phase-2
//! join, verification, query helpers, and JSON serialization. This crate
//! has no rustc dependency.
//!
//! The legacy `binding`, `joined`, `compat`, `item`, and `semantic`
//! layers were removed in Wave B of the legacy-removal refactor
//! (see `docs/discussions/models/regseq-model/legacy-removal-plan.md`).
//! The `declaration` module still hosts extraction-surface support
//! types consumed by `regseq-driver` and `construction/`; that module
//! is scoped for later removal once construction-side declarations are
//! fully retired.

pub mod declaration;
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
