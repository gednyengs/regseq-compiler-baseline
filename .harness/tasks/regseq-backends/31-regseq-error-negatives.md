# regseq-backends-31: RegseqError fail-loud negatives (unknown variant + entry shapes) (Phase-2a)

> Source: `.claude/specs/regseq-error/PLAN-2026-06-02-000057.md` (tasks **T13+T14**),
> decisions **D05/D08/D09**, requirements **R08/R09**, invariant **I05**.
> Required negative coverage (D08).

## Description

Prove every fail-loud path the design promises. Two families: (a) a
`regseq-error`-marked enum carrying a variant **absent from C02** fails at emit;
(b) every **rejected entry shape** fails at emit with a shape-naming diagnostic.
The backend is the single enforcement point, so **both** consumers reject
identically.

## Input Context

### Code anchors (confirm at HEAD)

- Entry classifier `Reject` arm (regseq-backends-27) and the C02 `None` → fail-emit
  path (regseq-backends-26/28).
- The backend `EmitError` type and the harness negative pattern (a `Stage::Emit`
  failure surfaced by the runner).
- `mod_tests.rs` hand-built-model pattern for backend unit negatives.

## Implementation Notes

Rejected entry shapes (D08/D09): non-unit `Ok` (`Result<T,_>`, `T != ()`); an `Err`
enum **not** marked `regseq-error`; an entry taking arguments; a **manifest-custom
entry combined with an error-returning signature** (now testable because
regseq-backends-27 made the API entry-aware). Each diagnostic must name the
offending variant/shape; never a degraded code path; never a silent `EIO` for a
known-but-unmapped variant.

## Non-Goals

- No positive fixtures (regseq-backends-30).
- No new mapping logic — only the fail-loud assertions over existing behavior.

## Test Plan

1. Backend unit (hand-built models): unknown marked variant → emit `Err` naming the
   variant; each rejected entry shape → emit `Err` naming the shape.
2. Harness integration negatives: a fixture with an unknown marked variant, and a
   fixture whose `manifest.entry` is custom **and** error-returning, each asserting
   a `Stage::Emit` failure (message substring).

## Quality Gate

`cargo fmt --all -- --check`; scoped clippy (`regseq-backends`,
`codegen-coverage-runner`); `RUST_TEST_THREADS=1 cargo test -p regseq-backends
-p codegen-coverage-runner -- --test-threads=1`; gate `pass`.

## Acceptance Criteria

1. Unknown marked variant → **fail emit** naming the variant; never `EIO`
   (**R08/I05/D05**).
2. Every rejected entry shape (incl. custom-entry+error-return) → **fail emit**
   naming the shape (**R09/D08/D09**).
3. Both consumers reject identically (single backend enforcement point).
4. fmt + scoped clippy clean; tests green; gate `pass`.
