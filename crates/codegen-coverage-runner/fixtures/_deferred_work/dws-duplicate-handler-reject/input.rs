#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// C06 (built): two `#[rseq::work_handler]` methods bound to the SAME (single)
// `Work` — a `Work` has exactly one handler (GRILL D12). This fires with ONE
// `Work` resource (the >=2-`Work` ambiguity is the design-in BARE-WH-AMBIGUOUS,
// NOT built). Rejected with `DWS-ERR-DUPLICATE-HANDLER`.
include!("../../_lifecycle_surface.rs");
include!("../../_deferred_work_surface.rs");

#[rseq::driver(name = "dwdup", compatible = ["regseq,dwdup"])]
pub struct DwDup {
    work: Work,
}

impl Lifecycle for DwDup {}

impl DwDup {
    #[rseq::work_handler]
    fn reset_task(&mut self) {}

    #[rseq::work_handler]
    fn other_task(&mut self) {}
}

pub fn make() -> DwDup {
    loop {}
}
