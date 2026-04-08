# Task

## Goal
Validate that the authored driver-struct declaration facts for PL011 are
preserved at the compiler-side boundary between the authored PL011 source and
the current Regseq extraction state.

## Input Context
- Authored evidence source:
  - [01-authored-source.md](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/docs/validation/01-pl011/01-authored-source.md)
- Authored source files:
  - [driver.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/src/driver.rs)
  - [registers.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/src/registers.rs)

## Output Path
- [03-driver-struct-extraction.md](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/docs/validation/01-pl011/checks/results/03-driver-struct-extraction.md)

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

- Driver identity surface:
  - `extract_driver_identities(tcx) -> Vec<DriverIdentity>`
- Canonical identity and field classification surface:
  - `extract_identity_enrichment(tcx) -> IdentityEnrichmentSurface`
  - inspect `IdentityEnrichmentSurface.structs`
  - inspect `IdentityEnrichmentSurface.modules`
- Resource/property declaration surfaces:
  - `extract_register_access_surface(tcx) -> RegisterAccessSurface`
    - inspect `mmio_fields`
  - `extract_interrupt_surface(tcx) -> InterruptSurface`
    - inspect `resources`
  - `extract_clock_surface(tcx) -> ClockSurface`
    - inspect `fields`
  - `extract_reset_surface(tcx) -> ResetSurface`
    - inspect `fields`
  - `extract_pinctrl_surface(tcx) -> PinCtrlSurface`
    - inspect `fields`
  - `extract_property_surface(tcx) -> PropertySurface`
    - inspect `fields`

## Boundary Expectations

- This check targets the compiler-side boundary between authored source and the
  current Regseq extraction state that Regseq later normalizes and serializes.
- Validation should use compiler-side programmatic inspection where possible.
- Classify outcomes using:
  - `pass`: explicitly preserved or exactly reconstructable without heuristics
  - `fail`: dropped, changed, or not exactly reconstructable
  - `unclear`: specification ambiguity rather than implementation ambiguity

## Validation Discipline

- A `pass` requires exercised evidence at the same specificity as the authored
  premise.
- Weaker related facts may appear in `Supporting Evidence`, but may not justify
  `pass`.
- Reduced specimens must preserve every authored fact needed by the checks.

## Authored Facts In Scope

Driver identity:
- `name = "pl011-full-spec"`
- `compatible = ["arm,pl011"]`
- `bus = "amba"`
- canonical path of driver struct:
  - `pl011_full_spec::driver::Pl011FullSpec`

Device class:
- `serial`

Resources:
- `regs`: canonical path `pl011_full_spec::driver::Pl011FullSpec::regs`, kind `mmio`, type `Mmio<Pl011Registers>`, required, `id = "uart_regs"`, join key `Properties.id`
- `irq`: canonical path `pl011_full_spec::driver::Pl011FullSpec::irq`, kind `interrupt`, type `Interrupt`, required, `id = "uart_irq"`, join key `Properties.id`
- `clock`: canonical path `pl011_full_spec::driver::Pl011FullSpec::clock`, kind `clock`, type `Option<Clock>`, optional, `id = "uart_clk"`, join key `Properties.id`
- `reset`: canonical path `pl011_full_spec::driver::Pl011FullSpec::reset`, kind `reset`, type `Option<ResetLine>`, optional, join key `Field name`
- `pinctrl`: canonical path `pl011_full_spec::driver::Pl011FullSpec::pinctrl`, kind `pinctrl`, type `Option<PinCtrl>`, optional, `id = "uart_pins"`, join key `Properties.id`

Properties:
- `current_speed`: canonical path `pl011_full_spec::driver::Pl011FullSpec::current_speed`, type `u32`, id `current-speed`, default `115200`, required
- `fifo_size`: canonical path `pl011_full_spec::driver::Pl011FullSpec::fifo_size`, type `u32`, id `fifo-size`, default `32`, required

User field:
- `pending_tx`: canonical path `pl011_full_spec::driver::Pl011FullSpec::pending_tx`, type `Option<u8>`, ordinary user state, not a Regseq resource or property

## Checks To Validate

### Check Group 1
- Premise:
  - driver identity preserves `name`, `compatible`, and `bus`
- Expected preservation condition:
  - extraction preserves those facts explicitly or exactly reconstructably

### Check Group 2
- Premise:
  - device class remains `serial`
- Expected preservation condition:
  - extraction preserves device-class identity exactly

### Check Group 3
- Premise:
  - each resource preserves field name, kind, Rust type, requiredness,
    resource properties, and join-key semantics
- Expected preservation condition:
  - extraction preserves those resource facts explicitly or exactly
    reconstructably

### Check Group 4
- Premise:
  - each property preserves field name, type, id, default, and requiredness
- Expected preservation condition:
  - extraction preserves those property facts explicitly or exactly
    reconstructably

### Check Group 5
- Premise:
  - `pending_tx` remains an ordinary user field and is not reclassified as a
    Regseq field
- Expected preservation condition:
  - extraction preserves user-field identity and does not invent resource or
    property metadata for it

## Output Schema

Write a certificate to the declared output path that begins with:

- `# Validation Certificate: PL011 Driver Struct Extraction Check`
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
- Absence claims such as "pending_tx is not reclassified" require closed-world
  evidence over the complete set of extracted classifications or metadata.
- Assertion predicates must be non-tautological statements about extraction
  output that would fail if the authored fact were not preserved.
- If a reduced or inline specimen is used, no affected check may be marked
  `pass` unless the report includes exercised specimen-fidelity evidence
  against the real fixture source.
- If the authored facts in scope disagree with the fixture source, record a
  blocking authored-source discrepancy before continuing.
- If claiming `fail`, provide a concrete counterexample and a narrow fix hint.
