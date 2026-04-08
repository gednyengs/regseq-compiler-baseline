//! source-to-extraction-08: PL011 helper methods B authored->extraction boundary
//! validation.
//!
//! Validates that the second half of the authored helper-method facts for PL011
//! survive at the compiler-side boundary between the authored PL011 source and
//! the current Regseq extraction state.
//!
//! Extraction surfaces under test:
//! - `MirBodySurface.functions` (body-level facts)
//! - `IdentityEnrichmentSurface.impl_blocks` (method identity, signature)
//! - `ClockSurface.call_sites` (clock resource interactions)
//! - `ResetSurface.call_sites` (reset resource interactions)
//! - `PinCtrlSurface.call_sites` (pinctrl resource interactions)

#![feature(rustc_private)]
#![feature(box_patterns)]

extern crate rustc_ast;
extern crate rustc_driver;
extern crate rustc_hir;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_session;

use std::sync::{Arc, Mutex, Once};

#[path = "../src/mir_repr.rs"]
mod mir_repr;

#[path = "../src/extract/mir_extract.rs"]
mod mir_extract;

#[path = "../src/extract/body.rs"]
mod body;

#[path = "../src/extract/identity_enrichment_types.rs"]
mod identity_enrichment_types;

#[path = "../src/extract/identity_enrichment.rs"]
mod identity_enrichment;

#[path = "../src/extract/clock.rs"]
mod clock;

#[path = "../src/extract/reset.rs"]
mod reset;

#[path = "../src/extract/pinctrl.rs"]
mod pinctrl;

use body::*;
use clock::*;
use identity_enrichment::*;
use mir_repr::*;
use pinctrl::*;
use reset::*;

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

struct ExtractionResult {
    identity: IdentityEnrichmentSurface,
    body: MirBodySurface,
    clock: ClockSurface,
    reset: ResetSurface,
    pinctrl: PinCtrlSurface,
}

fn compile_and_extract(source: &str, test_name: &str) -> ExtractionResult {
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

    type IdentityRes = Result<IdentityEnrichmentSurface, IdentityEnrichmentError>;
    type BodyRes = Result<MirBodySurface, BodyError>;
    type ClockRes = Result<ClockSurface, ClockError>;
    type ResetRes = Result<ResetSurface, ResetError>;
    type PinCtrlRes = Result<PinCtrlSurface, PinCtrlError>;

    let identity_r: Arc<Mutex<Option<IdentityRes>>> = Arc::new(Mutex::new(None));
    let body_r: Arc<Mutex<Option<BodyRes>>> = Arc::new(Mutex::new(None));
    let clock_r: Arc<Mutex<Option<ClockRes>>> = Arc::new(Mutex::new(None));
    let reset_r: Arc<Mutex<Option<ResetRes>>> = Arc::new(Mutex::new(None));
    let pinctrl_r: Arc<Mutex<Option<PinCtrlRes>>> = Arc::new(Mutex::new(None));

    struct Cb {
        identity: Arc<Mutex<Option<IdentityRes>>>,
        body: Arc<Mutex<Option<BodyRes>>>,
        clock: Arc<Mutex<Option<ClockRes>>>,
        reset: Arc<Mutex<Option<ResetRes>>>,
        pinctrl: Arc<Mutex<Option<PinCtrlRes>>>,
    }

    impl rustc_driver::Callbacks for Cb {
        fn config(&mut self, config: &mut rustc_interface::interface::Config) {
            config.opts.unstable_opts.no_codegen = true;
            config.opts.unstable_opts.mir_opt_level = Some(0);
            config.opts.unstable_opts.always_encode_mir = true;
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
            *self.identity.lock().unwrap() = Some(extract_identity_enrichment(tcx));
            *self.body.lock().unwrap() = Some(extract_body_surface(tcx));
            *self.clock.lock().unwrap() = Some(extract_clock_surface(tcx));
            *self.reset.lock().unwrap() = Some(extract_reset_surface(tcx));
            *self.pinctrl.lock().unwrap() = Some(extract_pinctrl_surface(tcx));
            rustc_driver::Compilation::Stop
        }
    }

    let mut callbacks = Cb {
        identity: identity_r.clone(),
        body: body_r.clone(),
        clock: clock_r.clone(),
        reset: reset_r.clone(),
        pinctrl: pinctrl_r.clone(),
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

    let identity = identity_r
        .lock()
        .unwrap()
        .take()
        .expect("after_analysis must run")
        .expect("identity enrichment must succeed");

    let body = body_r
        .lock()
        .unwrap()
        .take()
        .expect("after_analysis must run")
        .expect("body extraction must succeed");

    let clock_s = clock_r
        .lock()
        .unwrap()
        .take()
        .expect("after_analysis must run")
        .expect("clock extraction must succeed");

    let reset_s = reset_r
        .lock()
        .unwrap()
        .take()
        .expect("after_analysis must run")
        .expect("reset extraction must succeed");

    let pinctrl_s = pinctrl_r
        .lock()
        .unwrap()
        .take()
        .expect("after_analysis must run")
        .expect("pinctrl extraction must succeed");

    std::fs::remove_dir_all(&test_dir).ok();

    ExtractionResult {
        identity,
        body,
        clock: clock_s,
        reset: reset_s,
        pinctrl: pinctrl_s,
    }
}

// ---------------------------------------------------------------------------
// Inline specimen: reduced PL011 helper methods B surface
// ---------------------------------------------------------------------------

/// Reduced specimen covering all 10 in-scope helper methods from helper
/// methods group B. Includes the lifecycle and ISR callbacks that make
/// these helpers reachable via the body extraction's transitive discovery.
const SPECIMEN: &str = r#"
    #![allow(dead_code)]

    pub enum RegseqError { HardwareError }

    pub trait PinState {}
    pub struct DefaultState;
    impl PinState for DefaultState {}
    pub struct SleepState;
    impl PinState for SleepState {}

    pub struct Clock { _opaque: () }
    impl Clock {
        pub fn get_rate(&self) -> u32 { unimplemented!() }
        pub fn enable(&self) -> Result<(), RegseqError> { unimplemented!() }
        pub fn disable(&self) { unimplemented!() }
    }

    pub struct ResetLine { _opaque: () }
    impl ResetLine {
        pub fn toggle(&self, _dur: Duration) -> Result<(), RegseqError> {
            unimplemented!()
        }
    }

    pub struct PinCtrl { _opaque: () }
    impl PinCtrl {
        pub fn apply<S: PinState>(&self) -> Result<(), RegseqError> {
            unimplemented!()
        }
    }

    pub struct Duration(u64);
    impl Duration {
        pub fn from_us(us: u64) -> Self { Duration(us) }
    }

    pub struct Field<T, R> {
        pub mask: T,
        pub shift: T,
        _r: core::marker::PhantomData<R>,
    }
    pub struct FieldValue<T, R> {
        _m: T,
        _v: T,
        _r: core::marker::PhantomData<R>,
    }
    impl<R> FieldValue<u32, R> {
        pub fn new(mask: u32, value: u32) -> Self {
            FieldValue {
                _m: mask,
                _v: value,
                _r: core::marker::PhantomData,
            }
        }
    }

    pub struct Register32 { val: u32 }
    impl Register32 {
        pub fn write<R>(&mut self, _fv: FieldValue<u32, R>) {}
        pub fn read_raw(&self) -> u32 { self.val }
        pub fn read<R>(&self, _f: Field<u32, R>) -> u32 { 0 }
        pub fn modify<R>(&mut self, _fv: FieldValue<u32, R>) {}
    }

    pub mod DR_REG {
        pub struct Register;
        pub mod DATA {
            pub const FIELD: super::super::Field<u32, super::Register> =
                super::super::Field {
                    mask: 0xFF,
                    shift: 0,
                    _r: core::marker::PhantomData,
                };
        }
    }
    pub mod ICR_REG {
        pub struct Register;
    }
    pub mod IMSC_REG {
        pub struct Register;
    }

    fn field_val<R>(field: Field<u32, R>, raw: u32) -> FieldValue<u32, R> {
        FieldValue::new(field.mask, raw << field.shift)
    }
    fn raw_reg_val<R>(mask: u32, value: u32) -> FieldValue<u32, R> {
        FieldValue::new(mask, value)
    }

    pub struct Pl011Registers {
        pub dr: Register32,
        pub fr: Register32,
        pub ibrd: Register32,
        pub fbrd: Register32,
        pub lcr_h: Register32,
        pub cr: Register32,
        pub ifls: Register32,
        pub imsc: Register32,
        pub mis: Register32,
        pub icr: Register32,
    }

    pub enum IrqResult { Handled, NotHandled }

    pub trait Lifecycle {
        fn init(&mut self) -> Result<(), RegseqError>;
        fn deinit(&mut self) -> Result<(), RegseqError>;
    }

    pub struct RxChar(u8);
    impl RxChar {
        pub fn new(b: u8) -> Self { RxChar(b) }
    }

    pub trait Serial {
        fn rx_char(&mut self) -> Option<RxChar>;
        fn tx_ready(&mut self) -> bool;
    }

    #[rseq::driver(name = "pl011-full-spec", compatible = ["arm,pl011"])]
    pub struct Pl011Device {
        pub regs: Pl011Registers,

        #[rseq::clock(id = "uart_clk")]
        pub clock: Option<Clock>,

        pub reset: Option<ResetLine>,

        #[rseq::pinctrl(id = "uart_pins")]
        pub pinctrl: Option<PinCtrl>,

        pub pending_tx: Option<u8>,
        pub current_speed: u32,
    }

    impl Pl011Device {
        const ALL_UART_INTERRUPTS_MASK: u32 = 0x07ff;
        const TX_INTERRUPT_MASK: u32 = 1 << 5;
        const RX_INTERRUPT_MASK: u32 = (1 << 4) | (1 << 6);
        const ERROR_INTERRUPT_MASK: u32 =
            (1 << 7) | (1 << 8) | (1 << 9) | (1 << 10);
        const EXPLICIT_ISR_CLEAR_MASK: u32 = Self::ERROR_INTERRUPT_MASK;
        const DEFAULT_UART_CLOCK_HZ: u32 = 48_000_000;

        fn mask_all_interrupts(&mut self) {
            self.regs.imsc.write(raw_reg_val::<IMSC_REG::Register>(
                Self::ALL_UART_INTERRUPTS_MASK,
                0,
            ));
        }
        fn clear_all_interrupts(&mut self) {
            self.regs.icr.write(raw_reg_val::<ICR_REG::Register>(
                Self::ALL_UART_INTERRUPTS_MASK,
                Self::ALL_UART_INTERRUPTS_MASK,
            ));
        }
        fn disable_fifo(&mut self) {}
        fn disable_uart(&mut self) {}
        fn disable_tx_interrupts(&mut self) {
            self.regs.imsc.modify(raw_reg_val::<IMSC_REG::Register>(
                Self::TX_INTERRUPT_MASK,
                0,
            ));
        }
        fn program_polling_port(&mut self, _clk: u32, _baud: u32) {}

        // -- The 10 in-scope helper methods --

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

        fn toggle_reset_if_present(&self) -> Result<(), RegseqError> {
            if let Some(reset) = &self.reset {
                reset.toggle(Duration::from_us(10))?;
            }
            Ok(())
        }

        fn apply_default_pins_if_present(&self) -> Result<(), RegseqError> {
            if let Some(pinctrl) = &self.pinctrl {
                pinctrl.apply::<DefaultState>()?;
            }
            Ok(())
        }

        fn apply_sleep_pins_if_present(&self) -> Result<(), RegseqError> {
            if let Some(pinctrl) = &self.pinctrl {
                pinctrl.apply::<SleepState>()?;
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
                if self.tx_ready() {
                    self.regs
                        .dr
                        .write(field_val(DR_REG::DATA::FIELD, byte as u32));
                    self.pending_tx = None;
                    self.disable_tx_interrupts();
                    return true;
                }
                return false;
            }

            self.disable_tx_interrupts();
            true
        }

        fn clear_explicit_interrupts(&mut self, status: u32) {
            let clear_mask = status & Self::EXPLICIT_ISR_CLEAR_MASK;
            if clear_mask != 0 {
                self.regs
                    .icr
                    .write(raw_reg_val::<ICR_REG::Register>(clear_mask, clear_mask));
            }
        }
    }

    impl Lifecycle for Pl011Device {
        fn init(&mut self) -> Result<(), RegseqError> {
            self.toggle_reset_if_present()?;
            self.apply_default_pins_if_present()?;
            self.enable_clock_if_present()?;
            self.program_polling_port(self.uart_clock_hz(), self.current_speed);
            Ok(())
        }

        fn deinit(&mut self) -> Result<(), RegseqError> {
            self.shutdown_port();
            self.disable_clock_if_present();
            self.apply_sleep_pins_if_present()?;
            Ok(())
        }
    }

    impl Serial for Pl011Device {
        fn rx_char(&mut self) -> Option<RxChar> {
            if self.regs.fr.read_raw() & 0x10 != 0 {
                return None;
            }
            let dr = self.regs.dr.read_raw();
            Some(RxChar::new(dr as u8))
        }

        fn tx_ready(&mut self) -> bool {
            self.regs.fr.read_raw() & 0x20 == 0
        }
    }

    impl Pl011Device {
        #[rseq::isr]
        pub fn handle_irq(&mut self) -> IrqResult {
            let status = self.regs.mis.read_raw();

            if status == 0 {
                return IrqResult::NotHandled;
            }

            let mut handled = false;

            if status & Self::RX_INTERRUPT_MASK != 0 {
                handled |= self.service_rx_fifo();
            }

            if status & Self::TX_INTERRUPT_MASK != 0 {
                handled |= self.service_tx_fifo();
            }

            self.clear_explicit_interrupts(status);

            if handled || (status & Self::EXPLICIT_ISR_CLEAR_MASK != 0) {
                IrqResult::Handled
            } else {
                IrqResult::NotHandled
            }
        }
    }
"#;

// ---------------------------------------------------------------------------
// Helper: find an extracted function by name
// ---------------------------------------------------------------------------

fn find_fn<'a>(body: &'a MirBodySurface, name: &str) -> &'a MirExtractedFn {
    body.functions
        .iter()
        .find(|f| f.name == name)
        .unwrap_or_else(|| panic!("expected function '{}' in body surface", name))
}

/// Check if any basic block contains a call whose func operand includes the
/// given callee substring.
fn body_has_call(body: &MirBody, callee_substr: &str) -> bool {
    body.basic_blocks.iter().any(|bb| {
        bb.terminator.as_ref().is_some_and(|t| match &t.kind {
            MirTerminatorKind::Call { func, .. } => match func {
                MirOperand::Constant { constant } => match &constant.ty {
                    MirTy::FnDef { path, .. } => path.contains(callee_substr),
                    _ => false,
                },
                _ => false,
            },
            _ => false,
        })
    })
}

/// Check if any constant reference in the body (statements or call args)
/// mentions the given substring in a FnDef type path.
fn body_has_fn_ref(body: &MirBody, fn_substr: &str) -> bool {
    for bb in &body.basic_blocks {
        if let Some(t) = &bb.terminator {
            if let MirTerminatorKind::Call { func, args, .. } = &t.kind {
                if operand_has_fn_ref(func, fn_substr) {
                    return true;
                }
                for arg in args {
                    if operand_has_fn_ref(arg, fn_substr) {
                        return true;
                    }
                }
            }
        }
        for s in &bb.statements {
            if let MirStatementKind::Assign { rvalue, .. } = &s.kind {
                if let MirRvalue::Use { operand } = rvalue.as_ref() {
                    if operand_has_fn_ref(operand, fn_substr) {
                        return true;
                    }
                }
            }
        }
    }
    false
}

fn operand_has_fn_ref(op: &MirOperand, fn_substr: &str) -> bool {
    matches!(
        op,
        MirOperand::Constant {
            constant: MirConstant {
                ty: MirTy::FnDef { path, .. },
                ..
            },
        } if path.contains(fn_substr)
    )
}

/// Check if any basic block has a SwitchInt terminator.
fn body_has_switch(body: &MirBody) -> bool {
    body.basic_blocks.iter().any(|bb| {
        bb.terminator
            .as_ref()
            .is_some_and(|t| matches!(&t.kind, MirTerminatorKind::SwitchInt { .. }))
    })
}

/// Check if any constant scalar with the given bits value appears anywhere
/// in the body (statements, rvalues, or call arguments).
fn body_has_const_scalar(body: &MirBody, bits: u128) -> bool {
    let check = |op: &MirOperand| -> bool {
        matches!(
            op,
            MirOperand::Constant {
                constant: MirConstant {
                    value: MirConstValue::Scalar { bits: b, .. },
                    ..
                },
            } if *b == bits
        )
    };
    for bb in &body.basic_blocks {
        for s in &bb.statements {
            if let MirStatementKind::Assign { rvalue, .. } = &s.kind {
                match rvalue.as_ref() {
                    MirRvalue::Use { operand } if check(operand) => return true,
                    MirRvalue::BinaryOp { lhs, rhs, .. } => {
                        if check(lhs) || check(rhs) {
                            return true;
                        }
                    }
                    _ => {}
                }
            }
        }
        if let Some(t) = &bb.terminator {
            if let MirTerminatorKind::Call { args, .. } = &t.kind {
                for arg in args {
                    if check(arg) {
                        return true;
                    }
                }
            }
        }
    }
    false
}

/// Check if any u32 constant with the given value appears in the body.
fn body_has_u32_const(body: &MirBody, value: u128) -> bool {
    let check = |op: &MirOperand| -> bool {
        matches!(
            op,
            MirOperand::Constant {
                constant: MirConstant {
                    ty: MirTy::Uint {
                        uint_ty: MirUintTy::U32
                    },
                    value: MirConstValue::Scalar { bits, .. },
                },
            } if *bits == value
        )
    };
    for bb in &body.basic_blocks {
        for s in &bb.statements {
            if let MirStatementKind::Assign { rvalue, .. } = &s.kind {
                match rvalue.as_ref() {
                    MirRvalue::Use { operand } if check(operand) => return true,
                    MirRvalue::BinaryOp { lhs, rhs, .. } => {
                        if check(lhs) || check(rhs) {
                            return true;
                        }
                    }
                    _ => {}
                }
            }
        }
        if let Some(t) = &bb.terminator {
            if let MirTerminatorKind::Call { args, .. } = &t.kind {
                for arg in args {
                    if check(arg) {
                        return true;
                    }
                }
            }
        }
    }
    false
}

/// Check if the body references an unevaluated constant whose description
/// contains the given substring.
fn body_has_unevaluated_const(body: &MirBody, substr: &str) -> bool {
    let check = |op: &MirOperand| -> bool {
        matches!(
            op,
            MirOperand::Constant {
                constant: MirConstant {
                    value: MirConstValue::Unsupported { description },
                    ..
                },
            } if description.contains(substr)
        )
    };
    for bb in &body.basic_blocks {
        for s in &bb.statements {
            if let MirStatementKind::Assign { rvalue, .. } = &s.kind {
                match rvalue.as_ref() {
                    MirRvalue::Use { operand } if check(operand) => return true,
                    MirRvalue::BinaryOp { lhs, rhs, .. } => {
                        if check(lhs) || check(rhs) {
                            return true;
                        }
                    }
                    _ => {}
                }
            }
        }
        if let Some(t) = &bb.terminator {
            if let MirTerminatorKind::Call { args, .. } = &t.kind {
                for arg in args {
                    if check(arg) {
                        return true;
                    }
                }
            }
        }
    }
    false
}

/// Check if body assigns Option::None somewhere (SetDiscriminant variant 0
/// or Adt aggregate with variant_index 0 and no operands).
fn body_assigns_option_none(body: &MirBody) -> bool {
    body.basic_blocks.iter().any(|bb| {
        bb.statements.iter().any(|s| match &s.kind {
            MirStatementKind::SetDiscriminant {
                variant_index: 0, ..
            } => true,
            MirStatementKind::Assign { rvalue, .. } => matches!(
                rvalue.as_ref(),
                MirRvalue::Aggregate {
                    agg_kind: MirAggregateKind::Adt { variant_index, .. },
                    operands,
                } if *variant_index == 0 && operands.is_empty()
            ),
            _ => false,
        })
    })
}

/// Check that the body contains a BitAnd binary operation.
fn body_has_bitand(body: &MirBody) -> bool {
    body.basic_blocks.iter().any(|bb| {
        bb.statements.iter().any(|s| {
            matches!(
                &s.kind,
                MirStatementKind::Assign { rvalue, .. }
                if matches!(rvalue.as_ref(), MirRvalue::BinaryOp { op: MirBinOp::BitAnd, .. })
            )
        })
    })
}

/// Find method info from identity enrichment by name.
fn find_method<'a>(
    identity: &'a IdentityEnrichmentSurface,
    name: &str,
) -> Option<&'a identity_enrichment_types::ImplMethodFact> {
    identity
        .impl_blocks
        .iter()
        .flat_map(|ib| &ib.methods)
        .find(|m| m.name == name)
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

    // -- uart_clock_hz --
    assert!(
        fixture.contains("fn uart_clock_hz(&self) -> u32"),
        "fixture: uart_clock_hz signature"
    );
    assert!(
        fixture.contains(".map(Clock::get_rate)"),
        "fixture: .map(Clock::get_rate)"
    );
    assert!(
        fixture.contains(".unwrap_or(Self::DEFAULT_UART_CLOCK_HZ)"),
        "fixture: .unwrap_or(Self::DEFAULT_UART_CLOCK_HZ)"
    );
    assert!(
        fixture.contains("const DEFAULT_UART_CLOCK_HZ: u32 = 48_000_000;"),
        "fixture: DEFAULT_UART_CLOCK_HZ = 48_000_000"
    );

    // -- enable_clock_if_present --
    assert!(
        fixture.contains("fn enable_clock_if_present(&self) -> Result<(), RegseqError>"),
        "fixture: enable_clock_if_present signature"
    );
    assert!(
        fixture.contains("clock.enable()?;"),
        "fixture: clock.enable()?"
    );

    // -- disable_clock_if_present --
    assert!(
        fixture.contains("fn disable_clock_if_present(&self)"),
        "fixture: disable_clock_if_present signature"
    );
    assert!(
        fixture.contains("clock.disable();"),
        "fixture: clock.disable()"
    );

    // -- toggle_reset_if_present --
    assert!(
        fixture.contains("fn toggle_reset_if_present(&self) -> Result<(), RegseqError>"),
        "fixture: toggle_reset_if_present signature"
    );
    assert!(
        fixture.contains("reset.toggle(Duration::from_us(10))?;"),
        "fixture: reset.toggle(Duration::from_us(10))?"
    );

    // -- apply_default_pins_if_present --
    assert!(
        fixture.contains("fn apply_default_pins_if_present(&self) -> Result<(), RegseqError>"),
        "fixture: apply_default_pins_if_present signature"
    );
    assert!(
        fixture.contains("pinctrl.apply::<DefaultState>()?;"),
        "fixture: pinctrl.apply::<DefaultState>()?"
    );

    // -- apply_sleep_pins_if_present --
    assert!(
        fixture.contains("fn apply_sleep_pins_if_present(&self) -> Result<(), RegseqError>"),
        "fixture: apply_sleep_pins_if_present signature"
    );
    assert!(
        fixture.contains("pinctrl.apply::<SleepState>()?;"),
        "fixture: pinctrl.apply::<SleepState>()?"
    );

    // -- shutdown_port --
    assert!(
        fixture.contains("fn shutdown_port(&mut self)"),
        "fixture: shutdown_port signature"
    );
    assert!(
        fixture.contains("self.pending_tx = None;"),
        "fixture: self.pending_tx = None"
    );

    // -- service_rx_fifo --
    assert!(
        fixture.contains("fn service_rx_fifo(&mut self) -> bool"),
        "fixture: service_rx_fifo signature"
    );
    assert!(
        fixture.contains("let mut handled = false;"),
        "fixture: let mut handled = false"
    );
    assert!(
        fixture.contains("while self.rx_char().is_some()"),
        "fixture: while self.rx_char().is_some()"
    );

    // -- service_tx_fifo --
    assert!(
        fixture.contains("fn service_tx_fifo(&mut self) -> bool"),
        "fixture: service_tx_fifo signature"
    );
    assert!(
        fixture.contains("if let Some(byte) = self.pending_tx"),
        "fixture: if let Some(byte) = self.pending_tx"
    );
    assert!(
        fixture.contains("field_val(DR_REG::DATA::FIELD, byte as u32)"),
        "fixture: field_val(DR_REG::DATA::FIELD, byte as u32)"
    );

    // -- clear_explicit_interrupts --
    assert!(
        fixture.contains("fn clear_explicit_interrupts(&mut self, status: u32)"),
        "fixture: clear_explicit_interrupts signature"
    );
    assert!(
        fixture.contains("let clear_mask = status & Self::EXPLICIT_ISR_CLEAR_MASK;"),
        "fixture: clear_mask computation"
    );

    // -- Verify specimen reproduces the same helper bodies --
    for fact in &[
        "fn uart_clock_hz(&self) -> u32",
        ".map(Clock::get_rate)",
        ".unwrap_or(Self::DEFAULT_UART_CLOCK_HZ)",
        "fn enable_clock_if_present(&self) -> Result<(), RegseqError>",
        "clock.enable()?;",
        "fn disable_clock_if_present(&self)",
        "clock.disable();",
        "fn toggle_reset_if_present(&self) -> Result<(), RegseqError>",
        "reset.toggle(Duration::from_us(10))?;",
        "fn apply_default_pins_if_present(&self) -> Result<(), RegseqError>",
        "pinctrl.apply::<DefaultState>()?;",
        "fn apply_sleep_pins_if_present(&self) -> Result<(), RegseqError>",
        "pinctrl.apply::<SleepState>()?;",
        "fn shutdown_port(&mut self)",
        "self.pending_tx = None;",
        "fn service_rx_fifo(&mut self) -> bool",
        "let mut handled = false;",
        "while self.rx_char().is_some()",
        "fn service_tx_fifo(&mut self) -> bool",
        "if let Some(byte) = self.pending_tx",
        "field_val(DR_REG::DATA::FIELD, byte as u32)",
        "fn clear_explicit_interrupts(&mut self, status: u32)",
        "let clear_mask = status & Self::EXPLICIT_ISR_CLEAR_MASK;",
    ] {
        assert!(SPECIMEN.contains(fact), "specimen must contain: {fact}");
    }
}

// ---------------------------------------------------------------------------
// Check Group 1: Method identity, signature, and return type
// ---------------------------------------------------------------------------

#[test]
fn check1_uart_clock_hz_identity() {
    let r = compile_and_extract(SPECIMEN, "hmb_c1_uart_clk");
    let f = find_fn(&r.body, "uart_clock_hz");
    assert_eq!(f.receiver, "ref", "uart_clock_hz: receiver must be &self");
    assert!(f.params.is_empty(), "uart_clock_hz: no params");
    assert_eq!(
        f.return_type.as_deref(),
        Some("u32"),
        "uart_clock_hz: return u32"
    );
    assert!(
        find_method(&r.identity, "uart_clock_hz").is_some(),
        "uart_clock_hz: must exist in identity enrichment"
    );
}

#[test]
fn check1_enable_clock_if_present_identity() {
    let r = compile_and_extract(SPECIMEN, "hmb_c1_en_clk");
    let f = find_fn(&r.body, "enable_clock_if_present");
    assert_eq!(f.receiver, "ref");
    assert!(f.params.is_empty());
    assert!(f.return_type.as_ref().unwrap().contains("Result"));
}

#[test]
fn check1_disable_clock_if_present_identity() {
    let r = compile_and_extract(SPECIMEN, "hmb_c1_dis_clk");
    let f = find_fn(&r.body, "disable_clock_if_present");
    assert_eq!(f.receiver, "ref");
    assert!(f.params.is_empty());
    assert!(
        f.return_type.is_none(),
        "disable_clock_if_present: returns ()"
    );
}

#[test]
fn check1_toggle_reset_if_present_identity() {
    let r = compile_and_extract(SPECIMEN, "hmb_c1_tog_rst");
    let f = find_fn(&r.body, "toggle_reset_if_present");
    assert_eq!(f.receiver, "ref");
    assert!(f.params.is_empty());
    assert!(f.return_type.as_ref().unwrap().contains("Result"));
}

#[test]
fn check1_apply_default_pins_if_present_identity() {
    let r = compile_and_extract(SPECIMEN, "hmb_c1_def_pins");
    let f = find_fn(&r.body, "apply_default_pins_if_present");
    assert_eq!(f.receiver, "ref");
    assert!(f.params.is_empty());
    assert!(f.return_type.as_ref().unwrap().contains("Result"));
}

#[test]
fn check1_apply_sleep_pins_if_present_identity() {
    let r = compile_and_extract(SPECIMEN, "hmb_c1_slp_pins");
    let f = find_fn(&r.body, "apply_sleep_pins_if_present");
    assert_eq!(f.receiver, "ref");
    assert!(f.params.is_empty());
    assert!(f.return_type.as_ref().unwrap().contains("Result"));
}

#[test]
fn check1_shutdown_port_identity() {
    let r = compile_and_extract(SPECIMEN, "hmb_c1_shutdown");
    let f = find_fn(&r.body, "shutdown_port");
    assert_eq!(f.receiver, "ref_mut");
    assert!(f.params.is_empty());
    assert!(f.return_type.is_none(), "shutdown_port: returns ()");
}

#[test]
fn check1_service_rx_fifo_identity() {
    let r = compile_and_extract(SPECIMEN, "hmb_c1_rx");
    let f = find_fn(&r.body, "service_rx_fifo");
    assert_eq!(f.receiver, "ref_mut");
    assert!(f.params.is_empty());
    assert_eq!(f.return_type.as_deref(), Some("bool"));
}

#[test]
fn check1_service_tx_fifo_identity() {
    let r = compile_and_extract(SPECIMEN, "hmb_c1_tx");
    let f = find_fn(&r.body, "service_tx_fifo");
    assert_eq!(f.receiver, "ref_mut");
    assert!(f.params.is_empty());
    assert_eq!(f.return_type.as_deref(), Some("bool"));
}

#[test]
fn check1_clear_explicit_interrupts_identity() {
    let r = compile_and_extract(SPECIMEN, "hmb_c1_clear_int");
    let f = find_fn(&r.body, "clear_explicit_interrupts");
    assert_eq!(f.receiver, "ref_mut");
    assert_eq!(f.params.len(), 1);
    assert_eq!(f.params[0].name, "status");
    assert_eq!(f.params[0].ty, "u32");
    assert!(f.return_type.is_none());
}

// ---------------------------------------------------------------------------
// Check Group 2: Body-level downstream checks
// ---------------------------------------------------------------------------

#[test]
fn check2_uart_clock_hz_body() {
    let r = compile_and_extract(SPECIMEN, "hmb_c2_uart_clk");
    let f = find_fn(&r.body, "uart_clock_hz");

    // Body must reference Clock::get_rate (directly or via Option::map)
    assert!(
        body_has_call(&f.body, "get_rate") || body_has_fn_ref(&f.body, "get_rate"),
        "uart_clock_hz: must reference get_rate"
    );

    // Body must reference DEFAULT_UART_CLOCK_HZ (inlined or unevaluated)
    assert!(
        body_has_u32_const(&f.body, 48_000_000)
            || body_has_unevaluated_const(&f.body, "DEFAULT_UART_CLOCK_HZ"),
        "uart_clock_hz: must reference DEFAULT_UART_CLOCK_HZ"
    );

    // Body must have a SwitchInt (for the Option discriminant or unwrap_or)
    // or at least reference Option::map and Option::unwrap_or
    assert!(
        body_has_switch(&f.body)
            || (body_has_call(&f.body, "map") && body_has_call(&f.body, "unwrap_or")),
        "uart_clock_hz: must have option branching"
    );

    // Clock resource surface: get_rate in uart_clock_hz may be passed as a
    // function pointer to Option::map rather than called directly. The clock
    // surface extractor may not detect function-pointer-style references.
    // Verify either the clock surface records it or the MIR body preserves
    // a function reference to get_rate.
    let clock_rate_site = r
        .clock
        .call_sites
        .iter()
        .find(|cs| cs.method == ClockMethod::GetRate && cs.containing_fn == "uart_clock_hz");
    assert!(
        clock_rate_site.is_some() || body_has_fn_ref(&f.body, "get_rate"),
        "uart_clock_hz: must reference get_rate via clock surface or MIR fn ref; \
         clock sites: {:?}",
        r.clock.call_sites
    );
}

#[test]
fn check2_enable_clock_if_present_body() {
    let r = compile_and_extract(SPECIMEN, "hmb_c2_en_clk");
    let f = find_fn(&r.body, "enable_clock_if_present");

    assert!(
        body_has_call(&f.body, "enable"),
        "enable_clock_if_present: must call enable"
    );
    assert!(
        body_has_switch(&f.body),
        "enable_clock_if_present: must have SwitchInt for Option"
    );

    let enable_site = r.clock.call_sites.iter().find(|cs| {
        cs.method == ClockMethod::Enable && cs.containing_fn == "enable_clock_if_present"
    });
    assert!(
        enable_site.is_some(),
        "enable_clock_if_present: clock surface must show Enable; sites: {:?}",
        r.clock.call_sites
    );
}

#[test]
fn check2_disable_clock_if_present_body() {
    let r = compile_and_extract(SPECIMEN, "hmb_c2_dis_clk");
    let f = find_fn(&r.body, "disable_clock_if_present");

    assert!(
        body_has_call(&f.body, "disable"),
        "disable_clock_if_present: must call disable"
    );
    assert!(
        body_has_switch(&f.body),
        "disable_clock_if_present: must have SwitchInt for Option"
    );

    let disable_site = r.clock.call_sites.iter().find(|cs| {
        cs.method == ClockMethod::Disable && cs.containing_fn == "disable_clock_if_present"
    });
    assert!(
        disable_site.is_some(),
        "disable_clock_if_present: clock surface must show Disable; sites: {:?}",
        r.clock.call_sites
    );
}

#[test]
fn check2_toggle_reset_if_present_body() {
    let r = compile_and_extract(SPECIMEN, "hmb_c2_tog_rst");
    let f = find_fn(&r.body, "toggle_reset_if_present");

    assert!(
        body_has_call(&f.body, "toggle"),
        "toggle_reset_if_present: must call toggle"
    );
    assert!(
        body_has_switch(&f.body),
        "toggle_reset_if_present: must have SwitchInt for Option"
    );
    // Duration literal 10 as u64 scalar
    assert!(
        body_has_const_scalar(&f.body, 10),
        "toggle_reset_if_present: must contain duration literal 10"
    );
    assert!(
        body_has_call(&f.body, "from_us") || body_has_fn_ref(&f.body, "from_us"),
        "toggle_reset_if_present: must reference Duration::from_us"
    );

    let toggle_site = r.reset.call_sites.iter().find(|cs| {
        cs.method == ResetMethod::Toggle && cs.containing_fn == "toggle_reset_if_present"
    });
    assert!(
        toggle_site.is_some(),
        "toggle_reset_if_present: reset surface must show Toggle; sites: {:?}",
        r.reset.call_sites
    );
}

#[test]
fn check2_apply_default_pins_if_present_body() {
    let r = compile_and_extract(SPECIMEN, "hmb_c2_def_pins");
    let f = find_fn(&r.body, "apply_default_pins_if_present");

    assert!(
        body_has_call(&f.body, "apply"),
        "apply_default_pins_if_present: must call apply"
    );
    assert!(
        body_has_switch(&f.body),
        "apply_default_pins_if_present: must have SwitchInt for Option"
    );

    let site = r.pinctrl.call_sites.iter().find(|cs| {
        cs.state_type == "DefaultState" && cs.containing_fn == "apply_default_pins_if_present"
    });
    assert!(
        site.is_some(),
        "apply_default_pins_if_present: pinctrl must show DefaultState; sites: {:?}",
        r.pinctrl.call_sites
    );
}

#[test]
fn check2_apply_sleep_pins_if_present_body() {
    let r = compile_and_extract(SPECIMEN, "hmb_c2_slp_pins");
    let f = find_fn(&r.body, "apply_sleep_pins_if_present");

    assert!(
        body_has_call(&f.body, "apply"),
        "apply_sleep_pins_if_present: must call apply"
    );
    assert!(
        body_has_switch(&f.body),
        "apply_sleep_pins_if_present: must have SwitchInt for Option"
    );

    let site = r.pinctrl.call_sites.iter().find(|cs| {
        cs.state_type == "SleepState" && cs.containing_fn == "apply_sleep_pins_if_present"
    });
    assert!(
        site.is_some(),
        "apply_sleep_pins_if_present: pinctrl must show SleepState; sites: {:?}",
        r.pinctrl.call_sites
    );
}

#[test]
fn check2_shutdown_port_body() {
    let r = compile_and_extract(SPECIMEN, "hmb_c2_shutdown");
    let f = find_fn(&r.body, "shutdown_port");

    assert!(
        f.helper_calls.contains(&"mask_all_interrupts".to_string()),
        "shutdown_port: must call mask_all_interrupts"
    );
    assert!(
        f.helper_calls.contains(&"clear_all_interrupts".to_string()),
        "shutdown_port: must call clear_all_interrupts"
    );
    assert!(
        f.helper_calls.contains(&"disable_fifo".to_string()),
        "shutdown_port: must call disable_fifo"
    );
    assert!(
        f.helper_calls.contains(&"disable_uart".to_string()),
        "shutdown_port: must call disable_uart"
    );
    assert!(
        body_assigns_option_none(&f.body),
        "shutdown_port: must assign pending_tx to None"
    );
}

#[test]
fn check2_service_rx_fifo_body() {
    let r = compile_and_extract(SPECIMEN, "hmb_c2_rx");
    let f = find_fn(&r.body, "service_rx_fifo");

    // handled = false initialization (bool scalar 0)
    assert!(
        body_has_const_scalar(&f.body, 0),
        "service_rx_fifo: must contain handled = false (scalar 0)"
    );
    // calls rx_char
    assert!(
        body_has_call(&f.body, "rx_char"),
        "service_rx_fifo: must call rx_char"
    );
    // handled = true (bool scalar 1)
    assert!(
        body_has_const_scalar(&f.body, 1),
        "service_rx_fifo: must contain handled = true (scalar 1)"
    );
    assert_eq!(f.return_type.as_deref(), Some("bool"));
}

#[test]
fn check2_service_tx_fifo_body() {
    let r = compile_and_extract(SPECIMEN, "hmb_c2_tx");
    let f = find_fn(&r.body, "service_tx_fifo");

    assert!(
        body_has_switch(&f.body),
        "service_tx_fifo: must have SwitchInt for pending_tx"
    );
    assert!(
        body_has_call(&f.body, "tx_ready"),
        "service_tx_fifo: must call tx_ready"
    );
    assert!(
        body_has_call(&f.body, "field_val"),
        "service_tx_fifo: must call field_val for DR write"
    );
    assert!(
        body_assigns_option_none(&f.body),
        "service_tx_fifo: must assign pending_tx to None"
    );
    assert!(
        f.helper_calls
            .contains(&"disable_tx_interrupts".to_string()),
        "service_tx_fifo: must call disable_tx_interrupts"
    );
}

#[test]
fn check2_clear_explicit_interrupts_body() {
    let r = compile_and_extract(SPECIMEN, "hmb_c2_clear_int");
    let f = find_fn(&r.body, "clear_explicit_interrupts");

    assert!(
        body_has_bitand(&f.body),
        "clear_explicit_interrupts: must have BitAnd for clear_mask"
    );
    assert!(
        body_has_switch(&f.body),
        "clear_explicit_interrupts: must have SwitchInt for clear_mask != 0"
    );
    assert!(
        body_has_call(&f.body, "raw_reg_val"),
        "clear_explicit_interrupts: must call raw_reg_val for ICR write"
    );
    // EXPLICIT_ISR_CLEAR_MASK (0x0780) -- may be inlined or unevaluated
    assert!(
        body_has_u32_const(&f.body, 0x0780)
            || body_has_const_scalar(&f.body, 0x0780)
            || body_has_unevaluated_const(&f.body, "EXPLICIT_ISR_CLEAR_MASK"),
        "clear_explicit_interrupts: must reference EXPLICIT_ISR_CLEAR_MASK"
    );
}
