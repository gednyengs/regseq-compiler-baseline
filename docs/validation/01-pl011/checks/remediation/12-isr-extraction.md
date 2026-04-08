# Remediation Checklist: ISR Extraction

Source check:
- `docs/validation/01-pl011/checks/12-isr-extraction.md`

Reviewed certificate:
- `docs/validation/01-pl011/checks/results/12-isr-extraction.md`

## Current Status

- All check groups pass.

## Remediation

- No current extraction remediation required.

## Follow-Up Discipline

- Preserve current CFG-level strength for:
  - early `status == 0` return
  - RX/TX dispatch before clear
  - clear before final handled condition
  - final `Handled` / `NotHandled` mapping
