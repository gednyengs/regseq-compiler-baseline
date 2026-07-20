#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

include!("../../_lifecycle_surface.rs");
include!("../../_transport_surface.rs");

#[rseq::driver(name = "dctadmit", compatible = ["regseq,dct-admit"])]
pub struct DctAdmit {
    accepted: u32,
    rejected: u32,
}

impl Lifecycle for DctAdmit {
    fn on_init(&mut self) -> Result<(), RegseqError> {
        match submit(1, 0x11) {
            SubmissionOutcome::Accepted => self.accepted = 1,
            SubmissionOutcome::Rejected(_) => return Err(RegseqError::HardwareError),
        }
        match submit(0, 0x22) {
            SubmissionOutcome::Accepted => return Err(RegseqError::HardwareError),
            SubmissionOutcome::Rejected(-16) => self.rejected = 1,
            SubmissionOutcome::Rejected(_) => return Err(RegseqError::HardwareError),
        }
        if self.accepted == 1 && self.rejected == 1 {
            Ok(())
        } else {
            Err(RegseqError::HardwareError)
        }
    }
}
