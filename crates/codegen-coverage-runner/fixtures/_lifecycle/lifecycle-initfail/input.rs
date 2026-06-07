#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// The shared device-lifecycle author surface (the `Lifecycle` trait + an inline
// `RegseqError`). The harness verbs are gone (RD-1): the backend synthesizes the
// driving `main` from the model.
include!("../../_lifecycle_surface.rs");

// M3 init-failure driver: fieldless (ZST), one devicetree match. `on_init`
// returns `Err` so the synthesized `init-fail` scenario takes the rollback path;
// `on_init_failure` is OVERRIDDEN so the golden can assert the author body is
// actually invoked (stdout alone cannot distinguish the always-on default-no-op
// marker, CR-3, from a real author call).
#[rseq::driver(name = "fakefail", compatible = ["acme,fakefail"])]
pub struct FakeFail;

impl Lifecycle for FakeFail {
    fn on_init(&mut self) -> Result<(), RegseqError> {
        Err(RegseqError::HardwareError)
    }
    fn on_init_failure(&mut self) -> Result<(), RegseqError> {
        Ok(())
    }
}

// No `regseq_main`: the generic-c backend synthesizes `main` (RD-4/RD-6). The
// manifest's `lifecycle_scenario = "init-fail"` selects the rollback scenario:
// register -> create -> associate -> `LC call on_init` -> (on_init Errs)
// `LC on_init -> Err` -> `LC call on_init_failure` + a call to
// `FakeFail::on_init_failure`. Fieldless, so no partial postamble to roll back.
