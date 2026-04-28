# regseq-backends

Backend registry and C code generation for target OSes.

## Responsibilities

- Backend trait definition and registry
- Linux backend: generate Linux kernel module C code
  - Structural: module boilerplate, struct definitions, registration tables,
    function signatures, lifecycle preamble/postamble
  - Function bodies: MIR-based C code generation for driver callbacks
- Zephyr backend: generate Zephyr driver C code
  - Structural: Devicetree instantiation macros, struct definitions, API tables
  - Function bodies: MIR-based C code generation for driver callbacks

## Dependencies

- `regseq-config` for shared types
- `regseq-diagnostics` for error reporting
- rustc MIR types (for function body code generation)
