#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// NEGATIVE (Codex-M2 should-fix): an unrecognized `cardinality` value must be
// rejected (UnknownCardinality), not silently defaulted to SingleWaiter.

include!("../../_lifecycle_surface.rs");
include!("../../_sync_surface.rs");

#[rseq::driver(name = "sync-card", bus = "amba",
    amba = [{ id = 0x00099005, mask = 0x000fffff }],
    compatible = ["acme,sync-card"])]
pub struct SyncCard {
    #[rseq::sync(cardinality = Bogus)]
    pub done: Completion,
}

impl Lifecycle for SyncCard {
    fn on_init(&mut self) -> Result<(), RegseqError> {
        self.done.signal();
        Ok(())
    }
}
