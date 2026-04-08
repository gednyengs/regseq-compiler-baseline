# PL011 Source-to-Extraction Remediation Checklists

These files translate the validated extraction certificates in
`docs/validation/01-pl011/checks/results/` into implementation-oriented
checklists.

Use these checklists when creating follow-up compiler tasks that improve the
current extraction boundary.

## Conventions

- Each remediation file mirrors one extraction check file by number and topic.
- If the corresponding extraction certificate is all-pass, the remediation file
  records that no implementation work is currently required.
- If the certificate contains failures, the remediation file lists:
  - the current extraction gaps
  - the minimum facts that need to be preserved
  - likely implementation touchpoints
  - acceptance conditions for rerunning the extraction check

## Expected Workflow

1. Read the extraction check contract in `docs/validation/01-pl011/checks/`.
2. Read the reviewed certificate in `docs/validation/01-pl011/checks/results/`.
3. Use the remediation checklist to create or execute a compiler
   implementation task.
4. Rerun the corresponding `source-to-extraction-*` validation task and update
   the certificate.
