# Regseq IR Reference

This document defines the concrete IR constructs used in `regseq_model::ir`.
It complements the high-level design in [spec.md](spec.md) (Section 9: Bodies)
with precise definitions, MIR mappings, and design rationale.

The IR is constructed from rustc's MIR at the
`mir_drops_elaborated_and_const_checked` stage with `mir_opt_level = 0`,
using release profile semantics (`overflow-checks = false`,
`debug-assertions = false`). This means:
- Arithmetic uses `Add`/`Sub`/`Mul` (not `AddWithOverflow` etc.)
- `Assert` terminators do not appear for arithmetic overflow
- Bounds-check `Assert` terminators do not appear for array indexing
- No debug-assertion code paths in MIR

The compiler invokes rustc with these settings explicitly. The pinned
nightly version is `nightly-2026-02-01`.

The IR is a domain-specific IR -- not a MIR clone -- that preserves CFG
structure, adds augmented operations for Regseq domain concepts, and
drops Rust-specific artifacts that have no meaning in the target output.

---

## 1. Operands

An operand is a value source for expressions. It is the atomic unit that
flows into rvalues, statements, and terminators.

### Definition

```rust
/// An operand in the Regseq IR -- a value source for expressions.
pub enum Operand {
    /// Load a value from a place (bitwise copy).
    ///
    /// In MIR, `Copy` requires the place's type to be `Copy` before drop
    /// elaboration. At our extraction stage (post-drop-elaboration), this
    /// restriction is relaxed. Preserved in the Regseq IR for analysis
    /// (distinguishing copies from moves) and for C++/Rust backends where
    /// the distinction affects output semantics.
    Copy(Place),

    /// Load a value from a place, invalidating the source.
    ///
    /// The source place should not be read again without re-initialization.
    /// Backends may use this to emit `std::move()` (C++) or move semantics
    /// (Rust). The C backend treats this identically to `Copy`.
    Move(Place),

    /// Reference to a named constant declaration.
    ///
    /// Maps from MIR's `Constant(ConstOperand)` when the constant is
    /// unevaluated (e.g., `Self::RX_INTERRUPT_MASK`). The `ConstantId`
    /// preserves the constant's identity so backends can emit the symbolic
    /// name rather than an inlined value.
    Constant(ConstantId),

    /// Inline scalar or boolean value.
    ///
    /// Maps from MIR's `Constant(ConstOperand)` when the constant has been
    /// evaluated to a concrete scalar (e.g., `0x07ff`, `true`, `32u8`).
    /// These carry their value directly -- no identity, no lookup needed.
    Literal(LiteralValue),
}
```

### LiteralValue

```rust
/// Inline constant values that appear directly in expressions.
///
/// Each variant carries its value and implies a `RegseqType`:
/// `U32(v)` has type `Scalar(U32)`, `Bool(v)` has type `Bool`.
pub enum LiteralValue {
    U8(u8),
    U16(u16),
    U32(u32),
    U64(u64),
    Usize(u64),
    I8(i8),
    I16(i16),
    I32(i32),
    I64(i64),
    Isize(i64),
    Bool(bool),
}
```

`Unit` and `Never` do not have literal representations. `Unit` is structural
(empty tuple), and `Never` is a control-flow property, not a value.

`Usize(u64)` and `Isize(i64)` use 64-bit backing storage because the
actual width is target-dependent. The backend narrows to the target's
pointer width (from the platform file's `arch` field). During Phase 2
join, the compiler validates that `Usize` values fit within the
target's pointer width (e.g., a value exceeding `u32::MAX` on a 32-bit
target is a hard error). In practice this is unlikely -- `usize`
literals in driver specs are small values (array lengths, offsets, loop
bounds) that rustc itself constrains before MIR extraction.

### MIR mapping

| MIR `Operand<'tcx>`                                  | Regseq IR `Operand`     | Notes                                                      |
| ---------------------------------------------------- | ----------------------- | ---------------------------------------------------------- |
| `Copy(Place<'tcx>)`                                  | `Copy(Place)`           | Place preserved with projections. No lifetime.             |
| `Move(Place<'tcx>)`                                  | `Move(Place)`           | Preserves move semantics for analysis and backends.        |
| `Constant(Box<ConstOperand>)` with unevaluated const | `Constant(ConstantId)`  | Named constants (e.g., `Self::MASK`) resolved to typed ID. |
| `Constant(Box<ConstOperand>)` with evaluated scalar  | `Literal(LiteralValue)` | Inline values. No identity needed.                         |
| `RuntimeChecks(RuntimeChecks)`                       | Dropped                 | Compilation flag query. Not relevant for driver specs.     |

### Design rationale

**Why split MIR's `Constant` into two variants?** MIR uses a single
`Constant(ConstOperand)` for both named constants and inline values. The
Regseq IR splits them because they serve different purposes:

- `Constant(ConstantId)` preserves identity. The backend emits
  `RX_INTERRUPT_MASK` in the generated code, not `0x07ff`. The identity
  also matters for analyses that track which constants are used where.
- `Literal(LiteralValue)` is just a value. No name, no lookup, no
  indirection. It's `Copy` and as small as an ID would be.

**Why keep both `Copy` and `Move`?** The C backend ignores the distinction
(both are value copies in C). But:

- C++ backend: `Move` maps to `std::move()`.
- Rust backend: `Move` preserves Rust's move semantics.
- Analysis: use-after-move detection, resource lifecycle tracking.
- Cost: one enum tag. Dropping it loses information that is cheap to carry
  and expensive to recover (would require re-running type analysis to
  determine which types are `Copy`).

**Why `Place` and not `LocalId` in `Copy`/`Move`?** Place carries a local
plus a projection chain (field access, deref, indexing). Using bare `LocalId`
would require decomposing every compound access into temporary locals,
creating more statements and more locals. More critically, `Place` is needed
for the lvalue (write) side of assignments -- you can't write to a field
through a temporary. Using `Place` uniformly in operands avoids a hybrid
model and is familiar to anyone working with MIR.

**Why `Place` and `LiteralValue` are inline, not behind IDs:** Neither has
meaningful identity. `Place` is a small structural path (local + 0-2
projections). `LiteralValue` is a tag + scalar. Both are cheap to clone and
don't benefit from deduplication at our scale (one driver spec at a time).
IDs are reserved for named entities (constants, functions, types, resources).

---

## 2. Places and Projections

A place is a memory location: a local variable plus an optional chain of
projections that navigate into the local's value (dereference, field access,
indexing, etc.). Places appear on both sides of assignments -- as read sources
(inside operands) and as write targets (the left side of `Assign`).

### Place

```rust
/// A memory location: a local variable plus an optional chain of projections.
///
/// Maps directly from MIR's `Place<'tcx>`. The projection list is owned
/// (not arena-interned) since the Regseq IR uses owned types throughout.
///
/// An empty projection Vec means the place is just the local itself.
pub struct Place {
    /// The base local variable.
    pub local: LocalId,
    /// Zero or more projections applied to the local.
    pub projection: Vec<Projection>,
}
```

### Projection

```rust
/// A single projection step applied to a place.
pub enum Projection {
    /// Dereference a pointer or reference.
    ///
    /// Loads the pointer value from the parent place and dereferences it.
    /// Only valid when the parent place is a reference, raw pointer, or Box.
    ///
    /// Common in driver code: `self` is `&mut DriverStruct`, so any field
    /// access on self begins with `Deref`. For example, `self.uart_clk`
    /// is Place { local: _1, projection: [Deref, Field { index: 2, ... }] }.
    Deref,

    /// Access a field of a struct, tuple, or enum variant.
    ///
    /// Carries the field index, the field's name (when available), and the
    /// field's type. The name and type are provided directly so consumers
    /// (backends, diagnostics) don't need a lookup to produce readable
    /// output or determine the result type of the projection.
    ///
    /// `name` is `None` for tuple struct fields (positional: `.0`, `.1`)
    /// and compiler-generated fields.
    Field {
        index: u32,
        name: Option<Symbol>,
        ty: RegseqType,
    },

    /// Runtime index into an array or slice.
    ///
    /// The index local must have type `usize`. Note that this does NOT
    /// auto-dereference -- indexing a slice requires a `Deref` projection
    /// first, then `Index`. Used in driver code for array register access,
    /// interrupt array indexing, etc.
    Index(LocalId),

    /// Compile-time index generated by array/slice patterns.
    ///
    /// `from_end: false`: `offset` is 0-based from the start.
    /// `from_end: true`: `offset` is 1-based from the end.
    /// `min_length`: the array/slice must be at least this long.
    ///
    /// Rare in driver code but retained for completeness.
    ConstantIndex {
        offset: u64,
        min_length: u64,
        from_end: bool,
    },

    /// Slice subrange generated by slice patterns.
    ///
    /// `from_end: false`: `array[from..to]`.
    /// `from_end: true`: `slice[from..slice.len() - to]`.
    ///
    /// Rare in driver code but retained for completeness.
    Subslice {
        from: u64,
        to: u64,
        from_end: bool,
    },

    /// Select an enum variant before accessing its fields.
    ///
    /// Must precede any `Field` projection on an enum-typed place. The
    /// name is the variant's identifier (for diagnostics and display),
    /// the index is the variant's position in the enum definition.
    ///
    /// Essential for `Result`/`Option` pattern matching and any
    /// data-carrying enum access in driver code.
    Downcast {
        name: Option<Symbol>,
        variant_index: u32,
    },
}
```

### MIR mapping

| MIR `ProjectionElem`                             | Regseq IR `Projection`                           | Notes                                                                                                                      |
| ------------------------------------------------ | ------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------- |
| `Deref`                                          | `Deref`                                          | Direct mapping.                                                                                                            |
| `Field(FieldIdx, Ty<'tcx>)`                      | `Field { index, name, ty }`                      | Type mapped to `RegseqType`. Name added as interned `Symbol` (not in MIR) -- resolved during IR construction. |
| `Index(Local)`                                   | `Index(LocalId)`                                 | Direct mapping.                                                                                                            |
| `ConstantIndex { offset, min_length, from_end }` | `ConstantIndex { offset, min_length, from_end }` | Direct mapping. Rare in driver specs.                                                                                      |
| `Subslice { from, to, from_end }`                | `Subslice { from, to, from_end }`                | Direct mapping. Rare in driver specs.                                                                                      |
| `Downcast(Option<Symbol>, VariantIdx)`           | `Downcast { name, variant_index }`               | rustc `Symbol` becomes interned `Option<Symbol>`.                                                                          |
| `OpaqueCast(Ty<'tcx>)`                           | Dropped                                          | Rust-specific opaque type mechanism (`impl Trait`). Not relevant for driver specs. Being phased out within rustc itself.   |
| `UnwrapUnsafeBinder(Ty<'tcx>)`                   | Dropped                                          | Unsafe binder transmute. Niche feature, not relevant for driver specs.                                                     |

### Design rationale

**Six of eight projections kept.** The six retained projections are the ones
with semantic meaning in any C-like target language: dereference, field access,
runtime indexing, compile-time indexing, slice subrange, and variant selection.
`ConstantIndex` and `Subslice` are rare in driver code (generated by slice
patterns) but cheap to carry and preserve information that would be hard to
recover if dropped.

**Two dropped.** `OpaqueCast` is a Rust-specific mechanism for opaque types
(`impl Trait`) that is being phased out within rustc (unused with
`-Znext-solver`). `UnwrapUnsafeBinder` is for a niche unsafe feature. Neither
is expected in driver specs. If a future spec triggers one, the compiler
emits a hard error identifying the unrecognized MIR construct and stops.

**`Field` carries `name` (not in MIR).** MIR's `Field(FieldIdx, Ty)` carries
only the index and type -- the name must be resolved from the type definition.
The Regseq IR adds the field name directly on the projection so that:
- Backends emit readable code (`config->baud_rate` not `config->field_3`)
  without a type-definition lookup.
- IR dumps and diagnostics are human-readable by default.
- The name is resolved once during IR construction, not repeatedly by every
  consumer.

The name is `Option<Symbol>` because tuple struct fields are positional (no
name) and some compiler-generated fields may lack names.

**`Field` carries `RegseqType` (matching MIR).** MIR carries the field's type
on every Field projection. We follow the same pattern so the result type of
the projection is immediately available. This costs one `RegseqType` per Field
projection but saves every consumer from needing to resolve the type through
the parent type's definition.

**Place projections stay general-purpose.** During IR construction, when the
compiler encounters `self.field(N)` where field N is a resource, the augmented
operation (RegisterRead, IntrinsicCall) carries the `ResourceId` -- the Place
is not annotated with domain information. Place describes the structural memory
path; domain resolution happens in the operations that use places.

---

## 3. Rvalues

An rvalue is a value-producing expression that appears on the right side of
an `Assign` statement. Rvalues in the Regseq IR are **purely standard** --
arithmetic, logic, casts, aggregates, references. All domain-specific
operations (register access, intrinsic calls, sync operations) flow through
`IntrinsicCall` terminators (see Section 4), not rvalues.

### Definition

```rust
/// A value-producing expression on the right side of an assignment.
pub enum Rvalue {
    /// Pass an operand through unchanged.
    Use(Operand),

    /// Array repetition: `[operand; count]`.
    ///
    /// Maps from MIR's `Repeat(Operand, Const)` with the const resolved
    /// to a concrete `usize`.
    Repeat(Operand, usize),

    /// Create a reference to a place.
    ///
    /// Maps from MIR's `Ref(Region, BorrowKind, Place)`. The `Region`
    /// (lifetime) is dropped -- it is a borrow-checker artifact with no
    /// meaning after type checking. Borrow kind is simplified to
    /// Shared/Mutable.
    Ref(BorrowKind, Place),

    /// Type cast.
    ///
    /// Maps from MIR's `Cast(CastKind, Operand, Ty)` with the type
    /// mapped to `RegseqType` and `CastKind` simplified (see below).
    Cast(CastKind, Operand, RegseqType),

    /// Binary operation.
    ///
    /// All 26 MIR `BinOp` variants are defined in the enum for
    /// completeness. Under release profile semantics, `*WithOverflow`
    /// variants do not appear in practice (see preamble). `*Unchecked`
    /// variants carry distinct UB semantics for analysis.
    BinaryOp(BinOp, Operand, Operand),

    /// Unary operation.
    UnaryOp(UnOp, Operand),

    /// Read the discriminant (variant tag) of an enum-typed place.
    ///
    /// Returns an integer of the enum's discriminant type. Essential
    /// for enum pattern matching: the result feeds into `SwitchInt`
    /// to branch on the variant.
    Discriminant(Place),

    /// Construct an aggregate value (struct, enum variant, tuple, array).
    ///
    /// The operands are the field/element values in declaration order.
    Aggregate(AggregateKind, Vec<Operand>),
}
```

### BorrowKind

```rust
/// Borrow kind for the `Ref` rvalue.
pub enum BorrowKind {
    /// Shared (immutable) borrow: `&T`.
    Shared,
    /// Mutable borrow: `&mut T`.
    Mutable,
}
```

MIR's `BorrowKind` also has `Fake` (a borrow-checker marker) and sub-variants
of `Mut` (two-phase borrows). These are borrow-checker artifacts dropped in the
Regseq IR.

### BinOp

```rust
/// Binary operator.
pub enum BinOp {
    // Arithmetic -- wrapping
    Add,
    Sub,
    Mul,
    Div,                // UB on integer division by zero; safe for floats
    Rem,                // UB on integer zero modulus; safe for floats

    // Arithmetic -- UB on overflow (unchecked)
    AddUnchecked,
    SubUnchecked,
    MulUnchecked,

    // Arithmetic -- returns (result, overflow_bool)
    AddWithOverflow,
    SubWithOverflow,
    MulWithOverflow,

    // Bitwise
    BitXor,
    BitAnd,
    BitOr,

    // Shifts -- offset modulo LHS::BITS
    Shl,
    Shr,                // arithmetic if signed, logical if unsigned

    // Shifts -- UB if RHS >= LHS::BITS
    ShlUnchecked,
    ShrUnchecked,

    // Comparison -- all produce Bool
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,

    // Three-way comparison -- returns Ordering (-1, 0, 1)
    Cmp,

    // Pointer arithmetic
    Offset,
}
```

### UnOp

```rust
/// Unary operator.
pub enum UnOp {
    /// Logical/bitwise negation. Accepts bool, signed/unsigned integers.
    Not,
    /// Arithmetic negation. Accepts signed integers and floats.
    Neg,
}
```

MIR's `PtrMetadata` variant is dropped -- it is only permitted in
`MirPhase::Runtime`, which is after our extraction stage.

### CastKind

```rust
/// Type cast kind.
pub enum CastKind {
    /// Integer to integer (widening, narrowing, sign conversion).
    IntToInt,
    /// Integer to float.
    IntToFloat,
    /// Float to integer.
    FloatToInt,
    /// Float to float (precision change).
    FloatToFloat,
    /// Pointer to pointer.
    PtrToPtr,
    /// Reinterpret bits as a different type.
    Transmute,
}
```

### AggregateKind

```rust
/// Aggregate constructor kind.
pub enum AggregateKind {
    /// Array literal. Carries the element type.
    Array(RegseqType),
    /// Tuple literal.
    Tuple,
    /// Struct or enum variant construction.
    ///
    /// `variant_index` is 0 for structs, the variant position for enums.
    Adt {
        type_id: TypeDefId,
        variant_index: u32,
    },
}
```

### MIR mapping -- Rvalue

| MIR `Rvalue<'tcx>`               | Regseq IR `Rvalue`                       | Notes                                                           |
| -------------------------------- | ---------------------------------------- | --------------------------------------------------------------- |
| `Use(Operand)`                   | `Use(Operand)`                           | Direct.                                                         |
| `Repeat(Operand, Const)`         | `Repeat(Operand, usize)`                 | Const resolved to concrete `usize`.                             |
| `Ref(Region, BorrowKind, Place)` | `Ref(BorrowKind, Place)`                 | Region (lifetime) dropped. BorrowKind simplified.               |
| `Cast(CastKind, Operand, Ty)`    | `Cast(CastKind, Operand, RegseqType)`    | CastKind simplified, Ty mapped to RegseqType.                   |
| `BinaryOp(BinOp, Box<(Op, Op)>)` | `BinaryOp(BinOp, Operand, Operand)`      | Unboxed. All 26 BinOp variants kept.                            |
| `UnaryOp(UnOp, Operand)`         | `UnaryOp(UnOp, Operand)`                 | PtrMetadata dropped (Runtime-only).                             |
| `Discriminant(Place)`            | `Discriminant(Place)`                    | Direct.                                                         |
| `Aggregate(AggregateKind, ops)`  | `Aggregate(AggregateKind, Vec<Operand>)` | AggregateKind simplified.                                       |
| `ThreadLocalRef(DefId)`          | Dropped                                  | Thread locals not used in driver specs.                         |
| `RawPtr(RawPtrKind, Place)`      | Dropped                                  | Raw pointer construction not in current specs. Add when needed. |
| `ShallowInitBox(Operand, Ty)`    | Dropped                                  | Box-specific transmute. Not relevant.                           |
| `CopyForDeref(Place)`            | Replaced with `Use(Copy(place))`         | Semantically equivalent. Internal MIR detail.                   |
| `WrapUnsafeBinder(Operand, Ty)`  | Dropped                                  | Unsafe binder feature. Not relevant.                            |

### MIR mapping -- CastKind

| MIR `CastKind`                 | Regseq IR `CastKind` | Notes                                                      |
| ------------------------------ | -------------------- | ---------------------------------------------------------- |
| `IntToInt`                     | `IntToInt`           | Direct. Most common cast in driver code.                   |
| `IntToFloat`                   | `IntToFloat`         | Direct.                                                    |
| `FloatToInt`                   | `FloatToInt`         | Direct.                                                    |
| `FloatToFloat`                 | `FloatToFloat`       | Direct.                                                    |
| `PtrToPtr`                     | `PtrToPtr`           | Direct.                                                    |
| `Transmute`                    | `Transmute`          | Direct. May appear in low-level driver code.               |
| `PointerExposeProvenance`      | Dropped              | Provenance model. No meaning in C output.                  |
| `PointerWithExposedProvenance` | Dropped              | Provenance model.                                          |
| `PointerCoercion(_, _)`        | Map to `PtrToPtr`    | Analysis-phase coercions. Should be PtrToPtr by our stage. |
| `FnPtrToPtr`                   | Dropped              | Function pointer cast. Add when needed.                    |
| `Subtype`                      | Dropped              | Internal subtyping adjustment. No runtime effect.          |

### MIR mapping -- AggregateKind

| MIR `AggregateKind<'tcx>`     | Regseq IR `AggregateKind`        | Notes                                                              |
| ----------------------------- | -------------------------------- | ------------------------------------------------------------------ |
| `Array(Ty)`                   | `Array(RegseqType)`              | Direct.                                                            |
| `Tuple`                       | `Tuple`                          | Direct.                                                            |
| `Adt(DefId, VariantIdx, ...)` | `Adt { type_id, variant_index }` | DefId resolved to TypeDefId. Generic args dropped (monomorphized). |
| `Closure(DefId, _)`           | Dropped                          | See closure handling below. Add when needed.                       |
| `Coroutine(_, _)`             | Dropped                          | Async/coroutine. Not used in driver specs.                         |
| `CoroutineClosure(_, _)`      | Dropped                          | Async/coroutine. Not used in driver specs.                         |
| `RawPtr(Ty, Mutability)`      | Dropped                          | Raw pointer aggregate. Not in current specs.                       |

### Design rationale

**Rvalue is purely standard.** All domain-specific operations (register access,
intrinsic calls, sync operations) flow through `IntrinsicCall` terminators
(Section 4). This gives a clean separation: Rvalue computes values from
operands; domain operations are call-shaped constructs with explicit control
flow edges (including error paths for fallible operations).

**All 26 BinOp variants kept.** The `*Unchecked` variants (UB on overflow)
and `*WithOverflow` variants (return `(result, bool)`) are retained in the
enum definition for completeness, but `*WithOverflow` variants do not
appear in practice because extraction uses release profile semantics
(`overflow-checks = false`). `Cmp` (three-way comparison) and `Offset`
(pointer arithmetic) are rare in driver code but cheap to carry.

**CastKind reduced from 11 to 6.** Provenance casts have no meaning in C output.
`PointerCoercion` should be lowered to `PtrToPtr` by our MIR stage. `Subtype`
has no runtime effect. `FnPtrToPtr` can be added when needed.

**AggregateKind reduced from 7 to 3.** Closures, coroutines, and `RawPtr`
are not represented as aggregate kinds. See below for closure handling.

**Closures deferred.** In current driver specs, closures appear in two
contexts: (1) predicates passed to `poll::until`, and (2) arguments to
standard library generics like `Option::map`. Neither requires a
general-purpose closure construct in the IR:

- **`poll::until` predicates**: The compiler recognizes `poll::until` as a
  Regseq intrinsic. The closure body is extracted as a separate `Function`
  and referenced by the `IntrinsicCall` for `PollUntil`. The closure
  aggregate and `FnMut::call_mut` dispatch are eliminated.
- **Standard library generics** (`Option::map`, `Iterator::filter`, etc.):
  These are monomorphized and inlined during IR construction. After
  inlining, closure calls resolve to direct calls to the closure's body
  function, which is a normal `Function` in the IR.

In both cases, the closure dissolves into constructs the IR already has:
a `Function` (the body) and, if capturing, an `Adt` aggregate (the
captured state). If a future driver spec requires closures as first-class
values (stored, returned, passed through non-inlined paths), add
`Closure(TypeDefId)` back to `AggregateKind`.

**CopyForDeref replaced during construction.** It's semantically equivalent to
`Use(Copy(place))` -- an internal MIR optimization detail for the deref
separator pass. No consumer would handle it differently from `Use`.

---

## 4. Statements

A statement is an instruction within a basic block that does not transfer
control flow. Statements execute sequentially. A basic block contains zero
or more statements followed by exactly one terminator.

A `Statement` wraps a `StatementKind` with an optional source location
for diagnostics (see Section 6: Body Structure).

### StatementKind

```rust
/// The content of a statement -- what it does.
pub enum StatementKind {
    /// Assignment: evaluate the rvalue and store the result in the place.
    ///
    /// The workhorse of the IR. Every value computation flows through
    /// Assign. The place and rvalue types must match.
    Assign(Place, Rvalue),

    /// Write the discriminant tag for an enum variant.
    ///
    /// Sets the variant of an enum-typed place without writing the
    /// variant's fields. Used together with field assignments to
    /// construct enum values (e.g., setting a Result to Ok or Err
    /// before populating the inner value).
    SetDiscriminant {
        place: Place,
        variant_index: u32,
    },

    /// Mark a local's stack storage as allocated.
    ///
    /// After this statement, the local is live and may be written to.
    /// Locals without StorageLive/StorageDead are implicitly live for
    /// the full duration of the function.
    ///
    /// Retained for future analyses (liveness, scope reconstruction)
    /// and for backends that benefit from explicit stack lifetime
    /// information (C++, Rust).
    StorageLive(LocalId),

    /// Mark a local's stack storage as freed.
    ///
    /// After this statement, the local is dead. Using it without a
    /// subsequent StorageLive is undefined behavior. Paired with
    /// StorageLive.
    StorageDead(LocalId),

    /// MIR-level non-diverging intrinsic.
    ///
    /// These are compiler-level intrinsics carried from MIR, distinct
    /// from Regseq domain intrinsics (which flow through
    /// `IntrinsicCall` terminators). Named `MirIntrinsic` to avoid
    /// confusion with `RegseqIntrinsic`.
    MirIntrinsic(MirIntrinsic),
}
```

### MirIntrinsic

```rust
/// Non-diverging intrinsics carried from MIR.
///
/// These are MIR-level operations that always complete and never
/// transfer control flow. They are distinct from Regseq domain
/// intrinsics (`RegseqIntrinsic`), which represent OS-abstracted
/// driver operations and flow through `IntrinsicCall` terminators.
pub enum MirIntrinsic {
    /// Optimizer hint: assume this boolean is true at this point.
    ///
    /// No runtime effect. If the operand is false, behavior is
    /// undefined (equivalent to Unreachable). Optimizers and analyses
    /// may use the assumption to prune impossible paths or elide
    /// bounds checks.
    Assume(Operand),

    /// Copy elements from source to destination (non-overlapping).
    ///
    /// Copies `count * size_of::<T>()` bytes from `src` to `dst`.
    /// The regions must not overlap (use memmove for overlapping).
    /// Equivalent to C's `memcpy(dst, src, count * sizeof(T))`.
    ///
    /// May appear in driver code for DMA buffer setup, console write
    /// implementations, firmware data loading, etc.
    CopyNonOverlapping {
        src: Operand,
        dst: Operand,
        count: Operand,
    },
}
```

### MIR mapping

| MIR `StatementKind<'tcx>` | Regseq IR `Statement` | Notes |
|---|---|---|
| `Assign(Box<(Place, Rvalue)>)` | `Assign(Place, Rvalue)` | Unboxed. Place and Rvalue mapped to Regseq IR types. |
| `SetDiscriminant { place, variant_index }` | `SetDiscriminant { place, variant_index }` | Direct. Place mapped to Regseq IR Place. |
| `StorageLive(Local)` | `StorageLive(LocalId)` | Direct. Retained for future analyses and backend flexibility. |
| `StorageDead(Local)` | `StorageDead(LocalId)` | Direct. Paired with StorageLive. |
| `Intrinsic(Box<NonDivergingIntrinsic>)` | `MirIntrinsic(MirIntrinsic)` | Renamed to `MirIntrinsic` to distinguish from `RegseqIntrinsic`. Both sub-variants (`Assume`, `CopyNonOverlapping`) kept. |
| `FakeRead(...)` | Dropped | Disallowed after drop elaboration. Won't appear at our MIR stage. |
| `AscribeUserType(...)` | Dropped | Disallowed after drop elaboration. Won't appear at our MIR stage. |
| `Retag(...)` | Dropped | Stacked Borrows model. Only generated with `-Z mir-emit-retag`. |
| `PlaceMention(...)` | Dropped | Lint trace. Side effects of place computation captured by intrinsic calls. |
| `Coverage(...)` | Dropped | Coverage instrumentation. Not relevant for driver code generation. |
| `ConstEvalCounter` | Dropped | Const eval step counter. Nop at runtime. |
| `Nop` | Dropped | No reason to preserve no-ops in the IR. |
| `BackwardIncompatibleDropHint { ... }` | Dropped | Lint marker. Semantically Nop. |
### Design rationale

**Five statement variants.** Each has a clear reason to exist:

- **`Assign`**: Fundamental. Every value computation in the IR flows through
  assignment. Without it, nothing produces values.
- **`SetDiscriminant`**: Essential for constructing enum values (`Result`,
  `Option`, user-defined enums). Sets the variant tag before field writes.
- **`StorageLive` / `StorageDead`**: Retained even though C backends don't
  need them. Future analyses may use liveness information, and C++/Rust
  backends benefit from explicit stack lifetime for scope reconstruction
  and RAII patterns.
- **`MirIntrinsic`**: Carries compiler-level intrinsics (`assume`, `memcpy`)
  that are not Regseq domain operations. Named distinctly from
  `RegseqIntrinsic` to prevent confusion -- `MirIntrinsic` is a statement
  (non-diverging, no control flow), while `RegseqIntrinsic` flows through
  `IntrinsicCall` terminators (with error paths and domain metadata).
**Eight MIR variants dropped.** Two are disallowed at our MIR stage
(`FakeRead`, `AscribeUserType`). The rest are either borrow-checker
artifacts (`Retag`), instrumentation (`Coverage`), const-eval internals
(`ConstEvalCounter`), lint markers (`PlaceMention`, `BackwardIncompatibleDropHint`),
or vacuous (`Nop`). None carries semantic information needed by analyses,
transformations, or backends.

**`MirIntrinsic` vs. `RegseqIntrinsic` naming.** The two intrinsic namespaces
serve different purposes:

| | `MirIntrinsic` | `RegseqIntrinsic` |
|---|---|---|
| Origin | Rust compiler intrinsics | Regseq DSL domain operations |
| IR position | Statement (always completes, no control flow) | Terminator (with error_target for fallible operations) |
| Examples | `assume`, `copy_nonoverlapping` | `ClockEnable`, `RegisterReadField`, `MutexLock` (future) |
| Backend lowering | `memcpy`, optimizer hints | OS-specific API calls (`clk_enable`, `readl`, `mutex_lock`) |

---

## 5. Terminators

A terminator is the final instruction of a basic block. It transfers control
flow to one or more successor blocks. Every basic block has exactly one
terminator.

A `Terminator` wraps a `TerminatorKind` with an optional source location
for diagnostics (see Section 6: Body Structure).

### TerminatorKind

```rust
/// The content of a terminator -- how it transfers control flow.
pub enum TerminatorKind {
    /// Unconditional jump to a single successor block.
    Goto(BasicBlockId),

    /// Multi-way branch on an integer discriminant.
    ///
    /// Evaluates the discriminant operand and branches to the matching
    /// target. If no value matches, branches to the default target.
    /// Used for if/else (bool switch), match arms (enum discriminant),
    /// and integer switches.
    SwitchInt {
        discriminant: Operand,
        targets: SwitchTargets,
    },

    /// User-defined function call.
    ///
    /// The callee is opaque to the compiler -- it might diverge (never
    /// return). `target` is `None` when the function is known to diverge.
    ///
    /// `error_target` is populated during IR construction when the call
    /// site uses `?`. The IR constructor recognizes the `Try::branch`
    /// + `SwitchInt` + `FromResidual` desugaring (for any `Try`
    /// implementor) and folds it: `target` receives the Continue path,
    /// `error_target` receives the Break path (which contains the Drop
    /// cascade for live locals, as generated by Rust's drop
    /// elaboration). `None` if the call's result is not checked with `?`.
    Call {
        callee: FunctionId,
        args: Vec<Operand>,
        destination: Place,
        target: Option<BasicBlockId>,
        error_target: Option<BasicBlockId>,
    },

    /// Regseq intrinsic call.
    ///
    /// The compiler knows the intrinsic's semantics -- it always returns.
    /// `target` is always set (not `Option`).
    ///
    /// For user-extracted code: `error_target` is populated during IR
    /// construction when the intrinsic call site uses `?`. The IR
    /// constructor recognizes Rust's `Try::branch` + `SwitchInt` +
    /// `FromResidual` desugaring and folds it into the two edges.
    /// The error path contains the Drop cascade for live locals, as
    /// generated by Rust's drop elaboration.
    ///
    /// For compiler-generated code (pre-init, framework integration):
    /// the compiler sets `error_target` directly when constructing
    /// the IR for generated function bodies.
    ///
    /// `error_target` is `None` for infallible intrinsics (register
    /// access, logging, delays).
    ///
    /// `metadata` carries intrinsic-specific structured data that does
    /// not fit into the standard args. Currently used by logging
    /// intrinsics to carry the template string and per-placeholder
    /// format specifications. `None` for intrinsics that need only args.
    ///
    /// All domain operations flow through this terminator: register access
    /// (`RegisterRead`, `RegisterWriteField`), resource lifecycle
    /// (`ClockEnable`, `ResetDeassert`), synchronization (future),
    /// logging, delays, polling, etc.
    ///
    /// **No `resource` field.** For method intrinsics, the resource
    /// instance is the first argument (the `&self` receiver). The
    /// receiver's Place expression identifies the resource: a direct
    /// struct field access (`self.clock`) resolves to a ResourceId
    /// trivially; a function parameter means the resource identity
    /// flows through the call site. This design allows driver authors
    /// to write helper functions that take abstraction types as
    /// parameters without restricting intrinsic recognition to
    /// statically-resolvable struct field accesses.
    IntrinsicCall {
        intrinsic: RegseqIntrinsic,
        args: Vec<Operand>,
        metadata: Option<IntrinsicMetadata>,
        destination: Place,
        target: BasicBlockId,
        error_target: Option<BasicBlockId>,
    },

    /// Drop a value, running its destructor.
    ///
    /// Post-drop-elaboration, this is either a nop (for types without drop
    /// glue) or an explicit call to drop glue. Needed for user-defined
    /// types with `Drop` impls, and for wrapper types (e.g., `LockGuard`)
    /// whose `Drop` impl calls cleanup intrinsics.
    ///
    /// No `error_target` -- `Drop::drop` returns `()` and cannot
    /// propagate errors at the terminator level. If a Drop impl
    /// internally calls a fallible intrinsic, that error is handled
    /// within the Drop body's own CFG.
    ///
    /// Drop terminators appear on both normal paths (explicit `drop()`
    /// calls and scope exits) and error paths (Rust's drop elaboration
    /// inserts them for live locals on `?` error branches). This is the
    /// primary mechanism for resource cleanup in user code -- the Drop
    /// cascade on error paths is generated by Rust, not by a Regseq
    /// compiler pass.
    Drop {
        place: Place,
        target: BasicBlockId,
    },

    /// Return from the function.
    ///
    /// Returns the value in `_0` (the return place) to the caller.
    Return,

    /// Marks unreachable code.
    ///
    /// Executing this is undefined behavior. Used after exhaustive matches
    /// or after diverging calls.
    Unreachable,
}
```

### SwitchTargets

```rust
/// Branch targets for `SwitchInt`.
///
/// Maps discriminant values to target basic blocks, with a default
/// target for values not in the map. The last entry in `targets` is
/// the default/otherwise block.
pub struct SwitchTargets {
    /// Discriminant values to match against.
    pub values: Vec<u128>,
    /// Target basic blocks. Length is `values.len() + 1` -- the last
    /// entry is the default target.
    pub targets: Vec<BasicBlockId>,
}
```

### IntrinsicMetadata

```rust
/// Intrinsic-specific structured metadata.
///
/// Carried on `IntrinsicCall` for intrinsics that need domain-specific
/// data beyond the standard resource + args fields. Extensible as
/// new intrinsic families are added.
pub enum IntrinsicMetadata {
    /// Metadata for logging intrinsics (LogError, LogWarn, LogInfo, LogDebug).
    ///
    /// During IR construction, the compiler recognizes logging intrinsic
    /// calls, extracts the template string from the MIR constant argument,
    /// parses the restricted formatting subset, and strips the Rust-level
    /// LogArg carriers -- the argument values become plain Operands in
    /// `args`, and the template + placeholder metadata land here.
    Log {
        /// The template string literal as authored.
        ///
        /// e.g., "DMA channel {} acquisition failed"
        ///
        /// Placeholders are positional `{}` with optional format
        /// modifiers from the restricted Regseq formatting subset.
        /// The template is carried as-is -- backends convert
        /// placeholders to the target format (e.g., `%u` for printf).
        template: String,

        /// Per-placeholder format specifications, in positional order.
        ///
        /// One entry per `{}` in the template, matched positionally
        /// with entries in `IntrinsicCall.args`. Length must equal
        /// args length for logging intrinsics.
        placeholders: Vec<FormatPlaceholder>,
    },

    /// Metadata for register access intrinsics (RegisterRead,
    /// RegisterReadField, RegisterWrite, RegisterWriteField).
    ///
    /// During IR construction, the compiler resolves register identity
    /// from the monomorphized type parameters on the intrinsic call
    /// (see spec.md Section 10.2) and stores the resolved result here.
    /// The type parameters themselves do not survive into the Regseq IR
    /// -- this metadata is the resolved output.
    ///
    /// The transport resource is identified by the receiver argument's
    /// Place expression (the first arg in `IntrinsicCall.args`). This
    /// metadata carries the register-level identity within that
    /// transport's register block.
    Register {
        /// The register block containing the target register.
        register_block: RegisterBlockId,

        /// The register's byte offset within the block.
        /// Resolved from the register block's field layout during
        /// IR construction.
        register_offset: u32,

        /// The register's access width in bits (8, 16, 32).
        /// From the register block's access_width property or the
        /// register's T: RegWidth type parameter.
        access_width: u32,

        /// Field-level identity. Present for RegisterReadField and
        /// RegisterWriteField. None for RegisterRead and RegisterWrite
        /// (which operate on the full register).
        field: Option<RegisterFieldMeta>,
    },

    /// Metadata for PinCtrlApply intrinsic.
    ///
    /// During IR construction, the compiler resolves the monomorphized
    /// pin state type parameter `S` to a logical pin state identity by
    /// reading `#[rseq::pin_state(id = "...")]` from the type
    /// definition's Attrs bag.
    ///
    /// The PinCtrl resource is identified by the receiver argument's
    /// Place expression (the first arg in `IntrinsicCall.args`). This
    /// metadata carries the resolved pin state identity.
    PinCtrl {
        /// The logical pin state name, resolved from the
        /// `#[rseq::pin_state(id = "...")]` attribute on the
        /// monomorphized type parameter S.
        /// e.g., "default", "sleep"
        state_id: Symbol,
    },
}

/// Resolved field identity for register field intrinsics.
pub struct RegisterFieldMeta {
    /// The field's bit mask within the register.
    pub mask: u64,

    /// The field's bit shift (offset from bit 0).
    pub shift: u32,

    /// The field's name (interned symbol) for diagnostics and
    /// backend code comments.
    pub name: Symbol,
}
```

### FormatPlaceholder

```rust
/// A single placeholder's formatting specification.
///
/// Extracted from the authored format string during IR construction.
/// Only the restricted Regseq formatting subset is accepted --
/// unrecognized format specifiers produce an extraction error.
pub struct FormatPlaceholder {
    /// The display format for this placeholder.
    pub format: FormatKind,
}

/// Placeholder display format.
///
/// The restricted Regseq formatting subset. Each variant maps
/// cleanly to a printf-family format specifier for backend lowering.
pub enum FormatKind {
    /// Default display: `{}`
    ///
    /// Backend chooses the format specifier based on the argument's
    /// RegseqType: u32 -> %u, i32 -> %d, bool -> %d, etc.
    Display,

    /// Lowercase hex: `{:x}`
    LowerHex,

    /// Uppercase hex: `{:X}`
    UpperHex,

    /// Lowercase hex with 0x prefix: `{:#x}`
    LowerHexPrefixed,

    /// Uppercase hex with 0x prefix: `{:#X}`
    UpperHexPrefixed,

    /// Zero-padded hex with configurable width: `{:08x}`, `{:#010x}`, etc.
    ///
    /// Common in driver code for register values and masks.
    PaddedHex {
        /// Whether to include the 0x prefix (# flag).
        prefix: bool,
        /// Whether to use uppercase hex digits (X vs x).
        uppercase: bool,
        /// Total width including prefix and padding.
        width: u8,
    },

    /// Pointer display: `{:p}`
    Pointer,
}
```

### MIR mapping -- Terminators

| MIR `TerminatorKind<'tcx>`                              | Regseq IR `Terminator`                                                           | Notes                                                                                              |
| ------------------------------------------------------- | -------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------- |
| `Goto { target }`                                       | `Goto(BasicBlockId)`                                                             | Direct.                                                                                            |
| `SwitchInt { discr, targets }`                          | `SwitchInt { discr, targets }`                                                   | Direct. Operand and targets mapped to Regseq IR types.                                             |
| `Call { func, args, destination, target, unwind, ... }` | `Call { callee, args, destination, target, error_target }`                       | `func` resolved to `FunctionId`. `unwind` dropped. `error_target` populated by folding `?` pattern (see below). |
| *(intrinsic calls in MIR are `Call`)*                   | `IntrinsicCall { intrinsic, args, metadata, destination, target, error_target }` | Regseq intrinsic separated into its own terminator. Resource identity from receiver arg's Place. `?` folded into `error_target`. Logging args restructured into `metadata`. |
| `Drop { place, target, unwind, ... }`                   | `Drop { place, target }`                                                         | `unwind` dropped (no unwinding). `replace` and async fields dropped. No `error_target` -- Drop returns `()`. |
| `Return`                                                | `Return`                                                                         | Direct.                                                                                            |
| `Unreachable`                                           | `Unreachable`                                                                    | Direct.                                                                                            |
| `UnwindResume`                                          | Dropped                                                                          | No unwinding in driver code. DSL prohibits panicking.                                              |
| `UnwindTerminate(reason)`                               | Dropped                                                                          | No unwinding in driver code.                                                                       |
| `Assert { cond, expected, msg, target, unwind }`        | Dropped                                                                          | Panic-based assertions. Condition checks representable as `SwitchInt`.                             |
| `TailCall { func, args, ... }`                          | Dropped                                                                          | Unstable feature. Not used in driver specs.                                                        |
| `Yield { ... }`                                         | Dropped                                                                          | Coroutine-specific. Not used in driver specs.                                                      |
| `CoroutineDrop`                                         | Dropped                                                                          | Coroutine-specific. Not used in driver specs.                                                      |
| `InlineAsm { ... }`                                     | Dropped                                                                          | Inline assembly. Add when needed.                                                                  |
| `FalseEdge { ... }`                                     | Dropped                                                                          | Borrow-checker artifact. Not present at our MIR stage.                                             |
| `FalseUnwind { ... }`                                   | Dropped                                                                          | Borrow-checker artifact. Not present at our MIR stage.                                             |

### Design rationale

**Seven terminators.** This covers all control flow patterns in driver code:
unconditional jumps, conditional branches, function calls, intrinsic calls,
destructors, returns, and unreachable markers. Loops (`while`, `loop`,
`for`) are represented as natural CFG back-edges (`Goto` to an earlier
block with a `SwitchInt` condition check). No dedicated loop construct
is needed.

**`Call` and `IntrinsicCall` are separate terminators.** They carry different
metadata (`FunctionId` vs. `RegseqIntrinsic` + `ResourceId`), have different
semantics (opaque vs. compiler-known), and will evolve independently.
Intrinsic calls will grow domain-specific fields as the DSL evolves. Unifying
them would force a discriminant inside a shared struct with fields that only
apply to one variant.

**`IntrinsicCall.target` is not `Option`.** The compiler knows all intrinsics
and guarantees they return. There is no diverging intrinsic. This is a
structural guarantee, not a convention.

**`IntrinsicCall.metadata` carries intrinsic-specific structured data.**
Some intrinsics need domain information that does not fit into the generic
`resource` + `args` fields. Rather than adding per-intrinsic fields to the
terminator struct, `metadata: Option<IntrinsicMetadata>` provides a typed
extension point. Currently three variants:
- `Log`: template string and per-placeholder format specifications for
  logging intrinsics.
- `Register`: resolved register block, offset, access width, and
  optional field mask/shift for register access intrinsics. This is the
  resolved output of the type-parameter-based identity resolution
  described in spec.md Section 10.2.
- `PinCtrl`: resolved pin state identity (`state_id: Symbol`) for the
  `PinCtrlApply` intrinsic. Resolved from the monomorphized type
  parameter's `#[rseq::pin_state(id = "...")]` attribute.
`None` for intrinsics that need only resource and args (e.g., clock
lifecycle, reset, timing).

**Register intrinsic identity resolution.** Register access intrinsics
(`RegisterRead`, `RegisterReadField`, `RegisterWrite`,
`RegisterWriteField`) are methods on transport types (`Mmio<Block>`,
etc.) with the `&self` receiver as the first argument. After
monomorphization, the compiler resolves the full identity chain from
the intrinsic call's type parameters:
(1) `Block` type parameter (from receiver type `Mmio<Block>`) gives the
register block type.
(2) `R` type parameter (e.g., `pl011::dr::Register`) gives the register
identity -- scoped to the block, unique. The register's byte offset
is resolved from the register block's field layout.
(3) `R::Access` gives the access kind (`ReadWriteAccess`, `ReadOnlyAccess`,
`WriteOnlyAccess`).
(4) The `&self` receiver's place expression traces to the driver struct
field, giving the `ResourceId` and transport kind.
This resolution is deterministic and requires no interprocedural
analysis beyond the immediate intrinsic call site. See spec.md
Section 10.2 for the full architecture.

**`error_target` on Call and IntrinsicCall is a pattern-folding mechanism.**
Rust's `?` operator desugars into a multi-block pattern: `Call` ->
`<T as Try>::branch` -> `SwitchInt` on `ControlFlow` discriminant ->
`FromResidual` on the Break path. This is 4-5 basic blocks of `Try` trait
machinery per `?` with no meaning in C. During IR construction, this pattern
is recognized and folded for any `Try` implementor (`Result`, `Option`,
etc.): `target` receives the Continue path, `error_target` receives the
Break path destination (after `FromResidual`). The intermediate
`ControlFlow` values, `Try::branch` calls, and `FromResidual` calls are
eliminated.

The Break path blocks that `error_target` points to contain the `Drop`
cascade for live locals, as generated by Rust's drop elaboration. This is
the correct cleanup sequence -- Rust handles conditional drops, drop order
(reverse declaration order), and drop flags for conditionally-initialized
variables.

For compiler-generated function bodies (pre-init, framework integration),
the compiler sets `error_target` directly during IR construction. No
pattern recognition needed -- the compiler is building the IR from scratch.

Infallible operations (register read/write, logging, delays) have
`error_target: None`.

### `?`-pattern recognition algorithm

During IR construction, the compiler recognizes Rust's `?` desugaring
and folds it into `error_target`. The MIR pattern is mechanically
generated by rustc and has a fixed shape (verified experimentally at
`mir_drops_elaborated_and_const_checked` with `mir_opt_level = 0` on
nightly-2026-02-01).

The algorithm is type-agnostic: it matches the structural MIR pattern
produced by any `Try` implementor (`Result`, `Option`, or any future
type with a `Try` impl). The trigger is the `Try::branch` DefId, not
the concrete type.

**The MIR pattern for each `?`:**

```
bb_call:
    _result = some_fn(args) -> [return: bb_branch, unwind: bb_cleanup]

bb_branch:
    _cf = <T as Try>::branch(move _result)
        -> [return: bb_switch, ...]

bb_switch:
    _discr = discriminant(_cf)
    switchInt(_discr) -> [0: bb_continue, 1: bb_break, otherwise: unreachable]

bb_continue:                           // Continue path
    _val = move ((_cf as Continue).0)
    ... continues with _val ...

bb_break:                              // Break path
    _residual = move ((_cf as Break).0)
    _converted = <T as FromResidual>::from_residual(move _residual)
        -> [return: bb_store, ...]

bb_store:
    _0 = move _converted               // store in return place
    ... Drop cascade for live locals (from drop elaboration) ...
    return
```

Where `T` is the concrete `Try` implementor (`Result<V,E>`,
`Option<V>`, etc.). The pattern shape is identical regardless of `T`.

**Recognition steps:**

1. **Trigger.** When processing a `Call` terminator whose return target
   block starts with a call to `<T as Try>::branch`, begin pattern
   recognition. `Try::branch` is identified by its `DefId` (a known
   trait method in rustc), not by the implementing type.

2. **Validate the SwitchInt.** The `Try::branch` call's return target
   must contain `discriminant(_cf)` + `switchInt` with two meaningful
   targets: Continue (variant 0) and Break (variant 1), plus
   `otherwise: unreachable`.

3. **Extract the two paths and retarget the destination.**
   - **Continue path**: The Continue block extracts the success value:
     `_val = move ((_cf as Continue).0)`. The folded terminator's
     `destination` is retargeted to `_val`'s place (the inner type,
     not the `Try` wrapper). The `target` is set to the block that
     follows the Continue extraction -- where execution continues
     using `_val`.
   - **Break path**: Follow through to
     `<T as FromResidual>::from_residual` (identified by `DefId`).
     The block AFTER `from_residual` returns becomes `error_target`.
     This block contains the Drop cascade for live locals and
     eventually reaches `return`.

4. **Eliminate intermediates.** The following MIR constructs are dropped
   and do not appear in the Regseq IR:
   - The `Try`-wrapper temporary (`_result`) -- the original Call
     destination. Dead after folding.
   - The `ControlFlow` temporary (`_cf`) -- the `Try::branch` output.
   - The `Try::branch` call.
   - The `SwitchInt` on the `ControlFlow` discriminant.
   - The Continue extraction statement (`_val = (_cf as Continue).0`)
     -- absorbed into the retargeted `destination`.
   - The `FromResidual::from_residual` call.
   - The residual temporaries (`_residual`, `_converted`).

   The net effect: the folded Call/IntrinsicCall writes the unwrapped
   success value directly to `destination`, jumps to `target` on
   Continue, or jumps to `error_target` on Break. The intermediate
   `Try`-wrapper and `ControlFlow` values never exist in the Regseq IR.

5. **Fallback.** If any step does not match (no `Try::branch` at the
   return target, unexpected `SwitchInt` shape, missing
   `FromResidual`), treat the Call as a regular call with
   `error_target: None`. If the pattern is partially recognized but
   structurally invalid (e.g., `Try::branch` present but `SwitchInt`
   shape wrong), the compiler emits a hard error and stops.

**For IntrinsicCalls.** The same pattern applies. The initial Call is
first recognized as a Regseq intrinsic (by path), then the `?` folding
applies on top. The result is an `IntrinsicCall` terminator with both
the intrinsic identity and the `error_target`.

**Nested `?`.** Each `?` is an independent instance of this pattern on
a different Call terminator. Multiple `?` in one function produce
multiple instances. They may share error-path cleanup blocks (the Drop
cascade) but each recognition is self-contained.

**Pinned to nightly version.** The pattern is generated by rustc's
`?` desugaring through the `Try` trait v2 design. It is stable within
a pinned nightly version. When upgrading the pinned nightly, the MIR
pattern should be re-verified as part of the upgrade process.

**Concrete `Try` implementors in driver specs:**

- `Result<T, RegseqError>`: The dominant case. The Break path is error
  propagation. `error_target` points to the error-return cleanup.
- `Option<T>`: The Break path is None propagation. `error_target`
  points to a block that stores `None` in the return place and returns.
  Used in methods like `rx_char() -> Option<RxChar>`.

The field is named `error_target` because error propagation is the
primary use case, but it handles any `Try::branch` Break path.

**`Drop` has no `error_target`.** `Drop::drop` returns `()` in Rust -- it
cannot fail at the terminator level. If a Drop impl internally calls a
fallible intrinsic, that error is handled within the Drop body's own CFG.

**`Drop` included.** User-defined types in Section 3.4 are not constrained to
`Copy` (only driver struct user data fields are). Types with `Drop` impls --
including Regseq wrapper types like `LockGuard` whose destructors call
cleanup intrinsics -- generate `Drop` terminators in MIR. Drop terminators
appear on both normal paths and error paths. On `?` error branches, Rust's
drop elaboration inserts Drop terminators for all live locals in reverse
declaration order. This is the primary mechanism for resource cleanup in
user code -- the Regseq compiler does not generate a separate error handling
pass for user-written function bodies.

**No error handling pass for user code.** MIR at the
`mir_drops_elaborated_and_const_checked` stage (with `mir_opt_level = 0`)
already contains correct error handling: `?` produces `SwitchInt` branching
on the `ControlFlow` discriminant, and drop elaboration inserts `Drop` terminators
for live locals on all paths (including error paths). The IR construction
folds the `?` boilerplate into `error_target` and preserves the `Drop`
cascade. No subsequent transformation pass is needed.

**Unwind/cleanup blocks from MIR are dropped.** MIR blocks marked
`is_cleanup` handle Rust's panic/unwind mechanism (ending in `resume`).
Driver code does not unwind -- the DSL prohibits panicking operations.
These blocks are dropped during IR construction.

**`Assert` excluded.** Assert is a conditional panic. The condition check is
already present in the IR as standard operations producing a boolean. The
panic path is not modeled (driver code does not unwind). If bounds checking
is needed, it can be represented as `SwitchInt` with one branch being an
error path.

**`UnwindResume` / `UnwindTerminate` excluded.** These exist only in cleanup
blocks for Rust's panic/unwind mechanism. Driver code does not unwind. The
DSL prohibits panicking operations.

**`TailCall` excluded.** The `become` keyword is unstable in Rust. Driver
functions are short and don't use tail recursion.

**`InlineAsm` excluded.** Inline assembly support can be added when a driver
spec requires it. The terminator structure would carry template, operands,
and options similar to MIR's `InlineAsm`.

---

## 6. Body Structure

A body is the executable content of a function: local variable declarations
and a control flow graph of basic blocks. Every `Function` in the model
(Section 8 of spec.md) owns exactly one `Body`.

### Body

```rust
/// A function body: local declarations + control flow graph.
pub struct Body {
    /// Local variable declarations.
    ///
    /// Index 0 is the return place (_0). Indices 1..=arg_count are
    /// function arguments. The rest are user variables and compiler
    /// temporaries.
    pub local_decls: Vec<LocalDecl>,

    /// Number of function arguments.
    ///
    /// Locals _1 through _arg_count are arguments provided by the
    /// caller and can be assumed initialized at function entry.
    pub arg_count: usize,

    /// Basic blocks forming the control flow graph.
    ///
    /// Block 0 is the entry block. Blocks are indexed by BasicBlockId.
    pub basic_blocks: Vec<BasicBlock>,

    /// Source locations for body-level nodes.
    ///
    /// Structurally parallel to the body: one entry per local, one
    /// per statement, one per terminator. See Section 7 for details.
    pub source_map: BodySourceMap,

    /// Ordered regions within this body.
    ///
    /// Each region identifies a contiguous sequence of basic blocks
    /// whose execution order must be preserved -- the compiler and
    /// hardware must not reorder them. Backends emit ordering barriers
    /// (e.g., compiler barriers in C) at region boundaries.
    ///
    /// Constructed during IR construction from paired
    /// `regseq::internal::ordered_region_begin()` /
    /// `regseq::internal::ordered_region_end()` marker calls in MIR.
    /// The markers are expansions of the `ordered!` macro. The marker
    /// calls are dropped; the SESE region between them is recorded here.
    ///
    /// Regions may not overlap. Nesting is supported (an inner region
    /// strictly contained within an outer region) though not expected
    /// in practice. Each nested region gets its own barriers.
    pub ordered_regions: Vec<OrderedRegion>,
}

/// An ordered region defined by a single-entry single-exit (SESE)
/// invariant on the CFG.
///
/// A block belongs to the region if it is reachable from `entry`
/// without passing through `exit`. This reachability-based definition
/// is independent of basic block ID numbering and handles all CFG
/// shapes: straight-line calls, conditionals (diamonds), loops
/// (back-edges), and fallible calls with error exits.
///
/// **SESE invariant:**
/// - Single entry: all control flow into the region enters through
///   `entry`. No edge from outside the region targets an interior
///   block.
/// - Single normal exit: normal control flow leaves the region
///   through `exit`.
/// - Abnormal exits: `error_target` edges from Call/IntrinsicCall
///   terminators within the region may exit the region. These are
///   abnormal exits that bypass `exit`.
///
/// **Barrier placement (backend obligation):**
/// - Emit a compiler barrier at `entry` (before the first operation).
/// - Emit a compiler barrier at `exit` (after the last operation).
/// - Emit a compiler barrier at each abnormal exit (before the
///   error_target jump) to ensure operations already executed in the
///   sequence are visible before cleanup runs.
/// - The barrier kind is a compiler barrier (e.g.,
///   `asm volatile("" ::: "memory")` in C), preventing the C compiler
///   from reordering operations across region boundaries. Hardware
///   memory barriers are NOT implied -- MMIO volatile semantics and
///   future DMA barrier intrinsics handle hardware ordering separately.
pub struct OrderedRegion {
    /// The single entry point of the region.
    pub entry: BasicBlockId,
    /// The single normal exit point of the region (first block
    /// after the ordered sequence).
    pub exit: BasicBlockId,
}

impl Body {
    /// Compute the predecessor map for this body's CFG.
    ///
    /// Returns a map from each basic block to the list of blocks
    /// that have it as a successor (via any terminator edge:
    /// Goto, SwitchInt targets, Call/IntrinsicCall target and
    /// error_target, Drop target).
    ///
    /// Computed from the current terminator edges -- always
    /// consistent with the CFG's current state. Not stored on
    /// BasicBlock to avoid sync invariants during IR construction
    /// and transformation (error_target wiring, cleanup block
    /// dropping). At driver-spec scale
    /// (tens to low hundreds of blocks), recomputation is trivial.
    pub fn predecessors(&self) -> BTreeMap<BasicBlockId, Vec<BasicBlockId>> { ... }
}
```

### LocalDecl

```rust
/// A local variable, temporary, or the return place.
pub struct LocalDecl {
    /// The local's type.
    pub ty: RegseqType,

    /// Whether the binding is mutable (let mut vs let).
    ///
    /// Temporaries and the return place are always mutable. This is
    /// a property of the declaration, not the type (see Section 8
    /// of spec.md: Type System, Mutability).
    pub mutable: bool,

    /// The user's variable name, if this local corresponds to a
    /// user-declared variable or function argument.
    ///
    /// `None` for compiler-generated temporaries. Derived from MIR's
    /// `var_debug_info` during IR construction. Used by backends to
    /// emit readable variable names (e.g., `baud_rate` instead of
    /// `_7`) and by diagnostics to produce meaningful error messages.
    ///
    /// Interned via the model's `SymbolInterner` (see Section 7).
    pub name: Option<Symbol>,
}
```

### BasicBlock

```rust
/// A basic block: a sequence of statements followed by a terminator.
pub struct BasicBlock {
    /// Ordered statements executed sequentially.
    pub statements: Vec<Statement>,

    /// The block's terminator. Every block has exactly one.
    ///
    /// Non-optional -- unlike MIR (where it is temporarily `None`
    /// during construction), the Regseq IR is always fully
    /// constructed.
    pub terminator: Terminator,
}
```

### Statement and Terminator wrappers

Statements and terminators are wrapped in structs that carry the
semantic content. Source locations are stored separately in the
`BodySourceMap` (Section 7), not on the nodes themselves.

```rust
/// A statement: semantic content only.
///
/// Source location is in BodySourceMap, keyed by (block, statement index).
pub struct Statement {
    pub kind: StatementKind,
}

/// A terminator: semantic content only.
///
/// Source location is in BodySourceMap, keyed by block.
pub struct Terminator {
    pub kind: TerminatorKind,
}
```

### MIR mapping -- Body

| MIR `Body<'tcx>` field | Regseq IR | Reasoning |
|---|---|---|
| `basic_blocks` | **Keep** | The CFG. Core of the body. |
| `local_decls` | **Keep** | Types and names of all locals. Essential. |
| `arg_count` | **Keep** | Distinguishes arguments from temporaries. |
| `var_debug_info` | **Transform** → `name: Option<Symbol>` on `LocalDecl` | Separate table collapsed into per-local interned symbol. |
| `span` | **Transform** → `BodySourceMap.body_span` | Moved to source map side table. |
| `source_scopes` | **Drop** | Scope hierarchy for lint levels and debuginfo. Not needed. |
| `phase` | **Drop** | MIR phase tracking. Pipeline phases tracked at pipeline level. |
| `pass_count` | **Drop** | Internal debug counter. |
| `source` | **Drop** | MIR source identity. Body is owned by a Function with FunctionId. |
| `coroutine` | **Drop** | No coroutines in driver specs. |
| `user_type_annotations` | **Drop** | NLL type annotations. |
| `spread_arg` | **Drop** | Rust-call ABI tuple spreading. |
| `required_consts` | **Drop** | Const evaluation soundness tracking. |
| `mentioned_items` | **Drop** | Monomorphization tracking. |
| `is_polymorphic` | **Drop** | Everything is monomorphized. |
| `injection_phase` | **Drop** | `#[rustc_custom_mir]` only. |
| `tainted_by_errors` | **Drop** | rustc error tracking. |
| `coverage_*` | **Drop** | Coverage instrumentation. |

### MIR mapping -- LocalDecl

| MIR `LocalDecl<'tcx>` field | Regseq IR | Reasoning |
|---|---|---|
| `ty` | **Keep** → `RegseqType` | Every local has a type. Essential. |
| `mutability` | **Keep** → `mutable: bool` | Binding mutability. Backends may emit `const` for immutable locals. |
| *(from `var_debug_info`)* | **New** → `name: Option<Symbol>` | User variable name. Interned symbol, moved from separate table to per-local field. |
| `local_info` | **Drop** | Diagnostic classification (user var, temp, static). Not needed. |
| `user_ty` | **Drop** | NLL type annotation. |
| `source_info` | **Transform** → `BodySourceMap.locals` | Source location moved to side table. Scope dropped. |

### MIR mapping -- BasicBlockData

| MIR `BasicBlockData<'tcx>` field | Regseq IR | Reasoning |
|---|---|---|
| `statements` | **Keep** | The block's statements. Core content. |
| `terminator` | **Keep, non-optional** | Always set in the Regseq IR (fully constructed). |
| `is_cleanup` | **Drop** | Marks unwind cleanup blocks. No unwinding in driver code. These blocks are dropped during IR construction. |
| `after_last_stmt_debuginfos` | **Drop** | Debug info. |

### MIR mapping -- Statement / Terminator wrappers

| MIR wrapper field | Regseq IR | Reasoning |
|---|---|---|
| `source_info.span` | **Transform** → `BodySourceMap` | Resolved to `SourceLocation` range and stored in source map side table, not on the node. |
| `source_info.scope` | **Drop** | Scope index for lint levels. Not needed. |
| `debuginfos` | **Drop** | Internal debug info. |

### Design rationale

**Body has 5 fields (down from 18).** The added fields are `source_map`
(structurally parallel side table for source locations, see Section 7)
and `ordered_regions` (body-level SESE metadata for `ordered!` blocks).
The 14 dropped fields are rustc internals: phase tracking, error tracking,
coverage, NLL annotations, coroutine info, monomorphization metadata, const
evaluation tracking. None is needed for Regseq analyses, transformations,
or backend emission.

**Variable names on LocalDecl as interned symbols.** MIR uses
`var_debug_info` as a separate `Vec` mapping locals to user variable names.
The Regseq IR puts the name directly on `LocalDecl` as `Option<Symbol>`:
- Simpler -- no separate lookup table to maintain.
- The name is the only piece of debug info we need (no DWARF fragments,
  no composite variables, no argument index tracking).
- Interned via `SymbolInterner` -- `Symbol` is `Copy` and 4 bytes.

**Source locations in a side table, not on IR nodes.** MIR carries
`SourceInfo { span, scope }` on every `Statement`, `Terminator`, and
`LocalDecl`. The Regseq IR moves source locations to `BodySourceMap`
(see Section 7), keeping IR nodes clean. The source map is structurally
parallel: `Vec<SourceOrigin>` for locals, `Vec<BasicBlockSourceMap>` for
blocks, with per-block `Vec<SourceOrigin>` for statements and one
`SourceOrigin` for the terminator. This covers all body-level nodes
without cluttering the IR or requiring position-keyed maps.

Sub-expression nodes (`Rvalue`, `Operand`, `Place`) do not have
independent source locations -- they inherit from their containing
statement or terminator. This matches MIR's model.

**Terminator is non-optional.** MIR uses `Option<Terminator>` because blocks
are created before their terminators are known during MIR construction. The
Regseq IR is always fully constructed, so every block has a terminator. This
is a structural guarantee enforced by the type system -- you cannot represent
a block without a terminator.

**Predecessors computed lazily, not stored.** `BasicBlock` has no
`predecessors` field. Instead, `Body::predecessors()` computes the
predecessor map from the current terminator edges on demand. This avoids
sync invariants during IR construction and transformation passes (where
terminators are rewritten, blocks added or removed). At driver-spec scale
(tens to low hundreds of blocks), recomputation is trivial. Consumers
that need predecessors (dominance trees, liveness analysis, dead block
elimination) call the utility once and hold the result for the duration
of their analysis.

**`is_cleanup` dropped.** MIR's `is_cleanup` marks unwind cleanup blocks.
Driver code does not unwind -- the DSL prohibits panicking operations.
Cleanup blocks (those ending in `resume`) are dropped during IR construction.
Error-path blocks from `?` are normal control flow blocks, not cleanup
blocks -- they are reached via `error_target` on Call/IntrinsicCall
terminators and contain `Drop` terminators for live locals.

**StatementKind / TerminatorKind naming convention.** Following MIR's
established pattern: the enum is the "kind" (semantic content), the wrapper
struct carries no metadata (source locations are in the side table). Sections
4 and 5 of this document define `StatementKind` and `TerminatorKind`; the
`Statement` and `Terminator` structs defined here wrap only the kind.

---

## 7. Symbol Interning, Source Locations, and Serialization

### Symbol Interning

All strings in the IR are interned. Each unique string is stored once in
a central `SymbolInterner`; all references use a `Symbol(u32)` index.

```rust
/// An interned string identifier.
///
/// Copy, Eq, Hash -- all O(1). 4 bytes per reference instead of
/// 24+ bytes for a String. Equality is integer comparison.
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Symbol(u32);

/// An interned file path. Distinct newtype over the same interner
/// pool to prevent accidentally mixing identifiers and file paths.
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub struct FileId(u32);

/// The string intern table. Owned by the RegseqModel, populated
/// during IR construction, passed to consumers that need to resolve
/// symbols to human-readable strings.
pub struct SymbolInterner {
    /// Index -> string. The Symbol/FileId u32 is an index here.
    strings: Vec<String>,
    /// String -> index. Deduplication during construction.
    lookup: HashMap<String, u32>,
}

impl SymbolInterner {
    /// Intern a string. Returns existing Symbol if already interned.
    pub fn intern(&mut self, s: &str) -> Symbol { ... }

    /// Intern a file path. Same pool, distinct type.
    pub fn intern_file(&mut self, path: &str) -> FileId { ... }

    /// Resolve a Symbol to its string.
    pub fn resolve(&self, sym: Symbol) -> &str { ... }

    /// Resolve a FileId to its file path.
    pub fn resolve_file(&self, file: FileId) -> &str { ... }
}
```

All `String` and `Option<String>` identifier fields in the IR become
`Symbol` or `Option<Symbol>`. This includes: field names on projections
(`Field.name`), local variable names (`LocalDecl.name`), variant names
(`Downcast.name`), and all declaration-level identifiers in the model.

### Source Locations

Source locations use a range (start + end), not a point. Ranges support
diagnostics that underline spans, IDE integration, and multi-line constructs.

```rust
/// A source location range with interned file path.
pub struct SourceLocation {
    pub file: FileId,
    pub start_line: u32,
    pub start_column: u32,
    pub end_line: u32,
    pub end_column: u32,
}

/// The origin of an IR node: user source or compiler-generated.
pub enum SourceOrigin {
    /// From the spec author's source code. The SourceLocation is
    /// resolved from rustc's Span during IR construction.
    ///
    /// Compiler-inserted nodes that ARE attributable to a user
    /// construct (e.g., Drop terminators on ? error paths from
    /// Rust's drop elaboration) should use Source with the
    /// originating user span, not CompilerGenerated.
    Source(SourceLocation),

    /// Generated by the compiler with no corresponding source line.
    /// Used for pre-init/teardown code and compiler-constructed
    /// function bodies. Diagnostics display "in compiler-generated
    /// initialization code" rather than a source location.
    CompilerGenerated,
}
```

### What gets source locations

**Declaration-level items** -- keyed by `DeclId` in a model-level table.
All items with a `DeclId` (modules, functions, type definitions, constants,
register descriptors, register blocks, resources, properties, user data)
get a source location from their HIR/DefId span.

```rust
/// Model-level source locations for declarations.
pub source_locations: BTreeMap<DeclId, SourceOrigin>,
```

**Body-level nodes** -- stored in `BodySourceMap`, structurally parallel
to the body. No position-keyed maps; the source map mirrors the body's
structure exactly.

```rust
/// Source locations for all nodes in a function body.
///
/// Structurally parallel to Body: locals indexed by LocalId,
/// blocks indexed by BasicBlockId, statements indexed within
/// each block. Consistency is structural, not by key lookup.
pub struct BodySourceMap {
    /// The overall span of the function body.
    pub body_span: SourceOrigin,

    /// One per local, parallel to Body.local_decls.
    /// Index i corresponds to LocalId(i).
    pub locals: Vec<SourceOrigin>,

    /// One per basic block, parallel to Body.basic_blocks.
    /// Index i corresponds to BasicBlockId(i).
    pub blocks: Vec<BasicBlockSourceMap>,
}

/// Source locations for one basic block's contents.
pub struct BasicBlockSourceMap {
    /// One per statement, parallel to BasicBlock.statements.
    /// Index j corresponds to statement j in the block.
    pub statements: Vec<SourceOrigin>,

    /// The block's terminator.
    pub terminator: SourceOrigin,
}
```

**Not tracked** (inherit from containing statement/terminator):
`Rvalue`, `Operand`, `Place`, `Projection`. A diagnostic about a bad
rvalue uses the containing statement's source location.

### IR Serialization

IR types use serde for JSON serialization. Most types derive
`Serialize`/`Deserialize` directly. A few require custom impls:

- `Symbol` and `FileId` serialize as resolved strings (not raw `u32`
  indices). The interner is an internal compiler optimization, not
  part of the wire format. Every `Symbol` field emits its string
  value, every `FileId` emits the file path. This duplicates strings
  in JSON but the cost is trivial at our scale, and the gain in
  readability, debuggability, and fixture quality is significant.
- The top-level `RegseqModel` wrapper includes a `format_version`
  field not present on any inner IR type.

The JSON format is implicit -- defined by the Rust type definitions
and their serde attributes, not a separate schema. The format is
locked through golden-file tests: serialize the IR for known drivers,
store the expected JSON as test fixtures, and assert round-trip
equality.

**Enum representation: adjacently tagged.** All enums use serde's
adjacently tagged format: `{"kind": "Call", "data": {...}}`. This
is explicit, readable, stable across refactors, and works uniformly
for unit, tuple, and struct variants.

**Option fields: omit when None.** Optional fields are omitted from
the JSON, not serialized as `null`. Deserialization uses
`#[serde(default)]` for optional/additive fields. This keeps the
JSON clean and provides basic additive evolution.

**Format versioning.** A top-level `format_version` integer field is
present from the start. Additive changes (new optional fields) are
compatible. New enum variants or changed required fields bump the
version and fail cleanly on old readers.

**Deterministic output.** Collections are sorted before serialization
(no HashMap key-order flaps). Golden-file tests depend on deterministic
output.

**Golden files.** The PL011 spec is the primary golden-file fixture.
Additional small hand-built fixtures exercise every important enum
shape and edge case.

A single JSON format serves debugging, golden-file testing, and IR
dumps. No separate binary format until a concrete need is measured.

### Model structure

```rust
pub struct RegseqModel {
    /// The string interner. All identifiers and file paths
    /// in the model reference this table.
    pub symbols: SymbolInterner,

    /// Source locations for declaration-level items.
    pub source_locations: BTreeMap<DeclId, SourceOrigin>,

    pub root_module: Module,
    // ... everything else
}
```

Each `Body` carries its own `BodySourceMap` (see Section 6).

### Design rationale

**Single interner, distinct newtypes.** One string pool for both
identifiers and file paths, but `Symbol` and `FileId` are distinct
types to prevent accidental mixing. Simpler than two interners,
type-safe at call sites.

**Ranges, not points.** `SourceLocation` carries start and end
positions. Editors underline ranges, error messages reference multi-line
spans, and IDE integration (LSP) requires ranges. The information is
available from rustc's `Span` during IR construction at no extra cost.

**`SourceOrigin` over `Option<SourceLocation>`.** `CompilerGenerated` is
a semantic state ("this node was synthesized by the compiler"), not
missing data. `None` would conflate "compiler-generated" with "unknown
/ lost during construction," which is worse for diagnostics. Nodes
attributable to a user construct (e.g., drop elaboration from `?`) use
`Source` with the originating span, not `CompilerGenerated`.

**Parallel vectors, not maps.** `BodySourceMap` mirrors `Body`'s
structure: `Vec<SourceOrigin>` for locals (parallel to `local_decls`),
`Vec<BasicBlockSourceMap>` for blocks (parallel to `basic_blocks`),
`Vec<SourceOrigin>` per block for statements. No key lookups, no
consistency bugs. Index `i` in the source map is always index `i` in
the IR.

**Symbols as strings in JSON, not indices.** The `SymbolInterner` is
an internal runtime optimization (O(1) equality, deduplication in
memory). The serialization format does not expose it. `Symbol` and
`FileId` values serialize as their resolved strings. This makes the
JSON human-readable and self-contained -- no preamble table to
cross-reference. String duplication in JSON is trivial at our scale.

**JSON-only, golden-file tested.** Most IR types derive serde
traits directly; `Symbol` and `FileId` use custom impls to serialize
as strings. Enums use adjacently-tagged format. The format is locked
by golden-file tests, not a separate schema document. A binary format
adds maintenance burden for no measured benefit at driver-spec scale.

---

## 8. IR Construction Pass

This section describes how the Regseq IR is constructed from rustc's
MIR. The preceding sections (1-6) define what each IR construct is.
This section defines the strategy for building the complete
`RegseqModel` from MIR input.

### Input

The IR construction pass receives MIR from rustc at the
`mir_drops_elaborated_and_const_checked` stage with `mir_opt_level = 0`,
using release profile semantics (`overflow-checks = false`,
`debug-assertions = false`). At this stage:
- All types are monomorphized (no generics).
- Drops are elaborated (explicit `Drop` terminators for all types with
  destructors, on all paths including error paths from `?`).
- No MIR optimizations have run (the author's intent is preserved).
- Unwind paths are explicit (cleanup blocks with `resume`).
- Arithmetic overflow checks are absent (`Add`, not `AddWithOverflow`).
- Debug assertion `Assert` terminators are absent.

### Output

A `RegseqModel` with:
- Module tree with all declaration-level items populated.
- All function bodies constructed (CFG with basic blocks, statements,
  terminators).
- `SymbolInterner` populated with all interned strings.
- Source locations resolved and stored in `BodySourceMap`s and the
  model-level `source_locations` table.
- All binding slots set to `None` (Phase 1 -- bindings are populated
  later during platform join).

### Two-pass strategy

**Pass 1: Declaration skeleton.** Walk the crate's module tree and build
the declaration-level model structure. This assigns all `DeclId`s before
any function body is processed:

1. Walk modules, assign `ModuleId`s, determine `Driver` vs `Normal`.
2. For each item in each module:
   - Constants/statics -> assign `ConstantId`, record type and value.
   - Type definitions -> assign `TypeDefId`, record classification
     (`UserDefined`, `Abstraction`, `Framework`), fields, variants.
     Read `#[rseq::]` attributes into the `Attrs` bag.
   - Register descriptors -> assign `RegisterDescriptorId`.
   - Register blocks -> assign `RegisterBlockId`, record fields.
   - Free functions -> assign `FunctionId` (body deferred to Pass 2).
   - Impl blocks -> assign `FunctionId` for each method.
3. For the driver module:
   - Driver struct fields -> assign `ResourceId`, `PropertyId`,
     `UserDataId` per field. Record resource kinds, taxonomic
     properties, shapes.
   - Trait impls -> assign `FunctionId` for each trait method.
   - Driver impl blocks -> assign `FunctionId` for helpers and ISRs.

After Pass 1, every entity has its ID. Cross-references (e.g., MMIO
resource -> RegisterBlockId, function call -> FunctionId) can be
resolved by ID.

**Pass 2: Body construction.** For each function with a MIR body,
construct the Regseq IR `Body`:

1. Map local declarations (`MIR LocalDecl` -> `LocalDecl` with
   `RegseqType` and interned `Symbol` name).
2. Walk basic blocks in order. For each block:
   a. Translate each statement using the mapping tables (Section 4).
   b. Translate the terminator using the mapping tables (Section 5).
   c. Apply recognition rules (see below) for domain-specific
      constructs.
3. Drop cleanup blocks (`is_cleanup` blocks ending in `resume`).
4. Build the `BodySourceMap` (resolve `Span` -> `SourceLocation`
   for each statement, terminator, and local).

### Recognition rules during body construction

The following patterns are recognized and transformed during Pass 2.
They are applied in the order listed for each terminator encountered.

**1. Intrinsic recognition.** When a `Call` terminator's callee resolves
(by `DefId`) to a function or method marked with
`#[rseq::intrinsic("canonical_name")]`, the `Call` becomes an
`IntrinsicCall` terminator. The canonical name from the attribute maps
to the `RegseqIntrinsic` variant (see spec.md Section 10.2). For method
intrinsics (e.g., `Clock::enable`, `Mmio::read_field`), the `&self`
receiver is preserved as the first entry in `args`. The resource
identity is carried by the receiver's Place expression -- it may be a
direct driver struct field access (e.g., `self.clock`), a function
parameter, or any other Place. No resource tracing or resolution is
performed during IR construction; analyses and backends derive resource
identity from the Place when needed.

**2. `?`-pattern folding.** When a `Call` or `IntrinsicCall` terminator's
return target starts with `<T as Try>::branch` (for any `Try` implementor
`T`), the `?` desugaring is recognized and folded into `error_target`. See
the `?`-pattern recognition algorithm in Section 5 for the full procedure.

**3. Core library function extraction.** Regseq specs use `#![no_std]`
-- only `core` is available (not `std`). Calls to monomorphized `core`
library functions (e.g., `Option::map`, `Option::unwrap_or`,
`Option::is_some`, `<FieldValue as Add>::add`) are extracted as
regular `Function`s with bodies, just like user-defined functions.
The compiler has access to their monomorphized MIR bodies at the
extraction stage. No special inlining is required during IR
construction -- the backend decides whether to emit a call, inline
the body, or replace with a target-specific implementation.

**4. Deref collapse for transports.** `Deref::deref` on a known
transport type (`Mmio<T>`, `I2c<T>`, etc.) is recognized and treated
as a transparent access to the inner register block. In MIR, this is
a `Call` to `<Mmio<T> as Deref>::deref`. The compiler collapses it:
the result is treated as a direct reference to the transport's inner
block, preserving the transport identity for register intrinsic
resolution.

**5. OrderedRegion detection.** Paired calls to
`regseq::internal::ordered_region_begin()` and
`regseq::internal::ordered_region_end()` are recognized by `DefId`.
The marker calls are dropped. The entry block (first block after the
begin marker) and exit block (the block containing or following the
end marker) are recorded as an `OrderedRegion { entry, exit }` on the
`Body`. Region membership is defined by reachability from `entry`
without passing through `exit` -- no dependence on block ID ordering.
Because `ordered!` regions typically contain calls (which are
terminators spanning multiple basic blocks), ordered regions are
body-level metadata on the CFG, not nested statements.

**6. Logging intrinsic extraction.** Calls to logging intrinsics
(`regseq::intrinsics::diagnostic_logging::info`, etc.) are recognized
by path. The template string is extracted from the MIR constant
argument, the restricted format subset is parsed into
`Vec<FormatPlaceholder>`, the `LogArg` carriers are stripped from the
argument array, and the result is stored in
`IntrinsicMetadata::Log { template, placeholders }`.

### Type mapping

During body construction, rustc's `ty::Ty` is mapped to `RegseqType`
using the table in spec.md Section 8 (Type construction from MIR).
All struct and enum types -- including abstraction types (`Clock`,
`Mmio<T>`, etc.) and register layout types (`ReadOnly<T,R>`,
`ReadWrite<T,R>`, etc.) -- map to `Defined(TypeDefId)`. The
`classification` on the type definition distinguishes them for
backend lowering.

### Symbol interning

All identifier strings (field names, variable names, function names,
type names, file paths) are interned via the model's `SymbolInterner`
during IR construction. By the time construction is complete, no raw
strings remain in the model -- all references are `Symbol` or `FileId`.

### Source location resolution

During IR construction, every rustc `Span` is resolved to a
`SourceLocation` (file + start/end line:col) using rustc's source map.
The resolved locations are stored in `BodySourceMap` (for body-level
nodes) and the model-level `source_locations` table (for declarations).
After construction, rustc's source map is no longer needed.

### Construction order independence

Pass 1 assigns all IDs before any body is processed. This means:
- Forward references in function bodies (calling a function defined
  later in the module) are resolved by ID lookup, not by construction
  order.
- Type references (e.g., `Defined(TypeDefId)`) are always valid because
  all type definitions are registered in Pass 1.
- Cross-module references (a helper in a normal module calling a
  function in another module) work because the entire module tree is
  processed in Pass 1.

Body construction order (Pass 2) does not affect the result. Functions
can be processed in any order.
