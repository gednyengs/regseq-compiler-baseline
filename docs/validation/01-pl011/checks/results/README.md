# Results

This directory stores one stable validation report per check.

Naming convention:

- `<check-name>.md`

Example:

- `01-register-block-extraction.md`

Workflow:

- the check file under `docs/validation/01-pl011/checks/` defines the contract
- the harness task executes that contract
- the detailed validation certificate is written here
- the harness task status and failure notes summarize the outcome and should
  reference the report file when relevant
