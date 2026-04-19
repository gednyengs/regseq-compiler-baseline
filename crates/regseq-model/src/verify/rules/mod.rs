//! Verifier rule buckets, grouped by [`VerifierLevel`].
//!
//! Task 55 implements [`module`] and [`cross_item`]. Task 56 will fill
//! in [`driver_struct`] and [`body`].
//!
//! [`VerifierLevel`]: super::VerifierLevel

pub mod body;
pub mod cross_item;
pub mod driver_struct;
pub mod module;
