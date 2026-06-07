#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// The shared device-lifecycle author surface (the `Lifecycle` trait + an inline
// `RegseqError`). The harness verbs are gone (RD-1): the backend synthesizes the
// driving `main` from the model.
include!("../../_lifecycle_surface.rs");

// M0 walking-skeleton driver: fieldless (ZST), one devicetree match.
#[rseq::driver(name = "fake", compatible = ["acme,fake"])]
pub struct Fake;

// Overrides on_init only; the other four callbacks use the trait defaults.
impl Lifecycle for Fake {
    fn on_init(&mut self) -> Result<(), RegseqError> {
        Ok(())
    }
}

// No `regseq_main`: the generic-c backend synthesizes `main` (RD-4/RD-6) — it
// registers the driver, creates + associates the instance, and calls `on_init`.
// The manifest's `lifecycle_scenario` selects which scenarios the runner drives.
