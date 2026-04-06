//! Operators, assignment targets, and supporting value types for the semantic IR.

use serde::{Deserialize, Serialize};

use super::expr::Expr;
use super::local::LocalId;
use super::refs::{DriverFieldRef, RegisterFieldRef};

/// Binary operator for semantic expressions.
///
/// Shared between the semantic IR and the portable lowering plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BinOp {
    /// Addition.
    Add,
    /// Subtraction.
    Sub,
    /// Multiplication.
    Mul,
    /// Division.
    Div,
    /// Left shift.
    Shl,
    /// Right shift.
    Shr,
    /// Bitwise AND.
    BitAnd,
    /// Bitwise OR.
    BitOr,
    /// Equality comparison.
    Eq,
    /// Inequality comparison.
    Ne,
    /// Logical AND.
    And,
    /// Logical OR.
    Or,
}

/// Unary operator for semantic expressions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum UnaryOp {
    /// Logical not.
    Not,
    /// Arithmetic negate.
    Neg,
    /// Bitwise complement.
    BitNot,
}

/// Target of an assignment statement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data")]
pub enum AssignTarget {
    /// Assign to a local variable.
    Local { local: LocalId },
    /// Assign to a driver struct field.
    DriverField { field: DriverFieldRef },
}

/// A field-value assignment for a register write or modify.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldWrite {
    /// Target register field.
    pub field: RegisterFieldRef,
    /// Value to write.
    pub value: FieldWriteValue,
}

/// Value for a register field write.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data")]
pub enum FieldWriteValue {
    /// Named register value (e.g., "WLEN_8", "FEN_SET").
    Named { name: String },
    /// Raw numeric value.
    Raw { value: u32 },
    /// Zero / clear.
    Zero,
    /// Reference to a semantic local.
    Local { local: LocalId },
    /// Computed expression.
    Computed { expr: Box<Expr> },
}

/// A typed resource property that can be read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ResourceProperty {
    /// Clock rate (Hz).
    ClockRate,
    /// Reset line state.
    ResetState,
    /// Pin controller active state.
    PinCtrlState,
}

/// Resource operations (side-effect-only).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data")]
pub enum ResourceOperation {
    /// Enable the resource.
    Enable,
    /// Disable the resource.
    Disable,
    /// Toggle the resource with optional duration.
    Toggle { duration_us: Option<u64> },
    /// Apply a pin controller state.
    ApplyState { state: PinCtrlState },
}

/// Pin controller state for `ResourceOperation::ApplyState`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PinCtrlState {
    /// Default (active) state.
    Default,
    /// Sleep (low-power) state.
    Sleep,
}
