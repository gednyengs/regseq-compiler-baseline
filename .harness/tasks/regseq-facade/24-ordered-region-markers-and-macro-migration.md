# regseq-facade-24: Ordered region marker functions and ordered! macro migration

## Description

Add internal marker functions for ordered region boundaries and update
the `ordered!` macro to emit these markers instead of `memory_barrier()`
intrinsic calls.

This is a prerequisite for Phase 5 OrderedRegion detection, which
expects paired `ordered_region_begin()`/`ordered_region_end()` calls.

## Input Context

- `docs/discussions/models/regseq-model/execution-plan.md` (Phase 3.5)
- `docs/discussions/models/regseq-model/spec.md` (Section 9, Ordered Flat Regions)
- `docs/discussions/models/regseq-model/ir-reference.md` (Section 6, OrderedRegion)
- `crates/regseq-macros/src/ordered.rs`
- `crates/regseq/src/intrinsics/sync.rs`

## Implementation Notes

### Internal marker functions

Create `crates/regseq/src/internal.rs` (new module):

```rust
/// Compiler marker: beginning of an ordered region.
/// Not a real intrinsic -- exists only for IR constructor detection.
#[inline(never)]
pub fn ordered_region_begin() {}

/// Compiler marker: end of an ordered region.
#[inline(never)]
pub fn ordered_region_end() {}
```

Add `pub mod internal;` to `crates/regseq/src/lib.rs`.

These are NOT `#[rseq::intrinsic]` -- they are plain functions the
compiler recognizes by DefId during IR construction.

`#[inline(never)]` ensures they appear as `Call` terminators in MIR
at `mir_opt_level = 0`.

### ordered! macro update

Update `crates/regseq-macros/src/ordered.rs` to change the expansion
from:

```rust
regseq::intrinsics::sync::memory_barrier(regseq::intrinsics::sync::BarrierKind::Full);
{ /* user block */ }
regseq::intrinsics::sync::memory_barrier(regseq::intrinsics::sync::BarrierKind::Full);
```

to:

```rust
regseq::internal::ordered_region_begin();
/* user statements -- flat, no braces wrapping */
regseq::internal::ordered_region_end();
```

The expansion must NOT wrap the user block in braces. The output should
be flat: `begin(); stmt1; stmt2; end();` -- not
`begin(); { stmt1; stmt2; } end();`. This matches spec.md Section 9
which says ordered! "without introducing a new scope (no lifetime or
drop-order changes)".

Preserve the existing behavior of returning the block's final expression
value.

### Conventions

- Do NOT remove `memory_barrier()` or `BarrierKind` from
  `intrinsics/sync.rs`. They may be used for other purposes.
- The `internal` module is reserved for compiler markers and hints.

## Non-Goals

- No OrderedRegion detection logic (Phase 5).
- No changes to PL011 fixture (ordered! macro change is transparent).

## Test Plan

- `cargo build -p regseq` compiles with new internal module.
- `ordered! { expr }` expands to begin/end markers bracketing the block.
- `ordered! { expr }` still returns the block's value.
- `ordered!` does not introduce a new scope -- variables declared inside
  `ordered!` are accessible after the macro invocation.
- `cargo build -p pl011-full-spec` compiles (ordered! usage is
  transparent -- only the expansion changes).
- `cargo test --workspace` passes.

## Review

After tests pass, use `codex:rescue` to review. Verify macro expansion
uses new markers. Iterate until approved.

## Quality Gate

```
cargo test --workspace
cargo fmt --all -- --check
cargo clippy --workspace -- -D warnings
```

## Acceptance Criteria

- `internal.rs` created with marker functions.
- `ordered!` macro updated to use new markers.
- Workspace compiles and tests pass. Codex review approved.
