/// Dispatch result for bound IRQ handlers.
///
/// This is a dispatch result, not an error type. Handlers return one of these
/// variants to indicate how the interrupt was handled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IrqResult {
    Handled,
    NotHandled,
    DeferWork,
}
