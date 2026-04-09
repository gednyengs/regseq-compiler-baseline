# Declaration Model Spec

## 1. Overview

The Declaration Model is the unified, progressively-filled intermediate
representation that the compiler owns. It starts with normalized extraction data
(spec-side declarations), then absorbs platform binding data, becoming the single
model that both internal analyses and backend code generators operate on.

### Role in the pipeline

```
Extraction Surfaces (13 surfaces, JSON)
    |  normalization (Phase 1: declarations populated, bindings empty)
    v
Declaration Model
    |  platform file parsing + join (Phase 2: bindings populated)
    v
Declaration Model (fully populated)
    |
    v
Internal Regseq-specific analyses
    |
    v
Backend code generators (Linux, Zephyr, ...)
```

The Declaration Model replaces what were previously three separate representations
(Declaration Model, Binding Model, Joined Artifact) with a single structure that
progressively fills in.

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

The Declaration Model is a tree of `Module`s rooted at the crate module.

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

Top-level struct and enum definitions (non-driver types).

- **name**: The type's identifier.
- **kind**: Struct or Enum.
- **fields / variants**: Structural definition.

### 3.5 Impl Blocks for Non-Driver Types

`impl` blocks for types defined in Section 3.4.

- **target_type**: The type being implemented.
- **methods**: Associated functions, each with signature and body.

### 3.6 Driver Struct (Driver module only)

The driver struct declaration. See Section 4 for detail.

### 3.7 Regseq Trait Impls (Driver module only)

Implementations of Regseq-defined traits for the driver struct. See Section 5
for detail.

### 3.8 Driver Impl Blocks (Driver module only)

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

Declaration properties:
- (TBD as extraction surfaces evolve.)

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
- **register_block_type**: The register block type parameter `T`.

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

A `DeclarationModel` with all declaration-side fields populated and all binding
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

A Phase 1 `DeclarationModel` and a parsed platform file.

#### Output

The same `DeclarationModel` with binding slots populated.

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
