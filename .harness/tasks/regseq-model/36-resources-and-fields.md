# regseq-model-36: Resource, Property, and UserData Field Types

## Description

Implement the resource, property, and user data field model types used
by the driver struct. Each resource kind has its own declaration and
binding representations with progressive population (binding starts as
None, filled at join time).

## Input Context

- `docs/discussions/models/regseq-model/execution-plan.md` (Phase 2, step 2.6)
- `docs/discussions/models/regseq-model/spec.md` (Section 4.2: Resources, Section 4.3: Properties, Section 4.4: User Data)
- `crates/regseq-model/src/identity/` (ResourceId, PropertyId, UserDataId, RegisterBlockId, Symbol)
- `crates/regseq-model/src/ir/types.rs` (RegseqType, ScalarType)
- `crates/regseq-model/src/ir/operand.rs` (LiteralValue)

## Implementation Notes

Create `crates/regseq-model/src/model/resources.rs`. Update `model/mod.rs`.

### Public API

```rust
// model/resources.rs

/// Shape of a resource field: Single, Optional, or Array.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data")]
pub enum Shape {
    /// T
    Single,
    /// Option<T>
    Optional,
    /// [T; N]
    Array(usize),
}

// --- Clock ---

#[derive(Clone, Eq, PartialEq, Debug)]
pub struct ClockResource {
    pub id: ResourceId,
    pub field_name: Symbol,
    pub shape: Shape,
    pub decl: ClockDecl,
    pub binding: Option<ClockBinding>,
}

#[derive(Clone, Eq, PartialEq, Debug)]
pub struct ClockDecl {
    pub clock_id: Symbol,
}

#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct ClockBinding {
    pub default_frequency: u64,
}

// --- Reset ---

#[derive(Clone, Eq, PartialEq, Debug)]
pub struct ResetResource {
    pub id: ResourceId,
    pub field_name: Symbol,
    pub shape: Shape,
    pub decl: ResetDecl,
    pub binding: Option<ResetBinding>,
}

#[derive(Clone, Eq, PartialEq, Debug)]
pub struct ResetDecl {
    pub reset_id: Symbol,
}

#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct ResetBinding {
    pub reset_line_id: String,
    pub reset_shared: bool,
}

// --- PinCtrl ---

#[derive(Clone, Eq, PartialEq, Debug)]
pub struct PinCtrlResource {
    pub id: ResourceId,
    pub field_name: Symbol,
    pub shape: Shape,
    pub decl: PinCtrlDecl,
    pub binding: Option<PinCtrlBinding>,
}

#[derive(Clone, Eq, PartialEq, Debug)]
pub struct PinCtrlDecl {
    pub pinctrl_id: Symbol,
}

#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct PinCtrlBinding {
    pub pin_states: Vec<String>,
}

// --- Interrupt ---

#[derive(Clone, Eq, PartialEq, Debug)]
pub struct InterruptResource {
    pub id: ResourceId,
    pub field_name: Symbol,
    pub shape: Shape,
    pub decl: InterruptDecl,
    pub binding: Option<InterruptBinding>,
}

#[derive(Clone, Eq, PartialEq, Debug)]
pub struct InterruptDecl {
    pub interrupt_id: Symbol,
}

#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct InterruptBinding {
    pub irq_number: u32,
    pub trigger_type: Option<String>,
    pub trigger_polarity: Option<String>,
    pub shared: bool,
}

// --- Mmio ---

#[derive(Clone, Eq, PartialEq, Debug)]
pub struct MmioResource {
    pub id: ResourceId,
    pub field_name: Symbol,
    pub shape: Shape,
    pub decl: MmioDecl,
    pub binding: Option<MmioBinding>,
}

#[derive(Clone, Eq, PartialEq, Debug)]
pub struct MmioDecl {
    pub mmio_id: Symbol,
    /// Reference to the register block definition (from Mmio<T> type param).
    pub register_block: RegisterBlockId,
}

#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct MmioBinding {
    pub base: u64,
    pub size: u64,
}

// --- Synchronization ---

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data")]
pub enum SyncKind {
    Mutex,
    SpinLock,
    IrqSpinLock,
    Completion,
}

#[derive(Clone, Eq, PartialEq, Debug)]
pub struct SyncResource {
    pub id: ResourceId,
    pub field_name: Symbol,
    pub sync_kind: SyncKind,
    /// The type of the protected data. None for Completion.
    pub payload_type: Option<RegseqType>,
}

// --- Property ---

#[derive(Clone, Eq, PartialEq, Debug)]
pub struct PropertyField {
    pub id: PropertyId,
    pub field_name: Symbol,
    pub field_type: ScalarType,
    pub property_id: Symbol,
    pub default_value: Option<LiteralValue>,
    pub vendor_prefix: Option<Symbol>,
    pub required: bool,
    /// Concrete value from platform file. None until join.
    pub bound_value: Option<LiteralValue>,
}

// --- User Data ---

#[derive(Clone, Eq, PartialEq, Debug)]
pub struct UserDataField {
    pub id: UserDataId,
    pub field_name: Symbol,
    /// The inner type T in Option<T>. Constrained to Copy types.
    pub inner_type: RegseqType,
}
```

### Conventions

- Progressive population: `binding` fields start as `None` and are
  filled when platform data is joined (Phase 6).
- Binding types without Symbol fields (`ClockBinding`, `ResetBinding`,
  etc.) derive serde. Declaration types with Symbol fields do not.
- `Shape` is `Copy` and derives serde.
- `SyncKind` is `Copy` and derives serde.
- `SyncResource` has no binding data and no shape -- sync primitives
  are purely spec-side.
- `PropertyField.required` is `true` if the field type is `T`, `false`
  if `Option<T>`.

## Non-Goals

- No driver struct container (regseq-model-37).
- No binding population logic (Phase 6).
- No platform file parsing changes.

## Test Plan

- Construct each resource kind with binding = None.
- Construct each resource kind with binding populated.
- Test all 3 Shape variants including Array(4).
- Test all 4 SyncKind variants.
- Serde round-trip for Shape, SyncKind, and all binding types.
- Construct a PropertyField with and without default_value.
- Construct a UserDataField.
- `cargo test -p regseq-model` passes.

## Review

After tests pass, use `codex:rescue` to review against spec.md
Section 4.2 (Resources), 4.3 (Properties), and 4.4 (User Data).
Verify all resource kinds are present, progressive population pattern
is correct, and binding types match spec. Iterate until approved.

## Quality Gate

```
cargo test -p regseq-model
cargo fmt --all -- --check
cargo clippy -p regseq-model -- -D warnings
```

## Acceptance Criteria

- `model/resources.rs` created with all types.
- Re-exported from `model/mod.rs`.
- All tests pass. Codex review approved.
