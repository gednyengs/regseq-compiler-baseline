#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// C06 (built, SF-1): a NON-STRING name-value form `#[rseq::work_handler = 123]`.
// `value_str()` only sees string values, so the bare-only check keys on
// `Attribute::is_word()` (true only for `AttrArgs::Empty`) — rejecting EVERY
// non-bare spelling, including non-string `= lit` forms. Like the string
// name-value / list forms, this explicit form is design-in, NOT built in v1
// (GRILL D04/D12), and must not be silently treated as bare. Rejected with
// `DWS-ERR-HANDLER-SIGNATURE`.
include!("../../_lifecycle_surface.rs");
include!("../../_deferred_work_surface.rs");

#[rseq::driver(name = "dwnonstr", compatible = ["regseq,dwnonstr"])]
pub struct DwNonString {
    work: Work,
}

impl Lifecycle for DwNonString {}

impl DwNonString {
    #[rseq::work_handler = 123]
    fn reset_task(&mut self) {}
}

pub fn make() -> DwNonString {
    loop {}
}
