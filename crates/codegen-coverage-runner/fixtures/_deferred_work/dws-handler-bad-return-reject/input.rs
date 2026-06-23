#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// C06 (built): a `#[rseq::work_handler]` whose signature is not `fn(&mut self)`
// returning `()` — here it returns `u32`. A deferred-work handler runs in thread
// context and returns nothing (GRILL D12/G2). Rejected with
// `DWS-ERR-HANDLER-SIGNATURE`.
include!("../../_lifecycle_surface.rs");
include!("../../_deferred_work_surface.rs");

#[rseq::driver(name = "dwbadret", compatible = ["regseq,dwbadret"])]
pub struct DwBadRet {
    work: Work,
}

impl Lifecycle for DwBadRet {}

impl DwBadRet {
    #[rseq::work_handler]
    fn reset_task(&mut self) -> u32 {
        0
    }
}

pub fn make() -> DwBadRet {
    loop {}
}
