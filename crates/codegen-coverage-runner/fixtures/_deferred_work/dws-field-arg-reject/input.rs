#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// C06 (built, SF-1): a `#[rseq::work_handler(field = "…")]` carrying an explicit
// attribute argument. The bare `#[rseq::work_handler]` is the only built form;
// the explicit `field = "…"` form (multi-resource binding) is design-in, NOT
// built in v1 (GRILL D04/D12 + Out-of-Scope). Rejecting the arg (rather than
// silently treating it as bare) prevents a silent mis-bind of a `field = "typo"`
// to the sole `Work`. Rejected with `DWS-ERR-HANDLER-SIGNATURE`.
include!("../../_lifecycle_surface.rs");
include!("../../_deferred_work_surface.rs");

#[rseq::driver(name = "dwfield", compatible = ["regseq,dwfield"])]
pub struct DwField {
    work: Work,
}

impl Lifecycle for DwField {}

impl DwField {
    #[rseq::work_handler(field = "work")]
    fn reset_task(&mut self) {}
}

pub fn make() -> DwField {
    loop {}
}
