#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

include!("../../_lifecycle_surface.rs");
include!("../../_transport_surface.rs");

#[rseq::driver(name = "dctdiscard", compatible = ["regseq,dct-discard"])]
pub struct DctDiscard;

impl Lifecycle for DctDiscard {
    fn on_init(&mut self) -> Result<(), RegseqError> {
        let _ = submit(1, 0x11);
        Ok(())
    }
}
