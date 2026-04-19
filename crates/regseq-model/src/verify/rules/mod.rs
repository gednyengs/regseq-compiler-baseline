//! Verifier rule buckets, grouped by [`VerifierLevel`].
//!
//! Task 55 implements [`module`] and [`cross_item`]. Task 56 fills in
//! [`driver_struct`] and [`body`] plus the two Phase-2 specific
//! buckets [`phase_2_boundary`] and [`phase_2_cross_item`].
//!
//! [`VerifierLevel`]: super::VerifierLevel

pub mod body;
pub mod cross_item;
pub mod driver_struct;
pub mod module;
pub mod phase_2_boundary;
pub mod phase_2_cross_item;
