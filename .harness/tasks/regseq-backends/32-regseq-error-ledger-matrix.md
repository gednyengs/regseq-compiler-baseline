# regseq-backends-32: RegseqError coverage-ledger / matrix rows + docs note (Phase-2a)

> Source: `.claude/specs/regseq-error/PLAN-2026-06-02-000057.md` (task **T16**),
> open item **O2**, invariants **I03/I09**.

## Description

Wire the new RegseqError coverage into the codegen-coverage gate (the **O2**
taxonomy decision) and record the (already-applied) docs conformance notes. Keep
the gate `pass` and the ledger byte-stable.

## Input Context

### Code anchors (confirm at HEAD)

- `crates/codegen-coverage-runner/` — the `ledger`/`matrix` modules and the gate
  aggregation; the fixture `covers` wiring.
- The Doc-1 matrix markdown the matrix parser reads
  (`docs/.../phase-1/01-construct-surface-and-matrix.md` or its current home).
- The new fixtures (regseq-backends-30) and negatives (regseq-backends-31) that the
  rows attribute.
- Already-committed design docs: `roadmap.md` (Phase-3+ G01) and
  `regseq-error-spec.md` (§6 G01 addendum) — referenced, not re-edited.

## Implementation Notes

- **O2 recommendation:** one **combined** matrix row ("RegseqError boundary
  mapping + exit-code propagation") plus per-variant attribution via the
  regseq-backends-26 unit test — avoids ~10 near-duplicate fixtures (matches D07).
- Add a short conformance note (where the gate docs live) that spec §8 (facade) is
  relaxed per D06 and §6 carries the G01 addendum — **no behavior change**.

## Non-Goals

- No new fixtures/tests beyond row wiring (those are regseq-backends-30/31).
- No code behavior change.

## Test Plan

1. `ledger`/`matrix` tests pass with the new row(s) attributed by the new fixtures.
2. Run the gate twice: `pass` both times, **zero** ledger drift (byte-stable).

## Quality Gate

`cargo fmt --all -- --check`; `cargo run -p codegen-coverage-runner` → `pass`;
ledger byte-stable (runner twice + diff).

## Acceptance Criteria

1. New coverage-matrix row(s) added and attributed by the RegseqError fixtures; gate
   stays `pass` (**I09**, O2).
2. Ledger byte-stable across two runs; emitted artifacts deterministic (**I03**).
3. Docs conformance note recorded (D06 / G01); fmt clean.
