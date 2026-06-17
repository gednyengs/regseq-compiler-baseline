use crate::regseq::Duration;

/// Busy-wait for the specified duration.
///
/// Canonical intrinsic: the compiler recognizes this call site and lowers
/// it to the target OS busy-wait primitive.
#[inline(never)]
#[rseq::intrinsic("busy_wait")]
pub fn busy_wait(_duration: Duration) {
    unimplemented!("compiler-managed: timed_delay::busy_wait")
}

/// Sleep (yield the CPU) for the specified duration.
///
/// Canonical intrinsic: the compiler recognizes this call site and lowers
/// it to the target OS sleep primitive.
#[inline(never)]
#[rseq::intrinsic("sleep")]
pub fn sleep(_duration: Duration) {
    unimplemented!("compiler-managed: timed_delay::sleep")
}
