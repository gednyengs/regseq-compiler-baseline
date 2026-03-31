/// Canonical intrinsic namespace.
///
/// All function-like Regseq intrinsics have their canonical home under
/// `regseq::intrinsics::{construct}::{intrinsic}`. Other public paths
/// (e.g., `regseq::poll::until`) are compatibility aliases.
pub mod register_access;
pub mod sync;
