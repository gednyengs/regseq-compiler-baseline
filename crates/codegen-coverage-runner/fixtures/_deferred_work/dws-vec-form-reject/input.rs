#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// C06 (built): `Vec<Work>` — the dynamic-collection form. Unlike clock/interrupt
// (which only handle `Option`/`[N]`/`&`), this is the extra `Adt` case the
// deferred-work recognizer adds (GRILL T04). Recognized whole and rejected:
// dynamic multi-resource is design-in (GRILL D01). Rejected with
// `DWS-ERR-UNSUPPORTED-FORM`.
extern crate alloc;
use alloc::vec::Vec;

include!("../../_lifecycle_surface.rs");
include!("../../_deferred_work_surface.rs");

#[rseq::driver(name = "dwvec", compatible = ["regseq,dwvec"])]
pub struct DwVec {
    work: Vec<Work>,
}

impl Lifecycle for DwVec {}

pub fn make() -> DwVec {
    loop {}
}
