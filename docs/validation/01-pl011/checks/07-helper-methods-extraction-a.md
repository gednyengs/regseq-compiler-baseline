# Task

## Goal
Validate that the first half of the authored helper-method facts for PL011 are
preserved at the compiler-side boundary between the authored PL011 source and
the current Regseq extraction state.

## Input Context
- Authored evidence source:
  - [01-authored-source.md](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/docs/validation/01-pl011/01-authored-source.md)
- Authored source files:
  - [driver.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/src/driver.rs)

## Output Path
- [07-helper-methods-extraction-a.md](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/docs/validation/01-pl011/checks/results/07-helper-methods-extraction-a.md)

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

- Method identity surface:
  - `extract_identity_enrichment(tcx) -> IdentityEnrichmentSurface`
  - inspect `IdentityEnrichmentSurface.impl_blocks`
- Executable helper-body surface:
  - `extract_body_surface(tcx) -> MirBodySurface`
  - inspect `MirBodySurface.functions`
- Supporting register/call-site surfaces when needed:
  - `extract_register_access_surface(tcx) -> RegisterAccessSurface`
    - inspect `wrapper_call_sites`
  - `extract_clock_surface(tcx) -> ClockSurface`
    - inspect `call_sites`

## Boundary Expectations

- This check targets the current Regseq extraction boundary.
- Validate helper-method identity, signature, call structure, and the exact
  downstream checks captured in the authored-source ledger for the methods in
  scope.

## Validation Discipline

- Every `pass` must be backed by exercised evidence over the exact surface
  carrying helper identity and body facts.

## Authored Facts In Scope

Helper methods in scope:
- `disable_uart`
- `disable_fifo`
- `mask_all_interrupts`
- `clear_all_interrupts`
- `configure_interrupt_levels`
- `set_baud_rate`
- `configure_line_control`
- `enable_uart`
- `enable_rx_interrupts`
- `enable_tx_interrupts`
- `disable_tx_interrupts`
- `program_polling_port`

Authored obligations:
- method identity, signature, parameters, and return type survive
- body extraction preserves the per-method downstream checks captured in the
  authored-source entries
- ordering, call sequence, arithmetic, and literal/mask values remain exactly
  provable where the authored-source entries require them

Method-specific required predicates:
- `disable_uart`
  - one write to `cr`
  - value preserves `UARTEN = DISABLED`, `TXE = DISABLED`, `RXE = DISABLED`
  - no return value
- `disable_fifo`
  - one logical modify of `lcr_h`
  - exactly one source read and one write-back
  - only `FEN` changes to `DISABLED`
- `mask_all_interrupts`
  - one write to `imsc`
  - uses `ALL_UART_INTERRUPTS_MASK`
  - masked bits written as `0`
- `clear_all_interrupts`
  - one write to `icr`
  - mask and value both preserve `ALL_UART_INTERRUPTS_MASK (= 0x07ff)`
- `configure_interrupt_levels`
  - one write to `ifls`
  - preserves `RXIFLSEL = ONE_HALF`, `TXIFLSEL = ONE_EIGHTH`
- `set_baud_rate`
  - preserves parameters `uart_clk_hz`, `baud_rate`
  - preserves `u64` widening and divisor arithmetic
  - one write to `ibrd`, one write to `fbrd`
- `configure_line_control`
  - one write to `lcr_h`
  - preserves `WLEN = EIGHT_BITS`, `FEN = ENABLED`
  - preserves materialized literal `0x70`
- `enable_uart`
  - one write to `cr`
  - preserves enabled `UARTEN`, `TXE`, `RXE`
  - preserves materialized literal `0x301`
- `enable_rx_interrupts`
  - one `IMSC` read-modify-write
  - only bits selected by `RX_INTERRUPT_MASK (= 0x50)` change
- `enable_tx_interrupts`
  - one `IMSC` read-modify-write
  - only bits selected by `TX_INTERRUPT_MASK (= 0x20)` change
- `disable_tx_interrupts`
  - one `IMSC` read-modify-write
  - only bits selected by `TX_INTERRUPT_MASK (= 0x20)` change to `0`
- `program_polling_port`
  - `disable_uart()` then `disable_fifo()` before the ordered region
  - ordered region is bracketed by leading/trailing full memory barriers
  - ordered helper-call sequence:
    - `set_baud_rate`
    - `configure_line_control`
    - `configure_interrupt_levels`
    - `mask_all_interrupts`
    - `clear_all_interrupts`
    - `enable_uart`
    - `enable_rx_interrupts`

## Checks To Validate

### Check Group 1
- Premise:
  - every in-scope helper preserves identity, signature, and return type
- Expected preservation condition:
  - extraction preserves those declaration facts explicitly or exactly
    reconstructably

### Check Group 2
- Premise:
  - every in-scope helper preserves the body-level downstream checks captured in
    its authored-source entry
- Expected preservation condition:
  - extraction preserves enough body information to prove each required helper
    call, register interaction, literal, parameter flow, ordering edge, and
    return behavior

## Output Schema

Write a certificate to the declared output path that begins with:

- `# Validation Certificate: PL011 Helper Methods Extraction Check A`
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
