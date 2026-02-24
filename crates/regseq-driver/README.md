# regseq-driver

The rustc wrapper that drives the Regseq compilation pipeline.

## Responsibilities

- Implement `rustc_driver::Callbacks` to intercept compilation
- Two execution modes:
  - **Regseq mode**: when compiling a Regseq crate (has `regseq` dependency), run
    the full pipeline: HIR attribute extraction, MIR analysis, C code generation
  - **Passthrough mode**: when compiling build scripts, proc-macro crates, or
    non-Regseq dependencies, delegate to standard rustc
- Extract declarations from HIR: register blocks, driver struct, `#[rseq::*]`
  attributes, trait implementations
- Extract function bodies from MIR (post-drop-elaboration) for C code generation
- Orchestrate analysis passes (via `regseq-analysis`)
- Orchestrate C code generation (via `regseq-backends`)

## Binary

This crate produces the `regseq-driver` binary. It is invoked by `cargo-regseq`
via `RUSTC_WRAPPER`.

## Dependencies

- `rustc_driver`, `rustc_interface`, `rustc_hir`, `rustc_middle` (nightly rustc internals)
- `regseq-config` for shared configuration types
- `regseq-analysis` for static analysis passes
- `regseq-backends` for C code generation
- `regseq-diagnostics` for error reporting
