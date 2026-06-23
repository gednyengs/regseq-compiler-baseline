#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// C06 (built, SF-1): the NAME-VALUE explicit form `#[rseq::work_handler = "…"]`.
// Like the list form `(field = "…")`, any explicit argument/value on the handler
// attribute is the design-in form, NOT built in v1 (GRILL D04/D12). The bare
// `#[rseq::work_handler]` is the only built form; the name-value form must NOT be
// silently treated as bare (it would mis-bind the sole `Work`). Rejected with
// `DWS-ERR-HANDLER-SIGNATURE`.
include!("../../_lifecycle_surface.rs");
include!("../../_deferred_work_surface.rs");

#[rseq::driver(name = "dwnv", compatible = ["regseq,dwnv"])]
pub struct DwNameValue {
    work: Work,
}

impl Lifecycle for DwNameValue {}

impl DwNameValue {
    #[rseq::work_handler = "work"]
    fn reset_task(&mut self) {}
}

pub fn make() -> DwNameValue {
    loop {}
}
