#![feature(rustc_private)]

extern crate rustc_driver;
extern crate rustc_hir;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_session;

#[path = "support/regseq_model_harness.rs"]
mod harness;

use harness::{
    body_mentions_constant, build_model_with_method_bodies, compile_and_build, collect_call_names,
    count_switch_int, find_driver_constant, find_driver_impl_method, driver_struct,
};

const SPECIMEN: &str = r#"
#![no_std]

pub struct Interrupt;

pub enum IrqResult {
    Handled,
    NotHandled,
}

#[rseq::driver(name = "pl011-isr", compatible = ["arm,pl011"], bus = "amba")]
pub struct Pl011Isr {
    #[rseq::interrupt(id = "uart_irq")]
    pub irq: Interrupt,
    pub status: u32,
}

impl Pl011Isr {
    const EXPLICIT_ISR_CLEAR_MASK: u32 = 0x0780;

    fn read_status(&self) -> u32 { self.status }
    fn service_rx_fifo(&mut self) -> bool { true }
    fn service_tx_fifo(&mut self) -> bool { false }
    fn clear_explicit_interrupts(&mut self, _status: u32) {}

    #[rseq::isr]
    pub fn handle_irq(&mut self) -> IrqResult {
        let status = self.read_status();
        let mut handled = false;
        if self.service_rx_fifo() { handled = true; }
        if self.service_tx_fifo() { handled = true; }
        if status & Self::EXPLICIT_ISR_CLEAR_MASK != 0 {
            self.clear_explicit_interrupts(status);
            handled = true;
        }

        if handled {
            IrqResult::Handled
        } else {
            IrqResult::NotHandled
        }
    }
}
"#;

#[test]
fn isr_method_and_interrupt_resource_survive_in_model() {
    let model = compile_and_build(SPECIMEN, "source_to_regseq_model_12_identity")
        .expect("specimen must build");

    let driver = driver_struct(&model);
    assert_eq!(driver.interrupts.len(), 1);
    let handle_irq = find_driver_impl_method(&model, "handle_irq");
    assert!(
        handle_irq.attrs.entries.iter().any(|entry| entry.key == "rseq::isr"),
        "handle_irq must retain the rseq::isr attribute in the model"
    );
}

#[test]
fn isr_body_keeps_helper_dispatch_and_branching() {
    let model = build_model_with_method_bodies(
        SPECIMEN,
        "source_to_regseq_model_12_body",
        &[("Pl011Isr", "handle_irq")],
    )
    .expect("specimen must build with body");

    let explicit_mask = find_driver_constant(&model, "EXPLICIT_ISR_CLEAR_MASK");
    let handle_irq = find_driver_impl_method(&model, "handle_irq");
    assert_eq!(
        collect_call_names(&model, &handle_irq.body),
        vec![
            "read_status",
            "service_rx_fifo",
            "service_tx_fifo",
            "clear_explicit_interrupts",
        ]
    );
    assert!(count_switch_int(&handle_irq.body) >= 3);
    assert!(body_mentions_constant(&handle_irq.body, explicit_mask.id));
}
