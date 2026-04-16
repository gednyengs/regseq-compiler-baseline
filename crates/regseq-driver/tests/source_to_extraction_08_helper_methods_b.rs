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
            config.opts.unstable_opts.crate_attr.push(
                "allow(unused, non_snake_case, nonstandard_style, redundant_semicolons)"
                    .to_string(),
            );
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
// Path-sensitive MIR analysis helpers
// ---------------------------------------------------------------------------

/// Get all successor BB indices from a terminator kind.
fn terminator_successors(kind: &MirTerminatorKind) -> Vec<u32> {
    match kind {
        MirTerminatorKind::Goto { target } => vec![*target],
        MirTerminatorKind::SwitchInt { targets, .. } => targets.targets.clone(),
        MirTerminatorKind::Call { target, .. } => target.iter().copied().collect(),
        MirTerminatorKind::Drop { target, .. } => vec![*target],
        MirTerminatorKind::Assert { target, .. } => vec![*target],
        _ => vec![],
    }
}

/// Collect all non-cleanup BBs reachable from `start` following all edge types.
fn reachable_bbs_from(func: &MirExtractedFn, start: u32) -> std::collections::HashSet<u32> {
    let bbs = &func.body.basic_blocks;
    let mut visited = std::collections::HashSet::new();
    let mut queue = std::collections::VecDeque::new();
    queue.push_back(start);
    while let Some(idx) = queue.pop_front() {
        if !visited.insert(idx) {
            continue;
        }
        let Some(bb) = bbs.get(idx as usize) else {
            continue;
        };
        if bb.is_cleanup {
            visited.remove(&idx);
            continue;
        }
        let Some(term) = &bb.terminator else {
            continue;
        };
        for target in terminator_successors(&term.kind) {
            queue.push_back(target);
        }
    }
    visited
}

/// Collect BB indices along a straight-line path (Call.target, Goto.target,
/// Drop.target, Assert.target -- stops at SwitchInt or Return).
fn straight_line_bbs(func: &MirExtractedFn, start: u32) -> Vec<u32> {
    let bbs = &func.body.basic_blocks;
    let mut path = vec![start];
    let mut visited = std::collections::HashSet::new();
    visited.insert(start);
    let mut current = start;
    loop {
        let Some(bb) = bbs.get(current as usize) else {
            break;
        };
        if bb.is_cleanup {
            break;
        }
        let Some(term) = &bb.terminator else {
            break;
        };
        let next = match &term.kind {
            MirTerminatorKind::Call {
                target: Some(t), ..
            } => *t,
            MirTerminatorKind::Goto { target } => *target,
            MirTerminatorKind::Drop { target, .. } => *target,
            MirTerminatorKind::Assert { target, .. } => *target,
            _ => break,
        };
        if visited.insert(next) {
            path.push(next);
            current = next;
        } else {
            break;
        }
    }
    path
}

/// Check if any non-cleanup BB in the set has a Call to a function matching
/// `substr`.
fn bb_set_has_call(
    func: &MirExtractedFn,
    bbs: &std::collections::HashSet<u32>,
    substr: &str,
) -> bool {
    for &idx in bbs {
        let Some(bb) = func.body.basic_blocks.get(idx as usize) else {
            continue;
        };
        if bb.is_cleanup {
            continue;
        }
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

/// Check if any non-cleanup BB in the set has a None assignment
/// (SetDiscriminant with variant 0, or Aggregate with Option variant 0).
fn bb_set_has_none_assignment(func: &MirExtractedFn, bbs: &std::collections::HashSet<u32>) -> bool {
    for &idx in bbs {
        let Some(bb) = func.body.basic_blocks.get(idx as usize) else {
            continue;
        };
        if bb.is_cleanup {
            continue;
        }
        for stmt in &bb.statements {
            match &stmt.kind {
                MirStatementKind::SetDiscriminant {
                    variant_index: 0, ..
                } => return true,
                MirStatementKind::Assign { rvalue, .. } => {
                    if let MirRvalue::Aggregate {
                        agg_kind:
                            MirAggregateKind::Adt {
                                path,
                                variant_index: 0,
                                ..
                            },
                        ..
                    } = rvalue.as_ref()
                    {
                        if path.contains("Option") {
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

/// Find BB index of a Call to a function whose path contains `call_substr`,
/// where at least one argument is a FnDef constant with path containing
/// `arg_fn_substr`.
fn find_call_bb_with_fn_arg(
    func: &MirExtractedFn,
    call_substr: &str,
    arg_fn_substr: &str,
) -> Option<u32> {
    for (idx, bb) in func.body.basic_blocks.iter().enumerate() {
        if bb.is_cleanup {
            continue;
        }
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
                    args,
                    ..
                },
            ..
        }) = &bb.terminator
        {
            if path.contains(call_substr) {
                for arg in args {
                    if let MirOperand::Constant {
                        constant:
                            MirConstant {
                                ty: MirTy::Fn { path: ap, .. },
                                ..
                            },
                    } = arg
                    {
                        if ap.contains(arg_fn_substr) {
                            return Some(idx as u32);
                        }
                    }
                }
            }
        }
    }
    None
}

/// Find BB index of a Call to a function whose path contains `call_substr`,
/// where at least one argument is a scalar constant matching `scalar` or an
/// unevaluated const ref containing `const_substr`.
fn find_call_bb_with_const_arg(
    func: &MirExtractedFn,
    call_substr: &str,
    scalar: u128,
    const_substr: &str,
) -> Option<u32> {
    for (idx, bb) in func.body.basic_blocks.iter().enumerate() {
        if bb.is_cleanup {
            continue;
        }
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
                    args,
                    ..
                },
            ..
        }) = &bb.terminator
        {
            if path.contains(call_substr) {
                for arg in args {
                    if operand_has_const_scalar(arg, scalar)
                        || operand_has_const_ref(arg, const_substr)
                    {
                        return Some(idx as u32);
                    }
                }
            }
        }
    }
    None
}

/// Find a MIR local by its authored debug name.
fn find_local_by_name(func: &MirExtractedFn, name: &str) -> Option<u32> {
    func.body
        .local_decls
        .iter()
        .find(|d| d.name.as_deref() == Some(name))
        .map(|d| d.index)
}

/// Check if local 0 (return place) is ever assigned from the specified source
/// local via a Use(Copy/Move) statement in any non-cleanup BB.
fn return_assigned_from_local(func: &MirExtractedFn, source: u32) -> bool {
    for bb in &func.body.basic_blocks {
        if bb.is_cleanup {
            continue;
        }
        for stmt in &bb.statements {
            if let MirStatementKind::Assign { place, rvalue } = &stmt.kind {
                if place.local == 0 && place.projection.is_empty() {
                    match rvalue.as_ref() {
                        MirRvalue::Use {
                            operand: MirOperand::Copy { place: src },
                        }
                        | MirRvalue::Use {
                            operand: MirOperand::Move { place: src },
                        } => {
                            if src.local == source && src.projection.is_empty() {
                                return true;
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }
    false
}

/// Check if the CFG has a back-edge (non-cleanup BB targets a BB with lower
/// or equal index), indicating a loop.
fn has_back_edge(func: &MirExtractedFn) -> bool {
    for (idx, bb) in func.body.basic_blocks.iter().enumerate() {
        if bb.is_cleanup {
            continue;
        }
        let Some(term) = &bb.terminator else {
            continue;
        };
        for target in terminator_successors(&term.kind) {
            if (target as usize) <= idx {
                return true;
            }
        }
    }
    false
}

/// Find all non-cleanup BB indices containing a SwitchInt terminator.
fn find_switch_int_bb_indices(func: &MirExtractedFn) -> Vec<usize> {
    func.body
        .basic_blocks
        .iter()
        .enumerate()
        .filter(|(_, bb)| {
            !bb.is_cleanup
                && bb
                    .terminator
                    .as_ref()
                    .is_some_and(|t| matches!(&t.kind, MirTerminatorKind::SwitchInt { .. }))
        })
        .map(|(idx, _)| idx)
        .collect()
}

/// Check if any non-cleanup BB in the set assigns a constant scalar to a
/// specific local.
fn bb_set_assigns_local_scalar(
    func: &MirExtractedFn,
    bbs: &std::collections::HashSet<u32>,
    local: u32,
    value: u128,
) -> bool {
    for &bb_idx in bbs {
        let Some(bb) = func.body.basic_blocks.get(bb_idx as usize) else {
            continue;
        };
        if bb.is_cleanup {
            continue;
        }
        for stmt in &bb.statements {
            if let MirStatementKind::Assign { place, rvalue } = &stmt.kind {
                if place.local == local && place.projection.is_empty() {
                    if let MirRvalue::Use { operand } = rvalue.as_ref() {
                        if operand_has_const_scalar(operand, value) {
                            return true;
                        }
                    }
                }
            }
        }
    }
    false
}

/// Check if a Call in any non-cleanup BB in the set has two non-constant
/// arguments (both Copy/Move of locals) that share a common source, proving
/// the same computed value is used in both argument positions.
///
/// This handles the case where the compiler creates a temporary copy for one
/// of the arguments (e.g. `raw_reg_val(copy _3, move _5)` where `_5 = copy _3`).
fn bb_set_has_call_with_dual_use_args(
    func: &MirExtractedFn,
    bbs: &std::collections::HashSet<u32>,
    call_substr: &str,
) -> bool {
    for &idx in bbs {
        let Some(bb) = func.body.basic_blocks.get(idx as usize) else {
            continue;
        };
        if bb.is_cleanup {
            continue;
        }
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
                    args,
                    ..
                },
            ..
        }) = &bb.terminator
        {
            if path.contains(call_substr) && args.len() >= 2 {
                let l0 = match &args[0] {
                    MirOperand::Copy { place } | MirOperand::Move { place }
                        if place.projection.is_empty() =>
                    {
                        Some(place.local)
                    }
                    _ => None,
                };
                let l1 = match &args[1] {
                    MirOperand::Copy { place } | MirOperand::Move { place }
                        if place.projection.is_empty() =>
                    {
                        Some(place.local)
                    }
                    _ => None,
                };
                if let (Some(a), Some(b)) = (l0, l1) {
                    // Same local: direct dual-use.
                    if a == b {
                        return true;
                    }
                    // One local is a copy of the other.
                    if local_is_copy_of(func, a, b) || local_is_copy_of(func, b, a) {
                        return true;
                    }
                    // Both locals are copies of a common third local
                    // (compiler creates two temporaries from the same source).
                    let src_a = copy_source(func, a);
                    let src_b = copy_source(func, b);
                    if let (Some(sa), Some(sb)) = (src_a, src_b) {
                        if sa == sb {
                            return true;
                        }
                    }
                }
            }
        }
    }
    false
}

/// Find the source local that `local` was assigned from via Use(Copy/Move),
/// if such a simple assignment exists.
fn copy_source(func: &MirExtractedFn, local: u32) -> Option<u32> {
    for bb in &func.body.basic_blocks {
        if bb.is_cleanup {
            continue;
        }
        for stmt in &bb.statements {
            if let MirStatementKind::Assign { place, rvalue } = &stmt.kind {
                if place.local == local && place.projection.is_empty() {
                    if let MirRvalue::Use { operand } = rvalue.as_ref() {
                        match operand {
                            MirOperand::Copy { place: src } | MirOperand::Move { place: src } => {
                                if src.projection.is_empty() {
                                    return Some(src.local);
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    }
    None
}

/// Check if `local` was assigned as a Use(Copy/Move) of `source` in any
/// non-cleanup BB.
fn local_is_copy_of(func: &MirExtractedFn, local: u32, source: u32) -> bool {
    for bb in &func.body.basic_blocks {
        if bb.is_cleanup {
            continue;
        }
        for stmt in &bb.statements {
            if let MirStatementKind::Assign { place, rvalue } = &stmt.kind {
                if place.local == local && place.projection.is_empty() {
                    if let MirRvalue::Use { operand } = rvalue.as_ref() {
                        match operand {
                            MirOperand::Copy { place: src } | MirOperand::Move { place: src } => {
                                if src.local == source && src.projection.is_empty() {
                                    return true;
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    }
    false
}

/// Check if any non-cleanup BB in the set has a Return terminator.
fn bb_set_has_return(func: &MirExtractedFn, bbs: &std::collections::HashSet<u32>) -> bool {
    for &idx in bbs {
        let Some(bb) = func.body.basic_blocks.get(idx as usize) else {
            continue;
        };
        if bb.is_cleanup {
            continue;
        }
        if let Some(MirTerminator {
            kind: MirTerminatorKind::Return,
            ..
        }) = &bb.terminator
        {
            return true;
        }
    }
    false
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
        fixture.contains("dr::DATA, byte as u32"),
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

    // -- Path-sensitive assertions --

    // Higher-order combinator proof: the `map` call carries `Clock::get_rate`
    // as a FnDef argument, proving the authored
    // `self.clock.as_ref().map(Clock::get_rate)` combinator structure.
    let map_bb = find_call_bb_with_fn_arg(f, "map", "get_rate");
    assert!(
        map_bb.is_some(),
        "uart_clock_hz: map must carry Clock::get_rate as FnDef argument \
         (proves higher-order combinator flow from self.clock)"
    );

    // Fallback proof: the `unwrap_or` call carries the DEFAULT_UART_CLOCK_HZ
    // constant (48_000_000) as an argument, proving the authored fallback path.
    let unwrap_bb =
        find_call_bb_with_const_arg(f, "unwrap_or", 48_000_000, "DEFAULT_UART_CLOCK_HZ");
    assert!(
        unwrap_bb.is_some(),
        "uart_clock_hz: unwrap_or must carry DEFAULT_UART_CLOCK_HZ (48_000_000) \
         as fallback argument (proves exact fallback path)"
    );

    // Flow ordering proof: unwrap_or BB is on the straight-line path from map
    // BB, proving the authored `map(...).unwrap_or(...)` combinator chain.
    let map_idx = map_bb.unwrap();
    let unwrap_idx = unwrap_bb.unwrap();
    let path_from_map = straight_line_bbs(f, map_idx);
    assert!(
        path_from_map.contains(&unwrap_idx),
        "uart_clock_hz: unwrap_or must be reachable from map's straight-line \
         path (proves combinator->fallback ordering)"
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

    // -- Path-sensitive assertion --
    // Prove that `pending_tx = None` occurs after the four helper calls.
    // Find the BB containing the `disable_uart` call (last helper in the
    // authored sequence) and verify the None assignment appears on the
    // straight-line path from its target.
    let disable_uart_bb = f
        .body
        .basic_blocks
        .iter()
        .enumerate()
        .find_map(|(idx, bb)| {
            if bb.is_cleanup {
                return None;
            }
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
                        target: Some(_),
                        ..
                    },
                ..
            }) = &bb.terminator
            {
                if path.contains("disable_uart") {
                    return Some(idx as u32);
                }
            }
            None
        });
    assert!(
        disable_uart_bb.is_some(),
        "shutdown_port must have a disable_uart call with a target BB"
    );
    let du_bb = disable_uart_bb.unwrap();
    // Get the target BB of the disable_uart Call terminator.
    let du_target = match &f.body.basic_blocks[du_bb as usize]
        .terminator
        .as_ref()
        .unwrap()
        .kind
    {
        MirTerminatorKind::Call {
            target: Some(t), ..
        } => *t,
        _ => panic!("expected Call terminator for disable_uart"),
    };
    let path_after_disable_uart: std::collections::HashSet<u32> =
        straight_line_bbs(f, du_target).into_iter().collect();
    assert!(
        bb_set_has_none_assignment(f, &path_after_disable_uart),
        "shutdown_port: pending_tx = None must occur after disable_uart \
         in the MIR control-flow path (proves exact placement)"
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

    // -- Path-sensitive assertions --

    // Loop proof: the CFG must have a back-edge (proving a while-loop exists).
    assert!(
        has_back_edge(f),
        "service_rx_fifo: CFG must have a back-edge (proves while loop)"
    );

    // `handled` local proof: find the authored `handled` variable, verify it
    // is assigned false (0) and true (1) at different points, and that the
    // return place (local 0) is assigned from it.
    let handled = find_local_by_name(f, "handled");
    assert!(
        handled.is_some(),
        "service_rx_fifo: must have a local named 'handled'"
    );
    let handled = handled.unwrap();

    // `handled` is initialized to false (0).
    let all_bbs: std::collections::HashSet<u32> = (0..f.body.basic_blocks.len() as u32).collect();
    assert!(
        bb_set_assigns_local_scalar(f, &all_bbs, handled, 0),
        "service_rx_fifo: 'handled' must be assigned false (0)"
    );

    // `handled = true` on the loop-body path. Find the SwitchInt for the loop
    // condition (rx_char result discriminant). The "Some" branch (loop body)
    // must assign handled = true.
    let switch_bbs = find_switch_int_bb_indices(f);
    assert!(
        !switch_bbs.is_empty(),
        "service_rx_fifo: must have SwitchInt for loop condition"
    );
    // At least one SwitchInt branch must lead to a path where handled = true.
    let mut found_true_on_branch = false;
    for &sbb in &switch_bbs {
        let Some(MirTerminator {
            kind: MirTerminatorKind::SwitchInt { targets, .. },
            ..
        }) = &f.body.basic_blocks[sbb].terminator
        else {
            continue;
        };
        for &target in &targets.targets {
            let branch_bbs = reachable_bbs_from(f, target);
            if bb_set_assigns_local_scalar(f, &branch_bbs, handled, 1) {
                found_true_on_branch = true;
                break;
            }
        }
        if found_true_on_branch {
            break;
        }
    }
    assert!(
        found_true_on_branch,
        "service_rx_fifo: 'handled = true' must occur on a branch of the \
         loop condition SwitchInt (proves handled is set on successful iteration)"
    );

    // Return value proof: the return place (local 0) must be assigned from
    // the `handled` local, proving the returned bool is exactly `handled`.
    assert!(
        return_assigned_from_local(f, handled),
        "service_rx_fifo: return place must be assigned from 'handled' local \
         (proves the returned bool is exactly the handled variable)"
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

    // -- Path-sensitive assertions --
    // The authored function has three distinct paths:
    //   Some(byte) + tx_ready:     write DR, pending_tx=None, disable_tx, return true
    //   Some(byte) + !tx_ready:    return false
    //   None:                      disable_tx, return true
    //
    // Prove the exact branch structure by finding two SwitchInt terminators
    // and verifying that specific operations appear only on the correct paths.

    let switch_bbs = find_switch_int_bb_indices(f);
    assert!(
        switch_bbs.len() >= 2,
        "service_tx_fifo must have at least 2 SwitchInt terminators \
         (pending_tx Option + tx_ready bool), found {}",
        switch_bbs.len()
    );

    // The first SwitchInt is the pending_tx Option discriminant (Some/None).
    let first_switch_bb = switch_bbs[0];
    let MirTerminator {
        kind:
            MirTerminatorKind::SwitchInt {
                targets: first_targets,
                ..
            },
        ..
    } = f.body.basic_blocks[first_switch_bb]
        .terminator
        .as_ref()
        .unwrap()
    else {
        panic!("expected SwitchInt");
    };

    // For Option, value 1 = Some; otherwise = None.
    // targets layout: [value_1_target, ..., otherwise_target]
    let some_target = first_targets.targets[0]; // value 1 = Some
    let none_target = *first_targets.targets.last().unwrap(); // otherwise = None

    // None path: must have disable_tx_interrupts, must NOT have write (field_val).
    let none_bbs = reachable_bbs_from(f, none_target);
    // Exclude BBs also reachable from Some (reconvergence) by checking that
    // the None path reaches Return directly.
    assert!(
        bb_set_has_call(f, &none_bbs, "disable_tx_interrupts"),
        "service_tx_fifo None path must call disable_tx_interrupts"
    );
    assert!(
        bb_set_has_return(f, &none_bbs),
        "service_tx_fifo None path must reach Return"
    );

    // Some path: must contain tx_ready call and a second SwitchInt.
    let some_bbs = reachable_bbs_from(f, some_target);
    assert!(
        bb_set_has_call(f, &some_bbs, "tx_ready"),
        "service_tx_fifo Some path must call tx_ready"
    );

    // Find the second SwitchInt (tx_ready result) on the Some path.
    let second_switch = switch_bbs
        .iter()
        .find(|&&sbb| sbb != first_switch_bb && some_bbs.contains(&(sbb as u32)));
    assert!(
        second_switch.is_some(),
        "service_tx_fifo Some path must have a second SwitchInt for tx_ready"
    );
    let second_switch_bb = *second_switch.unwrap();
    let MirTerminator {
        kind:
            MirTerminatorKind::SwitchInt {
                targets: second_targets,
                ..
            },
        ..
    } = f.body.basic_blocks[second_switch_bb]
        .terminator
        .as_ref()
        .unwrap()
    else {
        panic!("expected SwitchInt");
    };

    // For bool SwitchInt: value 0 = false (not ready); otherwise = true (ready).
    let not_ready_target = second_targets.targets[0]; // value 0 = not ready
    let ready_target = *second_targets.targets.last().unwrap(); // otherwise = ready

    // Ready path: must have DR write (field_val), pending_tx=None,
    // disable_tx_interrupts.
    let ready_bbs = reachable_bbs_from(f, ready_target);
    assert!(
        bb_set_has_call(f, &ready_bbs, "field_val"),
        "service_tx_fifo ready path must call field_val (DR.DATA write)"
    );
    assert!(
        bb_set_has_none_assignment(f, &ready_bbs),
        "service_tx_fifo ready path must set pending_tx = None"
    );
    assert!(
        bb_set_has_call(f, &ready_bbs, "disable_tx_interrupts"),
        "service_tx_fifo ready path must call disable_tx_interrupts"
    );

    // Not-ready path: must NOT have DR write or pending_tx=None.
    let not_ready_bbs = reachable_bbs_from(f, not_ready_target);
    assert!(
        !bb_set_has_call(f, &not_ready_bbs, "field_val"),
        "service_tx_fifo not-ready path must NOT call field_val (no DR write)"
    );
    assert!(
        !bb_set_has_none_assignment(f, &not_ready_bbs),
        "service_tx_fifo not-ready path must NOT assign pending_tx = None"
    );
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

    // -- Path-sensitive assertions --
    // Prove the exact conditional-write behavior:
    //   clear_mask = status & EXPLICIT_ISR_CLEAR_MASK
    //   if clear_mask != 0 { write icr using clear_mask as both mask and value }

    // Find the SwitchInt for the `clear_mask != 0` guard.
    let switch_bbs = find_switch_int_bb_indices(f);
    assert!(
        !switch_bbs.is_empty(),
        "clear_explicit_interrupts must have SwitchInt for clear_mask guard"
    );

    // The SwitchInt for `!= 0`: value 0 = zero branch; otherwise = nonzero.
    let guard_bb = switch_bbs[0];
    let MirTerminator {
        kind:
            MirTerminatorKind::SwitchInt {
                targets: guard_targets,
                ..
            },
        ..
    } = f.body.basic_blocks[guard_bb].terminator.as_ref().unwrap()
    else {
        panic!("expected SwitchInt");
    };

    // value 0 -> zero branch (no write); otherwise -> nonzero branch (write).
    let zero_target = guard_targets.targets[0]; // value 0
    let nonzero_target = *guard_targets.targets.last().unwrap(); // otherwise

    // Nonzero branch: must have raw_reg_val + write calls.
    let nonzero_bbs = reachable_bbs_from(f, nonzero_target);
    assert!(
        bb_set_has_call(f, &nonzero_bbs, "raw_reg_val"),
        "clear_explicit_interrupts nonzero branch must call raw_reg_val"
    );
    assert!(
        bb_set_has_call(f, &nonzero_bbs, "write"),
        "clear_explicit_interrupts nonzero branch must call write"
    );

    // Zero branch: must NOT have raw_reg_val or write calls (write only when
    // clear_mask != 0).
    let zero_bbs = reachable_bbs_from(f, zero_target);
    assert!(
        !bb_set_has_call(f, &zero_bbs, "raw_reg_val"),
        "clear_explicit_interrupts zero branch must NOT call raw_reg_val \
         (write path occurs only when clear_mask != 0)"
    );
    assert!(
        !bb_set_has_call(f, &zero_bbs, "write"),
        "clear_explicit_interrupts zero branch must NOT call write \
         (write path occurs only when clear_mask != 0)"
    );

    // Dual-use proof: raw_reg_val must be called with the same local as both
    // arguments, proving clear_mask is used as both the raw write mask and
    // the raw write value.
    assert!(
        bb_set_has_call_with_dual_use_args(f, &nonzero_bbs, "raw_reg_val"),
        "clear_explicit_interrupts: raw_reg_val must receive clear_mask as \
         both mask and value arguments (proves dual-use)"
    );
}
