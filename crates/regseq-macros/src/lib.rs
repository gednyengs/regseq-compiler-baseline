//! Procedural macro crate for Regseq helper macros.
//!
//! Provides `register_bitfields!` for register layouts and any future helper
//! macros needed by driver specifications.
//!
//! `#[rseq::...]` tool attributes are **not** provided by this crate. They are
//! an inert custom tool-attribute namespace registered by the compiler wrapper
//! (`regseq-driver`), so the attributes remain available to HIR extraction
//! without requiring proc-macro expansion.
