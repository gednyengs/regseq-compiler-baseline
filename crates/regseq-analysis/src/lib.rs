//! Static analysis pass infrastructure for the Regseq compiler.
//!
//! Provides an analysis pass trait, a pass registry, and validation passes
//! over rustc-independent models from `regseq-model`. Used to verify
//! construct-level invariants before lowering begins.
