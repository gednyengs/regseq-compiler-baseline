# Task: regseq-facade-06 - Intrinsic namespace normalization and ordering anchors

## Description

Normalize the function-like intrinsic surface around the canonical
`regseq::intrinsics::{construct}::{intrinsic}` namespace, and add the shared
ordering intrinsic anchors needed before `regseq::ordered!` can be specified as
macro sugar. This task owns only the facade-side declaration surface and
compatibility re-exports.

## Input Context

Read before starting:

- crates/regseq/src/lib.rs
- crates/regseq/src/poll.rs
- docs/compiler-collateral/compiler-baseline-overview.md
- docs/compiler-collateral/compiler-constraints.md
- docs/compiler-collateral/constructs/register-access.md
- docs/compiler-collateral/constructs/synchronization.md
- docs/discussions/regseq-intrinsics/regseq-intrinsics-discussion.md

## Output Specification

Create:

- crates/regseq/src/intrinsics/mod.rs
- crates/regseq/src/intrinsics/register_access.rs
- crates/regseq/src/intrinsics/sync.rs
- crates/regseq/tests/intrinsics.rs

Modify:

- crates/regseq/src/lib.rs
- crates/regseq/src/poll.rs

## Implementation Notes

This task should establish the canonical intrinsic-home layout for the already
accepted function-like intrinsic surfaces and add the ordering anchors that the
later macro task will target.

Phase-B baseline:

- canonical function-like intrinsics live under
  `regseq::intrinsics::{construct}::{intrinsic}`
- canonical intrinsic declarations must carry both:
  - `#[inline(never)]`
  - `#[rseq::intrinsic]`
- for Phase B, add `#![feature(register_tool)]` and `#![register_tool(rseq)]`
  to the `regseq` crate root so these facade declarations compile cleanly in
  workspace tests without requiring the `regseq-driver` compilation pipeline
- the bodies remain intrinsic stubs only; do not add backend behavior

For this task, establish at least these canonical surfaces:

```rust
pub mod intrinsics {
    pub mod register_access {
        #[inline(never)]
        #[rseq::intrinsic]
        pub fn poll_until<C>(
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

    pub mod sync {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum BarrierKind {
            ReadRead,
            ReadWrite,
            WriteRead,
            WriteWrite,
            Full,
        }

        #[inline(never)]
        #[rseq::intrinsic]
        pub fn memory_barrier(kind: BarrierKind) {
            unimplemented!()
        }
    }
}
```

Compatibility rule for previously accepted intrinsic paths:

- keep `regseq::poll::until(...)` as a compatibility alias or re-export to the
  canonical `regseq::intrinsics::register_access::poll_until(...)` declaration
- implement that compatibility path as a `pub use` re-export, not as a second
  wrapper declaration
- the compiler should resolve call targets through re-exports to the canonical
  `DefId` for intrinsic identity checking
- the re-export path is not a separate intrinsic declaration and does not need
  its own `#[rseq::intrinsic]` marker
- do not introduce new legacy homes beyond that compatibility path

Do not include:

- new user-extensible intrinsic registration mechanisms
- macro surfaces such as `regseq::ordered!`
- backend lowering or barrier-selection policy

## Test Plan

Create:

- crates/regseq/tests/intrinsics.rs

Tests to implement:

1. Verify the canonical intrinsic paths are publicly available:
   - `regseq::intrinsics::register_access::poll_until`
   - `regseq::intrinsics::sync::memory_barrier`
2. Verify the compatibility path `regseq::poll::until` still compiles and has
   the same signature as the canonical polling intrinsic.
3. Verify `BarrierKind` is publicly available and usable at the canonical sync
   intrinsic path.

Test command: `cargo test -p regseq --test intrinsics`

## Acceptance Criteria

- the canonical intrinsic-home namespace exists under `regseq::intrinsics`
- the accepted polling intrinsic has a canonical home and a compatibility path
- the shared ordering intrinsic anchors exist for later macro sugar
- the task stays limited to facade declarations and compatibility plumbing
- the new tests pass
- `cargo test --workspace` passes
- `cargo clippy --workspace -- -D warnings` passes
