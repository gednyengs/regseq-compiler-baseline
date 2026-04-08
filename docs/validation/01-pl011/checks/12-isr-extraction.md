# Task

## Goal
Validate that the authored ISR facts for PL011 are preserved at the
compiler-side boundary between the authored PL011 source and the current
Regseq extraction state.

## Input Context
- Authored evidence source:
  - [01-authored-source.md](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/docs/validation/01-pl011/01-authored-source.md)
- Authored source files:
  - [driver.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/src/driver.rs)

## Output Path
- [12-isr-extraction.md](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/docs/validation/01-pl011/checks/results/12-isr-extraction.md)

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

- ISR-binding surface:
  - `extract_interrupt_surface(tcx) -> InterruptSurface`
  - inspect `InterruptSurface.isr_bindings`
- Method identity surface:
  - `extract_identity_enrichment(tcx) -> IdentityEnrichmentSurface`
  - inspect `IdentityEnrichmentSurface.impl_blocks`
- Executable ISR-body surface:
  - `extract_body_surface(tcx) -> MirBodySurface`
  - inspect `MirBodySurface.functions`

## Boundary Expectations

- This check targets the current Regseq extraction boundary.
- Validate ISR identity, `#[rseq::isr]` classification, `IrqResult` nominal
  identity, status-read behavior, helper dispatch, and handled/not-handled
  branching.

## Validation Discipline

- Every `pass` must be backed by exercised evidence over the exact surface
  carrying ISR identity and body facts.

## Authored Facts In Scope

ISR in scope:
- `handle_irq`

Authored obligations:
- ISR identity, annotation, signature, and return type survive
- raw `MIS` read, early `NotHandled`, `handled` accumulation, helper dispatch,
  explicit interrupt clearing, and final handled-condition logic remain exactly
  provable
- `IrqResult::{Handled, NotHandled}` remain nominally distinguishable

ISR-specific required predicates:
- reads raw `MIS` exactly once into `status`
- if `status == 0`, returns `IrqResult::NotHandled`
- initializes `handled = false`
- RX branch condition preserves `status & RX_INTERRUPT_MASK (= 0x50) != 0`
- TX branch condition preserves `status & TX_INTERRUPT_MASK (= 0x20) != 0`
- RX branch updates `handled |= self.service_rx_fifo()`
- TX branch updates `handled |= self.service_tx_fifo()`
- calls `clear_explicit_interrupts(status)` after RX/TX service branches
- final handled condition preserves `handled || (status & EXPLICIT_ISR_CLEAR_MASK (= 0x0780) != 0)`
- true branch returns `IrqResult::Handled`
- false branch returns `IrqResult::NotHandled`

## Checks To Validate

### Check Group 1
- Premise:
  - ISR identity, `#[rseq::isr]` role, signature, and return type survive
- Expected preservation condition:
  - extraction preserves those facts explicitly or exactly reconstructably

### Check Group 2
- Premise:
  - ISR body preserves the downstream checks captured in the authored-source
    entry
- Expected preservation condition:
  - extraction preserves enough body information to prove status read,
    branching, helper dispatch, explicit clear behavior, and result-variant
    identity

## Output Schema

Write a certificate to the declared output path that begins with:

- `# Validation Certificate: PL011 ISR Extraction Check`
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
- Assertion predicates must be non-tautological statements about extraction
  output that would fail if the authored fact were not preserved.
- If a reduced or inline specimen is used, no affected check may be marked
  `pass` unless the report includes exercised specimen-fidelity evidence
  against the real fixture source.
- If the authored facts in scope disagree with the fixture source, record a
  blocking authored-source discrepancy before continuing.
