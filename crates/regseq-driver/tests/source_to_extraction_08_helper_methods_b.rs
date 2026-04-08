//! source-to-extraction-08: PL011 helper methods B authored->extraction
//! boundary validation.
//!
//! Validates that the second half of the authored helper-method facts for
//! PL011 survive at the compiler-side boundary between the authored PL011
//! source and the current Regseq extraction state.
//!
//! Extraction surfaces under test:
//! - `IdentityEnrichmentSurface.impl_blocks` (method identity surface)
//! - `MirBodySurface.functions` (executable helper-body surface)

#![feature(rustc_private)]
#![feature(box_patterns)]

extern crate rustc_ast;
extern crate rustc_driver;
extern crate rustc_hir;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_session;

use std::sync::{Arc, Mutex, Once};

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

// ---------------------------------------------------------------------------
// In-scope helper method names
// ---------------------------------------------------------------------------

/// The 10 helper methods in scope for check group B.
const HELPER_B_METHODS: &[&str] = &[
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
];

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

struct DualSurface {
    identity: IdentityEnrichmentSurface,
    bodies: MirBodySurface,
}

fn compile_and_extract(source: &str, test_name: &str) -> DualSurface {
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

    type IdentityResult = Result<IdentityEnrichmentSurface, IdentityEnrichmentError>;
    type BodyResult = Result<MirBodySurface, BodyError>;

    let identity_result: Arc<Mutex<Option<IdentityResult>>> = Arc::new(Mutex::new(None));
    let body_result: Arc<Mutex<Option<BodyResult>>> = Arc::new(Mutex::new(None));

    struct ExtractCallbacks {
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
            *self.identity_result.lock().unwrap() = Some(extract_identity_enrichment(tcx));
            *self.body_result.lock().unwrap() = Some(extract_body_surface(tcx));
            rustc_driver::Compilation::Stop
        }
    }

    let mut callbacks = ExtractCallbacks {
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

    let identity = identity_result
        .lock()
        .unwrap()
        .take()
        .expect("after_analysis should have run")
        .expect("identity extraction should succeed");

    let bodies = body_result
        .lock()
        .unwrap()
        .take()
        .expect("after_analysis should have run")
        .expect("body extraction should succeed");

    std::fs::remove_dir_all(&test_dir).ok();

    DualSurface { identity, bodies }
}

// ---------------------------------------------------------------------------
// Reduced specimen
//
// Mirrors the 10 helper methods from group B of the PL011 fixture source
// at tests/fixtures/pl011-full-spec/src/driver.rs. Resource types are
// stubbed locally. The specimen includes a Lifecycle trait impl so that
// the body extractor discovers helper calls transitively.
//
// Structural fidelity with the real fixture is verified by the
// `specimen_fidelity_against_fixture` test.
// ---------------------------------------------------------------------------

const HELPER_B_SPECIMEN: &str = r#"
pub mod regseq {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum RegseqError {
        InvalidParameter,
        HardwareError,
        GenericFailure,
    }

    pub struct Clock {
        rate: u32,
    }
    impl Clock {
        pub fn get_rate(&self) -> u32 { self.rate }
        pub fn enable(&self) -> Result<(), RegseqError> { Ok(()) }
        pub fn disable(&self) {}
    }

    pub struct ResetLine;
    impl ResetLine {
        pub fn toggle(&self, _d: Duration) -> Result<(), RegseqError> { Ok(()) }
    }

    pub struct Duration;
    impl Duration {
        pub fn from_us(_us: u32) -> Self { Duration }
    }

    pub trait PinState {}

    pub struct PinCtrl;
    impl PinCtrl {
        pub fn apply<S: PinState>(&self) -> Result<(), RegseqError> { Ok(()) }
    }

    pub struct Mmio<T> {
        pub inner: T,
    }

    pub struct RxChar;
    impl RxChar {
        pub fn new(_b: u8, _e: u32) -> Self { RxChar }
    }

    pub trait Lifecycle {
        fn init(&mut self) -> Result<(), RegseqError>;
        fn deinit(&mut self) -> Result<(), RegseqError>;
    }

    pub trait Serial {
        fn rx_char(&mut self) -> Option<RxChar>;
        fn tx_ready(&mut self) -> bool;
    }
}

use regseq::*;

use core::marker::PhantomData;

pub struct ReadWrite<T, R> { _v: T, _r: PhantomData<R> }
impl<T, R> ReadWrite<T, R> {
    pub fn write(&mut self, _v: FieldValue<T, R>) {}
    pub fn modify(&mut self, _v: FieldValue<T, R>) {}
    pub fn read_raw(&self) -> u32 { 0 }
}

pub struct ReadOnly<T, R> { _v: T, _r: PhantomData<R> }
impl<T, R> ReadOnly<T, R> {
    pub fn read(&self, _f: Field<T, R>) -> u32 { 0 }
    pub fn read_raw(&self) -> u32 { 0 }
}

pub struct WriteOnly<T, R> { _v: T, _r: PhantomData<R> }
impl<T, R> WriteOnly<T, R> {
    pub fn write(&mut self, _v: FieldValue<T, R>) {}
}

#[derive(Clone, Copy)]
pub struct Field<T, R> {
    pub mask: T,
    pub shift: u32,
    _r: PhantomData<R>,
}

pub struct FieldValue<T, R> {
    _v: T,
    _r: PhantomData<R>,
}

impl<T, R> FieldValue<T, R> {
    pub fn new(_mask: T, _val: T) -> Self {
        FieldValue { _v: unsafe { core::mem::zeroed() }, _r: PhantomData }
    }
}

impl<T, R> core::ops::Add for FieldValue<T, R> {
    type Output = Self;
    fn add(self, _rhs: Self) -> Self { self }
}

fn field_val<R>(_field: Field<u32, R>, _raw: u32) -> FieldValue<u32, R> {
    FieldValue::new(0, 0)
}

fn raw_reg_val<R>(_mask: u32, _value: u32) -> FieldValue<u32, R> {
    FieldValue::new(0, 0)
}

pub struct DefaultState;
impl PinState for DefaultState {}

pub struct SleepState;
impl PinState for SleepState {}

#[allow(non_snake_case)]
mod DR_REG {
    pub struct Register;
    pub mod DATA {
        pub const FIELD: super::super::Field<u32, super::Register> =
            super::super::Field { mask: 0xFF, shift: 0, _r: core::marker::PhantomData };
    }
}

#[allow(non_snake_case)]
mod IMSC_REG { pub struct Register; }
#[allow(non_snake_case)]
mod ICR_REG { pub struct Register; }

pub struct Pl011Registers {
    pub dr: ReadWrite<u32, DR_REG::Register>,
    pub imsc: ReadWrite<u32, IMSC_REG::Register>,
    pub icr: WriteOnly<u32, ICR_REG::Register>,
}

#[rseq::driver(name = "pl011-full-spec", compatible = ["arm,pl011"], bus = "amba")]
pub struct Pl011FullSpec {
    pub regs: Mmio<Pl011Registers>,
    pub current_speed: u32,
    pub clock: Option<Clock>,
    pub reset: Option<ResetLine>,
    pub pinctrl: Option<PinCtrl>,
    pub pending_tx: Option<u8>,
}

impl Pl011FullSpec {
    const ALL_UART_INTERRUPTS_MASK: u32 = 0x07ff;
    const RX_INTERRUPT_MASK: u32 = (1 << 4) | (1 << 6);
    const TX_INTERRUPT_MASK: u32 = 1 << 5;
    const ERROR_INTERRUPT_MASK: u32 = (1 << 7) | (1 << 8) | (1 << 9) | (1 << 10);
    const EXPLICIT_ISR_CLEAR_MASK: u32 = Self::ERROR_INTERRUPT_MASK;
    const DEFAULT_UART_CLOCK_HZ: u32 = 48_000_000;

    fn mask_all_interrupts(&mut self) {
        self.regs.inner.imsc.write(raw_reg_val::<IMSC_REG::Register>(
            Self::ALL_UART_INTERRUPTS_MASK,
            0,
        ));
    }

    fn clear_all_interrupts(&mut self) {
        self.regs.inner.icr.write(raw_reg_val::<ICR_REG::Register>(
            Self::ALL_UART_INTERRUPTS_MASK,
            Self::ALL_UART_INTERRUPTS_MASK,
        ));
    }

    fn disable_fifo(&mut self) {}
    fn disable_uart(&mut self) {}
    fn enable_tx_interrupts(&mut self) {}

    fn disable_tx_interrupts(&mut self) {
        self.regs.inner.imsc.modify(raw_reg_val::<IMSC_REG::Register>(
            Self::TX_INTERRUPT_MASK,
            0,
        ));
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
                    .inner
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
                .inner
                .icr
                .write(raw_reg_val::<ICR_REG::Register>(clear_mask, clear_mask));
        }
    }
}

impl regseq::Lifecycle for Pl011FullSpec {
    fn init(&mut self) -> Result<(), RegseqError> {
        self.toggle_reset_if_present()?;
        self.apply_default_pins_if_present()?;
        self.enable_clock_if_present()?;
        let _clk = self.uart_clock_hz();
        Ok(())
    }
    fn deinit(&mut self) -> Result<(), RegseqError> {
        self.shutdown_port();
        self.disable_clock_if_present();
        self.apply_sleep_pins_if_present()?;
        Ok(())
    }
}

impl regseq::Serial for Pl011FullSpec {
    fn rx_char(&mut self) -> Option<RxChar> { None }
    fn tx_ready(&mut self) -> bool { false }
}

impl Pl011FullSpec {
    #[rseq::isr]
    pub fn handle_irq(&mut self) -> u32 {
        let _rx = self.service_rx_fifo();
        let _tx = self.service_tx_fifo();
        self.clear_explicit_interrupts(0);
        0
    }
}
"#;

// ---------------------------------------------------------------------------
// MIR analysis helpers
// ---------------------------------------------------------------------------

/// Find an extracted function by name.
fn find_fn<'a>(surface: &'a MirBodySurface, name: &str) -> Option<&'a MirExtractedFn> {
    surface.functions.iter().find(|f| f.name == name)
}

/// Recursively check if a MirTy (or any of its nested args) contains a path
/// matching the given substring.
fn mir_ty_contains(ty: &MirTy, substr: &str) -> bool {
    match ty {
        MirTy::Fn { path, params, ret } => {
            path.contains(substr)
                || params.iter().any(|a| mir_ty_contains(a, substr))
                || mir_ty_contains(ret, substr)
        }
        MirTy::Struct { path, fields } => {
            path.contains(substr) || fields.iter().any(|f| mir_ty_contains(&f.ty, substr))
        }
        MirTy::Enum { path, variants } => {
            path.contains(substr)
                || variants
                    .iter()
                    .any(|v| v.fields.iter().any(|f| mir_ty_contains(&f.ty, substr)))
        }
        MirTy::Ref { ty, .. } | MirTy::RawPtr { ty, .. } | MirTy::Slice { element: ty, .. } => {
            mir_ty_contains(ty, substr)
        }
        MirTy::Array { element: ty, .. } => mir_ty_contains(ty, substr),
        MirTy::NominalRef { path } => path.contains(substr),
        MirTy::Tuple { elements } => elements.iter().any(|e| mir_ty_contains(e, substr)),
        MirTy::ZeroSizedLike { description } | MirTy::Unsupported { description } => {
            description.contains(substr)
        }
        _ => false,
    }
}

/// Check if a function's MIR body contains a call whose FnDef type (including
/// type arguments) contains the given substring.  This checks both the path
/// and any generic type arguments.
fn body_has_call_containing_in_type(func: &MirExtractedFn, substr: &str) -> bool {
    for bb in &func.body.basic_blocks {
        if let Some(MirTerminator {
            kind:
                MirTerminatorKind::Call {
                    func:
                        MirOperand::Constant {
                            constant: MirConstant { ty, .. },
                        },
                    ..
                },
            ..
        }) = &bb.terminator
        {
            if mir_ty_contains(ty, substr) {
                return true;
            }
        }
    }
    false
}

/// Check if a function's MIR body contains a call to a function whose path
/// contains the given substring.
fn body_has_call_containing(func: &MirExtractedFn, substr: &str) -> bool {
    for bb in &func.body.basic_blocks {
        if let Some(MirTerminator {
            kind:
                MirTerminatorKind::Call {
                    func:
                        MirOperand::Constant {
                            constant:
                                MirConstant {
                                    ty: MirTy::Fn { path, .. },
                                    ..
                                },
                        },
                    ..
                },
            ..
        }) = &bb.terminator
        {
            if path.contains(substr) {
                return true;
            }
        }
    }
    false
}

/// Check if a function body contains a SwitchInt terminator (branch).
fn body_has_switch_int(func: &MirExtractedFn) -> bool {
    func.body.basic_blocks.iter().any(|bb| {
        bb.terminator
            .as_ref()
            .map(|t| matches!(&t.kind, MirTerminatorKind::SwitchInt { .. }))
            .unwrap_or(false)
    })
}

/// Check if the body contains a constant with the given scalar value.
fn body_has_const_scalar(func: &MirExtractedFn, value: u128) -> bool {
    for bb in &func.body.basic_blocks {
        for stmt in &bb.statements {
            if let MirStatementKind::Assign { rvalue, .. } = &stmt.kind {
                if rvalue_has_const_scalar(rvalue, value) {
                    return true;
                }
            }
        }
        if let Some(term) = &bb.terminator {
            match &term.kind {
                MirTerminatorKind::Call { func: op, args, .. } => {
                    if operand_has_const_scalar(op, value) {
                        return true;
                    }
                    for arg in args {
                        if operand_has_const_scalar(arg, value) {
                            return true;
                        }
                    }
                }
                MirTerminatorKind::SwitchInt { discr, targets } => {
                    if operand_has_const_scalar(discr, value) {
                        return true;
                    }
                    if targets.values.contains(&value) {
                        return true;
                    }
                }
                _ => {}
            }
        }
    }
    false
}

/// Check if the body contains an unevaluated constant reference whose
/// description contains the given substring.
fn body_has_const_ref_containing(func: &MirExtractedFn, substr: &str) -> bool {
    for bb in &func.body.basic_blocks {
        for stmt in &bb.statements {
            if let MirStatementKind::Assign { rvalue, .. } = &stmt.kind {
                if rvalue_has_const_ref(rvalue, substr) {
                    return true;
                }
            }
        }
        if let Some(MirTerminator {
            kind: MirTerminatorKind::Call { func: op, args, .. },
            ..
        }) = &bb.terminator
        {
            if operand_has_const_ref(op, substr) {
                return true;
            }
            for arg in args {
                if operand_has_const_ref(arg, substr) {
                    return true;
                }
            }
        }
    }
    false
}

fn operand_has_const_ref(op: &MirOperand, substr: &str) -> bool {
    if let MirOperand::Constant {
        constant:
            MirConstant {
                value: MirConstValue::Unevaluated { path },
                ..
            },
    } = op
    {
        if path.contains(substr) {
            return true;
        }
    }
    if let MirOperand::Constant {
        constant:
            MirConstant {
                value: MirConstValue::Unsupported { description, .. },
                ..
            },
    } = op
    {
        if description.contains(substr) {
            return true;
        }
    }
    if let MirOperand::Constant {
        constant: MirConstant {
            ty: MirTy::Fn { path, .. },
            ..
        },
    } = op
    {
        if path.contains(substr) {
            return true;
        }
    }
    false
}

fn rvalue_has_const_ref(rv: &MirRvalue, substr: &str) -> bool {
    match rv {
        MirRvalue::Use { operand } => operand_has_const_ref(operand, substr),
        MirRvalue::BinaryOp { lhs, rhs, .. } => {
            operand_has_const_ref(lhs, substr) || operand_has_const_ref(rhs, substr)
        }
        MirRvalue::Cast { operand, .. } => operand_has_const_ref(operand, substr),
        _ => false,
    }
}

fn rvalue_has_const_scalar(rv: &MirRvalue, value: u128) -> bool {
    match rv {
        MirRvalue::Use { operand } => operand_has_const_scalar(operand, value),
        MirRvalue::BinaryOp { lhs, rhs, .. } => {
            operand_has_const_scalar(lhs, value) || operand_has_const_scalar(rhs, value)
        }
        MirRvalue::UnaryOp { operand, .. } => operand_has_const_scalar(operand, value),
        MirRvalue::Cast { operand, .. } => operand_has_const_scalar(operand, value),
        MirRvalue::Aggregate { operands, .. } => {
            operands.iter().any(|o| operand_has_const_scalar(o, value))
        }
        _ => false,
    }
}

fn operand_has_const_scalar(op: &MirOperand, expected: u128) -> bool {
    if let MirOperand::Constant {
        constant:
            MirConstant {
                value: MirConstValue::Bits { value, .. },
                ..
            },
    } = op
    {
        *value == expected
    } else {
        false
    }
}

/// Check that a function body has a SetDiscriminant or an aggregate write
/// that sets `None` on a field (pending_tx = None pattern).
fn body_assigns_none_option(func: &MirExtractedFn) -> bool {
    // In MIR, `self.pending_tx = None` translates to either
    // a SetDiscriminant or an Aggregate with variant_index for None.
    for bb in &func.body.basic_blocks {
        for stmt in &bb.statements {
            match &stmt.kind {
                MirStatementKind::SetDiscriminant { .. } => return true,
                MirStatementKind::Assign { rvalue, .. } => {
                    if let MirRvalue::Aggregate {
                        agg_kind:
                            MirAggregateKind::Adt {
                                path,
                                variant_index,
                                ..
                            },
                        ..
                    } = rvalue.as_ref()
                    {
                        if path.contains("Option") && *variant_index == 0 {
                            return true;
                        }
                    }
                }
                _ => {}
            }
        }
    }
    false
}

/// Collect, in order, the call-target names appearing in a function's MIR basic
/// block chain.  Walks from bb0 through `target` edges of `Call` and `Goto`
/// terminators, collecting the callee function-path substring that matches any
/// entry in `needles`.  This preserves the execution order visible in MIR.
fn ordered_call_names_matching(func: &MirExtractedFn, needles: &[&str]) -> Vec<String> {
    let bbs = &func.body.basic_blocks;
    let mut result = Vec::new();
    let mut visited = std::collections::HashSet::new();
    let mut idx: u32 = 0;
    loop {
        if !visited.insert(idx) {
            break; // prevent infinite loop on cycles
        }
        let Some(bb) = bbs.get(idx as usize) else {
            break;
        };
        let Some(term) = &bb.terminator else { break };
        match &term.kind {
            MirTerminatorKind::Call {
                func:
                    MirOperand::Constant {
                        constant:
                            MirConstant {
                                ty: MirTy::Fn { path, .. },
                                ..
                            },
                    },
                target,
                ..
            } => {
                for needle in needles {
                    if path.contains(needle) {
                        result.push(needle.to_string());
                    }
                }
                if let Some(next) = target {
                    idx = *next;
                } else {
                    break;
                }
            }
            MirTerminatorKind::Call { target, .. } => {
                // Non-FnDef call; still follow the target edge.
                if let Some(next) = target {
                    idx = *next;
                } else {
                    break;
                }
            }
            MirTerminatorKind::Goto { target } => {
                idx = *target;
            }
            _ => break,
        }
    }
    result
}

/// Check that a function's return type local (local 0) has a given type kind.
fn return_type_is_bool(func: &MirExtractedFn) -> bool {
    func.body
        .local_decls
        .first()
        .map(|d| matches!(&d.ty, MirTy::Bool))
        .unwrap_or(false)
}

fn return_type_is_u32(func: &MirExtractedFn) -> bool {
    func.body
        .local_decls
        .first()
        .map(|d| {
            matches!(
                &d.ty,
                MirTy::Int {
                    signed: false,
                    width: IntWidth::U32
                }
            )
        })
        .unwrap_or(false)
}

fn return_type_is_result(func: &MirExtractedFn) -> bool {
    func.body
        .local_decls
        .first()
        .map(|d| matches!(&d.ty, MirTy::Enum { path, .. } if path.contains("Result")))
        .unwrap_or(false)
}

fn return_type_is_unit(func: &MirExtractedFn) -> bool {
    func.body
        .local_decls
        .first()
        .map(|d| matches!(&d.ty, MirTy::Tuple { elements } if elements.is_empty()))
        .unwrap_or(false)
}

// ---------------------------------------------------------------------------
// Precondition: specimen fidelity against real fixture source
// ---------------------------------------------------------------------------

#[test]
fn specimen_fidelity_against_fixture() {
    let fixture = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/pl011-full-spec/src/driver.rs"
    ))
    .expect("fixture source must be readable");

    // -- uart_clock_hz --
    assert!(
        fixture.contains("fn uart_clock_hz(&self) -> u32"),
        "fixture must declare uart_clock_hz with &self -> u32"
    );
    assert!(
        fixture.contains(".map(Clock::get_rate)"),
        "fixture uart_clock_hz must call Clock::get_rate via map"
    );
    assert!(
        fixture.contains(".unwrap_or(Self::DEFAULT_UART_CLOCK_HZ)"),
        "fixture uart_clock_hz must unwrap_or DEFAULT_UART_CLOCK_HZ"
    );
    assert!(
        fixture.contains("const DEFAULT_UART_CLOCK_HZ: u32 = 48_000_000;"),
        "fixture must define DEFAULT_UART_CLOCK_HZ = 48_000_000"
    );

    // -- enable_clock_if_present --
    assert!(
        fixture.contains("fn enable_clock_if_present(&self) -> Result<(), RegseqError>"),
        "fixture must declare enable_clock_if_present -> Result"
    );
    assert!(
        fixture.contains("if let Some(clock) = &self.clock {\n            clock.enable()?;"),
        "fixture enable_clock_if_present must test self.clock and call enable()?"
    );

    // -- disable_clock_if_present --
    assert!(
        fixture.contains("fn disable_clock_if_present(&self)"),
        "fixture must declare disable_clock_if_present with &self, no return"
    );
    assert!(
        fixture.contains("clock.disable();"),
        "fixture disable_clock_if_present must call clock.disable()"
    );

    // -- toggle_reset_if_present --
    assert!(
        fixture.contains("fn toggle_reset_if_present(&self) -> Result<(), RegseqError>"),
        "fixture must declare toggle_reset_if_present -> Result"
    );
    assert!(
        fixture.contains("reset.toggle(Duration::from_us(10))?;"),
        "fixture toggle_reset_if_present must call reset.toggle(Duration::from_us(10))?"
    );

    // -- apply_default_pins_if_present --
    assert!(
        fixture.contains("fn apply_default_pins_if_present(&self) -> Result<(), RegseqError>"),
        "fixture must declare apply_default_pins_if_present -> Result"
    );
    assert!(
        fixture.contains("pinctrl.apply::<DefaultState>()?;"),
        "fixture must call pinctrl.apply::<DefaultState>()?"
    );

    // -- apply_sleep_pins_if_present --
    assert!(
        fixture.contains("fn apply_sleep_pins_if_present(&self) -> Result<(), RegseqError>"),
        "fixture must declare apply_sleep_pins_if_present -> Result"
    );
    assert!(
        fixture.contains("pinctrl.apply::<SleepState>()?;"),
        "fixture must call pinctrl.apply::<SleepState>()?"
    );

    // -- shutdown_port --
    assert!(
        fixture.contains("fn shutdown_port(&mut self)"),
        "fixture must declare shutdown_port with &mut self"
    );
    // Verify call order in shutdown_port
    let shutdown_body_start = fixture
        .find("fn shutdown_port(&mut self)")
        .expect("shutdown_port not found");
    let shutdown_body = &fixture[shutdown_body_start..];
    let mask_pos = shutdown_body.find("self.mask_all_interrupts()").unwrap();
    let clear_pos = shutdown_body.find("self.clear_all_interrupts()").unwrap();
    let disable_fifo_pos = shutdown_body.find("self.disable_fifo()").unwrap();
    let disable_uart_pos = shutdown_body.find("self.disable_uart()").unwrap();
    let pending_none_pos = shutdown_body.find("self.pending_tx = None").unwrap();
    assert!(mask_pos < clear_pos, "mask_all before clear_all");
    assert!(
        clear_pos < disable_fifo_pos,
        "clear_all before disable_fifo"
    );
    assert!(
        disable_fifo_pos < disable_uart_pos,
        "disable_fifo before disable_uart"
    );
    assert!(
        disable_uart_pos < pending_none_pos,
        "disable_uart before pending_tx = None"
    );

    // -- service_rx_fifo --
    assert!(
        fixture.contains("fn service_rx_fifo(&mut self) -> bool"),
        "fixture must declare service_rx_fifo -> bool"
    );
    assert!(
        fixture.contains("let mut handled = false;"),
        "fixture service_rx_fifo must init handled = false"
    );
    assert!(
        fixture.contains("while self.rx_char().is_some()"),
        "fixture must loop on rx_char().is_some()"
    );

    // -- service_tx_fifo --
    assert!(
        fixture.contains("fn service_tx_fifo(&mut self) -> bool"),
        "fixture must declare service_tx_fifo -> bool"
    );
    assert!(
        fixture.contains("if let Some(byte) = self.pending_tx"),
        "fixture must branch on pending_tx"
    );
    assert!(
        fixture.contains("DR_REG::DATA::FIELD, byte as u32"),
        "fixture must write DR.DATA with byte as u32"
    );

    // -- clear_explicit_interrupts --
    assert!(
        fixture.contains("fn clear_explicit_interrupts(&mut self, status: u32)"),
        "fixture must declare clear_explicit_interrupts(status: u32)"
    );
    assert!(
        fixture.contains("let clear_mask = status & Self::EXPLICIT_ISR_CLEAR_MASK;"),
        "fixture must compute clear_mask"
    );
    assert!(
        fixture.contains("if clear_mask != 0"),
        "fixture must guard on clear_mask != 0"
    );
    assert!(
        fixture.contains("const EXPLICIT_ISR_CLEAR_MASK: u32 = Self::ERROR_INTERRUPT_MASK;"),
        "fixture must define EXPLICIT_ISR_CLEAR_MASK = ERROR_INTERRUPT_MASK"
    );
    assert!(
        fixture.contains(
            "const ERROR_INTERRUPT_MASK: u32 = (1 << 7) | (1 << 8) | (1 << 9) | (1 << 10);"
        ),
        "fixture must define ERROR_INTERRUPT_MASK = 0x0780"
    );
}

// ---------------------------------------------------------------------------
// Check Group 1: identity, signature, and return type for all 10 helpers
// ---------------------------------------------------------------------------

#[test]
fn check_group_1_helper_identity_and_signature() {
    let surface = compile_and_extract(HELPER_B_SPECIMEN, "src_to_ext_08_cg1");

    // Verify all 10 helpers exist in the body surface
    for name in HELPER_B_METHODS {
        let func = find_fn(&surface.bodies, name);
        assert!(
            func.is_some(),
            "helper `{name}` must be extracted in MirBodySurface"
        );
    }

    // Verify identity enrichment: all 10 helpers appear as methods in an
    // inherent impl block on Pl011FullSpec
    let inherent_impl = surface
        .identity
        .impl_blocks
        .iter()
        .find(|ib| ib.is_inherent && ib.self_type_name == "Pl011FullSpec")
        .expect("inherent impl on Pl011FullSpec must exist");

    for name in HELPER_B_METHODS {
        assert!(
            inherent_impl.methods.iter().any(|m| m.name == *name),
            "helper `{name}` must appear in inherent impl methods"
        );
    }

    // -- uart_clock_hz: &self -> u32 --
    let f = find_fn(&surface.bodies, "uart_clock_hz").unwrap();
    assert_eq!(
        f.receiver, "ref",
        "uart_clock_hz receiver must be &self (ref)"
    );
    assert!(
        f.params.is_empty(),
        "uart_clock_hz has no params beyond self"
    );
    assert_eq!(
        f.return_type.as_ref(),
        Some(&MirTy::Int {
            signed: false,
            width: IntWidth::U32
        }),
        "uart_clock_hz returns u32"
    );
    assert!(f.kind == MirFnKind::Helper, "uart_clock_hz is a helper");
    assert!(
        return_type_is_u32(f),
        "uart_clock_hz MIR return local is u32"
    );

    // -- enable_clock_if_present: &self -> Result<(), RegseqError> --
    let f = find_fn(&surface.bodies, "enable_clock_if_present").unwrap();
    assert_eq!(
        f.receiver, "ref",
        "enable_clock_if_present receiver must be &self"
    );
    assert!(f.params.is_empty(), "enable_clock_if_present has no params");
    assert!(
        f.return_type
            .as_ref()
            .map(|r| matches!(r, MirTy::Enum { path, .. } if path.contains("Result")))
            .unwrap_or(false),
        "enable_clock_if_present returns Result"
    );
    assert!(
        return_type_is_result(f),
        "enable_clock_if_present MIR return is Result ADT"
    );

    // -- disable_clock_if_present: &self -> () --
    let f = find_fn(&surface.bodies, "disable_clock_if_present").unwrap();
    assert_eq!(
        f.receiver, "ref",
        "disable_clock_if_present receiver must be &self"
    );
    assert!(
        f.params.is_empty(),
        "disable_clock_if_present has no params"
    );
    assert!(
        f.return_type.is_none(),
        "disable_clock_if_present returns ()"
    );
    assert!(
        return_type_is_unit(f),
        "disable_clock_if_present MIR return is ()"
    );

    // -- toggle_reset_if_present: &self -> Result<(), RegseqError> --
    let f = find_fn(&surface.bodies, "toggle_reset_if_present").unwrap();
    assert_eq!(
        f.receiver, "ref",
        "toggle_reset_if_present receiver must be &self"
    );
    assert!(f.params.is_empty(), "toggle_reset_if_present has no params");
    assert!(
        f.return_type
            .as_ref()
            .map(|r| matches!(r, MirTy::Enum { path, .. } if path.contains("Result")))
            .unwrap_or(false),
        "toggle_reset_if_present returns Result"
    );

    // -- apply_default_pins_if_present: &self -> Result<(), RegseqError> --
    let f = find_fn(&surface.bodies, "apply_default_pins_if_present").unwrap();
    assert_eq!(
        f.receiver, "ref",
        "apply_default_pins_if_present receiver must be &self"
    );
    assert!(
        f.return_type
            .as_ref()
            .map(|r| matches!(r, MirTy::Enum { path, .. } if path.contains("Result")))
            .unwrap_or(false),
        "apply_default_pins_if_present returns Result"
    );

    // -- apply_sleep_pins_if_present: &self -> Result<(), RegseqError> --
    let f = find_fn(&surface.bodies, "apply_sleep_pins_if_present").unwrap();
    assert_eq!(
        f.receiver, "ref",
        "apply_sleep_pins_if_present receiver must be &self"
    );
    assert!(
        f.return_type
            .as_ref()
            .map(|r| matches!(r, MirTy::Enum { path, .. } if path.contains("Result")))
            .unwrap_or(false),
        "apply_sleep_pins_if_present returns Result"
    );

    // -- shutdown_port: &mut self -> () --
    let f = find_fn(&surface.bodies, "shutdown_port").unwrap();
    assert_eq!(
        f.receiver, "ref_mut",
        "shutdown_port receiver must be &mut self"
    );
    assert!(f.params.is_empty(), "shutdown_port has no params");
    assert!(f.return_type.is_none(), "shutdown_port returns ()");

    // -- service_rx_fifo: &mut self -> bool --
    let f = find_fn(&surface.bodies, "service_rx_fifo").unwrap();
    assert_eq!(
        f.receiver, "ref_mut",
        "service_rx_fifo receiver must be &mut self"
    );
    assert!(f.params.is_empty(), "service_rx_fifo has no params");
    assert_eq!(
        f.return_type.as_ref(),
        Some(&MirTy::Bool),
        "service_rx_fifo returns bool"
    );
    assert!(return_type_is_bool(f), "service_rx_fifo MIR return is bool");

    // -- service_tx_fifo: &mut self -> bool --
    let f = find_fn(&surface.bodies, "service_tx_fifo").unwrap();
    assert_eq!(
        f.receiver, "ref_mut",
        "service_tx_fifo receiver must be &mut self"
    );
    assert!(f.params.is_empty(), "service_tx_fifo has no params");
    assert_eq!(
        f.return_type.as_ref(),
        Some(&MirTy::Bool),
        "service_tx_fifo returns bool"
    );
    assert!(return_type_is_bool(f), "service_tx_fifo MIR return is bool");

    // -- clear_explicit_interrupts: &mut self, status: u32 -> () --
    let f = find_fn(&surface.bodies, "clear_explicit_interrupts").unwrap();
    assert_eq!(
        f.receiver, "ref_mut",
        "clear_explicit_interrupts receiver must be &mut self"
    );
    assert_eq!(
        f.params.len(),
        1,
        "clear_explicit_interrupts has 1 param (status)"
    );
    assert_eq!(f.params[0].name, "status", "param name is status");
    assert_eq!(
        f.params[0].ty,
        MirTy::Int {
            signed: false,
            width: IntWidth::U32
        },
        "param type is u32"
    );
    assert!(
        f.return_type.is_none(),
        "clear_explicit_interrupts returns ()"
    );
}

// ---------------------------------------------------------------------------
// Check Group 2: body-level downstream checks per method
// ---------------------------------------------------------------------------

#[test]
fn check_group_2_uart_clock_hz_body() {
    let surface = compile_and_extract(HELPER_B_SPECIMEN, "src_to_ext_08_cg2_uart_clk");
    let f = find_fn(&surface.bodies, "uart_clock_hz").unwrap();

    // Must call Clock::get_rate -- either directly or via Option::map.
    // In the authored source, `self.clock.as_ref().map(Clock::get_rate)` passes
    // get_rate as a function pointer to Option::map. MIR may show this as a call
    // to `map` with the FnDef referencing get_rate, or as inlined calls.
    let calls_get_rate = body_has_call_containing(f, "get_rate");
    let calls_map = body_has_call_containing(f, "map");
    assert!(
        calls_get_rate || calls_map,
        "uart_clock_hz must call get_rate or Option::map (found neither)"
    );

    // Must contain DEFAULT_UART_CLOCK_HZ fallback = 48_000_000.
    // The constant may appear as an inlined scalar (48_000_000) or as an
    // unevaluated associated constant reference (DEFAULT_UART_CLOCK_HZ).
    let has_scalar = body_has_const_scalar(f, 48_000_000);
    let has_const_ref = body_has_const_ref_containing(f, "DEFAULT_UART_CLOCK_HZ");
    assert!(
        has_scalar || has_const_ref,
        "uart_clock_hz must contain 48_000_000 scalar or DEFAULT_UART_CLOCK_HZ const ref"
    );

    // Must have Option branching (SwitchInt for discriminant) or call
    // unwrap_or which internally branches.
    let has_branch = body_has_switch_int(f);
    let calls_unwrap_or = body_has_call_containing(f, "unwrap_or");
    assert!(
        has_branch || calls_unwrap_or,
        "uart_clock_hz must have SwitchInt for Option branching or call unwrap_or"
    );
}

#[test]
fn check_group_2_enable_clock_if_present_body() {
    let surface = compile_and_extract(HELPER_B_SPECIMEN, "src_to_ext_08_cg2_enable_clk");
    let f = find_fn(&surface.bodies, "enable_clock_if_present").unwrap();

    // Must have Option branching
    assert!(
        body_has_switch_int(f),
        "enable_clock_if_present must have SwitchInt for Option test"
    );

    // Must call Clock::enable
    assert!(
        body_has_call_containing(f, "enable"),
        "enable_clock_if_present must call enable"
    );
}

#[test]
fn check_group_2_disable_clock_if_present_body() {
    let surface = compile_and_extract(HELPER_B_SPECIMEN, "src_to_ext_08_cg2_disable_clk");
    let f = find_fn(&surface.bodies, "disable_clock_if_present").unwrap();

    // Must have Option branching
    assert!(
        body_has_switch_int(f),
        "disable_clock_if_present must have SwitchInt for Option test"
    );

    // Must call Clock::disable
    assert!(
        body_has_call_containing(f, "disable"),
        "disable_clock_if_present must call disable"
    );
}

#[test]
fn check_group_2_toggle_reset_if_present_body() {
    let surface = compile_and_extract(HELPER_B_SPECIMEN, "src_to_ext_08_cg2_toggle_rst");
    let f = find_fn(&surface.bodies, "toggle_reset_if_present").unwrap();

    // Must have Option branching
    assert!(
        body_has_switch_int(f),
        "toggle_reset_if_present must have SwitchInt for Option test"
    );

    // Must call ResetLine::toggle
    assert!(
        body_has_call_containing(f, "toggle"),
        "toggle_reset_if_present must call toggle"
    );

    // Must call Duration::from_us with 10
    assert!(
        body_has_call_containing(f, "from_us"),
        "toggle_reset_if_present must call Duration::from_us"
    );
    assert!(
        body_has_const_scalar(f, 10),
        "toggle_reset_if_present must contain constant 10 for Duration::from_us(10)"
    );
}

#[test]
fn check_group_2_apply_default_pins_if_present_body() {
    let surface = compile_and_extract(HELPER_B_SPECIMEN, "src_to_ext_08_cg2_default_pins");
    let f = find_fn(&surface.bodies, "apply_default_pins_if_present").unwrap();

    // Must have Option branching
    assert!(
        body_has_switch_int(f),
        "apply_default_pins_if_present must have SwitchInt for Option test"
    );

    // Must call PinCtrl::apply
    assert!(
        body_has_call_containing(f, "apply"),
        "apply_default_pins_if_present must call apply"
    );

    // Must reference the DefaultState type parameter to distinguish from
    // apply_sleep_pins_if_present. The type param appears in the Fn type's
    // path as embedded generic args (e.g. `apply::<DefaultState>`).
    assert!(
        body_has_call_containing_in_type(f, "DefaultState"),
        "apply_default_pins_if_present must reference DefaultState type parameter"
    );
}

#[test]
fn check_group_2_apply_sleep_pins_if_present_body() {
    let surface = compile_and_extract(HELPER_B_SPECIMEN, "src_to_ext_08_cg2_sleep_pins");
    let f = find_fn(&surface.bodies, "apply_sleep_pins_if_present").unwrap();

    // Must have Option branching
    assert!(
        body_has_switch_int(f),
        "apply_sleep_pins_if_present must have SwitchInt for Option test"
    );

    // Must call PinCtrl::apply
    assert!(
        body_has_call_containing(f, "apply"),
        "apply_sleep_pins_if_present must call apply"
    );

    // Must reference the SleepState type parameter to distinguish from
    // apply_default_pins_if_present.  The type param appears in FnDef args,
    // not in the path, so we check the full type tree.
    assert!(
        body_has_call_containing_in_type(f, "SleepState"),
        "apply_sleep_pins_if_present must reference SleepState type parameter"
    );
}

#[test]
fn check_group_2_shutdown_port_body() {
    let surface = compile_and_extract(HELPER_B_SPECIMEN, "src_to_ext_08_cg2_shutdown");
    let f = find_fn(&surface.bodies, "shutdown_port").unwrap();

    // Must call mask_all_interrupts, clear_all_interrupts, disable_fifo, disable_uart
    assert!(
        f.helper_calls.contains(&"mask_all_interrupts".to_string()),
        "shutdown_port must call mask_all_interrupts"
    );
    assert!(
        f.helper_calls.contains(&"clear_all_interrupts".to_string()),
        "shutdown_port must call clear_all_interrupts"
    );
    assert!(
        f.helper_calls.contains(&"disable_fifo".to_string()),
        "shutdown_port must call disable_fifo"
    );
    assert!(
        f.helper_calls.contains(&"disable_uart".to_string()),
        "shutdown_port must call disable_uart"
    );

    // Verify call ordering via basic-block terminator chain walk.
    // The spec requires: mask_all < clear_all < disable_fifo < disable_uart.
    let ordered = ordered_call_names_matching(
        f,
        &[
            "mask_all_interrupts",
            "clear_all_interrupts",
            "disable_fifo",
            "disable_uart",
        ],
    );
    assert!(
        ordered.len() >= 4,
        "shutdown_port MIR chain must contain all 4 ordered helper calls, found: {:?}",
        ordered
    );
    let mask_idx = ordered.iter().position(|n| n == "mask_all_interrupts");
    let clear_idx = ordered.iter().position(|n| n == "clear_all_interrupts");
    let fifo_idx = ordered.iter().position(|n| n == "disable_fifo");
    let uart_idx = ordered.iter().position(|n| n == "disable_uart");
    assert!(
        mask_idx < clear_idx,
        "MIR call order: mask_all_interrupts must precede clear_all_interrupts"
    );
    assert!(
        clear_idx < fifo_idx,
        "MIR call order: clear_all_interrupts must precede disable_fifo"
    );
    assert!(
        fifo_idx < uart_idx,
        "MIR call order: disable_fifo must precede disable_uart"
    );

    // Must assign pending_tx = None
    assert!(
        body_assigns_none_option(f),
        "shutdown_port must set pending_tx to None"
    );
}

#[test]
fn check_group_2_service_rx_fifo_body() {
    let surface = compile_and_extract(HELPER_B_SPECIMEN, "src_to_ext_08_cg2_srv_rx");
    let f = find_fn(&surface.bodies, "service_rx_fifo").unwrap();

    // Must call rx_char (loop on rx_char().is_some())
    assert!(
        body_has_call_containing(f, "rx_char"),
        "service_rx_fifo must call rx_char"
    );

    // Must initialize handled = false
    assert!(
        body_has_const_scalar(f, 0),
        "service_rx_fifo must have false/0 constant for handled init"
    );

    // Must set handled = true on each successful loop iteration.
    // In MIR, `true` appears as the scalar constant 1 (for bool).
    assert!(
        body_has_const_scalar(f, 1),
        "service_rx_fifo must have true/1 constant for handled = true assignment"
    );

    // Return type is bool
    assert!(return_type_is_bool(f), "service_rx_fifo returns bool");

    // Must have branching (loop + is_some check)
    assert!(
        body_has_switch_int(f),
        "service_rx_fifo must have SwitchInt for loop condition"
    );
}

#[test]
fn check_group_2_service_tx_fifo_body() {
    let surface = compile_and_extract(HELPER_B_SPECIMEN, "src_to_ext_08_cg2_srv_tx");
    let f = find_fn(&surface.bodies, "service_tx_fifo").unwrap();

    // Must branch on pending_tx (Option discriminant)
    assert!(
        body_has_switch_int(f),
        "service_tx_fifo must have SwitchInt for pending_tx branching"
    );

    // Must call tx_ready
    assert!(
        body_has_call_containing(f, "tx_ready"),
        "service_tx_fifo must call tx_ready"
    );

    // Must call disable_tx_interrupts
    assert!(
        f.helper_calls
            .contains(&"disable_tx_interrupts".to_string()),
        "service_tx_fifo must call disable_tx_interrupts"
    );

    // Must write DR register (via field_val call)
    assert!(
        body_has_call_containing(f, "field_val"),
        "service_tx_fifo must call field_val for DR.DATA write"
    );

    // Must assign pending_tx = None (after write path)
    assert!(
        body_assigns_none_option(f),
        "service_tx_fifo must set pending_tx to None"
    );

    // Return type is bool
    assert!(return_type_is_bool(f), "service_tx_fifo returns bool");
}

#[test]
fn check_group_2_clear_explicit_interrupts_body() {
    let surface = compile_and_extract(HELPER_B_SPECIMEN, "src_to_ext_08_cg2_clear_irq");
    let f = find_fn(&surface.bodies, "clear_explicit_interrupts").unwrap();

    // Must have SwitchInt for `clear_mask != 0` guard
    assert!(
        body_has_switch_int(f),
        "clear_explicit_interrupts must have SwitchInt for guard"
    );

    // Must have BitAnd operation (status & EXPLICIT_ISR_CLEAR_MASK)
    let has_bitand = f.body.basic_blocks.iter().any(|bb| {
        bb.statements.iter().any(|s| {
            if let MirStatementKind::Assign { rvalue, .. } = &s.kind {
                matches!(
                    rvalue.as_ref(),
                    MirRvalue::BinaryOp {
                        op: MirBinOp::BitAnd,
                        ..
                    }
                )
            } else {
                false
            }
        })
    });
    assert!(
        has_bitand,
        "clear_explicit_interrupts must have BitAnd for status & mask"
    );

    // Must call raw_reg_val (for ICR write)
    assert!(
        body_has_call_containing(f, "raw_reg_val"),
        "clear_explicit_interrupts must call raw_reg_val for ICR write"
    );

    // Must call write on ICR register
    assert!(
        body_has_call_containing(f, "write"),
        "clear_explicit_interrupts must call write on ICR"
    );
}
