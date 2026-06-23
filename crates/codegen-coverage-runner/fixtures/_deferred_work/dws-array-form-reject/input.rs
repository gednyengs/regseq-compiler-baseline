#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// C06 (built): `[Work; N]` — the array form is recognized whole and rejected (v1
// builds only the required single `Work` form; arrays are design-in multi-
// resource, GRILL D01/T04). Rejected with `DWS-ERR-UNSUPPORTED-FORM`.
include!("../../_lifecycle_surface.rs");
include!("../../_deferred_work_surface.rs");

#[rseq::driver(name = "dwarr", compatible = ["regseq,dwarr"])]
pub struct DwArr {
    work: [Work; 2],
}

impl Lifecycle for DwArr {}

pub fn make() -> DwArr {
    loop {}
}
