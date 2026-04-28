# regseq-analysis

Infrastructure for Regseq static analysis passes.

## Responsibilities

- Analysis pass trait and registry
- Concrete analysis implementations:
  - Register access type enforcement (ReadOnly/WriteOnly/ReadWrite)
  - Staging/trigger completeness verification (every path to trigger has all staging writes)
  - Shadow register inference (hw_updates + modify() usage analysis)
  - Synchronization checks SYN-001 through SYN-010
  - DMA lifecycle verification (bind before use, error check after bind)
  - Error propagation enforcement

## Dependencies

- `regseq-config` for shared types
- `regseq-diagnostics` for error reporting
- rustc MIR types (for control flow analysis)
