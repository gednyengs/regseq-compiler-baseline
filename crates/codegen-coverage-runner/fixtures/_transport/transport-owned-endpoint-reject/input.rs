#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

include!("../../_lifecycle_surface.rs");

pub enum SubmissionOutcome {
    Accepted,
    Rejected(isize),
}

pub struct CompilerOwnedEndpoint {
    channel: usize,
}

#[inline(never)]
#[rseq::intrinsic("command_transport_submit")]
pub fn submit(endpoint: CompilerOwnedEndpoint, payload: usize) -> SubmissionOutcome {
    let _ = (endpoint.channel, payload);
    SubmissionOutcome::Accepted
}

#[rseq::driver(name = "dctendpoint", compatible = ["regseq,dct-endpoint"])]
pub struct DctEndpoint;

impl Lifecycle for DctEndpoint {
    fn on_init(&mut self) -> Result<(), RegseqError> {
        match submit(CompilerOwnedEndpoint { channel: 1 }, 0x11) {
            SubmissionOutcome::Accepted => Ok(()),
            SubmissionOutcome::Rejected(_) => Err(RegseqError::HardwareError),
        }
    }
}
