# Remediation Checklist: Serial Callbacks Extraction A

Source check:
- `docs/validation/01-pl011/checks/10-serial-callbacks-extraction-a.md`

Reviewed certificate:
- `docs/validation/01-pl011/checks/results/10-serial-callbacks-extraction-a.md`

## Current Status

- Implemented via `extraction-remediation-10`
- The corresponding validation task `source-to-extraction-10` is complete

## Resolution Summary

- Serial-callback extraction now preserves exact symbolic constant identity and
  branch-local correspondence strongly enough to prove all four authored
  `rx_char` error-flag to `ErrorKind` pairings:
  - `OE -> OVERRUN`
  - `BE -> BREAK`
  - `PE -> PARITY`
  - `FE -> FRAMING`
- The previously passing serial-callbacks-A facts remain preserved, including:
  - identity / trait role / signature / return-type facts
  - `port_open`
  - `port_close`
  - `configure`
  - `tx_byte`
  - the already-strong structural facts for `rx_char`

## Evidence Notes

- The current validation certificate records exact proof of the `rx_char`
  mapping by following MIR CFG structure together with symbolic `Unevaluated`
  constant paths for the named `ErrorKind` values.

## Current Backlog State

- No current remediation work is required for this check.
- If this boundary regresses, reopen remediation only against the reviewed
  certificate and current `source-to-extraction-10` evidence.
