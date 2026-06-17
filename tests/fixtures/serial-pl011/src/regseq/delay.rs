/// Accepted compatibility surface for timed-delay intrinsics.
///
/// Driver authors use `regseq::delay::busy_wait` and `regseq::delay::sleep`
/// in their specs. These re-export the canonical intrinsic home under
/// `regseq::intrinsics::timed_delay`.
pub use crate::regseq::intrinsics::timed_delay::busy_wait;
pub use crate::regseq::intrinsics::timed_delay::sleep;
