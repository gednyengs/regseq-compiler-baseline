# Task

## Goal
Validate that the authored driver-constant facts for PL011 are preserved at the
compiler-side boundary between the authored PL011 source and the current
Regseq extraction state.

## Input Context
- Authored evidence source:
  - [01-authored-source.md](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/docs/validation/01-pl011/01-authored-source.md)
- Authored source files:
  - [driver.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/src/driver.rs)

## Output Path
- [06-driver-constants-extraction.md](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/docs/validation/01-pl011/checks/results/06-driver-constants-extraction.md)

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

- Associated-constant identity surface:
  - `extract_identity_enrichment(tcx) -> IdentityEnrichmentSurface`
  - inspect `IdentityEnrichmentSurface.impl_blocks`
- Executable body surface only when needed to prove definitional use sites:
  - `extract_body_surface(tcx) -> MirBodySurface`
  - inspect `MirBodySurface.functions`

## Boundary Expectations

- This check targets the current Regseq extraction boundary.
- Validate constant identity, Rust type, literal value, definitional
  relationships, and owner-path preservation.

## Validation Discipline

- A `pass` requires exercised evidence that proves exact constant identity and
  value.

## Authored Facts In Scope

Constants:
- `ALL_UART_INTERRUPTS_MASK`
  - Rust type: `u32`
  - value: `0x07ff`
  - canonical path: `pl011_full_spec::driver::Pl011FullSpec::ALL_UART_INTERRUPTS_MASK`
- `RX_INTERRUPT_MASK`
  - Rust type: `u32`
  - value: `(1 << 4) | (1 << 6)`
  - canonical path: `pl011_full_spec::driver::Pl011FullSpec::RX_INTERRUPT_MASK`
- `TX_INTERRUPT_MASK`
  - Rust type: `u32`
  - value: `1 << 5`
  - canonical path: `pl011_full_spec::driver::Pl011FullSpec::TX_INTERRUPT_MASK`
- `ERROR_INTERRUPT_MASK`
  - Rust type: `u32`
  - value: `(1 << 7) | (1 << 8) | (1 << 9) | (1 << 10)`
  - canonical path: `pl011_full_spec::driver::Pl011FullSpec::ERROR_INTERRUPT_MASK`
- `EXPLICIT_ISR_CLEAR_MASK`
  - Rust type: `u32`
  - value: `Self::ERROR_INTERRUPT_MASK`
  - canonical path: `pl011_full_spec::driver::Pl011FullSpec::EXPLICIT_ISR_CLEAR_MASK`
- `DEFAULT_UART_CLOCK_HZ`
  - Rust type: `u32`
  - value: `48_000_000`
  - canonical path: `pl011_full_spec::driver::Pl011FullSpec::DEFAULT_UART_CLOCK_HZ`

Authored obligations:
- constant identity survives
- constant Rust type survives
- constant literal value survives
- owner impl/path association survives
- definitional relationships survive:
  - `EXPLICIT_ISR_CLEAR_MASK` is defined as `Self::ERROR_INTERRUPT_MASK`

## Checks To Validate

### Check Group 1
- Premise:
  - every in-scope constant preserves name, Rust type, value, and owner path
- Expected preservation condition:
  - extraction preserves those constant facts explicitly or exactly
    reconstructably

### Check Group 2
- Premise:
  - definitional relationships between constants survive, especially
    `EXPLICIT_ISR_CLEAR_MASK = Self::ERROR_INTERRUPT_MASK`
- Expected preservation condition:
  - extraction preserves those constant facts explicitly or exactly
    reconstructably

## Output Schema

Write a certificate to the declared output path that begins with:

- `# Validation Certificate: PL011 Driver Constants Extraction Check`
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
- Absence claims such as "no definitional relationships are lost" require
  closed-world evidence over the complete extracted constant set or exact
  ownership membership.
- Assertion predicates must be non-tautological statements about extraction
  output that would fail if the authored fact were not preserved.
- If a reduced or inline specimen is used, no affected check may be marked
  `pass` unless the report includes exercised specimen-fidelity evidence
  against the real fixture source.
- If the authored facts in scope disagree with the fixture source, record a
  blocking authored-source discrepancy before continuing.
