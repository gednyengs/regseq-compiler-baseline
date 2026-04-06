//! Semantic body and block types.
//!
//! `SemanticBody` is the top-level container for a function's executable
//! body in the semantic procedural IR. It carries the full local environment,
//! parameter list, return type, and the function body as a `Block`.

use serde::{Deserialize, Serialize};

use super::local::{LocalId, SemanticLocal};
use super::stmt::Stmt;
use super::types::SemanticType;

/// A function's executable body in the semantic procedural IR.
///
/// The body is populated during semantic lowering (Phase 2B). Before
/// lowering, functions carry an empty body via `SemanticBody::empty()`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticBody {
    /// All locals (params, authored locals, derived temps, return place).
    /// Pre-declared with names, types, and origins.
    pub locals: Vec<SemanticLocal>,
    /// Parameter locals (indices into `locals`), in signature order.
    /// For methods, `self` is always `params[0]`.
    pub params: Vec<LocalId>,
    /// Return type.
    pub return_type: SemanticType,
    /// Function body.
    pub body: Block,
}

impl SemanticBody {
    /// Create an empty semantic body.
    ///
    /// Returns a valid, well-typed body with no locals, no statements, and
    /// `Void` return type. Used for item hierarchy construction before
    /// semantic lowering populates bodies from MIR.
    pub fn empty() -> Self {
        Self {
            locals: Vec::new(),
            params: Vec::new(),
            return_type: SemanticType::Void,
            body: Block { stmts: Vec::new() },
        }
    }
}

/// An ordered sequence of statements.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Block {
    /// Statements in execution order.
    pub stmts: Vec<Stmt>,
}
