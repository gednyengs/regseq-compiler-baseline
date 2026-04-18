# regseq-model-49: Rule 5 — OrderedRegion Detection

## Description

Recognize paired `regseq::internal::ordered_region_begin()` and
`regseq::internal::ordered_region_end()` marker Calls in rustc
MIR, drop both calls, and record an `OrderedRegion { entry,
exit }` on the owning `Body.ordered_regions`. Region membership
is reachability from `entry` without passing through `exit`
(SESE invariant per ir-reference.md Section 6).

## Input Context

- `docs/discussions/models/regseq-model/ir-reference.md`
  (Section 6 OrderedRegion definition + SESE invariant + barrier
  obligations)
- `docs/discussions/models/regseq-model/spec.md` (Section 9
  Ordered Flat Regions; Section 10 confirms the markers are NOT
  intrinsics -- they are compiler markers in
  `regseq::internal::*`)
- `docs/discussions/models/regseq-model/execution-plan.md`
  (Phase 5.4 Rule 5)
- `crates/regseq-model/src/ir/body.rs` (`OrderedRegion`,
  `Body.ordered_regions`)
- `crates/regseq-driver/src/construction.rs` (Pass 2 from tasks
  43-48)
- `crates/regseq/src/internal.rs` (the facade crate's marker
  functions, introduced by `regseq-facade-24`)
- `tests/fixtures/pl011-full-spec/src/driver.rs` (line 156
  `program_polling_port()`)

## Implementation Notes

### Marker DefId resolution

At construction setup, resolve the two marker `DefId`s once by
walking the facade crate's items (or a small
`tcx.all_diagnostic_items()` / path-based lookup against
`regseq::internal::ordered_region_begin` and
`ordered_region_end`). Cache both `DefId`s on the construction
context.

If the facade crate is compiled in but the markers cannot be
resolved, raise
`ConstructionError::OrderedMarkerLookupFailed` at setup --
defensive guard against facade renames.

### Recognition

Walk Pass 2 body translation. When a
`mir::TerminatorKind::Call`'s callee `DefId` equals the cached
begin marker, record the **next** basic block (the call's target)
as the region `entry`. When a Call's callee `DefId` equals the
cached end marker, record its target as the region `exit`. Drop
both Call terminators from the emitted CFG -- the block that
contained the begin marker becomes empty (or keeps preceding
statements) and falls through to `entry` via Goto; the block
that contained the end marker similarly falls through to `exit`.

### OrderedRegion record

```rust
Body.ordered_regions.push(OrderedRegion { entry, exit });
```

The struct lives on `Body`, not on any single basic block, per
ir-reference.md Section 6 (SESE region is reachability, not
block-ID adjacency).

### Pairing rule

Markers must be paired. If a `begin` marker has no matching
`end` in the same body, or an `end` marker has no preceding
`begin`, emit `ConstructionError::UnpairedOrderedMarker`.
Nesting is permitted (each nested pair becomes its own
`OrderedRegion` with strict containment per ir-reference.md
Section 6).

### Block-ID renumbering interaction

Marker Call dropping must respect the cleanup-block
renumbering established in tasks 43/44. Verify the recorded
`entry`/`exit` block IDs reference live (post-renumber) blocks.

## Non-Goals

- No barrier emission (Phase 8 backend obligation per
  ir-reference.md Section 6).
- No backend lowering of region membership.
- No closure-based region.

## Test Plan

Integration tests under `crates/regseq-driver/tests/`:

- Translate `program_polling_port()` from PL011 and assert:
  - Body contains zero Call terminators whose DefId matches
    either marker.
  - `Body.ordered_regions.len() == 1`.
  - The recorded `entry` block is the first block after the
    dropped begin call's site.
  - The recorded `exit` block is the block following the
    dropped end call's site.
  - All blocks between `entry` and `exit` are reachable from
    `entry` without passing through `exit` (SESE invariant).
- Fixture with two nested ordered regions; assert
  `Body.ordered_regions.len() == 2` and the inner region is
  **strictly contained** within the outer (every block
  reachable from inner.entry without passing inner.exit is
  also reachable from outer.entry without passing outer.exit).
- Fixture with an unpaired `begin` marker →
  `ConstructionError::UnpairedOrderedMarker`.
- Fixture with an unpaired `end` marker → same error.
- Synthetic setup where the facade markers cannot be resolved
  → `ConstructionError::OrderedMarkerLookupFailed`.
- `cargo test -p regseq-driver` passes.

## Review

After tests pass, use `codex:rescue` to verify the marker
`DefId`s are discovered through rustc queries (not hardcoded
path strings), the SESE invariant holds for recognized
regions, nesting produces strict containment, and pairing edge
cases are handled. Iterate until approved.

## Quality Gate

```
cargo test -p regseq-driver
cargo fmt --all -- --check
cargo clippy -p regseq-driver -- -D warnings
```

## Acceptance Criteria

- `program_polling_port()` produces a body with one
  `OrderedRegion { entry, exit }` and zero marker Calls.
- Nested regions produce multiple records with strict
  containment (inner strictly contained within outer per
  ir-reference.md Section 6).
- Unpaired markers hard-error with provenance.
- Marker DefIds resolved via live rustc query, not string.
- Codex review approved.
