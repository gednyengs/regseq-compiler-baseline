# Task: regseq-driver-15 - Timed-delay capture and legality baseline

## Description

Implement the narrow compiler-facing extraction scaffolding for the accepted
timed-delay intrinsic calls and the immediate legality rule around
`sleep(Duration)`. This task should prove `regseq-driver` can recover timed
delay frontend forms without introducing lowering behavior.

## Input Context

Read before starting:

- crates/regseq-driver/src/extract/callback_context.rs
- crates/regseq-driver/src/extract/interrupts.rs
- crates/regseq-driver/src/extract/traits.rs
- crates/regseq-driver/src/extract/mod.rs
- docs/compiler-collateral/compiler-baseline-overview.md
- docs/compiler-collateral/constructs/timed-cpu-delay-and-sleep.md
- docs/discussions/regseq-intrinsics/regseq-intrinsics-discussion.md

## Output Specification

Create:

- crates/regseq-driver/src/extract/timed_delay.rs
- crates/regseq-driver/tests/timed_delay.rs

Modify:

- crates/regseq-driver/src/extract/mod.rs

## Implementation Notes

Recognize and extract the accepted timed-delay frontend forms:

- canonical intrinsic paths:
  - `regseq::intrinsics::timed_delay::busy_wait(Duration)`
  - `regseq::intrinsics::timed_delay::sleep(Duration)`
- accepted compatibility paths:
  - `regseq::delay::busy_wait(Duration)`
  - `regseq::delay::sleep(Duration)`
- `Duration::from_us`, `Duration::from_ms`, and `Duration::from_s`
  constructor calls

The extraction output should remain internal to `regseq-driver`. A narrow
record such as `TimedDelaySurface`, `TimedDelayCallSite`, and
`DurationCtorSite` is appropriate.

Required capture details:

- intrinsic identity (`busy_wait` vs `sleep`)
- whether the call came through the canonical or compatibility path
- containing function/method name
- constructor kind (`from_us`, `from_ms`, `from_s`) for recognized
  `Duration` expressions

Required structural validations for this task:

- for this batch, framework-managed (`F`) and subsystem-serialized (`S`)
  callback contexts count as sleepable
- `sleep` is rejected in explicit handler-managed interrupt context (`H`)
- `busy_wait` is accepted in all callback contexts for this batch
- only recognized `Duration` constructor forms count as the stable authored
  delay argument shape

Keep this task bounded:

- do not implement “warn on long busy-wait in sleepable context”
- do not lower delay/sleep behavior
- do not widen legality beyond the accepted callback-context baseline
- do not treat poll-loop `cpu_relax` hints as authored timed-delay calls

## Test Plan

Create:

- crates/regseq-driver/tests/timed_delay.rs

Tests to implement:

1. Verify canonical and compatibility timed-delay call paths are recognized.
2. Verify `Duration::from_us/from_ms/from_s` constructor sites are recognized.
3. Verify `sleep` is accepted in sleepable trait-callback contexts.
4. Verify `sleep` is rejected in ISR context.
5. Verify `busy_wait` is accepted in both sleepable and ISR contexts.

Test command: `cargo test -p regseq-driver --test timed_delay`

## Acceptance Criteria

- `regseq-driver` can reach the accepted timed-delay call forms
- the immediate sleep-context legality rule is enforced
- the task stays limited to frontend capture and bounded legality checks
- the new tests pass
- `cargo test --workspace` passes
- `cargo clippy --workspace -- -D warnings` passes
