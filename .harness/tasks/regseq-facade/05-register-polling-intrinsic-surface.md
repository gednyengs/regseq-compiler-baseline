# Task: regseq-facade-05 - Register polling intrinsic surface

## Description

Implement the accepted free-function polling intrinsic for Register Access:
`regseq::poll::until(...) -> Result<(), RegseqError>`. This task owns only the
stable facade surface for the polling intrinsic and its export plumbing.

## Input Context

Read before starting:

- crates/regseq/src/lib.rs
- docs/compiler-collateral/compiler-baseline-overview.md
- docs/compiler-collateral/constructs/register-access.md
- docs/compiler-collateral/constructs/timed-cpu-delay-and-sleep.md
- docs/discussions/regseq-error/regseq-error-spec.md

## Output Specification

Create:

- crates/regseq/src/poll.rs
- crates/regseq/tests/poll.rs

Modify:

- crates/regseq/src/lib.rs

## Implementation Notes

Add the stable polling intrinsic surface:

```rust
pub mod poll {
    pub fn until<C>(
        condition: C,
        delay: Duration,
        timeout: Duration,
    ) -> Result<(), RegseqError>
    where
        C: FnMut() -> bool,
    {
        unimplemented!()
    }
}
```

Phase-B baseline for this task:

- use the current shared `RegseqError` baseline, not a local `PollError`
- use `Duration` from the shared facade baseline established in `regseq-facade-02`
- keep the body as an intrinsic stub only
- do not add backend behavior, busy-wait implementation, or ordered-access
  semantics here

## Test Plan

Create:

- crates/regseq/tests/poll.rs

Tests to implement:

1. Verify `regseq::poll::until` is publicly available.
2. Verify the accepted argument and return shape compiles.
3. Verify the intrinsic uses `Duration` and `RegseqError` from the shared
   facade baseline.

Test command: `cargo test -p regseq --test poll`

## Acceptance Criteria

- `regseq::poll::until(...) -> Result<(), RegseqError>` is exported
- the task stays limited to the polling intrinsic surface
- the new tests pass
- `cargo test --workspace` passes
- `cargo clippy --workspace -- -D warnings` passes
