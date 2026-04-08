# PL011 Validation

This directory tracks compiler validation against the concrete PL011 full-spec
fixture at:

- [tests/fixtures/pl011-full-spec](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec)

Its purpose is to validate the compiler boundary by boundary, starting from the
authored Regseq source and ending at the generated Linux artifacts.

This is intentionally separate from:

- design discussion docs under `docs/discussions/`
- harness task specs under `.harness/tasks/`

The validation flow here answers a different question:

- where is information preserved?
- where is it transformed?
- where is it lost, degraded, or invented?

When this review finds a real compiler issue:

1. record it here with evidence
2. assign it a stable issue id
3. create or update a harness task for the fix

## Scope

This validation is PL011-specific only in the sense that PL011 is the current
specimen driver.

The goal is not to bless PL011-specific shortcuts. The goal is to use one
concrete driver to validate that the compiler pipeline preserves the right
information across these boundaries:

1. authored Rust Regseq source
2. Rust compiler IR surface (HIR/MIR)
3. Regseq portable model/lowering
4. Linux backend realization
5. generated Linux artifacts

After PL011 is clean, the next validation series should move to:

- `docs/validation/02-e1000/`

## Files

- [00-status.md](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/docs/validation/01-pl011/00-status.md)
  High-level review/status tracker across layers.
- [01-authored-source.md](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/docs/validation/01-pl011/01-authored-source.md)
  Authored-source facts that must survive.
- [02-hir-mir.md](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/docs/validation/01-pl011/02-hir-mir.md)
  Rust compiler IR boundary review.
- [03-regseq-model.md](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/docs/validation/01-pl011/03-regseq-model.md)
  Regseq portable model and lowering review.
- [04-linux-backend-plan.md](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/docs/validation/01-pl011/04-linux-backend-plan.md)
  Linux backend realization review.
- [05-generated-c.md](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/docs/validation/01-pl011/05-generated-c.md)
  Final artifact and wrapper-driven proof review.
- [findings.md](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/docs/validation/01-pl011/findings.md)
  Stable issue ledger for findings that need compiler updates.
- [todo.md](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/docs/validation/01-pl011/todo.md)
  Implementation follow-up ledger for urgent and deferred work surfaced during validation.

## Status Model

Use these status values:

- `not started`
- `in progress`
- `completed`
- `issue found`
- `blocked`

## Review Method

For each boundary, answer:

1. what information exists upstream?
2. what representation exists downstream?
3. what information was preserved, transformed, lost, or invented?

Do not jump straight to generated artifacts. Work top-down.

## Exit Condition

PL011 validation is considered clean only when:

- every boundary has been reviewed
- every recorded finding has either:
  - been fixed, or
  - been explicitly deferred with rationale
- the canonical wrapper-driven compiler path produces valid Linux artifacts
- the remaining known gaps are not hidden by PL011-specific backend behavior
