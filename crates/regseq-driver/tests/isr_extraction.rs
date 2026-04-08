//! source-to-extraction-12: PL011 ISR authored->extraction boundary validation.
//!
//! Validates that the authored ISR facts for PL011 survive at the
//! compiler-side boundary between the authored PL011 source and the current
//! Regseq extraction state.
//!
//! Extraction surfaces under test:
//! - `InterruptSurface.isr_bindings` (ISR identity and annotation)
//! - `IdentityEnrichmentSurface.impl_blocks` (method identity, ISR marker)
//! - `MirBodySurface.functions` (ISR body: status read, branching, dispatch, clear)

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

#[path = "../src/extract/identity_enrichment_types.rs"]
mod identity_enrichment_types;

#[path = "../src/extract/identity_enrichment.rs"]
mod identity_enrichment;

#[path = "../src/extract/interrupts.rs"]
mod interrupts;

#[path = "../src/mir_repr.rs"]
mod mir_repr;

#[path = "../src/extract/mir_extract.rs"]
mod mir_extract;

#[path = "../src/extract/body.rs"]
mod body;

use identity_enrichment::*;
use interrupts::*;
use mir_repr::*;

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

struct IsrExtractionResult {
    interrupt: InterruptSurface,
    identity: IdentityEnrichmentSurface,
    body: MirBodySurface,
}

fn compile_and_extract(source: &str, test_name: &str) -> IsrExtractionResult {
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

    type IntResult = Result<InterruptSurface, InterruptError>;
    type IdResult = Result<IdentityEnrichmentSurface, IdentityEnrichmentError>;
    type BodyResult = Result<MirBodySurface, body::BodyError>;

    let int_result: Arc<Mutex<Option<IntResult>>> = Arc::new(Mutex::new(None));
    let id_result: Arc<Mutex<Option<IdResult>>> = Arc::new(Mutex::new(None));
    let body_result: Arc<Mutex<Option<BodyResult>>> = Arc::new(Mutex::new(None));

    struct ExtractCallbacks {
        int_result: Arc<Mutex<Option<IntResult>>>,
        id_result: Arc<Mutex<Option<IdResult>>>,
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
            *self.int_result.lock().unwrap() = Some(interrupts::extract_interrupt_surface(tcx));
            *self.id_result.lock().unwrap() =
                Some(identity_enrichment::extract_identity_enrichment(tcx));
            *self.body_result.lock().unwrap() = Some(body::extract_body_surface(tcx));
            rustc_driver::Compilation::Stop
        }
    }

    let mut callbacks = ExtractCallbacks {
        int_result: int_result.clone(),
        id_result: id_result.clone(),
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

    let interrupt = int_result
        .lock()
        .unwrap()
        .take()
        .expect("after_analysis should have been called")
        .expect("interrupt extraction should succeed");

    let identity = id_result
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

    IsrExtractionResult {
        interrupt,
        identity,
        body,
    }
}

// ---------------------------------------------------------------------------
// Inline specimen: reduced PL011 ISR surface
// ---------------------------------------------------------------------------

/// Reduced specimen covering all authored ISR facts for handle_irq:
/// - `#[rseq::isr]` annotation on `handle_irq`
/// - `&mut self` receiver, `-> IrqResult` return type
/// - reads raw MIS once into `status`
/// - early `IrqResult::NotHandled` return for `status == 0`
/// - `handled = false` initialization
/// - RX branch: `status & RX_INTERRUPT_MASK (= 0x50) != 0`
/// - TX branch: `status & TX_INTERRUPT_MASK (= 0x20) != 0`
/// - RX: `handled |= self.service_rx_fifo()`
/// - TX: `handled |= self.service_tx_fifo()`
/// - calls `clear_explicit_interrupts(status)` after RX/TX
/// - final: `handled || (status & EXPLICIT_ISR_CLEAR_MASK (= 0x0780) != 0)`
/// - true -> `IrqResult::Handled`, false -> `IrqResult::NotHandled`
const SPECIMEN: &str = r#"
    #![allow(dead_code)]

    pub struct Interrupt;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum IrqResult {
        Handled,
        NotHandled,
    }

    pub struct Mmio<T> { inner: T }

    pub struct Register { _opaque: () }

    pub struct Pl011Registers {
        pub mis: Register,
        pub icr: Register,
    }

    impl Register {
        pub fn read_raw(&self) -> u32 { 0 }
        pub fn write_raw(&mut self, _val: u32) {}
    }

    impl<T> core::ops::Deref for Mmio<T> {
        type Target = T;
        fn deref(&self) -> &T { &self.inner }
    }

    impl<T> core::ops::DerefMut for Mmio<T> {
        fn deref_mut(&mut self) -> &mut T { &mut self.inner }
    }

    #[rseq::driver(name = "pl011-full-spec", compatible = ["arm,pl011"])]
    pub struct Pl011FullSpec {
        #[rseq::mmio(id = "uart_regs")]
        pub regs: Mmio<Pl011Registers>,

        #[rseq::interrupt(id = "uart_irq")]
        pub irq: Interrupt,
    }

    impl Pl011FullSpec {
        const RX_INTERRUPT_MASK: u32 = (1 << 4) | (1 << 6);
        const TX_INTERRUPT_MASK: u32 = 1 << 5;
        const ERROR_INTERRUPT_MASK: u32 = (1 << 7) | (1 << 8) | (1 << 9) | (1 << 10);
        const EXPLICIT_ISR_CLEAR_MASK: u32 = Self::ERROR_INTERRUPT_MASK;

        fn service_rx_fifo(&mut self) -> bool {
            false
        }

        fn service_tx_fifo(&mut self) -> bool {
            false
        }

        fn clear_explicit_interrupts(&mut self, _status: u32) {
        }
    }

    impl Pl011FullSpec {
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
// Precondition: specimen fidelity against real fixture source
// ---------------------------------------------------------------------------

#[test]
fn precondition_specimen_fidelity() {
    let fixture = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/pl011-full-spec/src/driver.rs"
    ))
    .expect("fixture source must be readable");

    // Fact 1: handle_irq has #[rseq::isr] annotation
    assert!(
        fixture.contains("#[rseq::isr]"),
        "fixture must contain `#[rseq::isr]`"
    );

    // Fact 2: handle_irq signature with &mut self and IrqResult return
    assert!(
        fixture.contains("pub fn handle_irq(&mut self) -> IrqResult"),
        "fixture must contain `pub fn handle_irq(&mut self) -> IrqResult`"
    );

    // Fact 3: reads raw MIS once into status
    assert!(
        fixture.contains("let status = self.regs.mis.read_raw();"),
        "fixture must contain `let status = self.regs.mis.read_raw();`"
    );

    // Fact 4: early NotHandled return for status == 0
    assert!(
        fixture.contains("if status == 0 {"),
        "fixture must contain `if status == 0 {{`"
    );
    assert!(
        fixture.contains("return IrqResult::NotHandled;"),
        "fixture must contain `return IrqResult::NotHandled;`"
    );

    // Fact 5: handled = false initialization
    assert!(
        fixture.contains("let mut handled = false;"),
        "fixture must contain `let mut handled = false;`"
    );

    // Fact 6: RX branch condition
    assert!(
        fixture.contains("if status & Self::RX_INTERRUPT_MASK != 0 {"),
        "fixture must contain RX branch condition"
    );

    // Fact 7: TX branch condition
    assert!(
        fixture.contains("if status & Self::TX_INTERRUPT_MASK != 0 {"),
        "fixture must contain TX branch condition"
    );

    // Fact 8: RX helper dispatch
    assert!(
        fixture.contains("handled |= self.service_rx_fifo();"),
        "fixture must contain `handled |= self.service_rx_fifo();`"
    );

    // Fact 9: TX helper dispatch
    assert!(
        fixture.contains("handled |= self.service_tx_fifo();"),
        "fixture must contain `handled |= self.service_tx_fifo();`"
    );

    // Fact 10: clear_explicit_interrupts call
    assert!(
        fixture.contains("self.clear_explicit_interrupts(status);"),
        "fixture must contain `self.clear_explicit_interrupts(status);`"
    );

    // Fact 11: final handled condition
    assert!(
        fixture.contains("if handled || (status & Self::EXPLICIT_ISR_CLEAR_MASK != 0) {"),
        "fixture must contain final handled condition"
    );

    // Fact 12: final true branch returns Handled
    assert!(
        fixture.contains("IrqResult::Handled"),
        "fixture must contain `IrqResult::Handled`"
    );

    // Fact 13: final false branch returns NotHandled
    // (already checked above via return IrqResult::NotHandled)

    // Fact 14: RX_INTERRUPT_MASK = 0x50
    assert!(
        fixture.contains("const RX_INTERRUPT_MASK: u32 = (1 << 4) | (1 << 6);"),
        "fixture must contain RX_INTERRUPT_MASK definition"
    );

    // Fact 15: TX_INTERRUPT_MASK = 0x20
    assert!(
        fixture.contains("const TX_INTERRUPT_MASK: u32 = 1 << 5;"),
        "fixture must contain TX_INTERRUPT_MASK definition"
    );

    // Fact 16: EXPLICIT_ISR_CLEAR_MASK = ERROR_INTERRUPT_MASK = 0x0780
    assert!(
        fixture.contains("const EXPLICIT_ISR_CLEAR_MASK: u32 = Self::ERROR_INTERRUPT_MASK;"),
        "fixture must contain EXPLICIT_ISR_CLEAR_MASK definition"
    );
    assert!(
        fixture.contains(
            "const ERROR_INTERRUPT_MASK: u32 = (1 << 7) | (1 << 8) | (1 << 9) | (1 << 10);"
        ),
        "fixture must contain ERROR_INTERRUPT_MASK definition"
    );

    // Verify the specimen reproduces these same authored facts
    assert!(
        SPECIMEN.contains("#[rseq::isr]"),
        "specimen must contain `#[rseq::isr]`"
    );
    assert!(
        SPECIMEN.contains("pub fn handle_irq(&mut self) -> IrqResult"),
        "specimen must contain handle_irq signature"
    );
    assert!(
        SPECIMEN.contains("let status = self.regs.mis.read_raw();"),
        "specimen must contain MIS read"
    );
    assert!(
        SPECIMEN.contains("if status == 0 {"),
        "specimen must contain early zero check"
    );
    assert!(
        SPECIMEN.contains("return IrqResult::NotHandled;"),
        "specimen must contain early NotHandled return"
    );
    assert!(
        SPECIMEN.contains("let mut handled = false;"),
        "specimen must contain handled initialization"
    );
    assert!(
        SPECIMEN.contains("if status & Self::RX_INTERRUPT_MASK != 0 {"),
        "specimen must contain RX branch condition"
    );
    assert!(
        SPECIMEN.contains("if status & Self::TX_INTERRUPT_MASK != 0 {"),
        "specimen must contain TX branch condition"
    );
    assert!(
        SPECIMEN.contains("handled |= self.service_rx_fifo();"),
        "specimen must contain RX helper dispatch"
    );
    assert!(
        SPECIMEN.contains("handled |= self.service_tx_fifo();"),
        "specimen must contain TX helper dispatch"
    );
    assert!(
        SPECIMEN.contains("self.clear_explicit_interrupts(status);"),
        "specimen must contain clear call"
    );
    assert!(
        SPECIMEN.contains("if handled || (status & Self::EXPLICIT_ISR_CLEAR_MASK != 0) {"),
        "specimen must contain final handled condition"
    );
    assert!(
        SPECIMEN.contains("const RX_INTERRUPT_MASK: u32 = (1 << 4) | (1 << 6);"),
        "specimen must contain RX_INTERRUPT_MASK constant"
    );
    assert!(
        SPECIMEN.contains("const TX_INTERRUPT_MASK: u32 = 1 << 5;"),
        "specimen must contain TX_INTERRUPT_MASK constant"
    );
    assert!(
        SPECIMEN.contains("const EXPLICIT_ISR_CLEAR_MASK: u32 = Self::ERROR_INTERRUPT_MASK;"),
        "specimen must contain EXPLICIT_ISR_CLEAR_MASK constant"
    );
}

// ---------------------------------------------------------------------------
// Check Group 1: ISR identity, annotation, signature, return type survive
// ---------------------------------------------------------------------------

/// Check 1a: ISR binding surface preserves handle_irq as an ISR with
/// Auto bind target (bare #[rseq::isr]) and IrqResult return type.
#[test]
fn check1a_isr_binding_identity() {
    let result = compile_and_extract(SPECIMEN, "isr_check1a");

    // ISR binding must exist
    assert_eq!(
        result.interrupt.isr_bindings.len(),
        1,
        "exactly one ISR binding expected"
    );
    let binding = &result.interrupt.isr_bindings[0];

    // handler_name preserves authored identity
    assert_eq!(
        binding.handler_name, "handle_irq",
        "ISR handler name must be 'handle_irq'"
    );

    // bare #[rseq::isr] produces Auto bind target
    assert_eq!(
        binding.bind_target,
        IsrBindTarget::Auto,
        "bare #[rseq::isr] must produce Auto bind target"
    );

    // no index parameter
    assert!(
        !binding.has_index_param,
        "handle_irq must not have an index parameter"
    );
}

/// Check 1b: Identity enrichment surface preserves handle_irq as a method
/// with is_isr = true in an inherent impl block for Pl011FullSpec.
#[test]
fn check1b_identity_enrichment_isr_marker() {
    let result = compile_and_extract(SPECIMEN, "isr_check1b");

    // Find inherent impl blocks for Pl011FullSpec
    let pl011_inherent_impls: Vec<_> = result
        .identity
        .impl_blocks
        .iter()
        .filter(|ib| ib.self_type_name == "Pl011FullSpec" && ib.is_inherent)
        .collect();

    assert!(
        !pl011_inherent_impls.is_empty(),
        "at least one inherent impl for Pl011FullSpec must exist"
    );

    // Find handle_irq method across all inherent impl blocks
    let handle_irq_method = pl011_inherent_impls
        .iter()
        .flat_map(|ib| ib.methods.iter())
        .find(|m| m.name == "handle_irq");

    assert!(
        handle_irq_method.is_some(),
        "handle_irq method must exist in Pl011FullSpec inherent impls; methods: {:?}",
        pl011_inherent_impls
            .iter()
            .flat_map(|ib| ib.methods.iter().map(|m| &m.name))
            .collect::<Vec<_>>()
    );

    let method = handle_irq_method.unwrap();

    // is_isr must be true
    assert!(
        method.is_isr,
        "handle_irq must have is_isr = true in identity enrichment"
    );

    // canonical path must contain handle_irq
    assert!(
        method.canonical_path.0.contains("handle_irq"),
        "handle_irq canonical path must contain 'handle_irq', got: {}",
        method.canonical_path.0
    );
}

/// Check 1c: MIR body surface preserves handle_irq as an ISR with
/// &mut self receiver and IrqResult return type.
#[test]
fn check1c_mir_isr_identity_and_signature() {
    let result = compile_and_extract(SPECIMEN, "isr_check1c");

    let handle_irq = result
        .body
        .functions
        .iter()
        .find(|f| f.name == "handle_irq");

    assert!(
        handle_irq.is_some(),
        "handle_irq must be present in MIR body surface"
    );

    let handle_irq = handle_irq.unwrap();

    // Kind must be Isr
    assert_eq!(
        handle_irq.kind,
        MirFnKind::Isr,
        "handle_irq must be classified as Isr"
    );

    // Receiver must be ref_mut (&mut self)
    assert_eq!(
        handle_irq.receiver, "ref_mut",
        "handle_irq receiver must be 'ref_mut'"
    );

    // Return type must be IrqResult
    assert_eq!(
        handle_irq.return_type.as_deref(),
        Some("IrqResult"),
        "handle_irq return type must be 'IrqResult'"
    );

    // No extra parameters
    assert!(
        handle_irq.params.is_empty(),
        "handle_irq must have no parameters (beyond self)"
    );
}

// ---------------------------------------------------------------------------
// Check Group 2: ISR body preservation
// ---------------------------------------------------------------------------

/// Check 2a: handle_irq MIR body calls service_rx_fifo, service_tx_fifo,
/// and clear_explicit_interrupts as helper dispatches.
#[test]
fn check2a_helper_dispatch() {
    let result = compile_and_extract(SPECIMEN, "isr_check2a");

    let handle_irq = result
        .body
        .functions
        .iter()
        .find(|f| f.name == "handle_irq")
        .expect("handle_irq must be present");

    // helper_calls must contain all three helpers
    assert!(
        handle_irq
            .helper_calls
            .contains(&"service_rx_fifo".to_string()),
        "handle_irq must call service_rx_fifo; helper_calls: {:?}",
        handle_irq.helper_calls
    );
    assert!(
        handle_irq
            .helper_calls
            .contains(&"service_tx_fifo".to_string()),
        "handle_irq must call service_tx_fifo; helper_calls: {:?}",
        handle_irq.helper_calls
    );
    assert!(
        handle_irq
            .helper_calls
            .contains(&"clear_explicit_interrupts".to_string()),
        "handle_irq must call clear_explicit_interrupts; helper_calls: {:?}",
        handle_irq.helper_calls
    );
}

/// Check 2b: MIR body preserves the status == 0 early branch (SwitchInt
/// with value 0), RX mask BitAnd with 0x50, and TX mask BitAnd with 0x20.
#[test]
fn check2b_status_read_and_branch_conditions() {
    let result = compile_and_extract(SPECIMEN, "isr_check2b");

    let handle_irq = result
        .body
        .functions
        .iter()
        .find(|f| f.name == "handle_irq")
        .expect("handle_irq must be present");

    let body = &handle_irq.body;

    // Verify presence of SwitchInt for the early status == 0 check.
    // The MIR SwitchInt for `if status == 0` will branch on value 0.
    let has_early_zero_switch = body.basic_blocks.iter().any(|bb| {
        if let Some(ref term) = bb.terminator {
            if let MirTerminatorKind::SwitchInt { ref targets, .. } = term.kind {
                targets.values.contains(&0u128)
            } else {
                false
            }
        } else {
            false
        }
    });
    assert!(
        has_early_zero_switch,
        "MIR must contain a SwitchInt with value 0 for the early status check"
    );

    // Verify BitAnd operations preserve the mask constants.
    // RX_INTERRUPT_MASK = 0x50 and TX_INTERRUPT_MASK = 0x20 appear as
    // scalar constants in BitAnd rvalues. The constant may appear on
    // either the lhs or rhs operand.
    let mut found_rx_mask = false;
    let mut found_tx_mask = false;

    fn operand_has_scalar(op: &MirOperand, expected: u128) -> bool {
        if let MirOperand::Constant {
            constant:
                MirConstant {
                    value: MirConstValue::Scalar { bits, .. },
                    ..
                },
        } = op
        {
            *bits == expected
        } else {
            false
        }
    }

    let mut bitand_info: Vec<String> = Vec::new();

    for bb in &body.basic_blocks {
        for stmt in &bb.statements {
            if let MirStatementKind::Assign {
                rvalue:
                    box MirRvalue::BinaryOp {
                        op: MirBinOp::BitAnd,
                        ref lhs,
                        ref rhs,
                    },
                ..
            } = stmt.kind
            {
                bitand_info.push(format!("BitAnd(lhs={:?}, rhs={:?})", lhs, rhs));
                if operand_has_scalar(lhs, 0x50) || operand_has_scalar(rhs, 0x50) {
                    found_rx_mask = true;
                }
                if operand_has_scalar(lhs, 0x20) || operand_has_scalar(rhs, 0x20) {
                    found_tx_mask = true;
                }
            }
        }
    }

    // If mask constants are not inlined, they may be unevaluated const refs.
    if !found_rx_mask || !found_tx_mask {
        fn has_unevaluated_const(op: &MirOperand, name: &str) -> bool {
            if let MirOperand::Constant {
                constant:
                    MirConstant {
                        value: MirConstValue::Unsupported { ref description },
                        ..
                    },
            } = op
            {
                description.contains(name)
            } else {
                false
            }
        }
        for bb in &body.basic_blocks {
            for stmt in &bb.statements {
                if let MirStatementKind::Assign {
                    rvalue:
                        box MirRvalue::BinaryOp {
                            op: MirBinOp::BitAnd,
                            ref lhs,
                            ref rhs,
                        },
                    ..
                } = stmt.kind
                {
                    if !found_rx_mask
                        && (has_unevaluated_const(lhs, "RX_INTERRUPT_MASK")
                            || has_unevaluated_const(rhs, "RX_INTERRUPT_MASK"))
                    {
                        found_rx_mask = true;
                    }
                    if !found_tx_mask
                        && (has_unevaluated_const(lhs, "TX_INTERRUPT_MASK")
                            || has_unevaluated_const(rhs, "TX_INTERRUPT_MASK"))
                    {
                        found_tx_mask = true;
                    }
                }
            }
        }
    }

    assert!(
        found_rx_mask,
        "MIR must contain BitAnd with RX_INTERRUPT_MASK (0x50 or named ref); \
         BitAnd ops: {:?}",
        bitand_info
    );
    assert!(
        found_tx_mask,
        "MIR must contain BitAnd with TX_INTERRUPT_MASK (0x20 or named ref); \
         BitAnd ops: {:?}",
        bitand_info
    );
}

/// Check 2c: MIR body preserves EXPLICIT_ISR_CLEAR_MASK (0x0780) in a
/// BitAnd operation for the final handled condition.
#[test]
fn check2c_explicit_clear_mask_in_final_condition() {
    let result = compile_and_extract(SPECIMEN, "isr_check2c");

    let handle_irq = result
        .body
        .functions
        .iter()
        .find(|f| f.name == "handle_irq")
        .expect("handle_irq must be present");

    let body = &handle_irq.body;

    // EXPLICIT_ISR_CLEAR_MASK = 0x0780 must appear in a BitAnd.
    // The constant may appear on lhs or rhs, as scalar or unevaluated const.
    fn scalar_match(op: &MirOperand, expected: u128) -> bool {
        if let MirOperand::Constant {
            constant:
                MirConstant {
                    value: MirConstValue::Scalar { bits, .. },
                    ..
                },
        } = op
        {
            *bits == expected
        } else {
            false
        }
    }
    fn unevaluated_match(op: &MirOperand, name: &str) -> bool {
        if let MirOperand::Constant {
            constant:
                MirConstant {
                    value: MirConstValue::Unsupported { ref description },
                    ..
                },
        } = op
        {
            description.contains(name)
        } else {
            false
        }
    }

    let mut bitand_diag: Vec<String> = Vec::new();
    let found_explicit_mask = body.basic_blocks.iter().any(|bb| {
        bb.statements.iter().any(|stmt| {
            if let MirStatementKind::Assign {
                rvalue:
                    box MirRvalue::BinaryOp {
                        op: MirBinOp::BitAnd,
                        ref lhs,
                        ref rhs,
                    },
                ..
            } = stmt.kind
            {
                bitand_diag.push(format!("BitAnd(lhs={:?}, rhs={:?})", lhs, rhs));
                scalar_match(lhs, 0x0780)
                    || scalar_match(rhs, 0x0780)
                    || unevaluated_match(lhs, "EXPLICIT_ISR_CLEAR_MASK")
                    || unevaluated_match(rhs, "EXPLICIT_ISR_CLEAR_MASK")
            } else {
                false
            }
        })
    });

    assert!(
        found_explicit_mask,
        "MIR must contain BitAnd with EXPLICIT_ISR_CLEAR_MASK (0x0780 or named ref); \
         BitAnd ops: {:?}",
        bitand_diag
    );
}

/// Check 2d: MIR body preserves IrqResult::Handled and IrqResult::NotHandled
/// as nominally distinguishable aggregate constructors (distinct variant indices).
#[test]
fn check2d_irq_result_variants_distinguishable() {
    let result = compile_and_extract(SPECIMEN, "isr_check2d");

    let handle_irq = result
        .body
        .functions
        .iter()
        .find(|f| f.name == "handle_irq")
        .expect("handle_irq must be present");

    let body = &handle_irq.body;

    // Collect all Adt aggregate variant indices for IrqResult
    let mut irq_result_variants: Vec<u32> = Vec::new();

    for bb in &body.basic_blocks {
        for stmt in &bb.statements {
            if let MirStatementKind::Assign {
                rvalue:
                    box MirRvalue::Aggregate {
                        agg_kind:
                            MirAggregateKind::Adt {
                                ref path,
                                variant_index,
                                ..
                            },
                        ..
                    },
                ..
            } = stmt.kind
            {
                if path.contains("IrqResult") {
                    irq_result_variants.push(variant_index);
                }
            }
        }
    }

    // Must have at least 2 distinct variant indices (Handled and NotHandled)
    irq_result_variants.sort();
    irq_result_variants.dedup();

    assert!(
        irq_result_variants.len() >= 2,
        "IrqResult must have at least 2 distinct variant indices in MIR \
         (Handled and NotHandled); found: {:?}",
        irq_result_variants
    );
}

/// Check 2e: MIR body preserves BitOr operations for `handled |= ...` pattern.
/// The handled accumulation via `|=` must produce BitOr in MIR.
#[test]
fn check2e_handled_accumulation_via_bitor() {
    let result = compile_and_extract(SPECIMEN, "isr_check2e");

    let handle_irq = result
        .body
        .functions
        .iter()
        .find(|f| f.name == "handle_irq")
        .expect("handle_irq must be present");

    let body = &handle_irq.body;

    let has_bitor = body.basic_blocks.iter().any(|bb| {
        bb.statements.iter().any(|stmt| {
            matches!(
                stmt.kind,
                MirStatementKind::Assign {
                    rvalue: box MirRvalue::BinaryOp {
                        op: MirBinOp::BitOr,
                        ..
                    },
                    ..
                }
            )
        })
    });

    assert!(
        has_bitor,
        "MIR must contain BitOr for `handled |= ...` accumulation"
    );
}

/// Check 2f: MIR body contains Return terminators, proving that the ISR
/// has explicit return paths (both Handled and NotHandled branches return).
#[test]
fn check2f_return_terminators_present() {
    let result = compile_and_extract(SPECIMEN, "isr_check2f");

    let handle_irq = result
        .body
        .functions
        .iter()
        .find(|f| f.name == "handle_irq")
        .expect("handle_irq must be present");

    let return_count = handle_irq
        .body
        .basic_blocks
        .iter()
        .filter(|bb| {
            bb.terminator
                .as_ref()
                .map(|t| matches!(t.kind, MirTerminatorKind::Return))
                .unwrap_or(false)
        })
        .count();

    assert!(
        return_count >= 1,
        "MIR must contain at least one Return terminator"
    );
}

/// Check 2g: MIR body preserves the `handled` local as a bool type.
#[test]
fn check2g_handled_local_is_bool() {
    let result = compile_and_extract(SPECIMEN, "isr_check2g");

    let handle_irq = result
        .body
        .functions
        .iter()
        .find(|f| f.name == "handle_irq")
        .expect("handle_irq must be present");

    // At least one local must be bool (the `handled` variable)
    let has_bool_local = handle_irq
        .body
        .local_decls
        .iter()
        .any(|decl| matches!(decl.ty, MirTy::Bool));

    assert!(
        has_bool_local,
        "MIR must contain a bool local declaration for `handled`"
    );
}

/// Check 2h: The read_raw call (MIS status read) is present in MIR as
/// a Call terminator referencing read_raw.
#[test]
fn check2h_status_read_call_present() {
    let result = compile_and_extract(SPECIMEN, "isr_check2h");

    let handle_irq = result
        .body
        .functions
        .iter()
        .find(|f| f.name == "handle_irq")
        .expect("handle_irq must be present");

    let has_read_raw_call = handle_irq.body.basic_blocks.iter().any(|bb| {
        if let Some(ref term) = bb.terminator {
            if let MirTerminatorKind::Call { ref func, .. } = term.kind {
                if let MirOperand::Constant {
                    constant: MirConstant { ref ty, .. },
                } = func
                {
                    match ty {
                        MirTy::FnDef { ref path, .. } => path.contains("read_raw"),
                        _ => false,
                    }
                } else {
                    false
                }
            } else {
                false
            }
        } else {
            false
        }
    });

    assert!(
        has_read_raw_call,
        "MIR must contain a Call terminator referencing read_raw for the MIS status read"
    );
}

/// Check 2i: helpers service_rx_fifo, service_tx_fifo, and
/// clear_explicit_interrupts are extracted as Helper functions in MIR body surface.
#[test]
fn check2i_helpers_extracted_as_functions() {
    let result = compile_and_extract(SPECIMEN, "isr_check2i");

    let helper_names = [
        "service_rx_fifo",
        "service_tx_fifo",
        "clear_explicit_interrupts",
    ];

    for name in &helper_names {
        let helper = result.body.functions.iter().find(|f| f.name == *name);
        assert!(
            helper.is_some(),
            "helper function '{}' must be present in MIR body surface; \
             found: {:?}",
            name,
            result
                .body
                .functions
                .iter()
                .map(|f| &f.name)
                .collect::<Vec<_>>()
        );

        let helper = helper.unwrap();
        assert_eq!(
            helper.kind,
            MirFnKind::Helper,
            "helper '{}' must be classified as Helper",
            name
        );
    }
}
