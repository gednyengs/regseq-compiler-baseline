#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// C06 (built): a `#[rseq::intrinsic("deferred_work_submit")]` tag on a method
// whose receiver is NOT the `Work` resource shell — a spoofed submit op (the
// `submit` op exists only on `Work`, GRILL D11, mirror clock N22). The callee-
// signature check rejects with `DWS-ERR-SPOOFED-RECEIVER`.
include!("../../_lifecycle_surface.rs");
include!("../../_deferred_work_surface.rs");

pub struct Bogus {
    _x: u32,
}

impl Bogus {
    #[inline(never)]
    #[rseq::intrinsic("deferred_work_submit")]
    pub fn submit(&self) {}
}

#[rseq::driver(name = "dwspoof", compatible = ["regseq,dwspoof"])]
pub struct DwSpoof {
    bogus: Bogus,
}

impl Lifecycle for DwSpoof {
    fn on_init(&mut self) -> Result<(), RegseqError> {
        self.bogus.submit();
        Ok(())
    }
}

pub fn make() -> DwSpoof {
    loop {}
}
