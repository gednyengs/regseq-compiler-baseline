//! Semantic procedural IR for the Regseq compiler.
//!
//! This module defines the statement/expression-based IR for executable
//! function bodies, along with the explicit local environment, principled
//! type universe, and structured semantic references that eliminate
//! MIR/Rust residue from backend consumption.
//!
//! The semantic IR is part of the internal Regseq model, not a backend
//! pre-IR. It represents the compiler's canonical understanding of what
//! each function body means in Regseq's domain. Backends consume it;
//! they do not help construct it.
//!
//! Key types:
//! - [`SemanticBody`] -- function executable body container
//! - [`Block`] -- ordered statement container
//! - [`Stmt`] -- statement language (15 variants)
//! - [`Expr`] -- expression language (20 variants)
//! - [`SemanticLocal`], [`LocalId`], [`LocalOrigin`] -- local environment
//! - [`SemanticType`] -- type universe
//! - [`SemanticModel`] -- backend-facing container
//!
//! Body-level references use structured identity (`ItemId` + `CanonicalPath`)
//! rather than bare strings. See the [`refs`] module.

pub mod body;
pub mod expr;
pub mod local;
pub mod model;
pub mod ops;
pub mod refs;
pub mod stmt;
pub mod types;

// Re-export core types for convenience.
pub use body::{Block, SemanticBody};
pub use expr::Expr;
pub use local::{LocalId, LocalOrigin, SemanticLocal};
pub use model::SemanticModel;
pub use ops::{
    AssignTarget, BinOp, FieldWrite, FieldWriteValue, PinCtrlState, ResourceOperation,
    ResourceProperty, UnaryOp,
};
pub use refs::{
    ConstantRef, DriverFieldRef, FunctionRef, InterruptClassId, InterruptClassRef,
    RegisterFieldRef, RegisterRef, ResourceRef, SubsystemActionKind, SubsystemActionRef,
    SubsystemKind,
};
pub use stmt::Stmt;
pub use types::{ResourceKind, SemanticType};
