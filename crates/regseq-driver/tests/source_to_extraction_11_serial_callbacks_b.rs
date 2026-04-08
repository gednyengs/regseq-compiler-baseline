//! source-to-extraction-11: PL011 serial callbacks B authored->extraction
//! boundary validation.
//!
//! Validates that the second half of the authored serial-callback facts for
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
// Inline specimen: reduced PL011 serial callbacks B surface
// ---------------------------------------------------------------------------

/// Reduced specimen covering the serial callbacks in check group B:
/// tx_ready, tx_empty, flush_rx_fifo, flush_tx_fifo, send_break.
///
/// This specimen reproduces the authored facts from the fixture source
/// with only the types and trait definitions needed to compile and extract
/// the five callbacks and their register interactions.
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
            pub lcr_h: Register<u32>,
        }

        pub mod DR_REG {
            use super::super::*;
            pub struct Register;
            pub mod DATA {
                use super::super::super::*;
                pub const FIELD: Field<u32, super::Register> = Field {
                    mask: 0xFF, shift: 0, _r: std::marker::PhantomData
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
            pub mod BUSY {
                use super::super::super::*;
                pub const FIELD: Field<u32, super::Register> = Field {
                    mask: 1 << 3, shift: 3, _r: std::marker::PhantomData
                };
            }
        }
        pub mod LCRH_REG {
            use super::super::*;
            pub struct Register;
            pub mod FEN { use super::super::super::*;
                pub const ENABLED: FieldValue<u32, super::Register> = FieldValue::new(1 << 4, 1 << 4);
                pub const DISABLED: FieldValue<u32, super::Register> = FieldValue::new(1 << 4, 0);
            }
            pub mod BRK { use super::super::super::*;
                pub const ENABLED: FieldValue<u32, super::Register> = FieldValue::new(1, 1);
                pub const DISABLED: FieldValue<u32, super::Register> = FieldValue::new(1, 0);
            }
        }
    }

    fn field_val<R>(field: Field<u32, R>, raw: u32) -> FieldValue<u32, R> {
        FieldValue::new(field.mask, raw << field.shift)
    }

    use regs::*;

    #[rseq::driver(name = "pl011-full-spec", compatible = ["arm,pl011"])]
    pub struct Pl011FullSpec {
        pub regs: Pl011Registers,
    }

    impl regseq::Serial for Pl011FullSpec {
        fn port_open(&mut self) -> Result<(), RegseqError> { Ok(()) }
        fn port_close(&mut self) { }
        fn configure(&mut self, _config: &SerialConfig) -> Result<(), RegseqError> { Ok(()) }
        fn tx_byte(&mut self, _byte: u8) { }
        fn rx_char(&mut self) -> Option<RxChar> { None }

        fn tx_ready(&mut self) -> bool {
            self.regs.fr.read(FR_REG::TXFF::FIELD) == 0
        }

        fn tx_empty(&mut self) -> bool {
            self.regs.fr.read(FR_REG::BUSY::FIELD) == 0
        }

        fn flush_rx_fifo(&mut self) {
            while self.regs.fr.read(FR_REG::RXFE::FIELD) == 0 {
                let _ = self.regs.dr.read_raw();
            }
        }

        fn flush_tx_fifo(&mut self) {
            self.regs.lcr_h.modify(LCRH_REG::FEN::DISABLED);
            self.regs.lcr_h.modify(LCRH_REG::FEN::ENABLED);
        }

        fn send_break(&mut self, on: bool) {
            if on {
                self.regs.lcr_h.modify(LCRH_REG::BRK::ENABLED);
            } else {
                self.regs.lcr_h.modify(LCRH_REG::BRK::DISABLED);
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
            MirTy::Fn { path, .. } => path.contains(substr),
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
                    if let MirTy::Fn { path, .. } = &constant.ty {
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

/// Check if a MIR body has a Goto terminator (used for loop back-edges).
fn has_goto(body: &MirBody) -> bool {
    body.basic_blocks.iter().any(|bb| {
        matches!(
            bb.terminator.as_ref().map(|t| &t.kind),
            Some(MirTerminatorKind::Goto { .. })
        )
    })
}

/// Recursively check if a MirTy mentions a given substring in any path.
fn mir_ty_mentions(ty: &MirTy, substr: &str) -> bool {
    match ty {
        MirTy::Struct { path, fields } => {
            path.contains(substr) || fields.iter().any(|f| mir_ty_mentions(&f.ty, substr))
        }
        MirTy::Enum { path, variants } => {
            path.contains(substr)
                || variants
                    .iter()
                    .any(|v| v.fields.iter().any(|f| mir_ty_mentions(&f.ty, substr)))
        }
        MirTy::Fn { path, params, ret } => {
            path.contains(substr)
                || params.iter().any(|p| mir_ty_mentions(p, substr))
                || mir_ty_mentions(ret, substr)
        }
        MirTy::Ref { ty: inner, .. } | MirTy::RawPtr { ty: inner, .. } => {
            mir_ty_mentions(inner, substr)
        }
        MirTy::Tuple { elements } => elements.iter().any(|e| mir_ty_mentions(e, substr)),
        MirTy::Array { element, .. } | MirTy::Slice { element } => mir_ty_mentions(element, substr),
        MirTy::NominalRef { path } => path.contains(substr),
        MirTy::ZeroSizedLike { description } => description.contains(substr),
        MirTy::Unsupported { description, .. } => description.contains(substr),
        _ => false,
    }
}

/// Check if a Call terminator for `callee_substr` has a Fn type argument
/// whose path contains `type_arg_substr`. This proves which register module
/// the generic `R` parameter is instantiated with.
fn call_has_type_arg(body: &MirBody, callee_substr: &str, type_arg_substr: &str) -> bool {
    body.basic_blocks.iter().any(|bb| {
        bb.terminator.as_ref().is_some_and(|t| match &t.kind {
            MirTerminatorKind::Call { func, .. } => match func {
                MirOperand::Constant { constant } => match &constant.ty {
                    MirTy::Fn { path, params, ret } => {
                        path.contains(callee_substr)
                            && (params.iter().any(|p| mir_ty_mentions(p, type_arg_substr))
                                || mir_ty_mentions(ret, type_arg_substr))
                    }
                    _ => false,
                },
                _ => false,
            },
            _ => false,
        })
    })
}

/// Collect (callee_path, type_args_from_path) pairs from Call terminators.
///
/// The type arguments are extracted from the Fn path's embedded generic
/// annotations (e.g. `modify::<u32, regs::LCRH_REG::Register>`), not from
/// recursive field scanning. This gives the top-level generic instantiation
/// without noise from deeply nested field types.
fn collect_calls_with_type_args(body: &MirBody, callee_substr: &str) -> Vec<(String, Vec<String>)> {
    let mut result = Vec::new();
    for bb in &body.basic_blocks {
        if let Some(t) = &bb.terminator {
            if let MirTerminatorKind::Call { func, .. } = &t.kind {
                if let MirOperand::Constant { constant } = func {
                    if let MirTy::Fn { path, .. } = &constant.ty {
                        if path.contains(callee_substr) {
                            // Extract type args from the path's `<...>` annotations.
                            let type_args = extract_generic_args_from_path(path);
                            result.push((path.clone(), type_args));
                        }
                    }
                }
            }
        }
    }
    result
}

/// Extract generic type argument strings from a Fn path.
///
/// For `Register::modify::<u32, regs::LCRH_REG::Register>`, returns
/// `["u32", "regs::LCRH_REG::Register"]`.
fn extract_generic_args_from_path(path: &str) -> Vec<String> {
    let mut args = Vec::new();
    // Find the outermost `<...>` at the end of the path.
    if let Some(start) = path.rfind("::<") {
        let inner = &path[start + 3..path.len().saturating_sub(1)];
        // Split on commas, respecting nested `<>`.
        let mut depth = 0u32;
        let mut current = String::new();
        for ch in inner.chars() {
            match ch {
                '<' => {
                    depth += 1;
                    current.push(ch);
                }
                '>' => {
                    if depth > 0 {
                        depth -= 1;
                    }
                    current.push(ch);
                }
                ',' if depth == 0 => {
                    let trimmed = current.trim().to_string();
                    if !trimmed.is_empty() {
                        args.push(trimmed);
                    }
                    current.clear();
                }
                _ => current.push(ch),
            }
        }
        let trimmed = current.trim().to_string();
        if !trimmed.is_empty() {
            args.push(trimmed);
        }
    }
    args
}

/// Collect the Unevaluated constant paths from Call terminator arguments for
/// calls matching `callee_substr`, in block order. Each entry is a Vec of
/// constant paths (one per arg that is a Constant with Unevaluated value).
fn collect_call_arg_const_paths(body: &MirBody, callee_substr: &str) -> Vec<Vec<String>> {
    let mut result = Vec::new();
    for bb in &body.basic_blocks {
        if let Some(t) = &bb.terminator {
            if let MirTerminatorKind::Call { func, args, .. } = &t.kind {
                if operand_mentions(func, callee_substr) {
                    let paths: Vec<String> = args
                        .iter()
                        .filter_map(|arg| match arg {
                            MirOperand::Constant { constant } => match &constant.value {
                                MirConstValue::Unevaluated { path } => Some(path.clone()),
                                _ => None,
                            },
                            _ => None,
                        })
                        .collect();
                    result.push(paths);
                }
            }
        }
    }
    result
}

/// For a SwitchInt on a bool discriminant, find the constant arg paths of
/// calls matching `callee_substr` in each branch.
///
/// Returns `(true_branch_paths, false_branch_paths)` where each entry is
/// the set of Unevaluated constant paths found in Call args reachable from
/// the respective branch target before converging.
///
/// SwitchInt encodes bool as: value 0 -> first target (false), otherwise -> second target (true).
fn collect_branch_call_arg_paths(
    body: &MirBody,
    callee_substr: &str,
) -> (Vec<String>, Vec<String>) {
    // Find the SwitchInt terminator
    let switch_bb = body.basic_blocks.iter().find_map(|bb| {
        if let Some(t) = &bb.terminator {
            if let MirTerminatorKind::SwitchInt { targets, .. } = &t.kind {
                // Bool SwitchInt: values=[0], targets=[false_bb, true_bb]
                if targets.values == [0] && targets.targets.len() == 2 {
                    return Some((targets.targets[0], targets.targets[1]));
                }
            }
        }
        None
    });

    let Some((false_bb, true_bb)) = switch_bb else {
        return (vec![], vec![]);
    };

    let extract_paths = |start_bb: u32| -> Vec<String> {
        let mut paths = Vec::new();
        let mut bb_idx = start_bb as usize;
        // Walk blocks from the branch entry until we hit a Goto (convergence)
        // or Return, collecting call arg const paths along the way.
        loop {
            if bb_idx >= body.basic_blocks.len() {
                break;
            }
            let bb = &body.basic_blocks[bb_idx];
            if let Some(t) = &bb.terminator {
                if let MirTerminatorKind::Call {
                    func, args, target, ..
                } = &t.kind
                {
                    if operand_mentions(func, callee_substr) {
                        for arg in args {
                            if let MirOperand::Constant { constant } = arg {
                                if let MirConstValue::Unevaluated { path } = &constant.value {
                                    paths.push(path.clone());
                                }
                            }
                        }
                    }
                    // Follow the call's continuation target
                    if let Some(next) = target {
                        bb_idx = *next as usize;
                        continue;
                    }
                }
                // Stop at Goto (convergence point) or Return
                break;
            }
            break;
        }
        paths
    };

    (extract_paths(true_bb), extract_paths(false_bb))
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

    // -- tx_ready authored facts --
    assert!(
        fixture.contains("fn tx_ready(&mut self) -> bool"),
        "fixture must contain tx_ready signature"
    );
    assert!(
        fixture.contains("self.regs.fr.read(FR_REG::TXFF::FIELD) == 0"),
        "fixture tx_ready must test FR.TXFF == 0"
    );

    // -- tx_empty authored facts --
    assert!(
        fixture.contains("fn tx_empty(&mut self) -> bool"),
        "fixture must contain tx_empty signature"
    );
    assert!(
        fixture.contains("self.regs.fr.read(FR_REG::BUSY::FIELD) == 0"),
        "fixture tx_empty must test FR.BUSY == 0"
    );

    // -- flush_rx_fifo authored facts --
    assert!(
        fixture.contains("fn flush_rx_fifo(&mut self)"),
        "fixture must contain flush_rx_fifo signature"
    );
    assert!(
        fixture.contains("while self.regs.fr.read(FR_REG::RXFE::FIELD) == 0"),
        "fixture flush_rx_fifo must loop while FR.RXFE == 0"
    );
    assert!(
        fixture.contains("let _ = self.regs.dr.read_raw()"),
        "fixture flush_rx_fifo must drain DR via read_raw()"
    );

    // -- flush_tx_fifo authored facts --
    assert!(
        fixture.contains("fn flush_tx_fifo(&mut self)"),
        "fixture must contain flush_tx_fifo signature"
    );
    assert!(
        fixture.contains("self.regs.lcr_h.modify(LCRH_REG::FEN::DISABLED)"),
        "fixture flush_tx_fifo must apply FEN::DISABLED"
    );
    assert!(
        fixture.contains("self.regs.lcr_h.modify(LCRH_REG::FEN::ENABLED)"),
        "fixture flush_tx_fifo must apply FEN::ENABLED"
    );

    // -- send_break authored facts --
    assert!(
        fixture.contains("fn send_break(&mut self, on: bool)"),
        "fixture must contain send_break signature"
    );
    assert!(
        fixture.contains("self.regs.lcr_h.modify(LCRH_REG::BRK::ENABLED)"),
        "fixture send_break must apply BRK::ENABLED"
    );
    assert!(
        fixture.contains("self.regs.lcr_h.modify(LCRH_REG::BRK::DISABLED)"),
        "fixture send_break must apply BRK::DISABLED"
    );

    // -- Serial impl presence --
    assert!(
        fixture.contains("impl Serial for Pl011FullSpec"),
        "fixture must have Serial impl for Pl011FullSpec"
    );

    // -- Specimen reproduces all relevant facts --
    assert!(
        SPECIMEN.contains("fn tx_ready(&mut self) -> bool"),
        "specimen must reproduce tx_ready signature"
    );
    assert!(
        SPECIMEN.contains("self.regs.fr.read(FR_REG::TXFF::FIELD) == 0"),
        "specimen must reproduce tx_ready body"
    );
    assert!(
        SPECIMEN.contains("fn tx_empty(&mut self) -> bool"),
        "specimen must reproduce tx_empty signature"
    );
    assert!(
        SPECIMEN.contains("self.regs.fr.read(FR_REG::BUSY::FIELD) == 0"),
        "specimen must reproduce tx_empty body"
    );
    assert!(
        SPECIMEN.contains("fn flush_rx_fifo(&mut self)"),
        "specimen must reproduce flush_rx_fifo signature"
    );
    assert!(
        SPECIMEN.contains("while self.regs.fr.read(FR_REG::RXFE::FIELD) == 0"),
        "specimen must reproduce flush_rx_fifo loop"
    );
    assert!(
        SPECIMEN.contains("let _ = self.regs.dr.read_raw()"),
        "specimen must reproduce flush_rx_fifo drain"
    );
    assert!(
        SPECIMEN.contains("fn flush_tx_fifo(&mut self)"),
        "specimen must reproduce flush_tx_fifo signature"
    );
    assert!(
        SPECIMEN.contains("self.regs.lcr_h.modify(LCRH_REG::FEN::DISABLED)"),
        "specimen must reproduce FEN::DISABLED"
    );
    assert!(
        SPECIMEN.contains("self.regs.lcr_h.modify(LCRH_REG::FEN::ENABLED)"),
        "specimen must reproduce FEN::ENABLED"
    );
    assert!(
        SPECIMEN.contains("fn send_break(&mut self, on: bool)"),
        "specimen must reproduce send_break signature"
    );
    assert!(
        SPECIMEN.contains("self.regs.lcr_h.modify(LCRH_REG::BRK::ENABLED)"),
        "specimen must reproduce BRK::ENABLED"
    );
    assert!(
        SPECIMEN.contains("self.regs.lcr_h.modify(LCRH_REG::BRK::DISABLED)"),
        "specimen must reproduce BRK::DISABLED"
    );
}

// ---------------------------------------------------------------------------
// Check Group 1: callback identity, trait role, signature, return type
// ---------------------------------------------------------------------------

#[test]
fn check_group_1_serial_trait_role_recognized() {
    let ext = compile_and_extract_all(SPECIMEN, "serial_cb_b_g1_trait");

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
    let ext = compile_and_extract_all(SPECIMEN, "serial_cb_b_g1_identity");

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
    for cb in &[
        "tx_ready",
        "tx_empty",
        "flush_rx_fifo",
        "flush_tx_fifo",
        "send_break",
    ] {
        assert!(
            method_names.contains(cb),
            "Serial impl must contain method '{}'",
            cb
        );
    }
}

#[test]
fn check_group_1_callback_signatures_and_return_types() {
    let ext = compile_and_extract_all(SPECIMEN, "serial_cb_b_g1_sig");

    // tx_ready: receiver=ref_mut, params=[], return=bool
    let tx_ready = find_fn(&ext.body, "tx_ready");
    assert_eq!(
        tx_ready.receiver, "ref_mut",
        "tx_ready receiver must be &mut self"
    );
    assert!(
        tx_ready.params.is_empty(),
        "tx_ready must have no extra params"
    );
    assert!(
        tx_ready
            .return_type
            .as_ref()
            .is_some_and(|r| matches!(r, MirTy::Bool)),
        "tx_ready must return bool, got: {:?}",
        tx_ready.return_type
    );
    assert!(
        matches!(
            &tx_ready.kind,
            MirFnKind::Callback { trait_name, method_name }
            if trait_name == "Serial" && method_name == "tx_ready"
        ),
        "tx_ready kind must be Callback {{ trait_name: Serial, method_name: tx_ready }}"
    );

    // tx_empty: receiver=ref_mut, params=[], return=bool
    let tx_empty = find_fn(&ext.body, "tx_empty");
    assert_eq!(
        tx_empty.receiver, "ref_mut",
        "tx_empty receiver must be &mut self"
    );
    assert!(
        tx_empty.params.is_empty(),
        "tx_empty must have no extra params"
    );
    assert!(
        tx_empty
            .return_type
            .as_ref()
            .is_some_and(|r| matches!(r, MirTy::Bool)),
        "tx_empty must return bool, got: {:?}",
        tx_empty.return_type
    );
    assert!(
        matches!(
            &tx_empty.kind,
            MirFnKind::Callback { trait_name, method_name }
            if trait_name == "Serial" && method_name == "tx_empty"
        ),
        "tx_empty kind must be Callback {{ trait_name: Serial, method_name: tx_empty }}"
    );

    // flush_rx_fifo: receiver=ref_mut, params=[], return=unit
    let flush_rx = find_fn(&ext.body, "flush_rx_fifo");
    assert_eq!(
        flush_rx.receiver, "ref_mut",
        "flush_rx_fifo receiver must be &mut self"
    );
    assert!(
        flush_rx.params.is_empty(),
        "flush_rx_fifo must have no extra params"
    );
    assert!(
        flush_rx.return_type.is_none(),
        "flush_rx_fifo must return unit, got: {:?}",
        flush_rx.return_type
    );
    assert!(
        matches!(
            &flush_rx.kind,
            MirFnKind::Callback { trait_name, method_name }
            if trait_name == "Serial" && method_name == "flush_rx_fifo"
        ),
        "flush_rx_fifo kind must be Callback {{ trait_name: Serial, method_name: flush_rx_fifo }}"
    );

    // flush_tx_fifo: receiver=ref_mut, params=[], return=unit
    let flush_tx = find_fn(&ext.body, "flush_tx_fifo");
    assert_eq!(
        flush_tx.receiver, "ref_mut",
        "flush_tx_fifo receiver must be &mut self"
    );
    assert!(
        flush_tx.params.is_empty(),
        "flush_tx_fifo must have no extra params"
    );
    assert!(
        flush_tx.return_type.is_none(),
        "flush_tx_fifo must return unit, got: {:?}",
        flush_tx.return_type
    );
    assert!(
        matches!(
            &flush_tx.kind,
            MirFnKind::Callback { trait_name, method_name }
            if trait_name == "Serial" && method_name == "flush_tx_fifo"
        ),
        "flush_tx_fifo kind must be Callback {{ trait_name: Serial, method_name: flush_tx_fifo }}"
    );

    // send_break: receiver=ref_mut, params=[on: bool], return=unit
    let send_break = find_fn(&ext.body, "send_break");
    assert_eq!(
        send_break.receiver, "ref_mut",
        "send_break receiver must be &mut self"
    );
    assert_eq!(
        send_break.params.len(),
        1,
        "send_break must have exactly 1 extra param"
    );
    assert_eq!(
        send_break.params[0].name, "on",
        "send_break param must be named 'on'"
    );
    assert!(
        matches!(&send_break.params[0].ty, MirTy::Bool),
        "send_break param type must be bool, got: {:?}",
        send_break.params[0].ty
    );
    assert!(
        send_break.return_type.is_none(),
        "send_break must return unit, got: {:?}",
        send_break.return_type
    );
    assert!(
        matches!(
            &send_break.kind,
            MirFnKind::Callback { trait_name, method_name }
            if trait_name == "Serial" && method_name == "send_break"
        ),
        "send_break kind must be Callback {{ trait_name: Serial, method_name: send_break }}"
    );
}

// ---------------------------------------------------------------------------
// Check Group 2: callback bodies preserve downstream checks
// ---------------------------------------------------------------------------

#[test]
fn check_group_2_tx_ready_body() {
    let ext = compile_and_extract_all(SPECIMEN, "serial_cb_b_g2_tx_ready");
    let f = find_fn(&ext.body, "tx_ready");

    // tx_ready returns exactly whether TX FIFO is not full using FR.TXFF
    // MIR body must call read (for FR.TXFF check)
    assert!(
        has_call_to(&f.body, "read"),
        "tx_ready MIR body must call read (for FR.TXFF)"
    );

    // Strengthen: verify the read call is instantiated with FR_REG::Register,
    // proving that the read targets the FR register module (not some other reg).
    assert!(
        call_has_type_arg(&f.body, "read", "FR_REG"),
        "tx_ready read call must be instantiated with FR_REG::Register type arg"
    );

    // Must have a Return terminator
    assert!(
        has_return(&f.body),
        "tx_ready MIR body must have a Return terminator"
    );

    // Returns bool (predicate callback)
    assert!(
        f.return_type
            .as_ref()
            .is_some_and(|r| matches!(r, MirTy::Bool)),
        "tx_ready must return bool"
    );

    // Exact field identity: the read call argument must be FR_REG::TXFF::FIELD
    let read_arg_paths = collect_call_arg_const_paths(&f.body, "read");
    assert!(
        !read_arg_paths.is_empty(),
        "tx_ready must have at least one read call with constant args"
    );
    assert!(
        read_arg_paths
            .iter()
            .any(|paths| { paths.iter().any(|p| p.contains("FR_REG::TXFF::FIELD")) }),
        "tx_ready read call must pass FR_REG::TXFF::FIELD as argument, \
         got paths: {:?}",
        read_arg_paths
    );
}

#[test]
fn check_group_2_tx_empty_body() {
    let ext = compile_and_extract_all(SPECIMEN, "serial_cb_b_g2_tx_empty");
    let f = find_fn(&ext.body, "tx_empty");

    // tx_empty returns exactly whether the transmitter is not busy using FR.BUSY
    // MIR body must call read (for FR.BUSY check)
    assert!(
        has_call_to(&f.body, "read"),
        "tx_empty MIR body must call read (for FR.BUSY)"
    );

    // Strengthen: verify the read call is instantiated with FR_REG::Register.
    assert!(
        call_has_type_arg(&f.body, "read", "FR_REG"),
        "tx_empty read call must be instantiated with FR_REG::Register type arg"
    );

    // Must have a Return terminator
    assert!(
        has_return(&f.body),
        "tx_empty MIR body must have a Return terminator"
    );

    // Returns bool
    assert!(
        f.return_type
            .as_ref()
            .is_some_and(|r| matches!(r, MirTy::Bool)),
        "tx_empty must return bool"
    );

    // Exact field identity: the read call argument must be FR_REG::BUSY::FIELD
    let read_arg_paths = collect_call_arg_const_paths(&f.body, "read");
    assert!(
        !read_arg_paths.is_empty(),
        "tx_empty must have at least one read call with constant args"
    );
    assert!(
        read_arg_paths
            .iter()
            .any(|paths| { paths.iter().any(|p| p.contains("FR_REG::BUSY::FIELD")) }),
        "tx_empty read call must pass FR_REG::BUSY::FIELD as argument, \
         got paths: {:?}",
        read_arg_paths
    );
}

#[test]
fn check_group_2_flush_rx_fifo_body() {
    let ext = compile_and_extract_all(SPECIMEN, "serial_cb_b_g2_flush_rx");
    let f = find_fn(&ext.body, "flush_rx_fifo");

    // flush_rx_fifo loops while FR.RXFE == 0 and drains DR via read_raw()

    // Must call read (for FR.RXFE check in loop condition)
    assert!(
        has_call_to(&f.body, "read"),
        "flush_rx_fifo MIR body must call read (for FR.RXFE loop condition)"
    );

    // Strengthen: verify the read call is instantiated with FR_REG::Register.
    assert!(
        call_has_type_arg(&f.body, "read", "FR_REG"),
        "flush_rx_fifo read call must be instantiated with FR_REG::Register type arg"
    );

    // Must call read_raw (for DR drain on each iteration)
    assert!(
        has_call_to(&f.body, "read_raw"),
        "flush_rx_fifo MIR body must call read_raw (for DR drain)"
    );

    // Must have a SwitchInt (for the while loop condition check)
    assert!(
        has_switch_int(&f.body),
        "flush_rx_fifo MIR body must have SwitchInt for loop condition"
    );

    // Must have a Goto (for the loop back-edge)
    assert!(
        has_goto(&f.body),
        "flush_rx_fifo MIR body must have Goto for loop back-edge"
    );

    // Must have a Return terminator (loop exits and returns)
    assert!(
        has_return(&f.body),
        "flush_rx_fifo MIR body must have a Return terminator"
    );

    // Return type is unit
    assert!(f.return_type.is_none(), "flush_rx_fifo must return unit");
}

#[test]
fn check_group_2_flush_tx_fifo_body() {
    let ext = compile_and_extract_all(SPECIMEN, "serial_cb_b_g2_flush_tx");
    let f = find_fn(&ext.body, "flush_tx_fifo");

    // flush_tx_fifo performs exactly two sequential lcr_h.modify(...) calls:
    // first LCRH_REG::FEN::DISABLED, then LCRH_REG::FEN::ENABLED

    // Must call modify exactly 2 times
    let modify_count = count_calls_to(&f.body, "modify");
    assert_eq!(
        modify_count, 2,
        "flush_tx_fifo must call modify exactly 2 times, found: {}",
        modify_count
    );

    // Strengthen: verify both modify calls target LCRH_REG::Register.
    let modify_calls = collect_calls_with_type_args(&f.body, "modify");
    assert_eq!(
        modify_calls.len(),
        2,
        "flush_tx_fifo must have exactly 2 modify calls with type info"
    );
    for (i, (path, type_args)) in modify_calls.iter().enumerate() {
        assert!(path.contains("modify"), "call {} must be a modify call", i);
        assert!(
            type_args.iter().any(|a| a.contains("LCRH_REG")),
            "flush_tx_fifo modify call {} must be instantiated with LCRH_REG::Register, got type_args: {:?}",
            i, type_args
        );
    }

    // Verify ordering: the two modify calls appear sequentially
    let calls = collect_call_names(&f.body);
    let modify_indices: Vec<usize> = calls
        .iter()
        .enumerate()
        .filter(|(_, c)| c.contains("modify"))
        .map(|(i, _)| i)
        .collect();
    assert_eq!(
        modify_indices.len(),
        2,
        "flush_tx_fifo must have exactly 2 modify calls in call order"
    );
    assert!(
        modify_indices[0] < modify_indices[1],
        "first modify must come before second modify"
    );

    // Exact value ordering: first modify must pass FEN::DISABLED, second FEN::ENABLED
    let modify_arg_paths = collect_call_arg_const_paths(&f.body, "modify");
    assert_eq!(
        modify_arg_paths.len(),
        2,
        "flush_tx_fifo must have exactly 2 modify calls with arg paths"
    );
    assert!(
        modify_arg_paths[0]
            .iter()
            .any(|p| p.contains("FEN::DISABLED")),
        "flush_tx_fifo first modify must pass FEN::DISABLED, got: {:?}",
        modify_arg_paths[0]
    );
    assert!(
        modify_arg_paths[1]
            .iter()
            .any(|p| p.contains("FEN::ENABLED")),
        "flush_tx_fifo second modify must pass FEN::ENABLED, got: {:?}",
        modify_arg_paths[1]
    );

    // Must have a Return terminator
    assert!(
        has_return(&f.body),
        "flush_tx_fifo MIR body must have a Return terminator"
    );

    // Return type is unit
    assert!(f.return_type.is_none(), "flush_tx_fifo must return unit");
}

#[test]
fn check_group_2_send_break_body() {
    let ext = compile_and_extract_all(SPECIMEN, "serial_cb_b_g2_send_break");
    let f = find_fn(&ext.body, "send_break");

    // send_break branches on `on: bool` and modifies LCR_H with
    // BRK::ENABLED (when on==true) or BRK::DISABLED (when on==false)

    // Must have SwitchInt (for the `if on` branch)
    assert!(
        has_switch_int(&f.body),
        "send_break MIR body must have SwitchInt for `if on` branch"
    );

    // Must call modify (for lcr_h.modify(...))
    assert!(
        has_call_to(&f.body, "modify"),
        "send_break MIR body must call modify (for LCR_H)"
    );

    // Must call modify exactly 2 times (one per branch: BRK::ENABLED and
    // BRK::DISABLED)
    let modify_count = count_calls_to(&f.body, "modify");
    assert_eq!(
        modify_count, 2,
        "send_break must call modify exactly 2 times (one per branch), found: {}",
        modify_count
    );

    // Strengthen: verify both modify calls target LCRH_REG::Register.
    let modify_calls = collect_calls_with_type_args(&f.body, "modify");
    assert_eq!(
        modify_calls.len(),
        2,
        "send_break must have exactly 2 modify calls with type info"
    );
    for (i, (_path, type_args)) in modify_calls.iter().enumerate() {
        assert!(
            type_args.iter().any(|a| a.contains("LCRH_REG")),
            "send_break modify call {} must be instantiated with LCRH_REG::Register, got type_args: {:?}",
            i, type_args
        );
    }

    // Exact branch-to-value mapping: on==true -> BRK::ENABLED, on==false -> BRK::DISABLED
    let (true_paths, false_paths) = collect_branch_call_arg_paths(&f.body, "modify");
    assert!(
        true_paths.iter().any(|p| p.contains("BRK::ENABLED")),
        "send_break true branch must pass BRK::ENABLED to modify, got: {:?}",
        true_paths
    );
    assert!(
        false_paths.iter().any(|p| p.contains("BRK::DISABLED")),
        "send_break false branch must pass BRK::DISABLED to modify, got: {:?}",
        false_paths
    );

    // Must have a Return terminator
    assert!(
        has_return(&f.body),
        "send_break MIR body must have a Return terminator"
    );

    // Return type is unit
    assert!(f.return_type.is_none(), "send_break must return unit");

    // Parameter must be named "on" with type bool
    assert_eq!(f.params.len(), 1, "send_break must have exactly 1 param");
    assert_eq!(
        f.params[0].name, "on",
        "send_break param must be named 'on'"
    );
    assert!(
        matches!(&f.params[0].ty, MirTy::Bool),
        "send_break param type must be bool"
    );
}
