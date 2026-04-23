# PL011 Fixture Audit History

Codex adversarial-review transcripts for the PL011 fixture at
`tests/fixtures/pl011-full-spec/`. These are the raw Codex outputs,
preserved here so that future sessions can audit the REQ-by-REQ
provenance of the Stage 1 closure without depending on ephemeral
`/tmp` files.

## Stage 1 audit loop (2026-04-21, converged)

Three-round adversarial audit against the Stage 1 requirements in
`../pl011-spec-requirements.md` (§1.2 stage membership) and the
datasheet excerpt in `../pl011-minimized-datasheet.md`.

- `stage-1-round-1-gap-report.md` -- initial audit. Verdict:
  `stage-1-gap-large`. Found 10 items (3 user-decision, 7
  mechanical; plus a framework-investigation question about
  `register_block!` reserved-bit policy).
- `stage-1-round-2-verification.md` -- post-Round-1-fixes audit.
  Verdict: `stage-1-gap-small`. Found 2 residuals: a REQ-LIFE-02
  regression in the new reprogram helper (validation after
  disable could leave the port disabled on rejected configs), and
  a stale "Scheduled fixture work" section in
  `../pl011-full-spec-status.md`.
- `stage-1-round-3-convergence.md` -- post-Round-2-fixes audit.
  Verdict: `stage-1-complete`. Convergence signal: `converged`.

After Round 3, all 39 Stage-1 claim-path REQs are captured
(37 fixture-level, 2 deferred to backend emission per
REQ-INT-LIN-01 / REQ-INT-ZEP-01). No fixture work remains for
Stage 1.

## How to add new transcripts

When a future audit loop runs (Stage 2, Stage 3, or re-audit):

1. Save each Codex round output to a new file in this directory
   with a descriptive name (`stage-2-round-1-*.md`, etc.).
2. Update this README's "Audit history" section with the new
   round summary.
3. Reference the new files from `../pl011-full-spec-status.md`
   so the status doc's provenance stays auditable.

Do NOT reference ephemeral `/tmp/` paths from committed docs --
future sessions will not have those files.
