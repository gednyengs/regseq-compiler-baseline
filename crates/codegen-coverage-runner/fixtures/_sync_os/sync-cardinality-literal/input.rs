#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// NEGATIVE (Codex-M2 r3 should-fix): a NON-identifier `cardinality` value (a
// literal) must reject (UnknownCardinality), not silently default to SingleWaiter.

include!("../../_lifecycle_surface.rs");
include!("../../_sync_surface.rs");

#[rseq::driver(name = "sync-cardlit", bus = "amba",
    amba = [{ id = 0x00099006, mask = 0x000fffff }],
    compatible = ["acme,sync-cardlit"])]
pub struct SyncCardLit {
    #[rseq::sync(cardinality = 0)]
    pub done: Completion,
}

impl Lifecycle for SyncCardLit {
    fn on_init(&mut self) -> Result<(), RegseqError> {
        self.done.signal();
        Ok(())
    }
}
