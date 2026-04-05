//! Owned, rustc-independent MIR representation for serialized body capture.
//!
//! These types mirror the subset of rustc's MIR that the supported PL011
//! slice needs, with all arena-bound references resolved into owned data.
//! They are the primary body representation in the raw frontend artifact.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Top-level body surface
// ---------------------------------------------------------------------------

/// All extracted MIR function bodies for a driver crate.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MirBodySurface {
    /// Extracted function bodies in discovery order.
    pub functions: Vec<MirExtractedFn>,
}

/// A single extracted function with MIR body.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MirExtractedFn {
    /// Function name as authored.
    pub name: String,
    /// Stable def-path string for provenance.
    pub def_path: String,
    /// Function classification.
    pub kind: MirFnKind,
    /// Receiver style.
    pub receiver: String,
    /// Parameters (excluding self).
    pub params: Vec<MirFnParam>,
    /// Return type, if non-unit.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub return_type: Option<String>,
    /// The MIR body.
    pub body: MirBody,
    /// Names of helpers called from this function's MIR.
    pub helper_calls: Vec<String>,
}

/// Function classification.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type")]
pub enum MirFnKind {
    /// Trait callback method.
    Callback {
        trait_name: String,
        method_name: String,
    },
    /// ISR handler.
    Isr,
    /// Helper on the driver type.
    Helper,
    /// Free function.
    FreeFunction,
}

/// Function parameter.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MirFnParam {
    /// Parameter name.
    pub name: String,
    /// Parameter type.
    pub ty: String,
}

// ---------------------------------------------------------------------------
// MIR body core
// ---------------------------------------------------------------------------

/// Owned MIR body for a single function.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MirBody {
    /// Local variable declarations (index 0 = return place).
    pub local_decls: Vec<MirLocalDecl>,
    /// Number of function arguments (locals 1..=arg_count).
    pub arg_count: usize,
    /// Basic blocks forming the CFG.
    pub basic_blocks: Vec<MirBasicBlockData>,
    /// Source scope information for debug provenance.
    pub source_scopes: Vec<MirSourceScope>,
}

/// A local variable declaration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MirLocalDecl {
    /// Local index.
    pub index: u32,
    /// Type of this local.
    pub ty: MirTy,
    /// Mutability.
    pub mutability: MirMutability,
    /// Debug name, if available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// A basic block.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MirBasicBlockData {
    /// Statements executed in order.
    pub statements: Vec<MirStatement>,
    /// Block terminator.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terminator: Option<MirTerminator>,
    /// Whether this is a cleanup (unwind) block.
    pub is_cleanup: bool,
}

/// Source scope for debug provenance.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MirSourceScope {
    /// Scope index.
    pub index: u32,
    /// Scope name from debug info.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// Source location info attached to statements/terminators.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MirSourceInfo {
    /// Enclosing source scope index.
    pub scope: u32,
    /// Human-readable span description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub span: Option<String>,
}

/// Mutability marker.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum MirMutability {
    /// Immutable binding.
    Not,
    /// Mutable binding.
    Mut,
}

// ---------------------------------------------------------------------------
// Statements
// ---------------------------------------------------------------------------

/// A MIR statement.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MirStatement {
    /// Source location.
    pub source_info: MirSourceInfo,
    /// Statement kind.
    pub kind: MirStatementKind,
}

/// Statement kinds in the supported subset.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind")]
pub enum MirStatementKind {
    /// `place = rvalue`.
    Assign {
        place: MirPlace,
        rvalue: Box<MirRvalue>,
    },
    /// Mark local as live.
    StorageLive { local: u32 },
    /// Mark local as dead.
    StorageDead { local: u32 },
    /// Set the discriminant of an enum place.
    SetDiscriminant { place: MirPlace, variant_index: u32 },
    /// No operation.
    Nop,
    /// A statement kind outside the supported subset.
    Unsupported { description: String },
}

// ---------------------------------------------------------------------------
// Terminators
// ---------------------------------------------------------------------------

/// A MIR terminator.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MirTerminator {
    /// Source location.
    pub source_info: MirSourceInfo,
    /// Terminator kind.
    pub kind: MirTerminatorKind,
}

/// Terminator kinds in the supported subset.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind")]
pub enum MirTerminatorKind {
    /// Unconditional jump.
    Goto { target: u32 },
    /// Multi-way branch on an integer discriminant.
    SwitchInt {
        discr: MirOperand,
        targets: MirSwitchTargets,
    },
    /// Return from the function.
    Return,
    /// Unreachable code.
    Unreachable,
    /// Resume unwinding.
    UnwindResume,
    /// Terminate process during unwind.
    UnwindTerminate { reason: String },
    /// Function call.
    Call {
        func: MirOperand,
        args: Vec<MirOperand>,
        destination: MirPlace,
        target: Option<u32>,
        unwind: MirUnwindAction,
    },
    /// Drop a value.
    Drop {
        place: MirPlace,
        target: u32,
        unwind: MirUnwindAction,
        replace: bool,
    },
    /// Assertion check.
    Assert {
        cond: MirOperand,
        expected: bool,
        msg: String,
        target: u32,
        unwind: MirUnwindAction,
    },
    /// A terminator kind outside the supported subset.
    Unsupported { description: String },
}

/// Branch targets for `SwitchInt`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MirSwitchTargets {
    /// (value, target) pairs.
    pub values: Vec<u128>,
    /// Target basic blocks (last entry is the otherwise/default target).
    pub targets: Vec<u32>,
}

/// Unwind action on `Call` / `Drop` / `Assert`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "action")]
pub enum MirUnwindAction {
    /// Continue unwinding.
    Continue,
    /// Undefined behavior if reached.
    Unreachable,
    /// Terminate execution.
    Terminate { reason: String },
    /// Jump to cleanup block.
    Cleanup { target: u32 },
}

// ---------------------------------------------------------------------------
// Places
// ---------------------------------------------------------------------------

/// A memory location (local + projections).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MirPlace {
    /// Base local index.
    pub local: u32,
    /// Projection chain.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub projection: Vec<MirProjectionElem>,
}

/// A single projection step.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind")]
pub enum MirProjectionElem {
    /// Dereference (`*place`).
    Deref,
    /// Field access.
    Field { index: u32, ty: MirTy },
    /// Array/slice index by a local.
    Index { local: u32 },
    /// Constant array/slice index.
    ConstantIndex {
        offset: u64,
        min_length: u64,
        from_end: bool,
    },
    /// Subslice.
    Subslice { from: u64, to: u64, from_end: bool },
    /// Enum variant downcast.
    Downcast {
        #[serde(skip_serializing_if = "Option::is_none")]
        name: Option<String>,
        variant_index: u32,
    },
    /// Opaque type cast.
    OpaqueCast { ty: MirTy },
    /// Unsupported projection.
    Unsupported { description: String },
}

// ---------------------------------------------------------------------------
// Operands
// ---------------------------------------------------------------------------

/// An operand in a statement or terminator.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind")]
pub enum MirOperand {
    /// Copy from a place.
    Copy { place: MirPlace },
    /// Move from a place.
    Move { place: MirPlace },
    /// A compile-time constant.
    Constant { constant: MirConstant },
}

// ---------------------------------------------------------------------------
// Rvalues
// ---------------------------------------------------------------------------

/// An rvalue that produces a value.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind")]
pub enum MirRvalue {
    /// Pass operand through.
    Use { operand: MirOperand },
    /// Create a reference.
    Ref {
        borrow_kind: MirBorrowKind,
        place: MirPlace,
    },
    /// Binary operation.
    BinaryOp {
        op: MirBinOp,
        lhs: MirOperand,
        rhs: MirOperand,
    },
    /// Unary operation.
    UnaryOp { op: MirUnOp, operand: MirOperand },
    /// Read enum discriminant.
    Discriminant { place: MirPlace },
    /// Construct aggregate (struct, tuple, array, closure).
    Aggregate {
        agg_kind: MirAggregateKind,
        operands: Vec<MirOperand>,
    },
    /// Type cast.
    Cast {
        cast_kind: MirCastKind,
        operand: MirOperand,
        ty: MirTy,
    },
    /// Copy for deref coercion.
    CopyForDeref { place: MirPlace },
    /// Unsupported rvalue.
    Unsupported { description: String },
}

/// Borrow kind.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum MirBorrowKind {
    /// Shared (`&`).
    Shared,
    /// Mutable (`&mut`).
    Mut,
    /// Other borrow kind.
    Other(String),
}

/// Aggregate constructor kind.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind")]
pub enum MirAggregateKind {
    /// Array literal.
    Array { ty: MirTy },
    /// Tuple literal.
    Tuple,
    /// ADT (struct/enum) construction.
    Adt {
        path: String,
        variant_index: u32,
        args: Vec<MirTy>,
    },
    /// Closure construction.
    Closure { path: String, args: Vec<MirTy> },
    /// Unsupported aggregate kind.
    Unsupported { description: String },
}

/// Cast kind.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum MirCastKind {
    /// Integer to integer.
    IntToInt,
    /// Float to integer.
    FloatToInt,
    /// Float to float.
    FloatToFloat,
    /// Integer to float.
    IntToFloat,
    /// Pointer to pointer.
    PtrToPtr,
    /// Function pointer to pointer.
    FnPtrToPtr,
    /// Pointer coercion.
    PointerCoercion(String),
    /// Transmute.
    Transmute,
    /// Subtype coercion.
    Subtype,
    /// Other cast kind.
    Other(String),
}

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Owned type representation for MIR locals and expressions.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind")]
pub enum MirTy {
    /// `bool`.
    Bool,
    /// `char`.
    Char,
    /// Signed integer.
    Int { int_ty: MirIntTy },
    /// Unsigned integer.
    Uint { uint_ty: MirUintTy },
    /// `f32` or `f64`.
    Float { float_ty: MirFloatTy },
    /// Reference `&T` or `&mut T`.
    Ref { mutbl: bool, ty: Box<MirTy> },
    /// Raw pointer `*const T` or `*mut T`.
    RawPtr { mutbl: bool, ty: Box<MirTy> },
    /// ADT (struct/enum).
    Adt { path: String, args: Vec<MirTy> },
    /// Function definition type.
    FnDef { path: String, args: Vec<MirTy> },
    /// Tuple type.
    Tuple { elements: Vec<MirTy> },
    /// Array type.
    Array { ty: Box<MirTy>, len: u64 },
    /// Slice type.
    Slice { ty: Box<MirTy> },
    /// `str` type.
    Str,
    /// `!` never type.
    Never,
    /// Generic type parameter.
    Param { name: String },
    /// Unsupported type form.
    Unsupported { description: String },
}

/// Signed integer type.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum MirIntTy {
    Isize,
    I8,
    I16,
    I32,
    I64,
    I128,
}

/// Unsigned integer type.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum MirUintTy {
    Usize,
    U8,
    U16,
    U32,
    U64,
    U128,
}

/// Float type.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum MirFloatTy {
    F32,
    F64,
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// A constant value in MIR.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MirConstant {
    /// Type of the constant.
    pub ty: MirTy,
    /// Constant value.
    pub value: MirConstValue,
}

/// Constant value representation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind")]
pub enum MirConstValue {
    /// A scalar value (integer, bool, char).
    Scalar { bits: u128, size: u8 },
    /// A zero-sized value.
    ZeroSized,
    /// Unsupported constant form.
    Unsupported { description: String },
}

// ---------------------------------------------------------------------------
// Operators
// ---------------------------------------------------------------------------

/// Binary operator.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum MirBinOp {
    Add,
    AddUnchecked,
    AddWithOverflow,
    Sub,
    SubUnchecked,
    SubWithOverflow,
    Mul,
    MulUnchecked,
    MulWithOverflow,
    Div,
    Rem,
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    ShlUnchecked,
    Shr,
    ShrUnchecked,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    Cmp,
    Offset,
}

/// Unary operator.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum MirUnOp {
    /// Logical/bitwise NOT.
    Not,
    /// Arithmetic negation.
    Neg,
    /// Extract pointer metadata.
    PtrMetadata,
}
