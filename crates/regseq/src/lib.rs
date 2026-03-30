//! Public DSL facade for Regseq driver authors.
//!
//! Driver specifications depend on this crate to access helper macros such as
//! `register_bitfields!`, resource types, marker traits, and intrinsic stubs.
//!
//! `#[rseq::...]` attributes are compiler-recognized tool attributes registered
//! by the compiler wrapper (`regseq-driver`), not proc macros exported by this
//! facade. The `rseq` attribute namespace is distinct from the `regseq` crate
//! namespace.
