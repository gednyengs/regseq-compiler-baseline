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
#[rseq::framework_type("irq-result")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IrqResult {
    Handled,
    NotHandled,
    DeferWork,
}
