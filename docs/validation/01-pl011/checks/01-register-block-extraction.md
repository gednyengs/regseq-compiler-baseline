# Task

## Goal
Validate that the authored register-block facts for `Pl011Registers` are
preserved at the compiler-side boundary between the authored PL011 source and
the current Regseq extraction state.

## Input Context
- Authored evidence source:
  - [01-authored-source.md](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/docs/validation/01-pl011/01-authored-source.md)
- Authored source files:
  - [registers.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/src/registers.rs)
  - [driver.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/src/driver.rs)

## Output Path
- [01-register-block-extraction.md](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/docs/validation/01-pl011/checks/results/01-register-block-extraction.md)

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

- Primary declaration surface:
  - `extract_register_access_surface(tcx) -> RegisterAccessSurface`
  - inspect `RegisterAccessSurface.register_blocks`
- Canonical/module identity surface:
  - `extract_identity_enrichment(tcx) -> IdentityEnrichmentSurface`
  - inspect `IdentityEnrichmentSurface.structs`
  - inspect `IdentityEnrichmentSurface.modules`

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
    what should survive at this boundary, or the inspected boundary preserves only a
    weaker related fact and the specification does not clearly say whether that
    is sufficient
- For this specific check, apply these boundary-specific expectations:
  - exact canonical-path preservation is preferred, but source-local path
    preservation alone is not enough to count as `pass`
  - register-block properties such as `endian` and `access_width` are expected
    to survive explicitly at the extraction boundary
  - reserved-region byte gaps are expected to survive explicitly, or the
    boundary state must provide enough layout data to reconstruct them exactly
  - register identity checks require preservation of byte offsets and access
    kinds, not just register names
  - a complete extracted field sequence in authored order is sufficient for the
    inventory/order checks even if per-field offset and access metadata is not
    preserved

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

### Register Block: `Pl011Registers`

- Source Local Path:
  - `registers::Pl011Registers`
- Canonical Path:
  - `pl011_full_spec::registers::Pl011Registers`
- Properties:
  - `endian = little`
  - `access_width = 32`
- Reserved Regions:
  - `_reserved0: 0x008 -> 0x014`
  - `_reserved1: 0x01C -> 0x01C`
  - `_reserved2: 0x04C -> 0xFDC`
- Declared Registers:
  - `dr: DR_REG @ 0x000 [RW]`
  - `rsr_ecr: RSR_ECR_REG @ 0x004 [RW]`
  - `fr: FR_REG @ 0x018 [R]`
  - `ilpr: ILPR_REG @ 0x020 [RW]`
  - `ibrd: IBRD_REG @ 0x024 [RW]`
  - `fbrd: FBRD_REG @ 0x028 [RW]`
  - `lcr_h: LCRH_REG @ 0x02C [RW]`
  - `cr: CR_REG @ 0x030 [RW]`
  - `ifls: IFLS_REG @ 0x034 [RW]`
  - `imsc: IMSC_REG @ 0x038 [RW]`
  - `ris: RIS_REG @ 0x03C [R]`
  - `mis: MIS_REG @ 0x040 [R]`
  - `icr: ICR_REG @ 0x044 [W]`
  - `dmacr: DMACR_REG @ 0x048 [RW]`
  - `periph_id0: PERIPHID0_REG @ 0xFE0 [R]`
  - `periph_id1: PERIPHID1_REG @ 0xFE4 [R]`
  - `periph_id2: PERIPHID2_REG @ 0xFE8 [R]`
  - `periph_id3: PERIPHID3_REG @ 0xFEC [R]`
  - `pcell_id0: PCELLID0_REG @ 0xFF0 [R]`
  - `pcell_id1: PCELLID1_REG @ 0xFF4 [R]`
  - `pcell_id2: PCELLID2_REG @ 0xFF8 [R]`
  - `pcell_id3: PCELLID3_REG @ 0xFFC [R]`

## Checks To Validate

### Check 1
- Premise:
  - the register block remains identifiable as `Pl011Registers`
- Expected preservation condition:
  - the inspected boundary state contains register-block identity that still names
    `Pl011Registers`

### Check 2
- Premise:
  - the canonical path remains `pl011_full_spec::registers::Pl011Registers`
- Expected preservation condition:
  - the inspected boundary state preserves the same canonical path or enough path
    evidence to reconstruct it exactly

### Check 3
- Premise:
  - the register-block properties preserve:
    - `endian = little`
    - `access_width = 32`
- Expected preservation condition:
  - the inspected boundary state preserves both authored register-block properties
    without change

### Check 4
- Premise:
  - the reserved regions preserve their authored offsets:
    - `_reserved0: 0x008 -> 0x014`
    - `_reserved1: 0x01C -> 0x01C`
    - `_reserved2: 0x04C -> 0xFDC`
- Expected preservation condition:
  - the inspected boundary state preserves the reserved-layout facts, or preserves
    enough register-block layout information to reconstruct those gaps exactly

### Check 5
- Premise:
  - the declared register inventory remains complete and in authored offset order
- Expected preservation condition:
  - the inspected boundary state preserves the full register inventory and its authored
    ordering by offset

### Check 6
- Premise:
  - each declared register preserves:
    - struct field name
    - register name
    - byte offset
    - access kind
- Expected preservation condition:
  - for every declared register, the inspected boundary state preserves those four facts
    exactly

### Check 7
- Premise:
  - no registers are inserted, removed, renamed, or reordered
- Expected preservation condition:
  - the inspected boundary state introduces no extra register entries and loses none of
    the authored entries

### Check 8
- Premise:
  - no register-block properties are invented or dropped
- Expected preservation condition:
  - the inspected boundary state preserves exactly the authored register-block property
    set relevant to Regseq lowering

## Output Schema

When this check is executed manually or by an agent outside the harness,
write a certificate to the declared output path that begins with:

- `# Validation Certificate: PL011 Register-Block Extraction Check`
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

- Every premise must be evaluated against the authored facts copied into this
  document.
- Include a `Specimen Fidelity` section with:
  - `Fidelity Assertion Predicate(s):`
  - `Fidelity Evidence (Exact Citations):`
- Every `Target Evidence (Exact Citations)` section must cite exact evidence by
  path and line or other structured location from the inspected compiler-side
  boundary.
- Every `Target Evidence` section must include the assertion predicate itself,
  not just the file location of the test.
- Every `pass` claim must be supported by `Target Evidence (Exercised by Test)`
  and not only by prose reasoning or read-only source inspection.
- `Supporting Evidence` is optional and may clarify the reasoning, but it may
  not upgrade a check to `pass` by itself.
- Prose summaries may accompany the citations, but may not replace them.
- If a check depends on a specific extraction surface, the exercised evidence
  must come from a test that actually inspects that surface.
- If claiming `fail`, provide a concrete counterexample.
- If claiming `fail`, include a short `Suggested Fix Direction / Implementation Hints`
  section that points to the narrowest plausible extraction change.
- Keep `Suggested Fix Direction / Implementation Hints` boundary-local:
  identify the likely touchpoint and the minimal missing fact to preserve.
- Avoid speculative redesigns unless the failure evidence shows they are
  necessary.
- If a premise names multiple properties, the exercised assertion set must
  cover every named property. Partial assertions may not justify `pass` for the
  full premise.
- Absence claims such as "no registers are inserted, removed, renamed, or
  reordered" require closed-world evidence such as exact-length checks,
  exhaustive enumeration, or exact-set equality.
- Assertion predicates must be non-tautological statements about extraction
  output that would fail if the authored fact were not preserved.
- If a reduced or inline specimen is used, no affected check may be marked
  `pass` unless the report includes exercised specimen-fidelity evidence
  against the real fixture source.
- If the authored facts in scope disagree with the fixture source, record a
  blocking authored-source discrepancy before continuing.
- If claiming `pass`, state why no counterexample was found.
- If claiming `unclear`, explain exactly which boundary expectation is
  underspecified.
- Do not guess missing target facts from naming conventions alone; distinguish
  explicitly preserved facts from inferred ones.
