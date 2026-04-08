# Remediation Checklist: Driver Constants Extraction

Source check:
- `docs/validation/01-pl011/checks/06-driver-constants-extraction.md`

Reviewed certificate:
- `docs/validation/01-pl011/checks/results/06-driver-constants-extraction.md`

## Current Status

- Passing:
  - names
  - types
  - canonical paths
  - owner paths
  - closed-world membership
  - MIR named references / distinct entry survival
- Failing:
  - evaluated literal-value preservation
  - definitional relationship preservation

## Extraction Gaps To Fix

- Constant extraction does not preserve evaluated literal values in a form that
  the boundary can compare directly against the authored-source contract.
- Constant extraction does not preserve the definition-level relationship:
  - `EXPLICIT_ISR_CLEAR_MASK = Self::ERROR_INTERRUPT_MASK`

## Likely Touchpoints

- Constant extraction / identity enrichment for associated constants
- HIR- or body-level extraction for constant initializers
- Any path that currently preserves only constant names/types but not
  initializer structure

## Minimal Facts To Preserve

- Exact evaluated value for each driver constant
- Definition-expression shape for constants that are authored in terms of other
  constants

## Acceptance Checklist

- [ ] literal value checks pass for all six constants
- [ ] `EXPLICIT_ISR_CLEAR_MASK` preserves its authored dependency on
      `ERROR_INTERRUPT_MASK`
- [ ] `source-to-extraction-06` rerun passes the two currently failing checks
