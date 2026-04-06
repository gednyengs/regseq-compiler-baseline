//! Expression language for the semantic procedural IR.
//!
//! All value-producing constructs are modeled as `Expr`. Register reads,
//! helper calls, resource property reads, and option-carrier operations
//! produce values and live here. Side-effect-only operations (register
//! writes, resource ops) live in `Stmt`.

use serde::{Deserialize, Serialize};

use super::local::LocalId;
use super::ops::{BinOp, ResourceProperty, UnaryOp};
use super::refs::{
    ConstantRef, DriverFieldRef, FunctionRef, InterruptClassRef, RegisterFieldRef, RegisterRef,
    ResourceRef,
};
use super::types::SemanticType;

/// A value-producing expression in the semantic IR.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data")]
pub enum Expr {
    // --- Literals ---
    /// Integer literal with explicit type.
    IntLit { value: i128, ty: SemanticType },
    /// Boolean literal.
    BoolLit { value: bool },

    // --- Variable references ---
    /// Reference to a semantic local.
    LocalRef { local: LocalId },
    /// Read a driver struct field.
    DriverFieldRead { field: DriverFieldRef },

    // --- Constant references ---
    /// Reference to a named constant with full identity.
    Constant { constant: ConstantRef },

    // --- Arithmetic and logic ---
    /// Binary operation.
    BinaryOp {
        op: BinOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
    /// Unary operation.
    UnaryOp { op: UnaryOp, operand: Box<Expr> },
    /// Type cast.
    Cast { expr: Box<Expr>, ty: SemanticType },

    // --- Register reads (value-producing) ---
    /// Read a specific field from a register.
    RegFieldRead {
        register: RegisterRef,
        field: RegisterFieldRef,
    },
    /// Read the raw value of a register.
    RegReadRaw { register: RegisterRef },
    /// Extract a field from an already-read raw value.
    FieldExtract {
        register: RegisterRef,
        field: RegisterFieldRef,
        raw_value: Box<Expr>,
    },

    // --- Value-producing calls ---
    /// Call a helper function.
    HelperCall {
        callee: FunctionRef,
        args: Vec<Expr>,
    },

    // --- Resource property access ---
    /// Read a typed property from a resource.
    ResourcePropertyRead {
        resource: ResourceRef,
        property: ResourceProperty,
    },

    /// Provide a default for an expression that may be absent
    /// (optional resource not present, option-typed helper result, etc.).
    ///
    /// Only valid for expressions with explicit optionality semantics.
    WithDefault {
        value: Box<Expr>,
        default: Box<Expr>,
    },

    // --- Interrupt-domain ---
    /// Test whether an interrupt class is active in a status word.
    InterruptClassTest {
        status: Box<Expr>,
        class_ref: InterruptClassRef,
    },

    // --- Resource presence ---
    /// Check if an optional resource is present.
    ResourcePresent { resource: ResourceRef },

    // --- Option carrier operations ---
    /// Test whether an option-typed value is `Some` (presence test).
    ///
    /// Applies to helper results, driver fields, locals of type `Option<T>`.
    IsSome { inner: Box<Expr> },
    /// Construct an `Option::None` value.
    OptionNone,
    /// Construct an `Option::Some(inner)` value.
    OptionSome { inner: Box<Expr> },
    /// Extract the inner value from an `Option` that is known to be `Some`.
    ///
    /// Semantic lowering may only introduce this under a dominating
    /// `IsSome`/MIR discriminant check. A dedicated validation pass must
    /// reject any `OptionUnwrap` not proven to be dominated by a matching
    /// presence test. Backends treat `OptionUnwrap` as already-validated
    /// and never attempt to prove or repair its safety.
    OptionUnwrap { inner: Box<Expr> },
}
