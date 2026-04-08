# Task

## Goal
Validate that the authored per-register definition facts for the PL011 register
set are preserved at the compiler-side boundary between the authored PL011
source and the current Regseq extraction state.

## Input Context
- Authored evidence source:
  - [01-authored-source.md](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/docs/validation/01-pl011/01-authored-source.md)
- Authored source files:
  - [registers.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/src/registers.rs)

## Output Path
- [02-register-definitions-extraction.md](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/docs/validation/01-pl011/checks/results/02-register-definitions-extraction.md)

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

- Primary descriptor surface:
  - `extract_register_access_surface(tcx) -> RegisterAccessSurface`
  - inspect `RegisterAccessSurface.register_descriptors`
- Supporting block-order surface:
  - `extract_register_access_surface(tcx) -> RegisterAccessSurface`
  - inspect `RegisterAccessSurface.register_blocks`

## Boundary Expectations

- This check targets the compiler-side boundary between authored source and the
  current Regseq extraction state that Regseq later normalizes and serializes.
- Validation should use compiler-side programmatic inspection where possible.
- A persisted artifact such as `raw-frontend.json` may be consulted as
  secondary supporting evidence, but it is not the authoritative target of this
  check.
- Classify outcomes using this rule:
  - `pass`: the authored fact is explicitly preserved, or the boundary carries
    enough explicit evidence to reconstruct it exactly without heuristic
    guesswork
  - `fail`: the check spec says the fact should survive at this boundary, but
    the boundary state drops it, changes it, or makes exact reconstruction
    impossible
  - `unclear`: the check spec leaves room for multiple reasonable readings of
    what should survive at this boundary, or the inspected boundary preserves
    only a weaker related fact and the specification does not clearly say
    whether that is sufficient

## Validation Discipline

- A check may be marked `pass` only if the exercised evidence proves the
  premise at the same level of specificity as authored.
- A weaker related fact may appear in `Supporting Evidence`, but it may not
  satisfy the check by itself.
- Do not upgrade:
  - suffix matches into full identity preservation
  - partial shape matches into exact equivalence
  - analogous mechanisms into proven preservation
- If a reduced specimen is used, it must preserve every authored fact needed to
  exercise the check exactly. Otherwise the affected check cannot be marked
  `pass`.
- Judge only what the inspected boundary explicitly preserves or exactly
  reconstructs. Do not mark `pass` based on what would probably hold in a
  different specimen or later stage.
- If exercised evidence is weaker than the premise, prefer `fail` unless the
  specification itself is ambiguous enough to justify `unclear`.

## Authored Facts In Scope

Registers in scope:
- `DR_REG`
- `RSR_ECR_REG`
- `FR_REG`
- `ILPR_REG`
- `IBRD_REG`
- `FBRD_REG`
- `LCRH_REG`
- `CR_REG`
- `IFLS_REG`
- `IMSC_REG`
- `RIS_REG`
- `MIS_REG`
- `ICR_REG`
- `DMACR_REG`
- `PERIPHID0_REG`
- `PERIPHID1_REG`
- `PERIPHID2_REG`
- `PERIPHID3_REG`
- `PCELLID0_REG`
- `PCELLID1_REG`
- `PCELLID2_REG`
- `PCELLID3_REG`

Per-register authored obligations:
- register identity is preserved
- field inventory remains complete and in authored order
- each field preserves:
  - field name
  - bit offset
  - bit width
- each named field value preserves:
  - symbolic name
  - literal value
- no fields are inserted, removed, renamed, or reordered
- no named field values are inserted, removed, renamed, or changed

Register-specific required inventories:
- `DR_REG`: fields `OE`, `BE`, `PE`, `FE`, `DATA`
- `RSR_ECR_REG`: fields `OE`, `BE`, `PE`, `FE`
- `FR_REG`: fields `RI`, `TXFE`, `RXFF`, `TXFF`, `RXFE`, `BUSY`, `DCD`, `DSR`, `CTS`
- `ILPR_REG`: field `ILPDVSR`
- `IBRD_REG`: field `BAUD_DIVINT`
- `FBRD_REG`: field `BAUD_DIVFRAC`
- `LCRH_REG`: fields `SPS`, `WLEN`, `FEN`, `STP2`, `EPS`, `PEN`, `BRK`
  - required named values:
    - `SPS`: `DISABLED = 0`, `ENABLED = 1`
    - `WLEN`: `FIVE_BITS = 0b00`, `SIX_BITS = 0b01`, `SEVEN_BITS = 0b10`, `EIGHT_BITS = 0b11`
    - `FEN`: `DISABLED = 0`, `ENABLED = 1`
    - `STP2`: `ONE_STOP_BIT = 0`, `TWO_STOP_BITS = 1`
    - `EPS`: `ODD = 0`, `EVEN = 1`
    - `PEN`: `DISABLED = 0`, `ENABLED = 1`
    - `BRK`: `DISABLED = 0`, `ENABLED = 1`
- `CR_REG`: fields `CTSEN`, `RTSEN`, `OUT2`, `OUT1`, `RTS`, `DTR`, `RXE`, `TXE`, `LBE`, `SIRLP`, `SIREN`, `UARTEN`
  - required named values:
    - `CTSEN`: `DISABLED = 0`, `ENABLED = 1`
    - `RTSEN`: `DISABLED = 0`, `ENABLED = 1`
    - `OUT2`, `OUT1`, `RTS`, `DTR`: inactive/active pairs `0`/`1`
    - `RXE`, `TXE`, `LBE`, `SIREN`, `UARTEN`: `DISABLED = 0`, `ENABLED = 1`
    - `SIRLP`: `NORMAL = 0`, `LOW_POWER = 1`
- `IFLS_REG`: fields `RXIFLSEL`, `TXIFLSEL`
  - required named values:
    - `ONE_EIGHTH = 0b000`, `ONE_QUARTER = 0b001`, `ONE_HALF = 0b010`, `THREE_QUARTERS = 0b011`, `SEVEN_EIGHTHS = 0b100`
- `IMSC_REG`: fields `OEIM`, `BEIM`, `PEIM`, `FEIM`, `RTIM`, `TXIM`, `RXIM`, `DSRMIM`, `DCDMIM`, `CTSMIM`, `RIMIM`
- `RIS_REG`: fields `OERIS`, `BERIS`, `PERIS`, `FERIS`, `RTRIS`, `TXRIS`, `RXRIS`, `DSRRMIS`, `DCDRMIS`, `CTSRMIS`, `RIRMIS`
- `MIS_REG`: fields `OEMIS`, `BEMIS`, `PEMIS`, `FEMIS`, `RTMIS`, `TXMIS`, `RXMIS`, `DSRMMIS`, `DCDMMIS`, `CTSMMIS`, `RIMMIS`
- `ICR_REG`: fields `OEIC`, `BEIC`, `PEIC`, `FEIC`, `RTIC`, `TXIC`, `RXIC`, `DSRMIC`, `DCDMIC`, `CTSMIC`, `RIMIC`
- `DMACR_REG`: fields `DMAONERR`, `TXDMAE`, `RXDMAE`
  - required named values:
    - each field preserves `DISABLED = 0`, `ENABLED = 1`
- `PERIPHID0_REG`: field `PARTNUMBER0`
  - required named value: `PL011 = 0x11`
- `PERIPHID1_REG`: fields `DESIGNER0`, `PARTNUMBER1`
  - required named values: `DESIGNER0.ARM_LOW = 0x1`, `PARTNUMBER1.PL011 = 0x0`
- `PERIPHID2_REG`: fields `REVISION`, `DESIGNER1`
  - required named values:
    - `REVISION`: `R1P0 = 0x0`, `R1P1 = 0x1`, `R1P3 = 0x2`, `R1P4 = 0x2`, `R1P5 = 0x3`
    - `DESIGNER1.ARM_HIGH = 0x4`
- `PERIPHID3_REG`: field `CONFIGURATION`
  - required named value: `DEFAULT = 0x00`
- `PCELLID0_REG`: field `VALUE`
  - required named value: `DEFAULT = 0x0D`
- `PCELLID1_REG`: field `VALUE`
  - required named value: `DEFAULT = 0xF0`
- `PCELLID2_REG`: field `VALUE`
  - required named value: `DEFAULT = 0x05`
- `PCELLID3_REG`: field `VALUE`
  - required named value: `DEFAULT = 0xB1`

## Checks To Validate

### Check Group 1
- Premise:
  - every in-scope register remains identifiable by its authored register name
- Expected preservation condition:
  - the inspected boundary preserves register identity explicitly or carries
    enough evidence to reconstruct it exactly

### Check Group 2
- Premise:
  - every in-scope register preserves its complete field inventory in authored
    order
- Expected preservation condition:
  - the inspected boundary preserves field membership and order exactly

### Check Group 3
- Premise:
  - every field in every in-scope register preserves field name, bit offset,
    and bit width
- Expected preservation condition:
  - the inspected boundary preserves those field facts explicitly or exactly
    reconstructably

### Check Group 4
- Premise:
  - every authored named field value preserves symbolic name and literal value
- Expected preservation condition:
  - the inspected boundary preserves named-value facts explicitly or exactly
    reconstructably

### Check Group 5
- Premise:
  - no fields or named values are inserted, removed, renamed, reordered, or
    changed
- Expected preservation condition:
  - the inspected boundary preserves exact authored membership and ordering

## Output Schema

Write a certificate to the declared output path that begins with:

- `# Validation Certificate: PL011 Register Definitions Extraction Check`
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

- Include a `Specimen Fidelity` section.
- `Specimen Fidelity` must include:
  - `Fidelity Assertion Predicate(s):`
  - `Fidelity Evidence (Exact Citations):`
- Include a `Check-to-Surface Mapping` section.
- Every `Target Evidence (Exercised by Test; Exact Citations)` section must
  cite exact evidence by path and line or other structured location.
- Every `Target Evidence` section must include the assertion predicate itself,
  not just the file location of the test.
- Every `pass` claim must be supported by exercised test evidence.
- `Supporting Evidence` may clarify, but may not by itself justify `pass`.
- If a premise names multiple properties, the exercised assertion set must
  cover every named property. Partial assertions may not justify `pass` for the
  full premise.
- Absence claims such as "no fields or named values are inserted, removed,
  renamed, reordered, or changed" require closed-world evidence such as
  exact-length checks, exhaustive enumeration, or exact-set equality.
- Assertion predicates must be non-tautological statements about extraction
  output that would fail if the authored fact were not preserved.
- If a reduced or inline specimen is used, no affected check may be marked
  `pass` unless the report includes exercised specimen-fidelity evidence
  against the real fixture source.
- If the authored facts in scope disagree with the fixture source, record a
  blocking authored-source discrepancy before continuing.
- If claiming `fail`, provide a concrete counterexample and a narrow fix hint.
