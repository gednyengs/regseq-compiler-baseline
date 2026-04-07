# PL011 Layer 3: Regseq Portable Model and Lowering

This file validates how PL011 facts move from extracted Rust-facing compiler
surfaces into Regseq-owned model and lowering layers.

This includes:

- declaration/joined model
- item hierarchy
- semantic bodies
- `SemanticModel`
- `DriverLoweringPlan`

## Status

- `not started`

## Review Template

### Upstream facts

- facts visible from [02-hir-mir.md](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/docs/validation/01-pl011/02-hir-mir.md)

### Downstream representation

- Regseq model and lowering artifacts

### Preserved

- identity
- ownership
- typed locals
- option carriers
- enum variants
- external referenced items
- register/resource operations
- fallible sequencing

### Transformed

- facts transformed but still explicitly represented

### Lost or invented

- facts silently dropped
- backend-relevant residue left unresolved

### Evidence

- model/lowering files
- dumps/tests
- canonical emitted plan facts

### Findings

- link ids from [findings.md](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/docs/validation/01-pl011/findings.md)
