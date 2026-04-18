# regseq-model-53: Module Abstraction and Intrinsic Dependency Sets

## Description

Derive `Module.abstractions_used: BTreeSet<RegseqAbstraction>`
and `Module.intrinsics_used: BTreeSet<RegseqIntrinsic>` for
every module in the constructed `RegseqModel`. Runs after Pass 2
body construction completes for each module.

## Input Context

- `docs/discussions/models/regseq-model/spec.md` (Section 2
  module definition with abstractions_used + intrinsics_used;
  Section 10 catalogs; Section 10 derivation rule)
- `docs/discussions/models/regseq-model/ir-reference.md`
  (Section 4 + 5 IR shapes scanned during derivation)
- `crates/regseq-model/src/model/module.rs`
- `crates/regseq-model/src/model/driver.rs` (DriverStruct field
  categories)
- `crates/regseq-model/src/ir/intrinsic.rs`
- `crates/regseq-driver/src/construction.rs` (Pass 2 from tasks
  43-52)

## Implementation Notes

### Pass-3 derivation step

After Pass 2 completes for all bodies in a module, walk the
module's items and bodies once more:

1. **Abstractions** — in the Driver module, every entry in
   `DriverStruct.clocks`, `resets`, `pinctrl`, `interrupts`,
   `mmio`, and `sync` contributes the corresponding
   `RegseqAbstraction` (`Clock`, `Reset`, `PinCtrl`, `Interrupt`,
   `Mmio`, and the sync variant `Mutex`/`SpinLock`/`IrqSpinLock`
   /`Completion` from each `SyncResource.sync_kind`). In
   addition, scan every TypeDef field type, every Function
   param type, every Function return type, every `LocalDecl.ty`,
   and every `Place.projection` `Field.ty` for a
   `Defined(TypeDefId)` whose classification is
   `Abstraction(RegseqAbstraction)`. Insert each found
   abstraction into the module's `abstractions_used`.
2. **Intrinsics** — scan every body's terminators for
   `IntrinsicCall { intrinsic, .. }`. Insert each intrinsic
   into the module's `intrinsics_used`.

### Determinism

`BTreeSet` ordering ensures deterministic serialization.

### Module-tree recursion

Bottom-up: leaf modules first, parents last. A parent module's
sets do NOT include child modules' sets (per spec.md Section
2).

### No new errors

Derivation is purely observational and cannot fail given a
well-formed model. If a `Defined(TypeDefId)` references a
`TypeDefId` that does not exist, that is a Pass 1/Pass 2
invariant violation surfaced by their own error reporting.

## Non-Goals

- No backend include emission (Phase 8).
- No transitive aggregation across modules.
- No platform-binding-derived dependencies.

## Test Plan

Integration tests under `crates/regseq-driver/tests/`:

- After running the full Pass 1 + Pass 2 + this derivation step
  on PL011, assert:
  - Driver module's `abstractions_used` contains at least
    `Mmio`, `Clock`, `Interrupt`, `PinCtrl` (+ `Reset` if
    PL011 declares one; check the fixture).
  - Driver module's `intrinsics_used` contains at least
    `RegisterRead`, `RegisterReadField`, `RegisterWrite`,
    `RegisterWriteField`, `ClockEnable`, `ClockDisable`,
    `BusyWait`, `PinCtrlApply`, `LogInfo` (+ any others PL011
    actually invokes after task 50 adds the logging call).
  - A normal sibling module (if any) has its own restricted
    set.
- Hand-built minimal fixture: a module with only a TypeDef
  having no abstraction-classified fields produces empty
  `abstractions_used` and empty `intrinsics_used`.
- Hand-built fixture with a Function whose body has zero
  `IntrinsicCall`s but whose param type is `&Clock` produces
  `abstractions_used = {Clock}`, `intrinsics_used = {}`.
- `cargo test -p regseq-driver` passes.

## Review

After tests pass, use `codex:rescue` to verify the scan covers
all places abstraction types can appear (DriverStruct
resources, TypeDef fields, function params/returns, local
decls, projection field types) and that derivation is
deterministic. Iterate until approved.

## Quality Gate

```
cargo test -p regseq-driver
cargo fmt --all -- --check
cargo clippy -p regseq-driver -- -D warnings
```

## Acceptance Criteria

- PL011's Driver module has the expected `abstractions_used`
  and `intrinsics_used` sets.
- DriverStruct resource declarations contribute directly to
  `abstractions_used`.
- Two hand-built minimal cases produce the expected sets.
- Derivation is deterministic.
- Codex review approved.
