# Current RegseqModel IR (As-Implemented)

This document reconstructs the RegseqModel IR as it currently exists in
`crates/regseq-model/` and the construction surface in
`crates/regseq-driver/src/construction.rs`. It is descriptive (what is),
not prescriptive (what should be) -- the spec under
`docs/discussions/models/regseq-model/spec.md` is the target design;
this document is a faithful reading of the code.

All file:line citations refer to the working tree at the time of writing.

## 1. Crate boundary

- `regseq-model` (`crates/regseq-model/src/lib.rs`) is rustc-independent.
  It owns the IR types, the typed identity scheme, the CFG body IR, the
  Phase-1 normalization shape, the Phase-2 platform join, the verifier,
  query helpers, JSON serialization, and the platform-file parsed
  artifact. It does NOT depend on `rustc_*` crates.
- `regseq-driver` (`crates/regseq-driver/src/construction.rs`,
  `extract/`) is the rustc-linked construction layer. It walks HIR/MIR
  and produces a `RegseqModel`. Pass 1 builds the declaration skeleton;
  Pass 2 (`construction/pass2.rs`, `build_body`) translates MIR bodies.

Top-level public surface from `regseq-model::lib.rs`:

```
pub mod identity;
pub mod ir;
pub mod join;
pub mod model;
pub mod platform;
pub mod platform_parser;
pub mod properties;
pub mod query;
pub mod serialize;
pub mod verify;

pub use join::{populate_bindings, JoinError, JoinErrorKind, JoinProvenance};
```

## 2. Identity scheme

### 2.1 `DeclId` and typed wrappers

`identity/decl_id.rs`. Single flat `u32` namespace for all declarations:

```
pub struct DeclId(u32);
```

Nine typed newtype wrappers, all `(pub DeclId)`, all `Copy + Eq + Hash +
Ord + Serialize + Deserialize`, with `decl_id() -> DeclId` and
`From<DeclId>`:

| Wrapper | Identifies |
|---|---|
| `ModuleId` | A module in the tree |
| `ConstantId` | A constant or static (top-level or associated) |
| `FunctionId` | A free function, helper method, trait method, ISR handler |
| `TypeDefId` | A struct or enum (user-defined, abstraction, framework) |
| `RegisterDescriptorId` | A register bitfield descriptor |
| `RegisterBlockId` | A register block struct |
| `ResourceId` | A resource field on the driver struct |
| `PropertyId` | A property field on the driver struct |
| `UserDataId` | A user data field on the driver struct |

`DeclId` serializes as a plain integer.

### 2.2 Body-level IDs

```
pub struct LocalId(pub u32);
pub struct BasicBlockId(pub u32);
```

Both are raw `u32`-backed, distinct from `DeclId`, scoped to a single
function body. `BasicBlockId(0)` is the entry block.

### 2.3 Symbols and file paths

`identity/symbol.rs`.

```
pub struct Symbol(u32);
pub struct FileId(u32);
pub struct SymbolInterner { strings: Vec<String>, lookup: HashMap<String, u32> }
```

Single shared pool. `intern(&str) -> Symbol` and `intern_file(&str) ->
FileId` both deduplicate. `Symbol` and `FileId` are 4-byte `Copy`
handles with O(1) equality. Iteration order is insertion order via
`SymbolInterner::strings()`.

Serialization is dual-mode: when the per-thread serialization interner
is installed via `install_interner_for_serialization(&interner) ->
InternerGuard`, `Symbol` and `FileId` emit their resolved strings; when
no guard is active they fall back to raw `u32` indices (used by in-tree
round-trip tests). The guard is non-`Send`. `RegseqModel`'s own
serializer (`serialize::serialize_regseq_model`) installs the model's
own interner.

### 2.4 Source locations

`identity/source.rs`.

```
pub struct SourceLocation {
    pub file: FileId,
    pub start_line: u32,
    pub start_column: u32,
    pub end_line: u32,
    pub end_column: u32,
}

#[serde(tag = "kind", content = "data")]
pub enum SourceOrigin {
    Source(SourceLocation),
    CompilerGenerated,
}
```

1-based line/column numbers.

## 3. RegseqModel root

`model/regseq_model.rs`:

```
#[derive(Clone, Debug, Serialize)]
pub struct RegseqModel {
    #[serde(skip_serializing)]
    pub symbols: SymbolInterner,
    pub source_locations: BTreeMap<DeclId, SourceOrigin>,
    pub root_module: Module,
    pub format_version: u32,
}
```

Notes:

- `symbols` is skipped during serialization; its strings are emitted
  inline through `Symbol`/`FileId` resolution.
- `source_locations` is a `BTreeMap` (not a `Vec<SourceOrigin>` indexed
  by `DeclId`) so iteration is deterministic without requiring dense
  contiguous `DeclId` allocation.
- `root_module` is always `ModuleKind::Normal` (the crate module). The
  driver module is a submodule.
- `RegseqModel` does not derive `Eq`/`PartialEq` because
  `SymbolInterner` compares by identity, not content.
- `format_version` exists for serialization compatibility.

Two construction paths:

- `begin_mutation(MutationPhase) -> MutationGuard<'_>` -- the
  guarded, verifier-gated public path.
- `from_verified_components_unchecked(...)` -- `pub(crate)`,
  `#[doc(hidden)]` escape hatch for internal infrastructure
  (e.g., deserialization). Caller is responsible for running the
  verifier.

## 4. Module tree

`model/module.rs`:

```
#[serde(tag = "kind", content = "data")]
pub enum ModuleKind { Driver, Normal }

#[derive(Clone, Eq, PartialEq, Debug, Serialize)]
pub struct Module {
    pub id: ModuleId,
    pub name: Symbol,
    pub kind: ModuleKind,

    // Items (Section 3 in spec)
    pub submodules: Vec<Module>,
    pub constants: Vec<Constant>,
    pub free_functions: Vec<Function>,
    pub type_defs: Vec<TypeDef>,
    pub register_descriptors: Vec<RegisterDescriptor>,
    pub register_blocks: Vec<RegisterBlock>,
    pub impl_blocks: Vec<ImplBlock>,

    // Driver-only items (present when kind == Driver)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub driver: Option<DriverStruct>,
    pub trait_impls: Vec<ImplBlock>,
    pub driver_impl_blocks: Vec<ImplBlock>,

    // Dependency sets
    pub abstractions_used: BTreeSet<RegseqAbstraction>,
    pub intrinsics_used: BTreeSet<RegseqIntrinsic>,
}
```

Invariants and conventions:

- Exactly one `ModuleKind::Driver` module exists in the tree (verifier-
  enforced: `MissingDriverModule` / `MultipleDriverModules`).
- No two items in the same module category share a name (verifier-
  enforced: `DuplicateItemName`).
- By construction-time convention, `driver`, `trait_impls`, and
  `driver_impl_blocks` are populated only on the Driver module --
  Pass 1 only writes them inside the driver module. The current
  verifier does NOT separately reject a Normal module that carries
  one of these populated; an exactly-one-Driver model is the
  vehicle that makes the invariant hold in practice.
- `abstractions_used` / `intrinsics_used` are `BTreeSet`s for
  deterministic iteration.

## 5. Module items

### 5.1 Constants

`model/constant.rs`:

```
pub struct Constant {
    pub id: ConstantId,
    pub name: Symbol,
    pub ty: RegseqType,
    pub value: LiteralValue,
}
```

Used both for top-level `const`/`static` and for associated constants
inside `ImplBlock::associated_constants`. Non-literal initializers are
not supported.

### 5.2 Type definitions

`model/type_def.rs` and `model/framework.rs`:

```
pub struct TypeDef {
    pub id: TypeDefId,
    pub name: Symbol,
    pub kind: TypeDefKind,
    pub classification: TypeDefClassification,
    pub attrs: Attrs,
}

#[serde(tag = "kind", content = "data")]
pub enum TypeDefKind {
    Struct { fields: Vec<FieldDef> },
    Enum { variants: Vec<VariantDef> },
}

pub struct FieldDef { pub name: Symbol, pub ty: RegseqType }

pub struct VariantDef {
    pub name: Symbol,
    pub fields: Vec<FieldDef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub discriminant: Option<LiteralValue>,
}
```

Classification (`model/framework.rs`):

```
#[serde(tag = "kind", content = "data")]
pub enum TypeDefClassification {
    UserDefined,
    Abstraction(RegseqAbstraction),
    Framework(FrameworkTypeKind),
}

#[serde(tag = "kind", content = "data")]
pub enum FrameworkTypeKind {
    RegseqError,
    SerialErrors,
    RxChar,
    SerialConfig,
    DataBits,
    Parity,
    StopBits,
    FlowControl,
    Duration,
    IrqResult,
}
```

Generic attribute bag (`type_def.rs`):

```
pub struct Attrs { pub entries: Vec<AttrEntry> }

pub struct AttrEntry { pub key: String, pub value: AttrValue }

#[serde(tag = "kind", content = "data")]
pub enum AttrValue {
    String(String),
    Bool(bool),
    Int(i64),
    Unit,
    KeyValues(Vec<AttrEntry>),
}
```

`Attrs` is used by `TypeDef` and `Function` (via `Function.attrs`) to
carry extensible semantic annotations from `#[rseq::attrs(..)]` and
similar.

### 5.3 Register descriptors

`model/register.rs`:

```
pub struct RegisterDescriptor {
    pub id: RegisterDescriptorId,
    pub name: Symbol,
    pub access_width: u32,
    pub fields: Vec<BitfieldDef>,
}

pub struct BitfieldDef {
    pub name: Symbol,
    pub offset: u32,
    pub width: u32,
    pub named_values: Vec<NamedValue>,
}

pub struct NamedValue { pub name: Symbol, pub value: u64 }
```

### 5.4 Register blocks

```
pub struct RegisterBlock {
    pub id: RegisterBlockId,
    pub name: Symbol,
    pub block_attributes: RegisterBlockAttributes,
    pub fields: Vec<RegisterBlockField>,
}

pub struct RegisterBlockAttributes {
    pub endian: Symbol,
    pub access_width: u32,
}

pub struct RegisterBlockField {
    pub name: Symbol,
    pub access_kind: AccessKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub descriptor: Option<RegisterDescriptorId>,
    pub byte_offset: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub array_count: Option<usize>,
}

#[serde(tag = "kind", content = "data")]
pub enum AccessKind { ReadWrite, ReadOnly, WriteOnly, Reserved }
```

`Reserved` fields carry `descriptor: None`. `array_count` is `Some` for
reserved gaps expressed as arrays. Block <-> descriptor references
form a DAG (one descriptor can be referenced by multiple blocks).

A typed wrapper-method enum is exposed:

```
pub enum WrapperMethod { ReadRaw, Read, Write, Modify, Set }
```

with `from_raw(&str) -> Option<Self>` and `as_str(&self) -> &'static
str`.

### 5.5 Impl blocks

`model/impl_block.rs`. A single type for four use sites (inherent on
non-driver, trait on non-driver, Regseq trait on driver, driver impl):

```
pub struct ImplBlock {
    pub target_type: TypeDefId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trait_name: Option<Symbol>,
    pub methods: Vec<Function>,
    pub associated_constants: Vec<Constant>,
}
```

The structural position in `Module` (`impl_blocks` vs `trait_impls` vs
`driver_impl_blocks`) distinguishes the use site.

## 6. Driver struct

`model/driver.rs`:

```
pub struct DriverStruct {
    pub type_id: TypeDefId,
    pub name: Symbol,
    pub identity: DriverIdentity,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device_class: Option<DeviceClass>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pm_info: Option<PmInfo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub platform_metadata: Option<PlatformMetadata>,
    pub clocks: Vec<ClockResource>,
    pub resets: Vec<ResetResource>,
    pub pinctrl: Vec<PinCtrlResource>,
    pub interrupts: Vec<InterruptResource>,
    pub mmio: Vec<MmioResource>,
    pub sync: Vec<SyncResource>,
    pub properties: Vec<PropertyField>,
    pub user_data: Vec<UserDataField>,
}
```

Driver-level metadata:

```
pub struct DriverIdentity {
    pub name: Symbol,
    pub compatible: Vec<Symbol>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bus: Option<Symbol>,
}

pub struct DeviceClass { pub class: Symbol }
pub enum DeviceClassKind { SerialUart, Network, Other(String) }
pub enum BusKind { Integrated, Amba, Pci }

pub struct PmInfo { pub idle: bool, pub wakeup_capable: bool }

pub struct PlatformMetadata {
    pub os: String,
    pub board: String,
    pub arch: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bus: Option<PlatformBusBinding>,
}

pub struct PlatformBusBinding {
    pub bus_type: String,
    pub values: BTreeMap<String, PlatformValue>,
}
```

`platform_metadata` is `None` post-Phase-1 and populated by Phase 2.
`DeviceClassKind::from_raw` and `BusKind::from_raw`/`as_str` provide
typed parsing helpers.

## 7. Resources, properties, user data

`model/resources.rs`. Each resource kind has a Decl + optional Binding
pair; `binding` is `None` post-Phase-1 and populated at Phase-2 join.

### 7.1 Shared shape and resource family

```
#[serde(tag = "kind", content = "data")]
pub enum Shape { Single, Optional, Array(usize) }

pub enum ResourceKind {
    Mmio, Interrupt, Clock, Reset, PinCtrl, Property,
}
```

`Shape` corresponds to `T`, `Option<T>`, `[T; N]` in source.

### 7.2 Per-resource types

| Resource | Decl fields | Binding fields |
|---|---|---|
| `ClockResource` | `id: ResourceId`, `field_name: Symbol`, `shape: Shape`, `decl: ClockDecl{ clock_id: Symbol }` | `ClockBinding{ default_frequency: u64 }` |
| `ResetResource` | same shape, `decl: ResetDecl{ reset_id: Symbol }` | `ResetBinding{ reset_line_id: String, reset_shared: bool }` |
| `PinCtrlResource` | `decl: PinCtrlDecl{ pinctrl_id: Symbol }` | `PinCtrlBinding{ pin_states: Vec<String> }` |
| `InterruptResource` | `decl: InterruptDecl{ interrupt_id: Symbol }` | `InterruptBinding{ irq_number: u32, trigger_type: Option<String>, trigger_polarity: Option<String>, shared: bool, isr_target: Option<FunctionId> }` |
| `MmioResource` | `decl: MmioDecl{ mmio_id: Symbol, register_block: RegisterBlockId }` | `MmioBinding{ base: u64, size: u64 }` |

`InterruptBinding::isr_target` is the `FunctionId` of the bound ISR
(populated at Phase 2). `None` for unbound or polled interrupts.

### 7.3 Synchronization primitives

```
#[serde(tag = "kind", content = "data")]
pub enum SyncKind { Mutex, SpinLock, IrqSpinLock, Completion }

pub enum SyncFieldKind { Mutex, SpinLock, IrqSpinLock, Completion }

pub struct SyncResource {
    pub id: ResourceId,
    pub field_name: Symbol,
    pub sync_kind: SyncKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payload_type: Option<RegseqType>,
}
```

No `Shape` (always `Single`), no binding (purely spec-side).
`payload_type` is `None` for `Completion`. `SyncFieldKind` is the typed
analysis-side counterpart (with `from_raw`/`as_str`).

### 7.4 Properties

```
pub struct PropertyField {
    pub id: PropertyId,
    pub field_name: Symbol,
    pub field_type: ScalarType,
    pub property_id: Symbol,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_value: Option<LiteralValue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vendor_prefix: Option<Symbol>,
    pub required: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bound_value: Option<LiteralValue>,
}
```

`field_type` is restricted to `ScalarType`. `required = true` if the
spec-side type is `T`, `false` if `Option<T>`. `bound_value` is `None`
post-Phase-1 and populated at Phase 2 from `[properties]`.

### 7.5 User data

```
pub struct UserDataField {
    pub id: UserDataId,
    pub field_name: Symbol,
    pub kind: UserDataKind,
}

#[serde(tag = "kind", content = "data")]
pub enum UserDataKind {
    Option { inner_type: RegseqType },     // Rule 2: Option<T: Copy>
    ZeroInit { inner_type: RegseqType },   // Rule 1: zero-init whitelist
}
```

Rule 1 (`ZeroInit`) admits `u8`, `u16`, `u32`, `u64`, `bool`, arrays of
those, and `Copy` structs recursively composed of accepted fields. Rule
2 (`Option`) admits `Option<T>` where `T: Copy`. Construction-time
rejection is in `ConstructionError::InvalidUserDataShape /
UserDataNotCopy / UserDataNotZeroInitWhitelisted`.

## 8. Function

`model/function.rs`:

```
pub struct Function {
    pub id: FunctionId,
    pub name: Symbol,
    pub params: Vec<Param>,
    pub return_type: RegseqType,
    pub body: Body,
    pub attrs: Attrs,
}

pub struct Param {
    pub kind: ParamKind,
    pub name: Symbol,
    pub ty: RegseqType,
}

#[serde(tag = "kind", content = "data")]
pub enum ParamKind { Receiver, Regular }
```

A function is a method iff `params[0].kind == Receiver`. The receiver's
`ty` carries the target type and mutability (typically
`Ref(Defined(target_type), Mutability)`). Domain classification (trait
method, ISR handler, etc.) comes from structural position plus
`attrs` markers.

`Function.body` is mandatory (always present; no `Option<Body>`). The
doc comment in `function.rs:24-25` claims "functions are only extracted
if they have MIR bodies." Pass-1 placeholder bodies (a single block
whose only terminator is `TerminatorKind::Unreachable`, no statements,
constructed by `construction/walk.rs:empty_body`) are emitted for
cross-crate generic callees that the current `DefId`-keyed extraction
cannot monomorphize. The verifier's `is_placeholder_body` predicate
(`verify/rules/module.rs:208`) detects exactly this shape and rejects
it as `DeclarationFieldUnpopulated{ field: "body" }` -- this is the
body-completeness gap that
`docs/discussions/compiler-refactoring/model-refactor.md` targets.

## 9. CFG body IR

`ir/body.rs`:

```
pub struct Body {
    pub local_decls: Vec<LocalDecl>,
    pub arg_count: usize,
    pub basic_blocks: Vec<BasicBlock>,
    pub source_map: BodySourceMap,
    pub ordered_regions: Vec<OrderedRegion>,
}

pub struct LocalDecl {
    pub ty: RegseqType,
    pub mutable: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<Symbol>,
}

pub struct BasicBlock {
    pub statements: Vec<Statement>,
    pub terminator: Terminator,
}

pub struct OrderedRegion {
    pub entry: BasicBlockId,
    pub exit: BasicBlockId,
}

pub struct BodySourceMap {
    pub body_span: SourceOrigin,
    pub locals: Vec<SourceOrigin>,           // parallel to local_decls
    pub blocks: Vec<BasicBlockSourceMap>,    // parallel to basic_blocks
}

pub struct BasicBlockSourceMap {
    pub statements: Vec<SourceOrigin>,       // parallel to BasicBlock.statements
    pub terminator: SourceOrigin,
}
```

Conventions:

- `local_decls[0]` is the return place (`_0`). `local_decls[1..=arg_count]`
  are arguments. The rest are user variables and compiler temporaries.
- `basic_blocks[0]` is the entry block.
- Source map vectors are structurally parallel to the body content.
- `ordered_regions` records SESE regions originating from the
  `ordered!` macro. Region membership is reachability from `entry`
  without passing through `exit`.
- `Body::predecessors() -> BTreeMap<BasicBlockId, Vec<BasicBlockId>>`
  computes the predecessor map across all terminator edge kinds.

## 10. Type system

`ir/types.rs`:

```
#[serde(tag = "kind", content = "data")]
pub enum RegseqType {
    Scalar(ScalarType),
    Bool,
    Unit,
    Never,
    Option(Box<RegseqType>),
    Result(Box<RegseqType>, Box<RegseqType>),
    Array(Box<RegseqType>, usize),
    Slice(Box<RegseqType>),
    Ref(Box<RegseqType>, Mutability),
    Defined(TypeDefId),
}

#[serde(tag = "kind", content = "data")]
pub enum ScalarType { U8, U16, U32, U64, I8, I16, I32, I64, Usize, Isize }

#[serde(tag = "kind", content = "data")]
pub enum Mutability { Shared, Mutable }
```

Notes:

- All types are concrete (monomorphized in principle). No lifetimes,
  trait bounds, or generics.
- `Option`/`Result` are explicit constructors (parametric) rather than
  `Defined(...)`.
- `Defined(TypeDefId)` covers user-defined, abstraction, and framework
  types alike; the classification on the referenced `TypeDef`
  distinguishes them.
- `Mutability` applies to references only. Local-binding mutability
  lives on `LocalDecl::mutable`, not on the type.

## 11. Place and projection

`ir/place.rs`:

```
pub struct Place {
    pub local: LocalId,
    pub projection: Vec<Projection>,
}

#[serde(tag = "kind", content = "data")]
pub enum Projection {
    Deref,
    Field {
        index: u32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        name: Option<Symbol>,
        ty: RegseqType,
    },
    Index(LocalId),
    ConstantIndex { offset: u64, min_length: u64, from_end: bool },
    Subslice { from: u64, to: u64, from_end: bool },
    Downcast {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        name: Option<Symbol>,
        variant_index: u32,
    },
}
```

`Field` carries the field type inline so backends and diagnostics can
read it without a lookup.

## 12. Operand and literal

`ir/operand.rs`:

```
#[serde(tag = "kind", content = "data")]
pub enum Operand {
    Copy(Place),
    Move(Place),
    Constant(ConstantId),
    Literal(LiteralValue),
}

#[serde(tag = "kind", content = "data")]
pub enum LiteralValue {
    U8(u8), U16(u16), U32(u32), U64(u64), Usize(u64),
    I8(i8), I16(i16), I32(i32), I64(i64), Isize(i64),
    Bool(bool),
}
```

`Usize`/`Isize` use 64-bit backing storage; the actual width is
target-dependent and resolved during backend lowering.

## 13. Rvalue and operator vocabulary

`ir/rvalue.rs`:

```
#[serde(tag = "kind", content = "data")]
pub enum Rvalue {
    Use(Operand),
    Repeat(Operand, usize),
    Ref(BorrowKind, Place),
    Cast(CastKind, Operand, RegseqType),
    BinaryOp(BinOp, Operand, Operand),
    UnaryOp(UnOp, Operand),
    Discriminant(Place),
    Aggregate(AggregateKind, Vec<Operand>),
}

#[serde(tag = "kind", content = "data")]
pub enum BorrowKind { Shared, Mutable }

#[serde(tag = "kind", content = "data")]
pub enum CastKind {
    IntToInt, IntToFloat, FloatToInt, FloatToFloat, PtrToPtr, Transmute,
}

#[serde(tag = "kind", content = "data")]
pub enum AggregateKind {
    Array(RegseqType),
    Tuple,
    Adt { type_id: TypeDefId, variant_index: u32 },
}
```

Operators (`ir/ops.rs`, both `#[serde(tag = "kind", content = "data")]`):

```
pub enum BinOp {
    // arithmetic (wrapping)
    Add, Sub, Mul, Div, Rem,
    // arithmetic (UB on overflow)
    AddUnchecked, SubUnchecked, MulUnchecked,
    // arithmetic returning (result, overflow)
    AddWithOverflow, SubWithOverflow, MulWithOverflow,
    // bitwise
    BitXor, BitAnd, BitOr,
    // shifts
    Shl, Shr, ShlUnchecked, ShrUnchecked,
    // comparison
    Eq, Ne, Lt, Le, Gt, Ge,
    // three-way
    Cmp,
    // pointer arithmetic
    Offset,
}

pub enum UnOp { Not, Neg }
```

26 BinOp variants, 2 UnOp variants. No domain operations -- those flow
through `IntrinsicCall` terminators (Section 15).

## 14. Statement

`ir/statement.rs`:

```
pub struct Statement { pub kind: StatementKind }

#[serde(tag = "kind", content = "data")]
pub enum StatementKind {
    Assign(Place, Rvalue),
    SetDiscriminant { place: Place, variant_index: u32 },
    StorageLive(LocalId),
    StorageDead(LocalId),
    MirIntrinsic(MirIntrinsic),
}

#[serde(tag = "kind", content = "data")]
pub enum MirIntrinsic {
    Assume(Operand),
    CopyNonOverlapping { src: Operand, dst: Operand, count: Operand },
}
```

`MirIntrinsic` is the non-diverging MIR-level intrinsic family,
distinct from `RegseqIntrinsic` (which is domain-specific and flows
through terminators).

## 15. Terminator

`ir/terminator.rs`:

```
pub struct Terminator { pub kind: TerminatorKind }

#[serde(tag = "kind", content = "data")]
pub enum TerminatorKind {
    Goto(BasicBlockId),
    SwitchInt {
        discriminant: Operand,
        targets: SwitchTargets,
    },
    Call {
        callee: FunctionId,
        args: Vec<Operand>,
        destination: Place,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        target: Option<BasicBlockId>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        error_target: Option<BasicBlockId>,
    },
    IntrinsicCall {
        intrinsic: RegseqIntrinsic,
        args: Vec<Operand>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        metadata: Option<IntrinsicMetadata>,
        destination: Place,
        target: BasicBlockId,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        error_target: Option<BasicBlockId>,
    },
    Drop { place: Place, target: BasicBlockId },
    Return,
    Unreachable,
}

pub struct SwitchTargets {
    pub values: Vec<u128>,
    pub targets: Vec<BasicBlockId>,    // length = values.len() + 1; last is default
}
```

Key shapes:

- `Call::callee` is a typed `FunctionId`. Indirect / `dyn` calls have
  no representation -- they error during construction.
- `IntrinsicCall::target` is mandatory (intrinsics always return);
  `error_target` is `Some` for fallible intrinsics (those returning
  `Result<T, RegseqError>`) and `None` otherwise.
- `Drop` is a single-target terminator (no unwind edge in the model).

## 16. Intrinsics and abstractions

`ir/intrinsic.rs`:

```
#[serde(tag = "kind", content = "data")]
pub enum RegseqIntrinsic {
    // Register access (transport-tagged via canonical name)
    RegisterRead, RegisterReadField, RegisterWrite, RegisterWriteField,
    // Clock lifecycle
    ClockEnable, ClockDisable, ClockGetRate,
    // Reset lifecycle
    ResetAssert, ResetDeassert, ResetToggle,
    // PinCtrl
    PinCtrlApply,
    // Timing
    BusyWait, Sleep,
    // Logging
    LogError, LogWarn, LogInfo, LogDebug,
    // Polling
    PollUntil,
}
```

18 variants. `canonical_name(self) -> &'static str` and
`parse_canonical_intrinsic_name(name: &str) -> Option<RegseqIntrinsic>`
form an inverse pair. The current canonical names (per `canonical_name`):

| Variant | Canonical name |
|---|---|
| `RegisterRead` | `mmio_read` |
| `RegisterReadField` | `mmio_read_field` |
| `RegisterWrite` | `mmio_write` |
| `RegisterWriteField` | `mmio_write_field` |
| `ClockEnable` | `clock_enable` |
| `ClockDisable` | `clock_disable` |
| `ClockGetRate` | `clock_get_rate` |
| `ResetAssert` | `reset_assert` |
| `ResetDeassert` | `reset_deassert` |
| `ResetToggle` | `reset_toggle` |
| `PinCtrlApply` | `pinctrl_apply` |
| `BusyWait` | `busy_wait` |
| `Sleep` | `sleep` |
| `LogError` | `log_error` |
| `LogWarn` | `log_warn` |
| `LogInfo` | `log_info` |
| `LogDebug` | `log_debug` |
| `PollUntil` | `poll_until` |

```
#[serde(tag = "kind", content = "data")]
pub enum RegseqAbstraction {
    Clock, Reset, PinCtrl, Interrupt, Mmio,
    Mutex, SpinLock, IrqSpinLock, Completion,
}
```

Structured per-call metadata:

```
#[serde(tag = "kind", content = "data")]
pub enum IntrinsicMetadata {
    Log {
        template: String,
        placeholders: Vec<FormatPlaceholder>,
    },
    Register {
        register_block: RegisterBlockId,
        register_offset: u32,
        access_width: u32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        field: Option<RegisterFieldMeta>,
    },
    PinCtrl { state_id: Symbol },
}

pub struct FormatPlaceholder { pub format: FormatKind }

#[serde(tag = "kind", content = "data")]
pub enum FormatKind {
    Display, LowerHex, UpperHex, LowerHexPrefixed, UpperHexPrefixed,
    PaddedHex { prefix: bool, uppercase: bool, width: u8 },
    Pointer,
}

pub struct RegisterFieldMeta {
    pub mask: u64,
    pub shift: u32,
    pub name: Symbol,
}
```

Per-module summary sets `Module::abstractions_used` (`BTreeSet<
RegseqAbstraction>`) and `Module::intrinsics_used` (`BTreeSet<
RegseqIntrinsic>`) are derived after body construction by
`construction::derive_module_dependencies`
(`crates/regseq-driver/src/construction/module_deps.rs`).

## 17. Callback families

`model/callback.rs`:

```
pub enum CallbackFamily {
    Lifecycle, Serial, SerialConsole, PowerManagement,
}

pub enum CallbackContext {
    Framework, SubsystemSerialized, HandlerManaged,
}
```

Both have `as_str()`; `CallbackContext::from_raw` accepts both the
short label (`"F"`/`"S"`/`"H"`) and the long form
(`"framework"`/`"subsystem_serialized"`/`"handler_managed"`).

These are query/analysis/backend-side tags. They are not embedded
directly inside `Function`/`ImplBlock` -- callers infer them from
structural position and `Attrs`.

## 18. Provenance

`model/provenance.rs`:

```
pub struct Provenance {
    pub source_file: Option<String>,
    pub containing_item: Option<String>,
    pub family: String,
}
```

A small derived-projection record used by analyses and backends to
explain where a lowering or analysis obligation came from. Distinct
from `SourceOrigin` (which is the IR's primary source mapping).

## 19. Mutation API

`model/mutation.rs`:

```
pub enum MutationPhase { Phase1, Phase2 }

pub enum ResourceBinding {
    Clock(ClockBinding), Reset(ResetBinding), PinCtrl(PinCtrlBinding),
    Interrupt(InterruptBinding), Mmio(MmioBinding),
}

#[must_use = "MutationGuard must be committed or dropped to apply/discard mutations"]
pub struct MutationGuard<'m> { /* model: &'m mut RegseqModel, snapshot, phase */ }
```

Contract:

- `RegseqModel::begin_mutation(MutationPhase) -> MutationGuard<'_>`
  clones the model as a rollback snapshot.
- The guard exposes typed `modify_*` methods that hand out `&mut`
  access to specific submodel pieces:
  - `modify_module(ModuleId, F: FnOnce(&mut Module))`
  - `modify_driver_struct(F: FnOnce(&mut DriverStruct))`
  - `modify_body(FunctionId, F: FnOnce(&mut Body))`
  - `insert_binding(DeclId, ResourceBinding)`
  - `insert_property_value(DeclId, LiteralValue)`
- `commit(self) -> Result<VerifyReport, VerifierError>` runs the
  selected phase verifier; on `Ok` the snapshot is dropped and lint
  findings flow through; on `Err` the model is rolled back.
- `Drop` rolls back if `commit` was not called, so panics or early
  returns cannot leave the model in an unverified state.

## 20. Verification

`verify/`. Two phase entry points:

```
pub fn verify_phase_1(model: &RegseqModel) -> Result<VerifyReport, VerifierError>;
pub fn verify_phase_2(model: &RegseqModel) -> Result<VerifyReport, VerifierError>;
```

Both are fail-fast on structural errors and append non-fatal lint
findings on the success path.

### 20.1 Error taxonomy

```
#[serde(tag = "kind", content = "data")]
pub enum VerifierLevel { Module, DriverStruct, Body, CrossItem }

pub struct VerifierError {
    pub level: VerifierLevel,
    pub kind: VerifierErrorKind,
    pub provenance: VerifierProvenance,
    pub details: String,
}

pub struct VerifierProvenance {
    pub item_path: String,
    pub origin: SourceOrigin,
}
```

`VerifierErrorKind` variants (`verify/mod.rs`), grouped by bucket:

- Module level: `MissingDriverModule`, `MultipleDriverModules{count}`,
  `DuplicateItemName{module, name}`.
- Cross-item level: `UnresolvedTypeDefId{referencing, target}`,
  `UnresolvedFunctionId{...}`, `UnresolvedRegisterBlockId{...}`,
  `UnresolvedRegisterDescriptorId{...}`, `MissingSourceLocation{decl_id}`.
- Phase-1 boundary leakage: `UnexpectedBindingPopulated{resource, slot}`,
  `UnexpectedPropertyValuePopulated{property}`,
  `DeclarationFieldUnpopulated{decl_id, field}`.
- Driver-struct level: `ResourceAttributeIncomplete{resource, attribute}`,
  `DuplicateResourceId{id, kind}`,
  `UserDataFieldNotZeroInitOrOptionCopy{field, offending_ty}`.
- Body level: `UnresolvedBodyReference{body, decl_id}`,
  `OperandTypeMismatch{body, location, expected, got}`,
  `IsrContextViolation{body, kind: IsrViolationKind}`,
  `ReadOnlyRegisterWrite{body, register}`.
- Phase-2 boundary: `RequiredBindingMissing{resource, slot}`,
  `RequiredPropertyValueMissing{property}`.
- Phase-2 cross-item: `UnknownIsrTarget{interrupt, target}`,
  `NonIsrFunctionBoundAsIsr{interrupt, target}`.

```
#[serde(tag = "kind", content = "data")]
pub enum IsrViolationKind { Sleep }
```

### 20.2 Rule buckets

`verify/rules/`:

- `module.rs` -- module-tree invariants (one Driver), no duplicate item
  names, plus the Phase-1 leakage rules. Exposes
  `run(model)` (Phase-1 full) and `run_phase_2_subset(model)` (skips
  the leakage checks).
- `cross_item.rs` -- cross-reference id resolution and source-location
  coverage.
- `driver_struct.rs` -- resource attribute completeness, duplicate
  resource ids, user-data acceptance.
- `body.rs` -- body-level checks (referenced-id resolution, operand
  type matching, ReadOnlyRegisterWrite, ISR-context).
- `phase_2_boundary.rs` -- required binding / property populated.
- `phase_2_cross_item.rs` -- ISR target resolution and ISR
  classification.

Invocation order:

- `verify_phase_1` runs `module::run` then `cross_item::run` then
  lints.
- `verify_phase_2` runs `module::run_phase_2_subset`, `cross_item::run`,
  `driver_struct::run`, `body::run`, `phase_2_boundary::run`,
  `phase_2_cross_item::run`, then lints.

### 20.3 Lints (non-fatal channel)

`verify/lints/`:

```
pub struct LintFinding {
    pub level: LintLevel,
    pub kind: LintKind,
    pub provenance: VerifierProvenance,
    pub details: String,
}

pub enum LintLevel { Advisory, Warning }   // Ord: Advisory < Warning

#[serde(tag = "kind", content = "data")]
pub enum LintKind {
    PropertyRequiredWithDefault { property: DeclId },
}

pub struct VerifyReport { pub lints: Vec<LintFinding> }
```

Findings sort by `(level, subject DeclId, kind discriminant)` for
byte-identical determinism. The lint channel never produces
`VerifierError` and does not trigger rollback under
`MutationGuard::commit`.

## 21. Platform model and join

### 21.1 Parsed platform-file artifact

`platform.rs`:

```
pub struct PlatformFile {
    pub version: String,
    pub platform: PlatformHeader,
    pub properties: Vec<PropertyEntry>,
    pub bus: Option<BusSection>,
    pub construct_sections: Vec<ConstructSection>,
    pub unknown_sections: Vec<UnknownSection>,
}

pub struct PlatformHeader { pub os: String, pub board: String, pub arch: String }

pub struct PropertyEntry { pub key: String, pub value: PlatformValue }

pub struct ConstructSection {
    pub section_kind: String,         // e.g. "mmio", "clock"
    pub section_key: Option<String>,  // e.g. "uart_regs"
    pub values: BTreeMap<String, PlatformValue>,
}

pub struct BusSection {
    pub bus_type: String,
    pub values: BTreeMap<String, PlatformValue>,
}

pub struct UnknownSection {
    pub section_path: Vec<String>,
    pub values: BTreeMap<String, PlatformValue>,
}

pub enum PlatformValue {
    Integer(i64), Boolean(bool), String(String),
    List(Vec<PlatformValue>),
    Table(BTreeMap<String, PlatformValue>),
}
```

The parser lives in `platform_parser.rs` (TOML parser with
`PlatformParseError`). Property-side validation helpers in
`properties.rs` (`value_kind_name`, `is_scalar`, `is_aggregate`).

### 21.2 Phase-2 join

`join/mod.rs`:

```
pub fn populate_bindings(
    model: &mut RegseqModel,
    platform: &PlatformFile,
) -> Result<(), JoinError>;

pub struct JoinError {
    pub kind: JoinErrorKind,
    pub provenance: JoinProvenance,
}

#[serde(tag = "kind", content = "data")]
pub enum JoinErrorKind {
    UnknownPlatformSection { kind: String, id: String },
    RequiredResourceNotBound { kind: String, id: String },
    PropertyTypeMismatch { property_id: String, declared: String, provided: String },
    Phase2VerificationFailed { inner: VerifierError },
}

pub struct JoinProvenance {
    pub platform_section: String,    // always non-empty
    pub resource_id: Option<String>,
}
```

Mechanics:

- `DeclIndex::from_model(model)` pre-resolves declared resource id
  strings -> Vec indices (one `BTreeMap<String, usize>` per resource
  family, with `properties: BTreeMap<String, PropertyEntry>` carrying
  the declared `ScalarType`).
- A `MutationGuard(MutationPhase::Phase2)` is opened.
- `sections::join_platform_bus` populates `DriverStruct::platform_metadata`.
- `sections::join_construct_sections` matches `[clock.<id>]`,
  `[reset.<id>]`, `[pinctrl.<id>]`, `[interrupt.<id>]`, `[mmio.<id>]`
  to the corresponding resource and populates `binding`.
- `sections::join_properties` matches `[properties]` entries to
  `PropertyField::bound_value`.
- Required-bindings-populated and required-property-values-populated
  checks are folded into `verify_phase_2` (so they surface as
  `Phase2VerificationFailed{ inner: RequiredBindingMissing | ... }`).
- Determinism is enforced by `join/determinism.rs` (sort-before-
  serialize rules: platform sections by `(kind, id)` lexicographic
  order, properties by `property_id`).

## 22. Query helpers

`query/mod.rs`. Pure read-only deterministic helpers; assume the input
model has passed `verify_phase_1` or `verify_phase_2`.

- `call_sites` -- find every `IntrinsicCall` of a given family:
  `register_access_sites`, `poll_call_sites`, `log_call_sites`,
  `timed_delay_call_sites`, `wrapper_call_sites`, `lock_call_sites`,
  `completion_call_sites`, `pm_gate_call_sites`,
  `duration_ctor_call_sites`. Each returns `CallSite` records.
- `reachability` -- transitive call-graph closure rooted at seeds:
  `transitively_reachable_from`, `reachable_helper_names`.
- `topology` -- driver-module summaries: `bus_binding`, `bus_identity`,
  `callback_families_present`, `callback_impls`,
  `resource_field_names`, `CallbackImpl`.
- `ResourceKind` is re-exported from `model::resources`.

## 23. Serialization

`serialize.rs`:

```
pub fn to_json_pretty<T: Serialize>(value: &T, interner: &SymbolInterner)
    -> Result<String, serde_json::Error>;

pub fn body_to_json_pretty(body: &Body, interner: &SymbolInterner)
    -> Result<String, serde_json::Error>;

pub fn serialize_regseq_model(model: &RegseqModel) -> String;
```

The helpers install the per-thread serialization interner so every
`Symbol` and `FileId` emits its resolved string. Output is
newline-terminated to match committed golden files. `RegseqModel` is
serialized through its own embedded `SymbolInterner`.

Wire format properties:

- All tagged enums use `#[serde(tag = "kind", content = "data")]` (the
  "adjacently-tagged" representation): `{"kind": "Variant", "data":
  ...}`. Unit variants emit `{"kind": "Variant"}` with no `data`.
- `Option<T>` fields throughout the model are
  `#[serde(default, skip_serializing_if = "Option::is_none")]` -- they
  are absent in JSON when `None`.
- `BTreeMap` / `BTreeSet` give deterministic iteration. Most
  collections in the model are `Vec` with a defined source order;
  cross-cutting summaries that lack a natural source order use
  `BTreeMap` / `BTreeSet` (`source_locations`, `abstractions_used`,
  `intrinsics_used`, platform-bus `values`).

## 24. Construction surface (regseq-driver)

`crates/regseq-driver/src/construction.rs`. Two entry points:

```
pub fn build_declaration_skeleton<'tcx>(
    tcx: TyCtxt<'tcx>,
    registries: &ExtractionRegistries<'_>,
) -> Result<RegseqModel, ConstructionError>;

pub fn build_pass1_output<'tcx>(
    tcx: TyCtxt<'tcx>,
    registries: &ExtractionRegistries<'_>,
) -> Result<Pass1Output, ConstructionError>;
```

The first is the legacy live-callback entry; the second returns both
the model and the cross-reference maps Pass 2 needs.

```
pub struct ExtractionRegistries<'a> {
    pub intrinsic: &'a IntrinsicRegistry,
    pub framework_types: &'a FrameworkTypeRegistry,
    pub abstraction_types: &'a AbstractionTypeRegistry,
    pub pin_states: &'a PinStateRegistry,
}

pub struct Pass1Output {
    pub model: RegseqModel,
    pub cross_refs: CrossRefMaps,
}

pub struct CrossRefMaps {
    pub type_defs: HashMap<DefId, TypeDefId>,
    pub register_blocks: HashMap<DefId, RegisterBlockId>,
    pub functions: HashMap<DefId, FunctionId>,           // <- keyed by DefId, not Instance
    pub register_descriptors: HashMap<DefId, RegisterDescriptorId>,
    pub constants: HashMap<DefId, ConstantId>,
    pub register_index: RegisterIndex,
    pub pin_state_lookup: PinStateLookup,
}
```

Pass 1 walks the HIR (`construction/walk.rs`), builds the module tree,
allocates all `DeclId`s, populates per-module items, runs Rule 3
cross-crate function extraction (`cross_crate.rs`), runs cross-crate
type extraction (`cross_crate_types.rs`), and populates resources via
extraction surfaces (`resources.rs` + `extract/{clock, reset, pinctrl,
interrupts, synchronization, property, pm}.rs`). The pre-pass
`prealloc_register_block_ids_crate_wide` ensures `Mmio<T>` fields can
resolve their block id regardless of source order. Rule 7
(`register_metadata::RegisterIndex`) and Rule 8
(`pinctrl_metadata::PinStateLookup`) are built last, just before
returning.

Pass 2 (`construction/pass2.rs:build_body`) translates a single MIR
body into a `Body`, with helper indices `LocalIdMap`, `TypeDefIndex`,
`ConstantIndex`, `FunctionIndex`, `IntrinsicLookup`,
`DerefCollapseLookup`, `OrderedMarkers`, `TryMarkers`, `LogArgUnwrap`,
`RegisterIndex`, `PinStateLookup`. There is no production orchestrator
that calls `build_body` for every registered function -- tests in
`crates/regseq-driver/tests/pl011_construction.rs` provide their own
`run_pipeline` helper, and the live callback uses
`build_declaration_skeleton` (which leaves placeholder bodies in
place).

`ConstructionError` enumerates the construction-time failure modes
(30 variants in the current code). Notable categories:

- Driver-struct presence: `NoDriverStruct`, `MultipleDriverStructs`.
- Naming / shape: `DuplicateItemName`, `InvalidUserDataShape`,
  `UserDataNotCopy`, `UserDataNotZeroInitWhitelisted`,
  `MissingResourceId`.
- Extraction: `SurfaceExtractionFailed`, `UnknownRseqAttribute`,
  `RegistryLookupMismatch`, `UnknownIntrinsicCanonicalName`.
- Type mapping: `StdTypeNotPermitted`, `UnsupportedRustType`,
  `UnsupportedConstShape`, `UnsupportedProjection`,
  `UnsupportedRvalue`, `UnsupportedTerminator`,
  `UnresolvedCallTarget`, `SwitchTargetIntoCleanup`.
- Cross-crate / lang items: `NoDerefLangItem`,
  `OrderedMarkerLookupFailed`, `UnpairedOrderedMarker`,
  `StdFunctionNotPermitted`, `CrossCrateRecursionLimit`.
- Try fold / log: `MalformedTryPattern`, `UnsupportedLogFormat`,
  `LogPlaceholderArityMismatch`.
- Register / pinctrl metadata: `CrossBlockFieldUsage`,
  `WrongRegisterAccessKind`, `MissingPinStateAttribute`.

Errors carry `ErrorProvenance { item_path: String, span: Span }` (the
rustc-side analogue of `VerifierProvenance`).

## 25. Phase-1 vs Phase-2 invariants (summary)

The model is the same shape in both phases; what differs is which
slots are populated and which verifier rules apply.

Post-Phase-1 invariants (enforced by `verify_phase_1`):

- Module tree: exactly one Driver module; no duplicate item names
  within a module.
- Declaration completeness: every `Function` has a non-placeholder
  `Body`. (Currently violated by cross-crate generic callees -- see
  the model-refactor doc.)
- Phase-1 boundary leakage: every resource `binding` slot on the
  driver is `None` and every `PropertyField::bound_value` is `None`
  (`UnexpectedBindingPopulated`, `UnexpectedPropertyValuePopulated`).
  Note: `DriverStruct::platform_metadata` is also expected to be
  `None` post-Phase-1 by construction-time convention, but the
  Phase-1 leakage rule does NOT inspect it.
- Cross-reference resolution: every `TypeDefId`, `FunctionId`,
  `RegisterBlockId`, `RegisterDescriptorId` referenced anywhere in the
  model resolves to a declared entity.
- Source-location coverage: every user-authored `DeclId` (modules,
  functions, type defs, resources, register blocks/descriptors,
  constants, properties, user-data fields) has a
  `SourceOrigin::Source(_)` entry in `RegseqModel::source_locations`.
  Both a missing entry AND a `CompilerGenerated` entry raise
  `MissingSourceLocation` -- `CompilerGenerated` is treated as a hard
  violation, not a fallback.

Post-Phase-2 invariants (enforced by `verify_phase_2`):

- Module-tree invariants and cross-reference resolution still hold.
- The Phase-1 leakage rules are intentionally NOT re-run -- bindings
  are now expected to be populated.
- Driver-struct: resource attributes are complete, no duplicate
  resource ids within a kind, every user-data field satisfies Rule 1
  or Rule 2.
- Body-level: every body reference resolves; operand types match
  callee parameter types; no writes to `ReadOnly` registers
  (`ReadOnlyRegisterWrite`). The ISR-context check is plumbed but
  hook-only: the default forbidden-intrinsic policy set in
  `verify/rules/body.rs` is empty, so `IsrContextViolation` does not
  fire from end-user input today. `IsrViolationKind` defines the
  shape (currently a single `Sleep` variant) so a future policy swap
  can use the existing surface without reshaping it.
- Phase-2 boundary: every resource with `Shape::Single` has a
  populated `binding` (`RequiredBindingMissing`); resources with
  `Shape::Optional` or `Shape::Array(_)` may remain unbound. Every
  `PropertyField` with `required = true` has a populated
  `bound_value` (`RequiredPropertyValueMissing`).
- Phase-2 cross-item: every `InterruptBinding::isr_target` resolves to
  a declared, ISR-classified function.

## 26. Open gap: function bodies

The current model assumes `Function.body: Body` is always populated
with a real translated MIR body. Construction is `DefId`-keyed
(`CrossRefMaps::functions: HashMap<DefId, FunctionId>`), so generic
callees (`Option::<T>::map`, `Mmio::<T>::modify`, local `field_val<R>`)
end up as a single `FunctionId` whose `optimized_mir(def_id)` returns
generic MIR with `TyKind::Param`. Pass 2 cannot translate that into the
monomorphic `RegseqType` system. Today this is patched over by leaving
placeholder bodies (a single `Unreachable`-terminator block produced by
`construction/walk.rs:empty_body`), which `verify_phase_1`'s
declaration-completeness check rejects -- so the live callback path
uses `build_declaration_skeleton` and the verifier-clean Phase-1 model
never lands in production.

This is the gap that
`docs/discussions/compiler-refactoring/model-refactor.md` proposes
to close by rekeying extraction to `ty::Instance<'tcx>`. The model
structure in this document remains valid under that refactor; only
`Function` identity and the construction surface change (a new
`InstanceInterner`, a new optional `Function.mono_args: Option<Symbol>`
diagnostic field, and a production `build_layer1_model` orchestrator
that attaches a real `Body` per `(FunctionId, Instance)` pair).
