#![feature(rustc_private)]

extern crate rustc_driver;
extern crate rustc_hir;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_session;

#[path = "support/regseq_model_harness.rs"]
mod harness;

use harness::{
    build_model_with_method_bodies, compile_and_build, find_driver_impl_block,
    find_driver_impl_method, collect_call_names,
};
use regseq_model::ir::{Mutability, RegseqType, ScalarType};
use regseq_model::model::ParamKind;

const SPECIMEN: &str = r#"
#![no_std]

#[rseq::driver(name = "helpers-a", compatible = ["arm,pl011"], bus = "amba")]
pub struct Pl011HelpersA {
    pub current_speed: u32,
    pending_tx: Option<u8>,
}

impl Pl011HelpersA {
    fn disable_uart(&mut self) {}
    fn disable_fifo(&mut self) {}
    fn mask_all_interrupts(&mut self) {}
    fn clear_all_interrupts(&mut self) {}
    fn configure_interrupt_levels(&mut self) {}
    fn set_baud_rate(&mut self, uart_clock_hz: u32, baud_rate: u32) {
        let _divider = uart_clock_hz / baud_rate;
    }
    fn configure_line_control(&mut self) {}
    fn enable_uart(&mut self) {}
    fn enable_rx_interrupts(&mut self) {}
    fn enable_tx_interrupts(&mut self) {}
    fn disable_tx_interrupts(&mut self) {}

    fn program_polling_port(&mut self, uart_clock_hz: u32, baud_rate: u32) {
        self.disable_uart();
        self.disable_fifo();
        self.set_baud_rate(uart_clock_hz, baud_rate);
        self.configure_line_control();
        self.configure_interrupt_levels();
        self.mask_all_interrupts();
        self.clear_all_interrupts();
        self.enable_uart();
        self.enable_rx_interrupts();
    }
}
"#;

#[test]
fn helper_inventory_and_signatures_survive_in_driver_impl_block() {
    let model = compile_and_build(SPECIMEN, "source_to_regseq_model_07_inventory")
        .expect("specimen must build");
    let impl_block = find_driver_impl_block(&model);
    assert_eq!(impl_block.methods.len(), 12);

    let set_baud_rate = find_driver_impl_method(&model, "set_baud_rate");
    assert_eq!(set_baud_rate.params.len(), 3);
    assert_eq!(set_baud_rate.params[0].kind, ParamKind::Receiver);
    assert_eq!(set_baud_rate.return_type, RegseqType::Unit);
    assert_eq!(
        set_baud_rate.params[1].ty,
        RegseqType::Scalar(ScalarType::U32)
    );
    assert_eq!(
        set_baud_rate.params[2].ty,
        RegseqType::Scalar(ScalarType::U32)
    );

    let program_polling_port = find_driver_impl_method(&model, "program_polling_port");
    assert_eq!(program_polling_port.params.len(), 3);
    assert_eq!(program_polling_port.params[0].kind, ParamKind::Receiver);
    assert_eq!(program_polling_port.return_type, RegseqType::Unit);
    assert_eq!(
        program_polling_port.params[0].ty,
        RegseqType::Ref(Box::new(RegseqType::Defined(harness::driver_struct(&model).type_id)), Mutability::Mutable)
    );
}

#[test]
fn program_polling_port_body_keeps_helper_call_order_in_model_ir() {
    let model = build_model_with_method_bodies(
        SPECIMEN,
        "source_to_regseq_model_07_body",
        &[("Pl011HelpersA", "program_polling_port")],
    )
    .expect("specimen must build with body");

    let program_polling_port = find_driver_impl_method(&model, "program_polling_port");
    let call_names = collect_call_names(&model, &program_polling_port.body);
    assert_eq!(
        call_names,
        vec![
            "disable_uart",
            "disable_fifo",
            "set_baud_rate",
            "configure_line_control",
            "configure_interrupt_levels",
            "mask_all_interrupts",
            "clear_all_interrupts",
            "enable_uart",
            "enable_rx_interrupts",
        ]
    );
}
