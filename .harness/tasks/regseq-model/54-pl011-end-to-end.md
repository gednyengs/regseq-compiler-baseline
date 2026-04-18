# regseq-model-54: PL011 End-to-End IR Construction Validation

## Description

Lock the integrated Phase 5 pipeline by compiling the PL011
fixture crate through rustc with `regseq-driver` wired in,
running `build_pass1_output()` (Pass 1 entry that also returns
`CrossRefMaps`) + Pass 2 + dependency-set derivation, asserting
per-function structural invariants for all 9 incremental test
targets, and serializing each function's `Body` to JSON goldens
under `tests/fixtures/pl011-full-spec/expected/ir/`.

This task does not introduce new construction logic. It
exercises tasks 41-53 end-to-end against PL011 and freezes the
`RegseqModel`-level JSON representation so Phase 6 (normalize/
join) and Phase 7 (consumer migration) cannot regress the IR
shape silently.

## Input Context

- `docs/discussions/models/regseq-model/spec.md` (entire spec)
- `docs/discussions/models/regseq-model/ir-reference.md` (entire
  ir-reference; body shape is what the goldens lock)
- `.harness/spec-batches/regseqmodel-phase-5.md` (Acceptance
  Criteria + Phase boundary section)
- `crates/regseq-driver/src/construction.rs` (full pipeline from
  tasks 41-53)
- `crates/regseq-driver/src/callbacks.rs` (where construction
  is wired into the rustc callback)
- `tests/fixtures/pl011-full-spec/src/driver.rs` (the 9 test
  targets at the line numbers below, including the `info!`
  call added by task 50)

## Implementation Notes

Add `crates/regseq-driver/tests/pl011_construction.rs`
(integration test). Compile the PL011 fixture through rustc
with the `regseq-driver` callback and the pinned compiler
config (release-profile MIR semantics from
`regseq-driver-25`). Run the full construction pipeline and
assert.

### Per-function structural assertions

| Tier | Function | Line | Structural assertion |
|---|---|---|---|
| 1 | `disable_uart` | 69 | Straight-line body; no SwitchInt; one or more `IntrinsicCall`s for register writes; no `error_target` populated. |
| 2a | `tx_ready` | 338 | SwitchInt on bool discriminant; two-target SwitchTargets. |
| 2b | `send_break` | 357 | SwitchInt with correct branch structure for the bool arg. |
| 3 | `enable_clock_if_present` | 179 | `IntrinsicCall` for `ClockEnable` has `error_target: Some(_)`; zero `Try::branch`/`FromResidual`/`ControlFlow` remnants in IR. |
| 4 | `flush_rx_fifo` | 346 | Back-edge Goto from a SwitchInt branch back to the loop header; volatile dead-temp `let _ = self.regs.read(dr::REG)` retains `RegisterRead` IntrinsicCall. |
| 5 | `program_polling_port` | 156 | `Body.ordered_regions.len() == 1`; entry/exit recorded; no marker calls remain. |
| 6 | the `info!` call task 50 added to PL011 | (helper) | The `IntrinsicCall` carries `IntrinsicMetadata::Log { template, placeholders }` with expected template, one `Display` placeholder, stripped `LogArg`. |
| 7 | all register access | various | Every register `IntrinsicCall` carries `IntrinsicMetadata::Register { register_block, register_offset, access_width, field }` matching the `register_block!` definitions. |
| 8 | `disable_fifo` (75) + `configure_line_control` (120) | -- | Extracted `Mmio::<pl011::Regs>::modify` body contains exactly 2 `IntrinsicCall` terminators (`mmio_read` then `mmio_write`), same register. |
| 9 | volatile dead-temp | 346 | `flush_rx_fifo`'s read-and-discard produces `IntrinsicCall` with `intrinsic: RegisterRead`, retained in blocks. |

### Cross-cutting assertions

- Driver module's `abstractions_used` and `intrinsics_used`
  match PL011's actual usage (task 53 sets).
- Every `Body` has a structurally parallel `BodySourceMap`
  (one `SourceOrigin` per local, per statement, per terminator;
  populated `body_span`).
- Every cross-crate function (`Option::map`, `Option::unwrap_or`,
  `<FieldValue as Add>::add`, `Mmio::modify`) exists as a
  `Function` with a non-empty `Body`.
- No `is_cleanup` block survives; no `resume`/`UnwindResume`
  terminator remains.
- Every `PinCtrlApply` carries `IntrinsicMetadata::PinCtrl
  { state_id }` matching the marker type's
  `#[rseq::pin_state(id = "...")]`.
- **Phase-1 invariants** (mirroring spec.md Section 13
  post-Phase-1 verification; the verifier itself lands in
  Phase 6 but the model must already satisfy these):
  - Every declaration-side field populated (no placeholder
    `Body` remains after Pass 2 + cross-crate extraction;
    every TypeDef has a classification; every Resource has
    its taxonomic properties).
  - Every binding slot is `None`
    (clock/reset/pinctrl/interrupt/mmio resources have
    `binding == None`; properties have `bound_value == None`).
  - Module-tree invariants: exactly one Driver module, no
    duplicate item names per module.
  - Model-level `source_locations` carries an entry for every
    `DeclId` assigned during Pass 1 (no missing entries; no
    `CompilerGenerated` for entities sourced from the user
    spec).

### Golden serialization

For each of the 9 test functions, serialize the constructed
`Body` (with its owned `source_map`) to JSON and store under
`tests/fixtures/pl011-full-spec/expected/ir/<function_name>.json`.

This task **also owns the model-level serialization wiring**:
- Verify that every IR type used by `Body` derives or
  implements `Serialize` per the contract in ir-reference.md
  Section 7 (adjacently-tagged enums, omit-when-None options,
  `Symbol`/`FileId` rendered as resolved strings via the
  model's `SymbolInterner`, deterministic collection ordering).
- If any type is missing serde wiring, add it as part of this
  task's scope (it is a strict subset of what Phase 6's full
  `RegseqModel` serialization needs; landing it here unblocks
  Phase 6).
- The serialization helper takes a `(Body, &SymbolInterner)`
  pair so symbol resolution happens at write time without
  embedding the interner in `Body`.

The integration test asserts byte-equality with the committed
goldens after re-serializing. Drift fails the test loudly.

### Determinism

Run the full construction pipeline twice in the test (compile
the fixture crate once, run construction twice against the
same rustc state) and assert byte-identical serialization
across runs.

## Non-Goals

- No new construction logic, no new recognition rules.
- No platform-file binding (Phase 6).
- No backend changes (Phase 8).
- No tests for non-PL011 fixtures (Phase 5 charters PL011
  only).
- No `RawFrontendArtifact` golden maintenance (that file is
  legacy being deprecated).

## Test Plan

- `pl011_construction.rs` integration test:
  - Compiles the PL011 fixture crate through the
    regseq-driver rustc harness.
  - Runs `build_pass1_output()` (Pass 1, returns
    `Pass1Output { model, cross_refs }`) → Pass 2
    body construction → dependency-set derivation (task 53).
  - Asserts every cross-cutting and per-function invariant
    listed above.
  - Serializes each test function's `Body` to its golden path
    and asserts byte-equality with the committed golden.
  - Runs construction twice; asserts byte-identical output.
- `cargo test -p regseq-driver` passes.

## Review

After tests pass, use `codex:rescue` to verify the integration
test covers every Phase 5 brief acceptance criterion
(per-function structural assertions, modify body shape,
volatile dead-temp, cross-crate functions exist with bodies,
ordered region recorded, PinCtrl metadata, register metadata
for all access functions, no Try remnants, Phase-1 invariants
satisfied), the goldens are byte-stable across two runs, and
no Phase 6 work has bled into this task. Iterate until
approved.

## Quality Gate

```
cargo test -p regseq-driver
cargo test -p regseq-model
cargo fmt --all -- --check
cargo clippy -p regseq-driver -- -D warnings
cargo clippy -p regseq-model -- -D warnings
```

## Acceptance Criteria

- `pl011_construction.rs` integration test passes with all
  per-tier structural assertions + cross-cutting + Phase-1
  invariants.
- 9 JSON goldens committed under
  `tests/fixtures/pl011-full-spec/expected/ir/`.
- Determinism check (two runs produce byte-identical output)
  passes.
- No reference to `RawFrontendArtifact` or the legacy
  `regseq_model::declaration::mir_body::*` types in the test
  or in any construction code.
- Codex review approved.
- After this task: `regseqmodel-phase-5` is operationally
  complete and Phase 6 work can begin.
