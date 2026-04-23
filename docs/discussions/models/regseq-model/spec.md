# Regseq Model Spec

## 1. Overview

### Background

Regseq is a DSL and compiler for authoring hardware device drivers in an
OS-agnostic way. Driver authors write specifications using the Regseq DSL
(embedded in Rust), and the compiler generates target OS-specific code (e.g.,
C for Linux, Zephyr, etc.).

A driver specification has two layers:
- **Layer 1 (spec)**: Captures the platform-independent aspects of the driver.
- **Layer 2 (platform file)**: Captures platform-specific bindings such as MMIO
  addresses, interrupt line numbers, clock frequencies, etc. Analogous to device
  tree or ACPI tables.

The DSL provides three mechanisms for capturing device driver semantics:
- **Regseq intrinsics** (the verbs): The operational vocabulary. Calls to
  pre-specified functions (`busy_wait()`, `sleep()`, `log_info()`,
  `poll::until()`) that the compiler materializes into target OS-specific
  mechanisms. The spec author says WHAT should happen; the compiler implements
  the HOW.
- **Regseq properties** (the adjectives): The declarative vocabulary. Custom
  attributes (`#[rseq::clock(id = "...")]`, `#[rseq::interrupt(id = "...")]`,
  etc.) attached to Regseq abstractions. These emerged from taxonomy research
  that identified the cross-OS commonalities in how resources are described.
- **Regseq traits** (the integration surfaces): The structural contracts.
  Callback families (`Lifecycle`, `Serial`, `PowerManagement`) that define
  the shapes OS frameworks need to integrate a driver. The compiler uses these
  to generate the right registration and hookup code for each target.

The `#[rseq::]` namespace is the syntactic mechanism that carries properties,
but the three concepts are distinct.

### Spec environment

Driver specifications are `#![no_std]` Rust crates. Only `core` is
available -- `std` is not used. This mirrors embedded systems Rust
programming and ensures the spec does not depend on OS-specific runtime
facilities (heap allocation, I/O, threading) that are incompatible with
the driver domain.

The available libraries are:
- **`core`**: Rust's core library. Provides `Option`, `Result`, basic
  traits (`Add`, `BitOr`, `Deref`, etc.), primitive types, and
  `core::fmt::Debug`. Monomorphized `core` functions (e.g.,
  `Option::map`, `Option::unwrap_or`) are available in MIR at the
  extraction stage and are extracted as regular functions.
- **`regseq`**: The Regseq facade crate. Provides framework types
  (`Duration`, `RegseqError`, `SerialConfig`, etc.), abstraction types
  (`Clock`, `ResetLine`, `Mmio<T>`, etc.), intrinsic functions, register
  access machinery, and trait definitions (`Lifecycle`, `Serial`, etc.).
  This is the domain-specific standard library for driver specs.
- **`regseq_macros`**: Procedural macros (`register_bitfields!`,
  `ordered!`). The `#[rseq::]` tool attributes are NOT proc macros --
  they are inert attributes registered by the compiler wrapper.

The compiler should never encounter `std` types (`Vec`, `String`,
`HashMap`, etc.) in the MIR. If it does, extraction produces an error.

### Relationship to the existing codebase

These spec documents (spec.md, ir-reference.md, compiler-knowledge.md)
describe the **target design** for the Regseq compiler. They are the
authoritative source of truth.

The existing codebase (`crates/regseq-model`, `crates/regseq-driver`,
`crates/regseq`, `tests/fixtures/pl011-full-spec`, etc.) reflects an
earlier evolutionary stage with a different architecture:
- Three separate models (`DeclarationModel`, `BindingModel`,
  `JoinedArtifact`) instead of the unified `RegseqModel`.
- An expression-tree semantic IR (`Expr`/`Stmt`) instead of the
  CFG-based IR with `BasicBlock`s, `Statement`s, and `Terminator`s.
- A different identity scheme (`CanonicalPath`/`ItemId`) instead of
  `DeclId` with typed wrappers.
- Missing `#[rseq::]` attributes on framework and abstraction types.
- `ErrorKind` alias instead of canonical `SerialErrors`.

Implementation work will rewrite or refactor the codebase to match
these spec documents, including the `regseq` facade crate, the
`regseq-model` IR types, the `regseq-driver` extraction pass, and the
driver fixtures (e.g., PL011 full spec). Where the spec and codebase
disagree, the spec is correct.

### Purpose

The Regseq Model is the unified, progressively-filled intermediate representation
that the compiler owns. It starts with normalized extraction data (spec-side
declarations), then absorbs platform binding data, becoming the single model that
both internal analyses and backend code generators operate on.

### Role in the pipeline

```
rustc HIR/MIR
    |  construction (Phase 1: declarations populated, bindings empty)
    |  performed by regseq-driver's rustc integration
    v
Regseq Model
    |  platform file parsing + join (Phase 2: bindings populated)
    v
Regseq Model (fully populated)
    |
    v
Internal Regseq-specific analyses
    |
    v
Backend code generators (Linux, Zephyr, ...)
```

The Regseq Model replaces what were previously three separate representations
(Declaration Model, Binding Model, Joined Artifact), merging them into the
Regseq Model as a single structure that progressively fills in.
Construction runs in a single rustc-linked pass inside `regseq-driver`;
there is no intermediate serialized artifact between rustc and the
Regseq Model.

### Design principles

- **Structural fidelity**: The model mirrors the spec's source organization (modules,
  items, ownership) rather than slicing by cross-cutting concern.
- **Bodies live with their owners**: Every function body is attached to the item that
  declares it, not collected in a separate global set.
- **Typed over stringly-typed**: Enum IDs for abstractions, intrinsics, methods, and
  classifications. No string comparison in downstream consumers.
- **Honest representation**: No synthesized data. If extraction didn't find it, it's
  absent. If a binding hasn't been joined yet, the slot is `None`.
- **Progressive population**: Declaration-side fields are populated at normalization
  time. Binding-side fields start as `None` and are filled when platform data is
  joined.

## 2. Module Tree

The Regseq Model is a tree of `Module`s rooted at the crate module.

### Module definition

A `Module` has:

- **name**: The module's identifier.
- **kind**: One of:
  - `Driver` -- contains the driver struct, Regseq trait impls, and driver impl blocks.
  - `Normal` -- contains only general-purpose items.
- **items**: The module's items (see Section 3).
- **abstractions_used**: `BTreeSet<RegseqAbstraction>` -- Regseq abstractions referenced
  within this module (see Section 10).
- **intrinsics_used**: `BTreeSet<RegseqIntrinsic>` -- Regseq intrinsic functions
  invoked within this module (see Section 10).

### Nesting rules

- Modules can nest arbitrarily.
- Exactly one module in the tree has kind `Driver`.

## 3. Module Items

A module contains the following kinds of items:

### 3.1 Sub-modules

Nested `Module` definitions (see Section 2).

### 3.2 Constants and Statics

Top-level `const` and `static` items.

- **name**: The item's identifier.
- **type**: The item's `RegseqType` (see Section 8).
- **value**: The item's `LiteralValue` (see Section 7, LiteralValue subsection).

### 3.3 Free Functions

Top-level function definitions (typically helper functions used in various bodies).
Each is represented as a `Function` (see Section 8) with no `Receiver` parameter.

### 3.4 Type Definitions

Top-level struct and enum definitions (non-driver, non-register-block types).
This includes user-defined types, Regseq abstraction types (marked with
`#[rseq::abstraction("...")]`), and Regseq framework types (marked with
`#[rseq::framework_type("...")]`).

- **name**: The type's identifier.
- **kind**: `Struct` or `Enum`.
- **classification**: One of:
  - `UserDefined` -- spec author's types with no special Regseq semantics.
  - `Abstraction(RegseqAbstraction)` -- Regseq abstraction types discovered via
    `#[rseq::abstraction("...")]`. These are wrapper types for OS-managed resources
    (e.g., `Clock`, `Interrupt`). They are represented as `Defined(TypeDefId)` in
    the type system like any other struct. The classification tells the compiler
    and backend that these types have domain-specific semantics: their methods
    wrap intrinsics, their instances on the driver struct carry resource properties
    (`#[rseq::clock(id = "...")]`, etc.), and the backend lowers them to
    platform-specific handle types.
  - `Framework(FrameworkTypeKind)` -- Regseq framework types discovered via
    `#[rseq::framework_type("...")]`. These are known types with known C lowering.

`FrameworkTypeKind` identifies the specific framework type:

```
enum FrameworkTypeKind {
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
    // extensible as DSL grows
}
```

The attribute value (e.g., `#[rseq::framework_type("serial-config")]`) maps to
the corresponding enum variant during extraction.
- **fields**: For structs, list of named fields with types.
- **variants**: For enums, list of variants, each with:
  - **name**: Variant identifier.
  - **fields**: List of fields (empty for unit variants, populated for data
    variants).
  - **discriminant**: Optional explicit discriminant value.
- **attrs**: Extensible attribute bag for future semantic annotations.

Data-carrying enum variants are lowered to C as tagged unions: a discriminant
enum plus a union of variant structs.

### 3.5 Register Descriptors

Bitfield definitions for individual registers, expanded from the `register_bitfields!`
macro. Register descriptors are standalone, reusable items -- they are not owned by
any single register block and may be referenced by multiple blocks.

- **name**: The descriptor's identifier (e.g., `DR_REG`).
- **access_width**: The register width in bits (e.g., 32 from `u32`).
- **fields**: List of bitfield definitions, each with:
  - **name**: Field identifier (e.g., `DATA`, `OE`).
  - **offset**: Bit offset within the register.
  - **width**: Number of bits.
  - **named_values**: Optional list of named constants, each with:
    - **name**: Value identifier (e.g., `ENABLED`, `DISABLED`).
    - **value**: The numeric value.

### 3.6 Register Blocks

Memory-mapped register layout definitions. A register block is a struct with
`#[rseq::register_block(..)]` that defines the ordered layout of registers at
fixed offsets. Register blocks reference register descriptors (Section 3.5) but
do not own them.

- **name**: The block's identifier (e.g., `Pl011Registers`).
- **block_attributes**: Block-level properties from `#[rseq::register_block(..)]`:
  - **endian**: Byte order (e.g., `little`, `big`).
  - **access_width**: Default access width in bits (e.g., 32).
- **fields**: Ordered list of register fields, each with:
  - **name**: Field identifier (e.g., `dr`, `cr`).
  - **access_kind**: One of `ReadWrite`, `ReadOnly`, `WriteOnly`, `Reserved`.
  - **descriptor**: Reference to a register descriptor (Section 3.5), if not
    `Reserved`. This is the descriptor type from the generic parameter
    (e.g., `DR_REG::Register` → `DR_REG`).
  - **byte_offset**: Offset within the block (derived from field ordering and
    access width).
  - **array_count**: For reserved gaps expressed as arrays (e.g.,
    `[Reserved<u32>; 4]`), the element count. `None` for single fields.

The reference relationship between register blocks, descriptors, and
transport resources forms a DAG:

```
Register Descriptors (shared, standalone)
    ^              ^
    |              |
Register Block A   Register Block B
    ^                   ^
    |                   |
Transport Resource   Transport Resource
(e.g., Mmio<A>)     (e.g., I2c<B>)
```

Register blocks and descriptors are **transport-independent**. The same
register block definition can be accessed through different transport
mechanisms (MMIO, I2C, SPI, PCIe BAR, etc.). What changes per transport
is the lifecycle (map/unmap for MMIO, bus acquire/release for I2C), the
physical access mechanism, and the transport-specific properties. The
register layout, field definitions, and access permissions are the same
regardless of transport.

### 3.7 Impl Blocks for Non-Driver Types

Both inherent and trait impl blocks for non-driver types (Sections 3.4, 3.5, 3.6).

- **target_type**: The `TypeDefId` of the type being implemented.
- **trait_name**: The trait being implemented, if this is a trait impl. `None` for
  inherent impls.
- **methods**: List of `Function`s (see Section 8). Methods have a `Receiver`
  parameter whose type is `Ref(Defined(target_type), ...)`.
- **associated_constants**: Named constants defined in the impl block, each with
  a `RegseqType` and a `LiteralValue`.

### 3.8 Driver Struct (Driver module only)

The driver struct declaration. See Section 4 for detail.

### 3.9 Regseq Trait Impls (Driver module only)

Implementations of Regseq-defined traits for the driver struct. See Section 5
for detail.

### 3.10 Driver Impl Blocks (Driver module only)

`impl DriverType` blocks providing helper methods and ISR handlers. See Section 6
for detail.

## 4. Driver Struct

The driver struct is the resource manifest for the driver. It carries struct-level
metadata and contains fields in three categories.

### 4.1 Driver Metadata

Struct-level attributes that describe the driver's identity and characteristics.

#### Driver Identity

From `#[rseq::driver(..)]`:

- **name**: Driver name string.
- **compatible**: List of device compatibility strings.
- **bus**: Optional bus/fabric identity (e.g., `integrated`, `amba`, `pci`).

#### Device Class

From `#[rseq::device_class(..)]`:

- **class**: The device class identifier (e.g., `serial_uart`).

#### PM Info

From `#[rseq::pm(..)]`:

- **idle**: Whether the driver supports idle PM.
- **wakeup_capable**: Whether the device is wakeup-capable.

#### Extensibility

Additional metadata attributes will be added as the DSL evolves and more devices
are ported.

### 4.2 Resources

OS-managed objects with lifecycle. Each resource field has:

- **field_name**: The field's identifier.
- **shape**: One of `Single`, `Optional`, or `Array(N)` -- corresponding to `T`,
  `Option<T>`, and `[T; N]` in the spec source.
- **taxonomic_properties**: Resource-kind-specific declaration properties from
  `#[rseq::]` attributes.
- **binding**: Optional binding-side data from the platform file. Starts as `None`
  after normalization, populated at join time.

Each resource kind has its own declaration and binding representations:

#### Clock

From `Clock` / `Option<Clock>` / `[Clock; N]` with `#[rseq::clock(..)]`.

Declaration properties:
- **id**: Clock identifier (required).

Binding (from `[clock.<id>]` in platform file):
- **default_frequency**: Default clock frequency in Hz.

#### Reset

From `ResetLine` / `Option<ResetLine>` / `[ResetLine; N]` with `#[rseq::reset(..)]`.

The `#[rseq::reset(id = "...")]` attribute is required, consistent with all other
resource kinds.

Declaration properties:
- **id**: Reset line identifier (required).

Binding (from `[reset.<id>]` in platform file):
- **reset_line_id**: Reset line identifier string.
- **reset_shared**: Whether the reset line is shared.

#### PinCtrl

From `PinCtrl` / `Option<PinCtrl>` / `[PinCtrl; N]` with `#[rseq::pinctrl(..)]`.

Declaration properties:
- **id**: Pinctrl identifier (required).

Binding (from `[pinctrl.<id>]` in platform file):
- **pin_states**: List of pin state names (e.g., `["default", "sleep"]`).

#### Interrupt

From `Interrupt` / `Option<Interrupt>` / `[Interrupt; N]` with `#[rseq::interrupt(..)]`.

Declaration properties:
- **id**: Interrupt identifier (required).

Binding (from `[interrupt.<id>]` in platform file):
- **irq_number**: Platform-specific interrupt number (required). The
  backend uses this to emit the interrupt registration call (e.g.,
  `request_irq()` on Linux, `IRQ_CONNECT()` on Zephyr).
- **trigger_type**: Trigger type (e.g., `level`, `edge`).
- **trigger_polarity**: Trigger polarity (e.g., `high`, `low`).
- **shared**: Whether the interrupt is shared.

#### Mmio

From `Mmio<T>` / `Option<Mmio<T>>` / `[Mmio<T>; N]` with `#[rseq::mmio(..)]`.

Declaration properties:
- **id**: MMIO identifier (required).
- **register_block**: Reference to a register block definition (Section 3.6).
  Derived from the type parameter `T` in `Mmio<T>`.

Binding (from `[mmio.<id>]` in platform file):
- **base**: Base physical address.
- **size**: Region size in bytes.

#### Synchronization Primitives

From `Mutex<T>`, `SpinLock<T>`, `IrqSpinLock<T>`, `Completion`.

- **sync_kind**: One of `Mutex`, `SpinLock`, `IrqSpinLock`, `Completion`.
- **payload_type**: The `RegseqType` of the protected data (not applicable for
  `Completion`).

No binding data -- synchronization primitives are purely spec-side entities with
no platform-specific configuration.

#### Other Resource Kinds

DMA channels, DMA rings, work queues, etc. will be added as the DSL supports them.
Each will follow the same pattern: field name, shape, kind-specific declaration
properties, and optional kind-specific binding data.

### 4.3 Properties

Firmware configuration inputs read from device tree, ACPI, or similar.

From fields marked with `#[rseq::property(..)]`:

- **field_name**: The field's identifier.
- **field_type**: The property's type, constrained to `ScalarType` (see Section 8).
- **property_id**: The firmware property key (required).
- **default_value**: Optional default literal.
- **vendor_prefix**: Optional vendor prefix string.
- **required**: `true` if the field type is `T`, `false` if `Option<T>`.
- **bound_value**: Optional concrete value from the platform file. Starts as `None`,
  populated at join time from the `[properties]` section.

### 4.4 User Data

Author-defined state fields not managed by the framework.

Rules:
- Must be typed `Option<T>` where `T: Copy`.
- No `#[rseq::]` attributes.
- Cannot depend on live resources -- user data has no ordering relationship with
  resource initialization or teardown.
- Initialized to `None` by the compiler at construction time.
- Author sets the value in the `init()` lifecycle callback.
- No `Drop` semantics needed (`Copy` bound).

Representation:
- **field_name**: The field's identifier.
- **inner_type**: The `RegseqType` of `T` in `Option<T>` (constrained to `Copy`
  types).

No binding data -- user data is purely spec-side.

## 5. Regseq Trait Impls

Implementations of Regseq-defined traits for the driver struct. Each trait impl
contains methods with signatures and MIR bodies.

### 5.1 Lifecycle

Methods:
- `init(&mut self) -> Result<(), RegseqError>`
- `deinit(&mut self) -> Result<(), RegseqError>`

### 5.2 Serial

Methods:
- `port_open(&mut self) -> Result<(), RegseqError>`
- `port_close(&mut self)`
- `configure(&mut self, config: &SerialConfig) -> Result<(), RegseqError>`
- `tx_byte(&mut self, byte: u8)`
- `rx_char(&mut self) -> Option<RxChar>`
- `tx_ready(&mut self) -> bool`
- `tx_empty(&mut self) -> bool`
- `flush_rx_fifo(&mut self)`
- `flush_tx_fifo(&mut self)`
- `send_break(&mut self, active: bool)`

### 5.3 SerialConsole

Methods:
- `console_write(&mut self, data: &[u8])`

### 5.4 PowerManagement

Methods:
- `suspend(&mut self) -> Result<(), RegseqError>`
- `resume(&mut self) -> Result<(), RegseqError>`

### 5.5 Extensibility

Additional traits will be added as the DSL supports more device classes.

### Callback Context

Each trait impl carries a callback context classification:
- **Framework (F)**: Lifecycle, PowerManagement.
- **SubsystemSerialized (S)**: Serial, SerialConsole.

## 6. Driver Impl Blocks

`impl DriverType` blocks that provide:

### Associated Constants

Named constants defined on the driver type.

- **name**: Constant identifier.
- **type**: The constant's `RegseqType` (see Section 8).
- **value**: The constant's `LiteralValue` (see Section 7, LiteralValue subsection).

### Helper Methods

Normal associated functions used internally by the driver. Each is represented
as a `Function` (see Section 8) with a `Receiver` parameter typed as a reference
to the driver struct.

### ISR Handlers

Interrupt service routine functions marked with `#[rseq::isr]`. Each is represented
as a `Function` (see Section 8) with a `Receiver` parameter. ISR-specific metadata
is carried in the function's `attrs`:

- **bind_target**: `Auto` (implicit, requires exactly one interrupt resource) or
  `ExplicitField(name)` (bound to a named interrupt resource).
- **has_index_param**: Whether the handler accepts a `usize` index parameter (required
  for array interrupt bindings).
- **context**: Always `HandlerManaged (H)`.

## 7. Identity Scheme

The Regseq Model uses typed flat IDs to reference entities. All cross-references
between items use typed IDs rather than string paths or name matching.

### Declaration-level IDs

A base type `DeclId(u32)` provides a single flat namespace for all declarations.
Typed newtype wrappers enforce that cross-references point to the correct kind of
entity:

| ID type | What it identifies |
|---|---|
| `ModuleId(DeclId)` | A module in the tree |
| `ConstantId(DeclId)` | A constant or static item (top-level or associated) |
| `FunctionId(DeclId)` | A free function, helper method, trait method, or ISR handler |
| `TypeDefId(DeclId)` | A struct or enum (user-defined, abstraction, or framework) |
| `RegisterDescriptorId(DeclId)` | A register bitfield descriptor |
| `RegisterBlockId(DeclId)` | A register block struct |
| `ResourceId(DeclId)` | A resource field on the driver struct |
| `PropertyId(DeclId)` | A property field on the driver struct |
| `UserDataId(DeclId)` | A user data field on the driver struct |

Cross-references use the typed wrapper — e.g., an MMIO resource holds a
`RegisterBlockId`, not a raw `DeclId`. This makes invalid references a
compile-time error in the compiler itself.

A dispatch enum is derivable from `DeclId` when consumers need to branch on
the kind of declaration.

A name-resolution side table maps each ID to its human-readable name/path for
error messages and diagnostics. All names and identifiers in the model --
module names, type names, field names, variant names, constant names,
function names, resource/property field names -- use interned `Symbol`
values (see ir-reference.md Section 7: Symbol Interning). Name lookups
and comparisons are O(1) integer operations. The `SymbolInterner` is
owned by the `RegseqModel`.

### Body-level IDs

Within function bodies:

- **`LocalId(u32)`** -- identifies local variables and temporaries, scoped to
  the containing function body.
- **`BasicBlockId(u32)`** -- identifies basic blocks within a function body.

These do not use `DeclId` — they are internal to each function and do not
participate in cross-body references.

### Operand

The `Operand` type is a value source for expressions. It bridges
declaration-level and body-level identities. The authoritative definition
is in ir-reference.md Section 1; the summary here is for context.

```
enum Operand {
    Copy(Place),             // bitwise copy from a place (local + projections)
    Move(Place),             // move from a place, invalidating the source
    Constant(ConstantId),    // reference to a named constant declaration
    Literal(LiteralValue),   // inline scalar value (e.g., 0x07ff, true)
}
```

`Copy` and `Move` use `Place` (a local plus a projection chain) rather
than bare `LocalId`, because operands often involve compound access
(field access, deref, indexing). Both variants are preserved for analysis
and for backends where the distinction matters (C++ `std::move`, Rust
move semantics). The C backend treats both as value copies.

### LiteralValue

Inline constant values that appear directly in expressions:

```
enum LiteralValue {
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

Each variant carries its value and implies a `RegseqType` (`LiteralValue::U32(v)` has
type `Scalar(U32)`, `LiteralValue::Bool(v)` has type `Bool`). `Unit` and `Never` do
not have literal representations -- `Unit` is structural (empty tuple), and `Never`
is a control-flow property, not a value.

This allows expressions to mix places, named constants, and literals naturally.
For example, `status & Self::RX_INTERRUPT_MASK` becomes:

```
BinaryOp {
    op: BitAnd,
    lhs: Operand::Copy(Place { local: status_local, projection: [] }),
    rhs: Operand::Constant(rx_interrupt_mask_id),
}
```

### Verified: constant identity preservation

At the MIR stage used by the compiler (`mir_drops_elaborated_and_const_checked`
with `mir_opt_level = 0`), associated constants appear as `Const::Unevaluated`
with their def-path preserved (e.g., `"driver::Pl011FullSpec::RX_INTERRUPT_MASK"`).
This holds even when the constant is an operand inside a binary expression.
During Regseq IR construction, these paths are resolved to the appropriate
`ConstantId`.

## 8. Type System

The Regseq Model has its own type system representing types that flow through
expressions, function signatures, and local declarations. It is not a Rust type
system -- no lifetimes, trait bounds, or generics. All types are concrete
(monomorphized).

### Design principles

- **Minimal to the output language**: Only represent types that survive into C
  output. Rust-specific constructs (trait objects, generators) are not
  supported. Closures are decomposed during IR construction (see ir-reference.md).
- **Domain types are Defined types with classification and intrinsics**:
  Resource types (`Clock`, `Interrupt`, `Mmio<T>`, etc.), register types
  (`ReadWrite<W,R>`, `Field<W,R>`, etc.), and synchronization types are
  represented as `Defined(TypeDefId)` in the type system. They flow
  through the IR like any other struct or enum. What makes them special
  is twofold: (1) the `classification` field on their type definition
  (Section 3.4) tells the compiler and backend about their domain
  semantics (`Abstraction`, `Framework`, or `UserDefined`), and
  (2) they carry `#[rseq::intrinsic("...")]` methods that the compiler
  recognizes as domain operations (see Section 10.2). The classification
  drives backend lowering; the intrinsic methods drive IR construction.
- **Unified type references**: All struct and enum types -- whether defined by the
  spec author, as Regseq abstractions, or as Regseq framework types -- are
  represented as `Defined(TypeDefId)`. The classification on the type definition
  (Section 3.4) distinguishes them.
- **Monomorphized**: No generics. `Option<u8>` and `Option<u32>` are distinct
  concrete types.
- **Structural equality**: Two types are the same iff they have the same structure.
  For `Defined(TypeDefId)`, same ID means same type.
- **Owned representation, not interned**: `RegseqType` is an owned enum (with
  boxed inner types for recursive variants). Type expressions are not interned
  because the compiler's type-related work is primarily pattern matching against
  known type shapes and looking up type definitions by `TypeDefId` -- tasks that
  work well with an owned structural enum. Type definition names (e.g., the name
  of a struct or enum) use interned `Symbol` values (Section 7).

### RegseqType

```
enum RegseqType {
    Scalar(ScalarType),
    Bool,
    Unit,
    Never,
    Option(RegseqType),
    Result(RegseqType, RegseqType),
    Array(RegseqType, usize),
    Slice(RegseqType),
    Ref(RegseqType, Mutability),
    Defined(TypeDefId),
}
```

| Variant | Represents | C lowering (example) |
|---|---|---|
| `Scalar(U32)` | Fixed-width integer `u32` | `uint32_t` |
| `Bool` | Boolean | `bool` |
| `Unit` | Rust's `()` | `void` (or omitted) |
| `Never` | Diverging expression `!` | `__attribute__((noreturn))` |
| `Option(T)` | Monomorphized `Option<T>` | value + flag, or nullable pointer |
| `Result(T, E)` | Monomorphized `Result<T, E>` | error code, or struct |
| `Array(T, N)` | Fixed-size array `[T; N]` | `T[N]` |
| `Slice(T)` | Slice reference `&[T]` | pointer + length |
| `Ref(T, M)` | Reference `&T` or `&mut T` | `const T *` or `T *` |
| `Defined(id)` | Any struct or enum type | backend-determined |

`Option` and `Result` are explicit type constructors rather than
`Defined(TypeDefId)` because they are parametric (same definition, different inner
types) and their C lowering is structural, depending on the inner type.

### ScalarType

```
enum ScalarType {
    U8, U16, U32, U64,
    I8, I16, I32, I64,
    Usize, Isize,
}
```

`Usize` and `Isize` are target-dependent types (32-bit on 32-bit targets,
64-bit on 64-bit targets). The backend resolves the width from the platform
file's `arch` field. In C, they lower to `size_t` / `ssize_t` or to a
fixed-width type matching the target architecture.

`Bool` is separate from `ScalarType` because it has no bit-width in the integer
sense, maps to C's `_Bool` (not `uint8_t`), and should not participate in
arithmetic operations.

### Mutability

```
enum Mutability {
    Shared,     // &T — corresponds to const in C
    Mutable,    // &mut T — corresponds to non-const in C
}
```

Mutability in the type system applies to **references only** (`Ref` variant).
Binding mutability (`let` vs `let mut`) is a property of the local variable
declaration in the body (Section 9), not part of the type.

### Function parameters

Function parameters carry both a type and a semantic role:

```
struct Param {
    kind: ParamKind,
    name: Identifier,
    ty: RegseqType,
}

enum ParamKind {
    Receiver,   // self — this parameter carries method semantics
    Regular,    // normal parameter
}
```

A function with `params[0].kind == Receiver` is a method. The receiver's type
(in the `ty` field) determines the target type -- e.g.,
`Ref(Defined(clock_type_id), Mutable)` for a `&mut Clock` method. `ParamKind`
carries only the semantic role; mutability and target type live in the `Ref` type.

All callable entities (free functions, trait methods, helper methods, ISR handlers,
abstraction wrapper methods) share the same `Function` representation:

```
Function {
    id: FunctionId,
    params: [Param],
    return_type: RegseqType,
    body: Body,
    attrs: Attrs,
}
```

Domain-specific classification (trait method, ISR handler, etc.) comes from
structural position in the model (which section owns the function) and from the
`attrs` field.

### Attrs

`Attrs` is an extensible attribute bag carried by `Function`, `TypeDef`, and
other model entities. Attributes are captured generically from `#[rseq::attrs(..)]`
annotations in the source. No concrete attribute keys are defined at this time --
the mechanism exists so the compiler can recognize new semantic annotations without
requiring structural changes to the model.

Examples of future attributes:
- ISR binding metadata (bind target, index parameter, context classification)
- Callback context (Framework, SubsystemSerialized, HandlerManaged)
- Inlining hints
- Backend-specific annotations

### What is absent from the type system

| Absent | Reason |
|---|---|
| String / `&str` | Format strings are carried as structured metadata on logging `IntrinsicCall`s (see `IntrinsicMetadata::Log` in ir-reference.md), not as values in the type system. Add `Str` variant if strings need to flow as general values. |
| Lifetimes | No meaning in C output. |
| Trait bounds | Resolved at Rust compile time. |
| Generics | Everything monomorphized. |
| Closures | Decomposed during IR construction: body becomes a `Function`, captures become an `Adt`. `poll::until` predicates handled by intrinsic mechanism. Add when needed. |
| Raw pointers, function pointers, tuples | Not in current specs. Add when needed. |

### Type construction from MIR

During IR construction, rustc's `ty::Ty` is mapped to `RegseqType`:

| rustc `ty::Ty` | `RegseqType` |
|---|---|
| `ty::Uint(U32)` | `Scalar(U32)` |
| `ty::Bool` | `Bool` |
| `ty::Tuple([])` | `Unit` |
| `ty::Never` | `Never` |
| `ty::Adt(Option, [T])` | `Option(map(T))` |
| `ty::Adt(Result, [T, E])` | `Result(map(T), map(E))` |
| `ty::Array(T, n)` | `Array(map(T), n)` |
| `ty::Ref(_, ty::Slice(T), _)` | `Slice(map(T))` |
| `ty::Ref(_, T, mutability)` | `Ref(map(T), mutability)` |
| `ty::Adt(known_type, _)` | `Defined(lookup_type_def_id)` |
| `ty::Adt(abstraction_type, _)` | `Defined(lookup_type_def_id)` |

All struct and enum types -- user-defined, framework, and abstraction --
map to `Defined(TypeDefId)`. The `classification` on the type definition
(Section 3.4) distinguishes them. Abstraction types are recognized by
their `#[rseq::abstraction("...")]` attribute during extraction.

## 9. Bodies

The Regseq Model uses its own custom IR constructed from rustc's MIR. This IR is
not a Rust model — it is a domain-specific IR that speaks in Regseq concepts.
Concrete IR definitions (enums, fields, MIR mappings) are in
[ir-reference.md](ir-reference.md).

### IR design

The custom IR contains:

- **Standard expressions**: arithmetic, logical, bitwise, comparison — operations
  common to any C-like language.
- **Augmented expressions**: domain-specific operations elevated to first-class IR
  constructs (e.g., register read, register write, intrinsic calls). These carry
  typed references back to declarations via the identity scheme (Section 7).
- **Ordered regions**: body-level SESE (single-entry single-exit) metadata
  with ordering guarantees (e.g., `OrderedRegion { entry, exit }`).

By elevating domain operations to first-class constructs, analyses and backends
work with semantic operations directly rather than reverse-engineering intent from
generic function calls.

### Ownership

Every body is attached to the item that declares it:
- Free function bodies live under Section 3.3.
- Trait method bodies live under Section 5.
- Driver helper method bodies live under Section 6.
- ISR handler bodies live under Section 6.
- Impl method bodies live under Section 3.7.

There is no separate global body collection.

### Structure

Each body contains:
- **Local declarations**: Local variables and temporaries, each with:
  - **id**: `LocalId` -- unique within this body.
  - **ty**: `RegseqType` (see Section 8) -- the local's type.
  - **mutable**: Whether the binding is mutable (`let mut` vs `let`). This is a
    property of the declaration, not the type (see Section 8, Mutability).
- **Basic blocks**: A CFG of basic blocks (identified by `BasicBlockId`), each
  containing a sequence of statements and a terminator.

### Ordered Flat Regions

The `ordered!` macro in spec source enforces sequential execution of a sequence of
statements without introducing a new scope (no lifetime or drop-order changes).

**Source-level expansion**: The macro expands to flat statements bracketed by
compiler hint calls:
- `regseq::internal::ordered_region_begin()`
- `regseq::internal::ordered_region_end()`

The `regseq::internal` namespace is reserved for compiler markers and hints that
are not real intrinsics — they are not lowered by backends directly.

**Regseq Model representation**: During model construction, the compiler
recognizes the paired markers and drops them. The entry block (first
block after the begin marker) and exit block (block following the end
marker) are recorded as an `OrderedRegion { entry, exit }` on the
`Body` (see ir-reference.md Section 6). Region membership is defined
by reachability from `entry` without passing through `exit` (SESE
invariant). This is body-level metadata on the CFG, not a nested
statement, because `ordered!` regions typically
contain calls and intrinsics which are terminators spanning multiple basic
blocks.

**Backend lowering**: Backends see the `OrderedRegion` SESE region and emit
compiler barriers at the entry, normal exit, and each abnormal exit
(error_target edges that leave the region). See compiler-knowledge.md
Section 2 for the five CFG modes and concrete barrier placement examples.

## 10. Abstraction and Intrinsic IDs

Each module carries two dependency sets that allow backends to determine required
includes without scanning bodies.

### 10.1 RegseqAbstraction

Typed identifiers for Regseq abstractions:

```
enum RegseqAbstraction {
    Clock,
    Reset,
    PinCtrl,
    Interrupt,
    Mmio,
    Mutex,
    SpinLock,
    IrqSpinLock,
    Completion,
    // extensible: DmaChannel, DmaRing, WorkQueue, ...
}
```

An abstraction is present in a module's set if any item in that module references the
corresponding type.

### 10.2 RegseqIntrinsic

Typed identifiers for Regseq-provided intrinsic functions:

```
enum RegseqIntrinsic {
    // -- Register operations (transport-agnostic) --
    //
    // These operate on registers through a transport resource (MMIO,
    // PCIe BAR, I2C, SPI, etc.). The transport kind is determined by
    // the receiver argument's Place expression (the first arg in
    // IntrinsicCall.args), not by the intrinsic variant. Register
    // definitions are transport-independent and reusable across
    // transport kinds.

    RegisterRead,       // read full register
    RegisterReadField,  // read a specific field (mask + shift)
    RegisterWrite,      // write full register
    RegisterWriteField, // write a specific field

    // RegisterPoll and RegisterPollField require closure support
    // for the terminating condition predicate. Deferred until closure
    // support is added to the IR.
    //
    // RegisterPoll,      // poll full register with condition closure
    // RegisterPollField, // poll a specific field with condition closure

    // -- Clock lifecycle --
    ClockEnable,        // fallible
    ClockDisable,
    ClockGetRate,

    // -- Reset lifecycle --
    ResetAssert,        // fallible
    ResetDeassert,      // fallible
    ResetToggle,        // fallible; assert + delay + deassert as one operation

    // -- PinCtrl lifecycle --
    PinCtrlApply,       // fallible; applies a named pin state

    // -- Timing --
    BusyWait,
    Sleep,

    // -- Logging --
    LogError,
    LogWarn,
    LogInfo,
    LogDebug,

    // -- Polling (general) --
    PollUntil,          // deferred: requires closure support

    // -- Synchronization (deferred) --
    // MutexLock, MutexUnlock, SpinLockLock, SpinLockUnlock,
    // IrqSpinLockLock, IrqSpinLockUnlock, CompletionWait,
    // CompletionComplete, etc.
    // Deferred: define when a driver using sync primitives is ported.

    // extensible: transport lifecycle (MmioMap, I2cAcquire, ...), etc.
}
```

Transport lifecycle operations (MMIO map/unmap, interrupt request/free)
are performed in the compiler-generated pre-init and teardown contexts,
not as spec-author-invoked intrinsics. Their representation is a backend
code generation concern, not part of the spec-extracted intrinsic catalog.

**Resource cleanup on error paths.** When a lifecycle callback (e.g.,
`init()`) returns `Err`, the compiler generates rollback code on error
paths using dominator-based analysis. For each error exit point, the
compiler identifies reversible intrinsics (e.g., `ClockEnable` /
`ClockDisable`) that dominate that exit and generates the corresponding
reverse operations in reverse order. This produces the standard C
goto-chain cleanup pattern. The framework does NOT call the teardown
callback (e.g., `deinit()`) when `init()` fails -- the generated
rollback is the sole cleanup mechanism.

See compiler-knowledge.md Section 2 for the full analysis, handled
cases, and planned extensions for conditional and interprocedural
cleanup.

#### Intrinsic recognition mechanism

Intrinsics are methods on abstraction types or free functions, marked
with `#[rseq::intrinsic("canonical_name")]`. The attribute carries the
canonical name the compiler uses to identify the intrinsic. If omitted,
the canonical name defaults to the function name.

```
#[rseq::intrinsic("clock_enable")]    // explicit canonical name
pub fn enable(&self) -> Result<(), RegseqError> { ... }

#[rseq::intrinsic]                     // canonical name = function name
pub fn busy_wait(duration: Duration) { ... }
```

The compiler recognizes intrinsics during IR construction by checking
for the `#[rseq::intrinsic]` attribute on the call target. This
produces a single `Call` terminator in MIR with no wrapper indirection
-- the intrinsic is the direct call target.

**Two kinds of intrinsics:**

- **Method intrinsics** on abstraction types (`Clock::enable`,
  `Mmio::read_field`, `PinCtrl::apply`). The `&self` receiver is the
  resource instance, preserved as the first argument in
  `IntrinsicCall.args`. The receiver's Place expression identifies the
  resource -- it may be a direct struct field access, a function
  parameter, or any other valid Place. This allows driver authors to
  write helper functions that take abstraction types as parameters.
- **Free-function intrinsics** for operations without a resource
  receiver (`busy_wait`, `sleep`, logging). Called directly by the spec
  author.

#### Register access intrinsic architecture

Register access intrinsics are methods on transport types (`Mmio<T>`,
`I2c<T>`, etc.). Register definitions and the register block layout
are defined together by the `regseq::register_block!` macro, which
produces a module containing the block layout type and per-register
scoped field constants.

**Unified register block definition:**

```
regseq::register_block! {
    #[endian = "little", access_width = 32]
    pub pl011 {
        dr: ReadWrite {
            DATA OFFSET(0) NUMBITS(8) [],
            OE OFFSET(11) NUMBITS(1) [],
        },
        fr: ReadOnly {
            RXFE OFFSET(4) NUMBITS(1) [],
            TXFF OFFSET(5) NUMBITS(1) [],
        },
        cr: ReadWrite {
            UARTEN OFFSET(0) NUMBITS(1) [DISABLED = 0, ENABLED = 1],
            TXE OFFSET(8) NUMBITS(1) [DISABLED = 0, ENABLED = 1],
            RXE OFFSET(9) NUMBITS(1) [DISABLED = 0, ENABLED = 1],
        },
        // ...
    }
}
```

The macro generates:
- `pl011::Regs` -- the `#[repr(C)]` layout type, used as `T` in
  `Mmio<T>`.
- `pl011::dr`, `pl011::fr`, `pl011::cr`, etc. -- per-register modules
  with scoped `Field<W, R>`, `FieldValue<W, R>`, and `RegisterId<W, R>`
  constants. Each module has a unique `Register` marker enum scoped to
  that block + register combination.
- `pl011::dr::OFFSET`, `pl011::fr::OFFSET`, etc. -- per-register byte
  offset constants for the compiler's identity resolution.
- `RegisterOf<pl011::Regs>` trait impls for each register, carrying the
  access kind (`ReadWriteAccess`, `ReadOnlyAccess`, `WriteOnlyAccess`).

**Register access methods on the transport:**

```
impl<Block> Mmio<Block> {
    #[rseq::intrinsic("mmio_read")]
    pub fn read<W: RegWidth, R: RegisterOf<Block>>(
        &self, reg: RegisterId<W, R>,
    ) -> W
    where R::Access: ReadAccess { ... }

    #[rseq::intrinsic("mmio_read_field")]
    pub fn read_field<W: RegWidth, R: RegisterOf<Block>>(
        &self, field: Field<W, R>,
    ) -> W
    where R::Access: ReadAccess { ... }

    #[rseq::intrinsic("mmio_write")]
    pub fn write<W: RegWidth, R: RegisterOf<Block>>(
        &self, reg: RegisterId<W, R>, value: W,
    )
    where R::Access: WriteAccess { ... }

    #[rseq::intrinsic("mmio_write_field")]
    pub fn write_field<W: RegWidth, R: RegisterOf<Block>>(
        &self, values: FieldValue<W, R>,
    )
    where R::Access: WriteAccess { ... }
}
```

`modify` (read-modify-write) is a regular method, not an intrinsic. It
calls `read` then `write` internally, producing two `IntrinsicCall`
terminators in the IR.

**Fallibility.** MMIO register access is infallible (volatile
memory-mapped reads/writes cannot fail), so these intrinsics return
bare values and produce `error_target: None` in the IR. Future
transports (e.g., I2C, SPI) may define fallible register intrinsics
that return `Result<W, RegseqError>`. The compiler derives fallibility
from the intrinsic function's return type: bare `W` means infallible,
`Result<W, RegseqError>` means fallible (`error_target: Some(bb)`).
Intrinsic canonical names are per-transport (`mmio_read_field` vs. a
future `i2c_read_field`), so no existing intrinsic changes when a new
transport is added.

**`RegisterOf` trait with access control:**

```
trait RegisterOf<Block> {
    type Access;
}

trait ReadAccess {}
trait WriteAccess {}

struct ReadOnlyAccess;
struct WriteOnlyAccess;
struct ReadWriteAccess;

impl ReadAccess for ReadOnlyAccess {}
impl ReadAccess for ReadWriteAccess {}
impl WriteAccess for WriteOnlyAccess {}
impl WriteAccess for ReadWriteAccess {}
```

The `R: RegisterOf<Block>` bound prevents cross-block field usage
(rustc rejects using `pl011::dr::DATA` with `Mmio<other_block::Regs>`).
The `R::Access: ReadAccess` / `WriteAccess` bounds prevent wrong-kind
access (rustc rejects writing a ReadOnly register).

**Identity resolution.** After monomorphization:

```
Mmio::<pl011::Regs>::read_field::<u32, pl011::dr::Register>(
    &self.regs,      // &Mmio<pl011::Regs>
    dr::DATA         // Field<u32, pl011::dr::Register>
)
```

| Source | Resolves to |
|---|---|
| `&self` (`&Mmio<pl011::Regs>`) | Transport kind + block identity (from receiver arg's Place expression) |
| `R` (`pl011::dr::Register`) | Register identity (scoped to block, unique) + offset (from `pl011::dr::OFFSET`) |
| `field` argument | Field mask + shift |
| `R::Access` | Access kind (ReadWrite, ReadOnly, WriteOnly) |

**Authored surface:**

```
use pl011::{dr, fr, cr, lcr_h, imsc, mis};

self.regs.write_field(cr::UARTEN::ENABLED + cr::TXE::ENABLED);
self.regs.modify(lcr_h::FEN::DISABLED);
let status = self.regs.read(mis::REG);
if self.regs.read_field(fr::RXFE) != 0 { ... }
```

#### Clock intrinsic signatures

Clock intrinsics are methods on the `Clock` abstraction type, marked
with `#[rseq::intrinsic("...")]`. The `&self` receiver is the resource
instance, preserved as the first argument in `IntrinsicCall.args`.

```
impl Clock {
    #[rseq::intrinsic("clock_enable")]
    pub fn enable(&self) -> Result<(), RegseqError> { ... }  // fallible

    #[rseq::intrinsic("clock_disable")]
    pub fn disable(&self) { ... }                             // infallible

    #[rseq::intrinsic("clock_get_rate")]
    pub fn get_rate(&self) -> u32 { ... }                     // infallible
}
```

`Clock` is an opaque abstraction type with no public constructor. The
spec author obtains `&Clock` from a driver struct resource field
(`#[rseq::clock(id = "...")]`) or as a function parameter. The `&self`
receiver's Place expression in the IntrinsicCall args identifies the
resource instance. For direct struct field access, the Place resolves
to a specific resource. For function parameters, the resource identity
flows through the call site -- enabling helper functions that operate
on any Clock instance.

#### Reset intrinsic signatures

Reset intrinsics are methods on the `ResetLine` abstraction type.

```
impl ResetLine {
    #[rseq::intrinsic("reset_assert")]
    pub fn assert(&self) -> Result<(), RegseqError> { ... }       // fallible

    #[rseq::intrinsic("reset_deassert")]
    pub fn deassert(&self) -> Result<(), RegseqError> { ... }     // fallible

    #[rseq::intrinsic("reset_toggle")]
    pub fn toggle(&self, duration: Duration) -> Result<(), RegseqError> { ... }  // fallible
}
```

`reset_assert` and `reset_deassert` are the primitive operations.
`reset_toggle` is a single atomic operation (assert + delay + deassert)
that maps to dedicated OS APIs (e.g., Linux `reset_control_reset()`).

`Duration` is a framework type (Section 3.4) with known C lowering.

#### PinCtrl intrinsic signatures

PinCtrl intrinsics are methods on the `PinCtrl` abstraction type. The
pin state is a type parameter.

```
impl PinCtrl {
    #[rseq::intrinsic("pinctrl_apply")]
    pub fn apply<S: PinState>(&self) -> Result<(), RegseqError> { ... }  // fallible
}
```

After monomorphization, `S` resolves to a concrete pin state type
(e.g., `DefaultState`). The compiler reads
`#[rseq::pin_state(id = "...")]` from the type definition's `Attrs`
bag to get the logical pin state name (see compiler-knowledge.md
Section 2: PinCtrl).

```
#[rseq::pin_state(id = "default")]
pub struct DefaultState;
impl PinState for DefaultState {}

#[rseq::pin_state(id = "sleep")]
pub struct SleepState;
impl PinState for SleepState {}
```

An intrinsic is present in a module's set if any body in that module invokes the
corresponding function.

### Derivation

Both sets are derived during normalization by scanning the bodies and field types
within each module. No spec-level annotation is required from the author.

## 11. Platform Binding

Platform binding data comes from a platform file (e.g., `platform.toml`) and is
joined into the model after normalization.

### Platform Metadata

From the `[platform]` and `[bus]` sections:

- **os**: Target operating system (e.g., `linux`).
- **board**: Target board identifier (e.g., `raspberry-pi-4`).
- **arch**: Target architecture (e.g., `aarch64`).
- **bus_binding**: Bus-specific binding data. For AMBA: `amba_id`, `revision`.

Platform metadata is stored alongside driver metadata (Section 4.1), giving
consumers a single place to find both spec-declared and platform-provided identity.

### Resource Binding

Each resource's binding slot is populated by matching the resource's declared `id`
against the platform file's keyed sections (`[clock.<id>]`, `[interrupt.<id>]`,
etc.). See per-resource-kind binding fields in Section 4.2.

### Property Binding

Concrete property values from the `[properties]` section are matched to declared
properties by `property_id` and stored in the `bound_value` field (Section 4.3).

### Join rules

- A platform section with no matching declaration is an error (unknown resource).
- A required resource declaration with no matching platform section is an error
  (missing binding).
- An optional resource declaration with no matching platform section leaves the
  binding as `None` (valid -- the resource is absent on this platform).

## 12. Normalization Contract

### Phase 1: Construction

#### Input

rustc HIR/MIR, accessed through `regseq-driver`'s rustc integration.
Construction walks the driver spec crate directly -- there is no
intermediate serialized artifact.

#### Output

A `RegseqModel` with all declaration-side fields populated and all binding
slots set to `None`.

#### Properties

- **Deterministic**: Same input always produces the same output.
- **Locally scoped**: Each module and item is constructed independently.
- **Validated**: Unrecognized constructs, missing required attributes, and
  structural violations produce explicit errors, never silent defaults.
- **Provenance**: Each constructed entity can be traced back to its source
  location and containing item.

#### Error conditions

- Unrecognized construct in the input (e.g., an unsupported type shape or
  an unknown `#[rseq::]` attribute form).
- Missing required attribute (e.g., `clock` without `id`).
- Structural violation (e.g., driver struct in multiple modules).
- Type constraint violation (e.g., user data field not `Option<T: Copy>`).

### Phase 2: Platform join

#### Input

A Phase 1 `RegseqModel` and a parsed platform file.

#### Output

The same `RegseqModel` with binding slots populated.

#### Properties

- **Deterministic**: Same declaration model + platform file always produces the same
  output.
- **ID-matched**: Resources and properties are joined by their declared identifiers.
- **Validated**: Unknown platform sections and missing required bindings produce
  explicit errors.

#### Error conditions

- Platform section references an `id` with no matching resource declaration.
- Required resource has no corresponding platform section.
- Type mismatch between declared property type and bound value.

## 13. Verification

### Phase legality verification

Each phase in the pipeline has explicit post-conditions that are checked automatically
after the phase completes. This ensures that phase boundaries are enforced structurally,
not by convention.

After Phase 1 (extraction normalization):
- All declaration-side fields must be populated.
- All binding slots must be `None`. A populated binding slot indicates data leaking
  across the phase boundary.
- Module tree invariants hold (exactly one Driver module, no duplicate item names, etc.).

After Phase 2 (platform join):
- All required resource bindings must be populated.
- Optional resource bindings may remain `None`.
- Property `bound_value` fields for required properties must be populated.
- All resource IDs referenced by the platform file must match a declaration.

Before backend emission:
- The model must satisfy all backend-specific legality checks. For example, the Linux
  backend may require interrupt bindings to carry `trigger_type`. Other backends may
  impose their own requirements.

Phase legality verification catches errors at the boundary where they originate, not
downstream where they manifest as confusing failures.

### Verification after every transformation

Beyond phase boundaries, verification runs after every transformation applied to
the model. If a transformation produces invalid IR, the error is caught immediately
rather than propagating silently through subsequent passes.

Verification levels:
- **Module level**: Structural invariants (one Driver module, no duplicate names).
- **Driver struct level**: Resource attribute completeness, no duplicate IDs, user data
  field constraints.
- **Body level**: All referenced IDs resolve to existing declarations. Operand types
  match operation expectations. Context-specific legality (no `sleep()` in ISR handler
  context, no writes to read-only registers).
- **Cross-item level**: Register block references from MMIO resources resolve. Function
  call targets exist. Interrupt ISR bindings match declared interrupt resources.

The verifier is built into the pipeline infrastructure, not left to individual consumers
to invoke. Every pass that modifies the model triggers verification automatically.
