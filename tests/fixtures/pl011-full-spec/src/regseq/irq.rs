/// Typed interrupt resource shell.
///
/// `Interrupt` is an opaque resource type representing a device interrupt line.
/// It carries no driver-callable methods; interrupt operations are managed by
/// the compiler through `#[rseq::*]` attributes and lowered to OS-specific
/// registration and dispatch code.
#[rseq::abstraction("interrupt")]
pub struct Interrupt {
    _private: (),
}

/// Dispatch result for bound IRQ handlers.
///
/// This is a dispatch result, not an error type. Handlers return one of these
/// variants to indicate how the interrupt was handled.
// NO `Debug` derive (device-interrupt T13): the derived `fmt` method's
// `Formatter<'_>` carries `dyn fmt::Write` with a late-bound lifetime, which
// the bridge's fn-sig translation cannot layout-query (the same rule the
// shared `_interrupt_surface.rs` fixture records).
#[rseq::framework_type("irq-result")]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum IrqResult {
    Handled,
    NotHandled,
    DeferWork,
}
