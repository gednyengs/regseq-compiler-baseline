# Remediation Checklist: Helper Methods Extraction B

Source check:
- `docs/validation/01-pl011/checks/08-helper-methods-extraction-b.md`

Reviewed certificate:
- `docs/validation/01-pl011/checks/results/08-helper-methods-extraction-b.md`

## Current Status

- Implemented via `extraction-remediation-08`
- The corresponding validation task `source-to-extraction-08` is complete

## Resolution Summary

- Helper-body extraction now preserves exact operand flow, path-sensitive
  control flow, and branch-local effects strongly enough to prove:
  - `uart_clock_hz`
  - `shutdown_port`
  - `service_rx_fifo`
  - `service_tx_fifo`
  - `clear_explicit_interrupts`
- The previously passing helper-methods-B facts remain preserved, including:
  - identity/signature/return-type facts
  - the body checks for:
    - `enable_clock_if_present`
    - `disable_clock_if_present`
    - `toggle_reset_if_present`
    - `apply_default_pins_if_present`
    - `apply_sleep_pins_if_present`

## Evidence Notes

- The current validation certificate records exact proof of:
  - higher-order combinator flow for `uart_clock_hz`
  - helper ordering and `pending_tx = None` placement for `shutdown_port`
  - loop / local / return correspondence for `service_rx_fifo`
  - path-sensitive option and ready/not-ready branch separation for
    `service_tx_fifo`
  - conditional write and dual-use mask/value flow for
    `clear_explicit_interrupts`

## Current Backlog State

- No current remediation work is required for this check.
- If this boundary regresses, reopen remediation only against the reviewed
  certificate and current `source-to-extraction-08` evidence.
