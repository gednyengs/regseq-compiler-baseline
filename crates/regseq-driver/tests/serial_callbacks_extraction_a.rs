//! source-to-extraction-10: PL011 serial callbacks A authored->extraction
//! boundary validation.
//!
//! Validates that the first half of the authored serial-callback facts for
//! PL011 survive at the compiler-side boundary between the authored PL011
//! source and the current Regseq extraction state.
//!
//! Extraction surfaces under test:
//! - `TraitSurface.serial_impls` (trait-role identity)
//! - `IdentityEnrichmentSurface.impl_blocks` (callback identity)
//! - `MirBodySurface.functions` (executable callback bodies)

#![feature(rustc_private)]
#![feature(box_patterns)]

extern crate rustc_ast;
extern crate rustc_driver;
extern crate rustc_hir;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_session;

use std::sync::{Arc, Mutex, Once};

#[path = "../src/extract/callback_context.rs"]
mod callback_context;

#[path = "../src/extract/traits.rs"]
mod traits;

#[path = "../src/extract/identity_enrichment_types.rs"]
mod identity_enrichment_types;

#[path = "../src/extract/identity_enrichment.rs"]
mod identity_enrichment;

#[path = "../src/mir_repr.rs"]
mod mir_repr;

#[path = "../src/extract/mir_extract.rs"]
mod mir_extract;

#[path = "../src/extract/body.rs"]
mod body;

use body::*;
use identity_enrichment::*;
use mir_repr::*;
use traits::*;

// ---------------------------------------------------------------------------
// Shared compiler harness
// ---------------------------------------------------------------------------

static INIT: Once = Once::new();
static COMPILER_LOCK: Mutex<()> = Mutex::new(());

fn init_rustc() {
    INIT.call_once(|| {
        let early_dcx =
            rustc_session::EarlyDiagCtxt::new(rustc_session::config::ErrorOutputType::default());
        rustc_driver::init_rustc_env_logger(&early_dcx);
    });
}

fn get_sysroot() -> String {
    let output = std::process::Command::new("rustc")
        .arg("--print")
        .arg("sysroot")
        .output()
        .expect("failed to run rustc --print sysroot");
    String::from_utf8(output.stdout).unwrap().trim().to_string()
}

struct ExtractionTriple {
    trait_surface: TraitSurface,
    identity: IdentityEnrichmentSurface,
    body: MirBodySurface,
}

fn compile_and_extract_all(source: &str, test_name: &str) -> ExtractionTriple {
    let _lock = COMPILER_LOCK.lock().unwrap();
    init_rustc();

    let test_dir = std::env::temp_dir()
        .join("regseq-driver-tests")
        .join(test_name);
    if test_dir.exists() {
        std::fs::remove_dir_all(&test_dir).unwrap();
    }
    std::fs::create_dir_all(&test_dir).unwrap();

    let src_path = test_dir.join("test_input.rs");
    std::fs::write(&src_path, source).unwrap();

    let sysroot = get_sysroot();

    type TraitResult = Result<TraitSurface, TraitError>;
    type IdentityResult = Result<IdentityEnrichmentSurface, IdentityEnrichmentError>;
    type BodyResult = Result<MirBodySurface, BodyError>;

    let trait_result: Arc<Mutex<Option<TraitResult>>> = Arc::new(Mutex::new(None));
    let identity_result: Arc<Mutex<Option<IdentityResult>>> = Arc::new(Mutex::new(None));
    let body_result: Arc<Mutex<Option<BodyResult>>> = Arc::new(Mutex::new(None));

    struct ExtractCallbacks {
        trait_result: Arc<Mutex<Option<TraitResult>>>,
        identity_result: Arc<Mutex<Option<IdentityResult>>>,
        body_result: Arc<Mutex<Option<BodyResult>>>,
    }

    impl rustc_driver::Callbacks for ExtractCallbacks {
        fn config(&mut self, config: &mut rustc_interface::interface::Config) {
            config.opts.unstable_opts.no_codegen = true;
            config
                .opts
                .unstable_opts
                .crate_attr
                .push("feature(register_tool)".to_string());
            config
                .opts
                .unstable_opts
                .crate_attr
                .push("register_tool(rseq)".to_string());
        }

        fn after_analysis<'tcx>(
            &mut self,
            _compiler: &rustc_interface::interface::Compiler,
            tcx: rustc_middle::ty::TyCtxt<'tcx>,
        ) -> rustc_driver::Compilation {
            *self.trait_result.lock().unwrap() = Some(extract_trait_surface(tcx));
            *self.identity_result.lock().unwrap() = Some(extract_identity_enrichment(tcx));
            *self.body_result.lock().unwrap() = Some(extract_body_surface(tcx));
            rustc_driver::Compilation::Stop
        }
    }

    let mut callbacks = ExtractCallbacks {
        trait_result: trait_result.clone(),
        identity_result: identity_result.clone(),
        body_result: body_result.clone(),
    };

    let args = vec![
        "rustc".to_string(),
        src_path.to_str().unwrap().to_string(),
        "--sysroot".to_string(),
        sysroot,
        "--edition".to_string(),
        "2021".to_string(),
        "--crate-type".to_string(),
        "lib".to_string(),
        "--out-dir".to_string(),
        test_dir.to_str().unwrap().to_string(),
    ];

    rustc_driver::run_compiler(&args, &mut callbacks);

    let trait_surface = trait_result
        .lock()
        .unwrap()
        .take()
        .expect("after_analysis should have been called")
        .expect("trait extraction should succeed");

    let identity = identity_result
        .lock()
        .unwrap()
        .take()
        .expect("after_analysis should have been called")
        .expect("identity enrichment extraction should succeed");

    let body = body_result
        .lock()
        .unwrap()
        .take()
        .expect("after_analysis should have been called")
        .expect("body extraction should succeed");

    std::fs::remove_dir_all(&test_dir).ok();

    ExtractionTriple {
        trait_surface,
        identity,
        body,
    }
}

// ---------------------------------------------------------------------------
// Inline specimen: reduced PL011 serial callbacks A surface
// ---------------------------------------------------------------------------

/// Reduced specimen covering the serial callbacks in check group A:
/// port_open, port_close, configure, tx_byte, rx_char.
///
/// This specimen reproduces the authored facts from the fixture source
/// with only the types and trait definitions needed to compile and extract
/// the five callbacks and their reachable helpers.
const SPECIMEN: &str = r#"
    #![allow(dead_code, unused_variables)]

    pub mod regseq {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum RegseqError {
            InvalidParameter,
            HardwareError,
            GenericFailure,
        }

        pub struct SerialConfig { _private: () }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub struct ErrorKind(u32);

        impl ErrorKind {
            pub const OVERRUN: ErrorKind = ErrorKind(1);
            pub const BREAK: ErrorKind = ErrorKind(2);
            pub const PARITY: ErrorKind = ErrorKind(4);
            pub const FRAMING: ErrorKind = ErrorKind(8);

            pub fn empty() -> ErrorKind { ErrorKind(0) }
        }

        impl std::ops::BitOr for ErrorKind {
            type Output = ErrorKind;
            fn bitor(self, rhs: ErrorKind) -> ErrorKind { ErrorKind(self.0 | rhs.0) }
        }

        pub struct RxChar { byte: u8, errors: ErrorKind }
        impl RxChar {
            pub fn new(byte: u8, errors: ErrorKind) -> RxChar {
                RxChar { byte, errors }
            }
        }

        pub struct Clock { _opaque: () }
        impl Clock {
            pub fn get_rate(&self) -> u32 { 0 }
            pub fn enable(&self) -> Result<(), RegseqError> { Ok(()) }
            pub fn disable(&self) { }
        }

        pub trait Serial {
            fn port_open(&mut self) -> Result<(), RegseqError>;
            fn port_close(&mut self);
            fn configure(&mut self, config: &SerialConfig) -> Result<(), RegseqError>;
            fn tx_byte(&mut self, byte: u8);
            fn rx_char(&mut self) -> Option<RxChar>;
            fn tx_ready(&mut self) -> bool;
            fn tx_empty(&mut self) -> bool;
            fn flush_rx_fifo(&mut self);
            fn flush_tx_fifo(&mut self);
            fn send_break(&mut self, on: bool);
        }
    }

    use regseq::*;

    pub struct Field<T, R> { pub mask: T, pub shift: T, _r: std::marker::PhantomData<R> }
    pub struct FieldValue<T, R> { mask: T, value: T, _r: std::marker::PhantomData<R> }
    impl<R> FieldValue<u32, R> {
        pub const fn new(mask: u32, value: u32) -> Self {
            FieldValue { mask, value, _r: std::marker::PhantomData }
        }
    }
    impl<R> std::ops::Add for FieldValue<u32, R> {
        type Output = Self;
        fn add(self, rhs: Self) -> Self {
            FieldValue::new(self.mask | rhs.mask, self.value | rhs.value)
        }
    }

    pub struct Register<T> { val: std::cell::Cell<T> }
    impl Register<u32> {
        pub fn write<R>(&self, _fv: FieldValue<u32, R>) { }
        pub fn modify<R>(&self, _fv: FieldValue<u32, R>) { }
        pub fn read<R>(&self, _field: Field<u32, R>) -> u32 { 0 }
        pub fn read_raw(&self) -> u32 { 0 }
    }

    pub mod regs {
        use super::*;
        pub struct Pl011Registers {
            pub dr: Register<u32>,
            pub fr: Register<u32>,
            pub ibrd: Register<u32>,
            pub fbrd: Register<u32>,
            pub lcr_h: Register<u32>,
            pub cr: Register<u32>,
            pub ifls: Register<u32>,
            pub imsc: Register<u32>,
            pub icr: Register<u32>,
        }

        pub struct DrReg;
        pub mod DR_REG {
            use super::super::*;
            pub struct Register;
            pub mod DATA {
                use super::super::super::*;
                pub const FIELD: Field<u32, super::Register> = Field {
                    mask: 0xFF, shift: 0, _r: std::marker::PhantomData
                };
            }
            pub mod OE {
                use super::super::super::*;
                pub const FIELD: Field<u32, super::Register> = Field {
                    mask: 1 << 11, shift: 11, _r: std::marker::PhantomData
                };
            }
            pub mod BE {
                use super::super::super::*;
                pub const FIELD: Field<u32, super::Register> = Field {
                    mask: 1 << 10, shift: 10, _r: std::marker::PhantomData
                };
            }
            pub mod PE {
                use super::super::super::*;
                pub const FIELD: Field<u32, super::Register> = Field {
                    mask: 1 << 9, shift: 9, _r: std::marker::PhantomData
                };
            }
            pub mod FE {
                use super::super::super::*;
                pub const FIELD: Field<u32, super::Register> = Field {
                    mask: 1 << 8, shift: 8, _r: std::marker::PhantomData
                };
            }
        }
        pub mod FR_REG {
            use super::super::*;
            pub struct Register;
            pub mod RXFE {
                use super::super::super::*;
                pub const FIELD: Field<u32, super::Register> = Field {
                    mask: 1 << 4, shift: 4, _r: std::marker::PhantomData
                };
            }
            pub mod TXFF {
                use super::super::super::*;
                pub const FIELD: Field<u32, super::Register> = Field {
                    mask: 1 << 5, shift: 5, _r: std::marker::PhantomData
                };
            }
        }
        pub mod CR_REG {
            use super::super::*;
            pub struct Register;
            pub mod UARTEN { use super::super::super::*;
                pub const ENABLED: FieldValue<u32, super::Register> = FieldValue::new(1, 1);
                pub const DISABLED: FieldValue<u32, super::Register> = FieldValue::new(1, 0);
            }
            pub mod TXE { use super::super::super::*;
                pub const ENABLED: FieldValue<u32, super::Register> = FieldValue::new(1 << 8, 1 << 8);
                pub const DISABLED: FieldValue<u32, super::Register> = FieldValue::new(1 << 8, 0);
            }
            pub mod RXE { use super::super::super::*;
                pub const ENABLED: FieldValue<u32, super::Register> = FieldValue::new(1 << 9, 1 << 9);
                pub const DISABLED: FieldValue<u32, super::Register> = FieldValue::new(1 << 9, 0);
            }
        }
        pub mod LCRH_REG {
            use super::super::*;
            pub struct Register;
            pub mod WLEN { use super::super::super::*;
                pub const EIGHT_BITS: FieldValue<u32, super::Register> = FieldValue::new(0x60, 0x60);
            }
            pub mod FEN { use super::super::super::*;
                pub const ENABLED: FieldValue<u32, super::Register> = FieldValue::new(1 << 4, 1 << 4);
                pub const DISABLED: FieldValue<u32, super::Register> = FieldValue::new(1 << 4, 0);
            }
            pub mod BRK { use super::super::super::*;
                pub const ENABLED: FieldValue<u32, super::Register> = FieldValue::new(1, 1);
                pub const DISABLED: FieldValue<u32, super::Register> = FieldValue::new(1, 0);
            }
        }
        pub mod IBRD_REG {
            use super::super::*;
            pub struct Register;
            pub mod BAUD_DIVINT {
                use super::super::super::*;
                pub const FIELD: Field<u32, super::Register> = Field {
                    mask: 0xFFFF, shift: 0, _r: std::marker::PhantomData
                };
            }
        }
        pub mod FBRD_REG {
            use super::super::*;
            pub struct Register;
            pub mod BAUD_DIVFRAC {
                use super::super::super::*;
                pub const FIELD: Field<u32, super::Register> = Field {
                    mask: 0x3F, shift: 0, _r: std::marker::PhantomData
                };
            }
        }
        pub mod IFLS_REG {
            use super::super::*;
            pub struct Register;
            pub mod RXIFLSEL { use super::super::super::*;
                pub const ONE_HALF: FieldValue<u32, super::Register> = FieldValue::new(0x38, 0x10);
            }
            pub mod TXIFLSEL { use super::super::super::*;
                pub const ONE_EIGHTH: FieldValue<u32, super::Register> = FieldValue::new(0x07, 0x00);
            }
        }
        pub mod IMSC_REG {
            use super::super::*;
            pub struct Register;
        }
        pub mod ICR_REG {
            use super::super::*;
            pub struct Register;
        }
    }

    fn field_val<R>(field: Field<u32, R>, raw: u32) -> FieldValue<u32, R> {
        FieldValue::new(field.mask, raw << field.shift)
    }

    fn raw_reg_val<R>(mask: u32, value: u32) -> FieldValue<u32, R> {
        FieldValue::new(mask, value)
    }

    fn field_from_raw<R>(field: Field<u32, R>, raw: u32) -> u32 {
        (raw & field.mask) >> field.shift
    }

    use regs::*;

    #[rseq::driver(name = "pl011-full-spec", compatible = ["arm,pl011"])]
    pub struct Pl011FullSpec {
        pub regs: Pl011Registers,
        pub current_speed: u32,
        pub clock: Option<Clock>,
        pending_tx: Option<u8>,
    }

    impl Pl011FullSpec {
        const ALL_UART_INTERRUPTS_MASK: u32 = 0x07ff;
        const RX_INTERRUPT_MASK: u32 = (1 << 4) | (1 << 6);
        const TX_INTERRUPT_MASK: u32 = 1 << 5;
        const DEFAULT_UART_CLOCK_HZ: u32 = 48_000_000;

        fn disable_uart(&mut self) {
            self.regs.cr.write(
                CR_REG::UARTEN::DISABLED + CR_REG::TXE::DISABLED + CR_REG::RXE::DISABLED,
            );
        }

        fn disable_fifo(&mut self) {
            self.regs.lcr_h.modify(LCRH_REG::FEN::DISABLED);
        }

        fn mask_all_interrupts(&mut self) {
            self.regs.imsc.write(raw_reg_val::<IMSC_REG::Register>(
                Self::ALL_UART_INTERRUPTS_MASK, 0,
            ));
        }

        fn clear_all_interrupts(&mut self) {
            self.regs.icr.write(raw_reg_val::<ICR_REG::Register>(
                Self::ALL_UART_INTERRUPTS_MASK, Self::ALL_UART_INTERRUPTS_MASK,
            ));
        }

        fn configure_interrupt_levels(&mut self) {
            self.regs.ifls.write(
                IFLS_REG::RXIFLSEL::ONE_HALF + IFLS_REG::TXIFLSEL::ONE_EIGHTH,
            );
        }

        fn set_baud_rate(&mut self, uart_clk_hz: u32, baud_rate: u32) {
            let div_times_64 = (uart_clk_hz as u64 * 4) / baud_rate as u64;
            let ibrd = (div_times_64 >> 6) as u32;
            let fbrd = (div_times_64 & 0x3f) as u32;
            self.regs.ibrd.write(field_val(IBRD_REG::BAUD_DIVINT::FIELD, ibrd));
            self.regs.fbrd.write(field_val(FBRD_REG::BAUD_DIVFRAC::FIELD, fbrd));
        }

        fn configure_line_control(&mut self) {
            self.regs.lcr_h.write(
                LCRH_REG::WLEN::EIGHT_BITS + LCRH_REG::FEN::ENABLED,
            );
        }

        fn enable_uart(&mut self) {
            self.regs.cr.write(
                CR_REG::UARTEN::ENABLED + CR_REG::TXE::ENABLED + CR_REG::RXE::ENABLED,
            );
        }

        fn enable_rx_interrupts(&mut self) {
            self.regs.imsc.modify(raw_reg_val::<IMSC_REG::Register>(
                Self::RX_INTERRUPT_MASK, Self::RX_INTERRUPT_MASK,
            ));
        }

        fn enable_tx_interrupts(&mut self) {
            self.regs.imsc.modify(raw_reg_val::<IMSC_REG::Register>(
                Self::TX_INTERRUPT_MASK, Self::TX_INTERRUPT_MASK,
            ));
        }

        fn program_polling_port(&mut self, uart_clk_hz: u32, baud_rate: u32) {
            self.disable_uart();
            self.disable_fifo();
            self.set_baud_rate(uart_clk_hz, baud_rate);
            self.configure_line_control();
            self.configure_interrupt_levels();
            self.mask_all_interrupts();
            self.clear_all_interrupts();
            self.enable_uart();
            self.enable_rx_interrupts();
        }

        fn uart_clock_hz(&self) -> u32 {
            self.clock
                .as_ref()
                .map(Clock::get_rate)
                .unwrap_or(Self::DEFAULT_UART_CLOCK_HZ)
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

        fn shutdown_port(&mut self) {
            self.mask_all_interrupts();
            self.clear_all_interrupts();
            self.disable_fifo();
            self.disable_uart();
            self.pending_tx = None;
        }
    }

    impl regseq::Serial for Pl011FullSpec {
        fn port_open(&mut self) -> Result<(), RegseqError> {
            self.enable_clock_if_present()?;
            self.program_polling_port(self.uart_clock_hz(), self.current_speed);
            Ok(())
        }

        fn port_close(&mut self) {
            self.shutdown_port();
            self.disable_clock_if_present();
        }

        fn configure(&mut self, _config: &SerialConfig) -> Result<(), RegseqError> {
            self.enable_clock_if_present()?;
            self.program_polling_port(self.uart_clock_hz(), self.current_speed);
            Ok(())
        }

        fn tx_byte(&mut self, byte: u8) {
            if self.pending_tx.is_none() && self.tx_ready() {
                self.regs.dr.write(field_val(DR_REG::DATA::FIELD, byte as u32));
                return;
            }
            self.pending_tx = Some(byte);
            self.enable_tx_interrupts();
        }

        fn rx_char(&mut self) -> Option<RxChar> {
            if self.regs.fr.read(FR_REG::RXFE::FIELD) != 0 {
                return None;
            }
            let dr = self.regs.dr.read_raw();
            let byte = field_from_raw(DR_REG::DATA::FIELD, dr) as u8;
            let mut errors = ErrorKind::empty();
            if field_from_raw(DR_REG::OE::FIELD, dr) != 0 {
                errors = errors | ErrorKind::OVERRUN;
            }
            if field_from_raw(DR_REG::BE::FIELD, dr) != 0 {
                errors = errors | ErrorKind::BREAK;
            }
            if field_from_raw(DR_REG::PE::FIELD, dr) != 0 {
                errors = errors | ErrorKind::PARITY;
            }
            if field_from_raw(DR_REG::FE::FIELD, dr) != 0 {
                errors = errors | ErrorKind::FRAMING;
            }
            Some(RxChar::new(byte, errors))
        }

        fn tx_ready(&mut self) -> bool {
            self.regs.fr.read(FR_REG::TXFF::FIELD) == 0
        }

        fn tx_empty(&mut self) -> bool {
            false
        }

        fn flush_rx_fifo(&mut self) { }

        fn flush_tx_fifo(&mut self) { }

        fn send_break(&mut self, _on: bool) { }
    }
"#;

// ---------------------------------------------------------------------------
// Helper: find an extracted function by name
// ---------------------------------------------------------------------------

fn find_fn<'a>(body: &'a MirBodySurface, name: &str) -> &'a MirExtractedFn {
    body.functions
        .iter()
        .find(|f| f.name == name)
        .unwrap_or_else(|| panic!("function '{}' not found in body surface", name))
}

/// Check if a MIR body contains a Call terminator whose func operand
/// mentions the given callee name substring.
fn has_call_to(body: &MirBody, callee_substr: &str) -> bool {
    body.basic_blocks.iter().any(|bb| {
        bb.terminator.as_ref().is_some_and(|t| match &t.kind {
            MirTerminatorKind::Call { func, .. } => operand_mentions(func, callee_substr),
            _ => false,
        })
    })
}

/// Check if an operand's constant path mentions a given substring.
fn operand_mentions(op: &MirOperand, substr: &str) -> bool {
    match op {
        MirOperand::Constant { constant } => match &constant.ty {
            MirTy::FnDef { path, .. } => path.contains(substr),
            _ => false,
        },
        _ => false,
    }
}

/// Collect all Call terminator callee names from a MIR body, in block order.
fn collect_call_names(body: &MirBody) -> Vec<String> {
    let mut names = Vec::new();
    for bb in &body.basic_blocks {
        if let Some(t) = &bb.terminator {
            if let MirTerminatorKind::Call { func, .. } = &t.kind {
                if let MirOperand::Constant { constant } = func {
                    if let MirTy::FnDef { path, .. } = &constant.ty {
                        names.push(path.clone());
                    }
                }
            }
        }
    }
    names
}

/// Check if a MIR body has a Return terminator.
fn has_return(body: &MirBody) -> bool {
    body.basic_blocks.iter().any(|bb| {
        matches!(
            bb.terminator.as_ref().map(|t| &t.kind),
            Some(MirTerminatorKind::Return)
        )
    })
}

/// Check if a MIR body has a SwitchInt terminator.
fn has_switch_int(body: &MirBody) -> bool {
    body.basic_blocks.iter().any(|bb| {
        matches!(
            bb.terminator.as_ref().map(|t| &t.kind),
            Some(MirTerminatorKind::SwitchInt { .. })
        )
    })
}

/// Count how many times a given callee name appears in Call terminators.
fn count_calls_to(body: &MirBody, callee_substr: &str) -> usize {
    body.basic_blocks
        .iter()
        .filter(|bb| {
            bb.terminator.as_ref().is_some_and(|t| match &t.kind {
                MirTerminatorKind::Call { func, .. } => operand_mentions(func, callee_substr),
                _ => false,
            })
        })
        .count()
}

// ---------------------------------------------------------------------------
// Precondition: specimen fidelity against real fixture source
// ---------------------------------------------------------------------------

#[test]
fn precondition_specimen_fidelity() {
    let fixture = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/pl011-full-spec/src/driver.rs"
    ))
    .expect("fixture source must be readable");

    // -- port_open authored facts --
    assert!(
        fixture.contains("fn port_open(&mut self) -> Result<(), RegseqError>"),
        "fixture must contain port_open signature"
    );
    assert!(
        fixture.contains("self.enable_clock_if_present()?;"),
        "fixture port_open must call enable_clock_if_present with ?"
    );
    assert!(
        fixture.contains("self.program_polling_port(self.uart_clock_hz(), self.current_speed)"),
        "fixture port_open must call program_polling_port"
    );

    // -- port_close authored facts --
    assert!(
        fixture.contains("fn port_close(&mut self)"),
        "fixture must contain port_close signature"
    );
    assert!(
        fixture.contains("self.shutdown_port();"),
        "fixture port_close must call shutdown_port"
    );
    assert!(
        fixture.contains("self.disable_clock_if_present();"),
        "fixture port_close must call disable_clock_if_present"
    );

    // -- configure authored facts --
    assert!(
        fixture
            .contains("fn configure(&mut self, _config: &SerialConfig) -> Result<(), RegseqError>"),
        "fixture must contain configure signature with _config: &SerialConfig"
    );

    // -- tx_byte authored facts --
    assert!(
        fixture.contains("fn tx_byte(&mut self, byte: u8)"),
        "fixture must contain tx_byte signature"
    );
    assert!(
        fixture.contains("if self.pending_tx.is_none() && self.tx_ready()"),
        "fixture tx_byte must contain branch condition"
    );
    assert!(
        fixture.contains("self.pending_tx = Some(byte)"),
        "fixture tx_byte must assign pending_tx = Some(byte)"
    );
    assert!(
        fixture.contains("self.enable_tx_interrupts()"),
        "fixture tx_byte must call enable_tx_interrupts"
    );

    // -- rx_char authored facts --
    assert!(
        fixture.contains("fn rx_char(&mut self) -> Option<RxChar>"),
        "fixture must contain rx_char signature"
    );
    assert!(
        fixture.contains("self.regs.fr.read(FR_REG::RXFE::FIELD) != 0"),
        "fixture rx_char must test FR.RXFE"
    );
    assert!(
        fixture.contains("let dr = self.regs.dr.read_raw()"),
        "fixture rx_char must perform one raw DR read"
    );
    assert!(
        fixture.contains("ErrorKind::OVERRUN"),
        "fixture rx_char must reference OVERRUN"
    );
    assert!(
        fixture.contains("ErrorKind::BREAK"),
        "fixture rx_char must reference BREAK"
    );
    assert!(
        fixture.contains("ErrorKind::PARITY"),
        "fixture rx_char must reference PARITY"
    );
    assert!(
        fixture.contains("ErrorKind::FRAMING"),
        "fixture rx_char must reference FRAMING"
    );
    assert!(
        fixture.contains("Some(RxChar::new(byte, errors))"),
        "fixture rx_char must return Some(RxChar::new(byte, errors))"
    );

    // -- specimen faithfully reproduces these facts --
    assert!(
        SPECIMEN.contains("fn port_open(&mut self) -> Result<(), RegseqError>"),
        "specimen must reproduce port_open signature"
    );
    assert!(
        SPECIMEN.contains("self.enable_clock_if_present()?;"),
        "specimen must reproduce enable_clock_if_present call in port_open"
    );
    assert!(
        SPECIMEN.contains("fn port_close(&mut self)"),
        "specimen must reproduce port_close signature"
    );
    assert!(
        SPECIMEN.contains("self.shutdown_port();"),
        "specimen must reproduce shutdown_port call"
    );
    assert!(
        SPECIMEN
            .contains("fn configure(&mut self, _config: &SerialConfig) -> Result<(), RegseqError>"),
        "specimen must reproduce configure signature"
    );
    assert!(
        SPECIMEN.contains("fn tx_byte(&mut self, byte: u8)"),
        "specimen must reproduce tx_byte signature"
    );
    assert!(
        SPECIMEN.contains("if self.pending_tx.is_none() && self.tx_ready()"),
        "specimen must reproduce tx_byte branch condition"
    );
    assert!(
        SPECIMEN.contains("fn rx_char(&mut self) -> Option<RxChar>"),
        "specimen must reproduce rx_char signature"
    );
    assert!(
        SPECIMEN.contains("Some(RxChar::new(byte, errors))"),
        "specimen must reproduce rx_char return"
    );
}

// ---------------------------------------------------------------------------
// Check Group 1: callback identity, trait role, signature, return type
// ---------------------------------------------------------------------------

#[test]
fn check_group_1_serial_trait_role_recognized() {
    let ext = compile_and_extract_all(SPECIMEN, "serial_cb_a_g1_trait");

    // TraitSurface: serial_impls must contain exactly one Serial impl
    assert_eq!(
        ext.trait_surface.serial_impls.len(),
        1,
        "exactly one Serial impl expected"
    );
    assert_eq!(
        ext.trait_surface.serial_impls[0].driver_type, "Pl011FullSpec",
        "Serial impl must be for Pl011FullSpec"
    );
}

#[test]
fn check_group_1_serial_impl_in_identity_enrichment() {
    let ext = compile_and_extract_all(SPECIMEN, "serial_cb_a_g1_identity");

    // IdentityEnrichmentSurface: must have an impl block for Serial
    let serial_impl = ext
        .identity
        .impl_blocks
        .iter()
        .find(|ib| ib.trait_name.as_deref() == Some("Serial"))
        .expect("Serial trait impl block must appear in identity enrichment");

    assert_eq!(
        serial_impl.self_type_name, "Pl011FullSpec",
        "Serial impl self type must be Pl011FullSpec"
    );

    // All five callbacks in scope must appear as methods
    let method_names: Vec<&str> = serial_impl
        .methods
        .iter()
        .map(|m| m.name.as_str())
        .collect();
    for cb in &["port_open", "port_close", "configure", "tx_byte", "rx_char"] {
        assert!(
            method_names.contains(cb),
            "Serial impl must contain method '{}'",
            cb
        );
    }
}

#[test]
fn check_group_1_callback_signatures_and_return_types() {
    let ext = compile_and_extract_all(SPECIMEN, "serial_cb_a_g1_sig");

    // port_open: receiver=ref_mut, params=[], return=Result<(), RegseqError>
    let port_open = find_fn(&ext.body, "port_open");
    assert_eq!(
        port_open.receiver, "ref_mut",
        "port_open receiver must be &mut self"
    );
    assert!(
        port_open.params.is_empty(),
        "port_open must have no extra params"
    );
    assert!(
        port_open
            .return_type
            .as_ref()
            .is_some_and(|r| r.contains("Result")),
        "port_open must return Result type, got: {:?}",
        port_open.return_type
    );
    assert!(
        matches!(
            &port_open.kind,
            MirFnKind::Callback { trait_name, method_name }
            if trait_name == "Serial" && method_name == "port_open"
        ),
        "port_open kind must be Callback {{ trait_name: Serial, method_name: port_open }}"
    );

    // port_close: receiver=ref_mut, params=[], return=None (unit)
    let port_close = find_fn(&ext.body, "port_close");
    assert_eq!(
        port_close.receiver, "ref_mut",
        "port_close receiver must be &mut self"
    );
    assert!(
        port_close.params.is_empty(),
        "port_close must have no extra params"
    );
    assert!(
        port_close.return_type.is_none(),
        "port_close must return unit (None), got: {:?}",
        port_close.return_type
    );
    assert!(
        matches!(
            &port_close.kind,
            MirFnKind::Callback { trait_name, method_name }
            if trait_name == "Serial" && method_name == "port_close"
        ),
        "port_close kind must be Callback {{ trait_name: Serial, method_name: port_close }}"
    );

    // configure: receiver=ref_mut, params=[_config: &SerialConfig], return=Result
    let configure = find_fn(&ext.body, "configure");
    assert_eq!(
        configure.receiver, "ref_mut",
        "configure receiver must be &mut self"
    );
    assert_eq!(
        configure.params.len(),
        1,
        "configure must have exactly 1 extra param"
    );
    assert!(
        configure.params[0].name == "_config",
        "configure param must be named _config, got: {}",
        configure.params[0].name
    );
    assert!(
        configure.params[0].ty.contains("SerialConfig"),
        "configure param type must reference SerialConfig, got: {}",
        configure.params[0].ty
    );
    assert!(
        configure
            .return_type
            .as_ref()
            .is_some_and(|r| r.contains("Result")),
        "configure must return Result type, got: {:?}",
        configure.return_type
    );
    assert!(
        matches!(
            &configure.kind,
            MirFnKind::Callback { trait_name, method_name }
            if trait_name == "Serial" && method_name == "configure"
        ),
        "configure kind must be Callback {{ trait_name: Serial, method_name: configure }}"
    );

    // tx_byte: receiver=ref_mut, params=[byte: u8], return=None
    let tx_byte = find_fn(&ext.body, "tx_byte");
    assert_eq!(
        tx_byte.receiver, "ref_mut",
        "tx_byte receiver must be &mut self"
    );
    assert_eq!(
        tx_byte.params.len(),
        1,
        "tx_byte must have exactly 1 extra param"
    );
    assert_eq!(
        tx_byte.params[0].name, "byte",
        "tx_byte param must be named byte"
    );
    assert!(
        tx_byte.params[0].ty.contains("u8"),
        "tx_byte param type must be u8, got: {}",
        tx_byte.params[0].ty
    );
    assert!(
        tx_byte.return_type.is_none(),
        "tx_byte must return unit, got: {:?}",
        tx_byte.return_type
    );
    assert!(
        matches!(
            &tx_byte.kind,
            MirFnKind::Callback { trait_name, method_name }
            if trait_name == "Serial" && method_name == "tx_byte"
        ),
        "tx_byte kind must be Callback {{ trait_name: Serial, method_name: tx_byte }}"
    );

    // rx_char: receiver=ref_mut, params=[], return=Option<RxChar>
    let rx_char = find_fn(&ext.body, "rx_char");
    assert_eq!(
        rx_char.receiver, "ref_mut",
        "rx_char receiver must be &mut self"
    );
    assert!(
        rx_char.params.is_empty(),
        "rx_char must have no extra params"
    );
    assert!(
        rx_char
            .return_type
            .as_ref()
            .is_some_and(|r| r.contains("Option")),
        "rx_char must return Option type, got: {:?}",
        rx_char.return_type
    );
    assert!(
        matches!(
            &rx_char.kind,
            MirFnKind::Callback { trait_name, method_name }
            if trait_name == "Serial" && method_name == "rx_char"
        ),
        "rx_char kind must be Callback {{ trait_name: Serial, method_name: rx_char }}"
    );
}

// ---------------------------------------------------------------------------
// Check Group 2: callback bodies preserve downstream checks
// ---------------------------------------------------------------------------

#[test]
fn check_group_2_port_open_body() {
    let ext = compile_and_extract_all(SPECIMEN, "serial_cb_a_g2_port_open");
    let f = find_fn(&ext.body, "port_open");

    // Helper call sequencing: enable_clock_if_present then program_polling_port
    assert!(
        f.helper_calls
            .contains(&"enable_clock_if_present".to_string()),
        "port_open must call enable_clock_if_present, found: {:?}",
        f.helper_calls
    );
    assert!(
        f.helper_calls.contains(&"program_polling_port".to_string()),
        "port_open must call program_polling_port, found: {:?}",
        f.helper_calls
    );

    // MIR body must contain calls to both helpers
    assert!(
        has_call_to(&f.body, "enable_clock_if_present"),
        "port_open MIR body must contain call to enable_clock_if_present"
    );
    assert!(
        has_call_to(&f.body, "program_polling_port"),
        "port_open MIR body must contain call to program_polling_port"
    );

    // Call ordering: enable_clock_if_present must appear before program_polling_port
    let calls = collect_call_names(&f.body);
    let clock_idx = calls
        .iter()
        .position(|c| c.contains("enable_clock_if_present"))
        .expect("enable_clock_if_present call must exist in MIR");
    let poll_idx = calls
        .iter()
        .position(|c| c.contains("program_polling_port"))
        .expect("program_polling_port call must exist in MIR");
    assert!(
        clock_idx < poll_idx,
        "enable_clock_if_present (index {}) must come before program_polling_port (index {})",
        clock_idx,
        poll_idx
    );

    // Success path returns Ok(())
    assert!(has_return(&f.body), "port_open MIR body must have a Return");

    // Also verify that uart_clock_hz is called (argument to program_polling_port)
    assert!(
        has_call_to(&f.body, "uart_clock_hz"),
        "port_open MIR body must call uart_clock_hz for the first argument"
    );
}

#[test]
fn check_group_2_port_close_body() {
    let ext = compile_and_extract_all(SPECIMEN, "serial_cb_a_g2_port_close");
    let f = find_fn(&ext.body, "port_close");

    // Helper call sequencing: shutdown_port then disable_clock_if_present
    assert!(
        f.helper_calls.contains(&"shutdown_port".to_string()),
        "port_close must call shutdown_port, found: {:?}",
        f.helper_calls
    );
    assert!(
        f.helper_calls
            .contains(&"disable_clock_if_present".to_string()),
        "port_close must call disable_clock_if_present, found: {:?}",
        f.helper_calls
    );

    // MIR body must contain calls to both helpers
    assert!(
        has_call_to(&f.body, "shutdown_port"),
        "port_close MIR body must contain call to shutdown_port"
    );
    assert!(
        has_call_to(&f.body, "disable_clock_if_present"),
        "port_close MIR body must contain call to disable_clock_if_present"
    );

    // Call ordering: shutdown_port must come before disable_clock_if_present
    let calls = collect_call_names(&f.body);
    let shutdown_idx = calls
        .iter()
        .position(|c| c.contains("shutdown_port"))
        .expect("shutdown_port call must exist in MIR");
    let disable_idx = calls
        .iter()
        .position(|c| c.contains("disable_clock_if_present"))
        .expect("disable_clock_if_present call must exist in MIR");
    assert!(
        shutdown_idx < disable_idx,
        "shutdown_port (index {}) must come before disable_clock_if_present (index {})",
        shutdown_idx,
        disable_idx
    );

    // No return value (unit)
    assert!(f.return_type.is_none(), "port_close must return unit");
}

#[test]
fn check_group_2_configure_body() {
    let ext = compile_and_extract_all(SPECIMEN, "serial_cb_a_g2_configure");
    let f = find_fn(&ext.body, "configure");

    // Opaque _config parameter preservation
    assert_eq!(f.params.len(), 1, "configure must have exactly 1 param");
    assert_eq!(
        f.params[0].name, "_config",
        "configure param name must be _config"
    );
    assert!(
        f.params[0].ty.contains("SerialConfig"),
        "configure param type must reference SerialConfig"
    );

    // Helper call sequencing: enable_clock_if_present then program_polling_port
    assert!(
        f.helper_calls
            .contains(&"enable_clock_if_present".to_string()),
        "configure must call enable_clock_if_present, found: {:?}",
        f.helper_calls
    );
    assert!(
        f.helper_calls.contains(&"program_polling_port".to_string()),
        "configure must call program_polling_port, found: {:?}",
        f.helper_calls
    );

    // MIR body must contain both calls
    assert!(
        has_call_to(&f.body, "enable_clock_if_present"),
        "configure MIR body must contain call to enable_clock_if_present"
    );
    assert!(
        has_call_to(&f.body, "program_polling_port"),
        "configure MIR body must contain call to program_polling_port"
    );

    // Call ordering preserved
    let calls = collect_call_names(&f.body);
    let clock_idx = calls
        .iter()
        .position(|c| c.contains("enable_clock_if_present"))
        .expect("enable_clock_if_present call must exist in MIR");
    let poll_idx = calls
        .iter()
        .position(|c| c.contains("program_polling_port"))
        .expect("program_polling_port call must exist in MIR");
    assert!(
        clock_idx < poll_idx,
        "enable_clock_if_present must come before program_polling_port"
    );

    // Returns Result (Ok(()))
    assert!(
        f.return_type.as_ref().is_some_and(|r| r.contains("Result")),
        "configure must return Result"
    );
    assert!(has_return(&f.body), "configure MIR body must have a Return");
}

#[test]
fn check_group_2_tx_byte_body() {
    let ext = compile_and_extract_all(SPECIMEN, "serial_cb_a_g2_tx_byte");
    let f = find_fn(&ext.body, "tx_byte");

    // Branch: must have SwitchInt (from the if condition)
    assert!(
        has_switch_int(&f.body),
        "tx_byte MIR body must have SwitchInt for the branch condition"
    );

    // Must call is_none (on pending_tx) and tx_ready
    assert!(
        has_call_to(&f.body, "is_none"),
        "tx_byte MIR body must call is_none (for pending_tx.is_none())"
    );
    assert!(
        has_call_to(&f.body, "tx_ready"),
        "tx_byte MIR body must call tx_ready"
    );

    // Fast path: writes DR via field_val
    assert!(
        has_call_to(&f.body, "field_val"),
        "tx_byte MIR body must call field_val for DR write"
    );
    assert!(
        has_call_to(&f.body, "write"),
        "tx_byte MIR body must call write (register write)"
    );

    // Staged path: calls enable_tx_interrupts
    assert!(
        f.helper_calls.contains(&"enable_tx_interrupts".to_string()),
        "tx_byte must call enable_tx_interrupts in staged path, found: {:?}",
        f.helper_calls
    );
    assert!(
        has_call_to(&f.body, "enable_tx_interrupts"),
        "tx_byte MIR body must contain call to enable_tx_interrupts"
    );

    // Staged path: assigns pending_tx = Some(byte)
    // This is evidenced by the body having a SetDiscriminant or Aggregate
    // for Option::Some. Check that the body mentions Option-construction.
    let has_option_some = f.body.basic_blocks.iter().any(|bb| {
        bb.statements.iter().any(|s| match &s.kind {
            MirStatementKind::Assign { rvalue, .. } => match rvalue.as_ref() {
                MirRvalue::Aggregate {
                    agg_kind:
                        MirAggregateKind::Adt {
                            path,
                            variant_index,
                            ..
                        },
                    ..
                } => path.contains("Option") && *variant_index == 1,
                _ => false,
            },
            MirStatementKind::SetDiscriminant { variant_index, .. } => *variant_index == 1,
            _ => false,
        })
    });
    assert!(
        has_option_some,
        "tx_byte MIR body must construct Option::Some for pending_tx assignment"
    );

    // Return type is unit
    assert!(f.return_type.is_none(), "tx_byte must return unit");
}

#[test]
fn check_group_2_rx_char_body() {
    let ext = compile_and_extract_all(SPECIMEN, "serial_cb_a_g2_rx_char");
    let f = find_fn(&ext.body, "rx_char");

    // Return type: Option<RxChar>
    assert!(
        f.return_type.as_ref().is_some_and(|r| r.contains("Option")),
        "rx_char must return Option type, got: {:?}",
        f.return_type
    );

    // Must have SwitchInt for the RXFE branch
    assert!(
        has_switch_int(&f.body),
        "rx_char MIR body must have SwitchInt for RXFE branch"
    );

    // Must call read for FR.RXFE check
    assert!(
        has_call_to(&f.body, "read"),
        "rx_char MIR body must call read (for FR.RXFE)"
    );

    // Must call read_raw for the DR read
    assert!(
        has_call_to(&f.body, "read_raw"),
        "rx_char MIR body must call read_raw (for DR read)"
    );

    // Must call field_from_raw for byte and error extraction
    let field_from_raw_count = count_calls_to(&f.body, "field_from_raw");
    assert!(
        field_from_raw_count >= 5,
        "rx_char must call field_from_raw at least 5 times (DATA + OE + BE + PE + FE), found: {}",
        field_from_raw_count
    );

    // Must call ErrorKind::empty
    assert!(
        has_call_to(&f.body, "empty"),
        "rx_char MIR body must call ErrorKind::empty"
    );

    // Must have bitwise-or operations for error accumulation (bitor calls)
    assert!(
        has_call_to(&f.body, "bitor"),
        "rx_char MIR body must call bitor for error accumulation"
    );

    // Must construct RxChar::new
    assert!(
        has_call_to(&f.body, "new"),
        "rx_char MIR body must call RxChar::new"
    );

    // Must construct Option::Some (for the return value)
    let has_option_some = f.body.basic_blocks.iter().any(|bb| {
        bb.statements.iter().any(|s| match &s.kind {
            MirStatementKind::Assign { rvalue, .. } => match rvalue.as_ref() {
                MirRvalue::Aggregate {
                    agg_kind:
                        MirAggregateKind::Adt {
                            path,
                            variant_index,
                            ..
                        },
                    ..
                } => path.contains("Option") && *variant_index == 1,
                _ => false,
            },
            MirStatementKind::SetDiscriminant { variant_index, .. } => *variant_index == 1,
            _ => false,
        })
    });
    assert!(
        has_option_some,
        "rx_char MIR body must construct Option::Some for return"
    );

    // Must have a Return terminator
    assert!(has_return(&f.body), "rx_char MIR body must have a Return");

    // Must have an Option::None construction (for empty-FIFO early return)
    let has_option_none = f.body.basic_blocks.iter().any(|bb| {
        bb.statements.iter().any(|s| match &s.kind {
            MirStatementKind::Assign { rvalue, .. } => match rvalue.as_ref() {
                MirRvalue::Aggregate {
                    agg_kind:
                        MirAggregateKind::Adt {
                            path,
                            variant_index,
                            ..
                        },
                    ..
                } => path.contains("Option") && *variant_index == 0,
                _ => false,
            },
            MirStatementKind::SetDiscriminant { variant_index, .. } => *variant_index == 0,
            _ => false,
        })
    });
    assert!(
        has_option_none,
        "rx_char MIR body must construct Option::None for empty-FIFO return"
    );
}
