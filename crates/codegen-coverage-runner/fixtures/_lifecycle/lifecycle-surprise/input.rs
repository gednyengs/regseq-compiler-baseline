#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// The shared device-lifecycle author surface (the `Lifecycle` trait + an inline
// `RegseqError`). The harness verbs are gone (RD-1): the backend synthesizes the
// driving `main` from the model.
include!("../../_lifecycle_surface.rs");

// M4 surprise-removal driver: fieldless (ZST), one devicetree match. Overrides
// `on_surprise_remove` (returns Ok) so the synthesized `surprise` scenario drives
// the device-gone teardown branch and the golden can assert the author body is
// actually invoked (stdout alone cannot distinguish the always-on default-no-op
// marker, CR-3, from a real author call). `on_init` is the trait default.
#[rseq::driver(name = "fakesurprise", compatible = ["acme,fakesurprise"])]
pub struct FakeSurprise;

impl Lifecycle for FakeSurprise {
    fn on_surprise_remove(&mut self) -> Result<(), RegseqError> {
        Ok(())
    }
}

// No `regseq_main`: the generic-c backend synthesizes `main` (RD-4/RD-6). The
// manifest's `lifecycle_scenario = "surprise"` selects the device-gone teardown:
// register -> create -> associate -> `LC call on_init` -> `LC teardown present=0`
// -> `LC call on_surprise_remove` + a call to `FakeSurprise::on_surprise_remove`.
// Byte-distinct from orderly (present=0 vs 1, surprise_remove vs deinit). Fieldless,
// so no `skipped(surprise)` postambles (those land with a resourceful fixture, M7).
