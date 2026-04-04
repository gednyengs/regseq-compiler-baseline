//! Structured behavioral plan types.
//!
//! These types capture per-function authored behavior at a granularity
//! sufficient for backend emission. The backend consumes structured
//! behavioral data rather than inventing device-specific callback logic.
//!
//! All types use OS-neutral semantic vocabulary. Each OS backend maps
//! the portable behavior nodes to its own subsystem APIs.

use regseq_model::declaration::operations::{DelayIntrinsic, DelayPathKind};

use super::plan::ResourceKind;

// ---------------------------------------------------------------------------
// Top-level behavioral plan
// ---------------------------------------------------------------------------

/// Structured behavioral plan for all authored driver functions.
///
/// Each function's behavior is scoped to an individual authored callback
/// or helper, not aggregated by callback family.
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
/// For PL011: `rx` = `(1 << 4) | (1 << 6)`, `tx` = `1 << 5`,
/// `error` = `(1 << 7) | (1 << 8) | (1 << 9) | (1 << 10)`.
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
    /// Ordered body actions.
    pub body: Vec<BehaviorNode>,
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

    /// Explicit ordered sequence from `ordered!` blocks.
    ///
    /// Steps must be emitted in declaration order. This is a stronger
    /// constraint than incidental action order.
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
    },

    /// Check an RX error flag from a register field.
    RxErrorCheck {
        /// Error kind name (e.g. "overrun", "break", "parity", "framing").
        error_kind: String,
        /// Register containing the error bit.
        register: String,
        /// Bitfield name for the error flag.
        field: String,
    },

    /// Push a received character with error flags to the serial core.
    SerialPushRxChar,

    /// Compute baud divisor from clock and speed parameters.
    ComputeBaudDivisor,
}

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
}

/// Semantic meaning of a return statement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReturnSemantic {
    /// Return Ok(()) for a Result-returning function.
    Ok,
    /// Return None (no data available).
    None,
    /// Return Some(RxChar) with accumulated data/errors.
    SomeRxChar,
    /// Return IRQ handled.
    IrqHandled,
    /// Return IRQ not handled.
    IrqNotHandled,
    /// Return a conditional IRQ result based on service outcome.
    ConditionalIrqResult,
    /// Return a boolean value.
    Bool(bool),
    /// Void return (implicit).
    Void,
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
