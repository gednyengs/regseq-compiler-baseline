# Task

## Goal
Validate that the first half of the authored serial-callback facts for PL011
are preserved at the compiler-side boundary between the authored PL011 source
and the current Regseq extraction state.

## Input Context
- Authored evidence source:
  - [01-authored-source.md](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/docs/validation/01-pl011/01-authored-source.md)
- Authored source files:
  - [driver.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/src/driver.rs)

## Output Path
- [10-serial-callbacks-extraction-a.md](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/docs/validation/01-pl011/checks/results/10-serial-callbacks-extraction-a.md)

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
- Validate serial-callback identity, signature, helper sequencing, branching,
  register interactions, opaque parameter preservation, and return behavior.

## Validation Discipline

- Every `pass` must be backed by exercised evidence over the exact surface
  carrying callback identity and body facts.

## Authored Facts In Scope

Serial callbacks in scope:
- `port_open`
- `port_close`
- `configure`
- `tx_byte`
- `rx_char`

Authored obligations:
- callback identity, trait role, signature, and return type survive
- helper-call sequencing and error propagation remain exactly provable
- opaque `_config: &SerialConfig` preservation remains provable
- staged/fast-path branching, register reads/writes, and option-return
  behavior remain exactly provable
- body extraction preserves the per-callback downstream checks captured in the
  authored-source entries

Callback-specific required predicates:
- `port_open`
  - helper order:
    - `enable_clock_if_present()?`
    - `program_polling_port(self.uart_clock_hz(), self.current_speed)`
  - success path returns `Ok(())`
- `port_close`
  - helper order:
    - `shutdown_port()`
    - `disable_clock_if_present()`
  - no return value
- `configure`
  - preserves `_config: &SerialConfig` as opaque
  - helper order:
    - `enable_clock_if_present()?`
    - `program_polling_port(self.uart_clock_hz(), self.current_speed)`
  - success path returns `Ok(())`
- `tx_byte`
  - branch condition preserves `self.pending_tx.is_none() && self.tx_ready()`
  - fast path writes `DR.DATA(byte as u32)` and returns immediately
  - staged path assigns `self.pending_tx = Some(byte)` then calls `enable_tx_interrupts()`
- `rx_char`
  - returns `Option<RxChar>`
  - empty-FIFO path tests `FR.RXFE` and returns `None`
  - non-empty path performs one raw `DR` read reused for byte/error extraction
  - accumulates `OVERRUN`, `BREAK`, `PARITY`, `FRAMING`
  - returns `Some(RxChar::new(byte, errors))`

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
  - extraction preserves enough body information to prove helper sequencing,
    branching, register interactions, option/result behavior, and argument flow

## Output Schema

Write a certificate to the declared output path that begins with:

- `# Validation Certificate: PL011 Serial Callbacks Extraction Check A`
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
