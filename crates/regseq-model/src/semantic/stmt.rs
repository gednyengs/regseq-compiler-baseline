//! Statement language for the semantic procedural IR.
//!
//! Statements are side-effect-producing constructs. Register writes, resource
//! operations, and control flow live here. Value-producing constructs live
//! in `Expr`.
//!
//! Design rationale: `RegWrite`, `RegModify`, `ResourceOp`, and
//! `SubsystemAction` are statement-only because they never produce values
//! in the C domain. Register reads, helper calls, and resource property
//! reads produce values and live in `Expr`.

use serde::{Deserialize, Serialize};

use super::body::Block;
use super::expr::Expr;
use super::local::LocalId;
use super::ops::{AssignTarget, BinOp, FieldWrite, ResourceOperation};
use super::refs::{DriverFieldRef, RegisterRef, ResourceRef, SubsystemActionRef};

/// A statement in the semantic procedural IR.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data")]
pub enum Stmt {
    // --- Binding and assignment ---
    /// Declare and initialize a local: `type name = expr;`
    Let { local: LocalId, value: Expr },

    /// Assign to a previously declared target.
    Assign { target: AssignTarget, value: Expr },

    /// Compound assignment (`|=`, `&=`, `+=`, etc.).
    CompoundAssign {
        target: AssignTarget,
        op: BinOp,
        value: Expr,
    },

    /// Evaluate expression for side effects only.
    Eval { expr: Expr },

    // --- Control flow ---
    /// Structured if/else.
    Branch {
        condition: Expr,
        then_body: Block,
        else_body: Block,
    },

    /// Structured while loop.
    Loop { condition: Expr, body: Block },

    /// Ordered memory region (from `ordered!` macro barrier pairs).
    OrderedRegion { body: Block },

    /// Fallible operation (desugared `?` operator).
    ///
    /// Body executes; on error, `cleanup_on_error` runs.
    /// `error_local` is the local that receives the error value.
    FallibleOp {
        body: Block,
        /// Receives the success value when the fallible computation returns
        /// `Result<T, E>` with `T != ()`. `None` means the success type is unit.
        success_local: Option<LocalId>,
        /// Local that receives the error value.
        error_local: LocalId,
        /// Cleanup block executed on error.
        cleanup_on_error: Block,
    },

    /// Return from function.
    Return { value: Option<Expr> },

    // --- Device-domain side-effect statements ---
    /// Write register fields (never produces a value).
    RegWrite {
        register: RegisterRef,
        fields: Vec<FieldWrite>,
    },

    /// Read-modify-write register fields (never produces a value).
    RegModify {
        register: RegisterRef,
        fields: Vec<FieldWrite>,
    },

    /// Resource operation: enable, disable, toggle (never produces a value).
    ResourceOp {
        resource: ResourceRef,
        operation: ResourceOperation,
    },

    /// Driver struct field assignment (`d->field = value`).
    FieldAssign { field: DriverFieldRef, value: Expr },

    /// Subsystem action from a closed, validated vocabulary.
    SubsystemAction {
        action: SubsystemActionRef,
        args: Vec<Expr>,
    },
}
