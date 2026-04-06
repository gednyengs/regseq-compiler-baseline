//! Structured behavioral plan types.
//!
//! These types capture per-function authored behavior at a granularity
//! sufficient for backend emission. The backend consumes structured
//! behavioral data rather than inventing device-specific callback logic.
//!
//! All types use OS-neutral semantic vocabulary. Each OS backend maps
//! the portable behavior nodes to its own subsystem APIs.
//!
//! This is the **canonical** portable behavior representation consumed
//! by backend planners. The coarser `CallbackBodyPlan` in `plan.rs`
//! carries supplementary evidence but is not the backend-consumed
//! source of truth for behavior emission.

use regseq_model::declaration::operations::{DelayIntrinsic, DelayPathKind};

use super::plan::ResourceKind;

// ---------------------------------------------------------------------------
// Top-level behavioral plan
// ---------------------------------------------------------------------------

/// Structured behavioral plan for all authored driver functions.
///
/// Each function's behavior is scoped to an individual authored callback
/// or helper, not aggregated by callback family. This is the canonical
/// portable behavior representation that later backend work consumes for
/// code emission.
#[derive(Debug, Clone)]
pub struct BehaviorPlan {
    /// Per-function behavior entries.
    pub functions: Vec<FunctionBehavior>,
    /// Authored interrupt class definitions used in ISR discrimination.
    ///
    /// These carry the mask values from the authored spec so that the
    /// backend does not hardcode device-specific interrupt mask literals.
    pub interrupt_classes: Vec<InterruptClassDef>,
}

/// An authored interrupt class with its mask value.
///
/// Interrupt classes are derived from the authored ISR body's branch
/// conditions, not from backend-hardcoded mask literals.
#[derive(Debug, Clone)]
pub struct InterruptClassDef {
    /// Symbolic class name (e.g. "rx", "tx", "error").
    pub name: String,
    /// Bitmask value for this class.
    pub mask_value: u32,
}

// ---------------------------------------------------------------------------
// Per-function behavior
// ---------------------------------------------------------------------------

/// Behavior for a single authored function.
#[derive(Debug, Clone)]
pub struct FunctionBehavior {
    /// Function name as authored (e.g. "init", "tx_byte", "handle_irq").
    pub name: String,
    /// Behavioral role of this function.
    pub role: FunctionRole,
    /// Authored parameters (excluding `self` receiver).
    pub params: Vec<FunctionParam>,
    /// Authored return type, if non-unit.
    pub return_type: Option<PortableType>,
    /// Ordered body actions.
    pub body: Vec<BehaviorNode>,
}

/// An authored function parameter.
#[derive(Debug, Clone)]
pub struct FunctionParam {
    /// Parameter name as authored.
    pub name: String,
    /// Portable type.
    pub ty: PortableType,
}

/// Portable type representation for function signatures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PortableType {
    U8,
    U16,
    U32,
    U64,
    I8,
    I16,
    I32,
    I64,
    Bool,
    /// Fallback for unrecognized or complex types.
    Opaque(String),
}

/// Behavioral role of an authored function.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FunctionRole {
    /// Lifecycle init callback.
    LifecycleInit,
    /// Lifecycle deinit callback.
    LifecycleDeinit,
    /// Serial port open callback.
    SerialPortOpen,
    /// Serial port close callback.
    SerialPortClose,
    /// Serial configure callback.
    SerialConfigure,
    /// Serial transmit-byte fast path.
    SerialTxByte,
    /// Serial receive-character path.
    SerialRxChar,
    /// Serial transmit-ready predicate.
    SerialTxReady,
    /// Serial transmit-empty predicate.
    SerialTxEmpty,
    /// Serial RX FIFO flush.
    SerialFlushRxFifo,
    /// Serial TX FIFO flush.
    SerialFlushTxFifo,
    /// Serial break control.
    SerialSendBreak,
    /// Interrupt service routine.
    Isr,
    /// Authored helper function.
    Helper,
}

// ---------------------------------------------------------------------------
// Behavior nodes (structured body actions)
// ---------------------------------------------------------------------------

/// A single behavioral action node within a function body.
///
/// These use OS-neutral vocabulary. Backend planners map each node to
/// target-specific API calls.
#[derive(Debug, Clone)]
pub enum BehaviorNode {
    /// Call an authored helper function.
    CallHelper {
        /// Helper function name.
        callee: String,
        /// Arguments (excluding receiver).
        args: Vec<ValueExpr>,
    },

    /// Write a register with field values.
    RegWrite {
        /// Register name (e.g. "cr", "dr").
        register: String,
        /// Field-value assignments.
        fields: Vec<FieldWrite>,
    },

    /// Read-modify-write a register.
    RegModify {
        /// Register name.
        register: String,
        /// Field-value assignments to apply.
        fields: Vec<FieldWrite>,
    },

    /// Read a specific field from a register.
    RegFieldRead {
        /// Register name.
        register: String,
        /// Bitfield name within the register.
        field: String,
    },

    /// Read the raw value of a register.
    RegReadRaw {
        /// Register name.
        register: String,
    },

    /// Conditional branch.
    Branch {
        /// Condition expression.
        condition: BranchCondition,
        /// Actions when condition is true.
        then_body: Vec<BehaviorNode>,
        /// Actions when condition is false.
        else_body: Vec<BehaviorNode>,
    },

    /// While loop.
    Loop {
        /// Loop continuation condition.
        condition: BranchCondition,
        /// Loop body actions.
        body: Vec<BehaviorNode>,
    },

    /// Explicit ordered sequence derived from barrier-bracketed regions.
    ///
    /// Steps must be emitted in declaration order. Recognized from the
    /// expanded `ordered!` macro pattern (paired barrier intrinsic calls),
    /// not from a preserved source-macro marker.
    OrderedSequence {
        /// Ordered steps.
        steps: Vec<BehaviorNode>,
    },

    /// Return from function.
    Return {
        /// Semantic meaning of the return value.
        semantic: ReturnSemantic,
    },

    /// Assign to a driver field.
    FieldAssign {
        /// Target field name.
        field: String,
        /// Value to assign.
        value: AssignValue,
    },

    /// Optional resource operation (clock, reset, pinctrl).
    ResourceOp {
        /// Resource family.
        resource_kind: ResourceKind,
        /// Field name on the driver struct.
        field_name: String,
        /// Operation to perform.
        operation: ResourceOperation,
    },

    /// Timed delay.
    Delay {
        /// Delay intrinsic type.
        intrinsic: DelayIntrinsic,
        /// Path kind.
        path_kind: DelayPathKind,
    },

    /// Test interrupt status against a named class mask.
    InterruptClassTest {
        /// Variable holding the raw interrupt status.
        status_source: String,
        /// Named interrupt class to test against.
        class_name: String,
    },

    /// Clear interrupt sources matching a class mask.
    InterruptClassClear {
        /// Variable holding the raw interrupt status.
        status_source: String,
        /// Named interrupt class whose bits to clear.
        class_name: String,
        /// Register name for the write-to-clear operation.
        ///
        /// Derived from the MIR body's register write in the clear path.
        /// The backend uses this to emit the correct register define
        /// instead of hardcoding a device-specific register name.
        clear_register: String,
    },

    /// Check a register bitfield flag and accumulate into a result.
    ///
    /// Generic computation node for flag-checking patterns: read a
    /// bitfield from a register value and, if nonzero, accumulate a
    /// named flag into a result variable.
    RegisterFlagCheck {
        /// Flag name (e.g. "overrun", "parity", "timeout").
        flag_name: String,
        /// Register containing the flag bit.
        register: String,
        /// Bitfield name for the flag.
        field: String,
    },

    /// Local variable binding with a computed value.
    ///
    /// Represents `let name = value;` in the authored body. Captures
    /// intermediate computations and expressions.
    LocalBinding {
        /// Variable name.
        name: String,
        /// Whether the binding is mutable.
        mutable: bool,
        /// Computed value expression.
        value: ValueExpr,
    },

    /// Fallible operation with early-return on error.
    ///
    /// Represents the desugared `?` operator: execute the callee, and if
    /// the result is an error, execute cleanup actions and return early.
    /// Derived from `SwitchInt`-based result discrimination in captured MIR.
    FallibleOp {
        /// The inner action nodes to execute.
        body: Vec<BehaviorNode>,
        /// Cleanup actions on the error path (may be empty).
        cleanup_on_error: Vec<BehaviorNode>,
    },

    /// Cleanup region with a guaranteed cleanup path on scope exit.
    ///
    /// Represents cleanup-bearing MIR regions where the cleanup path
    /// executes regardless of whether the body succeeds or fails.
    CleanupRegion {
        /// The normal body actions.
        body: Vec<BehaviorNode>,
        /// Cleanup actions on scope exit.
        cleanup: Vec<BehaviorNode>,
    },

    /// OS-neutral subsystem-semantic action.
    ///
    /// Represents an interaction with a device-class subsystem that the
    /// backend maps to its own OS-specific API. Used instead of
    /// device-specific portable nodes.
    SubsystemAction {
        /// Subsystem name (e.g. "serial").
        subsystem: String,
        /// Action name (e.g. "push_rx_char", "signal_tx_ready").
        action: String,
    },

    /// Compound assignment operation (e.g. `|=`, `&=`).
    CompoundAssign {
        /// Target variable or field.
        target: String,
        /// Operator (e.g. "|", "&").
        op: String,
        /// Right-hand side value.
        value: ValueExpr,
    },

    /// Statement-level expression evaluation (side-effect only).
    Eval {
        /// Expression to evaluate.
        expr: ValueExpr,
    },
}

// ---------------------------------------------------------------------------
// Value expressions (for computations and bindings)
// ---------------------------------------------------------------------------

/// A value expression for use in local bindings and computed arguments.
///
/// Represents intermediate computations, casts, field reads, and
/// arithmetic that appear in authored function bodies. Backend emission
/// maps these to target-language expressions.
#[derive(Debug, Clone)]
pub enum ValueExpr {
    /// Integer literal.
    IntLit(u64),
    /// Boolean literal.
    BoolLit(bool),
    /// Reference to a function parameter.
    Param(String),
    /// Reference to a local variable.
    Local(String),
    /// Reference to a driver struct field.
    FieldRef(String),
    /// Binary operation.
    BinOp {
        /// Operator.
        op: BinOp,
        /// Left-hand side.
        lhs: Box<ValueExpr>,
        /// Right-hand side.
        rhs: Box<ValueExpr>,
    },
    /// Type cast.
    Cast {
        /// Expression to cast.
        expr: Box<ValueExpr>,
        /// Target type name.
        ty: String,
    },
    /// Method call that produces a value.
    MethodResult {
        /// Method / helper name.
        callee: String,
        /// Arguments.
        args: Vec<ValueExpr>,
    },
    /// Read a scalar value from a resource.
    ///
    /// Represents reading a property of a resource (e.g., clock frequency)
    /// that the backend maps to the appropriate OS-specific API call.
    ResourceRead {
        /// Resource family (Clock, Reset, etc.).
        resource_kind: ResourceKind,
        /// Driver field holding the resource.
        field_name: String,
        /// Property to read (e.g., "rate").
        property: String,
    },
    /// Extract a field from a raw register value.
    FieldExtract {
        /// Register name.
        register: String,
        /// Bitfield name.
        field: String,
        /// The raw value expression to extract from.
        raw_value: Box<ValueExpr>,
    },
    /// None literal.
    None,
    /// Some(inner) wrapper.
    Some(Box<ValueExpr>),
}

// BinOp is now defined in regseq-model::semantic::ops and re-exported here
// for backward compatibility with existing import sites.
pub use regseq_model::semantic::BinOp;

// ---------------------------------------------------------------------------
// Supporting types
// ---------------------------------------------------------------------------

/// A field-value assignment for a register write or modify.
#[derive(Debug, Clone)]
pub struct FieldWrite {
    /// Bitfield name.
    pub field: String,
    /// Value to write.
    pub value: FieldWriteValue,
}

/// Value for a register field write.
#[derive(Debug, Clone)]
pub enum FieldWriteValue {
    /// Named value from the bitfield enum (e.g. "ENABLED", "EIGHT_BITS").
    Named(String),
    /// Variable holding the value.
    Variable(String),
    /// Raw numeric literal.
    Raw(u32),
    /// All bits zero (clear).
    Zero,
}

/// Condition expression for branches and loops.
#[derive(Debug, Clone)]
pub enum BranchCondition {
    /// Register field equals a value.
    RegFieldEq {
        /// Register name.
        register: String,
        /// Bitfield name.
        field: String,
        /// Expected value.
        value: u32,
    },
    /// Register field is nonzero.
    RegFieldNonZero {
        /// Register name.
        register: String,
        /// Bitfield name.
        field: String,
    },
    /// Raw status variable equals zero.
    RawEqZero {
        /// Variable name.
        source: String,
    },
    /// Driver field is None.
    FieldIsNone {
        /// Field name.
        field: String,
    },
    /// Conjunction of two conditions.
    And(Box<BranchCondition>, Box<BranchCondition>),
    /// Boolean parameter to the function.
    BoolParam {
        /// Parameter name.
        name: String,
    },
    /// Helper function result used as predicate.
    HelperPredicate {
        /// Helper function name.
        callee: String,
    },
    /// Interrupt class mask is nonzero in status.
    InterruptClassActive {
        /// Variable holding interrupt status.
        status_source: String,
        /// Interrupt class name.
        class_name: String,
    },
    /// Value expression used as a boolean condition.
    ValueTest {
        /// The expression to test.
        expr: ValueExpr,
    },
}

/// Semantic meaning of a return statement.
#[derive(Debug, Clone)]
pub enum ReturnSemantic {
    /// Return Ok(()) for a Result-returning function.
    Ok,
    /// Return None (no data available).
    None,
    /// Return Some(value) with accumulated data.
    SomeValue,
    /// Return IRQ handled.
    IrqHandled,
    /// Return IRQ not handled.
    IrqNotHandled,
    /// Return a conditional result based on runtime outcome.
    ConditionalResult,
    /// Return a boolean value.
    Bool(bool),
    /// Void return (implicit).
    Void,
    /// Return a concrete value expression.
    WithValue(Box<ValueExpr>),
}

/// Value for a field assignment.
#[derive(Debug, Clone)]
pub enum AssignValue {
    /// Set to None.
    None,
    /// Wrap a byte in Some.
    SomeByte {
        /// Source variable or parameter.
        source: String,
    },
    /// Literal value.
    Literal(String),
}

/// Operation on an optional resource (clock, reset, pinctrl).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResourceOperation {
    /// Enable the resource.
    Enable,
    /// Disable the resource.
    Disable,
    /// Toggle the resource (assert then deassert).
    Toggle,
    /// Apply a named pin state.
    ApplyState {
        /// Pin state name (e.g. "default", "sleep").
        state: String,
    },
}
