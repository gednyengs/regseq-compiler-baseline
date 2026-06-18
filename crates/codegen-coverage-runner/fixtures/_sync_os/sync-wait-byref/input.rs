#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// NEGATIVE (Codex-M2 re-review blocking): the wait `Result` is consumed by a
// BORROW (`.is_ok()` takes `&self`), not a direct move/discriminant. The taint
// pass in `wait_result_consumed` must still catch it and reject.

include!("../../_lifecycle_surface.rs");
include!("../../_sync_surface.rs");

#[rseq::driver(name = "sync-wbr", bus = "amba",
    amba = [{ id = 0x00099004, mask = 0x000fffff }],
    compatible = ["acme,sync-wbr"])]
pub struct SyncWbr {
    #[rseq::sync(cardinality = SingleWaiter)]
    pub done: Completion,
}

impl Lifecycle for SyncWbr {
    fn on_init(&mut self) -> Result<(), RegseqError> {
        let r = self.done.wait(Duration::from_ms(5));
        if r.is_ok() {
            // branch on the timeout outcome through a borrow of `r`
        }
        Ok(())
    }
}
