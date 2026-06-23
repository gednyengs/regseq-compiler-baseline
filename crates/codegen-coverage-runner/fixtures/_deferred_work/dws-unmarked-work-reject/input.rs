#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// T14 ADT-DefId recognition PROBE — unmarked-Work half (PLAN T14, the negative
// twin of `deferred-work-alias`): a driver field whose type is a LOCAL struct
// merely NAMED `Work` but WITHOUT the `#[rseq::abstraction("deferred_work")]`
// marker. Recognition keys on the marker (the resolved ADT item DefId), NOT on the
// type NAME (GRILL D04) — so this `Work` is NOT recognized as a deferred-work
// resource. Because deferred-work recognition does NOT claim the field, it falls
// through to device-lifecycle USER-DATA classification, where a non-scalar struct
// is rejected ("field `work` of type `Work` is not a valid user-data field"). That
// rejection IS the proof recognition is marker-based, not name-based: a name-based
// recognizer would have claimed `work: Work` and never routed it to user-data.
// (The bare `#[rseq::work_handler]` never gets to raise BARE-WH-ZERO-WORK — the
// user-data validation fires first; this empirical ordering refined the PLAN's
// expectation.)
//
// Deliberately does NOT include `_deferred_work_surface.rs`: this fixture defines
// its OWN unmarked `Work` (including the facade would name-collide AND would make
// the type the marked one).
include!("../../_lifecycle_surface.rs");

// An ordinary user struct that merely happens to be named `Work` (NO marker).
pub struct Work {
    _pad: u32,
}

#[rseq::driver(name = "dwunmarked", compatible = ["regseq,dwunmarked"])]
pub struct DwUnmarked {
    // Typed by the LOCAL unmarked `Work` — not a recognized deferred-work resource.
    work: Work,
    done: u32,
}

impl Lifecycle for DwUnmarked {}

impl DwUnmarked {
    #[rseq::work_handler]
    fn reset_task(&mut self) {
        self.done = 1;
    }
}

pub fn make() -> DwUnmarked {
    loop {}
}
