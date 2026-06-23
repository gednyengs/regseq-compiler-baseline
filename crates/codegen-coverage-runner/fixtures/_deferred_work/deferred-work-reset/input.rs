#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// The deferred-work-item-scheduling M1 vertical-slice fixture: the smallest
// valid spec — a DT-matched `#[rseq::driver]` with ONE typed `Work` field, a
// bound `#[rseq::work_handler] fn(&mut self)` (the E1000 `reset_task` witness
// shape), and an `on_init` that schedules it with `self.work.submit()`. The
// compiler type-recognizes the `Work` field (no field attribute, GRILL D04),
// force-materializes + binds the handler, synthesizes a self-recovering lock-
// free wrapper, and emits `init_work` (preamble) / `cancel_work` (postamble) +
// the `submit` lowering. The synthesized scenario drains after `on_init` — so
// the `WRK` trace oracle is `init -> submit -> drain⇒dispatch -> cancel`.
include!("../../_lifecycle_surface.rs");
include!("../../_deferred_work_surface.rs");

#[rseq::driver(name = "dwreset", compatible = ["regseq,dwreset"])]
pub struct DwReset {
    // The typed deferred-work resource (recognized BY TYPE). No field attribute.
    work: Work,
    // A scalar user-data flag the handler sets, so the deferred dispatch is
    // observable via recovered `&mut self` (C05 item 2).
    done: u32,
}

impl Lifecycle for DwReset {
    fn on_init(&mut self) -> Result<(), RegseqError> {
        // Schedule the deferred reset to run later (sleepable thread context).
        // A direct field-path `submit` (the only form built, GRILL D11/G6).
        self.work.submit();
        Ok(())
    }
}

impl DwReset {
    // The deferred work body (the E1000 `reset_task` witness shape): runs later,
    // in sleepable thread context, with recovered `&mut self`. The bare
    // `#[rseq::work_handler]` binds the sole `Work` (GRILL G6).
    #[rseq::work_handler]
    fn reset_task(&mut self) {
        self.done = 1;
    }
}

// No `regseq_main`: the generic-c backend synthesizes `main` from the model; the
// manifest's `lifecycle_scenario = "orderly"` drives the run, and the scenario
// calls the harness-only `regseq_work_drain()` after `on_init` returns.
