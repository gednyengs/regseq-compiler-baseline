#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// C06 (built): a `#[rseq::work_handler]` whose receiver is `&self`, not the
// required `&mut self` (the handler mutates recovered driver state — GRILL
// D12/I01). Rejected with `DWS-ERR-HANDLER-SIGNATURE`.
include!("../../_lifecycle_surface.rs");
include!("../../_deferred_work_surface.rs");

#[rseq::driver(name = "dwbadrcv", compatible = ["regseq,dwbadrcv"])]
pub struct DwBadRcv {
    work: Work,
}

impl Lifecycle for DwBadRcv {}

impl DwBadRcv {
    #[rseq::work_handler]
    fn reset_task(&self) {}
}

pub fn make() -> DwBadRcv {
    loop {}
}
