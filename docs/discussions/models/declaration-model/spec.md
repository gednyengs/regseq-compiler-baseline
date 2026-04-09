# Regseq Model Spec

## 1. Overview

The Regseq Model is the unified, progressively-filled intermediate representation
that the compiler owns. It starts with normalized extraction data (spec-side
declarations), then absorbs platform binding data, becoming the single model that
both internal analyses and backend code generators operate on.

### Role in the pipeline

```
Extraction Surfaces (13 surfaces, JSON)
    |  normalization (Phase 1: declarations populated, bindings empty)
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

### Design principles

- **Structural fidelity**: The model mirrors the spec's source organization (modules,
  items, ownership) rather than slicing by cross-cutting concern.
- **Bodies live with their owners**: Every function body is attached to the item that
  declares it, not collected in a separate global set.
- **Typed over stringly-typed**: Enum IDs for constructs, intrinsics, methods, and
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
- **constructs_used**: `BTreeSet<RegseqConstruct>` -- Regseq abstractions referenced
  within this module (see Section 8).
- **intrinsics_used**: `BTreeSet<RegseqIntrinsic>` -- Regseq intrinsic functions
  invoked within this module (see Section 8).

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
- **type**: The item's type.
- **value**: The constant's value or static's initializer.

### 3.3 Free Functions

Top-level function definitions (typically helper functions used in various bodies).

- **name**: The function's identifier.
- **signature**: Parameters and return type.
- **body**: MIR-based function body (see Section 7).

### 3.4 Type Definitions

Top-level struct and enum definitions (non-driver, non-register-block types).

- **name**: The type's identifier.
- **kind**: Struct or Enum.
- **fields / variants**: Structural definition.

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

The reference relationship between register blocks and descriptors forms a DAG:

```
Register Descriptors (shared, standalone)
    ^              ^
    |              |
Register Block A   Register Block B
    ^
    |
MMIO Resource (driver struct field)
```

### 3.7 Impl Blocks for Non-Driver Types

Both inherent and trait impl blocks for non-driver types (Sections 3.4, 3.5, 3.6).

- **target_type**: The type being implemented.
- **trait_name**: The trait being implemented, if this is a trait impl. `None` for
  inherent impls.
- **methods**: Associated functions, each with signature and body.
- **associated_constants**: Named constants defined in the impl block.

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
- **id**: Optional pinctrl identifier.

Binding (from `[pinctrl.<id>]` in platform file):
- **pin_states**: List of pin state names (e.g., `["default", "sleep"]`).

#### Interrupt

From `Interrupt` / `Option<Interrupt>` / `[Interrupt; N]` with `#[rseq::interrupt(..)]`.

Declaration properties:
- **id**: Interrupt identifier (required).

Binding (from `[interrupt.<id>]` in platform file):
- **trigger_type**: Trigger type (e.g., `level`, `edge`).
- **trigger_polarity**: Trigger polarity (e.g., `high`, `low`).
- **shared**: Whether the interrupt is shared.

#### Mmio

From `Mmio<T>` / `Option<Mmio<T>>` / `[Mmio<T>; N]` with `#[rseq::mmio(..)]`.

Declaration properties:
- **id**: Optional MMIO identifier.
- **register_block**: Reference to a register block definition (Section 3.6).
  Derived from the type parameter `T` in `Mmio<T>`.

Binding (from `[mmio.<id>]` in platform file):
- **base**: Base physical address.
- **size**: Region size in bytes.

#### Synchronization Primitives

From `Mutex<T>`, `SpinLock<T>`, `IrqSpinLock<T>`, `Completion`.

- **sync_kind**: One of `Mutex`, `SpinLock`, `IrqSpinLock`, `Completion`.
- **payload_type**: Generic payload type (not applicable for `Completion`).

No binding data -- synchronization primitives are purely spec-side constructs with
no platform-specific configuration.

#### Other Resource Kinds

DMA channels, DMA rings, work queues, etc. will be added as the DSL supports them.
Each will follow the same pattern: field name, shape, kind-specific declaration
properties, and optional kind-specific binding data.

### 4.3 Properties

Firmware configuration inputs read from device tree, ACPI, or similar.

From fields marked with `#[rseq::property(..)]`:

- **field_name**: The field's identifier.
- **field_type**: The scalar type.
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
- Initialized to `None` by the compiler at construction time.
- Author sets the value in the `init()` lifecycle callback.
- No `Drop` semantics needed (`Copy` bound).

Representation:
- **field_name**: The field's identifier.
- **inner_type**: The `T` in `Option<T>`.

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
- **type**: The constant's type.
- **value**: The constant's value.

### Helper Methods

Normal associated functions used internally by the driver.

- **name**: Method identifier.
- **signature**: Parameters and return type.
- **body**: MIR-based function body.

### ISR Handlers

Interrupt service routine functions marked with `#[rseq::isr]`.

- **name**: Handler identifier.
- **bind_target**: `Auto` (implicit, requires exactly one interrupt resource) or
  `ExplicitField(name)` (bound to a named interrupt resource).
- **has_index_param**: Whether the handler accepts a `usize` index parameter (required
  for array interrupt bindings).
- **context**: Always `HandlerManaged (H)`.
- **body**: MIR-based function body.

## 7. Bodies

Function bodies use a MIR-based representation in CFG (control-flow graph) form.

### Ownership

Every body is attached to the item that declares it:
- Free function bodies live under Section 3.3.
- Trait method bodies live under Section 5.
- Driver helper method bodies live under Section 6.
- ISR handler bodies live under Section 6.
- Impl method bodies live under Section 3.5.

There is no separate global body collection.

### Structure

Each body contains:
- **Local declarations**: Typed local variables and temporaries.
- **Basic blocks**: A CFG of basic blocks, each containing a sequence of statements
  and a terminator.

The detailed MIR body representation is defined by the existing `MirBodyDeclSet` /
`MirBody` types and is not further specified here.

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
recognizes the paired markers and folds them into an `OrderedFlatRegion` construct
— a first-class body-level IR element that contains the enclosed statements. This
preserves the author's sequencing intent without altering scoping semantics.

**Backend lowering**: Backends see the `OrderedFlatRegion` and emit appropriate
ordering barriers (e.g., compiler barriers in C) around the enclosed statements.

## 8. Construct and Intrinsic IDs

Each module carries two dependency sets that allow backends to determine required
includes without scanning bodies.

### 8.1 RegseqConstruct

Typed identifiers for Regseq abstractions:

```
enum RegseqConstruct {
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

A construct is present in a module's set if any item in that module references the
corresponding type.

### 8.2 RegseqIntrinsic

Typed identifiers for Regseq-provided intrinsic functions:

```
enum RegseqIntrinsic {
    BusyWait,
    Sleep,
    LogError,
    LogWarn,
    LogInfo,
    LogDebug,
    PollUntil,
    // extensible
}
```

An intrinsic is present in a module's set if any body in that module invokes the
corresponding function.

### Derivation

Both sets are derived during normalization by scanning the bodies and field types
within each module. No spec-level annotation is required from the author.

## 9. Platform Binding

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

## 10. Normalization Contract

### Phase 1: Extraction normalization

#### Input

The 13 Extraction Surfaces serialized as `RawFrontendArtifact` JSON.

#### Output

A `RegseqModel` with all declaration-side fields populated and all binding
slots set to `None`.

#### Properties

- **Deterministic**: Same input always produces the same output.
- **Locally scoped**: Each module and item is normalized independently.
- **Validated**: Unrecognized tags, missing required attributes, and structural
  violations produce explicit errors, never silent defaults.
- **Provenance**: Each normalized entity can be traced back to its originating
  extraction surface and containing item.

#### Error conditions

- Unrecognized enum tag in raw input.
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
