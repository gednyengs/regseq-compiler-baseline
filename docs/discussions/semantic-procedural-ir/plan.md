# Semantic Procedural IR Design Proposal (Revision 4)

## Context

The Regseq compiler needs a stronger internal semantic model. The current
`BehaviorPlan` has the right domain vocabulary but the wrong execution model, and
the broader compiler lacks a principled Regseq-owned item hierarchy. This
proposal addresses both: the internal model shape and the procedural IR for
executable bodies.

The evidence base is the `pl011-full-spec` MIR study and the concrete failures
in emitted C code (undeclared temporaries, malformed returns, value-returning
helpers, Rust path leakage).

---

## 1. Executive Summary

The compiler needs:

1. **A Regseq item hierarchy** -- concrete Module/Item/Struct/Enum/Function/Impl/
   Constant types that form the internal Regseq model, independent of both the
   Rust source representation and backend target shapes.

2. **A semantic procedural IR** for executable function bodies -- statement/
   expression-based, with explicit locals, a principled type universe, and
   domain-specific semantic operations as first-class constructs.

3. **A clear extraction boundary** where HIR provides the item hierarchy and
   declarative structure, MIR provides executable body semantics, and all
   Rust/MIR-specific constructs are normalized before any backend sees them.

4. **A principled model-layering architecture** that defines how the existing
   `DeclarationModel`, the new item hierarchy, the semantic procedural IR, and
   `DriverLoweringPlan` relate, what each is authoritative for, and where
   backend consumption begins (Section 8).

The semantic IR is **part of the internal Regseq model**, not a backend pre-IR.
It represents the compiler's canonical understanding of what each function body
*means* in Regseq's domain. Backends consume it; they do not help construct it.
The item hierarchy and `DeclarationModel` coexist as complementary views --
construct-family-indexed (obligations) and item-indexed (structure/semantics) --
bridged by `DriverLoweringPlan` at the backend boundary.

Although the current extraction path is Rust-specific, the semantic model is
intended to be frontend-independent after construction. Any frontend that can
provide equivalent item hierarchy, type, constant, and executable-body facts
should be able to populate this model without changing backend contracts.

---

## 2. Proposed Top-Level Regseq Item Hierarchy

### 2.1 Module

```rust
/// A Regseq module -- a logical namespace for items.
///
/// Modules are logical groupings, not emitted OS files. Backends are free
/// to choose entirely different physical file layouts.
///
/// In the first implementation slice, modules are flat (one level per
/// driver spec). Recursive module nesting is intentionally deferred
/// until multi-module driver support is needed.
pub struct Module {
    /// Module name (e.g., "driver", "registers").
    pub name: String,
    pub canonical_path: CanonicalPath,
    /// Items in source order. Resolved through the canonical ItemStore.
    pub items: Vec<ItemId>,
    /// Provenance back to source.
    pub provenance: Provenance,
}
```

### 2.2 Item

```rust
/// A first-class Regseq model item.
pub enum Item {
    /// Struct definition (driver struct, register blocks, config types).
    Struct(StructDef),
    /// Enum definition (interrupt classes, mode enums, error types).
    Enum(EnumDef),
    /// Function definition (free functions with bodies).
    Function(FunctionDef),
    /// Impl block (groups methods with their owning type and optional trait).
    Impl(ImplDef),
    /// Named constant (associated constants, masks, configuration values).
    Constant(ConstantDef),
    /// Type alias.
    TypeAlias(TypeAliasDef),
}

/// Canonical item path derived from rustc DefId during extraction.
/// Shared cross-reference identity between DeclarationModel and Item Hierarchy.
pub struct CanonicalPath(pub String);
```

### 2.3 StructDef

```rust
/// A struct definition in the Regseq model.
///
/// Covers: driver structs, register block structs, configuration types.
pub struct StructDef {
    pub id: ItemId,
    pub canonical_path: CanonicalPath,
    pub name: String,
    pub fields: Vec<StructFieldDef>,
    /// Struct-level attributes (e.g., repr(C), register_block metadata).
    pub attributes: Vec<ItemAttribute>,
    pub provenance: Provenance,
}

pub struct StructFieldDef {
    /// Canonical path for field-level cross-referencing.
    /// Example: "pl011_full_spec::driver::Pl011FullSpec::regs"
    pub canonical_path: CanonicalPath,
    pub name: String,
    pub ty: SemanticType,
    /// Field-level attributes (e.g., register side_effects, hw_updates).
    pub attributes: Vec<FieldAttribute>,
}
```

### 2.4 EnumDef

```rust
/// An enum definition in the Regseq model.
///
/// Covers: interrupt class enums, error types, mode/config enums.
pub struct EnumDef {
    pub id: ItemId,
    pub canonical_path: CanonicalPath,
    pub name: String,
    pub variants: Vec<EnumVariantDef>,
    pub provenance: Provenance,
}

pub struct EnumVariantDef {
    pub name: String,
    /// Discriminant value, if explicitly assigned.
    pub discriminant: Option<u64>,
    /// Variant fields (for non-unit variants).
    pub fields: Vec<VariantFieldDef>,
}

pub struct VariantFieldDef {
    pub name: Option<String>,
    pub ty: SemanticType,
}
```

### 2.5 FunctionDef

```rust
/// A function definition with an executable body.
///
/// This is the item-level wrapper. The body is a SemanticBody,
/// which is the procedural IR described in Section 4.
pub struct FunctionDef {
    pub id: ItemId,
    pub canonical_path: CanonicalPath,
    pub name: String,
    /// Structural origin (free, inherent, trait).
    pub kind: FunctionKind,
    /// Semantic role for backend dispatch.
    pub role: FunctionRole,
    /// Executable body in the semantic procedural IR.
    pub body: SemanticBody,
    pub provenance: Provenance,
}
```

### 2.6 ImplDef

```rust
/// An impl block grouping methods on a type.
///
/// Impl is a first-class item for provenance and grouping. Methods within
/// an impl are also reachable as standalone FunctionDefs (the impl records
/// ownership, not exclusive containment).
pub struct ImplDef {
    pub id: ItemId,
    pub canonical_path: CanonicalPath,
    /// The type this impl block is for.
    pub self_type: TypeRef,
    /// Trait being implemented, if any.
    pub trait_ref: Option<TraitRef>,
    /// Method item IDs belonging to this impl.
    pub methods: Vec<ItemId>,
    /// Associated constant item IDs belonging to this impl.
    pub constants: Vec<ItemId>,
    pub provenance: Provenance,
}

pub struct TraitRef {
    /// Recognized trait name (e.g., "Serial", "Lifecycle", "PowerManagement").
    pub name: String,
    /// Full path for provenance (e.g., "regseq::serial::Serial").
    pub path: String,
}
```

### 2.7 ConstantDef

```rust
/// A named constant in the Regseq model.
///
/// Covers: associated constants (DEFAULT_UART_CLOCK_HZ, RX_INTERRUPT_MASK),
/// module-level constants, register bitfield values.
pub struct ConstantDef {
    pub id: ItemId,
    pub canonical_path: CanonicalPath,
    /// Constant name as authored (e.g., "DEFAULT_UART_CLOCK_HZ").
    pub name: String,
    /// Owning type, if this is an associated constant.
    pub owner: Option<ItemId>,
    /// Type of the constant.
    pub ty: SemanticType,
    /// Statically known value, if evaluable.
    pub value: Option<ConstValue>,
    /// Backend-facing stable name for emission.
    /// Derived deterministically from canonical_path.
    pub stable_name: String,
    pub provenance: Provenance,
}

/// A statically known constant value.
pub enum ConstValue {
    Integer(i128),
    Bool(bool),
    Str(String),
}
```

### 2.8 TypeAliasDef

```rust
/// A type alias in the Regseq model.
pub struct TypeAliasDef {
    pub id: ItemId,
    pub name: String,
    pub target: SemanticType,
    pub provenance: Provenance,
}
```

### 2.9 ItemId and Cross-References

```rust
/// Unique item identifier within a Regseq model.
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub struct ItemId(pub u32);

/// Canonical storage for all item-level entities in the Item Hierarchy.
///
/// Modules preserve authored containment. ItemId provides stable lookup into
/// this store. ImplDef.methods and ImplDef.constants always point into this
/// canonical store; they are references, not duplicate ownership.
pub struct ItemStore {
    pub items: Vec<Item>,
}

impl ItemStore {
    pub fn get(&self, id: ItemId) -> &Item { ... }
}

/// Reference to a type (by item ID or primitive).
pub enum TypeRef {
    /// Reference to a named item (struct, enum, type alias).
    Item(ItemId),
    /// Built-in/primitive type.
    Primitive(SemanticType),
}
```

**Storage model:** `ItemId` resolves through the canonical `ItemStore`. Methods
remain separately addressable `FunctionDef` items and are grouped by
`ImplDef.methods: Vec<ItemId>`. Associated constants follow the same rule via
`ImplDef.constants: Vec<ItemId>`. `Module.items` preserves authored ordering as
`Vec<ItemId>`; the canonical item payload lives only in `ItemStore`.

### 2.10 Implementation Slicing

**First implementation slice** (needed for pl011-full-spec):
- `Module` (single module per driver spec)
- `FunctionDef` (all callback, helper, ISR bodies)
- `ConstantDef` (associated constants: masks, defaults)
- `StructDef` (driver struct, register blocks -- for field reference resolution)
- `ImplDef` (for method grouping and trait provenance)

**Deferred to later slices:**
- `EnumDef` (interrupt class enums, error types -- currently carried as
  `InterruptClassDef` in the semantic model, which is sufficient)
- `TypeAliasDef` (no current evidence of need)
- Multi-module drivers
- Dedicated first-class callback vocabularies for additional device classes

### 2.11 Relationship to Existing DeclarationModel

The item hierarchy does **not** replace `DeclarationModel`. The two coexist as
complementary views of the same driver spec (see Section 8 for the full layering
and ownership model).

---

## 3. Proposed Function/Body Ownership Model

### 3.1 Structural Origin vs Semantic Role

These are orthogonal concerns and must be modeled separately.

**Structural origin (FunctionKind)** -- where this function comes from in the
Rust source:

```rust
pub enum FunctionKind {
    /// Free function (not associated with any type).
    FreeFunction,
    /// Inherent method on a type.
    InherentMethod {
        /// The owning type.
        owner: ItemId,
    },
    /// Trait implementation method.
    TraitMethod {
        /// The trait being implemented.
        trait_ref: TraitRef,
        /// The implementing type.
        owner: ItemId,
    },
}
```

**Semantic role (FunctionRole)** -- what this function does for the backend:

```rust
pub enum FunctionRole {
    // Lifecycle
    Lifecycle(LifecycleRole),
    // Device-class callbacks, validated against the declaration-layer schema
    // for the current driver class.
    DeviceClassCallback {
        class: DeviceClassKind,
        callback: DeviceCallbackRole,
    },
    // Interrupt
    Isr {
        /// Interrupt binding target for this ISR.
        binding: String,
    },
    // Helper
    Helper,
}

pub enum LifecycleRole {
    Init,
    Deinit,
}

pub enum DeviceClassKind {
    Serial,
    Network,
    Storage,
    Other(String),
}

/// First-slice callback vocabulary plus a schema-backed extension point.
///
/// `Named` is not cross-item identity. It is a validated callback schema member
/// name supplied by the declaration layer for device classes not yet modeled as
/// dedicated enum members.
pub enum DeviceCallbackRole {
    // Serial first slice
    PortOpen,
    PortClose,
    Configure,
    TxByte,
    RxChar,
    TxReady,
    TxEmpty,
    FlushRxFifo,
    FlushTxFifo,
    SendBreak,
    // Future device classes
    Named(String),
}
```

**ISR handling:** An ISR is structurally an `InherentMethod` on the driver type
(marked with `#[rseq::isr]`). Its semantic role is `FunctionRole::Isr { binding }`.
The ISR-specific metadata (interrupt binding target) lives in the role, not the
kind. The kind records the structural fact that it's an inherent method.

**ISR validation rules:**
- Only inherent methods on the driver struct type may carry
  `FunctionRole::Isr`. Trait methods, free functions, and methods on
  non-driver types cannot be ISR roots.
- The `#[rseq::isr]` attribute is required. An inherent method without the
  attribute cannot be assigned ISR role even if it has ISR-shaped behavior.
- The interrupt binding target in `Isr { binding }` must correspond to a
  declared interrupt resource in `DeclarationModel.resources.interrupts`.

**Examples:**

| Function | Kind | Role |
|----------|------|------|
| `init` | TraitMethod { trait: "Lifecycle", owner: Driver } | Lifecycle(Init) |
| `tx_byte` | TraitMethod { trait: "Serial", owner: Driver } | DeviceClassCallback { class: Serial, callback: TxByte } |
| `uart_clock_hz` | InherentMethod { owner: Driver } | Helper |
| `handle_irq` | InherentMethod { owner: Driver } | Isr { binding: "irq" } |
| `set_baud_rate` | InherentMethod { owner: Driver } | Helper |

**Device independence rule:** `FunctionRole` must not be hardwired to one
driver family. The first implementation slice carries the serial callback
vocabulary because that is the current evidence base, but the role structure is
device-class-neutral. Additional device classes, including E1000-style network
drivers, extend `DeviceCallbackRole` or validate through `Named(schema_member)`
without changing the rest of the semantic IR.

### 3.2 Function Metadata

```rust
pub struct FunctionDef {
    pub id: ItemId,
    pub canonical_path: CanonicalPath,
    pub name: String,
    pub kind: FunctionKind,
    pub role: FunctionRole,
    pub body: SemanticBody,
    pub provenance: Provenance,
}
```

Method grouping is recorded in `ImplDef.methods` (a list of `ItemId`s). A
function knows its own kind/role; the impl block records the grouping context.

### 3.3 Constant Ownership

Associated constants are `ConstantDef` items with `owner: Some(ItemId)`. The
owning type's `ImplDef` lists them in `constants: Vec<ItemId>`. The constant
is reachable both through the impl and directly by `ItemId`.

---

## 4. Proposed Semantic Procedural IR

### 4.1 SemanticBody

```rust
/// A function's executable body in the semantic procedural IR.
pub struct SemanticBody {
    /// All locals (params, authored locals, derived temps, return place).
    /// Pre-declared with names, types, and origins.
    pub locals: Vec<SemanticLocal>,
    /// Parameter locals (indices into locals), in signature order.
    /// For methods, `self` is always params[0].
    pub params: Vec<LocalId>,
    /// Return type.
    pub return_type: SemanticType,
    /// Function body.
    pub body: Block,
}
```

### 4.2 Local Environment

```rust
pub struct SemanticLocal {
    /// Unique identifier within the function.
    pub id: LocalId,
    /// Stable name -- always a valid C identifier.
    /// Assigned by the MIR interpreter using:
    ///   - authored param names (uart_clk_hz, baud_rate)
    ///   - MIR debug names for authored locals (handled, clear_mask)
    ///   - semantic derivation for computation temps (ibrd, fbrd, div64)
    ///   - deterministic fallback (t6, t9) for unnamed intermediates
    pub name: String,
    /// Semantic type.
    pub ty: SemanticType,
    /// How this local was introduced.
    pub origin: LocalOrigin,
}

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct LocalId(pub u32);

pub enum LocalOrigin {
    /// Function parameter (position in signature).
    Param { position: usize },
    /// User-declared local variable with authored name.
    Authored,
    /// Compiler-derived intermediate (semantically meaningful temp).
    Derived,
    /// Return place.
    ReturnPlace,
}
```

**Naming rule:** The MIR interpreter assigns all local names. Backends never
derive names from MIR indices. This is non-negotiable because only the
interpreter has access to MIR debug info, authored parameter names, and
copy-chain resolution.

**`self` representation:** For inherent methods and trait methods, `self` is
represented as the first parameter local:
- `origin = LocalOrigin::Param { position: 0 }`
- `name = "self"`
- `ty = SemanticType::DriverData` for driver-owned methods in the current slice
- `ty = Ref`, `RefMut`, or `Struct` for non-driver helper types as needed

The emitter maps this semantic receiver to the backend's concrete self pointer
representation (for Linux, `struct <driver>_data *d` or equivalent). Backends
never infer the existence of `self`; they receive it explicitly through the
local environment.

### 4.3 Type Universe

```rust
/// The semantic type model for the Regseq internal representation.
///
/// This is the type universe of the semantic model. It covers all types
/// that can appear in function signatures, local declarations, struct
/// fields, enum variants, and constant definitions.
pub enum SemanticType {
    // --- Scalar types ---
    Bool,
    U8, U16, U32, U64,
    I8, I16, I32, I64,

    // --- Unit / void ---
    Void,

    // --- Compound types ---
    /// Named struct type (by item reference).
    Struct { id: ItemId, name: String },
    /// Named enum type (by item reference).
    Enum { id: ItemId, name: String },
    /// Fixed-size array.
    Array { element: Box<SemanticType>, len: usize },

    // --- Reference types ---
    /// Shared reference.
    Ref { inner: Box<SemanticType> },
    /// Mutable reference.
    RefMut { inner: Box<SemanticType> },

    // --- Carrier types ---
    /// Option<T> -- may or may not hold a value.
    Option { inner: Box<SemanticType> },
    /// Result<T, E> -- success or error.
    Result { ok: Box<SemanticType>, err: Box<SemanticType> },

    // --- Domain-specific types ---
    /// Opaque driver data (the `self` pointer, `struct driver_data *`).
    DriverData,
    /// Register block reference.
    RegisterBlock { name: String },
    /// Register value with known access width.
    RegisterValue { width: u32 },
    /// Resource handle (clock, reset, pinctrl).
    ResourceHandle { kind: ResourceKind },

    // --- Deferred / opaque ---
    /// Type that the semantic model does not yet fully represent.
    /// Carries a description for diagnostics, not for emission.
    Opaque { description: String },
}
```

**What is first-class:**
- All scalar types (Bool, U8..U64, I8..I64, Void)
- Struct and Enum by named reference (not anonymous structural types)
- Array with known length
- Shared and mutable references
- Option and Result as explicit carriers
- Domain types: DriverData, RegisterBlock, RegisterValue, ResourceHandle

**What is opaque:**
- Types the compiler cannot yet classify (trait objects, complex generics,
  foreign types) are `Opaque { description }`. This is a diagnostic escape
  hatch, not a semantic one -- backends must not emit code from Opaque types.

**What is intentionally deferred:**
- Tuple types (no current evidence of need in driver bodies)
- Function pointer types (closures are out of scope per research task)
- Slice types (no current evidence)
- Raw pointer types (MMIO access is through typed wrappers, not raw ptrs)

**First implementation slice:**
Scalars, Void, Option, Result, DriverData, Struct, Ref, RefMut, RegisterValue,
Opaque. These cover the current PL011 needs for `self`, `SerialConfig`,
`RxChar`, and reference-carrying helper flows.

**Second slice (when needed):**
Enum, Array, RegisterBlock, ResourceHandle. These become necessary when the
model tracks richer data layouts and register-block semantics within function
bodies.

### 4.4 Statement Language

```rust
pub struct Block {
    pub stmts: Vec<Stmt>,
}

pub enum Stmt {
    // --- Binding and assignment ---

    /// Declare and initialize a local: `type name = expr;`
    Let { local: LocalId, value: Expr },

    /// Assign to a previously declared target.
    Assign { target: AssignTarget, value: Expr },

    /// Compound assignment (|=, &=, +=, etc.).
    CompoundAssign { target: AssignTarget, op: BinOp, value: Expr },

    /// Evaluate expression for side effects only.
    Eval { expr: Expr },

    // --- Control flow ---

    /// Structured if/else.
    Branch { condition: Expr, then_body: Block, else_body: Block },

    /// Structured while loop.
    Loop { condition: Expr, body: Block },

    /// Ordered memory region (from ordered! macro barrier pairs).
    OrderedRegion { body: Block },

    /// Fallible operation (desugared `?` operator).
    /// Body executes; on error, cleanup_on_error runs.
    /// `error_local` is the local that receives the error value.
    FallibleOp {
        body: Block,
        /// Receives the success value when the fallible computation returns
        /// Result<T, E> with T != (). `None` means the success type is unit.
        success_local: Option<LocalId>,
        error_local: LocalId,
        cleanup_on_error: Block,
    },

    /// Return from function.
    Return { value: Option<Expr> },

    // --- Device-domain side-effect statements ---

    /// Write register fields (never produces a value).
    RegWrite { register: RegisterRef, fields: Vec<FieldWrite> },

    /// Read-modify-write register fields (never produces a value).
    RegModify { register: RegisterRef, fields: Vec<FieldWrite> },

    /// Resource operation: enable, disable, toggle (never produces a value).
    ResourceOp {
        resource: ResourceRef,
        operation: ResourceOperation,
    },

    /// Driver struct field assignment (d->field = value).
    FieldAssign { field: DriverFieldRef, value: Expr },

    /// Subsystem action from a closed, validated vocabulary.
    SubsystemAction {
        action: SubsystemActionRef,
        args: Vec<Expr>,
    },
}

pub enum AssignTarget {
    Local(LocalId),
    DriverField(DriverFieldRef),
}
```

**Design rationale:** RegWrite, RegModify, ResourceOp, and SubsystemAction are
statement-only because they never produce values in the C domain. Register reads,
helper calls, and resource property reads produce values and live in Expr.

### 4.5 Expression Language

```rust
pub enum Expr {
    // --- Literals ---
    IntLit { value: i128, ty: SemanticType },
    BoolLit(bool),

    // --- Variable references ---
    LocalRef(LocalId),
    DriverFieldRef(DriverFieldRef),

    // --- Constant references ---
    /// Reference to a named constant with full identity.
    ConstantRef(ConstantRef),

    // --- Arithmetic and logic ---
    BinOp { op: BinOp, lhs: Box<Expr>, rhs: Box<Expr> },
    UnaryOp { op: UnaryOp, operand: Box<Expr> },
    Cast { expr: Box<Expr>, ty: SemanticType },

    // --- Register reads (value-producing) ---
    /// Read a specific field from a register.
    RegFieldRead { register: RegisterRef, field: RegisterFieldRef },
    /// Read the raw value of a register.
    RegReadRaw { register: RegisterRef },
    /// Extract a field from an already-read raw value.
    FieldExtract { register: RegisterRef, field: RegisterFieldRef, raw_value: Box<Expr> },

    // --- Value-producing calls ---
    /// Call a helper function.
    HelperCall { callee: FunctionRef, args: Vec<Expr> },

    // --- Resource property access ---
    /// Read a typed property from a resource.
    ResourcePropertyRead {
        resource: ResourceRef,
        property: ResourceProperty,
    },

    /// Provide a default for an expression that may be absent
    /// (optional resource not present, option-typed helper result, etc.).
    /// Only valid for expressions with explicit optionality semantics.
    WithDefault {
        value: Box<Expr>,
        default: Box<Expr>,
    },

    // --- Interrupt-domain ---
    /// Test whether an interrupt class is active in a status word.
    InterruptClassTest { status: Box<Expr>, class_ref: InterruptClassRef },

    // --- Resource presence ---
    /// Check if an optional resource is present.
    ResourcePresent { resource: ResourceRef },

    // --- Option carrier operations ---
    /// Test whether an option-typed value is Some (presence test).
    /// Applies to helper results, driver fields, locals of type Option<T>.
    IsSome(Box<Expr>),
    /// Construct an Option::None value.
    OptionNone,
    /// Construct an Option::Some(inner) value.
    OptionSome(Box<Expr>),
    /// Extract the inner value from an Option that is known to be Some.
    /// Semantic lowering may only introduce this under a dominating
    /// IsSome/MIR discriminant check. A dedicated validation pass must reject
    /// any OptionUnwrap not proven to be dominated by a matching presence test.
    OptionUnwrap(Box<Expr>),
}
```

### 4.6 Option Carrier Semantics

The four option carrier expressions (`IsSome`, `OptionNone`, `OptionSome`,
`OptionUnwrap`) provide explicit carrier-value semantics in the IR. They
are needed because:

1. Helper functions may return `Option<T>` (e.g., `rx_char() -> Option<RxChar>`)
2. Driver fields may be `Option<T>` (e.g., `pending_tx: Option<u8>`)
3. Local variables may hold option-typed values

Without them, conditions and assignments would rely on implicit truthiness
(treating `Option` as a boolean), which the semantic model explicitly rejects.

**Scope of the carrier model:**

This carrier model is **option-specific**. `Result` carriers are handled
structurally by `Stmt::FallibleOp` (which collapses `?` into
body + success_local + error_local + cleanup_on_error), so `Result` does not need expression-
level carrier constructors in the current PL011 slice.

If future drivers require explicit `Result` value construction (e.g.,
`Ok(value)` or `Err(e)` as expressions), the model can be extended with
`Expr::ResultOk(Box<Expr>)` and `Expr::ResultErr(Box<Expr>)` using the same
pattern. This is a deliberate, bounded deferral -- not an oversight.

**OptionUnwrap validity rule:**

`OptionUnwrap` is not a backend convenience. It is a semantic construct whose
soundness is guaranteed by lowering plus validation:
- semantic lowering may emit `OptionUnwrap(e)` only in a region dominated by a
  proven `IsSome(e)` test or equivalent MIR discriminant check
- a semantic-IR validation pass must reject any unmatched unwrap before
  `DriverLoweringPlan` construction
- backends treat `OptionUnwrap` as already-validated and never attempt to prove
  or repair its safety

**How carriers participate in statements and control flow:**

| Pattern | Statement form | Expression form |
|---------|---------------|-----------------|
| `rx_char().is_some()` as loop condition | `Stmt::Loop { condition: IsSome(HelperCall(...)) }` | `Expr::IsSome(Expr::HelperCall { ... })` |
| `if let Some(byte) = pending_tx` | `Stmt::Branch { condition: IsSome(DriverFieldRef(pending_tx_ref)) }` then `Stmt::Let { local: byte, value: OptionUnwrap(DriverFieldRef(pending_tx_ref)) }` | n/a |
| `pending_tx = None` | `Stmt::FieldAssign { field: pending_tx_ref, value: OptionNone }` | `Expr::OptionNone` |
| `pending_tx = Some(ch)` | `Stmt::FieldAssign { field: pending_tx_ref, value: OptionSome(LocalRef(ch)) }` | `Expr::OptionSome(Expr::LocalRef(ch))` |

### 4.7 Body-Level Semantic References

```rust
/// Reference to another function in the same semantic model.
pub struct FunctionRef {
    pub item: ItemId,
    pub canonical_path: CanonicalPath,
    pub name: String,
}

/// Reference to a driver-owned field.
pub struct DriverFieldRef {
    pub owner: ItemId,
    pub canonical_path: CanonicalPath,
    pub name: String,
}

/// Reference to a register within a register block.
pub struct RegisterRef {
    pub block: ItemId,
    pub canonical_path: CanonicalPath,
    pub name: String,
}

/// Reference to a field within a register.
pub struct RegisterFieldRef {
    pub register: RegisterRef,
    pub canonical_path: CanonicalPath,
    pub name: String,
}

/// Reference to a named interrupt class.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct InterruptClassId(pub u32);

pub struct InterruptClassRef {
    /// SemanticModel-scoped interrupt-class identity.
    pub id: InterruptClassId,
    pub name: String,
}

/// Closed vocabulary for cross-subsystem actions.
pub struct SubsystemActionRef {
    pub subsystem: SubsystemKind,
    pub action: SubsystemActionKind,
}

pub enum SubsystemKind {
    DeviceClassIo,
    Buffering,
}

pub enum SubsystemActionKind {
    IngestRxUnit,
    PublishRxBatch,
}
```

**Design rule:** Semantic bodies do not use short-name `String` fields for
cross-item identity. Human-readable names remain for display and emission, but
identity flows through `ItemId` and `CanonicalPath` carrying refs.

**Validation rule:** Any subsystem action outside this closed vocabulary is a
semantic-lowering hard error. Backends do not interpret free-form subsystem or
action strings.

**OS/device-neutrality rule:** Subsystem actions name semantic hooks, not
target-OS helper names. Linux, Zephyr, and future backends map these hooks to
their own APIs. The current vocabulary is intentionally backend-neutral and may
grow per device class without introducing raw OS spellings into the semantic IR.

### 4.8 Resource References and Properties

```rust
/// A reference to a driver resource.
pub struct ResourceRef {
    /// Resource kind (clock, reset, pinctrl).
    pub kind: ResourceKind,
    /// Driver struct field holding this resource.
    pub field: DriverFieldRef,
}

/// A typed resource property that can be read.
pub enum ResourceProperty {
    /// Clock rate (Hz).
    ClockRate,
    /// Reset line state.
    ResetState,
    /// Pin controller active state.
    PinCtrlState,
}

/// Resource operations (side-effect-only).
pub enum ResourceOperation {
    Enable,
    Disable,
    Toggle { duration_us: Option<u64> },
    ApplyState { state: PinCtrlState },
}

pub enum PinCtrlState {
    Default,
    Sleep,
}
```

**Extensibility rule:** `ResourceProperty` is a closed vocabulary per revision,
but it is not permanently tied to the PL011 serial slice. Additional
device/resource properties are added by extending the enum, not by introducing
backend-specific strings.

**Design rationale for resource reads:** Instead of the previous ad-hoc
`ResourceRead { property: String, default }`, resource property access is now
two separate, composable constructs:

- `ResourcePropertyRead` -- typed, closed set of resource properties
- `WithDefault` -- combinator for expressions with explicit optionality semantics

`uart_clock_hz` becomes:
```
Let(rate, WithDefault {
    value: ResourcePropertyRead { resource: clock_ref, property: ClockRate },
    default: ConstantRef { canonical_path: "...::DEFAULT_UART_CLOCK_HZ", ... }
})
Return { value: Some(LocalRef(rate)) }
```

This is principled because:
- `ResourcePropertyRead` is a finite, typed vocabulary (not an arbitrary string)
- `WithDefault` is reusable for expressions whose absence is explicit in the
  semantic IR (`ResourcePropertyRead`, option-typed helper results), not for
  arbitrary total expressions
- The two compose cleanly for the MIR pattern `as_ref().map(f).unwrap_or(d)`

**Canonicalization rule:** semantic lowering should prefer `WithDefault` for
direct optional-value defaulting patterns (`option_expr.unwrap_or(default)` and
equivalent MIR). Structured `Branch + IsSome + OptionUnwrap` remains for cases
where the control-flow shape itself is semantically significant.

### 4.9 Constant References

```rust
/// A reference to a named constant with full identity.
///
/// Constants carry enough identity for deterministic ownership,
/// namespacing, and provenance. Backend-facing names are derived
/// separately from the symbolic identity.
pub struct ConstantRef {
    /// Item ID of the ConstantDef, if the constant is in the model.
    /// `None` is reserved for framework-/backend-defined constants that are
    /// not authored ConstantDef items.
    pub id: Option<ItemId>,
    /// Canonical path for unambiguous cross-referencing.
    /// Example: "pl011_full_spec::driver::Pl011FullSpec::DEFAULT_UART_CLOCK_HZ"
    pub canonical_path: CanonicalPath,
    /// Short display name (e.g., "DEFAULT_UART_CLOCK_HZ").
    pub name: String,
    /// Backend-facing stable identifier for emission.
    /// Derived deterministically from canonical_path.
    pub stable_name: String,
    /// Statically known value, if available.
    pub known_value: Option<i128>,
}
```

**Why not bare `name`:** Associated constants like
`Pl011FullSpec::DEFAULT_UART_CLOCK_HZ` and `Pl011FullSpec::RX_INTERRUPT_MASK`
need ownership provenance for:
- Deterministic backend naming (PL011_FULL_SPEC_DEFAULT_UART_CLOCK_HZ)
- Disambiguation when multiple types define constants with the same short name
- Traceability back to source

The `stable_name` is computed once during semantic IR construction, not
re-derived by backends. Backends use `stable_name` directly for `#define` names.

### 4.10 Supporting Types

```rust
pub struct FieldWrite {
    pub field: RegisterFieldRef,
    pub value: FieldWriteValue,
}

pub enum FieldWriteValue {
    /// Named register value (e.g., WLEN_8, FEN_SET).
    Named(String),
    /// Raw numeric value.
    Raw(u32),
    /// Zero / clear.
    Zero,
    /// Reference to a semantic local.
    Local(LocalId),
    /// Computed expression.
    Computed(Box<Expr>),
}

pub enum UnaryOp {
    Not,     // logical not
    Neg,     // arithmetic negate
    BitNot,  // bitwise complement
}

// BinOp reused from current behavior.rs (12 variants):
// Add, Sub, Mul, Div, Shl, Shr, BitAnd, BitOr, Eq, Ne, And, Or
```

### 4.11 Condition Unification

All conditions are `Expr` values. The mapping from current `BranchCondition`:

| BranchCondition variant    | Expr form                                             |
|---------------------------|-------------------------------------------------------|
| RegFieldEq                | BinOp(Eq, FieldExtract, IntLit)                      |
| RegFieldNonZero           | FieldExtract (nonzero implicit in C `if`)            |
| RawEqZero                 | BinOp(Eq, source_expr, IntLit(0))                    |
| FieldIsNone               | UnaryOp(Not, ResourcePresent { ... })                |
| InterruptClassActive      | InterruptClassTest { status, class_ref }             |
| HelperPredicate           | HelperCall { callee: FunctionRef, args }             |
| And(a, b)                 | BinOp(And, a, b)                                     |
| BoolParam                 | LocalRef(param_id)                                   |
| ValueTest                 | eliminated (was already wrapping ValueExpr)          |

### 4.12 What the IR Eliminates

| Removed type        | Replacement                                  |
|--------------------|----------------------------------------------|
| BehaviorPlan       | SemanticModel                                |
| BehaviorNode       | Stmt                                         |
| ValueExpr          | Expr                                         |
| BranchCondition    | Expr (unified)                               |
| ReturnSemantic     | Stmt::Return { value: Option<Expr> }         |
| AssignValue        | Expr                                         |
| PortableType       | SemanticType                                 |
| FunctionBehavior   | FunctionDef + SemanticBody                   |

### 4.13 Validation Against MIR Study Functions

Every PL011 function maps cleanly:

**1. program_polling_port** -- sequential helpers in ordered region:
```
OrderedRegion {
    [Eval(HelperCall(set_baud_rate_ref, [LocalRef(clk), LocalRef(baud)])),
     Eval(HelperCall(configure_line_control_ref, [])),
     ...]
}
```

**2. set_baud_rate** -- arithmetic + register writes:
```
Let(div64, BinOp(Div, BinOp(Mul, Cast(LocalRef(clk), U64), IntLit(4)), Cast(LocalRef(baud), U64)))
Let(ibrd, Cast(BinOp(Shr, LocalRef(div64), IntLit(6)), U32))
Let(fbrd, Cast(BinOp(BitAnd, LocalRef(div64), IntLit(0x3f)), U32))
RegWrite(ibrd_reg_ref, [FieldWrite(baud_divint_field_ref, Local(ibrd_id))])
RegWrite(fbrd_reg_ref, [FieldWrite(baud_divfrac_field_ref, Local(fbrd_id))])
```

**3. uart_clock_hz** -- optional resource read with default:
```
Let(rate, WithDefault {
    value: ResourcePropertyRead { resource: clock_ref, property: ClockRate },
    default: ConstantRef { canonical_path: "...::DEFAULT_UART_CLOCK_HZ", ... }
})
Return { value: Some(LocalRef(rate)) }
```

**4. enable_clock_if_present** -- optional resource + fallible enable:
```
Branch {
    condition: ResourcePresent { resource: clock_ref },
    then_body: [
        FallibleOp {
            body: [ResourceOp { resource: clock_ref, operation: Enable }],
            success_local: None,
            error_local: ret_id,
            cleanup_on_error: [Return { value: Some(LocalRef(ret_id)) }]
        }
    ],
    else_body: []
}
Return { value: Some(IntLit { value: 0, ty: I32 }) }
```

**5. toggle_reset_if_present** -- same pattern, Toggle with duration:
```
ResourceOp { resource: reset_ref, operation: Toggle { duration_us: Some(10) } }
```

**6. clear_explicit_interrupts** -- bitmask test + conditional write:
```
Let(mask, BinOp(BitAnd, LocalRef(status), ConstantRef { canonical_path: "...::EXPLICIT_ISR_CLEAR_MASK", ... }))
Branch {
    condition: BinOp(Ne, LocalRef(mask), IntLit(0)),
    then_body: [RegWrite(icr_reg_ref, [FieldWrite(raw_field_ref, Local(mask_id))])],
    else_body: []
}
```

**7. service_rx_fifo** -- loop with option presence test + loop-carried local:
```
Let(handled, BoolLit(false))
Loop {
    condition: IsSome(HelperCall(rx_char_ref, [])),
    body: [Assign(Local(handled_id), BoolLit(true))]
}
Return { value: Some(LocalRef(handled)) }
```
Note: `rx_char()` returns `Option<RxChar>`. The loop condition is an explicit
`IsSome` test, not implicit truthiness.

**8. service_tx_fifo** -- option unwrap + register write + option assignment:
```
Branch {
    condition: IsSome(DriverFieldRef(pending_tx_ref)),
    then_body: [
        Let(byte, OptionUnwrap(DriverFieldRef(pending_tx_ref))),
        Branch {
            condition: HelperCall(tx_ready_ref, []),
            then_body: [
                Let(byte_u32, Cast(LocalRef(byte), U32)),
                RegWrite(dr_reg_ref, [FieldWrite(data_field_ref, Local(byte_u32_id))]),
                FieldAssign(pending_tx_ref, OptionNone),
                Eval(HelperCall(disable_tx_interrupts_ref, [])),
                Return { value: Some(BoolLit(true)) }
            ],
            else_body: [Return { value: Some(BoolLit(false)) }]
        }
    ],
    else_body: [
        Eval(HelperCall(disable_tx_interrupts_ref, [])),
        Return { value: Some(BoolLit(true)) }
    ]
}
```
Note: `pending_tx` is `Option<u8>`. The branch condition is `IsSome`, the
inner value is extracted with `OptionUnwrap`, and `None` assignment uses
`OptionNone` -- all type-correct.

**9. init** -- sequential fallible helper calls:
```
FallibleOp { body: [Eval(HelperCall(toggle_reset_if_present_ref, []))], success_local: None, ... }
FallibleOp { body: [Eval(HelperCall(enable_clock_if_present_ref, []))], success_local: None, ... }
Let(clk, HelperCall(uart_clock_hz_ref, []))
Eval(HelperCall(program_polling_port_ref, [LocalRef(clk), DriverFieldRef(current_speed_ref)]))
Return { value: Some(IntLit { value: 0, ty: I32 }) }
```

**10. handle_irq** -- ISR with class tests + accumulated result:
```
Let(status, RegReadRaw(mis_reg_ref))
Branch { condition: BinOp(Eq, LocalRef(status), IntLit(0)),
         then_body: [Return { value: Some(ConstantRef { canonical_path: "...::IRQ_NONE", ... }) }] }
Let(handled, BoolLit(false))
Branch { condition: InterruptClassTest { status: LocalRef(status), class_ref: rx_irq_class_ref },
         then_body: [CompoundAssign(Local(handled), BitOr, HelperCall(service_rx_fifo_ref, []))] }
Branch { condition: InterruptClassTest { status: LocalRef(status), class_ref: tx_irq_class_ref },
         then_body: [CompoundAssign(Local(handled), BitOr, HelperCall(service_tx_fifo_ref, []))] }
Eval(HelperCall(clear_explicit_interrupts_ref, [LocalRef(status)]))
Branch { condition: LocalRef(handled),
         then_body: [Return { value: Some(ConstantRef { canonical_path: "...::IRQ_HANDLED", ... }) }],
         else_body: [Return { value: Some(ConstantRef { canonical_path: "...::IRQ_NONE", ... }) }] }
```

**11. Ordered subsystem action** -- closed, backend-neutral subsystem vocabulary:
```
OrderedRegion {
    [
        SubsystemAction { action: rx_ingest_unit_ref, args: [LocalRef(ch)] },
        SubsystemAction { action: rx_publish_batch_ref, args: [] },
    ]
}
```

**12. Explicit RegModify** -- no helper-name reconstruction:
```
RegModify(cr_reg_ref, [
    FieldWrite(uarten_field_ref, Named("UARTEN_ENABLE")),
    FieldWrite(txe_field_ref, Named("TXE_ENABLE")),
])
```

All current function families map without structural gaps.

---

## 5. HIR Extraction Scope

### What should come from HIR

| Fact | HIR source | Regseq model destination |
|------|-----------|-------------------------|
| Module hierarchy | Item ownership | Module.items (Vec<ItemId>) |
| Driver identity | #[rseq::driver] attribute | DriverDecl |
| Struct definitions | Struct items with fields | StructDef |
| Enum definitions | Enum items with variants | EnumDef |
| Impl blocks | Impl items with trait ref | ImplDef |
| Method signatures | FnDecl params + return type | FunctionDef.body.params, .return_type |
| Parameter names | FnDecl param patterns | SemanticLocal { origin: Param, name } |
| Associated constants | Const items in impls | ConstantDef with owner |
| Trait references | Impl trait refs | ImplDef.trait_ref |
| Register blocks | #[rseq::register_block] structs | StructDef with register attributes |
| Resource fields | Typed struct fields (Mmio, Clock, etc.) | StructFieldDef with semantic types |
| ISR markers | #[rseq::isr] attributes | FunctionDef.role = Isr |

### What should NOT be recomputed from MIR

- Function signatures (HIR has authored names; MIR has `_argN`)
- Type names and field names (HIR preserves source-level names)
- Module/item hierarchy and provenance
- Register block layouts and bitfield metadata
- Driver metadata, compatible strings, device class
- Impl-to-trait provenance
- Associated constant ownership

### Source-level provenance

Every item carries `provenance: Provenance` linking back to the raw frontend
artifact. Module structure preserves the logical source organization. Backends
may flatten or reorganize for emission, but the model preserves the authored
grouping.

---

## 6. MIR Normalization Scope

### MIR constructs that survive into the semantic IR

| MIR construct          | Semantic IR form                                    |
|-----------------------|----------------------------------------------------|
| Authored params       | SemanticLocal { origin: Param }                    |
| Authored locals       | SemanticLocal { origin: Authored }                 |
| Value-producing temps | SemanticLocal { origin: Derived }                  |
| Return place          | SemanticLocal { origin: ReturnPlace }              |
| Assign                | Stmt::Let or Stmt::Assign                          |
| BinaryOp/UnaryOp     | Expr::BinOp / Expr::UnaryOp                       |
| Cast                  | Expr::Cast                                         |
| Call (helper)         | Expr::HelperCall or Stmt::Eval(HelperCall)         |
| Call (register write) | Stmt::RegWrite / Stmt::RegModify                   |
| Call (register read)  | Expr::RegReadRaw / Expr::RegFieldRead              |
| Call (resource op)    | Stmt::ResourceOp                                   |
| Call (resource read)  | Expr::ResourcePropertyRead                         |
| SwitchInt (branch)    | Stmt::Branch                                       |
| SwitchInt (loop)      | Stmt::Loop                                         |
| Associated consts     | Expr::ConstantRef with full identity               |

### MIR constructs normalized away

| MIR construct                | Normalization                                    |
|-----------------------------|--------------------------------------------------|
| Raw MIR locals (_3, _14)    | Named SemanticLocals with stable identifiers     |
| Projection chains            | Resolved by place_resolver into semantic places  |
| Deref calls                  | Absorbed into place resolution                   |
| Try::branch / FromResidual   | Collapsed into Stmt::FallibleOp                  |
| Option::as_ref/map/unwrap_or | ResourcePropertyRead + WithDefault, or Branch    |
| Option::is_some/is_none      | Expr::IsSome (explicit presence test)            |
| Option discriminant + payload | IsSome condition + OptionUnwrap in branch body   |
| Option None/Some construction | Expr::OptionNone / Expr::OptionSome              |
| FieldValue constructors      | Absorbed into FieldWriteValue                    |
| Memory barrier pairs         | Paired into Stmt::OrderedRegion                  |
| Duration constructors        | Absorbed into ResourceOperation.Toggle params    |
| Rust paths (regseq::...)     | Replaced with semantic names / ConstantRef       |
| Core library calls           | Normalized or hard error                         |
| Unwind/cleanup BBs           | Absorbed into FallibleOp.cleanup_on_error        |
| StorageLive/StorageDead      | Dropped (no semantic meaning)                    |
| Discriminant reads           | Absorbed into Branch conditions                  |
| Basic block indices          | Replaced by structured Block control flow        |
| SetDiscriminant              | Absorbed into Assign with appropriate value      |

### Hard errors

Any MIR construct that survives normalization without a semantic mapping is a
hard error in the MIR interpreter. The interpreter must not:
- Silently drop unrecognized calls
- Default to zero for unknown values
- Emit TODO comments for unresolved paths
- Use Debug format for IR interchange

This is grounded in the memory rules: "no silent fallbacks in lowering",
"no TODO comments for unresolved lowering", "no Debug format as semantic
interchange."

---

## 7. Backend Consumption Boundary

### What backends consume

Backends receive a `DriverLoweringPlan` containing:

- `identity: DriverIdentity` (name, compatible, device_class, bus)
- `resources: ResourceObligations` (MMIO, interrupts, clocks, resets, pinctrl)
- `register_obligations: RegisterObligations` (blocks, fields, bitfield info)
- `behavior: SemanticModel` (NEW -- replaces BehaviorPlan)

`SemanticModel`:
```rust
pub struct SemanticModel {
    /// Exactly one driver spec per SemanticModel.
    /// Per-function semantic bodies.
    pub functions: Vec<FunctionDef>,
    /// Constants referenced during backend planning or emission.
    pub constants: Vec<ConstantDef>,
    /// Interrupt class definitions.
    pub interrupt_classes: Vec<InterruptClassDef>,
}
```

For each function, backends walk `Stmt`/`Expr` trees and map:
- `SemanticType` -> C type string (`u32`, `s8`, `bool`, `int`, etc.)
- `Stmt::Let` -> C local declaration with initializer
- `Stmt::RegWrite` -> `driver_reg_write(d, REG_OFFSET, value)` call
- `Stmt::ResourceOp` -> OS-specific API call
- `Expr::HelperCall` -> `driver_helper(d, args)` call
- `Expr::ConstantRef` -> `#define` name from `stable_name`
- `Expr::RegReadRaw` -> `driver_reg_read(d, REG_OFFSET)` call

### What backends never see

- Raw MIR locals, basic blocks, projections, terminators
- Rust paths (regseq::ReadWrite, core::option::Option, core::ops::Try)
- Wrapper-specific generic syntax or method dispatch spelling
- SSA temporaries without names and types
- MIR return-place conventions (_0)
- Unwind actions and cleanup block indices
- StorageLive/StorageDead markers
- Raw rustc type representations

### No second C-oriented lower IR needed

The semantic IR is already statement/expression-based with structured control
flow, typed locals, and named constants. This maps directly to C. Linux and
Zephyr backends differ in:
- OS-specific API calls for resources (clk_*, devm_*, etc.)
- Registration/module infrastructure (platform_driver, DEVICE_DT_*)
- File layout and includes

These differences live in `LinuxBackendPlan` / `ZephyrBackendPlan`, not in a
lower IR.

---

## 8. Model Layering and Ownership

This section defines the long-term architecture of the compiler's internal model
layers: what they are, what each is authoritative for, how they reference each
other, and where backend consumption begins.

### 8.1 The Four Model Layers

The compiler has four distinct model layers. They are listed in pipeline order:

```
┌─────────────────────────────────────────────────────────────────┐
│  Layer 1: DeclarationModel                                      │
│  (construct-family-indexed declaration facts)                    │
│  Authority: resource obligations, callback inventory,            │
│             register metadata, property constraints              │
│  Produced from: RawFrontendArtifact (HIR extraction)            │
│  Lives in: regseq-model                                         │
├─────────────────────────────────────────────────────────────────┤
│  Layer 2: Item Hierarchy (Module / Item / FunctionDef / ...)    │
│  (item-indexed logical source structure + semantic bodies)       │
│  Authority: item identity, containment, structural ownership,   │
│             type definitions, constant identity, executable      │
│             body semantics (via SemanticBody on FunctionDef)     │
│  Produced from: HIR (structure) + MIR (bodies)                  │
│  Lives in: regseq-model (target) / regseq-backends (initial)    │
├─────────────────────────────────────────────────────────────────┤
│  Layer 3: DriverLoweringPlan                                    │
│  (portable obligation plan + semantic model for emission)        │
│  Authority: backend-consumable resource obligations,             │
│             acquisition/teardown ordering, device-class roles,   │
│             register obligations, synchronization obligations    │
│  Produced from: DeclarationModel + analysis + SemanticModel      │
│  Lives in: regseq-backends/lowering                             │
├─────────────────────────────────────────────────────────────────┤
│  Layer 4: OS-Specific Backend Plan (LinuxBackendPlan / ...)     │
│  (OS-specific realization decisions)                             │
│  Authority: registration shape, probe/remove orchestration,      │
│             OS-specific API mapping, file layout                 │
│  Produced from: DriverLoweringPlan                              │
│  Lives in: regseq-backends/linux, regseq-backends/zephyr        │
└─────────────────────────────────────────────────────────────────┘
```

### 8.2 What Each Layer Is Authoritative For

| Concern | Authoritative layer | Why |
|---------|-------------------|-----|
| Resource obligations (MMIO, interrupts, clocks, resets, pinctrl) | DeclarationModel | Derived from driver struct fields and attributes |
| Callback inventory and family grouping | DeclarationModel | Derived from trait impl scanning |
| Register block layouts and bitfield definitions | DeclarationModel | Derived from register_bitfields! and register_block attributes |
| Property declarations and defaults | DeclarationModel | Derived from property field analysis |
| **Item identity** (what items exist, their names) | **Item Hierarchy** | Captures logical source structure |
| **Item containment** (which module owns which items) | **Item Hierarchy** | Module → Vec<ItemId> captures hierarchy |
| **Structural function ownership** (FunctionKind) | **Item Hierarchy** | ImplDef + FunctionDef.kind captures free/inherent/trait |
| **Type definitions** (struct fields, enum variants) | **Item Hierarchy** | StructDef, EnumDef capture type shape |
| **Constant identity** (owner, name, stable_name, value) | **Item Hierarchy** | ConstantDef with full provenance |
| **Executable body semantics** (what a function body means) | **Semantic IR** (on FunctionDef.body) | Canonical procedural representation |
| **Semantic function role** (FunctionRole) | **Semantic IR** | Backend dispatch classification |
| Backend-facing resource obligations | DriverLoweringPlan | Portable obligations consumed by all backends |
| Acquisition/teardown ordering | DriverLoweringPlan | Sequenced resource lifecycle |
| Device-class role obligations | DriverLoweringPlan | Serial/network/etc. role realization |
| Portable behavioral plan | DriverLoweringPlan (carries SemanticModel) | Functions with semantic bodies |
| OS-specific API mappings | OS Backend Plan | Linux/Zephyr-specific realization |
| File layout, registration shape | OS Backend Plan | OS-specific infrastructure |

### 8.3 How The Layers Reference Each Other

**Canonical item identity: `CanonicalPath`**

Both models need a shared identity for the same source item. This identity
already exists in the compiler: rustc's `DefId` is rendered as a fully-
qualified `def_path` string during extraction (e.g.,
`"pl011_full_spec::driver::Pl011FullSpec::init"`). The current `MirFnBodyDecl`
already carries `def_path: String` for exactly this purpose.

We formalize this as:

```rust
/// Canonical item path derived from rustc DefId during extraction.
///
/// This is the shared cross-reference identity between DeclarationModel
/// and the Item Hierarchy. It is fully qualified, stable within a
/// compilation, and unambiguous across modules, impls, and types.
///
/// Examples:
///   "pl011_full_spec::driver::Pl011FullSpec"          (driver struct)
///   "pl011_full_spec::driver::Pl011FullSpec::init"    (trait method)
///   "pl011_full_spec::driver::Pl011FullSpec::DEFAULT_UART_CLOCK_HZ"
///   "pl011_full_spec::registers::Pl011Registers"      (register block)
///   "pl011_full_spec::driver::<impl Serial for Pl011FullSpec>"  (impl)
pub struct CanonicalPath(pub String);
```

**Normalization rule:** `CanonicalPath` is the rustc `DefId`-derived fully
qualified path rendered in a canonical textual form:
- `::` as the segment separator
- impl blocks rendered explicitly (`<impl Trait for Type>` or `<impl Type>`)
- field paths extend the parent item path with `::field_name`
- no backend-specific mangling or stable-name rewriting in the identity itself

`stable_name` is derived from `CanonicalPath`; it is not the identity.

Both DeclarationModel entities and Item Hierarchy items carry a
`CanonicalPath`. This is derived once during HIR/MIR extraction from the
rustc `DefId` and is the **authoritative cross-reference key** between
layers.

**Other shared identifiers:**

- `ItemId` -- numeric identifier within the Item Hierarchy only. Used for
  efficient intra-hierarchy cross-references (ImplDef.methods → Vec<ItemId>,
  ConstantDef.owner → Option<ItemId>). Not shared with DeclarationModel.

- `LocalId` -- unique local identifier within a single SemanticBody. Not
  cross-referenced between functions.

- `Provenance` -- shared provenance type (already in regseq-model). Both
  models carry Provenance back to the raw frontend artifact. Provenance is
  supplementary metadata; `CanonicalPath` is the identity key.

**DeclarationModel carries canonical paths:**

DeclarationModel entities are extended to carry `CanonicalPath` alongside
their existing fields:
- `CallbackImplDecl` carries the canonical path of the impl block
- `MirFnBodyDecl` already carries `def_path` (rename to `CanonicalPath`)
- `RegisterBlockDecl` carries the canonical path of the register block struct
- Resource field declarations carry the canonical path of the driver struct
  field

This is a backward-compatible enrichment: existing short-name fields (`.name`,
`.field_name`) remain for display and emission, while `CanonicalPath` provides
unambiguous cross-reference.

**Item Hierarchy carries canonical paths:**

Every Item Hierarchy item carries a `CanonicalPath`:
- `FunctionDef.canonical_path` = "module::Type::method"
- `StructDef.canonical_path` = "module::StructName"
- `ImplDef.canonical_path` = "module::<impl Trait for Type>"
- `ConstantDef.canonical_path` = "module::Type::CONST_NAME"
- `StructFieldDef.canonical_path` = "module::Type::field"

**Cross-reference by canonical path:**

When `DriverLoweringPlan` is built, the cross-reference between
DeclarationModel entities and Item Hierarchy items uses `CanonicalPath`
matching, not short names:

```
DeclarationModel entity          CanonicalPath
─────────────────────────        ─────────────────────────────────
CallbackImplDecl (init)          "...::Pl011FullSpec::init"
RegisterBlockDecl (Pl011Regs)    "...::registers::Pl011Registers"
MmioFieldDecl (regs)             "...::Pl011FullSpec::regs"

Item Hierarchy item              CanonicalPath
─────────────────────────        ─────────────────────────────────
FunctionDef (init)               "...::Pl011FullSpec::init"
StructDef (Pl011Registers)       "...::registers::Pl011Registers"
StructFieldDef (regs)            "...::Pl011FullSpec::regs"
```

This eliminates the ambiguity risks of short-name matching:
- Multiple types with `init` methods → distinguished by owner path
- Multiple structs with same field names → distinguished by parent path
- Associated constants with overlapping names → distinguished by owner path

**Cross-link resolution table:**

During `DriverLoweringPlan` construction, a one-time resolution table maps
`CanonicalPath` to both item-level and field-level targets. This table is
built once and used for all cross-references:

```rust
/// Built once during plan construction.
pub struct CrossLinkTable {
    /// Item-level resolution (structs, enums, functions, impls, constants).
    path_to_item: BTreeMap<CanonicalPath, ItemId>,
    /// Field-level resolution (struct fields, enum variant fields).
    /// Maps field canonical path → (owning ItemId, field index).
    path_to_field: BTreeMap<CanonicalPath, (ItemId, usize)>,
}
```

Both item-level paths (e.g., `"...::Pl011FullSpec"`) and field-level paths
(e.g., `"...::Pl011FullSpec::regs"`) are resolved through this single table.
DeclarationModel entities use `CanonicalPath` to look up their corresponding
Item Hierarchy targets at either granularity.

**Item Hierarchy → DeclarationModel:**

The Item Hierarchy does not directly import DeclarationModel types. Instead,
it captures the same source information in a different shape:
- DeclarationModel's `CallbackDeclSet` → Item Hierarchy's `ImplDef` with
  `trait_ref` + method `FunctionDef`s
- DeclarationModel's `RegisterDeclSet` → Item Hierarchy's `StructDef` with
  register-block attributes
- DeclarationModel's `MirBodyDeclSet` → Item Hierarchy's `FunctionDef.body`
  (SemanticBody)

The two models are **parallel derivations from the same source**, linked by
`CanonicalPath` identity rather than short names.

**DriverLoweringPlan bridges both:**

`DriverLoweringPlan` is derived from DeclarationModel (for obligations) and
carries a `SemanticModel` (which is a projection of the Item Hierarchy's
function bodies + referenced constants + interrupt class definitions):

```rust
pub struct DriverLoweringPlan {
    // From DeclarationModel:
    pub identity: DriverIdentity,
    pub resources: ResourceObligations,
    pub register_obligations: RegisterObligations,
    pub callbacks: CallbackInventory,
    pub acquisition_order: Vec<AcquisitionStep>,
    pub teardown_order: Vec<TeardownStep>,
    pub device_class_roles: DeviceClassRoles,
    pub synchronization: SynchronizationObligations,

    // From Item Hierarchy:
    pub behavior: SemanticModel,
    // SemanticModel.functions carries FunctionDef with SemanticBody
    // SemanticModel.constants carries ConstantDef with stable emission names
    // SemanticModel.interrupt_classes carries InterruptClassDef
}
```

The plan is the **meeting point** of the two model faces. Obligation fields
come from the construct-family view. Behavioral fields come from the
item/semantic view.

**Backend consumption path:**

```
DeclarationModel ──→ DriverLoweringPlan.obligations
                          ↓
Item Hierarchy ────→ DriverLoweringPlan.behavior (SemanticModel)
                          ↓
                    LinuxBackendPlan / ZephyrBackendPlan
                          ↓
                    Emitted artifacts
```

Backends consume **only** `DriverLoweringPlan` + their own OS-specific plan.
They never import `DeclarationModel` or traverse the Item Hierarchy directly.
This is the litmus test from the lowering-plan-interface-boundary discussion:
backend planners must not re-walk the raw model to recover facts that should
have been in the plan.

This is also the frontend-independence boundary. By this point, Rust-specific
HIR/MIR details have already been normalized away; backends consume only
Regseq-owned items, semantic bodies, obligations, and backend plans.

### 8.4 Long-Term Coexistence

**Yes, DeclarationModel and the Item Hierarchy coexist long-term.** This is
not redundant because they serve fundamentally different purposes:

**DeclarationModel is construct-family-indexed.** It answers questions like:
- "What MMIO resources does this driver need?" → `resources.mmio`
- "What callbacks does this driver implement?" → `callbacks.lifecycle_impls`
- "What register blocks exist?" → `registers.blocks`
- "What properties are declared?" → `properties.fields`

This indexing is natural for **obligation planning** (determining what a
backend must acquire, register, and orchestrate). Obligation planning does not
need to know that `init` is the third method in the second impl block -- it
needs to know that there is a lifecycle init callback.

**The Item Hierarchy is item-indexed.** It answers questions like:
- "What items exist in this module?" → `module.items` resolved through `ItemStore`
- "What methods belong to this impl?" → `impl_def.methods`
- "What is the body of this function?" → `function_def.body`
- "What constants does this type own?" → `impl_def.constants`

This indexing is natural for **semantic body elaboration** and **backend
emission** (generating code that reflects the logical structure of the driver).

**Neither subsumes the other:**
- DeclarationModel cannot easily answer "what is the structured body of
  `set_baud_rate`?" (it has `MirBodyDeclSet` with raw MIR, not semantic bodies)
- The Item Hierarchy cannot easily answer "what is the total MMIO resource
  set across all driver struct fields?" (that requires a construct-family scan)

**Long-term convergence possibility:** The Item Hierarchy could eventually
absorb DeclarationModel's data as structured attributes on items (resource
obligations as attributes on struct fields, callback roles as attributes on
impl methods, etc.). If this convergence happens, DeclarationModel would become
a derived projection rather than a primary model. But this convergence should
only be pursued when there is concrete evidence of redundancy pain, not as a
premature unification.

### 8.5 Crate Placement

**Initial implementation:**

Place the semantic IR types and item hierarchy types in
`crates/regseq-backends/src/lowering/semantic_ir.rs`. This keeps them close to
the MIR interpreter that produces them and avoids a premature crate split.

**Architectural target:**

The item hierarchy and semantic IR conceptually belong in a model-level crate.
When the types stabilize, the natural options are:
- Expand `regseq-model` to hold both the declarative face (DeclarationModel)
  and the semantic face (Module/Item/SemanticBody)
- Introduce a `regseq-semantic` crate that depends on `regseq-model` for shared
  types (Provenance, etc.) and is depended on by `regseq-backends`

The first option is simpler (one model crate). The second is cleaner if the
two faces have different change velocities. Both are viable; the initial
placement in `regseq-backends` is pragmatic and non-blocking for either target.

---

## 9. Migration Strategy

### Core principle

The real semantic IR must be built from HIR declarative facts and MIR executable
facts directly. `BehaviorPlan` does not carry enough information to reconstruct
a proper local environment, local types, structural function ownership, or full
value semantics. A `BehaviorPlan -> SemanticModel` translator cannot be the
primary validation story.

### Phase 0: Enrich extraction and normalization inputs

Before semantic lowering, upstream extraction must be enriched so the compiler
actually carries the data this model depends on:
- `CanonicalPath` for every function, impl, constant, struct, and relevant field
- enough HIR structure to build Module/Item/Impl ownership
- explicit ISR marker capture
- associated-constant ownership
- resource field and register-block field canonical paths

This is a prerequisite phase, not an implementation detail hidden inside Phase 2.
Without it, the semantic model would be forced back into short-name matching.

### Phase 1: Introduce semantic IR types

Create `crates/regseq-backends/src/lowering/semantic_ir.rs` with:
- Item hierarchy types: Module, Item, StructDef, EnumDef, FunctionDef, ImplDef,
  ConstantDef, TypeAliasDef, ItemId, ItemStore
- Body types: SemanticBody, Block, Stmt, Expr, SemanticLocal, LocalId,
  LocalOrigin
- Type types: SemanticType
- Reference types: FunctionRef, DriverFieldRef, RegisterRef, RegisterFieldRef,
  InterruptClassRef, SubsystemActionRef, ConstantRef, ResourceRef,
  ResourceProperty, FieldWrite, FieldWriteValue
- SemanticModel as top-level container

Reuse from existing code: FunctionRole, InterruptClassDef, BinOp, ResourceKind.

No changes to existing pipeline. Pure type addition.

### Phase 2: Build semantic IR directly from MIR + HIR

Write a new `semantic_lowering` module that produces the Layer 2 item hierarchy
and its Layer 3 behavioral projection from:
- `AnalysisInput` (provides HIR-derived declarative facts)
- `MirBodyDeclSet` (provides MIR executable bodies)
- `MirInterpreterContext` (provides shared analysis context)

This module replaces the current `build_behavior_plan` path. It:
1. Builds the Layer 2 item hierarchy skeleton:
   - `Module`
   - canonical `ItemStore`
   - `FunctionDef`, `ImplDef`, `ConstantDef`, and minimal `StructDef`
     population for the first slice
2. Populates `FunctionDef.kind` and `role` from HIR-derived callback/ISR
   classification
3. Walks MIR bodies using a heavily revised `cfg_walker` path to produce
   `Block`/`Stmt`/`Expr`
4. Builds the local environment (`Vec<SemanticLocal>`) during walking, assigning
   names from MIR debug info and authored parameter names
5. Resolves all constant references to `ConstantRef` with full identity
6. Resolves body-level references (`FunctionRef`, `DriverFieldRef`,
   `RegisterRef`, `RegisterFieldRef`) from canonical identities rather than
   short names
7. Collapses wrapper patterns (Option chains, Try, barriers) into semantic constructs
8. Projects the backend-facing `SemanticModel` from the populated item hierarchy:
   - `functions`
   - `constants`
   - `interrupt_classes`
9. Runs semantic-IR validation passes, including:
   - `OptionUnwrap` domination validation
   - no unresolved/Opaque backend-facing types in supported bodies
   - no stringly cross-item references

This is closer to a structured lowering rewrite than an output-type swap. The
following components remain genuinely reusable:
- `call_classifier.rs` (CallClassification enum is IR-independent)
- `place_resolver.rs` (ResolvedPlace enum is IR-independent)
- `context.rs` (MirInterpreterContext / FnContext APIs unchanged)

And produces new types instead of old:
- `cfg_walker` -> returns `Block` instead of `Vec<BehaviorNode>`
- `value_lowering` -> returns `Expr` instead of `ValueExpr`/`BranchCondition`
- `cfg_helpers` -> returns `Vec<Stmt>` instead of `Vec<BehaviorNode>`

Every existing `BehaviorNode` family must have an explicit disposition during
this phase:
- `Delay` -> semantic delay/subsystem/resource operation or hard error
- `RegisterFlagCheck` -> `Expr` form in unified condition language
- `CleanupRegion` -> `FallibleOp.cleanup_on_error` or dedicated cleanup stmt
- `InterruptClassClear` -> explicit interrupt-clear lowering or helper call

### Phase 3: Update Linux emitter to consume SemanticModel

Rewrite `emit_behavior.rs` to walk `Stmt`/`Expr` trees:
- Emit local declarations from `SemanticBody.locals` at function start
- Walk `Stmt` tree for body emission
- `Expr` emission uses `SemanticLocal.ty` for type annotations (no more
  `infer_c_type`)
- `ConstantRef.stable_name` for constant emission (no more path parsing)

Update `emit_module.rs` to use `SemanticModel.functions` instead of
`BehaviorPlan.functions`.

### Phase 4: Remove BehaviorPlan

Delete: BehaviorNode, ValueExpr, BranchCondition, ReturnSemantic, AssignValue,
PortableType, FunctionBehavior, BehaviorPlan.

Change `DriverLoweringPlan.behavior` from `BehaviorPlan` to `SemanticModel`.

### Optional: BehaviorPlan translator as scaffold

A `BehaviorPlan -> SemanticModel` translator may be useful as a temporary test
scaffold during Phase 2 to compare outputs. But it is *not* the main migration
path because BehaviorPlan lacks:
- Proper local environment with types
- Structural function ownership (kind vs role)
- Full constant identity
- Principled resource property reads

The translator can only produce a degraded SemanticModel. The real validation
is that Phase 2's direct MIR+HIR -> SemanticModel produces well-formed output
that Phase 3's emitter can turn into correct C.

### Files to modify

| File | Phase | Scope |
|------|-------|-------|
| `lowering/semantic_ir.rs` (NEW) | 1 | All new types |
| `lowering/mod.rs` | 1 | Add `pub mod semantic_ir` |
| `lowering/semantic_lowering.rs` (NEW) | 2 | MIR+HIR -> SemanticModel |
| `lowering/mir_interpreter/cfg_walker.rs` | 2 | Adapt to produce Stmt/Expr |
| `lowering/mir_interpreter/value_lowering.rs` | 2 | Adapt to produce Expr |
| `lowering/mir_interpreter/cfg_helpers.rs` | 2 | Adapt to return Stmt |
| `linux/emit_behavior.rs` | 3 | Rewrite for Stmt/Expr |
| `linux/emit_module.rs` | 3 | Use SemanticModel |
| `lowering/behavior.rs` | 4 | Remove replaced types |
| `lowering/plan.rs` | 4 | Change behavior field type |

### Modules unchanged

| Module | Why |
|--------|-----|
| `call_classifier.rs` | CallClassification is IR-independent |
| `place_resolver.rs` | ResolvedPlace is IR-independent |
| `context.rs` | FnContext API stable |

---

## 10. Open Questions / Tradeoffs

### Tradeoff: Item hierarchy now vs later

**Chosen: Define types now, populate minimally.** The item hierarchy types
(Module, StructDef, FunctionDef, etc.) are defined in Phase 1. The first
implementation slice populates only FunctionDef (with bodies), ConstantDef
(for referenced constants), and minimal StructDef/ImplDef for ownership context.
Full population (EnumDef, TypeAliasDef, all struct fields) comes when needed.

### Tradeoff: Replace vs extend BehaviorPlan

**Chosen: Replace.** The BehaviorNode/ValueExpr conflation of statements and
expressions is structural. The absence of a local environment is fundamental.
The domain vocabulary (RegWrite, Branch, HelperCall) is preserved in the new
Stmt/Expr types.

### Open: Option chain collapse generality

The MIR study shows `as_ref().map(f).unwrap_or(default)` for clock rate reads.
`ResourcePropertyRead + WithDefault` handles this. Deeper chains (nested maps,
and_then, etc.) may need a richer combinator set. Defer until evidence emerges
from non-PL011 driver specs.

### Open: Structured option binding form

The current plan uses `IsSome + OptionUnwrap + validation` rather than a
dedicated match/bind statement. This is sufficient for the current slice and
keeps the expression language compact. If future drivers require richer nested
pattern binding, loop-carried option payloads, or multi-arm option matching, a
structural form such as `MatchOption` can be added without changing the rest of
the item hierarchy or backend boundary.

### Open: Condition-bound option binding

The current IR uses `IsSome(cond)` plus `OptionUnwrap(cond)` with a validation
invariant. If future drivers need single-evaluation condition binding
(especially loop-carried binding from a condition expression), a structural
`MatchOption` or `LetSome` form may be added. The current PL011 slice does not
require that stronger construct.

### Open: Interrupt class naming source

Interrupt class names are currently derived from MIR constants (mask values or
constant paths). For stability, class names should be declared at the HIR level
(e.g., via attributes on ISR dispatch logic). This is a declaration-model
concern, not a procedural-IR concern, but it affects how InterruptClassDef
is populated.

### Open: Error type model for FallibleOp

The current plan adds `success_local: Option<LocalId>` so the first
implementation slice can handle both:
- `Result<(), E>` flows (`success_local = None`)
- `let x = foo()?;` flows (`success_local = Some(local_id)`)

Richer error payload modeling beyond the carried error local remains deferred.

### Open: Multi-module driver support

The first implementation slice uses flat modules (one level per driver spec).
Recursive module nesting is intentionally deferred, not intentionally excluded.
When multi-module support is needed, `Module` gains a `submodules: Vec<Module>`
field or `Item::Module(Module)` is added to the Item enum. `CanonicalPath`
already supports arbitrary depth, so cross-references remain unambiguous.

### Open: Crate split timing

The semantic IR is conceptually model-level but initially placed in
regseq-backends. The crate split to regseq-semantic or expanded regseq-model
should happen when either: (a) a second backend needs the IR, or (b) the item
hierarchy types become shared across compilation phases.

---

## Verification

Since this is a research/design deliverable, verification means:

1. **Type-level completeness**: Every PL011 function body maps to the Stmt/Expr
   language without gaps (Section 4.13)
2. **MIR normalization completeness**: Every MIR construct has an explicit
   semantic mapping or hard-error disposition (Section 6)
3. **Backend sufficiency**: The emitter can walk Stmt/Expr trees with typed
   locals and produce well-formed C declarations, expressions, and statements
4. **No MIR leakage**: No raw MIR locals, Rust paths, or wrapper syntax visible
   to backends (Section 7)
5. **Ownership separation**: FunctionKind and FunctionRole are orthogonal and
   independently meaningful (Section 3.1)
6. **Model layering coherence**: Each concern has exactly one authoritative layer,
   the cross-reference story is explicit, and backends consume only
   DriverLoweringPlan (Section 8)
7. **Reference identity coherence**: Function, field, register, and helper
   references in semantic bodies use structured refs, not bare names
8. **OptionUnwrap safety**: Semantic validation rejects any unwrap not
   dominated by a matching presence test

When implementation begins:
- Phase 2 produces SemanticModel from MIR+HIR with no `Opaque` fallbacks for
  pl011-full-spec bodies
- Phase 3 emitter produces C output with:
  - All locals declared before use
  - No undeclared temporaries
  - No malformed returns
  - No Rust paths
  - Correct typed register reads bound to locals
- E2E test `pl011_driver_emission` passes end-to-end
- Acceptance examples include OrderedRegion, RegModify, and SubsystemAction
