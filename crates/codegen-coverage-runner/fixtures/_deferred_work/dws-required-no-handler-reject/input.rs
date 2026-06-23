#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// C06 (built): a required `Work` field with NO bound `#[rseq::work_handler]` —
// a `Work` you can never run is broken (GRILL D10/G5; `required` is operationally
// complete). Rejected with `DWS-ERR-REQUIRED-WORK-NO-HANDLER`.
include!("../../_lifecycle_surface.rs");
include!("../../_deferred_work_surface.rs");

#[rseq::driver(name = "dwnohnd", compatible = ["regseq,dwnohnd"])]
pub struct DwNoHandler {
    work: Work,
}

impl Lifecycle for DwNoHandler {}

pub fn make() -> DwNoHandler {
    loop {}
}
