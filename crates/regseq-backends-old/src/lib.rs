//! C code generation backend for the Regseq compiler.
//!
//! Provides a backend-neutral C AST (`c/`), a Linux-specific module
//! (`linux/`), and a portable lowering layer (`lowering/`).
//!
//! During the RegseqModel-only rebuild, this crate is idle: no caller
//! in the main pipeline consumes it. The modules that remain are the
//! ADAPT/MIXED survivors of the Wave A audit in
//! `docs/discussions/models/regseq-model/regseq-backends-audit.md`;
//! they will be rewritten to consume `RegseqModel` directly when
//! backends are re-introduced.

pub mod c;
pub mod linux;
pub mod lowering;
