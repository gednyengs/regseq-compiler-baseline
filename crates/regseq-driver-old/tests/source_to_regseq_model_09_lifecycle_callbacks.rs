//! Voided pending MonoRust + RegseqModel redesign.
//! See docs/discussions/compiler-refactoring/test-audit.md for the classification rationale
//! and the retirement criterion.
#![cfg(feature = "post-monorust-tests")]

#![feature(rustc_private)]

extern crate rustc_driver;
extern crate rustc_hir;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_session;

#[path = "support/regseq_model_harness.rs"]
mod harness;

use harness::{
    build_model_with_method_bodies, collect_call_names, compile_and_build, find_trait_method,
    has_call_named_with_error_target,
};
use regseq_model_old::model::ParamKind;

const SPECIMEN: &str = r#"
#![no_std]

pub enum RegseqError { Failure }
pub struct Duration;
impl Duration {
    pub fn from_us(_us: u32) -> Self { Self }
}
pub struct Clock;
impl Clock {
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

pub trait Lifecycle {
    fn init(&mut self) -> Result<(), RegseqError>;
    fn deinit(&mut self) -> Result<(), RegseqError>;
}

#[rseq::driver(name = "lifecycle", compatible = ["arm,pl011"], bus = "amba")]
pub struct Pl011Lifecycle {
    #[rseq::clock(id = "uart_clk")]
    pub clock: Option<Clock>,
    #[rseq::reset(id = "uart_rst")]
    pub reset: Option<ResetLine>,
    #[rseq::pinctrl(id = "uart_pins")]
    pub pinctrl: Option<PinCtrl>,
    pub current_speed: u32,
}

impl Pl011Lifecycle {
    fn toggle_reset_if_present(&self) -> Result<(), RegseqError> {
        if let Some(reset) = &self.reset { reset.toggle(Duration::from_us(10))?; }
        Ok(())
    }
    fn apply_default_pins_if_present(&self) -> Result<(), RegseqError> {
        if let Some(pins) = &self.pinctrl { pins.apply::<DefaultState>()?; }
        Ok(())
    }
    fn enable_clock_if_present(&self) -> Result<(), RegseqError> {
        if let Some(clock) = &self.clock { clock.enable()?; }
        Ok(())
    }
    fn uart_clock_hz(&self) -> u32 { 48_000_000 }
    fn program_polling_port(&mut self, _uart_clock_hz: u32, _baud_rate: u32) {}
    fn shutdown_port(&mut self) {}
    fn disable_clock_if_present(&self) {
        if let Some(clock) = &self.clock { clock.disable(); }
    }
    fn apply_sleep_pins_if_present(&self) -> Result<(), RegseqError> {
        if let Some(pins) = &self.pinctrl { pins.apply::<SleepState>()?; }
        Ok(())
    }
}

impl Lifecycle for Pl011Lifecycle {
    fn init(&mut self) -> Result<(), RegseqError> {
        self.toggle_reset_if_present()?;
        self.apply_default_pins_if_present()?;
        self.enable_clock_if_present()?;
        let uart_clock_hz = self.uart_clock_hz();
        self.program_polling_port(uart_clock_hz, self.current_speed);
        Ok(())
    }

    fn deinit(&mut self) -> Result<(), RegseqError> {
        self.shutdown_port();
        self.disable_clock_if_present();
        self.apply_sleep_pins_if_present()?;
        Ok(())
    }
}
"#;

#[test]
fn lifecycle_trait_impl_is_present_with_result_signatures() {
    let model = compile_and_build(SPECIMEN, "source_to_regseq_model_09_signatures")
        .expect("specimen must build");

    let init = find_trait_method(&model, "Lifecycle", "init");
    assert_eq!(init.params[0].kind, ParamKind::Receiver);
    assert_eq!(init.params.len(), 1);

    let deinit = find_trait_method(&model, "Lifecycle", "deinit");
    assert_eq!(deinit.params[0].kind, ParamKind::Receiver);
    assert_eq!(deinit.params.len(), 1);
}

#[test]
fn lifecycle_bodies_keep_helper_order_and_try_edges() {
    let model = build_model_with_method_bodies(
        SPECIMEN,
        "source_to_regseq_model_09_bodies",
        &[("Pl011Lifecycle", "init"), ("Pl011Lifecycle", "deinit")],
    )
    .expect("specimen must build with bodies");

    let init = find_trait_method(&model, "Lifecycle", "init");
    assert_eq!(
        collect_call_names(&model, &init.body),
        vec![
            "toggle_reset_if_present",
            "apply_default_pins_if_present",
            "enable_clock_if_present",
            "uart_clock_hz",
            "program_polling_port",
        ]
    );
    assert!(has_call_named_with_error_target(
        &model,
        &init.body,
        "toggle_reset_if_present"
    ));
    assert!(has_call_named_with_error_target(
        &model,
        &init.body,
        "apply_default_pins_if_present"
    ));
    assert!(has_call_named_with_error_target(
        &model,
        &init.body,
        "enable_clock_if_present"
    ));

    let deinit = find_trait_method(&model, "Lifecycle", "deinit");
    assert_eq!(
        collect_call_names(&model, &deinit.body),
        vec![
            "shutdown_port",
            "disable_clock_if_present",
            "apply_sleep_pins_if_present",
        ]
    );
    assert!(has_call_named_with_error_target(
        &model,
        &deinit.body,
        "apply_sleep_pins_if_present"
    ));
}
