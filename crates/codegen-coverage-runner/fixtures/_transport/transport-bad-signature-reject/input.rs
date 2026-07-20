#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

include!("../../_lifecycle_surface.rs");

pub enum SubmissionOutcome {
    Accepted,
    Rejected(u32),
}

#[inline(never)]
#[rseq::intrinsic("command_transport_submit")]
pub fn submit(resolved_endpoint: usize, payload: usize) -> SubmissionOutcome {
    let _ = (resolved_endpoint, payload);
    SubmissionOutcome::Accepted
}

#[rseq::driver(name = "dctbadsig", compatible = ["regseq,dct-bad-signature"])]
pub struct DctBadSignature;

impl Lifecycle for DctBadSignature {
    fn on_init(&mut self) -> Result<(), RegseqError> {
        match submit(1, 0x11) {
            SubmissionOutcome::Accepted => Ok(()),
            SubmissionOutcome::Rejected(_) => Err(RegseqError::HardwareError),
        }
    }
}
