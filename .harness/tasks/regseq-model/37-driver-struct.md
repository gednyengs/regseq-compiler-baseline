# regseq-model-37: DriverStruct and Driver Metadata

## Description

Implement the driver struct model type with struct-level metadata
(driver identity, device class, PM info) and field lists for resources,
properties, and user data.

## Input Context

- `docs/discussions/models/regseq-model/execution-plan.md` (Phase 2, step 2.7)
- `docs/discussions/models/regseq-model/spec.md` (Section 4: Driver Struct, Section 4.1: Driver Metadata)
- `crates/regseq-model/src/identity/` (TypeDefId, Symbol)
- `crates/regseq-model/src/model/resources.rs` (all resource, property, and user data types)

## Implementation Notes

Create `crates/regseq-model/src/model/driver.rs`. Update `model/mod.rs`.

### Public API

```rust
// model/driver.rs

/// The driver struct: resource manifest with metadata.
#[derive(Clone, Eq, PartialEq, Debug)]
pub struct DriverStruct {
    /// The TypeDefId of the driver struct itself.
    pub type_id: TypeDefId,
    pub name: Symbol,
    pub identity: DriverIdentity,
    pub device_class: Option<DeviceClass>,
    pub pm_info: Option<PmInfo>,
    // Resource fields
    pub clocks: Vec<ClockResource>,
    pub resets: Vec<ResetResource>,
    pub pinctrl: Vec<PinCtrlResource>,
    pub interrupts: Vec<InterruptResource>,
    pub mmio: Vec<MmioResource>,
    pub sync: Vec<SyncResource>,
    // Configuration fields
    pub properties: Vec<PropertyField>,
    pub user_data: Vec<UserDataField>,
}

/// Driver identity from #[rseq::driver(..)].
#[derive(Clone, Eq, PartialEq, Debug)]
pub struct DriverIdentity {
    pub name: Symbol,
    pub compatible: Vec<Symbol>,
    pub bus: Option<Symbol>,
}

/// Device class from #[rseq::device_class(..)].
#[derive(Clone, Eq, PartialEq, Debug)]
pub struct DeviceClass {
    pub class: Symbol,
}

/// Power management info from #[rseq::pm(..)].
#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct PmInfo {
    pub idle: bool,
    pub wakeup_capable: bool,
}
```

### Conventions

- `DriverStruct` contains Symbol fields, so no standalone serde.
  `PmInfo` can derive serde (no Symbol fields).
- Each resource category is a separate `Vec` for type-safe access.
- `device_class` and `pm_info` are `Option` because not all drivers
  carry these attributes.
- `DriverStruct.type_id` references the driver struct's own type
  definition (which is also a `TypeDef` in the module's type
  definitions).

## Non-Goals

- No module tree (regseq-model-39).
- No trait impl or impl block containers (regseq-model-38).

## Test Plan

- Construct a `DriverStruct` with all metadata populated.
- Construct a `DriverStruct` with device_class = None, pm_info = None.
- Construct a `DriverStruct` with 2 clocks, 1 mmio, 1 property, 1 user_data.
- Serde round-trip for `PmInfo`.
- `cargo test -p regseq-model` passes.

## Review

After tests pass, use `codex:rescue` to review against spec.md
Section 4 (Driver Struct) and Section 4.1 (Driver Metadata). Verify
all field categories are present and metadata fields match spec.
Iterate until approved.

## Quality Gate

```
cargo test -p regseq-model
cargo fmt --all -- --check
cargo clippy -p regseq-model -- -D warnings
```

## Acceptance Criteria

- `model/driver.rs` created with all types.
- Re-exported from `model/mod.rs`.
- All tests pass. Codex review approved.
