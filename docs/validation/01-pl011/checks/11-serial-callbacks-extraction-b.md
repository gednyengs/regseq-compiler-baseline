# Task

## Goal
Validate that the second half of the authored serial-callback facts for PL011
are preserved at the compiler-side boundary between the authored PL011 source
and the current Regseq extraction state.

## Input Context
- Authored evidence source:
  - [01-authored-source.md](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/docs/validation/01-pl011/01-authored-source.md)
- Authored source files:
  - [driver.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/src/driver.rs)

## Output Path
- [11-serial-callbacks-extraction-b.md](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/docs/validation/01-pl011/checks/results/11-serial-callbacks-extraction-b.md)

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

- Trait-role surface:
  - `extract_trait_surface(tcx) -> TraitSurface`
  - inspect `serial_impls`
- Callback identity surface:
  - `extract_identity_enrichment(tcx) -> IdentityEnrichmentSurface`
  - inspect `IdentityEnrichmentSurface.impl_blocks`
- Executable callback-body surface:
  - `extract_body_surface(tcx) -> MirBodySurface`
  - inspect `MirBodySurface.functions`

## Boundary Expectations

- This check targets the current Regseq extraction boundary.
- Validate serial-callback identity, signature, predicate returns, loop/flush
  behavior, and break-control behavior.

## Validation Discipline

- Every `pass` must be backed by exercised evidence over the exact surface
  carrying callback identity and body facts.

## Authored Facts In Scope

Serial callbacks in scope:
- `tx_ready`
- `tx_empty`
- `flush_rx_fifo`
- `flush_tx_fifo`
- `send_break`

Authored obligations:
- callback identity, trait role, signature, and return type survive
- predicate-return logic remains exactly provable
- flush loops and helper sequencing remain exactly provable
- break-control behavior and literal values remain exactly provable
- body extraction preserves the per-callback downstream checks captured in the
  authored-source entries

Callback-specific required predicates:
- `tx_ready`
  - returns exactly whether TX FIFO is not full using `FR.TXFF`
- `tx_empty`
  - returns exactly whether the transmitter is not busy using `FR.BUSY`
- `flush_rx_fifo`
  - loops while `FR.RXFE == 0`
  - drains `DR` via `read_raw()` on each iteration
- `flush_tx_fifo`
  - performs exactly two sequential `lcr_h.modify(...)` calls
  - first applies `LCRH_REG::FEN::DISABLED`
  - then applies `LCRH_REG::FEN::ENABLED`
- `send_break`
  - branches on `on: bool`
  - modifies `LCR_H`
  - preserves `on == true -> BRK::ENABLED` and `on == false -> BRK::DISABLED`

## Checks To Validate

### Check Group 1
- Premise:
  - serial callback identity, trait role, signature, and return type survive
- Expected preservation condition:
  - extraction preserves those facts explicitly or exactly reconstructably

### Check Group 2
- Premise:
  - serial callback bodies preserve the downstream checks captured in the
    authored-source entries
- Expected preservation condition:
  - extraction preserves enough body information to prove predicate logic,
    loop behavior, helper sequencing, and register interactions

## Output Schema

Write a certificate to the declared output path that begins with:

- `# Validation Certificate: PL011 Serial Callbacks Extraction Check B`
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
