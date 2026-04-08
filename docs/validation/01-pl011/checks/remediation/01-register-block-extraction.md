# Remediation Checklist: Register Block Extraction

Source check:
- `docs/validation/01-pl011/checks/01-register-block-extraction.md`

Reviewed certificate:
- `docs/validation/01-pl011/checks/results/01-register-block-extraction.md`

## Current Status

- Passing: checks `1`, `2`, `5`, `7`
- Failing: checks `3`, `4`, `6`, `8`

## Extraction Gaps To Fix

- Preserve the full authored register-block property set, not just the two
  properties that currently fail in PL011.
- At minimum, the current PL011 block must preserve:
  - `endian = little`
  - `access_width = 32`
- Preserve reserved-region extents, not just reserved field names.
- Preserve per-register metadata at the block boundary:
  - register field name
  - register type name
  - byte offset
  - access kind
- Preserve the exact authored property set for the register block so the block
  contract can be reconstructed without guessing.

## Likely Touchpoints

- Register-block extraction in `regseq-driver`
- Register-field declaration structures used by the register access surface
- Identity/canonical-path enrichment only if additional block-level metadata is
  currently routed there

## Minimal Facts To Preserve

- Register block property bag that preserves all authored block-level
  properties as explicit key/value facts
- Reserved field layout metadata sufficient to reconstruct byte extents
- Explicit register declaration metadata for every non-reserved field:
  - field name
  - descriptor/type name
  - offset
  - access kind

## Acceptance Checklist

- [ ] `check 3` passes with explicit preservation of the full authored
      register-block property set
- [ ] `check 4` passes with reserved-region extents reconstructable from the
      extraction surface
- [ ] `check 6` passes with explicit register type/offset/access metadata
- [ ] `check 8` passes with the exact authored block property set preserved
- [ ] `source-to-extraction-01` rerun remains green with no weakened
      correspondence for the above facts
