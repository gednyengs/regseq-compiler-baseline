// Locked device-command-transport author surface. L2 supplies both opaque
// words; the compiler retains only this immediate admission operation.

pub enum SubmissionOutcome {
    Accepted,
    Rejected(isize),
}

#[inline(never)]
#[rseq::intrinsic("command_transport_submit")]
pub fn submit(resolved_endpoint: usize, payload: usize) -> SubmissionOutcome {
    let _ = payload;
    if resolved_endpoint == 0 {
        SubmissionOutcome::Rejected(-16)
    } else {
        SubmissionOutcome::Accepted
    }
}
