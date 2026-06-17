/// Register-access polling compatibility surface.
///
/// `until` is a compatibility re-export of the canonical intrinsic at
/// `regseq::intrinsics::register_access::poll_until`. New code should use
/// the canonical path directly.
pub use crate::regseq::intrinsics::register_access::poll_until as until;
