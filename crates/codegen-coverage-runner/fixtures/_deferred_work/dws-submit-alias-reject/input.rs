#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// C06 (built): a `submit` whose receiver is NOT a direct field path to the bound
// `Work` — here a local alias `let w = &self.work; w.submit()`. v1 recognizes
// `Work::submit` ONLY on a direct field path (`self.work.submit()`); local
// aliases / projections are design-in, NOT built (GRILL D11 + Out-of-Scope:
// "submit alias/projection receiver recognition"). So this rejects with
// `DWS-ERR-SPOOFED-RECEIVER` (a submit that does not resolve to a direct,
// bound-`Work` field path).
include!("../../_lifecycle_surface.rs");
include!("../../_deferred_work_surface.rs");

#[rseq::driver(name = "dwalias", compatible = ["regseq,dwalias"])]
pub struct DwAlias {
    work: Work,
    done: u32,
}

impl Lifecycle for DwAlias {
    fn on_init(&mut self) -> Result<(), RegseqError> {
        // A local alias to the `Work` field, then submit through the alias — the
        // design-in alias form, rejected in v1.
        let w = &self.work;
        w.submit();
        Ok(())
    }
}

impl DwAlias {
    #[rseq::work_handler]
    fn reset_task(&mut self) {
        self.done = 1;
    }
}
