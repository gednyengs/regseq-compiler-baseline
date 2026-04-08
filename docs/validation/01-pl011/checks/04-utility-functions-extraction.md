# Task

## Goal
Validate that the authored local utility-function facts for PL011 are preserved
at the compiler-side boundary between the authored PL011 source and the current
Regseq extraction state.

## Input Context
- Authored evidence source:
  - [01-authored-source.md](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/docs/validation/01-pl011/01-authored-source.md)
- Authored source files:
  - [driver.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/src/driver.rs)

## Output Path
- [04-utility-functions-extraction.md](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/docs/validation/01-pl011/checks/results/04-utility-functions-extraction.md)

## Validation Precondition

- Before evaluating the extraction boundary, verify that every authored fact
  cited in `Authored Facts In Scope` matches the actual PL011 fixture source.
- If a reduced or inline specimen is used, specimen fidelity must be
  established by exercised evidence against the real fixture source for every
  authored fact relevant to the checks in scope.
- Narrative equivalence claims are not sufficient to satisfy this precondition.
- Any discrepancy between this check contract and the fixture source is a
  blocking authored-source transcription error. Record it in the report and do
  not mark extraction checks `pass`.

## Extraction Surface

- Free-function identity surface:
  - `extract_identity_enrichment(tcx) -> IdentityEnrichmentSurface`
  - inspect `IdentityEnrichmentSurface.free_functions`
- Executable body surface:
  - `extract_body_surface(tcx) -> MirBodySurface`
  - inspect `MirBodySurface.functions`

## Boundary Expectations

- This check targets the current Regseq extraction boundary.
- Validation should inspect whichever extraction surfaces carry free-function
  identity and body information.
- Use `pass` only for explicitly preserved or exactly reconstructable facts.

## Validation Discipline

- Every `pass` must be backed by exercised test evidence over the exact surface
  needed by the check.
- Reduced specimens must preserve generic signatures and bodies exactly for the
  functions in scope.

## Authored Facts In Scope

Utility functions:
- `field_val`
  - canonical path: `pl011_full_spec::driver::field_val`
  - signature: `fn field_val<R>(field: Field<u32, R>, raw: u32) -> FieldValue<u32, R>`
  - exact body effect: `field.val(raw)`
- `raw_reg_val`
  - canonical path: `pl011_full_spec::driver::raw_reg_val`
  - signature: `fn raw_reg_val<R>(mask: u32, value: u32) -> FieldValue<u32, R>`
  - exact body effect: `FieldValue::<u32, R>::new(mask, value)`
- `field_from_raw`
  - canonical path: `pl011_full_spec::driver::field_from_raw`
  - signature: `fn field_from_raw<R>(field: Field<u32, R>, raw: u32) -> u32`
  - exact body effect: `field.read(raw)`

## Checks To Validate

### Check Group 1
- Premise:
  - each utility function preserves identity, canonical path, generics,
    parameter list, and return type
- Expected preservation condition:
  - extraction preserves those declaration facts explicitly or exactly
    reconstructably

### Check Group 2
- Premise:
  - each utility body preserves its exact authored call shape and parameter flow
- Expected preservation condition:
  - extraction preserves enough body information to prove exact call target and
    argument mapping

## Output Schema

Write a certificate to the declared output path that begins with:

- `# Validation Certificate: PL011 Utility Functions Extraction Check`
- `Source premise: <path reference>`
- `Target boundary evidence: <path reference(s)>`

- Check:
- Status: `pass` | `fail` | `unclear`
- Premise:
- Assertion Predicate(s):
- Target Evidence (Exercised by Test; Exact Citations):
- Supporting Evidence (Optional; Exact Citations):
- Reasoning Trace:
-   Authored Fact (Verbatim):
-   Extracted Fact (Verbatim):
-   Correspondence: `exact` | `weakened` | `absent`
-   Gap / Loss Description:
-   Why This Status Follows:
- Counterexample:
- Suggested Fix Direction / Implementation Hints:
  - Likely Touchpoint:
  - Minimal Missing Fact To Preserve:
- Conclusion:

## Recording Rules

- Include `Specimen Fidelity` and `Check-to-Surface Mapping`.
- `Specimen Fidelity` must include:
  - `Fidelity Assertion Predicate(s):`
  - `Fidelity Evidence (Exact Citations):`
- Every `Target Evidence` section must include the assertion predicate itself,
  not just the file location of the test.
- Every `pass` must be backed by exercised evidence.
- `Supporting Evidence` may clarify, but may not by itself justify `pass`.
- If a premise names multiple properties, the exercised assertion set must
  cover every named property. Partial assertions may not justify `pass` for the
  full premise.
- Assertion predicates must be non-tautological statements about extraction
  output that would fail if the authored fact were not preserved.
- If a reduced or inline specimen is used, no affected check may be marked
  `pass` unless the report includes exercised specimen-fidelity evidence
  against the real fixture source.
- If the authored facts in scope disagree with the fixture source, record a
  blocking authored-source discrepancy before continuing.
