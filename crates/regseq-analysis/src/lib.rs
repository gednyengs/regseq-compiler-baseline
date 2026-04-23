//! Static analysis pass infrastructure for the Regseq compiler.
//!
//! Hosts reviewer-facing readiness diagnostics computed directly
//! against [`regseq_model::RegseqModel`] plus a
//! [`regseq_model::verify::VerifyReport`].
//!
//! The former `legacy_input` shim was removed in Wave A of the
//! legacy-removal refactor; see
//! `docs/discussions/models/regseq-model/legacy-removal-plan.md`.

pub mod readiness;
