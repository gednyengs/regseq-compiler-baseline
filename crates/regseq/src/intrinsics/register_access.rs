/// Canonical register-access intrinsics.
///
/// These are compiler-known operations under
/// `regseq::intrinsics::register_access`. The compiler recognizes them by
/// path and lowers them to backend-specific implementations.
use crate::{Duration, RegseqError};

/// Poll until a condition becomes true, checking at the given delay interval
/// up to the given timeout.
///
/// Returns `Ok(())` when the condition is met, or `Err(RegseqError::Timeout)`
/// if the timeout elapses first.
#[inline(never)]
#[rseq::intrinsic]
pub fn poll_until<C>(_condition: C, _delay: Duration, _timeout: Duration) -> Result<(), RegseqError>
where
    C: FnMut() -> bool,
{
    unimplemented!("facade stub: lowered by the compiler backend")
}
