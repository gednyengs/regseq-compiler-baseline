# regseq-model-52: PinCtrl Metadata Resolution

## Description

For `PinCtrlApply` `IntrinsicCall`s, resolve the monomorphized
type parameter `S` (the pin state marker type) to a logical pin
state name by reading `#[rseq::pin_state(id = "...")]` from the
type definition's `Attrs` bag (populated in Pass 1 from the
Phase 4 `PinStateRegistry`), and store the resolved value in
`IntrinsicMetadata::PinCtrl { state_id: Symbol }`.

## Input Context

- `docs/discussions/models/regseq-model/spec.md` (Section 10.2
  PinCtrl intrinsic signatures with monomorphized `S` resolution
  rule)
- `docs/discussions/models/regseq-model/ir-reference.md`
  (Section 5 `IntrinsicMetadata::PinCtrl`)
- `docs/discussions/models/regseq-model/compiler-knowledge.md`
  (Section 2 PinCtrl)
- `crates/regseq-model/src/ir/intrinsic.rs`
  (`IntrinsicMetadata::PinCtrl`)
- `crates/regseq-model/src/model/type_def.rs` (TypeDef.attrs bag)
- `crates/regseq-driver/src/extract/pinctrl.rs` (Phase 4.2
  `#[rseq::pin_state(id = "...")]` extraction surface →
  `PinStateRegistry` in memory)
- `crates/regseq-driver/src/construction.rs` (Pass 2 from tasks
  43-51)
- `tests/fixtures/pl011-full-spec/src/driver.rs` (`DefaultState`,
  `SleepState` markers + the helpers that apply them)

## Implementation Notes

### Recognition

When task 45 produces an `IntrinsicCall` whose `intrinsic` is
`PinCtrlApply`:

1. Resolve the monomorphized type parameter `S` from the
   intrinsic's call-site `Instance::args` (the concrete type
   satisfying `S: PinState`).
2. Look up `S`'s rustc `DefId` → `TypeDefId` via
   `cross_refs.type_defs` (from `Pass1Output`), then read the
   `TypeDef.attrs` bag populated in Pass 1.
3. Locate the entry whose `key == "rseq::pin_state"`. Its `value`
   is an `AttrValue::KeyValues(Vec<AttrEntry>)`; find the inner
   entry whose `key == "id"`, match its `value` as
   `AttrValue::String(s)`, intern `s` via `SymbolInterner` as a
   `Symbol`. Any other `AttrValue` shape at that slot is a bug
   in task 41 / task 30 and should trip
   `ConstructionError::MissingPinStateAttribute`.
4. Set `metadata = Some(IntrinsicMetadata::PinCtrl { state_id
   })`.

### Hard error

If the resolved `S`'s TypeDef has no `pin_state` attribute,
emit `ConstructionError::MissingPinStateAttribute` citing the
marker type's `DefId`. Phase 4.2 attribute extraction should
already have failed this case earlier; this is a defensive
guard inside Pass 2.

### Symbol interning

`state_id` is an interned `Symbol`, not a raw `String`.

## Non-Goals

- No backend lowering of `state_id` (Phase 8).
- No verification of pin-state availability against
  platform-file pin_states list (Phase 6 legality verification).
- No pin-state synthesis.

## Test Plan

Integration tests under `crates/regseq-driver/tests/`:

- Translate the PL011 helper that calls
  `pinctrl.apply::<DefaultState>()` and assert the
  `PinCtrlApply` IntrinsicCall carries
  `metadata: Some(IntrinsicMetadata::PinCtrl { state_id })`
  where `state_id` resolves to the interned symbol for
  `"default"`.
- Translate the helper that calls
  `pinctrl.apply::<SleepState>()` and assert `state_id` is
  `"sleep"`.
- Hand-built fixture: a marker type `FooState` without
  `#[rseq::pin_state(id = "...")]` referenced by a synthetic
  `PinCtrlApply` → `ConstructionError::MissingPinStateAttribute`.
- `cargo test -p regseq-driver` passes.

## Review

After tests pass, use `codex:rescue` to verify the metadata
construction matches spec.md Section 10.2 PinCtrl, the
`state_id` is interned, `Instance::args` is used for type
parameter resolution (not string matching), and the missing
attribute case hard-errors. Iterate until approved.

## Quality Gate

```
cargo test -p regseq-driver
cargo fmt --all -- --check
cargo clippy -p regseq-driver -- -D warnings
```

## Acceptance Criteria

- PL011 `apply_default_pins_if_present()` /
  `apply_sleep_pins_if_present()` produce `PinCtrlApply`
  IntrinsicCalls with the correct `state_id` symbols.
- Missing `pin_state` attribute hard-errors with provenance.
- `state_id` is an interned `Symbol`.
- Codex review approved.
