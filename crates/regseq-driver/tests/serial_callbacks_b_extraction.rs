//! source-to-extraction-11: PL011 serial callbacks B authored->extraction boundary
//! validation.
//!
//! Validates that the second half of the authored serial-callback facts for PL011
//! survive at the compiler-side boundary between the authored PL011 source and
//! the current Regseq extraction state.
//!
//! Callbacks in scope: tx_ready, tx_empty, flush_rx_fifo, flush_tx_fifo, send_break
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

struct ExtractionAll {
    trait_surface: TraitSurface,
    identity: IdentityEnrichmentSurface,
    body: MirBodySurface,
}

fn compile_and_extract_all(source: &str, test_name: &str) -> ExtractionAll {
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

    ExtractionAll {
        trait_surface,
        identity,
        body,
    }
}

// ---------------------------------------------------------------------------
// Inline specimen: reduced PL011 serial callbacks B surface
// ---------------------------------------------------------------------------

/// Reduced specimen covering the five serial callbacks in scope:
/// tx_ready, tx_empty, flush_rx_fifo, flush_tx_fifo, send_break.
///
/// Includes the Serial trait definition with all methods from the authored
/// source, the register infrastructure needed for callback bodies, and the
/// driver struct implementing Serial.
const SPECIMEN: &str = r#"
    #![allow(dead_code)]

    pub mod regseq {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum RegseqError {
            InvalidParameter,
            HardwareError,
            GenericFailure,
        }

        pub struct SerialConfig { _private: () }

        #[derive(Debug, Clone, Copy)]
        pub struct RxChar { byte: u8 }
        impl RxChar {
            pub fn new(byte: u8, _errors: u8) -> Self { Self { byte } }
        }

        pub trait Lifecycle {
            fn init(&mut self) -> Result<(), RegseqError>;
            fn deinit(&mut self) -> Result<(), RegseqError>;
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

    pub struct RegField;

    pub mod FR_REG {
        use super::RegField;
        pub mod TXFF {
            use super::super::RegField;
            pub const FIELD: RegField = RegField;
        }
        pub mod BUSY {
            use super::super::RegField;
            pub const FIELD: RegField = RegField;
        }
        pub mod RXFE {
            use super::super::RegField;
            pub const FIELD: RegField = RegField;
        }
    }

    pub mod LCRH_REG {
        pub struct FieldValue;
        pub mod FEN {
            use super::FieldValue;
            pub const DISABLED: FieldValue = FieldValue;
            pub const ENABLED: FieldValue = FieldValue;
        }
        pub mod BRK {
            use super::FieldValue;
            pub const ENABLED: FieldValue = FieldValue;
            pub const DISABLED: FieldValue = FieldValue;
        }
    }

    pub struct FrReg { _opaque: () }
    impl FrReg {
        pub fn read(&self, _field: RegField) -> u32 { 0 }
    }

    pub struct DrReg { _opaque: () }
    impl DrReg {
        pub fn read_raw(&self) -> u32 { 0 }
    }

    pub struct LcrhReg { _opaque: () }
    impl LcrhReg {
        pub fn modify(&mut self, _val: LCRH_REG::FieldValue) {}
    }

    pub struct Pl011Registers {
        pub fr: FrReg,
        pub dr: DrReg,
        pub lcr_h: LcrhReg,
    }

    #[rseq::driver(name = "pl011-full-spec", compatible = ["arm,pl011"])]
    pub struct Pl011FullSpec {
        regs: Pl011Registers,
    }

    impl regseq::Serial for Pl011FullSpec {
        fn port_open(&mut self) -> Result<(), RegseqError> { Ok(()) }
        fn port_close(&mut self) {}
        fn configure(&mut self, _config: &SerialConfig) -> Result<(), RegseqError> { Ok(()) }
        fn tx_byte(&mut self, _byte: u8) {}
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
// Precondition: specimen fidelity against real fixture source
// ---------------------------------------------------------------------------

/// Reads the real PL011 fixture source and verifies that the specimen's
/// authored facts match the real fixture for every fact in scope.
#[test]
fn precondition_specimen_fidelity() {
    let fixture = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/pl011-full-spec/src/driver.rs"
    ))
    .expect("fixture source must be readable");

    // -- tx_ready facts --
    // Fact: fn tx_ready(&mut self) -> bool exists
    assert!(
        fixture.contains("fn tx_ready(&mut self) -> bool"),
        "fixture must contain `fn tx_ready(&mut self) -> bool`"
    );
    // Fact: body reads FR.TXFF and compares to 0
    assert!(
        fixture.contains("self.regs.fr.read(FR_REG::TXFF::FIELD) == 0"),
        "fixture must contain `self.regs.fr.read(FR_REG::TXFF::FIELD) == 0`"
    );

    // -- tx_empty facts --
    // Fact: fn tx_empty(&mut self) -> bool exists
    assert!(
        fixture.contains("fn tx_empty(&mut self) -> bool"),
        "fixture must contain `fn tx_empty(&mut self) -> bool`"
    );
    // Fact: body reads FR.BUSY and compares to 0
    assert!(
        fixture.contains("self.regs.fr.read(FR_REG::BUSY::FIELD) == 0"),
        "fixture must contain `self.regs.fr.read(FR_REG::BUSY::FIELD) == 0`"
    );

    // -- flush_rx_fifo facts --
    // Fact: fn flush_rx_fifo(&mut self) exists
    assert!(
        fixture.contains("fn flush_rx_fifo(&mut self)"),
        "fixture must contain `fn flush_rx_fifo(&mut self)`"
    );
    // Fact: while loop condition on FR.RXFE
    assert!(
        fixture.contains("while self.regs.fr.read(FR_REG::RXFE::FIELD) == 0"),
        "fixture must contain while loop on FR.RXFE"
    );
    // Fact: loop body discards raw DR read
    assert!(
        fixture.contains("let _ = self.regs.dr.read_raw()"),
        "fixture must contain `let _ = self.regs.dr.read_raw()`"
    );

    // -- flush_tx_fifo facts --
    // Fact: fn flush_tx_fifo(&mut self) exists
    assert!(
        fixture.contains("fn flush_tx_fifo(&mut self)"),
        "fixture must contain `fn flush_tx_fifo(&mut self)`"
    );
    // Fact: first modify sets FEN::DISABLED
    assert!(
        fixture.contains("self.regs.lcr_h.modify(LCRH_REG::FEN::DISABLED)"),
        "fixture must contain `self.regs.lcr_h.modify(LCRH_REG::FEN::DISABLED)`"
    );
    // Fact: second modify sets FEN::ENABLED
    assert!(
        fixture.contains("self.regs.lcr_h.modify(LCRH_REG::FEN::ENABLED)"),
        "fixture must contain `self.regs.lcr_h.modify(LCRH_REG::FEN::ENABLED)`"
    );

    // -- send_break facts --
    // Fact: fn send_break(&mut self, on: bool) exists
    assert!(
        fixture.contains("fn send_break(&mut self, on: bool)"),
        "fixture must contain `fn send_break(&mut self, on: bool)`"
    );
    // Fact: on==true path modifies BRK::ENABLED
    assert!(
        fixture.contains("self.regs.lcr_h.modify(LCRH_REG::BRK::ENABLED)"),
        "fixture must contain `self.regs.lcr_h.modify(LCRH_REG::BRK::ENABLED)`"
    );
    // Fact: on==false path modifies BRK::DISABLED
    assert!(
        fixture.contains("self.regs.lcr_h.modify(LCRH_REG::BRK::DISABLED)"),
        "fixture must contain `self.regs.lcr_h.modify(LCRH_REG::BRK::DISABLED)`"
    );

    // -- Verify specimen reproduces these facts --
    assert!(
        SPECIMEN.contains("fn tx_ready(&mut self) -> bool"),
        "specimen must contain tx_ready signature"
    );
    assert!(
        SPECIMEN.contains("self.regs.fr.read(FR_REG::TXFF::FIELD) == 0"),
        "specimen must contain tx_ready body"
    );
    assert!(
        SPECIMEN.contains("fn tx_empty(&mut self) -> bool"),
        "specimen must contain tx_empty signature"
    );
    assert!(
        SPECIMEN.contains("self.regs.fr.read(FR_REG::BUSY::FIELD) == 0"),
        "specimen must contain tx_empty body"
    );
    assert!(
        SPECIMEN.contains("fn flush_rx_fifo(&mut self)"),
        "specimen must contain flush_rx_fifo signature"
    );
    assert!(
        SPECIMEN.contains("while self.regs.fr.read(FR_REG::RXFE::FIELD) == 0"),
        "specimen must contain flush_rx_fifo loop condition"
    );
    assert!(
        SPECIMEN.contains("let _ = self.regs.dr.read_raw()"),
        "specimen must contain flush_rx_fifo loop body"
    );
    assert!(
        SPECIMEN.contains("fn flush_tx_fifo(&mut self)"),
        "specimen must contain flush_tx_fifo signature"
    );
    assert!(
        SPECIMEN.contains("self.regs.lcr_h.modify(LCRH_REG::FEN::DISABLED)"),
        "specimen must contain flush_tx_fifo first modify"
    );
    assert!(
        SPECIMEN.contains("self.regs.lcr_h.modify(LCRH_REG::FEN::ENABLED)"),
        "specimen must contain flush_tx_fifo second modify"
    );
    assert!(
        SPECIMEN.contains("fn send_break(&mut self, on: bool)"),
        "specimen must contain send_break signature"
    );
    assert!(
        SPECIMEN.contains("self.regs.lcr_h.modify(LCRH_REG::BRK::ENABLED)"),
        "specimen must contain send_break enabled path"
    );
    assert!(
        SPECIMEN.contains("self.regs.lcr_h.modify(LCRH_REG::BRK::DISABLED)"),
        "specimen must contain send_break disabled path"
    );

    // Verify all five callbacks exist in the Serial trait impl
    assert!(
        SPECIMEN.contains("impl regseq::Serial for Pl011FullSpec"),
        "specimen must contain Serial impl for Pl011FullSpec"
    );
    // Fixture uses `impl Serial for Pl011FullSpec` (Serial imported via use)
    assert!(
        fixture.contains("impl Serial for Pl011FullSpec"),
        "fixture must contain Serial impl for Pl011FullSpec"
    );
}

// ---------------------------------------------------------------------------
// Helper: find extracted function by name
// ---------------------------------------------------------------------------

fn find_fn<'a>(surface: &'a MirBodySurface, name: &str) -> &'a MirExtractedFn {
    surface
        .functions
        .iter()
        .find(|f| f.name == name)
        .unwrap_or_else(|| panic!("expected function '{}' in body surface", name))
}

/// Check that a MIR body contains a call to a function whose def-path contains
/// the given substring.
fn body_has_call_containing(body: &MirBody, pattern: &str) -> bool {
    body.basic_blocks.iter().any(|bb| {
        if let Some(MirTerminator {
            kind: MirTerminatorKind::Call { func, .. },
            ..
        }) = &bb.terminator
        {
            match func {
                MirOperand::Constant { constant } => match &constant.ty {
                    MirTy::FnDef { path, .. } => path.contains(pattern),
                    _ => false,
                },
                _ => false,
            }
        } else {
            false
        }
    })
}

/// Check that a MIR body contains a SwitchInt terminator (branch).
fn body_has_switch_int(body: &MirBody) -> bool {
    body.basic_blocks.iter().any(|bb| {
        matches!(
            &bb.terminator,
            Some(MirTerminator {
                kind: MirTerminatorKind::SwitchInt { .. },
                ..
            })
        )
    })
}

/// Check that a MIR body contains a Eq binary comparison.
fn body_has_eq_comparison(body: &MirBody) -> bool {
    body.basic_blocks.iter().any(|bb| {
        bb.statements.iter().any(|stmt| {
            matches!(
                &stmt.kind,
                MirStatementKind::Assign {
                    rvalue: box MirRvalue::BinaryOp {
                        op: MirBinOp::Eq,
                        ..
                    },
                    ..
                }
            )
        })
    })
}

/// Collect all call target paths from a MIR body.
fn collect_call_paths(body: &MirBody) -> Vec<String> {
    let mut paths = Vec::new();
    for bb in &body.basic_blocks {
        if let Some(MirTerminator {
            kind: MirTerminatorKind::Call { func, .. },
            ..
        }) = &bb.terminator
        {
            if let MirOperand::Constant { constant } = func {
                if let MirTy::FnDef { path, .. } = &constant.ty {
                    paths.push(path.clone());
                }
            }
        }
    }
    paths
}

/// Check that a MIR body has a while-loop pattern: a block that branches
/// back to an earlier block (backedge) via SwitchInt.
fn body_has_loop_pattern(body: &MirBody) -> bool {
    for (idx, bb) in body.basic_blocks.iter().enumerate() {
        if let Some(MirTerminator {
            kind: MirTerminatorKind::SwitchInt { targets, .. },
            ..
        }) = &bb.terminator
        {
            // A backedge exists if any target points to this block or an earlier block
            for &t in &targets.targets {
                if (t as usize) <= idx {
                    return true;
                }
            }
        }
        // Also check Goto backedges
        if let Some(MirTerminator {
            kind: MirTerminatorKind::Goto { target },
            ..
        }) = &bb.terminator
        {
            if (*target as usize) <= idx {
                return true;
            }
        }
    }
    false
}

// ---------------------------------------------------------------------------
// Check Group 1: callback identity, trait role, signature, and return type
// ---------------------------------------------------------------------------

#[test]
fn check_group_1_serial_callback_identity_and_signatures() {
    let extracted = compile_and_extract_all(SPECIMEN, "serial_callbacks_b_cg1");

    // -- Trait surface: serial impl exists --
    assert!(
        !extracted.trait_surface.serial_impls.is_empty(),
        "TraitSurface must contain at least one Serial impl"
    );
    let serial_impl = &extracted.trait_surface.serial_impls[0];
    assert_eq!(
        serial_impl.driver_type, "Pl011FullSpec",
        "Serial impl must be for Pl011FullSpec"
    );

    // -- Identity enrichment: Serial trait impl block with methods --
    let serial_impl_block = extracted
        .identity
        .impl_blocks
        .iter()
        .find(|ib| ib.trait_name.as_deref() == Some("Serial"))
        .expect("IdentityEnrichmentSurface must contain a Serial impl block");

    assert_eq!(
        serial_impl_block.self_type_name, "Pl011FullSpec",
        "Serial impl block self type must be Pl011FullSpec"
    );

    let method_names: Vec<&str> = serial_impl_block
        .methods
        .iter()
        .map(|m| m.name.as_str())
        .collect();

    // All five callbacks must appear as methods
    for cb in &[
        "tx_ready",
        "tx_empty",
        "flush_rx_fifo",
        "flush_tx_fifo",
        "send_break",
    ] {
        assert!(
            method_names.contains(cb),
            "Serial impl block must contain method '{}'",
            cb
        );
    }

    // -- Body surface: each callback has correct signature --
    // tx_ready: (&mut self) -> bool
    let tx_ready = find_fn(&extracted.body, "tx_ready");
    assert_eq!(
        tx_ready.receiver, "ref_mut",
        "tx_ready receiver must be &mut self"
    );
    assert!(tx_ready.params.is_empty(), "tx_ready must have no params");
    assert_eq!(
        tx_ready.return_type.as_deref(),
        Some("bool"),
        "tx_ready must return bool"
    );
    assert!(
        matches!(
            &tx_ready.kind,
            MirFnKind::Callback { trait_name, method_name }
            if trait_name == "Serial" && method_name == "tx_ready"
        ),
        "tx_ready must be classified as Serial::tx_ready callback"
    );

    // tx_empty: (&mut self) -> bool
    let tx_empty = find_fn(&extracted.body, "tx_empty");
    assert_eq!(
        tx_empty.receiver, "ref_mut",
        "tx_empty receiver must be &mut self"
    );
    assert!(tx_empty.params.is_empty(), "tx_empty must have no params");
    assert_eq!(
        tx_empty.return_type.as_deref(),
        Some("bool"),
        "tx_empty must return bool"
    );
    assert!(
        matches!(
            &tx_empty.kind,
            MirFnKind::Callback { trait_name, method_name }
            if trait_name == "Serial" && method_name == "tx_empty"
        ),
        "tx_empty must be classified as Serial::tx_empty callback"
    );

    // flush_rx_fifo: (&mut self) -> ()
    let flush_rx_fifo = find_fn(&extracted.body, "flush_rx_fifo");
    assert_eq!(
        flush_rx_fifo.receiver, "ref_mut",
        "flush_rx_fifo receiver must be &mut self"
    );
    assert!(
        flush_rx_fifo.params.is_empty(),
        "flush_rx_fifo must have no params"
    );
    assert_eq!(
        flush_rx_fifo.return_type, None,
        "flush_rx_fifo must return unit (None)"
    );
    assert!(
        matches!(
            &flush_rx_fifo.kind,
            MirFnKind::Callback { trait_name, method_name }
            if trait_name == "Serial" && method_name == "flush_rx_fifo"
        ),
        "flush_rx_fifo must be classified as Serial::flush_rx_fifo callback"
    );

    // flush_tx_fifo: (&mut self) -> ()
    let flush_tx_fifo = find_fn(&extracted.body, "flush_tx_fifo");
    assert_eq!(
        flush_tx_fifo.receiver, "ref_mut",
        "flush_tx_fifo receiver must be &mut self"
    );
    assert!(
        flush_tx_fifo.params.is_empty(),
        "flush_tx_fifo must have no params"
    );
    assert_eq!(
        flush_tx_fifo.return_type, None,
        "flush_tx_fifo must return unit (None)"
    );
    assert!(
        matches!(
            &flush_tx_fifo.kind,
            MirFnKind::Callback { trait_name, method_name }
            if trait_name == "Serial" && method_name == "flush_tx_fifo"
        ),
        "flush_tx_fifo must be classified as Serial::flush_tx_fifo callback"
    );

    // send_break: (&mut self, on: bool) -> ()
    let send_break = find_fn(&extracted.body, "send_break");
    assert_eq!(
        send_break.receiver, "ref_mut",
        "send_break receiver must be &mut self"
    );
    assert_eq!(send_break.params.len(), 1, "send_break must have one param");
    assert_eq!(
        send_break.params[0].ty, "bool",
        "send_break param must be bool"
    );
    assert_eq!(
        send_break.return_type, None,
        "send_break must return unit (None)"
    );
    assert!(
        matches!(
            &send_break.kind,
            MirFnKind::Callback { trait_name, method_name }
            if trait_name == "Serial" && method_name == "send_break"
        ),
        "send_break must be classified as Serial::send_break callback"
    );
}

// ---------------------------------------------------------------------------
// Check Group 2: callback bodies preserve authored logic
// ---------------------------------------------------------------------------

#[test]
fn check_group_2_tx_ready_body_preserves_predicate_logic() {
    let extracted = compile_and_extract_all(SPECIMEN, "serial_callbacks_b_cg2_tx_ready");
    let tx_ready = find_fn(&extracted.body, "tx_ready");

    // tx_ready reads FR.TXFF via a call to `read`
    assert!(
        body_has_call_containing(&tx_ready.body, "read"),
        "tx_ready body must contain a call to `read` (for FR.TXFF)"
    );

    // tx_ready compares the result with Eq (== 0)
    assert!(
        body_has_eq_comparison(&tx_ready.body),
        "tx_ready body must contain an Eq comparison (== 0)"
    );

    // tx_ready returns bool (return place is bool)
    assert_eq!(
        tx_ready.body.local_decls[0].ty,
        MirTy::Bool,
        "tx_ready return place (local 0) must be bool"
    );
}

#[test]
fn check_group_2_tx_empty_body_preserves_predicate_logic() {
    let extracted = compile_and_extract_all(SPECIMEN, "serial_callbacks_b_cg2_tx_empty");
    let tx_empty = find_fn(&extracted.body, "tx_empty");

    // tx_empty reads FR.BUSY via a call to `read`
    assert!(
        body_has_call_containing(&tx_empty.body, "read"),
        "tx_empty body must contain a call to `read` (for FR.BUSY)"
    );

    // tx_empty compares the result with Eq (== 0)
    assert!(
        body_has_eq_comparison(&tx_empty.body),
        "tx_empty body must contain an Eq comparison (== 0)"
    );

    // tx_empty returns bool
    assert_eq!(
        tx_empty.body.local_decls[0].ty,
        MirTy::Bool,
        "tx_empty return place (local 0) must be bool"
    );
}

#[test]
fn check_group_2_flush_rx_fifo_body_preserves_loop_and_drain() {
    let extracted = compile_and_extract_all(SPECIMEN, "serial_callbacks_b_cg2_flush_rx");
    let flush_rx = find_fn(&extracted.body, "flush_rx_fifo");

    // flush_rx_fifo has a loop (backedge in CFG)
    assert!(
        body_has_loop_pattern(&flush_rx.body),
        "flush_rx_fifo body must contain a loop backedge"
    );

    // flush_rx_fifo reads FR.RXFE via `read` as loop condition
    assert!(
        body_has_call_containing(&flush_rx.body, "read"),
        "flush_rx_fifo body must contain a call to `read` (for FR.RXFE loop condition)"
    );

    // flush_rx_fifo reads DR via `read_raw` in loop body
    assert!(
        body_has_call_containing(&flush_rx.body, "read_raw"),
        "flush_rx_fifo body must contain a call to `read_raw` (for DR drain)"
    );

    // flush_rx_fifo compares loop condition with Eq (== 0)
    assert!(
        body_has_eq_comparison(&flush_rx.body),
        "flush_rx_fifo body must contain an Eq comparison for loop condition"
    );

    // flush_rx_fifo returns unit
    let ret_ty = &flush_rx.body.local_decls[0].ty;
    assert!(
        matches!(ret_ty, MirTy::Tuple { elements } if elements.is_empty()),
        "flush_rx_fifo return place must be unit (empty tuple)"
    );
}

#[test]
fn check_group_2_flush_tx_fifo_body_preserves_fen_toggle() {
    let extracted = compile_and_extract_all(SPECIMEN, "serial_callbacks_b_cg2_flush_tx");
    let flush_tx = find_fn(&extracted.body, "flush_tx_fifo");

    // flush_tx_fifo calls `modify` twice (FEN::DISABLED then FEN::ENABLED)
    let call_paths = collect_call_paths(&flush_tx.body);
    let modify_calls: Vec<&String> = call_paths.iter().filter(|p| p.contains("modify")).collect();
    assert!(
        modify_calls.len() >= 2,
        "flush_tx_fifo body must contain at least 2 calls to `modify`, found {}",
        modify_calls.len()
    );

    // flush_tx_fifo returns unit
    let ret_ty = &flush_tx.body.local_decls[0].ty;
    assert!(
        matches!(ret_ty, MirTy::Tuple { elements } if elements.is_empty()),
        "flush_tx_fifo return place must be unit (empty tuple)"
    );

    // flush_tx_fifo has no loop (no backedge)
    assert!(
        !body_has_loop_pattern(&flush_tx.body),
        "flush_tx_fifo body must not contain a loop"
    );
}

#[test]
fn check_group_2_send_break_body_preserves_branch_and_modify() {
    let extracted = compile_and_extract_all(SPECIMEN, "serial_callbacks_b_cg2_send_break");
    let send_break = find_fn(&extracted.body, "send_break");

    // send_break branches on the `on` parameter (SwitchInt)
    assert!(
        body_has_switch_int(&send_break.body),
        "send_break body must contain a SwitchInt (branch on `on: bool`)"
    );

    // send_break calls `modify` (for LCR_H.BRK)
    assert!(
        body_has_call_containing(&send_break.body, "modify"),
        "send_break body must contain a call to `modify` (for LCR_H.BRK)"
    );

    // send_break returns unit
    let ret_ty = &send_break.body.local_decls[0].ty;
    assert!(
        matches!(ret_ty, MirTy::Tuple { elements } if elements.is_empty()),
        "send_break return place must be unit (empty tuple)"
    );

    // send_break has the `on` parameter as arg1 with bool type
    assert_eq!(
        send_break.body.arg_count, 2,
        "send_break must have 2 MIR args (self + on)"
    );
    // arg 1 = self (index 1), arg 2 = on (index 2)
    let on_param = &send_break.body.local_decls[2];
    assert_eq!(
        on_param.ty,
        MirTy::Bool,
        "send_break arg 2 (on) must be bool in MIR local_decls"
    );
}
