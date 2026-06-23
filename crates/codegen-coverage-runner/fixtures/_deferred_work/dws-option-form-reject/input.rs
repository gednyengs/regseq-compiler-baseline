#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// C06 (built): `Option<Work>` — the optional form is recognized whole and
// rejected (v1 builds only the required single `Work` form; the optional form is
// design-in, GRILL D01/T04). Rejected with `DWS-ERR-UNSUPPORTED-FORM`.
include!("../../_lifecycle_surface.rs");
include!("../../_deferred_work_surface.rs");

#[rseq::driver(name = "dwopt", compatible = ["regseq,dwopt"])]
pub struct DwOpt {
    work: Option<Work>,
}

impl Lifecycle for DwOpt {}

pub fn make() -> DwOpt {
    loop {}
}
