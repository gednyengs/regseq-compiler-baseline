#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// C06 (built): a bare `#[rseq::work_handler]` on a driver that declares NO `Work`
// resource — the bare handler binds the sole `Work`; with zero `Work` there is
// nothing to bind (GRILL D12/G6). This is reachable single-resource (the >=2 case
// is the design-in BARE-WH-AMBIGUOUS). Rejected with `DWS-ERR-BARE-WH-ZERO-WORK`.
include!("../../_lifecycle_surface.rs");
include!("../../_deferred_work_surface.rs");

#[rseq::driver(name = "dwzero", compatible = ["regseq,dwzero"])]
pub struct DwZero {
    done: u32,
}

impl Lifecycle for DwZero {}

impl DwZero {
    #[rseq::work_handler]
    fn reset_task(&mut self) {
        self.done = 1;
    }
}

pub fn make() -> DwZero {
    loop {}
}
