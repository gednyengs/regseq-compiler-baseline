#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// T14 ADT-DefId recognition PROBE — alias half (PLAN T14, the genuinely-new
// type-driven recognition path): the deferred-work `Work` field is declared via a
// `type WorkAlias = Work;` alias rather than the bare `Work` name. Recognition
// keys on the field type's RESOLVED ADT item DefId + its
// `#[rseq::abstraction("deferred_work")]` marker (GRILL D04), NOT on the syntactic
// field-type spelling — so the aliased field still recognizes, binds the bare
// handler, and tags the alias-routed `submit`. The negative twin (an unmarked
// local `struct Work` does NOT recognize) is `dws-unmarked-work-reject`.
//
// This is a RECOGNITION PROBE: the dedicated `dws_alias_recognition_e2e` binary
// bridges it once and inspects the model (it has NO golden by design — the emit
// path is identical to the reset keystone, T15).
include!("../../_lifecycle_surface.rs");
include!("../../_deferred_work_surface.rs");

// A transparent alias to the marked facade `Work` — resolves to the SAME ADT item.
type WorkAlias = Work;

#[rseq::driver(name = "dwalias", compatible = ["regseq,dwalias"])]
pub struct DwAlias {
    // Declared via the alias; recognition resolves it to the marked `Work` item.
    work: WorkAlias,
    done: u32,
}

impl Lifecycle for DwAlias {
    fn on_init(&mut self) -> Result<(), RegseqError> {
        // A direct field-path `submit` routed through the aliased field type.
        self.work.submit();
        Ok(())
    }
}

impl DwAlias {
    #[rseq::work_handler]
    fn reset_task(&mut self) {
        self.done = 1;
    }
}
