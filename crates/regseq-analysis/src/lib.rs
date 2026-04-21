//! Static analysis pass infrastructure for the Regseq compiler.
//!
//! Hosts reviewer-facing readiness diagnostics computed directly
//! against [`regseq_model::RegseqModel`] plus a
//! [`regseq_model::verify::VerifyReport`], and preserves a
//! transitional [`legacy_input`] shim for callers that have not yet
//! migrated. See the module-level docs on [`legacy_input`] for the
//! shim's retirement target (regseq-lowering-10).

pub mod legacy_input;
pub mod readiness;
