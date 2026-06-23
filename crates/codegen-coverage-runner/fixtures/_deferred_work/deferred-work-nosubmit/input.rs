#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// The deferred-work-item-scheduling T-nosubmit distinguisher (PLAN T16): a
// required, type-recognized `Work` field with a bound `#[rseq::work_handler]`
// but NO `self.work.submit()` anywhere. This proves the compiler-emitted
// `init_work` (preamble) / `cancel_work` (postamble) lifecycle is RESOURCE-gated,
// NOT submit-gated (GRILL D07/D13): both still emit even though the author never
// schedules the work. Because nothing is submitted, the harness drain + the
// post-cancel re-arm probe are skipped (they model *running queued work*) — so
// the `WRK` trace is `init … cancel` with no `submit`/`drain`/`dispatch`. Without
// this fixture the reset golden cannot distinguish a correct resource-gated build
// from a wrong intrinsic-gated one.
include!("../../_lifecycle_surface.rs");
include!("../../_deferred_work_surface.rs");

#[rseq::driver(name = "dwnosub", compatible = ["regseq,dwnosub"])]
pub struct DwNoSub {
    // The typed deferred-work resource (recognized BY TYPE). No field attribute.
    work: Work,
    // A scalar user-data flag the (never-dispatched) handler would set — present
    // only to mirror the reset fixture's shape; it stays 0 in this scenario.
    done: u32,
}

impl Lifecycle for DwNoSub {
    fn on_init(&mut self) -> Result<(), RegseqError> {
        // Declares + binds the `Work`, but never schedules it (no `submit`).
        Ok(())
    }
}

impl DwNoSub {
    // The bound handler still exists (a required `Work` MUST have one, GRILL G5),
    // but is never dispatched because nothing submits it.
    #[rseq::work_handler]
    fn reset_task(&mut self) {
        self.done = 1;
    }
}
