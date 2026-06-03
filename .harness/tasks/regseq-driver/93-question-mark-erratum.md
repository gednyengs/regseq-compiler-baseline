# regseq-driver-93: RegseqError `?`-support erratum (R02/D07 correction) (Phase-2a)

> Source: the `?`-support investigation corrected a wrong design assumption. This
> task records the erratum in the design artifacts so they stop asserting something
> false. Docs-only (no code). Depends on `regseq-driver-92` so it reflects the
> as-built scope.

## Description

PRD **R02** and GRILL **D02** state `?` "lowers as ordinary MIR", and GRILL **D07**
(the fixture plan) relied on that to use `?` freely in a fixture. That is **wrong**:
`?` is a `Try`-trait desugar that, at
`mir_opt_level=0` (no inliner), emits `core` calls (`Try::branch`,
`FromResidual::from_residual`) and associated-type-projection-typed locals — making it
un-bridgeable as-is (it blocked the original fixture-3). Record the corrected
understanding **and** the resolution: regseq supports **same-error-type `?` on
`core::result::Result`** via a pre-translation MIR normalizer (`regseq-driver-92`);
**cross-type `?` (`From` conversion) and `Option`/other-carrier `?` are deferred**
(fail-loud).

## Input Context

### Code anchors (confirm at HEAD)

- `.claude/specs/regseq-error/PRD-2026-06-01-234426.md` (**R02**, ~line 66) and
  `.claude/specs/regseq-error/GRILL-2026-06-01-233339.md` (**D02** ~line 157 carries
  the "ordinary MIR" claim; **D07** ~line 279 is the fixture plan that relied on it) —
  the artifacts carrying the stale assumption.
- `docs/discussions/regseq-error/regseq-error-spec.md` — carries the existing **G01**
  dated addendum; add the `?`-erratum addendum in the same style.
- `regseq-driver-92` — the as-built same-type `?` lowering (scope + fail-loud
  deferrals) this erratum must describe accurately.
- Memory [[question-mark-unbridgeable]].

## Implementation Notes

- Add a dated `?`-erratum addendum to `regseq-error-spec.md` (mirroring the G01
  addendum precedent): `?` is a core-call desugar, not ordinary MIR; supported scope
  = same-error-type `Result`-`?`; deferred = cross-type / `Option`.
- Append a short erratum pointer to the PRD (R02) and GRILL (D02 + D07) — append-only;
  do not rewrite their bodies. Note the artifacts remain otherwise valid.
- Docs/spec only — no code change.

## Non-Goals

- No code, fixtures, or `tasks.json` semantics change beyond this task's own record.
- Does not re-open the GRILL/PRD decisions; only records the `?` correction.

## Test Plan

- Doc consistency review: the erratum accurately states the blocker (core-call
  desugar / projections) and the as-built scope of `regseq-driver-92` (same-type
  only; cross-type/`Option` deferred); cross-references R02/D07 and `regseq-driver-92`.
- No build/test impact (docs-only).

## Quality Gate

`cargo fmt --all -- --check` (no-op; confirms no stray code change); the addendum
renders and cross-references resolve.

## Acceptance Criteria

1. `regseq-error-spec.md` carries a dated `?`-erratum addendum correcting the
   "ordinary MIR" claim and stating the supported (same-type `Result`) vs deferred
   (cross-type / `Option`) scope, consistent with `regseq-driver-92`.
2. R02 (PRD) and D02 + D07 (GRILL) carry an erratum pointer (append-only).
3. No code change; the erratum is accurate to the as-built behavior.
