# PL011 Layer 5: Generated C and Wrapper-Driven Proof

This file validates the final emitted Linux artifacts for the PL011 specimen.

It covers both:

- backend-local emitted artifact review
- canonical wrapper-driven `regseq-driver` proof

## Status

- `not started`

## Review Template

### Upstream facts

- facts visible from [04-linux-backend-plan.md](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/docs/validation/01-pl011/04-linux-backend-plan.md)

### Downstream representation

- generated `.c`, `.h`, `Makefile`, `Kbuild`

### Preserved

- valid C signatures
- helper/callback bodies
- Linux serial-core structures
- plan-driven constants and register helpers

### Transformed

- final C naming and API realization details

### Lost or invented

- raw Rust syntax
- MIR locals/SSA names
- dropped callbacks/helpers
- fake stubs where real supported lowering is required

### Evidence

- emitted files
- e2e test output
- host build proof

### Findings

- link ids from [findings.md](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/docs/validation/01-pl011/findings.md)
