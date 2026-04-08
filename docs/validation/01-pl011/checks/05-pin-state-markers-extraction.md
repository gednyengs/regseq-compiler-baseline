# Task

## Goal
Validate that the authored pin-state marker facts for PL011 are preserved at
the compiler-side boundary between the authored PL011 source and the current
Regseq extraction state.

## Input Context
- Authored evidence source:
  - [01-authored-source.md](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/docs/validation/01-pl011/01-authored-source.md)
- Authored source files:
  - [driver.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/src/driver.rs)

## Output Path
- [05-pin-state-markers-extraction.md](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/docs/validation/01-pl011/checks/results/05-pin-state-markers-extraction.md)

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

- Type identity surface:
  - `extract_identity_enrichment(tcx) -> IdentityEnrichmentSurface`
  - inspect `IdentityEnrichmentSurface.structs`
- Usage-site surface:
  - `extract_pinctrl_surface(tcx) -> PinCtrlSurface`
  - inspect `PinCtrlSurface.call_sites`
- Executable body surface when call-site reconstruction is insufficient:
  - `extract_body_surface(tcx) -> MirBodySurface`
  - inspect `MirBodySurface.functions`

## Boundary Expectations

- This check targets the current Regseq extraction boundary.
- Validate marker-type identity and trait-implementation evidence without
  surrogate reasoning.

## Validation Discipline

- Every `pass` must be backed by exercised evidence over the exact surface
  carrying type and trait facts.

## Authored Facts In Scope

Pin-state marker types:
- `DefaultState`
  - canonical path: `pl011_full_spec::driver::DefaultState`
  - zero-sized marker type
  - implements `PinState`
- `SleepState`
  - canonical path: `pl011_full_spec::driver::SleepState`
  - zero-sized marker type
  - implements `PinState`

Authored obligations:
- marker-type identity remains distinct
- `DefaultState` and `SleepState` do not collapse
- `PinState` implementation evidence remains available
- downstream pinctrl state application can still distinguish the two markers
- usage-site dependency:
  - `apply_default_pins_if_present` must preserve `pinctrl.apply::<DefaultState>()`
  - `apply_sleep_pins_if_present` must preserve `pinctrl.apply::<SleepState>()`

## Checks To Validate

### Check Group 1
- Premise:
  - both marker types remain identifiable and distinct
- Expected preservation condition:
  - extraction preserves distinct type identity explicitly or exactly
    reconstructably

### Check Group 2
- Premise:
  - `PinState` implementation evidence survives for each marker
- Expected preservation condition:
  - extraction preserves enough trait identity or type-role evidence to prove
    marker participation in pinctrl state selection

## Output Schema

Write a certificate to the declared output path that begins with:

- `# Validation Certificate: PL011 Pin-State Markers Extraction Check`
- `Source premise: <path reference>`
- `Target boundary evidence: <path reference(s)>`

For each check, produce:

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
- Absence claims such as "the markers do not collapse" require closed-world
  evidence over the complete extracted marker set or usage-site set.
- Assertion predicates must be non-tautological statements about extraction
  output that would fail if the authored fact were not preserved.
- If a reduced or inline specimen is used, no affected check may be marked
  `pass` unless the report includes exercised specimen-fidelity evidence
  against the real fixture source.
- If the authored facts in scope disagree with the fixture source, record a
  blocking authored-source discrepancy before continuing.
