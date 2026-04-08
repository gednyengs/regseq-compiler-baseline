# Remediation Checklist: Serial Callbacks Extraction A

Source check:
- `docs/validation/01-pl011/checks/10-serial-callbacks-extraction-a.md`

Reviewed certificate:
- `docs/validation/01-pl011/checks/results/10-serial-callbacks-extraction-a.md`

## Current Status

- Passing:
  - identity / trait role / signature / return-type group
- Failing:
  - body group

## Extraction Gap To Fix

- Most callback body predicates are strongly preserved, but `rx_char` does not
  preserve the exact named mapping:
  - `OE -> OVERRUN`
  - `BE -> BREAK`
  - `PE -> PARITY`
  - `FE -> FRAMING`

MIR currently preserves the branch/count structure and aggregate construction,
but not the named `ErrorKind` identity needed for an exact body pass.

## Likely Touchpoints

- MIR/body extraction for named constants used in `rx_char`
- Any HIR-side semantic layer that can preserve enum/bitflag constant identity
  before MIR lowers them to integer literals

## Minimal Facts To Preserve

- Symbolic identity for the four `ErrorKind` values used in `rx_char`
- Enough argument or expression metadata to tie each DR error flag check to the
  correct named `ErrorKind`

## Acceptance Checklist

- [ ] `rx_char` proves all four authored error-flag to `ErrorKind` pairings
- [ ] `source-to-extraction-10` body group upgrades from `fail` to `pass`
- [ ] no regression in the already-strong proofs for `port_open`, `port_close`,
      `configure`, and `tx_byte`
