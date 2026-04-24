//! Static analysis pass infrastructure for the Regseq compiler.
//!
//! Hosts reviewer-facing readiness diagnostics computed directly
//! against [`regseq_model::RegseqModel`] plus a
//! [`regseq_model::verify::VerifyReport`].

pub mod readiness;
