# Remediation Checklist: Helper Methods Extraction B

Source check:
- `docs/validation/01-pl011/checks/08-helper-methods-extraction-b.md`

Reviewed certificate:
- `docs/validation/01-pl011/checks/results/08-helper-methods-extraction-b.md`

## Current Status

- Passing:
  - identity/signature/return-type group
  - body checks for:
    - `enable_clock_if_present`
    - `disable_clock_if_present`
    - `toggle_reset_if_present`
    - `apply_default_pins_if_present`
    - `apply_sleep_pins_if_present`
- Failing:
  - `uart_clock_hz`
  - `shutdown_port`
  - `service_rx_fifo`
  - `service_tx_fifo`
  - `clear_explicit_interrupts`

## Extraction Gaps To Fix

- `uart_clock_hz` does not preserve the higher-order combinator structure
  strongly enough to prove the exact authored fallback logic.
- `shutdown_port` does not prove exact helper ordering together with the
  placement of `pending_tx = None`.
- `service_rx_fifo` does not prove path-sensitive loop / return behavior.
- `service_tx_fifo` does not prove exact `Some` / `None` and ready /
  not-ready path separation.
- `clear_explicit_interrupts` does not prove exact conditional write-on-nonzero
  behavior.

## Likely Touchpoints

- MIR/body extraction for higher-order call structure
- CFG/path-sensitive helper extraction
- Assignment extraction for field writes like `pending_tx = None`
- Conditional write / branch extraction for register-clearing helpers

## Minimal Facts To Preserve

- Exact operand and control-flow structure for the five failing helpers
- Enough branch-local evidence to distinguish authored paths, not just call
  presence

## Acceptance Checklist

- [ ] `uart_clock_hz` proves exact `Clock::get_rate` / fallback behavior
- [ ] `shutdown_port` proves exact helper order and `pending_tx = None`
      placement
- [ ] `service_rx_fifo` proves authored loop and return semantics
- [ ] `service_tx_fifo` proves authored path split and option-state behavior
- [ ] `clear_explicit_interrupts` proves exact conditional write semantics
- [ ] `source-to-extraction-08` rerun passes all body subchecks
