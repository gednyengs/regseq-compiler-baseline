//! Canonical synchronization intrinsics.
//!
//! These are compiler-known operations under `regseq::intrinsics::sync`.
//! The compiler recognizes them by path and lowers them to backend-specific
//! barrier implementations.

/// Memory barrier kind selector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BarrierKind {
    ReadRead,
    ReadWrite,
    WriteRead,
    WriteWrite,
    Full,
}

/// Emit a memory barrier of the given kind.
///
/// The compiler replaces this with the appropriate backend-specific barrier
/// instruction (e.g., `rmb()`, `wmb()`, `mb()` on Linux).
#[inline(never)]
#[rseq::intrinsic]
pub fn memory_barrier(_kind: BarrierKind) {
    unimplemented!("facade stub: lowered by the compiler backend")
}
