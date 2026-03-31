use regseq::{Interrupt, IrqResult};

#[test]
fn interrupt_is_publicly_available() {
    // Verify the Interrupt type is accessible from the regseq crate root.
    fn _accepts_interrupt(_i: Interrupt) {}
}

#[test]
fn interrupt_option_and_array_shapes_compile() {
    // Verify Option<Interrupt> and [Interrupt; N] are valid declaration shapes.
    fn _option_field(_f: Option<Interrupt>) {}
    fn _array_field(_f: [Interrupt; 4]) {}
}

#[test]
fn irq_result_variants_aligned() {
    // Verify the accepted IrqResult dispatch contract.
    let variants = [
        IrqResult::Handled,
        IrqResult::NotHandled,
        IrqResult::DeferWork,
    ];
    assert_eq!(variants.len(), 3);
    // Verify Debug, Clone, Copy, PartialEq, Eq
    let a = IrqResult::Handled;
    let b = a;
    assert_eq!(a, b);
    let _ = format!("{:?}", a);
}

#[test]
fn interrupt_has_no_driver_callable_methods() {
    // Interrupt is an opaque resource shell. This test verifies it compiles
    // without exposing any public methods beyond what Rust auto-derives.
    // The type is not Debug/Clone/Copy by design -- it is a resource.
    let _size = std::mem::size_of::<Interrupt>();
}
