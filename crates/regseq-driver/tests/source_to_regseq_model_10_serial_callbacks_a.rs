#![feature(rustc_private)]

extern crate rustc_driver;
extern crate rustc_hir;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_session;

#[path = "support/regseq_model_harness.rs"]
mod harness;

use harness::{
    build_model_with_method_bodies, collect_call_names, compile_and_build, count_switch_int,
    find_trait_method, has_call_named, has_call_named_with_error_target,
};
use regseq_model::ir::{RegseqType, ScalarType};
use regseq_model::model::ParamKind;

const SPECIMEN: &str = r#"
#![no_std]

pub enum RegseqError { Failure }
pub struct SerialConfig;
pub struct RxChar(pub u8);
impl RxChar { pub fn new(byte: u8) -> Self { Self(byte) } }

pub fn field_from_raw(raw: u8) -> u8 { raw }

pub trait Serial {
    fn port_open(&mut self) -> Result<(), RegseqError>;
    fn port_close(&mut self);
    fn configure(&mut self, config: &SerialConfig) -> Result<(), RegseqError>;
    fn tx_byte(&mut self, byte: u8);
    fn rx_char(&mut self) -> Option<RxChar>;
}

#[rseq::driver(name = "serial-a", compatible = ["arm,pl011"], bus = "amba")]
pub struct Pl011SerialA {
    pub current_speed: u32,
    pending_tx: Option<u8>,
    rx_queue: Option<u8>,
}

impl Pl011SerialA {
    fn enable_clock_if_present(&self) -> Result<(), RegseqError> { Ok(()) }
    fn disable_clock_if_present(&self) {}
    fn shutdown_port(&mut self) {}
    fn uart_clock_hz(&self) -> u32 { 48_000_000 }
    fn program_polling_port(&mut self, _uart_clock_hz: u32, _baud_rate: u32) {}
    fn tx_ready(&mut self) -> bool { true }
    fn enable_tx_interrupts(&mut self) {}
    fn send_byte(&mut self, _byte: u8) {}
    fn read_raw(&mut self) -> Option<u8> { self.rx_queue }
}

impl Serial for Pl011SerialA {
    fn port_open(&mut self) -> Result<(), RegseqError> {
        self.enable_clock_if_present()?;
        let uart_clock_hz = self.uart_clock_hz();
        self.program_polling_port(uart_clock_hz, self.current_speed);
        Ok(())
    }

    fn port_close(&mut self) {
        self.shutdown_port();
        self.disable_clock_if_present();
    }

    fn configure(&mut self, _config: &SerialConfig) -> Result<(), RegseqError> {
        self.enable_clock_if_present()?;
        let uart_clock_hz = self.uart_clock_hz();
        self.program_polling_port(uart_clock_hz, self.current_speed);
        Ok(())
    }

    fn tx_byte(&mut self, byte: u8) {
        if self.pending_tx.is_none() && self.tx_ready() {
            self.send_byte(byte);
        } else {
            self.pending_tx = Some(byte);
            self.enable_tx_interrupts();
        }
    }

    fn rx_char(&mut self) -> Option<RxChar> {
        if let Some(raw) = self.read_raw() {
            let byte = field_from_raw(raw);
            Some(RxChar::new(byte))
        } else {
            None
        }
    }
}
"#;

#[test]
fn serial_trait_callbacks_lower_with_expected_signatures() {
    let model = compile_and_build(SPECIMEN, "source_to_regseq_model_10_signatures")
        .expect("specimen must build");

    let port_open = find_trait_method(&model, "Serial", "port_open");
    assert_eq!(port_open.params[0].kind, ParamKind::Receiver);
    assert_eq!(port_open.params.len(), 1);

    let port_close = find_trait_method(&model, "Serial", "port_close");
    assert_eq!(port_close.return_type, RegseqType::Unit);

    let configure = find_trait_method(&model, "Serial", "configure");
    assert_eq!(configure.params.len(), 2);

    let tx_byte = find_trait_method(&model, "Serial", "tx_byte");
    assert_eq!(tx_byte.params[1].kind, ParamKind::Regular);
    assert_eq!(tx_byte.params[1].ty, RegseqType::Scalar(ScalarType::U8));

    let rx_char = find_trait_method(&model, "Serial", "rx_char");
    assert_eq!(rx_char.params.len(), 1);
}

#[test]
fn serial_open_close_and_configure_bodies_keep_helper_flow() {
    let model = build_model_with_method_bodies(
        SPECIMEN,
        "source_to_regseq_model_10_control",
        &[
            ("Pl011SerialA", "port_open"),
            ("Pl011SerialA", "port_close"),
            ("Pl011SerialA", "configure"),
        ],
    )
    .expect("specimen must build with bodies");

    let port_open = find_trait_method(&model, "Serial", "port_open");
    assert_eq!(
        collect_call_names(&model, &port_open.body),
        vec![
            "enable_clock_if_present",
            "uart_clock_hz",
            "program_polling_port"
        ]
    );
    assert!(has_call_named_with_error_target(
        &model,
        &port_open.body,
        "enable_clock_if_present"
    ));

    let port_close = find_trait_method(&model, "Serial", "port_close");
    assert_eq!(
        collect_call_names(&model, &port_close.body),
        vec!["shutdown_port", "disable_clock_if_present"]
    );

    let configure = find_trait_method(&model, "Serial", "configure");
    assert_eq!(
        collect_call_names(&model, &configure.body),
        vec![
            "enable_clock_if_present",
            "uart_clock_hz",
            "program_polling_port"
        ]
    );
    assert!(has_call_named_with_error_target(
        &model,
        &configure.body,
        "enable_clock_if_present"
    ));
}

#[test]
fn serial_tx_and_rx_callbacks_keep_branchy_model_bodies() {
    let model = build_model_with_method_bodies(
        SPECIMEN,
        "source_to_regseq_model_10_data",
        &[("Pl011SerialA", "tx_byte"), ("Pl011SerialA", "rx_char")],
    )
    .expect("specimen must build with bodies");

    let tx_byte = find_trait_method(&model, "Serial", "tx_byte");
    assert!(count_switch_int(&tx_byte.body) >= 1);
    assert!(has_call_named(&model, &tx_byte.body, "tx_ready"));
    assert!(has_call_named(&model, &tx_byte.body, "send_byte"));
    assert!(has_call_named(
        &model,
        &tx_byte.body,
        "enable_tx_interrupts"
    ));

    let rx_char = find_trait_method(&model, "Serial", "rx_char");
    assert!(count_switch_int(&rx_char.body) >= 1);
    assert!(has_call_named(&model, &rx_char.body, "read_raw"));
    assert!(has_call_named(&model, &rx_char.body, "field_from_raw"));
    assert!(has_call_named(&model, &rx_char.body, "new"));
}
