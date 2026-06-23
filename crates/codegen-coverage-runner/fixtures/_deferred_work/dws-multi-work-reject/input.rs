#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// C06 (built, SF-2): TWO `Work` fields with one bare `#[rseq::work_handler]`. v1
// does NOT build multi-resource: the design-in `DWS-ERR-BARE-WH-AMBIGUOUS` is NOT
// added (GRILL D12). Instead, multi-`Work` is HARD-REJECTED via the bare-handler
// discipline — the bare handler binds the sole/first `Work`, leaving the second
// required `Work` unhandled, which is `DWS-ERR-REQUIRED-WORK-NO-HANDLER`. (Two
// handlers would instead collide on the first slot → DWS-ERR-DUPLICATE-HANDLER;
// either way multi-`Work` never silently mis-binds.)
include!("../../_lifecycle_surface.rs");
include!("../../_deferred_work_surface.rs");

#[rseq::driver(name = "dwmulti", compatible = ["regseq,dwmulti"])]
pub struct DwMulti {
    work_a: Work,
    work_b: Work,
}

impl Lifecycle for DwMulti {}

impl DwMulti {
    #[rseq::work_handler]
    fn reset_task(&mut self) {}
}

pub fn make() -> DwMulti {
    loop {}
}
