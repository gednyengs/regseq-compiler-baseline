#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// NEGATIVE fixture (T07-lowering §D4 / Codex-M2 blocking #1): a `Completion::wait`
// whose `Result<(), RegseqError>` is CONSUMED (here via `?`). The first slice
// discards the wait result (the timeout→`Err` outcome is not modeled), so a body
// that branches on it must be rejected loud at recognition rather than emitting C
// that reads the unset destination (I06). The bridge must return
// `SyncDiagnostic::CompletionWaitResultConsumed`.

include!("../../_lifecycle_surface.rs");
include!("../../_sync_surface.rs");

#[rseq::driver(name = "sync-wc", bus = "amba",
    amba = [{ id = 0x00099003, mask = 0x000fffff }],
    compatible = ["acme,sync-wc"])]
pub struct SyncWc {
    #[rseq::sync(cardinality = SingleWaiter)]
    pub done: Completion,
}

impl Lifecycle for SyncWc {
    fn on_init(&mut self) -> Result<(), RegseqError> {
        // `?` reads the wait Result's discriminant — a consuming use.
        self.done.wait(Duration::from_ms(5))?;
        Ok(())
    }
}
