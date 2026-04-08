# Remediation Checklist: Serial Callbacks Extraction B

Source check:
- `docs/validation/01-pl011/checks/11-serial-callbacks-extraction-b.md`

Reviewed certificate:
- `docs/validation/01-pl011/checks/results/11-serial-callbacks-extraction-b.md`

## Current Status

- Implemented via `extraction-remediation-11`
- The corresponding validation task `source-to-extraction-11` is complete

## Resolution Summary

- The prior remediation hypothesis was too pessimistic. The current MIR/body
  extraction already preserves symbolic constant identity strongly enough to
  prove:
  - `tx_ready` uses `FR_REG::TXFF::FIELD`
  - `tx_empty` uses `FR_REG::BUSY::FIELD`
  - `flush_tx_fifo` applies `FEN::DISABLED` then `FEN::ENABLED` in order
  - `send_break` maps `on == true -> BRK::ENABLED` and
    `on == false -> BRK::DISABLED`
- The missing work was in the validation proof and certificate, not in the
  compiler boundary itself.
- The previously passing `flush_rx_fifo` and callback identity/signature facts
  remain preserved.

## Evidence Notes

- `MirConstValue::Unevaluated` already carries constant def-path identity for
  call arguments at the current extraction boundary.
- `source-to-extraction-11` now proves those authored facts directly against
  the existing extraction surface.

## Current Backlog State

- No current compiler remediation work is required for this check.
- If this boundary regresses, first confirm whether the regression is in the
  extraction surface or only in the task-owned proof before opening a new
  remediation task.
