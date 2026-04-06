//! Backend-facing semantic model container.
//!
//! `SemanticModel` is the backend-facing projection of the item hierarchy's
//! function bodies, referenced constants, and interrupt class definitions.
//! It replaces `BehaviorPlan` as the canonical behavioral representation
//! carried in `DriverLoweringPlan.behavior`.

use serde::{Deserialize, Serialize};

use crate::item::constant::ConstantDef;
use crate::item::function::FunctionDef;
use crate::item::interrupt::InterruptClassDef;

/// Backend-facing semantic model.
///
/// Contains exactly one driver spec's worth of semantic information:
/// per-function semantic bodies, referenced constants, and interrupt
/// class definitions.
///
/// Backends consume this through `DriverLoweringPlan.behavior`. They walk
/// `Stmt`/`Expr` trees and map semantic constructs to OS-specific API calls.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticModel {
    /// Per-function semantic bodies.
    pub functions: Vec<FunctionDef>,
    /// Constants referenced during backend planning or emission.
    pub constants: Vec<ConstantDef>,
    /// Interrupt class definitions.
    pub interrupt_classes: Vec<InterruptClassDef>,
}
