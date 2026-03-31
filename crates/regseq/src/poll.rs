/// Register-access polling intrinsic surface.
///
/// `until` is the accepted free-function intrinsic for polling a register
/// condition. The body is an intrinsic stub; the compiler replaces it with
/// backend-specific poll-loop lowering during code generation.
use crate::{Duration, RegseqError};

/// Poll until a condition becomes true, checking at the given delay interval
/// up to the given timeout.
///
/// Returns `Ok(())` when the condition is met, or `Err(RegseqError::Timeout)`
/// if the timeout elapses first.
pub fn until<C>(_condition: C, _delay: Duration, _timeout: Duration) -> Result<(), RegseqError>
where
    C: FnMut() -> bool,
{
    unimplemented!("facade stub: lowered by the compiler backend")
}
