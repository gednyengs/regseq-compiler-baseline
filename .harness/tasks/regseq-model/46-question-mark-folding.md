# regseq-model-46: Rule 2 — `?`-Pattern Folding

## Description

Implement recognition rule 2: fold Rust's `?` desugaring into the
`error_target` field of `Call` and `IntrinsicCall` terminators.
Recognizes the structural MIR pattern produced by any `Try`
implementor (`Result`, `Option`, future types) by matching
`<T as Try>::branch` and `<T as FromResidual>::from_residual` via
live rustc `DefId`s, validates the SwitchInt shape, and
eliminates the intermediate `ControlFlow` plumbing.

This task carries the named **`?`-pattern correctness** risk
from the execution plan. Concentrated in one task with explicit
fallback and hard-error policies.

## Input Context

- `docs/discussions/models/regseq-model/ir-reference.md`
  (Section 5 TerminatorKind, `error_target` semantics;
  `?`-pattern recognition algorithm -- the authoritative
  procedure with the full MIR pattern shape, recognition steps,
  fallback rule, and concrete Try implementors)
- `docs/discussions/models/regseq-model/spec.md` (Section 10
  intrinsic recognition; Section 7 SourceOrigin rule for
  drop-elaboration nodes on error paths)
- `docs/discussions/models/regseq-model/execution-plan.md`
  (Phase 5.4 Rule 2; Gaps+Risks row "?-pattern folding
  correctness")
- `crates/regseq-model/src/ir/terminator.rs` (Call/IntrinsicCall
  with `error_target: Option<BasicBlockId>`)
- `crates/regseq-driver/src/construction.rs` (Pass 2 from tasks
  43-45)
- `tests/fixtures/pl011-full-spec/src/driver.rs` (line 179
  `enable_clock_if_present()`)

## Implementation Notes

Implement the algorithm exactly as documented in
ir-reference.md Section 5 "?-pattern recognition algorithm".
Below summarizes; the reference is authoritative.

### Recognition trigger

Identify `Try::branch` and `FromResidual::from_residual` by
their live rustc trait-method `DefId`s. At construction setup,
resolve these `DefId`s once via rustc's lang-item / trait-item
query chain:

```rust
let try_trait_def_id = tcx.lang_items().try_trait().ok_or(...)?;
let branch_fn_def_id = /* find Try::branch trait item on try_trait_def_id */;
let from_residual_fn_def_id = /* find FromResidual::from_residual */;
```

Cache both `DefId`s on the construction context. Matching per
call is a direct `DefId` equality check -- no string
comparison, no prefix matching.

When processing a `Call` (or `IntrinsicCall` from task 45)
whose `target` block begins with a `Call` whose callee `DefId`
**equals** the cached `Try::branch` `DefId` (through the trait
impl's instance resolution; rustc's monomorphization supplies a
specific instance, and the underlying trait-method identity
survives), begin pattern recognition. The algorithm is
`T`-agnostic.

### Validation

The `Try::branch` call's return target must contain
`mir::Rvalue::Discriminant(_cf)` followed by a `SwitchInt` with
exactly:
- `values == [0, 1]` (Continue, Break).
- `targets == [bb_continue, bb_break, bb_unreachable]`.

The `bb_unreachable` (the `otherwise` slot) must contain
`mir::TerminatorKind::Unreachable`.

### Folding

- The folded terminator's `destination` is retargeted to the
  Continue extraction's destination (the inner success value:
  `V` in `Result<V, E>` or `Option<V>`).
- `target` becomes the block immediately AFTER the Continue
  extraction statement.
- `error_target` becomes the block AFTER the
  `FromResidual::from_residual` Call returns. This block
  contains the Drop cascade for live locals and eventually
  reaches `return`.

### Eliminations

Drop the following from the Regseq IR:
- The original Try-wrapper temporary (the Call's MIR
  destination before folding).
- The `_cf: ControlFlow<...>` temporary.
- The `Try::branch` Call.
- The `Discriminant` rvalue assignment.
- The `SwitchInt` terminator.
- The Continue extraction statement
  (`_val = move ((_cf as Continue).0)`) -- absorbed into the
  retargeted `destination`.
- The `FromResidual::from_residual` Call.
- The residual temporaries (`_residual`, `_converted`).

### Fallback rule

If the trigger does NOT match (no `Try::branch` DefId at the
return target), keep the original `Call` / `IntrinsicCall`
with `error_target: None`. No error.

### Hard-error rule

If `Try::branch` DefId is present but any subsequent step fails
(SwitchInt shape wrong, missing `FromResidual`, partially
recognized but structurally invalid), emit
`ConstructionError::MalformedTryPattern` carrying the affected
Call's `BasicBlockId` and the offending sub-step. Stop
processing this body.

### Source-origin rule

Drop terminators on the Break path (inserted by Rust's drop
elaboration) use `Source(<originating user span>)` per
ir-reference.md Section 7 -- never `CompilerGenerated`. The
user span is the span of the originating `?` operator. The
`BodySourceMap` for the error-path blocks must reflect this.

### Nested `?`

Each `?` is a self-contained instance of the pattern on a
different Call terminator. Multiple `?` in one function produce
multiple folds; they may share Drop-cascade blocks but each
recognition is independent.

## Non-Goals

- No closure support (`poll_until` predicates remain as
  `IntrinsicCall` with closure args -- future closure work).
- No structured error analysis (matching on specific
  RegseqError variants is plain SwitchInt + Discriminant; not
  this task).
- No backend lowering of `error_target` (Phase 8).

## Test Plan

Integration tests under `crates/regseq-driver/tests/`:

- Translate `enable_clock_if_present()` from PL011 and assert:
  - The `IntrinsicCall` for `clock_enable` has `error_target:
    Some(bb)` pointing to the Drop cascade block.
  - The folded body contains zero references to `Try::branch`,
    zero `FromResidual::from_residual`, and zero `ControlFlow`
    temporaries (walk all statements/operands).
  - `destination` of the folded `IntrinsicCall` is the
    unwrapped inner type (`()`), not `Result<(), RegseqError>`.
  - `source_map` entries for blocks reachable only via
    `error_target` carry `Source(...)` (not
    `CompilerGenerated`).
- Fixture with two sequential `?` to verify nested `?`
  produce two independent folds.
- Fixture with `Option::?` (different Try implementor) to
  verify the algorithm is type-agnostic.
- Negative test 1 (no-match fallback): a Call NOT followed by
  `Try::branch` translates to a regular `Call` with
  `error_target: None`.
- Negative test 2 (malformed): fixture with a synthetic
  SwitchInt shape having 3 values produces
  `ConstructionError::MalformedTryPattern`.
- Negative test 3 (malformed): `Try::branch` followed by a
  return target whose `otherwise` slot is NOT `Unreachable`
  produces `MalformedTryPattern`.
- Negative test 4 (malformed FromResidual): `Try::branch` +
  valid SwitchInt followed by a Break path that does NOT call
  `FromResidual::from_residual` produces `MalformedTryPattern`.
- Negative test 5 (malformed FromResidual signature): a
  `FromResidual` Call whose DefId matches but whose argument
  shape (`_residual`) is wrong produces `MalformedTryPattern`.
- `cargo test -p regseq-driver` passes.

## Review

After tests pass, use `codex:rescue` to verify the algorithm
matches ir-reference.md Section 5 step-for-step, uses live
`DefId` matching, the elimination list is exhaustive (no
remnants survive), the source-origin rule is followed, and
both fallback and hard-error policies are tested in isolation.
Iterate until approved.

## Quality Gate

```
cargo test -p regseq-driver
cargo fmt --all -- --check
cargo clippy -p regseq-driver -- -D warnings
```

## Acceptance Criteria

- `enable_clock_if_present()` folds with unwrapped Continue as
  `destination`, Break path as `error_target`, zero
  Try-machinery remnants.
- Algorithm verified type-agnostic via `Option::?` test.
- Both fallback and hard-error policies tested in isolation.
- Drop-elaboration source-origin rule verified.
- `Try::branch` / `FromResidual::from_residual` matched via
  live rustc `DefId`, not string comparison.
- Codex review approved.
