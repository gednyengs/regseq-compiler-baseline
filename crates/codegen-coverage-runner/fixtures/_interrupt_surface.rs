// Device-interrupt AUTHOR surface — shared across the `irq-*` fixtures.
//
// Just the author-facing pieces: the typed `Interrupt` resource shell and the
// `IrqResult` dispatch-result enum. Declared **inline per fixture** (the
// CR-2 discipline the lifecycle surface established): identity is the
// attribute marker — `#[rseq::framework_type("irq-result")]` on the enum;
// the `Interrupt` shell is detected by type name, matching the name-based
// `Mmio` anchor (its `#[rseq::abstraction("interrupt")]` tag is
// documentation; recognition keys are the attribute strings — GRILL O4). To
// avoid per-fixture drift it lives in this single file and each fixture pulls
// it in with
//   include!("../_interrupt_surface.rs");
// The including `input.rs` owns the crate attrs (`#![feature(register_tool)]`,
// `#![register_tool(rseq)]`).

// -- Interrupt -----------------------------------------------------------------
// The typed interrupt-line resource shell. It carries no driver-callable
// methods (I02): interrupt operations — register/enable/disable/deregister —
// are compiler-managed lifecycle placement, never author calls. The driver
// declares the field with `#[rseq::interrupt(id = "…")]`; the `id` joins the
// platform file's `[interrupt.<id>]` section (C03).
#[rseq::abstraction("interrupt")]
pub struct Interrupt {
    _private: (),
}

// -- IrqResult -----------------------------------------------------------------
// Dispatch result for bound IRQ handlers (C04): a dispatch result, NOT an
// error type — handlers never return `Result<_, RegseqError>`. `DeferWork` is
// recognized whole but emit-stage rejected this slice (R09: names the
// deferred-work-item-scheduling construct). NO `Debug` derive: the derived
// `fmt` method's `Formatter<'_>` carries `dyn fmt::Write` with a late-bound
// lifetime, which the bridge's fn-sig translation cannot layout-query
// (mirrors `RegseqError`'s no-`Display` rule above).
#[rseq::framework_type("irq-result")]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum IrqResult {
    Handled,
    NotHandled,
    DeferWork,
}
