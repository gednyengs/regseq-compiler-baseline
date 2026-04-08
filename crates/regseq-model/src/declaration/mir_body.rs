//! MIR-based executable body model.
//!
//! These types represent the canonical executable-body representation in
//! the declaration model. They carry post-drop-elaboration MIR structure
//! from `mir_drops_elaborated_and_const_checked` without collapsing it into
//! device-specific, OS-specific, or backend-oriented lowering decisions.
//!
//! The types are rustc-independent owned values matching the JSON shape
//! emitted by `regseq-driver`'s MIR extraction pipeline.

use serde::{Deserialize, Serialize};

use super::provenance::Provenance;

// ---------------------------------------------------------------------------
// Top-level body declarations
// ---------------------------------------------------------------------------

/// Set of MIR-derived function body declarations.
///
/// This is the canonical executable-body path in the declaration model.
/// It carries CFG/basic-block MIR structure, not HIR expression trees.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MirBodyDeclSet {
    /// All extracted function bodies in discovery order.
    pub functions: Vec<MirFnBodyDecl>,
    /// MIR query flavor that produced the bodies.
    pub query_flavor: MirQueryFlavor,
}

/// Query flavor tag preserving how the MIR was obtained.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MirQueryFlavor {
    /// From `mir_drops_elaborated_and_const_checked`.
    DropsElaborated,
}

/// A function with its MIR body and metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MirFnBodyDecl {
    /// Function name as authored.
    pub name: String,
    /// Stable def-path for provenance.
    pub def_path: String,
    /// Function classification.
    pub kind: FnKind,
    /// Receiver style (`"ref_mut"`, `"ref"`, `"none"`).
    pub receiver: String,
    /// Parameters (excluding receiver).
    pub params: Vec<FnParamDecl>,
    /// Return type, if non-unit.
    pub return_type: Option<MirTy>,
    /// The MIR body (CFG form).
    pub body: MirBody,
    /// Names of helpers called from this function's MIR.
    pub helper_calls: Vec<String>,
    /// Monomorphization key for generic-instantiated functions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub monomorphization: Option<MonomorphizationKey>,
    /// Provenance back to the raw artifact.
    pub provenance: Provenance,
}

/// Function kind distinguishing callbacks from helpers and ISR handlers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum FnKind {
    /// A trait callback method.
    Callback {
        trait_name: String,
        method_name: String,
    },
    /// An ISR handler.
    Isr,
    /// A helper method.
    Helper,
    /// A free function.
    FreeFunction,
}

/// Function parameter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FnParamDecl {
    /// Parameter name.
    pub name: String,
    /// Parameter type.
    pub ty: MirTy,
}

impl MirFnBodyDecl {
    /// Whether this is a callback.
    pub fn is_callback(&self) -> bool {
        matches!(self.kind, FnKind::Callback { .. })
    }
    /// Whether this is an ISR handler.
    pub fn is_isr(&self) -> bool {
        matches!(self.kind, FnKind::Isr)
    }
    /// Whether this is a helper.
    pub fn is_helper(&self) -> bool {
        matches!(self.kind, FnKind::Helper)
    }
    /// Whether this is a free function.
    pub fn is_free_function(&self) -> bool {
        matches!(self.kind, FnKind::FreeFunction)
    }
}

impl MirBodyDeclSet {
    /// Return all callback function bodies.
    pub fn callbacks(&self) -> Vec<&MirFnBodyDecl> {
        self.functions.iter().filter(|f| f.is_callback()).collect()
    }
    /// Return all helper function bodies.
    pub fn helpers(&self) -> Vec<&MirFnBodyDecl> {
        self.functions.iter().filter(|f| f.is_helper()).collect()
    }
    /// Return all ISR function bodies.
    pub fn isrs(&self) -> Vec<&MirFnBodyDecl> {
        self.functions.iter().filter(|f| f.is_isr()).collect()
    }
    /// Return all free function bodies.
    pub fn free_functions(&self) -> Vec<&MirFnBodyDecl> {
        self.functions
            .iter()
            .filter(|f| f.is_free_function())
            .collect()
    }
    /// Look up a function body by name.
    pub fn by_name(&self, name: &str) -> Option<&MirFnBodyDecl> {
        self.functions.iter().find(|f| f.name == name)
    }
}

// ---------------------------------------------------------------------------
// MIR body core
// ---------------------------------------------------------------------------

/// Owned MIR body for a single function.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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

/// Source location info.
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
    Not,
    Mut,
}

// ---------------------------------------------------------------------------
// Statements
// ---------------------------------------------------------------------------

/// A MIR statement.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MirStatement {
    /// Source location.
    pub source_info: MirSourceInfo,
    /// Statement kind.
    pub kind: MirStatementKind,
}

/// Statement kinds in the supported subset.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MirTerminator {
    /// Source location.
    pub source_info: MirSourceInfo,
    /// Terminator kind.
    pub kind: MirTerminatorKind,
}

/// Terminator kinds in the supported subset.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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
    /// Discriminant values.
    #[serde(deserialize_with = "serde_u128_vec::deserialize")]
    pub values: Vec<u128>,
    /// Target basic blocks (last entry is the otherwise/default).
    pub targets: Vec<u32>,
}

/// Unwind action on `Call` / `Drop` / `Assert`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "action")]
pub enum MirUnwindAction {
    Continue,
    Unreachable,
    Terminate { reason: String },
    Cleanup { target: u32 },
}

// ---------------------------------------------------------------------------
// Places
// ---------------------------------------------------------------------------

/// A memory location (local + projections).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MirPlace {
    /// Base local index.
    pub local: u32,
    /// Projection chain.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub projection: Vec<MirProjectionElem>,
}

/// A single projection step.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind")]
pub enum MirProjectionElem {
    Deref,
    Field {
        index: u32,
        ty: MirTy,
    },
    Index {
        local: u32,
    },
    ConstantIndex {
        offset: u64,
        min_length: u64,
        from_end: bool,
    },
    Subslice {
        from: u64,
        to: u64,
        from_end: bool,
    },
    Downcast {
        #[serde(skip_serializing_if = "Option::is_none")]
        name: Option<String>,
        variant_index: u32,
    },
    OpaqueCast {
        ty: MirTy,
    },
    Unsupported {
        description: String,
    },
}

// ---------------------------------------------------------------------------
// Operands
// ---------------------------------------------------------------------------

/// An operand in a statement or terminator.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind")]
pub enum MirOperand {
    Copy { place: MirPlace },
    Move { place: MirPlace },
    Constant { constant: MirConstant },
}

// ---------------------------------------------------------------------------
// Rvalues
// ---------------------------------------------------------------------------

/// An rvalue that produces a value.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind")]
pub enum MirRvalue {
    Use {
        operand: MirOperand,
    },
    Ref {
        borrow_kind: MirBorrowKind,
        place: MirPlace,
    },
    BinaryOp {
        op: MirBinOp,
        lhs: MirOperand,
        rhs: MirOperand,
    },
    UnaryOp {
        op: MirUnOp,
        operand: MirOperand,
    },
    Discriminant {
        place: MirPlace,
    },
    Aggregate {
        agg_kind: MirAggregateKind,
        operands: Vec<MirOperand>,
    },
    Cast {
        cast_kind: MirCastKind,
        operand: MirOperand,
        ty: MirTy,
    },
    CopyForDeref {
        place: MirPlace,
    },
    Unsupported {
        description: String,
    },
}

/// Borrow kind.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum MirBorrowKind {
    Shared,
    Mut,
    Other(String),
}

/// Aggregate constructor kind.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind")]
pub enum MirAggregateKind {
    Array {
        ty: MirTy,
    },
    Tuple,
    Adt {
        path: String,
        variant_index: u32,
        args: Vec<MirTy>,
    },
    Closure {
        path: String,
        args: Vec<MirTy>,
    },
    Unsupported {
        description: String,
    },
}

/// Cast kind.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum MirCastKind {
    IntToInt,
    FloatToInt,
    FloatToFloat,
    IntToFloat,
    PtrToPtr,
    FnPtrToPtr,
    PointerCoercion(String),
    Transmute,
    Subtype,
    Other(String),
}

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Resolved, backend-friendly type representation for MIR locals and
/// expressions.
///
/// This enum carries fully resolved type schemas rather than
/// rustc-shaped `Adt`/`FnDef` wrappers. Struct and enum types include
/// their field layouts; function types include parameter and return
/// types. Recursive or mutually recursive nominal types use
/// [`NominalRef`](MirTy::NominalRef) to break infinite expansion.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind")]
pub enum MirTy {
    /// Boolean type.
    Bool,
    /// Character type.
    Char,
    /// Integer type (signed or unsigned).
    Int { signed: bool, width: IntWidth },
    /// Floating-point type.
    Float { width: FloatWidth },
    /// Reference type.
    Ref { mutbl: bool, ty: Box<MirTy> },
    /// Raw pointer type.
    RawPtr { mutbl: bool, ty: Box<MirTy> },
    /// Struct type with resolved field layout.
    Struct {
        path: String,
        fields: Vec<MirFieldTy>,
    },
    /// Enum type with resolved variant layout.
    Enum {
        path: String,
        variants: Vec<MirVariantTy>,
    },
    /// Tuple type.
    Tuple { elements: Vec<MirTy> },
    /// Fixed-size array type.
    Array { element: Box<MirTy>, len: u64 },
    /// Slice type.
    Slice { element: Box<MirTy> },
    /// Function type with resolved parameter and return types.
    Fn {
        path: String,
        params: Vec<MirTy>,
        ret: Box<MirTy>,
    },
    /// String slice type (`&str` payload).
    Str,
    /// Byte slice type (`&[u8]` payload).
    Bytes,
    /// Never type (`!`).
    Never,
    /// Zero-sized type placeholder (e.g. `PhantomData`, closures).
    ZeroSizedLike { description: String },
    /// Nominal reference to a type defined elsewhere.
    ///
    /// Used to break infinite recursion when resolving recursive or
    /// mutually recursive nominal types. The `path` is the canonical
    /// def-path of the referenced type. Consumers should treat this as
    /// an opaque nominal identity rather than attempting to inline the
    /// referenced schema.
    NominalRef { path: String },
    /// A type outside the supported subset.
    Unsupported { description: String },
}

/// Integer width (covers both signed and unsigned).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum IntWidth {
    I8,
    I16,
    I32,
    I64,
    I128,
    Isize,
    U8,
    U16,
    U32,
    U64,
    U128,
    Usize,
}

/// Floating-point width.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum FloatWidth {
    F32,
    F64,
}

/// A field within a struct type schema.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MirFieldTy {
    /// Field index within the struct.
    pub index: u32,
    /// Field name.
    pub name: String,
    /// Field type.
    pub ty: MirTy,
}

/// A variant within an enum type schema.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MirVariantTy {
    /// Variant name.
    pub name: String,
    /// Variant fields.
    pub fields: Vec<MirFieldTy>,
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// A typed constant value in MIR.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MirConstant {
    /// Type of the constant.
    pub ty: MirTy,
    /// Constant value.
    pub value: MirConstValue,
}

/// Structured constant value representation.
///
/// This enum carries resolved constant data instead of encoding
/// structured payloads into opaque strings. Struct, enum, tuple, and
/// array constants are represented with their field/element structure
/// intact. Constants that cannot be evaluated at extraction time are
/// represented as [`Unevaluated`](MirConstValue::Unevaluated).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind")]
pub enum MirConstValue {
    /// Raw bit pattern (integers, booleans encoded as 0/1, etc.).
    Bits {
        #[serde(deserialize_with = "serde_u128_scalar::deserialize")]
        value: u128,
    },
    /// Boolean constant.
    Bool { value: bool },
    /// Character constant (as Unicode scalar value).
    Char { value: u32 },
    /// String constant.
    Str { value: String },
    /// Byte string constant.
    Bytes { value: Vec<u8> },
    /// Struct constant with named fields.
    Struct { fields: Vec<ConstField> },
    /// Enum variant constant with optional fields.
    Enum {
        variant: String,
        fields: Vec<ConstField>,
    },
    /// Tuple constant.
    Tuple { elements: Vec<MirConstValue> },
    /// Array constant.
    Array { elements: Vec<MirConstValue> },
    /// A constant that could not be evaluated at extraction time.
    Unevaluated { path: String },
    /// Zero-sized constant (e.g. unit, PhantomData).
    ZeroSized,
    /// A constant outside the supported subset.
    Unsupported {
        #[serde(skip_serializing_if = "Option::is_none")]
        path: Option<String>,
        description: String,
    },
}

/// A field within a struct or enum constant.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConstField {
    /// Field index within the aggregate.
    pub index: u32,
    /// Field name, if available (None for tuple-struct fields).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Field value.
    pub value: Box<MirConstValue>,
}

// ---------------------------------------------------------------------------
// Monomorphization
// ---------------------------------------------------------------------------

/// Tracks generic-instantiation identity for monomorphized functions.
///
/// Two extracted functions may share the same `origin_path` (authored
/// generic function) but differ in their instantiated type arguments.
/// This key distinguishes them without requiring backends to perform
/// generic substitution.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MonomorphizationKey {
    /// The authored generic function path.
    pub origin_path: String,
    /// The instantiated type arguments.
    pub args: Vec<MirTy>,
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
    Not,
    Neg,
    PtrMetadata,
}

// ---------------------------------------------------------------------------
// Traversal helpers
// ---------------------------------------------------------------------------

/// Collect helper call names from MIR Call terminators.
pub fn collect_mir_call_targets(body: &MirBody) -> Vec<String> {
    let mut targets = Vec::new();
    for bb in &body.basic_blocks {
        let Some(ref term) = bb.terminator else {
            continue;
        };
        if let MirTerminatorKind::Call {
            func: MirOperand::Constant { ref constant },
            ..
        } = term.kind
        {
            if let MirTy::Fn { ref path, .. } = constant.ty {
                if let Some(short) = path.rsplit("::").next() {
                    targets.push(short.to_string());
                }
            }
        }
    }
    targets.sort();
    targets.dedup();
    targets
}

// ---------------------------------------------------------------------------
// u128 serde helpers
// ---------------------------------------------------------------------------

/// Custom deserializer for `Vec<u128>` that accepts JSON integers via u64.
///
/// Standard `serde_json` does not support u128. This deserializer reads
/// values as u64 and widens to u128, which is correct for all discriminant
/// values produced by the Regseq pipeline.
mod serde_u128_vec {
    use serde::{Deserialize, Deserializer};

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u128>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let v: Vec<u64> = Vec::deserialize(deserializer)?;
        Ok(v.into_iter().map(|x| x as u128).collect())
    }
}

/// Custom deserializer for a single `u128` that accepts JSON integers via u64.
mod serde_u128_scalar {
    use serde::{Deserialize, Deserializer};

    pub fn deserialize<'de, D>(deserializer: D) -> Result<u128, D::Error>
    where
        D: Deserializer<'de>,
    {
        let v: u64 = u64::deserialize(deserializer)?;
        Ok(v as u128)
    }
}

/// Count unsupported MIR constructs in a body.
pub fn count_unsupported(body: &MirBody) -> usize {
    let mut count = 0;
    for bb in &body.basic_blocks {
        for stmt in &bb.statements {
            if matches!(stmt.kind, MirStatementKind::Unsupported { .. }) {
                count += 1;
            }
        }
        if let Some(ref term) = bb.terminator {
            if matches!(term.kind, MirTerminatorKind::Unsupported { .. }) {
                count += 1;
            }
        }
    }
    count
}
