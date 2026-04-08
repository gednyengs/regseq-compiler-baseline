# Remediation Checklist: Helper Methods Extraction A

Source check:
- `docs/validation/01-pl011/checks/07-helper-methods-extraction-a.md`

Reviewed certificate:
- `docs/validation/01-pl011/checks/results/07-helper-methods-extraction-a.md`

## Current Status

- Passing:
  - identity/signature/return-type group
  - body checks for:
    - `disable_uart`
    - `mask_all_interrupts`
    - `clear_all_interrupts`
    - `configure_interrupt_levels`
    - `set_baud_rate`
    - `enable_rx_interrupts`
    - `enable_tx_interrupts`
    - `disable_tx_interrupts`
    - `program_polling_port`
- Failing:
  - `disable_fifo`
  - `configure_line_control`
  - `enable_uart`

## Extraction Gaps To Fix

- `disable_fifo` proof does not yet preserve exact field-local modify semantics
  strongly enough.
- `configure_line_control` does not preserve the authored scalar/materialized
  value strongly enough.
- `enable_uart` does not preserve the authored scalar/materialized value
  strongly enough.

## Likely Touchpoints

- MIR/body extraction for constant and `FieldValue` arguments
- Expression extraction for register modify/write operations
- Any helper that currently records call presence but not exact argument/value
  identity

## Minimal Facts To Preserve

- Exact symbolic or scalar identity for the values used by:
  - `disable_fifo`
  - `configure_line_control`
  - `enable_uart`
- Enough operation detail to prove authored register-field locality, not just
  that a matching helper call exists

## Acceptance Checklist

- [ ] `disable_fifo` proves the exact authored register-field update
- [ ] `configure_line_control` proves the authored literal / combined value
- [ ] `enable_uart` proves the authored literal / combined value
- [ ] `source-to-extraction-07` rerun passes all body subchecks
