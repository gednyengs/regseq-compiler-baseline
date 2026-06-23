#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// C06 (built): `&Work` — a borrow, never an owned resource field (v1 builds only
// the required owned single `Work` form; the borrow form is design-in, GRILL
// D01/T04). Rejected with `DWS-ERR-UNSUPPORTED-FORM`.
include!("../../_lifecycle_surface.rs");
include!("../../_deferred_work_surface.rs");

#[rseq::driver(name = "dwref", compatible = ["regseq,dwref"])]
pub struct DwRef {
    work: &'static Work,
}

impl Lifecycle for DwRef {}

pub fn make() -> DwRef {
    loop {}
}
