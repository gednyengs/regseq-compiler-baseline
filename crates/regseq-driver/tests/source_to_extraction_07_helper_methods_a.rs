//! source-to-extraction-07: PL011 helper methods A authored-to-extraction
//! boundary validation.
//!
//! Validates that the first half of PL011 helper-method facts survive at the
//! compiler-side extraction boundary. Uses an inline specimen compiled through
//! rustc_driver with body and identity enrichment extraction.

#![feature(rustc_private)]
#![feature(box_patterns)]

extern crate rustc_ast;
extern crate rustc_driver;
extern crate rustc_hir;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_session;

use std::sync::{Arc, Mutex, Once};

// Provide `crate::mir_repr` so the #[path]-inlined extraction modules
// can resolve `crate::mir_repr::*`.
#[path = "../src/mir_repr.rs"]
mod mir_repr;

#[allow(unused_imports)]
use mir_repr::*;

// Inline the pub(crate) extraction modules via the wrapper module that
// provides the correct `super::mir_extract` resolution.
mod helper_methods_a_extract;

use helper_methods_a_extract::body;
use helper_methods_a_extract::body::extract_body_surface;
use helper_methods_a_extract::identity_enrichment::extract_identity_enrichment;
use helper_methods_a_extract::identity_enrichment_types::*;

// ============================================================================
// Compiler harness
// ============================================================================

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

/// Result of compiling and extracting both surfaces.
struct ExtractionResult {
    identity: IdentityEnrichmentSurface,
    bodies: MirBodySurface,
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

    type SurfacePair = (
        Result<IdentityEnrichmentSurface, IdentityEnrichmentError>,
        Result<MirBodySurface, body::BodyError>,
    );
    let result: Arc<Mutex<Option<SurfacePair>>> = Arc::new(Mutex::new(None));

    struct ExtractCallbacks {
        result: Arc<Mutex<Option<SurfacePair>>>,
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
            config
                .opts
                .unstable_opts
                .crate_attr
                .push(
                    "allow(unused, non_snake_case, nonstandard_style, redundant_semicolons)"
                        .to_string(),
                );
            config.opts.unstable_opts.mir_opt_level = Some(0);
            config.opts.unstable_opts.always_encode_mir = true;
        }

        fn after_analysis<'tcx>(
            &mut self,
            _compiler: &rustc_interface::interface::Compiler,
            tcx: rustc_middle::ty::TyCtxt<'tcx>,
        ) -> rustc_driver::Compilation {
            let identity = extract_identity_enrichment(tcx);
            let bodies = extract_body_surface(tcx);
            *self.result.lock().unwrap() = Some((identity, bodies));
            rustc_driver::Compilation::Stop
        }
    }

    let mut callbacks = ExtractCallbacks {
        result: result.clone(),
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

    let pair: SurfacePair = result
        .lock()
        .unwrap()
        .take()
        .expect("after_analysis should have been called");

    std::fs::remove_dir_all(&test_dir).ok();

    ExtractionResult {
        identity: pair
            .0
            .expect("identity enrichment extraction should succeed"),
        bodies: pair.1.expect("body extraction should succeed"),
    }
}

// ============================================================================
// Inline specimen
// ============================================================================

/// Inline specimen that faithfully mirrors the PL011 helper methods in scope.
///
/// This specimen provides stub types (`Mmio`, `Field`, `FieldValue`) with
/// the same structural API used by the real PL011 fixture, so that the MIR
/// body shapes are equivalent. Method signatures, body structure, call
/// sequences, arithmetic, literals, and ordering patterns are preserved
/// verbatim from the fixture source.
const SPECIMEN: &str = r#"
// Stub types mirroring regseq facade types for self-contained compilation.

pub struct Field<T, R> {
    pub mask: T,
    pub shift: T,
    _r: core::marker::PhantomData<R>,
}

impl<T, R> Field<T, R> {
    pub const fn new(mask: T, shift: T) -> Self {
        Self { mask, shift, _r: core::marker::PhantomData }
    }
}

pub struct FieldValue<T, R> {
    mask: T,
    value: T,
    _r: core::marker::PhantomData<R>,
}

impl<T: Copy, R> FieldValue<T, R> {
    pub fn new(mask: T, value: T) -> Self {
        Self { mask, value, _r: core::marker::PhantomData }
    }
}

impl<R> core::ops::Add for FieldValue<u32, R> {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self { mask: self.mask | rhs.mask, value: self.value | rhs.value, _r: core::marker::PhantomData }
    }
}

pub struct Register<T, R> {
    value: T,
    _r: core::marker::PhantomData<R>,
}

impl<R> Register<u32, R> {
    pub fn write(&mut self, fv: FieldValue<u32, R>) {
        self.value = fv.value;
    }
    pub fn modify(&mut self, fv: FieldValue<u32, R>) {
        self.value = (self.value & !fv.mask) | fv.value;
    }
    pub fn read(&self, field: Field<u32, R>) -> u32 {
        (self.value & field.mask) >> field.shift
    }
    pub fn read_raw(&self) -> u32 { self.value }
}

// Stub register module types.
pub enum CrReg {}
pub enum LcrhReg {}
pub enum ImscReg {}
pub enum IcrReg {}
pub enum IflsReg {}
pub enum IbrdReg {}
pub enum FbrdReg {}

// Named field constants mirroring the register module.
pub mod CR_REG {
    pub type Register = super::CrReg;
    pub mod UARTEN { use super::super::*; pub const ENABLED: FieldValue<u32, CrReg> = FieldValue { mask: 1, value: 1, _r: core::marker::PhantomData }; pub const DISABLED: FieldValue<u32, CrReg> = FieldValue { mask: 1, value: 0, _r: core::marker::PhantomData }; }
    pub mod TXE { use super::super::*; pub const ENABLED: FieldValue<u32, CrReg> = FieldValue { mask: 0x100, value: 0x100, _r: core::marker::PhantomData }; pub const DISABLED: FieldValue<u32, CrReg> = FieldValue { mask: 0x100, value: 0, _r: core::marker::PhantomData }; }
    pub mod RXE { use super::super::*; pub const ENABLED: FieldValue<u32, CrReg> = FieldValue { mask: 0x200, value: 0x200, _r: core::marker::PhantomData }; pub const DISABLED: FieldValue<u32, CrReg> = FieldValue { mask: 0x200, value: 0, _r: core::marker::PhantomData }; }
}

pub mod LCRH_REG {
    pub type Register = super::LcrhReg;
    pub mod WLEN { use super::super::*; pub const EIGHT_BITS: FieldValue<u32, LcrhReg> = FieldValue { mask: 0x60, value: 0x60, _r: core::marker::PhantomData }; }
    pub mod FEN { use super::super::*; pub const ENABLED: FieldValue<u32, LcrhReg> = FieldValue { mask: 0x10, value: 0x10, _r: core::marker::PhantomData }; pub const DISABLED: FieldValue<u32, LcrhReg> = FieldValue { mask: 0x10, value: 0, _r: core::marker::PhantomData }; }
}

pub mod IMSC_REG {
    pub type Register = super::ImscReg;
}

pub mod ICR_REG {
    pub type Register = super::IcrReg;
}

pub mod IFLS_REG {
    pub type Register = super::IflsReg;
    pub mod RXIFLSEL { use super::super::*; pub const ONE_HALF: FieldValue<u32, IflsReg> = FieldValue { mask: 0x38, value: 0x10, _r: core::marker::PhantomData }; }
    pub mod TXIFLSEL { use super::super::*; pub const ONE_EIGHTH: FieldValue<u32, IflsReg> = FieldValue { mask: 0x07, value: 0x00, _r: core::marker::PhantomData }; }
}

pub mod IBRD_REG {
    pub type Register = super::IbrdReg;
    pub mod BAUD_DIVINT { use super::super::*; pub const FIELD: Field<u32, IbrdReg> = Field::new(0xffff, 0); }
}

pub mod FBRD_REG {
    pub type Register = super::FbrdReg;
    pub mod BAUD_DIVFRAC { use super::super::*; pub const FIELD: Field<u32, FbrdReg> = Field::new(0x3f, 0); }
}

// Stub Mmio-like register block.
pub struct Pl011Registers {
    pub cr: Register<u32, CrReg>,
    pub lcr_h: Register<u32, LcrhReg>,
    pub imsc: Register<u32, ImscReg>,
    pub icr: Register<u32, IcrReg>,
    pub ifls: Register<u32, IflsReg>,
    pub ibrd: Register<u32, IbrdReg>,
    pub fbrd: Register<u32, FbrdReg>,
}

// Stub field_val / raw_reg_val utilities matching the fixture.
fn field_val<R>(field: Field<u32, R>, raw: u32) -> FieldValue<u32, R> {
    FieldValue::new(field.mask, raw << field.shift)
}

fn raw_reg_val<R>(mask: u32, value: u32) -> FieldValue<u32, R> {
    FieldValue::new(mask, value)
}

// Ordered macro stub -- wraps content in compiler fences.
macro_rules! ordered {
    ($($stmt:stmt)*) => {
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
        $($stmt)*
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
    };
}

// Stub trait to provide a callback root for body extraction discovery.
pub trait Lifecycle {
    fn init(&mut self);
}

// The driver struct.
#[rseq::driver(name = "pl011-full-spec", compatible = ["arm,pl011"])]
pub struct Pl011FullSpec {
    pub regs: Pl011Registers,
}

// The inherent impl with all 12 in-scope helper methods.
impl Pl011FullSpec {
    const ALL_UART_INTERRUPTS_MASK: u32 = 0x07ff;
    const RX_INTERRUPT_MASK: u32 = (1 << 4) | (1 << 6);
    const TX_INTERRUPT_MASK: u32 = 1 << 5;

    fn disable_uart(&mut self) {
        self.regs
            .cr
            .write(CR_REG::UARTEN::DISABLED + CR_REG::TXE::DISABLED + CR_REG::RXE::DISABLED);
    }

    fn disable_fifo(&mut self) {
        self.regs.lcr_h.modify(LCRH_REG::FEN::DISABLED);
    }

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

    fn configure_interrupt_levels(&mut self) {
        self.regs
            .ifls
            .write(IFLS_REG::RXIFLSEL::ONE_HALF + IFLS_REG::TXIFLSEL::ONE_EIGHTH);
    }

    fn set_baud_rate(&mut self, uart_clk_hz: u32, baud_rate: u32) {
        let div_times_64 = (uart_clk_hz as u64 * 4) / baud_rate as u64;
        let ibrd = (div_times_64 >> 6) as u32;
        let fbrd = (div_times_64 & 0x3f) as u32;

        self.regs
            .ibrd
            .write(field_val(IBRD_REG::BAUD_DIVINT::FIELD, ibrd));
        self.regs
            .fbrd
            .write(field_val(FBRD_REG::BAUD_DIVFRAC::FIELD, fbrd));
    }

    fn configure_line_control(&mut self) {
        self.regs
            .lcr_h
            .write(LCRH_REG::WLEN::EIGHT_BITS + LCRH_REG::FEN::ENABLED);
    }

    fn enable_uart(&mut self) {
        self.regs
            .cr
            .write(CR_REG::UARTEN::ENABLED + CR_REG::TXE::ENABLED + CR_REG::RXE::ENABLED);
    }

    fn enable_rx_interrupts(&mut self) {
        self.regs.imsc.modify(raw_reg_val::<IMSC_REG::Register>(
            Self::RX_INTERRUPT_MASK,
            Self::RX_INTERRUPT_MASK,
        ));
    }

    fn enable_tx_interrupts(&mut self) {
        self.regs.imsc.modify(raw_reg_val::<IMSC_REG::Register>(
            Self::TX_INTERRUPT_MASK,
            Self::TX_INTERRUPT_MASK,
        ));
    }

    fn disable_tx_interrupts(&mut self) {
        self.regs.imsc.modify(raw_reg_val::<IMSC_REG::Register>(
            Self::TX_INTERRUPT_MASK,
            0,
        ));
    }

    fn program_polling_port(&mut self, uart_clk_hz: u32, baud_rate: u32) {
        self.disable_uart();
        self.disable_fifo();
        ordered! {
            self.set_baud_rate(uart_clk_hz, baud_rate);
            self.configure_line_control();
            self.configure_interrupt_levels();
            self.mask_all_interrupts();
            self.clear_all_interrupts();
            self.enable_uart();
            self.enable_rx_interrupts();
        }
    }
}

// Trait impl provides a callback root so body extraction discovers helpers.
impl Lifecycle for Pl011FullSpec {
    fn init(&mut self) {
        self.program_polling_port(48000000, 115200);
        self.enable_tx_interrupts();
        self.disable_tx_interrupts();
    }
}
"#;

// ============================================================================
// Helper: find a function by name in the body surface
// ============================================================================

fn find_fn<'a>(surface: &'a MirBodySurface, name: &str) -> &'a MirExtractedFn {
    surface
        .functions
        .iter()
        .find(|f| f.name == name)
        .unwrap_or_else(|| panic!("function `{name}` not found in body surface"))
}

/// Collect all Call terminators from a MIR body, returning (callee_path, args_count).
fn collect_calls(body: &MirBody) -> Vec<(String, usize)> {
    let mut calls = Vec::new();
    for bb in &body.basic_blocks {
        if let Some(ref term) = bb.terminator {
            if let MirTerminatorKind::Call {
                ref func, ref args, ..
            } = term.kind
            {
                let path = match func {
                    MirOperand::Constant { constant } => match &constant.ty {
                        MirTy::Fn { path, .. } => path.clone(),
                        _ => "<non-fndef>".to_string(),
                    },
                    _ => "<non-const>".to_string(),
                };
                calls.push((path, args.len()));
            }
        }
    }
    calls
}

/// Collect all constant operands from the entire body (statements + terminators).
/// Returns pairs of (description_or_scalar, MirConstValue).
fn collect_all_constants(body: &MirBody) -> Vec<MirConstant> {
    let mut consts = Vec::new();

    let collect_from_operand = |op: &MirOperand, out: &mut Vec<MirConstant>| {
        if let MirOperand::Constant { constant } = op {
            out.push(constant.clone());
        }
    };

    for bb in &body.basic_blocks {
        for stmt in &bb.statements {
            if let MirStatementKind::Assign { ref rvalue, .. } = stmt.kind {
                match rvalue.as_ref() {
                    MirRvalue::Use { operand } => collect_from_operand(operand, &mut consts),
                    MirRvalue::BinaryOp { lhs, rhs, .. } => {
                        collect_from_operand(lhs, &mut consts);
                        collect_from_operand(rhs, &mut consts);
                    }
                    MirRvalue::UnaryOp { operand, .. } => {
                        collect_from_operand(operand, &mut consts);
                    }
                    MirRvalue::Cast { operand, .. } => {
                        collect_from_operand(operand, &mut consts);
                    }
                    _ => {}
                }
            }
        }
        if let Some(ref term) = bb.terminator {
            match &term.kind {
                MirTerminatorKind::Call { func, args, .. } => {
                    collect_from_operand(func, &mut consts);
                    for arg in args {
                        collect_from_operand(arg, &mut consts);
                    }
                }
                MirTerminatorKind::Assert { cond, .. } => {
                    collect_from_operand(cond, &mut consts);
                }
                _ => {}
            }
        }
    }
    consts
}

/// Check if a body contains an unevaluated constant reference whose description
/// contains the given substring.
fn has_unevaluated_const(body: &MirBody, name_substring: &str) -> bool {
    collect_all_constants(body).iter().any(|c| {
        matches!(&c.value, MirConstValue::Unevaluated { path }
            if path.contains(name_substring))
    })
}

/// Check if a body contains a scalar constant with the given value.
fn has_scalar_const(body: &MirBody, expected_bits: u128) -> bool {
    collect_all_constants(body)
        .iter()
        .any(|c| matches!(&c.value, MirConstValue::Bits { value, .. } if *value == expected_bits))
}

/// Count unevaluated constant references matching a substring.
fn count_unevaluated_const(body: &MirBody, name_substring: &str) -> usize {
    collect_all_constants(body)
        .iter()
        .filter(|c| {
            matches!(&c.value, MirConstValue::Unevaluated { path }
                if path.contains(name_substring))
        })
        .count()
}

/// Recursively check if a MirTy mentions a given substring in any path.
fn ty_mentions(ty: &MirTy, substr: &str) -> bool {
    match ty {
        MirTy::Struct { path, fields } => {
            path.contains(substr) || fields.iter().any(|f| ty_mentions(&f.ty, substr))
        }
        MirTy::Enum { path, variants } => {
            path.contains(substr)
                || variants
                    .iter()
                    .any(|v| v.fields.iter().any(|f| ty_mentions(&f.ty, substr)))
        }
        MirTy::Fn { path, params, ret } => {
            path.contains(substr)
                || params.iter().any(|p| ty_mentions(p, substr))
                || ty_mentions(ret, substr)
        }
        MirTy::Ref { ty: inner, .. } | MirTy::RawPtr { ty: inner, .. } => {
            ty_mentions(inner, substr)
        }
        MirTy::Tuple { elements } => elements.iter().any(|e| ty_mentions(e, substr)),
        MirTy::Array { element, .. } | MirTy::Slice { element } => ty_mentions(element, substr),
        MirTy::NominalRef { path } => path.contains(substr),
        MirTy::ZeroSizedLike { description } => description.contains(substr),
        MirTy::Unsupported { description } => description.contains(substr),
        _ => false,
    }
}

/// Check if a body has a field projection accessing a specific register name.
#[allow(clippy::collapsible_match, clippy::collapsible_if)]
fn has_register_field_access(body: &MirBody, register_type_name: &str) -> bool {
    for bb in &body.basic_blocks {
        for stmt in &bb.statements {
            if let MirStatementKind::Assign { ref rvalue, .. } = stmt.kind {
                if let MirRvalue::Ref { ref place, .. } = rvalue.as_ref() {
                    for proj in &place.projection {
                        if let MirProjectionElem::Field { ref ty, .. } = proj {
                            if ty_mentions(ty, register_type_name) {
                                return true;
                            }
                        }
                    }
                }
            }
        }
    }
    false
}

/// Check if a body contains a call to a function whose path contains the given substring.
fn has_call_containing(body: &MirBody, substring: &str) -> bool {
    collect_calls(body)
        .iter()
        .any(|(path, _)| path.contains(substring))
}

/// Collect ordered call paths from a body.
#[allow(clippy::collapsible_match)]
fn ordered_call_paths(body: &MirBody) -> Vec<String> {
    let mut paths = Vec::new();
    for bb in &body.basic_blocks {
        if let Some(ref term) = bb.terminator {
            if let MirTerminatorKind::Call { ref func, .. } = term.kind {
                if let MirOperand::Constant { constant } = func {
                    if let MirTy::Fn { ref path, .. } = constant.ty {
                        paths.push(path.clone());
                    }
                }
            }
        }
    }
    paths
}

/// Check if a body has a compiler_fence call (evidence of ordered! barrier).
fn has_compiler_fence(body: &MirBody) -> bool {
    has_call_containing(body, "compiler_fence")
}

/// Count how many calls match a substring.
fn count_calls_containing(body: &MirBody, substring: &str) -> usize {
    collect_calls(body)
        .iter()
        .filter(|(path, _)| path.contains(substring))
        .count()
}

/// Retrieve the resolved scalar field at the given index for a constant
/// whose type path contains the given name substring. Resolved unevaluated
/// constants now appear as `MirConstValue::Struct { fields }` with
/// `ConstField` entries containing `MirConstValue::Bits { value }`.
/// Returns `None` if the constant is not found or has no resolved field at
/// that index.
fn resolved_field_bits(body: &MirBody, name_substring: &str, field_index: u32) -> Option<u128> {
    collect_all_constants(body).iter().find_map(|c| {
        // Resolved unevaluated constants use Unevaluated { path } with bracket
        // notation: "path[0=0x1,1=0x0]". Check if path contains the substring
        // and then parse the bracket fields.
        if let MirConstValue::Unevaluated { path } = &c.value {
            if !path.contains(name_substring) {
                return None;
            }
            // Parse bracket notation: "path[idx=val,idx=val]"
            if let Some(bracket_start) = path.find('[') {
                let bracket_content = &path[bracket_start + 1..path.len() - 1];
                for entry in bracket_content.split(',') {
                    let parts: Vec<&str> = entry.splitn(2, '=').collect();
                    if parts.len() == 2 {
                        if let Ok(idx) = parts[0].parse::<u32>() {
                            if idx == field_index {
                                let val_str = parts[1].trim();
                                if let Ok(val) = u128::from_str_radix(
                                    val_str.strip_prefix("0x").unwrap_or(val_str),
                                    16,
                                ) {
                                    return Some(val);
                                }
                            }
                        }
                    }
                }
            }
        }
        None
    })
}

// ============================================================================
// Fixture source loading for specimen fidelity
// ============================================================================

fn fixture_source() -> String {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let fixture_path = format!(
        "{}/../../tests/fixtures/pl011-full-spec/src/driver.rs",
        manifest_dir
    );
    std::fs::read_to_string(&fixture_path)
        .unwrap_or_else(|e| panic!("failed to read fixture at {fixture_path}: {e}"))
}

/// Normalize whitespace for comparison: collapse runs of whitespace to single space, trim.
fn normalize_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

// ============================================================================
// Specimen fidelity checks
// ============================================================================

/// List of all in-scope helper method names.
const IN_SCOPE_HELPERS: &[&str] = &[
    "disable_uart",
    "disable_fifo",
    "mask_all_interrupts",
    "clear_all_interrupts",
    "configure_interrupt_levels",
    "set_baud_rate",
    "configure_line_control",
    "enable_uart",
    "enable_rx_interrupts",
    "enable_tx_interrupts",
    "disable_tx_interrupts",
    "program_polling_port",
];

#[test]
fn specimen_fidelity_method_signatures_match_fixture() {
    let fixture = fixture_source();

    // Verify each method signature appears in both fixture and specimen.
    let signature_fragments = [
        "fn disable_uart(&mut self)",
        "fn disable_fifo(&mut self)",
        "fn mask_all_interrupts(&mut self)",
        "fn clear_all_interrupts(&mut self)",
        "fn configure_interrupt_levels(&mut self)",
        "fn set_baud_rate(&mut self, uart_clk_hz: u32, baud_rate: u32)",
        "fn configure_line_control(&mut self)",
        "fn enable_uart(&mut self)",
        "fn enable_rx_interrupts(&mut self)",
        "fn enable_tx_interrupts(&mut self)",
        "fn disable_tx_interrupts(&mut self)",
        "fn program_polling_port(&mut self, uart_clk_hz: u32, baud_rate: u32)",
    ];

    for sig in &signature_fragments {
        let norm_sig = normalize_ws(sig);
        let norm_fixture = normalize_ws(&fixture);
        assert!(
            norm_fixture.contains(&norm_sig),
            "signature not found in fixture: {sig}"
        );
        let norm_specimen = normalize_ws(SPECIMEN);
        assert!(
            norm_specimen.contains(&norm_sig),
            "signature not found in specimen: {sig}"
        );
    }
}

#[test]
fn specimen_fidelity_body_fragments_match_fixture() {
    let fixture = fixture_source();

    // Key body fragments that must match between fixture and specimen.
    let body_fragments = [
        // disable_uart body
        "CR_REG::UARTEN::DISABLED + CR_REG::TXE::DISABLED + CR_REG::RXE::DISABLED",
        // disable_fifo body
        "self.regs.lcr_h.modify(LCRH_REG::FEN::DISABLED)",
        // mask_all_interrupts body
        "self.regs.imsc.write(raw_reg_val::<IMSC_REG::Register>(",
        // clear_all_interrupts body
        "self.regs.icr.write(raw_reg_val::<ICR_REG::Register>(",
        // configure_interrupt_levels body
        "IFLS_REG::RXIFLSEL::ONE_HALF + IFLS_REG::TXIFLSEL::ONE_EIGHTH",
        // set_baud_rate arithmetic
        "let div_times_64 = (uart_clk_hz as u64 * 4) / baud_rate as u64;",
        "let ibrd = (div_times_64 >> 6) as u32;",
        "let fbrd = (div_times_64 & 0x3f) as u32;",
        // configure_line_control body
        "LCRH_REG::WLEN::EIGHT_BITS + LCRH_REG::FEN::ENABLED",
        // enable_uart body
        "CR_REG::UARTEN::ENABLED + CR_REG::TXE::ENABLED + CR_REG::RXE::ENABLED",
        // constants
        "const ALL_UART_INTERRUPTS_MASK: u32 = 0x07ff;",
        "const RX_INTERRUPT_MASK: u32 = (1 << 4) | (1 << 6);",
        "const TX_INTERRUPT_MASK: u32 = 1 << 5;",
    ];

    for frag in &body_fragments {
        let norm_frag = normalize_ws(frag);
        let norm_fixture = normalize_ws(&fixture);
        assert!(
            norm_fixture.contains(&norm_frag),
            "body fragment not found in fixture: {frag}"
        );
        let norm_specimen = normalize_ws(SPECIMEN);
        assert!(
            norm_specimen.contains(&norm_frag),
            "body fragment not found in specimen: {frag}"
        );
    }
}

#[test]
fn specimen_fidelity_program_polling_port_sequence_matches_fixture() {
    let fixture = fixture_source();

    // The ordered sequence inside program_polling_port.
    let ordered_sequence = [
        "self.set_baud_rate(uart_clk_hz, baud_rate);",
        "self.configure_line_control();",
        "self.configure_interrupt_levels();",
        "self.mask_all_interrupts();",
        "self.clear_all_interrupts();",
        "self.enable_uart();",
        "self.enable_rx_interrupts();",
    ];

    for stmt in &ordered_sequence {
        let norm = normalize_ws(stmt);
        assert!(
            normalize_ws(&fixture).contains(&norm),
            "ordered sequence statement not found in fixture: {stmt}"
        );
        assert!(
            normalize_ws(SPECIMEN).contains(&norm),
            "ordered sequence statement not found in specimen: {stmt}"
        );
    }

    // Verify pre-ordered calls appear in both.
    assert!(normalize_ws(&fixture).contains(&normalize_ws("self.disable_uart();")));
    assert!(normalize_ws(SPECIMEN).contains(&normalize_ws("self.disable_uart();")));
    assert!(normalize_ws(&fixture).contains(&normalize_ws("self.disable_fifo();")));
    assert!(normalize_ws(SPECIMEN).contains(&normalize_ws("self.disable_fifo();")));
}

// ============================================================================
// Check Group 1: Identity, Signature, Return Type Preservation
// ============================================================================

#[test]
fn check_group_1_identity_surface_all_helpers_present() {
    let result = compile_and_extract(SPECIMEN, "cg1_identity");

    // Find the inherent impl block for Pl011FullSpec.
    let inherent = result
        .identity
        .impl_blocks
        .iter()
        .find(|ib| ib.is_inherent && ib.self_type_name == "Pl011FullSpec")
        .expect("inherent impl block for Pl011FullSpec must exist");

    for name in IN_SCOPE_HELPERS {
        let method = inherent
            .methods
            .iter()
            .find(|m| m.name == *name)
            .unwrap_or_else(|| panic!("method `{name}` not in identity surface"));

        // canonical path contains the method name
        assert!(
            method.canonical_path.0.contains(name),
            "canonical path for `{name}` should contain the method name, got: {}",
            method.canonical_path.0
        );

        // none of these are ISR handlers
        assert!(!method.is_isr, "`{name}` should not be marked as ISR");
    }
}

#[test]
fn check_group_1_body_surface_all_helpers_present_with_correct_signatures() {
    let result = compile_and_extract(SPECIMEN, "cg1_body_sigs");

    for name in IN_SCOPE_HELPERS {
        let func = find_fn(&result.bodies, name);

        // All helpers have &mut self receiver.
        assert_eq!(
            func.receiver, "ref_mut",
            "`{name}` should have &mut self receiver"
        );

        // All are classified as Helper.
        assert_eq!(
            func.kind,
            MirFnKind::Helper,
            "`{name}` should be Helper kind"
        );

        // All return unit (None).
        assert_eq!(
            func.return_type, None,
            "`{name}` should have unit return type"
        );

        // set_baud_rate and program_polling_port take 2 params.
        if *name == "set_baud_rate" || *name == "program_polling_port" {
            assert_eq!(func.params.len(), 2, "`{name}` should have 2 parameters");
            assert_eq!(func.params[0].name, "uart_clk_hz");
            assert_eq!(
                func.params[0].ty,
                MirTy::Int {
                    signed: false,
                    width: IntWidth::U32
                }
            );
            assert_eq!(func.params[1].name, "baud_rate");
            assert_eq!(
                func.params[1].ty,
                MirTy::Int {
                    signed: false,
                    width: IntWidth::U32
                }
            );
        } else {
            assert_eq!(
                func.params.len(),
                0,
                "`{name}` should have 0 parameters (besides self)"
            );
        }
    }
}

// ============================================================================
// Check Group 2: Body-Level Downstream Predicate Preservation
// ============================================================================

#[test]
fn check_group_2_disable_uart_body() {
    let result = compile_and_extract(SPECIMEN, "cg2_disable_uart");
    let func = find_fn(&result.bodies, "disable_uart");

    // One write to cr register.
    let calls = collect_calls(&func.body);
    let write_calls: Vec<_> = calls.iter().filter(|(p, _)| p.contains("write")).collect();
    assert_eq!(
        write_calls.len(),
        1,
        "disable_uart: exactly one write call expected"
    );

    // UARTEN::DISABLED, TXE::DISABLED, RXE::DISABLED appear as unevaluated constants.
    assert!(
        has_unevaluated_const(&func.body, "CR_REG::UARTEN::DISABLED"),
        "disable_uart: CR_REG::UARTEN::DISABLED must appear"
    );
    assert!(
        has_unevaluated_const(&func.body, "CR_REG::TXE::DISABLED"),
        "disable_uart: CR_REG::TXE::DISABLED must appear"
    );
    assert!(
        has_unevaluated_const(&func.body, "CR_REG::RXE::DISABLED"),
        "disable_uart: CR_REG::RXE::DISABLED must appear"
    );

    // Register target is cr (CrReg).
    assert!(
        has_register_field_access(&func.body, "CrReg"),
        "disable_uart: must access CrReg register"
    );
}

#[test]
fn check_group_2_disable_fifo_body() {
    let result = compile_and_extract(SPECIMEN, "cg2_disable_fifo");
    let func = find_fn(&result.bodies, "disable_fifo");

    // One modify call on lcr_h (read-modify-write).
    let calls = collect_calls(&func.body);
    let modify_calls: Vec<_> = calls.iter().filter(|(p, _)| p.contains("modify")).collect();
    assert_eq!(
        modify_calls.len(),
        1,
        "disable_fifo: exactly one modify call expected"
    );

    // No write calls (it's a modify, not a write).
    let write_calls: Vec<_> = calls
        .iter()
        .filter(|(p, _)| p.contains("write") && !p.contains("modify"))
        .collect();
    assert_eq!(
        write_calls.len(),
        0,
        "disable_fifo: no write calls expected (uses modify)"
    );

    // FEN::DISABLED appears as unevaluated constant.
    assert!(
        has_unevaluated_const(&func.body, "FEN::DISABLED"),
        "disable_fifo: LCRH_REG::FEN::DISABLED must appear"
    );

    // Register target is lcr_h (LcrhReg).
    assert!(
        has_register_field_access(&func.body, "LcrhReg"),
        "disable_fifo: must access LcrhReg register"
    );

    // Field-locality proof: FEN::DISABLED resolves to mask=0x10, value=0x00.
    // This proves only the FEN bit (bit 4) is affected and its value is 0.
    let fen_mask = resolved_field_bits(&func.body, "FEN::DISABLED", 0)
        .expect("disable_fifo: FEN::DISABLED must have resolved mask field");
    assert_eq!(
        fen_mask, 0x10,
        "disable_fifo: FEN::DISABLED mask must be 0x10 (only FEN bit affected)"
    );
    let fen_value = resolved_field_bits(&func.body, "FEN::DISABLED", 1)
        .expect("disable_fifo: FEN::DISABLED must have resolved value field");
    assert_eq!(
        fen_value, 0x00,
        "disable_fifo: FEN::DISABLED value must be 0x00 (disabled)"
    );
}

#[test]
fn check_group_2_mask_all_interrupts_body() {
    let result = compile_and_extract(SPECIMEN, "cg2_mask_all");
    let func = find_fn(&result.bodies, "mask_all_interrupts");

    // One write to imsc.
    let calls = collect_calls(&func.body);
    let write_calls: Vec<_> = calls.iter().filter(|(p, _)| p.contains("write")).collect();
    assert_eq!(
        write_calls.len(),
        1,
        "mask_all_interrupts: exactly one write call"
    );

    // Uses raw_reg_val.
    assert!(
        has_call_containing(&func.body, "raw_reg_val"),
        "mask_all_interrupts must call raw_reg_val"
    );

    // ALL_UART_INTERRUPTS_MASK is passed as unevaluated constant reference.
    assert!(
        has_unevaluated_const(&func.body, "ALL_UART_INTERRUPTS_MASK"),
        "mask_all_interrupts: ALL_UART_INTERRUPTS_MASK must appear as unevaluated constant"
    );

    // Masked bits written as 0 (scalar).
    assert!(
        has_scalar_const(&func.body, 0),
        "mask_all_interrupts: zero value must appear as scalar constant (masked bits = 0)"
    );

    // Register target is imsc (ImscReg).
    assert!(
        has_register_field_access(&func.body, "ImscReg"),
        "mask_all_interrupts: must access ImscReg register"
    );
}

#[test]
fn check_group_2_clear_all_interrupts_body() {
    let result = compile_and_extract(SPECIMEN, "cg2_clear_all");
    let func = find_fn(&result.bodies, "clear_all_interrupts");

    // One write to icr.
    let calls = collect_calls(&func.body);
    let write_calls: Vec<_> = calls.iter().filter(|(p, _)| p.contains("write")).collect();
    assert_eq!(
        write_calls.len(),
        1,
        "clear_all_interrupts: exactly one write call"
    );

    // Uses raw_reg_val.
    assert!(
        has_call_containing(&func.body, "raw_reg_val"),
        "clear_all_interrupts must call raw_reg_val"
    );

    // Both mask and value are ALL_UART_INTERRUPTS_MASK (appears at least twice as unevaluated).
    let mask_count = count_unevaluated_const(&func.body, "ALL_UART_INTERRUPTS_MASK");
    assert!(
        mask_count >= 2,
        "clear_all_interrupts: ALL_UART_INTERRUPTS_MASK must appear at least twice (mask + value), got {mask_count}"
    );

    // Register target is icr (IcrReg).
    assert!(
        has_register_field_access(&func.body, "IcrReg"),
        "clear_all_interrupts: must access IcrReg register"
    );
}

#[test]
fn check_group_2_configure_interrupt_levels_body() {
    let result = compile_and_extract(SPECIMEN, "cg2_cfg_int_lvl");
    let func = find_fn(&result.bodies, "configure_interrupt_levels");

    // One write to ifls.
    let calls = collect_calls(&func.body);
    let write_calls: Vec<_> = calls.iter().filter(|(p, _)| p.contains("write")).collect();
    assert_eq!(
        write_calls.len(),
        1,
        "configure_interrupt_levels: exactly one write call"
    );

    // Uses RXIFLSEL + TXIFLSEL combination via Add.
    assert!(
        has_unevaluated_const(&func.body, "RXIFLSEL::ONE_HALF"),
        "configure_interrupt_levels: RXIFLSEL::ONE_HALF must appear"
    );
    assert!(
        has_unevaluated_const(&func.body, "TXIFLSEL::ONE_EIGHTH"),
        "configure_interrupt_levels: TXIFLSEL::ONE_EIGHTH must appear"
    );

    // Register target is ifls (IflsReg).
    assert!(
        has_register_field_access(&func.body, "IflsReg"),
        "configure_interrupt_levels: must access IflsReg register"
    );
}

#[test]
fn check_group_2_set_baud_rate_body() {
    let result = compile_and_extract(SPECIMEN, "cg2_set_baud");
    let func = find_fn(&result.bodies, "set_baud_rate");

    // Parameters preserved.
    assert_eq!(func.params.len(), 2);
    assert_eq!(func.params[0].name, "uart_clk_hz");
    assert_eq!(
        func.params[0].ty,
        MirTy::Int {
            signed: false,
            width: IntWidth::U32
        }
    );
    assert_eq!(func.params[1].name, "baud_rate");
    assert_eq!(
        func.params[1].ty,
        MirTy::Int {
            signed: false,
            width: IntWidth::U32
        }
    );

    // u64 widening: check for IntToInt casts in the body (u32 -> u64).
    let has_cast = func.body.basic_blocks.iter().any(|bb| {
        bb.statements.iter().any(|stmt| {
            if let MirStatementKind::Assign { rvalue, .. } = &stmt.kind {
                matches!(
                    rvalue.as_ref(),
                    MirRvalue::Cast {
                        cast_kind: MirCastKind::IntToInt,
                        ..
                    }
                )
            } else {
                false
            }
        })
    });
    assert!(has_cast, "set_baud_rate: u64 widening cast must be present");

    // Divisor arithmetic: multiplication by 4, division, shift right by 6, mask 0x3f.
    // These are inline literals, so they appear as scalar constants in MIR.
    assert!(
        has_scalar_const(&func.body, 4),
        "set_baud_rate: constant 4 must appear (multiply by 4)"
    );

    assert!(
        has_scalar_const(&func.body, 6),
        "set_baud_rate: constant 6 must appear (shift right by 6)"
    );

    // 0x3f = 63
    assert!(
        has_scalar_const(&func.body, 63),
        "set_baud_rate: constant 0x3f (63) must appear (fractional mask)"
    );

    // Two write calls (ibrd + fbrd).
    let calls = collect_calls(&func.body);
    let write_calls: Vec<_> = calls.iter().filter(|(p, _)| p.contains("write")).collect();
    assert_eq!(
        write_calls.len(),
        2,
        "set_baud_rate: exactly two write calls (ibrd, fbrd)"
    );

    // Two field_val calls.
    let field_val_calls: Vec<_> = calls
        .iter()
        .filter(|(p, _)| p.contains("field_val"))
        .collect();
    assert_eq!(
        field_val_calls.len(),
        2,
        "set_baud_rate: exactly two field_val calls"
    );

    // Register targets: ibrd (IbrdReg) and fbrd (FbrdReg).
    assert!(
        has_register_field_access(&func.body, "IbrdReg"),
        "set_baud_rate: must access IbrdReg register"
    );
    assert!(
        has_register_field_access(&func.body, "FbrdReg"),
        "set_baud_rate: must access FbrdReg register"
    );

    // IBRD_REG::BAUD_DIVINT::FIELD and FBRD_REG::BAUD_DIVFRAC::FIELD as unevaluated.
    assert!(
        has_unevaluated_const(&func.body, "IBRD_REG::BAUD_DIVINT::FIELD"),
        "set_baud_rate: IBRD_REG::BAUD_DIVINT::FIELD must appear"
    );
    assert!(
        has_unevaluated_const(&func.body, "FBRD_REG::BAUD_DIVFRAC::FIELD"),
        "set_baud_rate: FBRD_REG::BAUD_DIVFRAC::FIELD must appear"
    );

    // Binary operations for the arithmetic.
    let has_mul = func.body.basic_blocks.iter().any(|bb| {
        bb.statements.iter().any(|stmt| {
            if let MirStatementKind::Assign { rvalue, .. } = &stmt.kind {
                matches!(
                    rvalue.as_ref(),
                    MirRvalue::BinaryOp {
                        op: MirBinOp::Mul,
                        ..
                    } | MirRvalue::BinaryOp {
                        op: MirBinOp::MulUnchecked,
                        ..
                    } | MirRvalue::BinaryOp {
                        op: MirBinOp::MulWithOverflow,
                        ..
                    }
                )
            } else {
                false
            }
        })
    });
    assert!(has_mul, "set_baud_rate: multiplication must appear in MIR");

    let has_div = func.body.basic_blocks.iter().any(|bb| {
        bb.statements.iter().any(|stmt| {
            if let MirStatementKind::Assign { rvalue, .. } = &stmt.kind {
                matches!(
                    rvalue.as_ref(),
                    MirRvalue::BinaryOp {
                        op: MirBinOp::Div,
                        ..
                    }
                )
            } else {
                false
            }
        })
    });
    assert!(has_div, "set_baud_rate: division must appear in MIR");

    let has_shr = func.body.basic_blocks.iter().any(|bb| {
        bb.statements.iter().any(|stmt| {
            if let MirStatementKind::Assign { rvalue, .. } = &stmt.kind {
                matches!(
                    rvalue.as_ref(),
                    MirRvalue::BinaryOp {
                        op: MirBinOp::Shr,
                        ..
                    } | MirRvalue::BinaryOp {
                        op: MirBinOp::ShrUnchecked,
                        ..
                    }
                )
            } else {
                false
            }
        })
    });
    assert!(has_shr, "set_baud_rate: right shift must appear in MIR");

    let has_bitand = func.body.basic_blocks.iter().any(|bb| {
        bb.statements.iter().any(|stmt| {
            if let MirStatementKind::Assign { rvalue, .. } = &stmt.kind {
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
    assert!(has_bitand, "set_baud_rate: bitwise AND must appear in MIR");
}

#[test]
fn check_group_2_configure_line_control_body() {
    let result = compile_and_extract(SPECIMEN, "cg2_cfg_lc");
    let func = find_fn(&result.bodies, "configure_line_control");

    // One write to lcr_h.
    let calls = collect_calls(&func.body);
    let write_calls: Vec<_> = calls.iter().filter(|(p, _)| p.contains("write")).collect();
    assert_eq!(
        write_calls.len(),
        1,
        "configure_line_control: exactly one write call"
    );

    // WLEN::EIGHT_BITS and FEN::ENABLED appear as unevaluated constant references.
    assert!(
        has_unevaluated_const(&func.body, "WLEN::EIGHT_BITS"),
        "configure_line_control: WLEN::EIGHT_BITS must appear"
    );
    assert!(
        has_unevaluated_const(&func.body, "FEN::ENABLED"),
        "configure_line_control: FEN::ENABLED must appear"
    );

    // Register target is lcr_h (LcrhReg).
    assert!(
        has_register_field_access(&func.body, "LcrhReg"),
        "configure_line_control: must access LcrhReg register"
    );

    // Materialized literal 0x70 proof via resolved FieldValue fields.
    // WLEN::EIGHT_BITS has value = 0x60, FEN::ENABLED has value = 0x10.
    // The Add::add impl combines with bitwise OR: 0x60 | 0x10 = 0x70.
    let wlen_value = resolved_field_bits(&func.body, "WLEN::EIGHT_BITS", 1)
        .expect("configure_line_control: WLEN::EIGHT_BITS must have resolved value field");
    assert_eq!(
        wlen_value, 0x60,
        "configure_line_control: WLEN::EIGHT_BITS value must be 0x60"
    );

    let fen_value = resolved_field_bits(&func.body, "FEN::ENABLED", 1)
        .expect("configure_line_control: FEN::ENABLED must have resolved value field");
    assert_eq!(
        fen_value, 0x10,
        "configure_line_control: FEN::ENABLED value must be 0x10"
    );

    // Combined materialized literal: 0x60 | 0x10 = 0x70.
    assert_eq!(
        wlen_value | fen_value,
        0x70,
        "configure_line_control: combined value must equal materialized literal 0x70"
    );
}

#[test]
fn check_group_2_enable_uart_body() {
    let result = compile_and_extract(SPECIMEN, "cg2_enable_uart");
    let func = find_fn(&result.bodies, "enable_uart");

    // One write to cr.
    let calls = collect_calls(&func.body);
    let write_calls: Vec<_> = calls.iter().filter(|(p, _)| p.contains("write")).collect();
    assert_eq!(write_calls.len(), 1, "enable_uart: exactly one write call");

    // UARTEN::ENABLED, TXE::ENABLED, RXE::ENABLED appear as unevaluated constants.
    assert!(
        has_unevaluated_const(&func.body, "CR_REG::UARTEN::ENABLED"),
        "enable_uart: CR_REG::UARTEN::ENABLED must appear"
    );
    assert!(
        has_unevaluated_const(&func.body, "CR_REG::TXE::ENABLED"),
        "enable_uart: CR_REG::TXE::ENABLED must appear"
    );
    assert!(
        has_unevaluated_const(&func.body, "CR_REG::RXE::ENABLED"),
        "enable_uart: CR_REG::RXE::ENABLED must appear"
    );

    // Register target is cr (CrReg).
    assert!(
        has_register_field_access(&func.body, "CrReg"),
        "enable_uart: must access CrReg register"
    );

    // Materialized literal 0x301 proof via resolved FieldValue fields.
    // UARTEN::ENABLED has value = 0x01, TXE::ENABLED has value = 0x100,
    // RXE::ENABLED has value = 0x200.
    // The Add::add impl combines with bitwise OR: 0x01 | 0x100 | 0x200 = 0x301.
    let uarten_value = resolved_field_bits(&func.body, "CR_REG::UARTEN::ENABLED", 1)
        .expect("enable_uart: CR_REG::UARTEN::ENABLED must have resolved value field");
    assert_eq!(
        uarten_value, 0x01,
        "enable_uart: UARTEN::ENABLED value must be 0x01"
    );

    let txe_value = resolved_field_bits(&func.body, "CR_REG::TXE::ENABLED", 1)
        .expect("enable_uart: CR_REG::TXE::ENABLED must have resolved value field");
    assert_eq!(
        txe_value, 0x100,
        "enable_uart: TXE::ENABLED value must be 0x100"
    );

    let rxe_value = resolved_field_bits(&func.body, "CR_REG::RXE::ENABLED", 1)
        .expect("enable_uart: CR_REG::RXE::ENABLED must have resolved value field");
    assert_eq!(
        rxe_value, 0x200,
        "enable_uart: RXE::ENABLED value must be 0x200"
    );

    // Combined materialized literal: 0x01 | 0x100 | 0x200 = 0x301.
    assert_eq!(
        uarten_value | txe_value | rxe_value,
        0x301,
        "enable_uart: combined value must equal materialized literal 0x301"
    );
}

#[test]
fn check_group_2_enable_rx_interrupts_body() {
    let result = compile_and_extract(SPECIMEN, "cg2_enable_rx");
    let func = find_fn(&result.bodies, "enable_rx_interrupts");

    // One modify call on imsc.
    let calls = collect_calls(&func.body);
    let modify_calls: Vec<_> = calls.iter().filter(|(p, _)| p.contains("modify")).collect();
    assert_eq!(
        modify_calls.len(),
        1,
        "enable_rx_interrupts: exactly one modify call"
    );

    // RX_INTERRUPT_MASK appears as unevaluated constant (both mask and value args).
    let rx_mask_count = count_unevaluated_const(&func.body, "RX_INTERRUPT_MASK");
    assert!(
        rx_mask_count >= 2,
        "enable_rx_interrupts: RX_INTERRUPT_MASK must appear at least twice (mask + value), got {rx_mask_count}"
    );

    // Register target is imsc (ImscReg).
    assert!(
        has_register_field_access(&func.body, "ImscReg"),
        "enable_rx_interrupts: must access ImscReg register"
    );
}

#[test]
fn check_group_2_enable_tx_interrupts_body() {
    let result = compile_and_extract(SPECIMEN, "cg2_enable_tx");
    let func = find_fn(&result.bodies, "enable_tx_interrupts");

    // One modify call on imsc.
    let calls = collect_calls(&func.body);
    let modify_calls: Vec<_> = calls.iter().filter(|(p, _)| p.contains("modify")).collect();
    assert_eq!(
        modify_calls.len(),
        1,
        "enable_tx_interrupts: exactly one modify call"
    );

    // TX_INTERRUPT_MASK appears as unevaluated constant (both mask and value args).
    let tx_mask_count = count_unevaluated_const(&func.body, "TX_INTERRUPT_MASK");
    assert!(
        tx_mask_count >= 2,
        "enable_tx_interrupts: TX_INTERRUPT_MASK must appear at least twice (mask + value), got {tx_mask_count}"
    );

    // Register target is imsc (ImscReg).
    assert!(
        has_register_field_access(&func.body, "ImscReg"),
        "enable_tx_interrupts: must access ImscReg register"
    );
}

#[test]
fn check_group_2_disable_tx_interrupts_body() {
    let result = compile_and_extract(SPECIMEN, "cg2_disable_tx");
    let func = find_fn(&result.bodies, "disable_tx_interrupts");

    // One modify call on imsc.
    let calls = collect_calls(&func.body);
    let modify_calls: Vec<_> = calls.iter().filter(|(p, _)| p.contains("modify")).collect();
    assert_eq!(
        modify_calls.len(),
        1,
        "disable_tx_interrupts: exactly one modify call"
    );

    // TX_INTERRUPT_MASK appears as unevaluated constant (mask arg).
    assert!(
        has_unevaluated_const(&func.body, "TX_INTERRUPT_MASK"),
        "disable_tx_interrupts: TX_INTERRUPT_MASK must appear as unevaluated constant"
    );

    // Value 0 appears as scalar (bits disabled to 0).
    assert!(
        has_scalar_const(&func.body, 0),
        "disable_tx_interrupts: zero value must appear as scalar constant (bits disabled to 0)"
    );

    // Register target is imsc (ImscReg).
    assert!(
        has_register_field_access(&func.body, "ImscReg"),
        "disable_tx_interrupts: must access ImscReg register"
    );
}

#[test]
fn check_group_2_program_polling_port_body() {
    let result = compile_and_extract(SPECIMEN, "cg2_prog_poll");
    let func = find_fn(&result.bodies, "program_polling_port");

    // Parameters.
    assert_eq!(func.params.len(), 2);
    assert_eq!(func.params[0].name, "uart_clk_hz");
    assert_eq!(func.params[1].name, "baud_rate");

    // Helper calls present.
    let call_paths = ordered_call_paths(&func.body);

    // disable_uart and disable_fifo must be called (pre-ordered).
    assert!(
        call_paths.iter().any(|p| p.contains("disable_uart")),
        "program_polling_port must call disable_uart"
    );
    assert!(
        call_paths.iter().any(|p| p.contains("disable_fifo")),
        "program_polling_port must call disable_fifo"
    );

    // Ordered region helpers must be called.
    let ordered_helpers = [
        "set_baud_rate",
        "configure_line_control",
        "configure_interrupt_levels",
        "mask_all_interrupts",
        "clear_all_interrupts",
        "enable_uart",
        "enable_rx_interrupts",
    ];

    for helper in &ordered_helpers {
        assert!(
            call_paths.iter().any(|p| p.contains(helper)),
            "program_polling_port must call {helper}"
        );
    }

    // compiler_fence calls must be present (ordered! barriers).
    assert!(
        has_compiler_fence(&func.body),
        "program_polling_port must have compiler_fence calls (ordered! barriers)"
    );

    // Exactly 2 compiler_fence calls (leading + trailing).
    let fence_count = count_calls_containing(&func.body, "compiler_fence");
    assert!(
        fence_count >= 2,
        "program_polling_port: at least 2 compiler_fence calls expected (got {fence_count})"
    );

    // Verify ordering: disable_uart and disable_fifo appear before the
    // first compiler_fence, and the ordered helpers appear between fences.
    // In MIR basic block order, the call sequence should be:
    //   disable_uart -> disable_fifo -> fence -> ordered_helpers -> fence
    let mut disable_uart_idx = None;
    let mut disable_fifo_idx = None;
    let mut first_fence_idx = None;
    let mut last_fence_idx = None;
    let mut ordered_helper_indices: Vec<(usize, &str)> = Vec::new();

    for (i, path) in call_paths.iter().enumerate() {
        if path.contains("disable_uart") && disable_uart_idx.is_none() {
            disable_uart_idx = Some(i);
        }
        if path.contains("disable_fifo") && disable_fifo_idx.is_none() {
            disable_fifo_idx = Some(i);
        }
        if path.contains("compiler_fence") {
            if first_fence_idx.is_none() {
                first_fence_idx = Some(i);
            }
            last_fence_idx = Some(i);
        }
        for helper in &ordered_helpers {
            if path.contains(helper) {
                ordered_helper_indices.push((i, helper));
            }
        }
    }

    let du = disable_uart_idx.expect("disable_uart call index");
    let df = disable_fifo_idx.expect("disable_fifo call index");
    let ff = first_fence_idx.expect("first fence index");
    let lf = last_fence_idx.expect("last fence index");

    assert!(
        du < ff,
        "disable_uart (idx {du}) must come before first fence (idx {ff})"
    );
    assert!(
        df < ff,
        "disable_fifo (idx {df}) must come before first fence (idx {ff})"
    );

    // Ordered helpers must appear between the fences.
    for (idx, name) in &ordered_helper_indices {
        assert!(
            *idx > ff && *idx < lf,
            "ordered helper `{name}` (idx {idx}) must be between fences ({ff}..{lf})"
        );
    }

    // Verify the relative ordering of helpers inside the ordered region.
    let helper_positions: Vec<usize> = ordered_helpers
        .iter()
        .map(|h| {
            ordered_helper_indices
                .iter()
                .find(|(_, name)| *name == *h)
                .unwrap_or_else(|| panic!("ordered helper `{h}` not found"))
                .0
        })
        .collect();

    for i in 1..helper_positions.len() {
        assert!(
            helper_positions[i] > helper_positions[i - 1],
            "ordered helper sequence violation: {} (idx {}) should come after {} (idx {})",
            ordered_helpers[i],
            helper_positions[i],
            ordered_helpers[i - 1],
            helper_positions[i - 1]
        );
    }
}

// ============================================================================
// Check Group 2 supplementary: helper_calls field validation
// ============================================================================

#[test]
fn check_group_2_program_polling_port_helper_calls_field() {
    let result = compile_and_extract(SPECIMEN, "cg2_prog_poll_hc");
    let func = find_fn(&result.bodies, "program_polling_port");

    // The helper_calls field should contain all directly called helpers.
    let expected_helpers = [
        "disable_uart",
        "disable_fifo",
        "set_baud_rate",
        "configure_line_control",
        "configure_interrupt_levels",
        "mask_all_interrupts",
        "clear_all_interrupts",
        "enable_uart",
        "enable_rx_interrupts",
    ];

    for helper in &expected_helpers {
        assert!(
            func.helper_calls.contains(&helper.to_string()),
            "program_polling_port helper_calls should contain `{helper}`"
        );
    }
}
