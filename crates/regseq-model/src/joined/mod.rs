//! Joined IR: combined Layer 1 declaration + Layer 2 binding artifact.
//!
//! This module defines the joined-artifact types and the join logic that
//! combines normalized declaration IR with normalized binding IR. The
//! joined artifact makes explicit what joined, what remained
//! declaration-only, and what remained binding-only.
//!
//! The joined artifact is a separate IR family from declaration and
//! binding. It preserves provenance back to both sides and does not
//! introduce backend/lowering structure.

pub mod artifact;
pub mod join;

// Re-export top-level types for convenience.
pub use artifact::{
    ClockJoinEntry, InterruptJoinEntry, JoinOutcome, JoinedArtifact, JoinedHeader, MmioJoinEntry,
    PinCtrlJoinEntry, PropertyJoinEntry, ResetJoinEntry,
};
pub use join::{perform_join, JoinError};
