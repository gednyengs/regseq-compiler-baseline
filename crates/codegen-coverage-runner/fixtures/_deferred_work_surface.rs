// Deferred-work-item-scheduling AUTHOR surface — shared across the
// `_deferred_work/*` fixtures. Just the typed `Work` resource facade and its
// `submit()` intrinsic. The `#[rseq::work_handler]` attribute needs NO facade
// code — it is an inert `rseq` tool attribute (the including crate declares
// `#![register_tool(rseq)]`), recognized by the bridge's Phase-5 handler pass.
//
// Declared **inline per fixture** (the CR-2 discipline the lifecycle / clock
// surfaces established): identity is the attribute marker, NOT a facade crate.
// Each `_deferred_work/*` fixture pulls this in alongside
// `_lifecycle_surface.rs` (which provides `RegseqError` + `Lifecycle`) with
//   include!("../../_deferred_work_surface.rs");
// The including `input.rs` owns the crate attrs (`#![no_std]`,
// `#![feature(register_tool)]`, `#![register_tool(rseq)]`).

// ---------------------------------------------------------------------------
// The typed `Work` resource facade (deferred-work-item-scheduling construct).
// `Work` is the SIMPLEST typed resource in the taxonomy (0 L1 / 0 L2 properties)
// and the only one recognized BY TYPE: its `#[rseq::abstraction("deferred_work")]`
// marker on THIS item is the recognition authority (NOT the type name, NOT a
// field attribute — GRILL D04), so an alias / re-export of `Work` recognizes
// while an unmarked local `struct Work` does not. The author declares a
// `work: Work` field, binds a `#[rseq::work_handler] fn(&mut self)`, and
// schedules it with `self.work.submit()`. The compiler-managed `init_work`/
// `cancel_work` are synthesized into the lifecycle preamble/postamble, never
// author-called.
// ---------------------------------------------------------------------------
#[rseq::abstraction("deferred_work")]
pub struct Work {
    _opaque: (),
}

impl Work {
    /// Schedule the bound handler to run later in sleepable thread context.
    /// Infallible at the surface (`-> ()`).
    #[inline(never)]
    #[rseq::intrinsic("deferred_work_submit")]
    pub fn submit(&self) {}
}
