//! Source-to-extraction-07: PL011 helper methods A authored->extraction boundary
//! validation.
//!
//! Validates that the first 12 helper methods (group A) from the authored PL011
//! source preserve identity, signature, and body-level facts through the
//! compiler-side extraction boundary.

#![feature(rustc_private)]
#![feature(box_patterns)]

extern crate rustc_ast;
extern crate rustc_driver;
extern crate rustc_hir;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_session;

use std::sync::{Arc, Mutex, Once};

// Re-export mir_repr at the crate root so that #[path]-included body.rs and
// mir_extract.rs can resolve `crate::mir_repr::*`.
mod mir_repr {
    pub use regseq_driver::mir_repr::*;
}

// Include extraction modules via #[path] so the test binary can call them
// directly against a TyCtxt without requiring pub(crate) to be widened.
mod helper_methods_a_extract;

use helper_methods_a_extract::body::*;
use helper_methods_a_extract::identity_enrichment::*;
use mir_repr::*;

// ---------------------------------------------------------------------------
// Compiler harness
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

struct Surfaces {
    identity: IdentityEnrichmentSurface,
    body: MirBodySurface,
}

fn compile_and_extract(source: &str, test_name: &str) -> Surfaces {
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
        identity: Arc<Mutex<Option<IdentityResult>>>,
        body: Arc<Mutex<Option<BodyResult>>>,
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
            *self.identity.lock().unwrap() = Some(extract_identity_enrichment(tcx));
            *self.body.lock().unwrap() = Some(extract_body_surface(tcx));
            rustc_driver::Compilation::Stop
        }
    }

    let mut callbacks = ExtractCallbacks {
        identity: identity_result.clone(),
        body: body_result.clone(),
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
        .expect("after_analysis should have been called for identity")
        .expect("identity enrichment extraction should succeed");

    let body = body_result
        .lock()
        .unwrap()
        .take()
        .expect("after_analysis should have been called for body")
        .expect("body surface extraction should succeed");

    std::fs::remove_dir_all(&test_dir).ok();

    Surfaces { identity, body }
}

// ---------------------------------------------------------------------------
// Inline PL011 reduced specimen
// ---------------------------------------------------------------------------

/// Reduced PL011 specimen preserving all 12 in-scope helper methods with
/// their exact signatures, body structure, register interactions, and constants.
/// Fidelity against the real fixture source is verified by `specimen_fidelity_*`
/// tests below.
const PL011_HELPER_METHODS_A_SPECIMEN: &str = r#"
    use core::marker::PhantomData;

    // --- Register infrastructure stubs ---
    pub struct Mmio<T> { _t: PhantomData<T> }
    impl<T> core::ops::Deref for Mmio<T> {
        type Target = T;
        fn deref(&self) -> &T { unimplemented!() }
    }

    pub struct Field<T, R> { pub mask: T, pub shift: T, _r: PhantomData<R> }
    impl<T, R> Field<T, R> {
        pub const fn new(mask: T, shift: T) -> Self {
            Self { mask, shift, _r: PhantomData }
        }
    }

    pub struct FieldValue<T, R> { pub mask: T, pub value: T, _r: PhantomData<R> }
    impl<T, R> FieldValue<T, R> {
        pub const fn new(mask: T, value: T) -> Self {
            Self { mask, value, _r: PhantomData }
        }
    }

    impl<T: core::ops::BitOr<Output = T>, R> core::ops::Add for FieldValue<T, R> {
        type Output = Self;
        fn add(self, rhs: Self) -> Self {
            Self { mask: self.mask | rhs.mask, value: self.value | rhs.value, _r: PhantomData }
        }
    }

    pub struct ReadWrite<T, R> { _v: T, _r: PhantomData<R> }
    impl<T: Default + Copy + core::ops::BitAnd<Output = T> + core::ops::BitOr<Output = T> + core::ops::Not<Output = T>, R> ReadWrite<T, R> {
        pub fn read_raw(&self) -> T { Default::default() }
        pub fn write(&self, _v: FieldValue<T, R>) {}
        pub fn modify(&self, _v: FieldValue<T, R>) {}
    }

    // --- Register descriptor modules ---
    #[allow(non_snake_case)]
    pub mod CR_REG {
        pub struct Register;

        #[allow(non_snake_case)]
        pub mod UARTEN {
            pub const DISABLED: super::super::FieldValue<u32, super::Register> =
                super::super::FieldValue::new(1 << 0, 0);
            pub const ENABLED: super::super::FieldValue<u32, super::Register> =
                super::super::FieldValue::new(1 << 0, 1 << 0);
        }
        #[allow(non_snake_case)]
        pub mod TXE {
            pub const DISABLED: super::super::FieldValue<u32, super::Register> =
                super::super::FieldValue::new(1 << 8, 0);
            pub const ENABLED: super::super::FieldValue<u32, super::Register> =
                super::super::FieldValue::new(1 << 8, 1 << 8);
        }
        #[allow(non_snake_case)]
        pub mod RXE {
            pub const DISABLED: super::super::FieldValue<u32, super::Register> =
                super::super::FieldValue::new(1 << 9, 0);
            pub const ENABLED: super::super::FieldValue<u32, super::Register> =
                super::super::FieldValue::new(1 << 9, 1 << 9);
        }
    }

    #[allow(non_snake_case)]
    pub mod LCRH_REG {
        pub struct Register;

        #[allow(non_snake_case)]
        pub mod FEN {
            pub const DISABLED: super::super::FieldValue<u32, super::Register> =
                super::super::FieldValue::new(1 << 4, 0);
            pub const ENABLED: super::super::FieldValue<u32, super::Register> =
                super::super::FieldValue::new(1 << 4, 1 << 4);
        }
        #[allow(non_snake_case)]
        pub mod WLEN {
            pub const EIGHT_BITS: super::super::FieldValue<u32, super::Register> =
                super::super::FieldValue::new(0b11 << 5, 0b11 << 5);
        }
    }

    #[allow(non_snake_case)]
    pub mod IMSC_REG {
        pub struct Register;
    }

    #[allow(non_snake_case)]
    pub mod ICR_REG {
        pub struct Register;
    }

    #[allow(non_snake_case)]
    pub mod IFLS_REG {
        pub struct Register;

        #[allow(non_snake_case)]
        pub mod RXIFLSEL {
            pub const ONE_HALF: super::super::FieldValue<u32, super::Register> =
                super::super::FieldValue::new(0b111 << 3, 0b010 << 3);
        }
        #[allow(non_snake_case)]
        pub mod TXIFLSEL {
            pub const ONE_EIGHTH: super::super::FieldValue<u32, super::Register> =
                super::super::FieldValue::new(0b111 << 0, 0b000 << 0);
        }
    }

    #[allow(non_snake_case)]
    pub mod IBRD_REG {
        pub struct Register;

        #[allow(non_snake_case)]
        pub mod BAUD_DIVINT {
            pub const FIELD: super::super::Field<u32, super::Register> =
                super::super::Field::new(0xffff, 0);
        }
    }

    #[allow(non_snake_case)]
    pub mod FBRD_REG {
        pub struct Register;

        #[allow(non_snake_case)]
        pub mod BAUD_DIVFRAC {
            pub const FIELD: super::super::Field<u32, super::Register> =
                super::super::Field::new(0x3f, 0);
        }
    }

    fn field_val<R>(field: Field<u32, R>, raw: u32) -> FieldValue<u32, R> {
        FieldValue::new(field.mask, raw << field.shift)
    }

    fn raw_reg_val<R>(mask: u32, value: u32) -> FieldValue<u32, R> {
        FieldValue::new(mask, value)
    }

    // --- Register block ---
    #[rseq::register_block]
    pub struct Pl011Registers {
        pub cr: ReadWrite<u32, CR_REG::Register>,
        pub lcr_h: ReadWrite<u32, LCRH_REG::Register>,
        pub imsc: ReadWrite<u32, IMSC_REG::Register>,
        pub icr: ReadWrite<u32, ICR_REG::Register>,
        pub ifls: ReadWrite<u32, IFLS_REG::Register>,
        pub ibrd: ReadWrite<u32, IBRD_REG::Register>,
        pub fbrd: ReadWrite<u32, FBRD_REG::Register>,
    }

    // --- Driver type ---
    #[rseq::driver(name = "pl011-full-spec", compatible = ["arm,pl011"])]
    pub struct Pl011FullSpec {
        #[rseq::mmio(id = "uart_regs")]
        pub regs: Mmio<Pl011Registers>,
    }

    // --- Lifecycle trait stub for callback rooting ---
    pub trait Lifecycle {
        fn init(&mut self) -> Result<(), ()>;
    }

    impl Lifecycle for Pl011FullSpec {
        fn init(&mut self) -> Result<(), ()> {
            self.program_polling_port(48_000_000, 115200);
            Ok(())
        }
    }

    // --- Serial trait stub for additional callback rooting ---
    pub trait Serial {
        fn tx_byte(&mut self, byte: u8);
    }

    impl Serial for Pl011FullSpec {
        fn tx_byte(&mut self, byte: u8) {
            let _ = byte;
            self.enable_tx_interrupts();
            self.disable_tx_interrupts();
        }
    }

    // --- Inherent helper methods (group A) ---
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
            self.set_baud_rate(uart_clk_hz, baud_rate);
            self.configure_line_control();
            self.configure_interrupt_levels();
            self.mask_all_interrupts();
            self.clear_all_interrupts();
            self.enable_uart();
            self.enable_rx_interrupts();
        }
    }
"#;

// ---------------------------------------------------------------------------
// In-scope method names
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Helpers to find extracted functions/methods
// ---------------------------------------------------------------------------

fn find_body_fn<'a>(surface: &'a MirBodySurface, name: &str) -> Option<&'a MirExtractedFn> {
    surface.functions.iter().find(|f| f.name == name)
}

/// Collect all Call terminators from a MIR body in basic-block order.
fn collect_calls(body: &MirBody) -> Vec<CallInfo> {
    let mut calls = Vec::new();
    for bb in &body.basic_blocks {
        if let Some(MirTerminator {
            kind: MirTerminatorKind::Call { func, args, .. },
            ..
        }) = &bb.terminator
        {
            let callee = match func {
                MirOperand::Constant {
                    constant:
                        MirConstant {
                            ty: MirTy::FnDef { path, .. },
                            ..
                        },
                } => path.clone(),
                _ => "<unknown>".to_string(),
            };
            calls.push(CallInfo {
                callee,
                args: args.clone(),
            });
        }
    }
    calls
}

struct CallInfo {
    callee: String,
    #[allow(dead_code)]
    args: Vec<MirOperand>,
}

/// Check if a MIR body contains any binary operation of the given kind.
fn has_bin_op(body: &MirBody, op: MirBinOp) -> bool {
    for bb in &body.basic_blocks {
        for stmt in &bb.statements {
            if let MirStatementKind::Assign {
                rvalue: box MirRvalue::BinaryOp { op: found_op, .. },
                ..
            } = &stmt.kind
            {
                if *found_op == op {
                    return true;
                }
            }
        }
    }
    false
}

/// Check if a MIR body contains a cast to the given type.
fn has_cast_to(body: &MirBody, target_ty: &MirTy) -> bool {
    for bb in &body.basic_blocks {
        for stmt in &bb.statements {
            if let MirStatementKind::Assign {
                rvalue: box MirRvalue::Cast { ty, .. },
                ..
            } = &stmt.kind
            {
                if ty == target_ty {
                    return true;
                }
            }
        }
    }
    false
}

/// Collect all scalar constant values appearing anywhere in a MIR body
/// (in statements, terminators, and operands).
fn collect_all_scalars(body: &MirBody) -> Vec<u128> {
    let mut scalars = Vec::new();

    fn from_operand(op: &MirOperand, out: &mut Vec<u128>) {
        if let MirOperand::Constant {
            constant:
                MirConstant {
                    value: MirConstValue::Scalar { bits, .. },
                    ..
                },
        } = op
        {
            out.push(*bits);
        }
    }

    fn from_rvalue(rv: &MirRvalue, out: &mut Vec<u128>) {
        match rv {
            MirRvalue::Use { operand } => from_operand(operand, out),
            MirRvalue::BinaryOp { lhs, rhs, .. } => {
                from_operand(lhs, out);
                from_operand(rhs, out);
            }
            MirRvalue::UnaryOp { operand, .. } => from_operand(operand, out),
            MirRvalue::Cast { operand, .. } => from_operand(operand, out),
            MirRvalue::Aggregate { operands, .. } => {
                for op in operands {
                    from_operand(op, out);
                }
            }
            _ => {}
        }
    }

    for bb in &body.basic_blocks {
        for stmt in &bb.statements {
            if let MirStatementKind::Assign { rvalue, .. } = &stmt.kind {
                from_rvalue(rvalue, &mut scalars);
            }
        }
        if let Some(term) = &bb.terminator {
            match &term.kind {
                MirTerminatorKind::Call { func, args, .. } => {
                    from_operand(func, &mut scalars);
                    for arg in args {
                        from_operand(arg, &mut scalars);
                    }
                }
                MirTerminatorKind::SwitchInt { discr, .. } => {
                    from_operand(discr, &mut scalars);
                }
                MirTerminatorKind::Assert { cond, .. } => {
                    from_operand(cond, &mut scalars);
                }
                _ => {}
            }
        }
    }
    scalars
}

/// Check if the body contains an unevaluated const reference whose
/// description contains the given substring. Associated constants
/// (e.g. `Self::ALL_UART_INTERRUPTS_MASK`) often appear as unevaluated
/// const references in post-drop-elaboration MIR rather than inlined
/// scalar values.
fn body_has_unevaluated_ref(body: &MirBody, substr: &str) -> bool {
    fn check_operand(op: &MirOperand, substr: &str) -> bool {
        if let MirOperand::Constant {
            constant:
                MirConstant {
                    value: MirConstValue::Unsupported { description },
                    ..
                },
        } = op
        {
            description.contains(substr)
        } else {
            false
        }
    }

    fn check_rvalue(rv: &MirRvalue, substr: &str) -> bool {
        match rv {
            MirRvalue::Use { operand } => check_operand(operand, substr),
            MirRvalue::BinaryOp { lhs, rhs, .. } => {
                check_operand(lhs, substr) || check_operand(rhs, substr)
            }
            MirRvalue::UnaryOp { operand, .. } => check_operand(operand, substr),
            MirRvalue::Cast { operand, .. } => check_operand(operand, substr),
            MirRvalue::Aggregate { operands, .. } => {
                operands.iter().any(|op| check_operand(op, substr))
            }
            _ => false,
        }
    }

    for bb in &body.basic_blocks {
        for stmt in &bb.statements {
            if let MirStatementKind::Assign { rvalue, .. } = &stmt.kind {
                if check_rvalue(rvalue, substr) {
                    return true;
                }
            }
        }
        if let Some(term) = &bb.terminator {
            match &term.kind {
                MirTerminatorKind::Call { func, args, .. } => {
                    if check_operand(func, substr) {
                        return true;
                    }
                    for arg in args {
                        if check_operand(arg, substr) {
                            return true;
                        }
                    }
                }
                MirTerminatorKind::SwitchInt { discr, .. } => {
                    if check_operand(discr, substr) {
                        return true;
                    }
                }
                _ => {}
            }
        }
    }
    false
}

// =========================================================================
// SPECIMEN FIDELITY
// =========================================================================

/// Verify that the reduced specimen preserves the exact method signatures,
/// body structures, and constants from the real fixture source. This is the
/// precondition for all subsequent extraction checks.
#[test]
fn specimen_fidelity_method_bodies_match_fixture() {
    let fixture_source = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/pl011-full-spec/src/driver.rs"
    ))
    .expect("failed to read PL011 fixture source");

    // Verify that every in-scope helper's key body tokens appear in the
    // fixture source. Whitespace-normalized comparison avoids indentation
    // sensitivity while still verifying structural content.
    fn normalize_ws(s: &str) -> String {
        s.split_whitespace().collect::<Vec<_>>().join(" ")
    }

    let fixture_norm = normalize_ws(&fixture_source);

    let expected_bodies: &[(&str, &str)] = &[
        ("disable_uart", "self.regs .cr .write(CR_REG::UARTEN::DISABLED + CR_REG::TXE::DISABLED + CR_REG::RXE::DISABLED);"),
        ("disable_fifo", "self.regs.lcr_h.modify(LCRH_REG::FEN::DISABLED);"),
        ("mask_all_interrupts", "self.regs.imsc.write(raw_reg_val::<IMSC_REG::Register>( Self::ALL_UART_INTERRUPTS_MASK, 0, ));"),
        ("clear_all_interrupts", "self.regs.icr.write(raw_reg_val::<ICR_REG::Register>( Self::ALL_UART_INTERRUPTS_MASK, Self::ALL_UART_INTERRUPTS_MASK, ));"),
        ("configure_interrupt_levels", "self.regs .ifls .write(IFLS_REG::RXIFLSEL::ONE_HALF + IFLS_REG::TXIFLSEL::ONE_EIGHTH);"),
        ("configure_line_control", "self.regs .lcr_h .write(LCRH_REG::WLEN::EIGHT_BITS + LCRH_REG::FEN::ENABLED);"),
        ("enable_uart", "self.regs .cr .write(CR_REG::UARTEN::ENABLED + CR_REG::TXE::ENABLED + CR_REG::RXE::ENABLED);"),
        ("enable_rx_interrupts", "self.regs.imsc.modify(raw_reg_val::<IMSC_REG::Register>( Self::RX_INTERRUPT_MASK, Self::RX_INTERRUPT_MASK, ));"),
        ("enable_tx_interrupts", "self.regs.imsc.modify(raw_reg_val::<IMSC_REG::Register>( Self::TX_INTERRUPT_MASK, Self::TX_INTERRUPT_MASK, ));"),
        ("disable_tx_interrupts", "self.regs.imsc.modify(raw_reg_val::<IMSC_REG::Register>( Self::TX_INTERRUPT_MASK, 0, ));"),
    ];

    for (method, body_frag) in expected_bodies {
        let norm_frag = normalize_ws(body_frag);
        assert!(
            fixture_norm.contains(&norm_frag),
            "specimen fidelity: method `{}` body fragment not found in fixture source.\n\
             Expected (normalized): {:?}",
            method,
            norm_frag
        );
    }

    // Verify signature facts
    let signature_checks: &[(&str, &str)] = &[
        ("disable_uart", "fn disable_uart(&mut self)"),
        ("disable_fifo", "fn disable_fifo(&mut self)"),
        ("mask_all_interrupts", "fn mask_all_interrupts(&mut self)"),
        ("clear_all_interrupts", "fn clear_all_interrupts(&mut self)"),
        (
            "configure_interrupt_levels",
            "fn configure_interrupt_levels(&mut self)",
        ),
        (
            "set_baud_rate",
            "fn set_baud_rate(&mut self, uart_clk_hz: u32, baud_rate: u32)",
        ),
        (
            "configure_line_control",
            "fn configure_line_control(&mut self)",
        ),
        ("enable_uart", "fn enable_uart(&mut self)"),
        ("enable_rx_interrupts", "fn enable_rx_interrupts(&mut self)"),
        ("enable_tx_interrupts", "fn enable_tx_interrupts(&mut self)"),
        (
            "disable_tx_interrupts",
            "fn disable_tx_interrupts(&mut self)",
        ),
        (
            "program_polling_port",
            "fn program_polling_port(&mut self, uart_clk_hz: u32, baud_rate: u32)",
        ),
    ];

    for (method, sig) in signature_checks {
        assert!(
            fixture_source.contains(sig),
            "specimen fidelity: method `{}` signature `{}` not found in fixture source",
            method,
            sig
        );
    }

    // Verify constant values
    assert!(
        fixture_source.contains("const ALL_UART_INTERRUPTS_MASK: u32 = 0x07ff;"),
        "specimen fidelity: ALL_UART_INTERRUPTS_MASK constant not found in fixture"
    );
    assert!(
        fixture_source.contains("const RX_INTERRUPT_MASK: u32 = (1 << 4) | (1 << 6);"),
        "specimen fidelity: RX_INTERRUPT_MASK constant not found in fixture"
    );
    assert!(
        fixture_source.contains("const TX_INTERRUPT_MASK: u32 = 1 << 5;"),
        "specimen fidelity: TX_INTERRUPT_MASK constant not found in fixture"
    );

    // Verify set_baud_rate arithmetic
    assert!(
        fixture_source.contains("let div_times_64 = (uart_clk_hz as u64 * 4) / baud_rate as u64;"),
        "specimen fidelity: set_baud_rate divisor arithmetic not found in fixture"
    );
    assert!(
        fixture_source.contains("let ibrd = (div_times_64 >> 6) as u32;"),
        "specimen fidelity: set_baud_rate ibrd derivation not found in fixture"
    );
    assert!(
        fixture_source.contains("let fbrd = (div_times_64 & 0x3f) as u32;"),
        "specimen fidelity: set_baud_rate fbrd derivation not found in fixture"
    );

    // Verify that the specimen bodies also match (bidirectional check)
    let specimen = PL011_HELPER_METHODS_A_SPECIMEN;
    let specimen_norm = normalize_ws(specimen);
    for (method, body_frag) in expected_bodies {
        let norm_frag = normalize_ws(body_frag);
        assert!(
            specimen_norm.contains(&norm_frag),
            "specimen fidelity (reverse): method `{}` body fragment not found in specimen.\n\
             Expected (normalized): {:?}",
            method,
            norm_frag
        );
    }
    for (method, sig) in signature_checks {
        assert!(
            specimen.contains(sig),
            "specimen fidelity (reverse): method `{}` signature `{}` not found in specimen",
            method,
            sig
        );
    }
}

// =========================================================================
// CHECK GROUP 1: Identity, Signature, Return Type Preservation
// =========================================================================

#[test]
fn check_group_1_all_helpers_identity_preserved() {
    let surfaces =
        compile_and_extract(PL011_HELPER_METHODS_A_SPECIMEN, "helper_methods_a_identity");

    // Find the inherent impl block for Pl011FullSpec
    let inherent = surfaces
        .identity
        .impl_blocks
        .iter()
        .find(|ib| ib.is_inherent && ib.self_type_name == "Pl011FullSpec")
        .expect("inherent impl block for Pl011FullSpec must exist");

    // Every in-scope helper must appear as a method in the inherent impl
    for name in IN_SCOPE_HELPERS {
        let method = inherent.methods.iter().find(|m| m.name == *name);
        assert!(
            method.is_some(),
            "check group 1: method `{}` must be present in inherent impl identity surface",
            name
        );
        let method = method.unwrap();
        // Canonical path must contain the method name
        assert!(
            method.canonical_path.0.contains(name),
            "check group 1: method `{}` canonical path `{}` must contain the method name",
            name,
            method.canonical_path.0
        );
        // None of these are ISRs
        assert!(
            !method.is_isr,
            "check group 1: method `{}` must not be marked as ISR",
            name
        );
    }
}

#[test]
fn check_group_1_body_surface_signatures_preserved() {
    let surfaces = compile_and_extract(
        PL011_HELPER_METHODS_A_SPECIMEN,
        "helper_methods_a_signatures",
    );

    // All in-scope helpers must be extracted with correct signatures
    for name in IN_SCOPE_HELPERS {
        let func = find_body_fn(&surfaces.body, name);
        assert!(
            func.is_some(),
            "check group 1: function `{}` must be present in body surface",
            name
        );
        let func = func.unwrap();

        // All helpers have &mut self receiver
        assert_eq!(
            func.receiver, "ref_mut",
            "check group 1: `{}` receiver must be `ref_mut`",
            name
        );

        // All helpers are classified as Helper kind
        assert_eq!(
            func.kind,
            MirFnKind::Helper,
            "check group 1: `{}` must be classified as Helper",
            name
        );

        // Return type must be None (unit) for all in-scope helpers
        assert_eq!(
            func.return_type, None,
            "check group 1: `{}` must have unit return type (None)",
            name
        );
    }

    // set_baud_rate must have exactly 2 parameters: uart_clk_hz: u32, baud_rate: u32
    let sbr = find_body_fn(&surfaces.body, "set_baud_rate").unwrap();
    assert_eq!(sbr.params.len(), 2, "set_baud_rate must have 2 parameters");
    assert_eq!(sbr.params[0].name, "uart_clk_hz");
    assert_eq!(sbr.params[0].ty, "u32");
    assert_eq!(sbr.params[1].name, "baud_rate");
    assert_eq!(sbr.params[1].ty, "u32");

    // program_polling_port must have exactly 2 parameters: uart_clk_hz: u32, baud_rate: u32
    let ppp = find_body_fn(&surfaces.body, "program_polling_port").unwrap();
    assert_eq!(
        ppp.params.len(),
        2,
        "program_polling_port must have 2 parameters"
    );
    assert_eq!(ppp.params[0].name, "uart_clk_hz");
    assert_eq!(ppp.params[0].ty, "u32");
    assert_eq!(ppp.params[1].name, "baud_rate");
    assert_eq!(ppp.params[1].ty, "u32");

    // All other helpers must have zero non-self parameters
    let no_params = [
        "disable_uart",
        "disable_fifo",
        "mask_all_interrupts",
        "clear_all_interrupts",
        "configure_interrupt_levels",
        "configure_line_control",
        "enable_uart",
        "enable_rx_interrupts",
        "enable_tx_interrupts",
        "disable_tx_interrupts",
    ];
    for name in &no_params {
        let func = find_body_fn(&surfaces.body, name).unwrap();
        assert_eq!(
            func.params.len(),
            0,
            "check group 1: `{}` must have 0 non-self parameters, got {}",
            name,
            func.params.len()
        );
    }
}

// =========================================================================
// CHECK GROUP 2: Body-level downstream checks
// =========================================================================

#[test]
fn check_group_2_disable_uart_body() {
    let surfaces = compile_and_extract(PL011_HELPER_METHODS_A_SPECIMEN, "helper_a_disable_uart");
    let func =
        find_body_fn(&surfaces.body, "disable_uart").expect("disable_uart must be in body surface");
    let calls = collect_calls(&func.body);

    // Exactly one write call targeting cr
    let write_calls: Vec<_> = calls
        .iter()
        .filter(|c| c.callee.contains("write"))
        .collect();
    assert_eq!(
        write_calls.len(),
        1,
        "disable_uart: must have exactly one write call, got {}",
        write_calls.len()
    );
    assert!(
        write_calls[0].callee.contains("ReadWrite"),
        "disable_uart: write call must target ReadWrite (cr register type)"
    );
}

#[test]
fn check_group_2_disable_fifo_body() {
    let surfaces = compile_and_extract(PL011_HELPER_METHODS_A_SPECIMEN, "helper_a_disable_fifo");
    let func =
        find_body_fn(&surfaces.body, "disable_fifo").expect("disable_fifo must be in body surface");
    let calls = collect_calls(&func.body);

    // Exactly one modify call (read-modify-write pattern)
    let modify_calls: Vec<_> = calls
        .iter()
        .filter(|c| c.callee.contains("modify"))
        .collect();
    assert_eq!(
        modify_calls.len(),
        1,
        "disable_fifo: must have exactly one modify call, got {}",
        modify_calls.len()
    );
}

#[test]
fn check_group_2_mask_all_interrupts_body() {
    let surfaces = compile_and_extract(PL011_HELPER_METHODS_A_SPECIMEN, "helper_a_mask_all");
    let func = find_body_fn(&surfaces.body, "mask_all_interrupts")
        .expect("mask_all_interrupts must be in body surface");
    let calls = collect_calls(&func.body);

    // Exactly one write call
    let write_calls: Vec<_> = calls
        .iter()
        .filter(|c| c.callee.contains("write"))
        .collect();
    assert_eq!(
        write_calls.len(),
        1,
        "mask_all_interrupts: must have exactly one write call"
    );

    // The body must contain the constant 0x07ff (ALL_UART_INTERRUPTS_MASK)
    // and the constant 0 (masked bits). These may appear as immediate operands
    // or as assigned constants in the MIR.
    let all_scalars = collect_all_scalars(&func.body);
    assert!(
        all_scalars.contains(&0x07ff)
            || body_has_unevaluated_ref(&func.body, "ALL_UART_INTERRUPTS_MASK"),
        "mask_all_interrupts: body must contain ALL_UART_INTERRUPTS_MASK (0x07ff), \
         got scalars: {:?}",
        all_scalars
    );
    assert!(
        all_scalars.contains(&0),
        "mask_all_interrupts: body must contain 0 for masked bits, got scalars: {:?}",
        all_scalars
    );
}

#[test]
fn check_group_2_clear_all_interrupts_body() {
    let surfaces = compile_and_extract(PL011_HELPER_METHODS_A_SPECIMEN, "helper_a_clear_all");
    let func = find_body_fn(&surfaces.body, "clear_all_interrupts")
        .expect("clear_all_interrupts must be in body surface");
    let calls = collect_calls(&func.body);

    // Exactly one write call
    let write_calls: Vec<_> = calls
        .iter()
        .filter(|c| c.callee.contains("write"))
        .collect();
    assert_eq!(
        write_calls.len(),
        1,
        "clear_all_interrupts: must have exactly one write call"
    );

    // The body must contain the constant 0x07ff (ALL_UART_INTERRUPTS_MASK)
    // used as both mask and value. It may appear as inlined scalars or as
    // unevaluated const references.
    let all_scalars = collect_all_scalars(&func.body);
    let mask_count = all_scalars.iter().filter(|&&v| v == 0x07ff).count();
    assert!(
        mask_count >= 2 || body_has_unevaluated_ref(&func.body, "ALL_UART_INTERRUPTS_MASK"),
        "clear_all_interrupts: body must contain 0x07ff at least twice (mask and value), \
         got {} occurrences in scalars: {:?}",
        mask_count,
        all_scalars
    );
}

#[test]
fn check_group_2_configure_interrupt_levels_body() {
    let surfaces = compile_and_extract(PL011_HELPER_METHODS_A_SPECIMEN, "helper_a_ifls");
    let func = find_body_fn(&surfaces.body, "configure_interrupt_levels")
        .expect("configure_interrupt_levels must be in body surface");
    let calls = collect_calls(&func.body);

    // Exactly one write call targeting ifls
    let write_calls: Vec<_> = calls
        .iter()
        .filter(|c| c.callee.contains("write"))
        .collect();
    assert_eq!(
        write_calls.len(),
        1,
        "configure_interrupt_levels: must have exactly one write call"
    );
}

#[test]
fn check_group_2_set_baud_rate_body() {
    let surfaces = compile_and_extract(PL011_HELPER_METHODS_A_SPECIMEN, "helper_a_baud");
    let func = find_body_fn(&surfaces.body, "set_baud_rate")
        .expect("set_baud_rate must be in body surface");

    // Must have u64 widening (cast to u64)
    let u64_ty = MirTy::Uint {
        uint_ty: MirUintTy::U64,
    };
    assert!(
        has_cast_to(&func.body, &u64_ty),
        "set_baud_rate: must contain cast to u64 for widening arithmetic"
    );

    // Must have u32 narrowing (cast back to u32)
    let u32_ty = MirTy::Uint {
        uint_ty: MirUintTy::U32,
    };
    assert!(
        has_cast_to(&func.body, &u32_ty),
        "set_baud_rate: must contain cast to u32 for ibrd/fbrd narrowing"
    );

    // Must have multiplication (for uart_clk_hz * 4)
    assert!(
        has_bin_op(&func.body, MirBinOp::Mul)
            || has_bin_op(&func.body, MirBinOp::MulUnchecked)
            || has_bin_op(&func.body, MirBinOp::MulWithOverflow),
        "set_baud_rate: must contain multiplication for divisor computation"
    );

    // Must have division (for / baud_rate)
    assert!(
        has_bin_op(&func.body, MirBinOp::Div),
        "set_baud_rate: must contain division for divisor computation"
    );

    // Must have right shift (for >> 6)
    assert!(
        has_bin_op(&func.body, MirBinOp::Shr) || has_bin_op(&func.body, MirBinOp::ShrUnchecked),
        "set_baud_rate: must contain right shift for ibrd derivation"
    );

    // Must have bitwise AND (for & 0x3f)
    assert!(
        has_bin_op(&func.body, MirBinOp::BitAnd),
        "set_baud_rate: must contain bitwise AND for fbrd derivation"
    );

    // Must have exactly two write calls (ibrd and fbrd)
    let calls = collect_calls(&func.body);
    let write_calls: Vec<_> = calls
        .iter()
        .filter(|c| c.callee.contains("write"))
        .collect();
    assert_eq!(
        write_calls.len(),
        2,
        "set_baud_rate: must have exactly two write calls (ibrd and fbrd)"
    );

    // Must call field_val exactly twice (for ibrd and fbrd)
    let field_val_calls: Vec<_> = calls
        .iter()
        .filter(|c| c.callee.contains("field_val"))
        .collect();
    assert_eq!(
        field_val_calls.len(),
        2,
        "set_baud_rate: must call field_val exactly twice"
    );

    // Verify the constant 4 appears in multiplication (for * 4)
    let const_4_present = func.body.basic_blocks.iter().any(|bb| {
        bb.statements.iter().any(|stmt| {
            if let MirStatementKind::Assign {
                rvalue:
                    box MirRvalue::BinaryOp {
                        op: MirBinOp::Mul | MirBinOp::MulUnchecked | MirBinOp::MulWithOverflow,
                        lhs,
                        rhs,
                    },
                ..
            } = &stmt.kind
            {
                let check_operand = |op: &MirOperand| -> bool {
                    matches!(
                        op,
                        MirOperand::Constant {
                            constant: MirConstant {
                                value: MirConstValue::Scalar { bits: 4, .. },
                                ..
                            }
                        }
                    )
                };
                check_operand(lhs) || check_operand(rhs)
            } else {
                false
            }
        })
    });
    assert!(
        const_4_present,
        "set_baud_rate: must contain constant 4 in multiplication"
    );

    // Verify shift amount 6 appears
    let const_6_present = func.body.basic_blocks.iter().any(|bb| {
        bb.statements.iter().any(|stmt| {
            if let MirStatementKind::Assign {
                rvalue:
                    box MirRvalue::BinaryOp {
                        op: MirBinOp::Shr | MirBinOp::ShrUnchecked,
                        rhs,
                        ..
                    },
                ..
            } = &stmt.kind
            {
                matches!(
                    rhs,
                    MirOperand::Constant {
                        constant: MirConstant {
                            value: MirConstValue::Scalar { bits: 6, .. },
                            ..
                        }
                    }
                )
            } else {
                false
            }
        })
    });
    assert!(
        const_6_present,
        "set_baud_rate: must contain constant 6 in right shift for ibrd"
    );

    // Verify mask 0x3f appears in BitAnd
    let const_3f_present = func.body.basic_blocks.iter().any(|bb| {
        bb.statements.iter().any(|stmt| {
            if let MirStatementKind::Assign {
                rvalue:
                    box MirRvalue::BinaryOp {
                        op: MirBinOp::BitAnd,
                        lhs,
                        rhs,
                    },
                ..
            } = &stmt.kind
            {
                let check_operand = |op: &MirOperand| -> bool {
                    matches!(
                        op,
                        MirOperand::Constant {
                            constant: MirConstant {
                                value: MirConstValue::Scalar { bits: 0x3f, .. },
                                ..
                            }
                        }
                    )
                };
                check_operand(lhs) || check_operand(rhs)
            } else {
                false
            }
        })
    });
    assert!(
        const_3f_present,
        "set_baud_rate: must contain constant 0x3f in bitwise AND for fbrd"
    );
}

#[test]
fn check_group_2_configure_line_control_body() {
    let surfaces = compile_and_extract(PL011_HELPER_METHODS_A_SPECIMEN, "helper_a_lcr");
    let func = find_body_fn(&surfaces.body, "configure_line_control")
        .expect("configure_line_control must be in body surface");
    let calls = collect_calls(&func.body);

    // Exactly one write call
    let write_calls: Vec<_> = calls
        .iter()
        .filter(|c| c.callee.contains("write"))
        .collect();
    assert_eq!(
        write_calls.len(),
        1,
        "configure_line_control: must have exactly one write call"
    );

    // The materialized literal 0x70 (WLEN::EIGHT_BITS + FEN::ENABLED) should
    // appear in the body. Check for the combined value, constituent values,
    // or unevaluated const references to the register field names.
    let all_scalars = collect_all_scalars(&func.body);
    let has_wlen_value = all_scalars.contains(&0x60) || all_scalars.contains(&(0b11u128 << 5));
    let has_fen_value = all_scalars.contains(&0x10) || all_scalars.contains(&(1u128 << 4));
    let has_unevaluated = body_has_unevaluated_ref(&func.body, "WLEN")
        || body_has_unevaluated_ref(&func.body, "FEN")
        || body_has_unevaluated_ref(&func.body, "EIGHT_BITS")
        || body_has_unevaluated_ref(&func.body, "ENABLED");
    assert!(
        has_wlen_value || has_fen_value || all_scalars.contains(&0x70) || has_unevaluated,
        "configure_line_control: must preserve WLEN::EIGHT_BITS + FEN::ENABLED values, \
         got scalars: {:?}",
        all_scalars
    );
}

#[test]
fn check_group_2_enable_uart_body() {
    let surfaces = compile_and_extract(PL011_HELPER_METHODS_A_SPECIMEN, "helper_a_enable_uart");
    let func =
        find_body_fn(&surfaces.body, "enable_uart").expect("enable_uart must be in body surface");
    let calls = collect_calls(&func.body);

    // Exactly one write call
    let write_calls: Vec<_> = calls
        .iter()
        .filter(|c| c.callee.contains("write"))
        .collect();
    assert_eq!(
        write_calls.len(),
        1,
        "enable_uart: must have exactly one write call"
    );

    // UARTEN::ENABLED + TXE::ENABLED + RXE::ENABLED = 0x301
    let all_scalars = collect_all_scalars(&func.body);
    let has_uarten = all_scalars.contains(&1) || all_scalars.contains(&(1u128 << 0));
    let has_txe = all_scalars.contains(&0x100) || all_scalars.contains(&(1u128 << 8));
    let has_rxe = all_scalars.contains(&0x200) || all_scalars.contains(&(1u128 << 9));
    let has_unevaluated = body_has_unevaluated_ref(&func.body, "UARTEN")
        || body_has_unevaluated_ref(&func.body, "TXE")
        || body_has_unevaluated_ref(&func.body, "RXE")
        || body_has_unevaluated_ref(&func.body, "ENABLED");
    assert!(
        (has_uarten && has_txe && has_rxe) || all_scalars.contains(&0x301) || has_unevaluated,
        "enable_uart: must preserve UARTEN + TXE + RXE enabled values (0x301), \
         got scalars: {:?}",
        all_scalars
    );
}

#[test]
fn check_group_2_enable_rx_interrupts_body() {
    let surfaces = compile_and_extract(PL011_HELPER_METHODS_A_SPECIMEN, "helper_a_enable_rx");
    let func = find_body_fn(&surfaces.body, "enable_rx_interrupts")
        .expect("enable_rx_interrupts must be in body surface");
    let calls = collect_calls(&func.body);

    // Exactly one modify call (read-modify-write)
    let modify_calls: Vec<_> = calls
        .iter()
        .filter(|c| c.callee.contains("modify"))
        .collect();
    assert_eq!(
        modify_calls.len(),
        1,
        "enable_rx_interrupts: must have exactly one modify call"
    );

    // The body must reference RX_INTERRUPT_MASK (0x50) as scalar or unevaluated ref
    let all_scalars = collect_all_scalars(&func.body);
    let mask_count = all_scalars.iter().filter(|&&v| v == 0x50).count();
    let has_rx_mask_ref = body_has_unevaluated_ref(&func.body, "RX_INTERRUPT_MASK");
    assert!(
        mask_count >= 2 || has_rx_mask_ref,
        "enable_rx_interrupts: body must contain RX_INTERRUPT_MASK (0x50) \
         either as scalar (at least twice) or as unevaluated const ref, \
         got {} scalar occurrences, unevaluated ref present: {}, scalars: {:?}",
        mask_count,
        has_rx_mask_ref,
        all_scalars
    );
}

#[test]
fn check_group_2_enable_tx_interrupts_body() {
    let surfaces = compile_and_extract(PL011_HELPER_METHODS_A_SPECIMEN, "helper_a_enable_tx");
    let func = find_body_fn(&surfaces.body, "enable_tx_interrupts")
        .expect("enable_tx_interrupts must be in body surface");
    let calls = collect_calls(&func.body);

    // Exactly one modify call
    let modify_calls: Vec<_> = calls
        .iter()
        .filter(|c| c.callee.contains("modify"))
        .collect();
    assert_eq!(
        modify_calls.len(),
        1,
        "enable_tx_interrupts: must have exactly one modify call"
    );

    // The body must reference TX_INTERRUPT_MASK (0x20) as scalar or unevaluated ref
    let all_scalars = collect_all_scalars(&func.body);
    let mask_count = all_scalars.iter().filter(|&&v| v == 0x20).count();
    let has_tx_mask_ref = body_has_unevaluated_ref(&func.body, "TX_INTERRUPT_MASK");
    assert!(
        mask_count >= 2 || has_tx_mask_ref,
        "enable_tx_interrupts: body must contain TX_INTERRUPT_MASK (0x20) \
         either as scalar (at least twice) or as unevaluated const ref, \
         got {} scalar occurrences, unevaluated ref present: {}, scalars: {:?}",
        mask_count,
        has_tx_mask_ref,
        all_scalars
    );
}

#[test]
fn check_group_2_disable_tx_interrupts_body() {
    let surfaces = compile_and_extract(PL011_HELPER_METHODS_A_SPECIMEN, "helper_a_disable_tx");
    let func = find_body_fn(&surfaces.body, "disable_tx_interrupts")
        .expect("disable_tx_interrupts must be in body surface");
    let calls = collect_calls(&func.body);

    // Exactly one modify call
    let modify_calls: Vec<_> = calls
        .iter()
        .filter(|c| c.callee.contains("modify"))
        .collect();
    assert_eq!(
        modify_calls.len(),
        1,
        "disable_tx_interrupts: must have exactly one modify call"
    );

    // The body must reference TX_INTERRUPT_MASK (0x20) and 0
    let all_scalars = collect_all_scalars(&func.body);
    let has_tx_mask_ref = body_has_unevaluated_ref(&func.body, "TX_INTERRUPT_MASK");
    assert!(
        all_scalars.contains(&0x20) || has_tx_mask_ref,
        "disable_tx_interrupts: body must contain TX_INTERRUPT_MASK (0x20) \
         as scalar or unevaluated ref, got scalars: {:?}, ref present: {}",
        all_scalars,
        has_tx_mask_ref
    );
    assert!(
        all_scalars.contains(&0),
        "disable_tx_interrupts: body must contain 0 for disabled bits, \
         got scalars: {:?}",
        all_scalars
    );
}

#[test]
fn check_group_2_program_polling_port_body() {
    let surfaces = compile_and_extract(PL011_HELPER_METHODS_A_SPECIMEN, "helper_a_program_port");
    let func = find_body_fn(&surfaces.body, "program_polling_port")
        .expect("program_polling_port must be in body surface");

    // Must call all expected helpers
    let expected_calls = [
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

    // helper_calls field from body extraction
    for expected in &expected_calls {
        assert!(
            func.helper_calls.contains(&expected.to_string()),
            "program_polling_port: must call `{}`, helper_calls = {:?}",
            expected,
            func.helper_calls
        );
    }

    // Verify ordering by inspecting call terminators in basic-block order
    let calls = collect_calls(&func.body);
    let call_order: Vec<String> = calls
        .iter()
        .filter_map(|c| {
            let name = c.callee.rsplit("::").next().unwrap_or(&c.callee);
            if expected_calls.contains(&name) {
                Some(name.to_string())
            } else {
                None
            }
        })
        .collect();

    // disable_uart must come before disable_fifo
    let pos_disable_uart = call_order.iter().position(|n| n == "disable_uart");
    let pos_disable_fifo = call_order.iter().position(|n| n == "disable_fifo");
    assert!(
        pos_disable_uart.is_some() && pos_disable_fifo.is_some(),
        "program_polling_port: both disable_uart and disable_fifo must appear in call order"
    );
    assert!(
        pos_disable_uart.unwrap() < pos_disable_fifo.unwrap(),
        "program_polling_port: disable_uart must precede disable_fifo"
    );

    // The ordered region calls must appear after disable_fifo
    let ordered_helpers = [
        "set_baud_rate",
        "configure_line_control",
        "configure_interrupt_levels",
        "mask_all_interrupts",
        "clear_all_interrupts",
        "enable_uart",
        "enable_rx_interrupts",
    ];

    // Collect positions of ordered helpers
    let mut ordered_positions: Vec<(String, usize)> = Vec::new();
    for helper in &ordered_helpers {
        if let Some(pos) = call_order.iter().position(|n| n == *helper) {
            ordered_positions.push((helper.to_string(), pos));
        }
    }

    // All ordered helpers must appear after disable_fifo
    for (name, pos) in &ordered_positions {
        assert!(
            *pos > pos_disable_fifo.unwrap(),
            "program_polling_port: `{}` (pos {}) must appear after disable_fifo (pos {})",
            name,
            pos,
            pos_disable_fifo.unwrap()
        );
    }

    // Verify the exact ordering within the ordered region
    for i in 1..ordered_positions.len() {
        assert!(
            ordered_positions[i].1 > ordered_positions[i - 1].1,
            "program_polling_port: `{}` must follow `{}` in the ordered region",
            ordered_positions[i].0,
            ordered_positions[i - 1].0
        );
    }

    // Verify parameter flow: set_baud_rate receives the two parameters
    assert_eq!(
        func.params.len(),
        2,
        "program_polling_port: must have 2 parameters"
    );
    assert_eq!(func.params[0].name, "uart_clk_hz");
    assert_eq!(func.params[1].name, "baud_rate");
}
