# Task

## Goal
Validate that the second half of the authored helper-method facts for PL011 are
preserved at the compiler-side boundary between the authored PL011 source and
the current Regseq extraction state.

## Input Context
- Authored evidence source:
  - [01-authored-source.md](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/docs/validation/01-pl011/01-authored-source.md)
- Authored source files:
  - [driver.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/src/driver.rs)

## Output Path
- [08-helper-methods-extraction-b.md](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/docs/validation/01-pl011/checks/results/08-helper-methods-extraction-b.md)

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
- Supporting resource/call-site surfaces when needed:
  - `extract_clock_surface(tcx) -> ClockSurface`
    - inspect `fields`, `call_sites`
  - `extract_reset_surface(tcx) -> ResetSurface`
    - inspect `fields`, `call_sites`
  - `extract_pinctrl_surface(tcx) -> PinCtrlSurface`
    - inspect `fields`, `call_sites`

## Boundary Expectations

- This check targets the current Regseq extraction boundary.
- Validate helper-method identity, signature, option/result handling, resource
  operations, user-state updates, and the exact downstream checks captured in
  the authored-source ledger for the methods in scope.

## Validation Discipline

- Every `pass` must be backed by exercised evidence over the exact surface
  carrying helper identity and body facts.

## Authored Facts In Scope

Helper methods in scope:
- `uart_clock_hz`
- `enable_clock_if_present`
- `disable_clock_if_present`
- `toggle_reset_if_present`
- `apply_default_pins_if_present`
- `apply_sleep_pins_if_present`
- `shutdown_port`
- `service_rx_fifo`
- `service_tx_fifo`
- `clear_explicit_interrupts`

Authored obligations:
- method identity, signature, parameters, and return type survive
- option/result branching remains exactly provable
- resource-property and resource-operation identity remains attached to the
  correct resource field in focus
- user-field updates and boolean/option return behavior remain exactly provable
- body extraction preserves the per-method downstream checks captured in the
  authored-source entries

Method-specific required predicates:
- `uart_clock_hz`
  - returns `u32`
  - `Some(clock)` path calls `Clock::get_rate`
  - `None` path falls back to `DEFAULT_UART_CLOCK_HZ (= 48_000_000)`
  - clock-rate read stays attached to the specific `clock` field
- `enable_clock_if_present`
  - tests `self.clock` once
  - `Some` path performs fallible `Clock::enable`
  - `None` path performs no resource operation
- `disable_clock_if_present`
  - tests `self.clock` once
  - `Some` path performs `Clock::disable`
  - `None` path performs no resource operation
- `toggle_reset_if_present`
  - tests `self.reset` once
  - `Some` path performs fallible `ResetLine::toggle(Duration::from_us(10))`
  - `None` path performs no resource operation
- `apply_default_pins_if_present`
  - `Some` path performs fallible `PinCtrl::apply::<DefaultState>`
  - `None` path performs no resource operation
- `apply_sleep_pins_if_present`
  - `Some` path performs fallible `PinCtrl::apply::<SleepState>`
  - `None` path performs no resource operation
- `shutdown_port`
  - helper-call order: `mask_all_interrupts`, `clear_all_interrupts`, `disable_fifo`, `disable_uart`
  - then `self.pending_tx = None`
- `service_rx_fifo`
  - initializes `handled = false`
  - loops on `self.rx_char().is_some()`
  - sets `handled = true` on each successful iteration
  - returns exactly final `handled`
- `service_tx_fifo`
  - branches on `self.pending_tx`
  - ready `Some(byte)` path writes `DR.DATA`, clears `pending_tx`, disables TX interrupts, returns `true`
  - not-ready `Some(byte)` path returns `false`
  - `None` path disables TX interrupts then returns `true`
- `clear_explicit_interrupts`
  - computes `clear_mask = status & EXPLICIT_ISR_CLEAR_MASK (= 0x0780)`
  - writes `icr` only when `clear_mask != 0`
  - uses `clear_mask` as both raw write mask and value

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
  - extraction preserves enough body information to prove each required branch,
    resource interaction, state update, literal, and return behavior

## Output Schema

Write a certificate to the declared output path that begins with:

- `# Validation Certificate: PL011 Helper Methods Extraction Check B`
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
