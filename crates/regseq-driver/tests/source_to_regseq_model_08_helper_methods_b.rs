#![feature(rustc_private)]

extern crate rustc_driver;
extern crate rustc_hir;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_session;

#[path = "support/regseq_model_harness.rs"]
mod harness;

use harness::{
    body_mentions_constant, build_model_with_method_bodies, collect_call_names, compile_and_build,
    count_switch_int, find_driver_constant, find_driver_impl_block, find_driver_impl_method,
    has_call_named, has_call_named_with_error_target,
};

const SPECIMEN: &str = r#"
#![no_std]

pub enum RegseqError { Failure }
pub struct Duration;
impl Duration {
    pub fn from_us(_us: u32) -> Self { Self }
}
pub struct Clock;
impl Clock {
    pub fn get_rate(&self) -> u32 { 24_000_000 }
    pub fn enable(&self) -> Result<(), RegseqError> { Ok(()) }
    pub fn disable(&self) {}
}

pub struct ResetLine;
impl ResetLine {
    pub fn toggle(&self, _duration: Duration) -> Result<(), RegseqError> { Ok(()) }
}

pub trait PinState {}
pub struct DefaultState;
impl PinState for DefaultState {}
pub struct SleepState;
impl PinState for SleepState {}

pub struct PinCtrl;
impl PinCtrl {
    pub fn apply<S: PinState>(&self) -> Result<(), RegseqError> { Ok(()) }
}

#[rseq::driver(name = "helpers-b", compatible = ["arm,pl011"], bus = "amba")]
pub struct Pl011HelpersB {
    #[rseq::clock(id = "uart_clk")]
    pub clock: Option<Clock>,
    #[rseq::reset(id = "uart_rst")]
    pub reset: Option<ResetLine>,
    #[rseq::pinctrl(id = "uart_pins")]
    pub pinctrl: Option<PinCtrl>,
    pending_tx: Option<u8>,
}

impl Pl011HelpersB {
    const DEFAULT_UART_CLOCK_HZ: u32 = 48_000_000;
    const ERROR_INTERRUPT_MASK: u32 = 0x0780;
    const EXPLICIT_ISR_CLEAR_MASK: u32 = 0x0780;

    fn mask_all_interrupts(&mut self) {}
    fn clear_all_interrupts(&mut self) {}
    fn disable_fifo(&mut self) {}
    fn disable_uart(&mut self) {}
    fn rx_char(&mut self) -> Option<u8> { None }
    fn send_byte(&mut self, _byte: u8) {}
    fn record_clear(&mut self, _mask: u32) {}

    fn uart_clock_hz(&self) -> u32 {
        if let Some(clock) = &self.clock {
            clock.get_rate()
        } else {
            Self::DEFAULT_UART_CLOCK_HZ
        }
    }

    fn enable_clock_if_present(&self) -> Result<(), RegseqError> {
        if let Some(clock) = &self.clock {
            clock.enable()?;
        }
        Ok(())
    }

    fn disable_clock_if_present(&self) {
        if let Some(clock) = &self.clock {
            clock.disable();
        }
    }

    fn toggle_reset_if_present(&self) -> Result<(), RegseqError> {
        if let Some(reset) = &self.reset {
            reset.toggle(Duration::from_us(10))?;
        }
        Ok(())
    }

    fn apply_default_pins_if_present(&self) -> Result<(), RegseqError> {
        if let Some(pins) = &self.pinctrl {
            pins.apply::<DefaultState>()?;
        }
        Ok(())
    }

    fn apply_sleep_pins_if_present(&self) -> Result<(), RegseqError> {
        if let Some(pins) = &self.pinctrl {
            pins.apply::<SleepState>()?;
        }
        Ok(())
    }

    fn shutdown_port(&mut self) {
        self.mask_all_interrupts();
        self.clear_all_interrupts();
        self.disable_fifo();
        self.disable_uart();
        self.pending_tx = None;
    }

    fn service_rx_fifo(&mut self) -> bool {
        let mut handled = false;
        while self.rx_char().is_some() {
            handled = true;
        }
        handled
    }

    fn service_tx_fifo(&mut self) -> bool {
        if let Some(byte) = self.pending_tx {
            self.pending_tx = None;
            self.send_byte(byte);
            true
        } else {
            false
        }
    }

    fn clear_explicit_interrupts(&mut self, status: u32) {
        let clear_mask = status & Self::EXPLICIT_ISR_CLEAR_MASK;
        if clear_mask != 0 {
            self.record_clear(clear_mask | Self::ERROR_INTERRUPT_MASK);
        }
    }
}
"#;

#[test]
fn helper_b_inventory_and_constants_land_in_driver_impl_block() {
    let model = compile_and_build(SPECIMEN, "source_to_regseq_model_08_inventory")
        .expect("specimen must build");
    let impl_block = find_driver_impl_block(&model);
    assert_eq!(impl_block.associated_constants.len(), 3);
    for name in [
        "uart_clock_hz",
        "enable_clock_if_present",
        "disable_clock_if_present",
        "toggle_reset_if_present",
        "apply_default_pins_if_present",
        "apply_sleep_pins_if_present",
        "shutdown_port",
        "service_rx_fifo",
        "service_tx_fifo",
        "clear_explicit_interrupts",
    ] {
        assert!(
            impl_block
                .methods
                .iter()
                .any(|method| harness::resolve_symbol(&model, method.name) == name),
            "driver impl block should contain helper `{name}`"
        );
    }
}

#[test]
fn helper_b_bodies_use_model_calls_switches_and_constants() {
    let model = build_model_with_method_bodies(
        SPECIMEN,
        "source_to_regseq_model_08_bodies",
        &[
            ("Pl011HelpersB", "uart_clock_hz"),
            ("Pl011HelpersB", "enable_clock_if_present"),
            ("Pl011HelpersB", "disable_clock_if_present"),
            ("Pl011HelpersB", "shutdown_port"),
            ("Pl011HelpersB", "service_rx_fifo"),
            ("Pl011HelpersB", "service_tx_fifo"),
            ("Pl011HelpersB", "clear_explicit_interrupts"),
        ],
    )
    .expect("specimen must build with bodies");

    let default_clock = find_driver_constant(&model, "DEFAULT_UART_CLOCK_HZ");
    let error_mask = find_driver_constant(&model, "ERROR_INTERRUPT_MASK");
    let explicit_mask = find_driver_constant(&model, "EXPLICIT_ISR_CLEAR_MASK");

    let uart_clock_hz = find_driver_impl_method(&model, "uart_clock_hz");
    assert!(has_call_named(&model, &uart_clock_hz.body, "get_rate"));
    assert!(body_mentions_constant(
        &uart_clock_hz.body,
        default_clock.id
    ));

    let enable_clock = find_driver_impl_method(&model, "enable_clock_if_present");
    assert!(has_call_named(&model, &enable_clock.body, "enable"));
    assert!(has_call_named_with_error_target(
        &model,
        &enable_clock.body,
        "enable"
    ));
    assert!(count_switch_int(&enable_clock.body) >= 1);

    let disable_clock = find_driver_impl_method(&model, "disable_clock_if_present");
    assert!(has_call_named(&model, &disable_clock.body, "disable"));
    assert!(count_switch_int(&disable_clock.body) >= 1);

    let shutdown = find_driver_impl_method(&model, "shutdown_port");
    assert_eq!(
        collect_call_names(&model, &shutdown.body),
        vec![
            "mask_all_interrupts",
            "clear_all_interrupts",
            "disable_fifo",
            "disable_uart",
        ]
    );

    let service_rx = find_driver_impl_method(&model, "service_rx_fifo");
    assert!(has_call_named(&model, &service_rx.body, "rx_char"));
    assert!(count_switch_int(&service_rx.body) >= 1);

    let service_tx = find_driver_impl_method(&model, "service_tx_fifo");
    assert!(has_call_named(&model, &service_tx.body, "send_byte"));
    assert!(count_switch_int(&service_tx.body) >= 1);

    let clear = find_driver_impl_method(&model, "clear_explicit_interrupts");
    assert!(body_mentions_constant(&clear.body, error_mask.id));
    assert!(body_mentions_constant(&clear.body, explicit_mask.id));
    assert!(has_call_named(&model, &clear.body, "record_clear"));
    assert!(count_switch_int(&clear.body) >= 1);
}
