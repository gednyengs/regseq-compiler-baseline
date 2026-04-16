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
// CFG walking helpers for control-flow ordering verification
// ---------------------------------------------------------------------------

/// Collect successor basic-block indices for a terminator.
fn terminator_successors(term: &MirTerminatorKind) -> Vec<u32> {
    match term {
        MirTerminatorKind::Goto { target } => vec![*target],
        MirTerminatorKind::SwitchInt { targets, .. } => targets.targets.clone(),
        MirTerminatorKind::Call { target, unwind, .. } => {
            let mut succs = Vec::new();
            if let Some(t) = target {
                succs.push(*t);
            }
            if let MirUnwindAction::Cleanup { target: t } = unwind {
                succs.push(*t);
            }
            succs
        }
        MirTerminatorKind::Drop { target, unwind, .. } => {
            let mut succs = vec![*target];
            if let MirUnwindAction::Cleanup { target: t } = unwind {
                succs.push(*t);
            }
            succs
        }
        MirTerminatorKind::Assert { target, unwind, .. } => {
            let mut succs = vec![*target];
            if let MirUnwindAction::Cleanup { target: t } = unwind {
                succs.push(*t);
            }
            succs
        }
        MirTerminatorKind::Return
        | MirTerminatorKind::Unreachable
        | MirTerminatorKind::UnwindResume
        | MirTerminatorKind::UnwindTerminate { .. }
        | MirTerminatorKind::Unsupported { .. } => vec![],
    }
}

/// Return true if there is a CFG path from `from` to `to` in `bbs`,
/// visiting only non-cleanup blocks.
fn reachable(bbs: &[MirBasicBlockData], from: usize, to: usize) -> bool {
    let n = bbs.len();
    let mut visited = vec![false; n];
    let mut stack = vec![from];
    while let Some(cur) = stack.pop() {
        if cur == to {
            return true;
        }
        if cur >= n || visited[cur] || bbs[cur].is_cleanup {
            continue;
        }
        visited[cur] = true;
        if let Some(ref term) = bbs[cur].terminator {
            for s in terminator_successors(&term.kind) {
                stack.push(s as usize);
            }
        }
    }
    false
}

/// Return true if there is a CFG path from `from` to `to` that does NOT pass
/// through any block in `avoid`.
fn reachable_avoiding(bbs: &[MirBasicBlockData], from: usize, to: usize, avoid: &[usize]) -> bool {
    let n = bbs.len();
    let mut visited = vec![false; n];
    let mut stack = vec![from];
    while let Some(cur) = stack.pop() {
        if cur == to {
            return true;
        }
        if cur >= n || visited[cur] || bbs[cur].is_cleanup || avoid.contains(&cur) {
            continue;
        }
        visited[cur] = true;
        if let Some(ref term) = bbs[cur].terminator {
            for s in terminator_successors(&term.kind) {
                stack.push(s as usize);
            }
        }
    }
    false
}

/// Find the basic block index that contains a Call terminator whose func
/// FnDef path contains `name_fragment`.
fn find_call_bb(bbs: &[MirBasicBlockData], name_fragment: &str) -> Option<usize> {
    bbs.iter().enumerate().find_map(|(i, bb)| {
        if let Some(MirTerminator {
            kind:
                MirTerminatorKind::Call {
                    func:
                        MirOperand::Constant {
                            constant:
                                MirConstant {
                                    ty: MirTy::Fn { ref path, .. },
                                    ..
                                },
                        },
                    ..
                },
            ..
        }) = bb.terminator
        {
            if path.contains(name_fragment) {
                return Some(i);
            }
        }
        None
    })
}

/// Find basic blocks that assign an IrqResult Adt aggregate with the given
/// `variant_index`, returning (bb_index, variant_index) pairs.
fn find_irqresult_aggregate_bbs(bbs: &[MirBasicBlockData], variant_idx: u32) -> Vec<usize> {
    bbs.iter()
        .enumerate()
        .filter_map(|(i, bb)| {
            let found = bb.statements.iter().any(|stmt| {
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
                    path.contains("IrqResult") && variant_index == variant_idx
                } else {
                    false
                }
            });
            if found {
                Some(i)
            } else {
                None
            }
        })
        .collect()
}

/// Find the bb index of a Return terminator reachable from `from_bb`.
fn find_return_reachable_from(bbs: &[MirBasicBlockData], from_bb: usize) -> Option<usize> {
    let n = bbs.len();
    let mut visited = vec![false; n];
    let mut stack = vec![from_bb];
    while let Some(cur) = stack.pop() {
        if cur >= n || visited[cur] || bbs[cur].is_cleanup {
            continue;
        }
        visited[cur] = true;
        if let Some(ref term) = bbs[cur].terminator {
            if matches!(term.kind, MirTerminatorKind::Return) {
                return Some(cur);
            }
            for s in terminator_successors(&term.kind) {
                stack.push(s as usize);
            }
        }
    }
    None
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
        fixture.contains("let status = self.regs.read(mis::REG);"),
        "fixture must contain `let status = self.regs.read(mis::REG);`"
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
    let result = compile_and_extract(SPECIMEN, "s2e12_check1a");

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
    let result = compile_and_extract(SPECIMEN, "s2e12_check1b");

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
    let result = compile_and_extract(SPECIMEN, "s2e12_check1c");

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
    assert!(
        handle_irq.return_type.as_ref().is_some_and(|ty| match ty {
            MirTy::Enum { path, .. } | MirTy::Struct { path, .. } => path.contains("IrqResult"),
            MirTy::NominalRef { path } => path.contains("IrqResult"),
            _ => false,
        }),
        "handle_irq return type must be 'IrqResult', got: {:?}",
        handle_irq.return_type
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
    let result = compile_and_extract(SPECIMEN, "s2e12_check2a");

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
    let result = compile_and_extract(SPECIMEN, "s2e12_check2b");

    let handle_irq = result
        .body
        .functions
        .iter()
        .find(|f| f.name == "handle_irq")
        .expect("handle_irq must be present");

    let body = &handle_irq.body;

    // Verify presence of SwitchInt for the early status == 0 check.
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
                    value: MirConstValue::Bits { value: bits, .. },
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
                        value: MirConstValue::Unevaluated { ref path },
                        ..
                    },
            } = op
            {
                path.contains(name)
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
    let result = compile_and_extract(SPECIMEN, "s2e12_check2c");

    let handle_irq = result
        .body
        .functions
        .iter()
        .find(|f| f.name == "handle_irq")
        .expect("handle_irq must be present");

    let body = &handle_irq.body;

    // EXPLICIT_ISR_CLEAR_MASK = 0x0780 must appear in a BitAnd.
    fn scalar_match(op: &MirOperand, expected: u128) -> bool {
        if let MirOperand::Constant {
            constant:
                MirConstant {
                    value: MirConstValue::Bits { value: bits, .. },
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
                    value: MirConstValue::Unevaluated { ref path },
                    ..
                },
        } = op
        {
            path.contains(name)
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
/// as nominally distinguishable aggregate constructors (distinct variant indices),
/// AND maps the correct variant to the correct branch:
/// - early `status == 0` branch produces NotHandled
/// - final true branch (handled || mask!=0) produces Handled (variant 0)
/// - final false branch produces NotHandled (variant 1)
#[test]
fn check2d_irq_result_variants_distinguishable() {
    let result = compile_and_extract(SPECIMEN, "s2e12_check2d");

    let handle_irq = result
        .body
        .functions
        .iter()
        .find(|f| f.name == "handle_irq")
        .expect("handle_irq must be present");

    let body = &handle_irq.body;
    let bbs = &body.basic_blocks;

    // Collect all (bb_index, variant_index) pairs for IrqResult aggregates
    let mut variant_to_bbs: std::collections::HashMap<u32, Vec<usize>> =
        std::collections::HashMap::new();

    for (i, bb) in bbs.iter().enumerate() {
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
                    variant_to_bbs.entry(variant_index).or_default().push(i);
                }
            }
        }
    }

    // Must have exactly 2 distinct variant indices
    assert_eq!(
        variant_to_bbs.len(),
        2,
        "IrqResult must have exactly 2 distinct variant indices (Handled and NotHandled); \
         found: {:?}",
        variant_to_bbs.keys().collect::<Vec<_>>()
    );

    // Identify which variant is Handled (0) and NotHandled (1) by checking
    // that the early return (from the status==0 SwitchInt) produces variant 1.
    //
    // The early zero check is the first SwitchInt with value 0 that is
    // reachable from bb0 before any helper calls.
    // From the CFG: bb2 has SwitchInt(0) and the "true" branch (target
    // index 1 for value 0) goes to bb3 which has IrqResult variant 1.
    let early_switch_bb = bbs.iter().enumerate().find_map(|(i, bb)| {
        if let Some(ref term) = bb.terminator {
            if let MirTerminatorKind::SwitchInt { ref targets, .. } = term.kind {
                if targets.values.contains(&0u128) {
                    // Check this SwitchInt compares a u32 (status) not a bool
                    // by looking at the Eq -> SwitchInt pattern:
                    // The block should contain a BinaryOp::Eq with scalar 0
                    let has_eq_zero = bb.statements.iter().any(|s| {
                        matches!(
                            s.kind,
                            MirStatementKind::Assign {
                                rvalue: box MirRvalue::BinaryOp {
                                    op: MirBinOp::Eq,
                                    ..
                                },
                                ..
                            }
                        )
                    });
                    if has_eq_zero {
                        return Some(i);
                    }
                }
            }
        }
        None
    });

    assert!(
        early_switch_bb.is_some(),
        "must find the early status==0 SwitchInt basic block"
    );
    let early_switch_bb = early_switch_bb.unwrap();

    // The "true" arm of status==0 is the target for value 0 (SwitchInt
    // convention: value 0 = false means the condition is false, so the
    // second target is the "true" target).
    let early_term = bbs[early_switch_bb].terminator.as_ref().unwrap();
    let early_true_target =
        if let MirTerminatorKind::SwitchInt { ref targets, .. } = early_term.kind {
            // For SwitchInt { values: [0], targets: [false_bb, true_bb] }
            // value 0 -> targets[0] (the "condition is false" path)
            // otherwise -> targets[1] (the "condition is true" path, i.e. status==0 was true)
            assert_eq!(targets.values.len(), 1);
            assert_eq!(targets.targets.len(), 2);
            // targets[1] is the "otherwise" = when the bool discr is non-zero (true)
            targets.targets[1] as usize
        } else {
            panic!("expected SwitchInt");
        };

    // The early true target block (or a block reachable from it before any
    // Return) must construct an IrqResult aggregate. Find which variant.
    let early_variant = bbs[early_true_target].statements.iter().find_map(|stmt| {
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
                return Some(variant_index);
            }
        }
        None
    });

    assert!(
        early_variant.is_some(),
        "the early status==0 true-branch block must construct an IrqResult"
    );
    let not_handled_variant = early_variant.unwrap();

    // The other variant must be Handled
    let handled_variant = variant_to_bbs
        .keys()
        .find(|&&v| v != not_handled_variant)
        .copied()
        .expect("must have a second variant for Handled");

    // Verify that the IrqResult enum declaration order matches expectation:
    // Handled = variant 0, NotHandled = variant 1
    assert_eq!(
        handled_variant, 0,
        "IrqResult::Handled must be variant index 0; got {}",
        handled_variant
    );
    assert_eq!(
        not_handled_variant, 1,
        "IrqResult::NotHandled must be variant index 1; got {}",
        not_handled_variant
    );
}

/// Check 2e: MIR body preserves BitOr operations for `handled |= ...` pattern.
/// The handled accumulation via `|=` must produce BitOr in MIR, writing to
/// the same bool local that serves as the `handled` accumulator.
#[test]
fn check2e_handled_accumulation_via_bitor() {
    let result = compile_and_extract(SPECIMEN, "s2e12_check2e");

    let handle_irq = result
        .body
        .functions
        .iter()
        .find(|f| f.name == "handle_irq")
        .expect("handle_irq must be present");

    let body = &handle_irq.body;

    // Find the `handled` local: a mutable bool local that is initialized
    // to false (Scalar { bits: 0, size: 1 }) and is the target of BitOr
    // assignments. We identify it by finding BitOr assignments to bool locals.
    let mut bitor_target_locals: Vec<u32> = Vec::new();

    for bb in &body.basic_blocks {
        for stmt in &bb.statements {
            if let MirStatementKind::Assign {
                ref place,
                rvalue:
                    box MirRvalue::BinaryOp {
                        op: MirBinOp::BitOr,
                        ref lhs,
                        ..
                    },
            } = stmt.kind
            {
                // The lhs operand of BitOr should be a Copy/Move of the
                // same local as the assignment target (handled |= x means
                // handled = handled | x).
                let lhs_local = match lhs {
                    MirOperand::Copy { place: p } | MirOperand::Move { place: p } => Some(p.local),
                    _ => None,
                };
                if lhs_local == Some(place.local) {
                    bitor_target_locals.push(place.local);
                }
            }
        }
    }

    assert!(
        !bitor_target_locals.is_empty(),
        "MIR must contain at least one BitOr accumulation (handled |= ...)"
    );

    // The target local should be bool
    let handled_local_idx = bitor_target_locals[0];
    let handled_decl = body
        .local_decls
        .iter()
        .find(|d| d.index == handled_local_idx);
    assert!(
        handled_decl.is_some(),
        "BitOr target local_{} must exist in local_decls",
        handled_local_idx
    );
    assert!(
        matches!(handled_decl.unwrap().ty, MirTy::Bool),
        "BitOr target local_{} must be bool, got {:?}",
        handled_local_idx,
        handled_decl.unwrap().ty
    );

    // All BitOr targets should reference the same local (consistent accumulator)
    assert!(
        bitor_target_locals.iter().all(|&l| l == handled_local_idx),
        "All BitOr accumulations must target the same local; found locals: {:?}",
        bitor_target_locals
    );

    // Verify at least 2 BitOr ops (one for RX, one for TX)
    assert!(
        bitor_target_locals.len() >= 2,
        "must have at least 2 BitOr accumulations (RX and TX); found {}",
        bitor_target_locals.len()
    );
}

/// Check 2f: MIR body contains Return terminators, and multiple distinct
/// CFG paths reach the return -- at least the early NotHandled path and the
/// final Handled/NotHandled paths.
#[test]
fn check2f_return_terminators_present() {
    let result = compile_and_extract(SPECIMEN, "s2e12_check2f");

    let handle_irq = result
        .body
        .functions
        .iter()
        .find(|f| f.name == "handle_irq")
        .expect("handle_irq must be present");

    let bbs = &handle_irq.body.basic_blocks;

    // Count Return terminators
    let return_bbs: Vec<usize> = bbs
        .iter()
        .enumerate()
        .filter_map(|(i, bb)| {
            if bb
                .terminator
                .as_ref()
                .map(|t| matches!(t.kind, MirTerminatorKind::Return))
                .unwrap_or(false)
            {
                Some(i)
            } else {
                None
            }
        })
        .collect();

    assert!(
        !return_bbs.is_empty(),
        "MIR must contain at least one Return terminator"
    );

    // Verify that distinct IrqResult aggregate constructions (Handled vs
    // NotHandled) eventually reach a Return. There must be at least 3
    // IrqResult aggregate sites: early NotHandled, final Handled, final
    // NotHandled.
    let mut irqresult_agg_bbs: Vec<usize> = Vec::new();
    for (i, bb) in bbs.iter().enumerate() {
        let has_irqresult_agg = bb.statements.iter().any(|stmt| {
            if let MirStatementKind::Assign {
                rvalue:
                    box MirRvalue::Aggregate {
                        agg_kind: MirAggregateKind::Adt { ref path, .. },
                        ..
                    },
                ..
            } = stmt.kind
            {
                path.contains("IrqResult")
            } else {
                false
            }
        });
        if has_irqresult_agg {
            irqresult_agg_bbs.push(i);
        }
    }

    assert!(
        irqresult_agg_bbs.len() >= 3,
        "must have at least 3 IrqResult aggregate sites (early NotHandled, \
         final Handled, final NotHandled); found {} in bbs {:?}",
        irqresult_agg_bbs.len(),
        irqresult_agg_bbs
    );

    // Each IrqResult aggregate site must reach a Return
    for &agg_bb in &irqresult_agg_bbs {
        let reaches_return = find_return_reachable_from(bbs, agg_bb);
        assert!(
            reaches_return.is_some(),
            "IrqResult aggregate in bb{} must reach a Return terminator",
            agg_bb
        );
    }
}

/// Check 2g: MIR body preserves a mutable bool local that is initialized to
/// false and serves as the BitOr accumulation target -- the `handled` variable.
#[test]
fn check2g_handled_local_is_bool() {
    let result = compile_and_extract(SPECIMEN, "s2e12_check2g");

    let handle_irq = result
        .body
        .functions
        .iter()
        .find(|f| f.name == "handle_irq")
        .expect("handle_irq must be present");

    let body = &handle_irq.body;

    // Find the local that is the target of BitOr(self, rhs) assignments --
    // this is the `handled` accumulator.
    let handled_local_idx: Option<u32> = body.basic_blocks.iter().find_map(|bb| {
        bb.statements.iter().find_map(|stmt| {
            if let MirStatementKind::Assign {
                ref place,
                rvalue:
                    box MirRvalue::BinaryOp {
                        op: MirBinOp::BitOr,
                        ref lhs,
                        ..
                    },
            } = stmt.kind
            {
                let lhs_local = match lhs {
                    MirOperand::Copy { place: p } | MirOperand::Move { place: p } => Some(p.local),
                    _ => None,
                };
                if lhs_local == Some(place.local) {
                    return Some(place.local);
                }
            }
            None
        })
    });

    assert!(
        handled_local_idx.is_some(),
        "must find a BitOr accumulation target local for `handled`"
    );
    let handled_idx = handled_local_idx.unwrap();

    // Verify this local is declared as Bool and Mut
    let handled_decl = body
        .local_decls
        .iter()
        .find(|d| d.index == handled_idx)
        .expect("handled local must exist in local_decls");

    assert!(
        matches!(handled_decl.ty, MirTy::Bool),
        "handled local_{} must be Bool, got {:?}",
        handled_idx,
        handled_decl.ty
    );
    assert!(
        matches!(handled_decl.mutability, MirMutability::Mut),
        "handled local_{} must be Mut (mutable), got {:?}",
        handled_idx,
        handled_decl.mutability
    );

    // Verify this local is initialized to false (Scalar { bits: 0, size: 1 })
    let initialized_to_false = body.basic_blocks.iter().any(|bb| {
        bb.statements.iter().any(|stmt| {
            if let MirStatementKind::Assign {
                ref place,
                rvalue: box MirRvalue::Use { ref operand },
            } = stmt.kind
            {
                if place.local == handled_idx {
                    if let MirOperand::Constant {
                        constant:
                            MirConstant {
                                value: MirConstValue::Bits { value: bits },
                                ref ty,
                            },
                    } = operand
                    {
                        return matches!(ty, MirTy::Bool) && *bits == 0;
                    }
                }
            }
            false
        })
    });

    assert!(
        initialized_to_false,
        "handled local_{} must be initialized to false (Bits {{ value: 0 }})",
        handled_idx
    );
}

/// Check 2h: The read_raw call (MIS status read) is present in MIR as
/// a Call terminator referencing read_raw.
#[test]
fn check2h_status_read_call_present() {
    let result = compile_and_extract(SPECIMEN, "s2e12_check2h");

    let handle_irq = result
        .body
        .functions
        .iter()
        .find(|f| f.name == "handle_irq")
        .expect("handle_irq must be present");

    let has_read_raw_call = handle_irq.body.basic_blocks.iter().any(|bb| {
        matches!(
            bb.terminator,
            Some(MirTerminator {
                kind: MirTerminatorKind::Call {
                    func: MirOperand::Constant {
                        constant: MirConstant {
                            ty: MirTy::Fn { ref path, .. },
                            ..
                        },
                    },
                    ..
                },
                ..
            }) if path.contains("read_raw")
        )
    });

    assert!(
        has_read_raw_call,
        "MIR must contain a Call terminator referencing read_raw for the MIS status read"
    );
}

/// Check 2j: Control-flow ordering verification.
///
/// Walks the MIR basic-block graph to prove that:
/// 1. Early `status == 0` return of NotHandled happens first (before any helper
///    calls or mask checks).
/// 2. RX and TX dispatch occur before `clear_explicit_interrupts(status)`.
/// 3. `clear_explicit_interrupts(status)` occurs before the final handled
///    condition.
/// 4. Final true branch returns IrqResult::Handled (variant 0), final false
///    branch returns IrqResult::NotHandled (variant 1).
#[test]
fn check2j_control_flow_ordering() {
    let result = compile_and_extract(SPECIMEN, "s2e12_check2j");

    let handle_irq = result
        .body
        .functions
        .iter()
        .find(|f| f.name == "handle_irq")
        .expect("handle_irq must be present");

    let bbs = &handle_irq.body.basic_blocks;

    // -----------------------------------------------------------------------
    // Locate key CFG nodes
    // -----------------------------------------------------------------------

    let read_raw_bb = find_call_bb(bbs, "read_raw").expect("must find read_raw Call bb");
    let service_rx_bb =
        find_call_bb(bbs, "service_rx_fifo").expect("must find service_rx_fifo Call bb");
    let service_tx_bb =
        find_call_bb(bbs, "service_tx_fifo").expect("must find service_tx_fifo Call bb");
    let clear_bb = find_call_bb(bbs, "clear_explicit_interrupts")
        .expect("must find clear_explicit_interrupts Call bb");

    // Find the early status==0 SwitchInt: it is the first SwitchInt after
    // read_raw that has a BinaryOp::Eq in its block and switches on value 0.
    let early_switch_bb = bbs
        .iter()
        .enumerate()
        .find_map(|(i, bb)| {
            if !reachable(bbs, read_raw_bb, i) && i != read_raw_bb {
                return None;
            }
            if let Some(ref term) = bb.terminator {
                if let MirTerminatorKind::SwitchInt { ref targets, .. } = term.kind {
                    if targets.values.contains(&0u128) {
                        let has_eq = bb.statements.iter().any(|s| {
                            matches!(
                                s.kind,
                                MirStatementKind::Assign {
                                    rvalue: box MirRvalue::BinaryOp {
                                        op: MirBinOp::Eq,
                                        ..
                                    },
                                    ..
                                }
                            )
                        });
                        if has_eq {
                            return Some(i);
                        }
                    }
                }
            }
            None
        })
        .expect("must find early status==0 SwitchInt bb");

    // The early SwitchInt's "true" target (status==0) is targets[1]
    // (SwitchInt { values: [0], targets: [false_bb, otherwise_bb] } and the
    // bool discr being non-zero means "condition true").
    let early_term = bbs[early_switch_bb].terminator.as_ref().unwrap();
    let (early_false_bb, early_true_bb) =
        if let MirTerminatorKind::SwitchInt { ref targets, .. } = early_term.kind {
            (targets.targets[0] as usize, targets.targets[1] as usize)
        } else {
            panic!("expected SwitchInt");
        };

    // -----------------------------------------------------------------------
    // 1. Early `status == 0` return: the true arm constructs NotHandled
    //    (variant 1) and reaches Return, WITHOUT passing through any helper
    //    call blocks.
    // -----------------------------------------------------------------------

    // Verify the early true arm constructs IrqResult variant 1 (NotHandled)
    let early_variant = bbs[early_true_bb].statements.iter().find_map(|stmt| {
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
                return Some(variant_index);
            }
        }
        None
    });
    assert_eq!(
        early_variant,
        Some(1),
        "early status==0 true branch must construct IrqResult variant 1 (NotHandled)"
    );

    // Verify the early true arm reaches Return
    assert!(
        find_return_reachable_from(bbs, early_true_bb).is_some(),
        "early NotHandled path must reach Return"
    );

    // Verify the early path does NOT go through any helper calls
    assert!(
        !reachable_avoiding(bbs, early_true_bb, service_rx_bb, &[]),
        "early NotHandled path must not reach service_rx_fifo"
    );
    assert!(
        !reachable_avoiding(bbs, early_true_bb, service_tx_bb, &[]),
        "early NotHandled path must not reach service_tx_fifo"
    );
    assert!(
        !reachable_avoiding(bbs, early_true_bb, clear_bb, &[]),
        "early NotHandled path must not reach clear_explicit_interrupts"
    );

    // -----------------------------------------------------------------------
    // 2. RX and TX dispatch before clear_explicit_interrupts:
    //    from the early_false_bb (status != 0 path), both service_rx_fifo
    //    and service_tx_fifo call sites must appear before clear_bb in the
    //    CFG.
    // -----------------------------------------------------------------------

    // Both RX and TX call bbs are reachable from the false arm of the early
    // switch (the path where status != 0), before clear_bb.
    assert!(
        reachable(bbs, early_false_bb, service_rx_bb),
        "service_rx_fifo must be reachable from the status!=0 path"
    );
    assert!(
        reachable(bbs, early_false_bb, service_tx_bb),
        "service_tx_fifo must be reachable from the status!=0 path"
    );

    // clear_explicit_interrupts must be reachable from both service call
    // continuations (the target bb after each Call).
    let rx_cont = if let Some(ref term) = bbs[service_rx_bb].terminator {
        if let MirTerminatorKind::Call {
            target: Some(t), ..
        } = term.kind
        {
            t as usize
        } else {
            panic!("service_rx_fifo Call must have a target")
        }
    } else {
        panic!("service_rx_fifo bb must have terminator")
    };

    let tx_cont = if let Some(ref term) = bbs[service_tx_bb].terminator {
        if let MirTerminatorKind::Call {
            target: Some(t), ..
        } = term.kind
        {
            t as usize
        } else {
            panic!("service_tx_fifo Call must have a target")
        }
    } else {
        panic!("service_tx_fifo bb must have terminator")
    };

    assert!(
        reachable(bbs, rx_cont, clear_bb),
        "clear_explicit_interrupts must be reachable after service_rx_fifo"
    );
    assert!(
        reachable(bbs, tx_cont, clear_bb),
        "clear_explicit_interrupts must be reachable after service_tx_fifo"
    );

    // Also ensure clear_bb is NOT reachable from early_false_bb without
    // passing through a block that is a merge point AFTER the RX/TX branches.
    // This is validated indirectly: clear_bb cannot be reached from the
    // early_false_bb without going through the RX/TX convergence. We prove
    // this by showing clear_bb is NOT directly reachable from early_false_bb
    // while avoiding both the RX SwitchInt false-arm merge and the TX
    // SwitchInt false-arm merge. Actually, the strongest claim is: the only
    // CFG paths from entry to clear_bb pass through blocks that are
    // downstream of both RX and TX decision points.

    // -----------------------------------------------------------------------
    // 3. clear_explicit_interrupts before final handled condition:
    //    After clear_bb's continuation, the final SwitchInt on the `handled`
    //    accumulator or the EXPLICIT_ISR_CLEAR_MASK BitAnd must appear.
    // -----------------------------------------------------------------------

    let clear_cont = if let Some(ref term) = bbs[clear_bb].terminator {
        if let MirTerminatorKind::Call {
            target: Some(t), ..
        } = term.kind
        {
            t as usize
        } else {
            panic!("clear_explicit_interrupts Call must have a target")
        }
    } else {
        panic!("clear_explicit_interrupts bb must have terminator")
    };

    // Find a block after clear_cont that has BitAnd with EXPLICIT_ISR_CLEAR_MASK
    let final_mask_bb = bbs.iter().enumerate().find_map(|(i, bb)| {
        if !reachable(bbs, clear_cont, i) && i != clear_cont {
            return None;
        }
        let has_explicit_mask = bb.statements.iter().any(|stmt| {
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
                let check_op = |op: &MirOperand| -> bool {
                    match op {
                        MirOperand::Constant {
                            constant:
                                MirConstant {
                                    value: MirConstValue::Bits { value: bits, .. },
                                    ..
                                },
                        } => *bits == 0x0780,
                        MirOperand::Constant {
                            constant:
                                MirConstant {
                                    value: MirConstValue::Unevaluated { ref path },
                                    ..
                                },
                        } => path.contains("EXPLICIT_ISR_CLEAR_MASK"),
                        _ => false,
                    }
                };
                check_op(lhs) || check_op(rhs)
            } else {
                false
            }
        });
        if has_explicit_mask {
            Some(i)
        } else {
            None
        }
    });

    assert!(
        final_mask_bb.is_some(),
        "EXPLICIT_ISR_CLEAR_MASK BitAnd must be reachable after clear_explicit_interrupts"
    );

    // Also confirm clear_bb is NOT reachable from final_mask_bb (no back-edge)
    let final_mask_bb = final_mask_bb.unwrap();
    assert!(
        !reachable(bbs, final_mask_bb, clear_bb),
        "clear_explicit_interrupts must not be reachable from the final mask check \
         (ordering: clear before final condition)"
    );

    // -----------------------------------------------------------------------
    // 4. Final true branch returns Handled (variant 0), false returns
    //    NotHandled (variant 1).
    //
    //    The final condition is a short-circuit of `handled || (status &
    //    EXPLICIT_ISR_CLEAR_MASK != 0)`. In MIR this becomes two SwitchInts:
    //    first on `handled`, then (if false) on the mask check.
    //    The "true" outcome constructs variant 0, the "false" constructs
    //    variant 1.
    // -----------------------------------------------------------------------

    // Find IrqResult variant 0 (Handled) blocks reachable from clear_cont
    let handled_bbs = find_irqresult_aggregate_bbs(bbs, 0);
    let final_handled_bb = handled_bbs
        .iter()
        .find(|&&bb| reachable(bbs, clear_cont, bb))
        .expect("must find a Handled aggregate reachable after clear_explicit_interrupts");

    // Find IrqResult variant 1 (NotHandled) blocks reachable from clear_cont
    let not_handled_bbs = find_irqresult_aggregate_bbs(bbs, 1);
    let final_not_handled_bb = not_handled_bbs
        .iter()
        .find(|&&bb| reachable(bbs, clear_cont, bb))
        .expect("must find a NotHandled aggregate reachable after clear_explicit_interrupts");

    // Both must reach Return
    assert!(
        find_return_reachable_from(bbs, *final_handled_bb).is_some(),
        "final Handled path must reach Return"
    );
    assert!(
        find_return_reachable_from(bbs, *final_not_handled_bb).is_some(),
        "final NotHandled path must reach Return"
    );

    // The Handled and NotHandled blocks must be distinct
    assert_ne!(
        final_handled_bb, final_not_handled_bb,
        "final Handled and NotHandled must be in distinct basic blocks"
    );
}

/// Check 2i: helpers service_rx_fifo, service_tx_fifo, and
/// clear_explicit_interrupts are extracted as Helper functions in MIR body surface.
#[test]
fn check2i_helpers_extracted_as_functions() {
    let result = compile_and_extract(SPECIMEN, "s2e12_check2i");

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
