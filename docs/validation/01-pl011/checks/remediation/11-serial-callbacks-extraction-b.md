# Remediation Checklist: Serial Callbacks Extraction B

Source check:
- `docs/validation/01-pl011/checks/11-serial-callbacks-extraction-b.md`

Reviewed certificate:
- `docs/validation/01-pl011/checks/results/11-serial-callbacks-extraction-b.md`

## Current Status

- Passing:
  - identity / trait role / signature / return-type group
- Failing:
  - body group

## Extraction Gaps To Fix

- `tx_ready` does not preserve field-level identity strongly enough to prove
  `FR_REG::TXFF::FIELD`.
- `tx_empty` does not preserve field-level identity strongly enough to prove
  `FR_REG::BUSY::FIELD`.
- `flush_tx_fifo` does not preserve symbolic value ordering strongly enough to
  prove `FEN::DISABLED` then `FEN::ENABLED`.
- `send_break` does not preserve branch-to-value mapping strongly enough to
  prove `on == true -> BRK::ENABLED` and `on == false -> BRK::DISABLED`.

## Likely Touchpoints

- MIR/body extraction for call-argument symbolic identity
- Any intermediate representation that can preserve field and `FieldValue`
  constant paths before lowering to scalars

## Minimal Facts To Preserve

- Symbolic path of `Field` constants passed to `read`
- Symbolic path of `FieldValue` constants passed to `modify`
- Branch-local association between `send_break` control-flow arms and the
  authored `BRK` values

## Acceptance Checklist

- [ ] `tx_ready` proves exact `FR_REG::TXFF::FIELD`
- [ ] `tx_empty` proves exact `FR_REG::BUSY::FIELD`
- [ ] `flush_tx_fifo` proves exact ordered `FEN::DISABLED` then
      `FEN::ENABLED`
- [ ] `send_break` proves exact branch-to-`BRK` mapping
- [ ] `source-to-extraction-11` body group upgrades from `fail` to `pass`
