# Task: regseq-facade-15 - Timed-delay intrinsic surface

## Description

Implement the canonical timed-delay intrinsic home and the accepted
compatibility surface under `regseq::delay`. This task owns only the facade
declarations, compatibility re-exports, and public exports.

## Input Context

Read before starting:

- crates/regseq/src/lib.rs
- crates/regseq/src/time.rs
- crates/regseq/src/intrinsics/mod.rs
- docs/compiler-collateral/compiler-baseline-overview.md
- docs/compiler-collateral/constructs/timed-cpu-delay-and-sleep.md
- docs/discussions/regseq-intrinsics/regseq-intrinsics-discussion.md

## Output Specification

Create:

- crates/regseq/src/intrinsics/timed_delay.rs
- crates/regseq/src/delay.rs
- crates/regseq/tests/delay.rs

Modify:

- crates/regseq/src/intrinsics/mod.rs
- crates/regseq/src/lib.rs

## Implementation Notes

Establish the canonical function-like intrinsic home:

```rust
pub mod intrinsics {
    pub mod timed_delay {
        #[inline(never)]
        #[rseq::intrinsic]
        pub fn busy_wait(duration: Duration) { unimplemented!() }

        #[inline(never)]
        #[rseq::intrinsic]
        pub fn sleep(duration: Duration) { unimplemented!() }
    }
}
```

Preserve the accepted compatibility surface as `pub use` re-exports:

```rust
pub mod delay {
    pub use crate::intrinsics::timed_delay::busy_wait;
    pub use crate::intrinsics::timed_delay::sleep;
}
```

Phase-B baseline:

- canonical intrinsic identity lives under
  `regseq::intrinsics::timed_delay::*`
- the accepted authored surface remains `regseq::delay::busy_wait` and
  `regseq::delay::sleep`
- both canonical declarations must carry:
  - `#[inline(never)]`
  - `#[rseq::intrinsic]`
- bodies remain intrinsic stubs only

Do not include:

- legality analysis
- backend delay/sleep behavior
- poll-loop `cpu_relax` hints from Register Access lowering

## Test Plan

Create:

- crates/regseq/tests/delay.rs

Tests to implement:

1. Verify the canonical intrinsic paths are publicly available:
   - `regseq::intrinsics::timed_delay::busy_wait`
   - `regseq::intrinsics::timed_delay::sleep`
2. Verify the accepted compatibility paths remain available:
   - `regseq::delay::busy_wait`
   - `regseq::delay::sleep`
3. Verify both surfaces use the shared `Duration` type.
4. Verify the canonical and compatibility paths have matching signatures.

Test command: `cargo test -p regseq --test delay`

## Acceptance Criteria

- the canonical timed-delay intrinsic home exists under `regseq::intrinsics`
- the accepted compatibility surface exists under `regseq::delay`
- the task stays limited to facade declarations and compatibility plumbing
- the new tests pass
- `cargo test --workspace` passes
- `cargo clippy --workspace -- -D warnings` passes
