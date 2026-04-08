# Remediation Checklist: Utility Functions Extraction

Source check:
- `docs/validation/01-pl011/checks/04-utility-functions-extraction.md`

Reviewed certificate:
- `docs/validation/01-pl011/checks/results/04-utility-functions-extraction.md`

## Current Status

- Passing:
  - function identity
  - canonical-path reconstruction
- Failing:
  - generics / parameter-list / return-type richness
  - free-function body extraction

## Extraction Gaps To Fix

- Free helper functions are not extracted with enough signature richness to
  prove the authored parameter and return-type contract exactly.
- Free helper function bodies are not extracted strongly enough to prove the
  authored body-level facts.

## Likely Touchpoints

- Free-function extraction in the body surface
- Function-signature extraction for non-method functions
- Any path that currently classifies only inherent methods and trait callbacks

## Minimal Facts To Preserve

- Exact parameter list for:
  - `field_val`
  - `raw_reg_val`
  - `field_from_raw`
- Exact return-type rendering for all three helpers
- A body surface for free functions that can prove:
  - callee identity
  - argument flow
  - arithmetic / masking / shifting structure

## Acceptance Checklist

- [ ] signature check passes with exact generics, parameters, and return types
- [ ] body check passes for all three utilities without relying on source-only
      reasoning
- [ ] `source-to-extraction-04` rerun passes both failing groups
