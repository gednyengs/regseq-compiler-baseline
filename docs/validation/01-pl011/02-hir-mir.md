# PL011 Layer 2: HIR and MIR

This file validates what authored PL011 source facts remain structurally
visible after Rust lowers the driver into HIR/MIR-like extraction surfaces.

## Status

- `not started`

## Review Template

### Upstream facts

- facts from [01-authored-source.md](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/docs/validation/01-pl011/01-authored-source.md)

### Downstream representation

- HIR/module-level facts
- MIR/body-level facts

### Preserved

- facts that survive structurally

### Transformed

- facts lowered into indices, aggregates, temporaries, discriminants, etc.

### Lost or invented

- facts no longer recoverable without compiler-side reconstruction

### Evidence

- extraction/model files
- MIR/HIR references
- tests

### Findings

- link ids from [findings.md](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/docs/validation/01-pl011/findings.md)
