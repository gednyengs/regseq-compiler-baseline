# regseq-backends-24: Generic-C opaque-handle emission (`opaque_handle_eligible`, IT-14)

> **REDESIGN (2026-05-30).** A first implementation (auto-classify *every* struct with
> no by-value use → opaque handle; commit `bf37c1d`) was **reverted** (`bdef425`): the
> automatic, codebase-wide criterion was too broad — it reclassified pointer-only structs
> across all real specs/tests and broke 2 cross-crate snapshot/smoke targets
> (`regseq-model::e2e_trait_method_resolution`, `bridge-coverage-runner::smoke`). The
> opaque-handle pattern must be **OPT-IN** (a deliberate marker on specific handle types),
> NOT an automatic property of every pointer-only struct. This task is reset to design that
> opt-in surface first. (Prior auto-classifier code for reference: `bf37c1d`
> `bridge/opaque_handle.rs`.)

## Description

`Item.opaque_handle_eligible` drives the opaque-handle pattern: an aggregate used
only behind a pointer is emitted as a **forward declaration only** (`struct X;` +
`struct X *` at use sites), with **no** full definition. Today it is unreachable
end-to-end on two counts (Codex spec review):

1. **Bridge:** `opaque_handle_eligible` is hardcoded `false` at every ADT-item
   construction site, so the flag is never set.
2. **Backend:** there is no generic-C consumption of the flag — bank 2 forward-declares
   every aggregate and bank 3 still defines them all, so even a `true` flag would not
   change emission.

Blocks **IT-14**. Split out of `regseq-driver-83` (which handles the
straightforward bridge field-propagation rows) because IT-14 needs an eligibility
classifier + a distinct backend emission path, not just field propagation.

## Input Context

### Code anchors (confirm at HEAD — per Codex review)

- `crates/regseq-driver/src/bridge/ty.rs:673` — `opaque_handle_eligible: false`
  (hardcoded). Needs the eligibility classifier (the criteria — e.g. an ADT reachable
  only behind a `Ref`/`Ptr`, never by value/field — are the core design question).
- `crates/regseq-backends/src/generic_c/mod.rs:~666-724` — bank 2 (forward decls) +
  bank 3 (definitions). An eligible aggregate must emit the forward decl WITHOUT the
  bank-3 definition (and its layout `_Static_assert`s, which require a complete type,
  must be suppressed for the opaque case).
- Matrix IT-14 (`docs/.../01-construct-surface-and-matrix.md:119`), gc §2
  opaque-handle paragraph.

## Implementation Notes

Two coupled pieces: (1) a bridge eligibility classifier that sets
`opaque_handle_eligible` when an ADT qualifies for the opaque-handle pattern; (2) a
backend path that, for an eligible aggregate, emits the forward declaration only and
suppresses the definition + its layout asserts. Pin the eligibility criteria first
(design note / Codex round) — that is the load-bearing decision; the emission is
mechanical once the criteria are fixed.

## Non-Goals

- Not the straightforward field propagations (pack/link_section/inline_hint) —
  `regseq-driver-83`.
- If the eligibility criteria prove out-of-scope for Phase-1, an honest disposition
  (reclassify IT-14 out-of-scope, like DL-5) is an acceptable alternative to landing
  the feature — record the decision either way.

## Test Plan

1. Build + workspace green.
2. A `kind=rust` codegen-coverage fixture with an opaque-handle-eligible struct (used
   only behind a pointer): golden.c shows the forward decl + `struct X *` use, NO
   definition; compiles-and-runs → IT-14 green; re-run the runner, byte-stable.
3. Bridge unit test (classifier sets the flag for an eligible ADT, not for a by-value
   one) + backend unit test (eligible aggregate emits forward-decl-only).

## Quality Gate

`cargo fmt --all -- --check`; scoped clippy (`regseq-driver`, `regseq-backends`,
`codegen-coverage-runner`); single-threaded scoped tests; ledger byte-stable.

## Acceptance Criteria

1. An opaque-handle-eligible ADT emits forward-decl-only (no definition, no layout
   asserts); IT-14 green in the ledger — OR IT-14 reclassified out-of-scope with a
   recorded rationale if the pattern is deferred past Phase-1.
2. The eligibility classifier does not mis-flag by-value-used ADTs.
3. fmt + clippy (scoped) clean; scoped tests green; ledger byte-stable.
