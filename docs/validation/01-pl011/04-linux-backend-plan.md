# PL011 Layer 4: Linux Backend Plan

This file validates how the portable Regseq lowering output is realized into
Linux backend planning decisions.

## Status

- `not started`

## Review Template

### Upstream facts

- facts visible from [03-regseq-model.md](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/docs/validation/01-pl011/03-regseq-model.md)

### Downstream representation

- `LinuxBackendPlan`
- uart/callback realization
- probe/remove orchestration
- register/resource realization

### Preserved

- justified Linux realization decisions

### Transformed

- wrappers, stubs, naming, and API adaptation decisions that remain traceable

### Lost or invented

- backend-side semantic reconstruction
- unjustified callback reuse
- plan facts that disappear before emission

### Evidence

- backend plan files
- tests
- plan-to-emission traces

### Findings

- link ids from [findings.md](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/docs/validation/01-pl011/findings.md)
