# Task: regseq-driver-09 - Ordered frontend recognition

## Description

Implement the bounded compiler-facing recognition for the deferred
`regseq::ordered!` surface. This task should prove `regseq-driver` can recover
ordered regions as a shared frontend concept without committing lowering
behavior.

## Input Context

Read before starting:

- crates/regseq-driver/src/extract/mod.rs
- crates/regseq-driver/src/extract/register_access.rs
- docs/compiler-collateral/compiler-baseline-overview.md
- docs/compiler-collateral/compiler-constraints.md
- docs/compiler-collateral/constructs/register-access.md
- docs/discussions/regseq-intrinsics/regseq-intrinsics-discussion.md

## Output Specification

Create:

- crates/regseq-driver/src/extract/ordering.rs
- crates/regseq-driver/tests/ordering.rs

Modify:

- crates/regseq-driver/src/extract/mod.rs

## Implementation Notes

Recognize the accepted ordered surface in the bounded way needed by later
analysis and lowering work.

This task may recognize either:

- the authored `regseq::ordered!` surface if it remains visible in the chosen
  extraction layer, or
- the predictable expanded pattern inserted by that macro

The stable ordering anchors for this task are the canonical intrinsic calls
from `regseq::intrinsics::sync::memory_barrier(...)`. Treat those calls as
ordering anchors, not as ordinary helper-function calls.

For this bounded task, recover at least:

- ordered-region identity or span
- explicit barrier-kind anchors present in the region
- enclosed register wrapper or polling operations that later passes will need
  to reason about as ordered

Keep the output internal to `regseq-driver`. A narrow record such as
`OrderedRegion` or `OrderingSurface` is appropriate.

Do not include:

- barrier-strength inference
- backend lowering or target barrier selection
- generalized non-register ordering analysis

## Test Plan

Create:

- crates/regseq-driver/tests/ordering.rs

Tests to implement:

1. Verify a small fixture using `regseq::ordered!` around register operations is
   recognized as an ordered region.
2. Verify the canonical `memory_barrier` intrinsic anchors are recoverable from
   the recognized ordered region.
3. Verify enclosed register wrapper or polling operations remain associated with
   the ordered region.
4. Verify malformed or incomplete ordered patterns are rejected deterministically.

Test command: `cargo test -p regseq-driver --test ordering`

## Acceptance Criteria

- `regseq-driver` can recognize the deferred ordered frontend surface in a
  bounded compiler-facing way
- canonical ordering intrinsic anchors are treated as part of that surface
- the task stays separate from lowering behavior and global ordering analysis
- the new tests pass
- `cargo test --workspace` passes
- `cargo clippy --workspace -- -D warnings` passes
