# Remediation Checklist: Driver Struct Extraction

Source check:
- `docs/validation/01-pl011/checks/03-driver-struct-extraction.md`

Reviewed certificate:
- `docs/validation/01-pl011/checks/results/03-driver-struct-extraction.md`

## Current Status

- Passing:
  - driver identity
  - device class
  - `regs` resource
  - both property fields
  - `pending_tx` user field classification
- Failing:
  - `irq` exact Rust type fidelity
  - `clock` exact Rust type fidelity
  - `reset` exact Rust type fidelity
  - `pinctrl` exact Rust type fidelity

## Extraction Gaps To Fix

- Resource extraction preserves shape, ids, requiredness, and classification,
  but does not preserve exact Rust types for:
  - `Interrupt`
  - `Option<Clock>`
  - `Option<ResetLine>`
  - `Option<PinCtrl>`

## Likely Touchpoints

- Driver/resource extraction surfaces in `regseq-driver`
- Resource declaration structs for interrupt, clock, reset, and pinctrl
- Any type-rendering helpers already used by property extraction

## Minimal Facts To Preserve

- Add an explicit rendered type field, or equivalent exact type representation,
  to the affected resource surfaces.
- Keep the currently passing facts intact:
  - field name
  - resource id
  - requiredness
  - classification
  - canonical path reconstruction

## Acceptance Checklist

- [ ] `irq` resource proves exact type `Interrupt`
- [ ] `clock` resource proves exact type `Option<Clock>`
- [ ] `reset` resource proves exact type `Option<ResetLine>`
- [ ] `pinctrl` resource proves exact type `Option<PinCtrl>`
- [ ] `source-to-extraction-03` rerun passes all resource subchecks without
      proxy reasoning from requiredness or shape
