# regseq-model-44: SwitchInt Translation and Back-Edge Loop Verification

## Description

Add `SwitchInt` terminator translation to Pass 2 body construction
(in `crates/regseq-driver/src/construction.rs`) and verify that
back-edge `Goto` terminators naturally form CFG loops without any
additional handling. Validates against the PL011 incremental tier
2 (if/else) and tier 4 (loop) targets.

## Input Context

- `docs/discussions/models/regseq-model/ir-reference.md`
  (Section 5 TerminatorKind / SwitchTargets)
- `docs/discussions/models/regseq-model/spec.md` (Section 9 body
  IR)
- `crates/regseq-model/src/ir/terminator.rs` (`SwitchTargets`)
- `crates/regseq-driver/src/construction.rs` (Pass 2 from task
  43)
- `tests/fixtures/pl011-full-spec/src/driver.rs` (line 338
  `tx_ready()`, 357 `send_break()`, 346 `flush_rx_fifo()`)

## Implementation Notes

Extend terminator translation in `construction.rs`. Map rustc
`mir::TerminatorKind::SwitchInt { discr, targets }`:

```rust
TerminatorKind::SwitchInt {
    discriminant: map_operand(discr, ...),
    targets: SwitchTargets {
        values: <Vec<u128> from targets.iter().map(|(v,_)| v)>,
        targets: <Vec<BasicBlockId>, default/otherwise as the LAST entry>,
    },
}
```

`SwitchTargets.targets.len() == values.len() + 1` -- the last
entry is the default block (per ir-reference.md Section 5).

### Loop verification

No new code is needed for loops. A `mir::TerminatorKind::Goto`
whose target has a lower `BasicBlockId` than the source forms a
back-edge. After SwitchInt translation lands,
`flush_rx_fifo()` produces a body with a back-edge Goto from a
SwitchInt branch back to the loop header. The task validates
this structurally -- no loop-detection code is introduced.

### Cleanup-block renumbering interaction

SwitchInt targets must respect the cleanup-block renumbering
map from task 43. Verify a SwitchInt whose original target
indexed a dropped cleanup block re-targets correctly after
renumbering (or produces
`ConstructionError::SwitchTargetIntoCleanup` if the target was
itself a cleanup block).

## Non-Goals

- No new loop construct in the IR (loops are natural CFG
  back-edges per ir-reference.md Section 5 design rationale).
- No intrinsic recognition (task 45).
- No `?`-pattern folding (task 46).

## Test Plan

Integration tests under `crates/regseq-driver/tests/`:

- Translate `tx_ready()` and `send_break()` from PL011 and
  assert each has at least one `SwitchInt` terminator with
  `values: [0]`, two targets (false + true branch), discriminant
  is an `Operand::Copy` of the bool place.
- Translate `flush_rx_fifo()` and assert:
  - Body contains a back-edge `Goto` whose target
    `BasicBlockId` is strictly less than the source block ID.
  - The loop header has a SwitchInt that exits the loop on the
    appropriate branch.
- Fixture with multi-target SwitchInt (3 values, default block)
  to verify `SwitchTargets` length invariant
  (`targets.len() == values.len() + 1`).
- `cargo test -p regseq-driver` passes.

## Review

After tests pass, use `codex:rescue` to verify SwitchInt
translation honours the `targets.len() == values.len() + 1`
invariant, the discriminant operand maps through task 42
helpers, and back-edge Gotos are not specially detected.
Iterate until approved.

## Quality Gate

```
cargo test -p regseq-driver
cargo fmt --all -- --check
cargo clippy -p regseq-driver -- -D warnings
```

## Acceptance Criteria

- SwitchInt translation handles `tx_ready()` and `send_break()`
  PL011 bodies plus a multi-target fixture.
- `flush_rx_fifo()` produces a body with a back-edge Goto and
  exiting SwitchInt.
- Renumbering after cleanup-block drop preserves SwitchInt
  target correctness.
- Codex review approved.
