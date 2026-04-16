# regseq-model-27: RegseqIntrinsic, RegseqAbstraction, IntrinsicMetadata

## Description

Implement the intrinsic and abstraction identifier enums and the
structured metadata carried by IntrinsicCall terminators.

## Input Context

- `docs/discussions/models/regseq-model/execution-plan.md` (Phase 1, step 1.4)
- `docs/discussions/models/regseq-model/spec.md` (Section 10: Abstraction and Intrinsic IDs)
- `docs/discussions/models/regseq-model/ir-reference.md` (Section 5: IntrinsicMetadata)
- `crates/regseq-model/src/identity/` (Symbol, RegisterBlockId)

## Implementation Notes

Create `crates/regseq-model/src/ir/intrinsic.rs`. Update `ir/mod.rs`.

### Public API

```rust
// ir/intrinsic.rs

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data")]
pub enum RegseqIntrinsic {
    RegisterRead,
    RegisterReadField,
    RegisterWrite,
    RegisterWriteField,
    ClockEnable,
    ClockDisable,
    ClockGetRate,
    ResetAssert,
    ResetDeassert,
    ResetToggle,
    PinCtrlApply,
    BusyWait,
    Sleep,
    LogError,
    LogWarn,
    LogInfo,
    LogDebug,
    PollUntil,
}

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data")]
pub enum RegseqAbstraction {
    Clock,
    Reset,
    PinCtrl,
    Interrupt,
    Mmio,
    Mutex,
    SpinLock,
    IrqSpinLock,
    Completion,
}

#[derive(Clone, Eq, PartialEq, Debug)]
pub enum IntrinsicMetadata {
    Log {
        template: String,
        placeholders: Vec<FormatPlaceholder>,
    },
    Register {
        register_block: RegisterBlockId,
        register_offset: u32,
        access_width: u32,
        field: Option<RegisterFieldMeta>,
    },
    PinCtrl {
        state_id: Symbol,
    },
}

#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct FormatPlaceholder {
    pub format: FormatKind,
}

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data")]
pub enum FormatKind {
    Display,
    LowerHex,
    UpperHex,
    LowerHexPrefixed,
    UpperHexPrefixed,
    PaddedHex { prefix: bool, uppercase: bool, width: u8 },
    Pointer,
}

#[derive(Clone, Eq, PartialEq, Debug)]
pub struct RegisterFieldMeta {
    pub mask: u64,
    pub shift: u32,
    pub name: Symbol,
}
```

### Conventions

- `RegseqIntrinsic` has 18 variants (not 17 -- PollUntil was added).
- `IntrinsicMetadata::Log.template` is a `String`, not `Symbol`, because
  templates may be long and are not identifiers.
- `RegisterFieldMeta.mask` is `u64` to accommodate registers up to 64 bits wide.
- **Serde**: `IntrinsicMetadata::PinCtrl.state_id` and
  `RegisterFieldMeta.name` are `Symbol` fields which do not implement
  standalone serde. Do NOT derive serde on types containing Symbol
  fields yet. Serde will be added when model-level serialization is
  built. Types without Symbol fields (`RegseqIntrinsic`,
  `RegseqAbstraction`, `FormatPlaceholder`, `FormatKind`) CAN derive
  serde with adjacently-tagged format.

## Non-Goals

- No Statement or Terminator types (regseq-model-28, 29).
- No intrinsic recognition logic (Phase 5).

## Test Plan

- Construct each of the 18 `RegseqIntrinsic` variants.
- Construct each of the 9 `RegseqAbstraction` variants.
- Construct each `IntrinsicMetadata` variant (Log, Register, PinCtrl).
- Construct each `FormatKind` variant including `PaddedHex`.
- Construct `RegisterFieldMeta` with mask, shift, name.
- `cargo test -p regseq-model` passes.

## Review

After tests pass, use `codex:rescue` to review against spec.md Section 10
and ir-reference.md Section 5. Iterate until approved.

## Quality Gate

```
cargo test -p regseq-model
cargo fmt --all -- --check
cargo clippy -p regseq-model -- -D warnings
```

## Acceptance Criteria

- `ir/intrinsic.rs` created with all types.
- Re-exported from `ir/mod.rs`.
- All tests pass. Codex review approved.
