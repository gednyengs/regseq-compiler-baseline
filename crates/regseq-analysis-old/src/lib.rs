//! Static analysis pass infrastructure for the Regseq compiler.
//!
//! Hosts reviewer-facing readiness diagnostics computed directly
//! against [`regseq_model_old::RegseqModel`] plus a
//! [`regseq_model_old::verify::VerifyReport`].

pub mod readiness;
