//! source-to-extraction-09: PL011 lifecycle callbacks authored->extraction
//! boundary validation.
//!
//! Validates that the authored lifecycle-callback facts for PL011 survive at
//! the compiler-side boundary between the authored PL011 source and the
//! current Regseq extraction state.
//!
//! Extraction surfaces under test:
//! - `TraitSurface.lifecycle_impls` (trait-role surface)
//! - `IdentityEnrichmentSurface.impl_blocks` (callback identity surface)
//! - `MirBodySurface.functions` (executable callback-body surface)

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

struct TripleSurface {
    traits: TraitSurface,
    identity: IdentityEnrichmentSurface,
    bodies: MirBodySurface,
}

fn compile_and_extract_all(source: &str, test_name: &str) -> TripleSurface {
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

    let traits = trait_result
        .lock()
        .unwrap()
        .take()
        .expect("after_analysis should have run")
        .expect("trait extraction should succeed");

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

    TripleSurface {
        traits,
        identity,
        bodies,
    }
}

// ---------------------------------------------------------------------------
// Reduced specimen -- lifecycle callbacks only
// ---------------------------------------------------------------------------

/// Reduced specimen preserving the lifecycle callback authored facts.
/// Fidelity against the real fixture source is verified by the
/// `specimen_fidelity_against_fixture` test.
const LIFECYCLE_SPECIMEN: &str = r#"
pub mod regseq {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum RegseqError {
        InvalidParameter,
        HardwareError,
        GenericFailure,
    }

    pub trait Lifecycle {
        fn init(&mut self) -> Result<(), RegseqError>;
        fn deinit(&mut self) -> Result<(), RegseqError>;
    }
}

use regseq::*;

#[rseq::driver(name = "pl011-full-spec", compatible = ["arm,pl011"], bus = "amba")]
pub struct Pl011FullSpec {
    current_speed: u32,
}

impl Pl011FullSpec {
    fn toggle_reset_if_present(&self) -> Result<(), RegseqError> {
        Ok(())
    }

    fn apply_default_pins_if_present(&self) -> Result<(), RegseqError> {
        Ok(())
    }

    fn enable_clock_if_present(&self) -> Result<(), RegseqError> {
        Ok(())
    }

    fn program_polling_port(&mut self, _uart_clk_hz: u32, _baud_rate: u32) {
    }

    fn uart_clock_hz(&self) -> u32 {
        48_000_000
    }

    fn shutdown_port(&mut self) {
    }

    fn disable_clock_if_present(&self) {
    }

    fn apply_sleep_pins_if_present(&self) -> Result<(), RegseqError> {
        Ok(())
    }
}

impl regseq::Lifecycle for Pl011FullSpec {
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
"#;

// ---------------------------------------------------------------------------
// Precondition: specimen fidelity against real fixture source
// ---------------------------------------------------------------------------

/// Verify that the reduced specimen preserves every authored lifecycle fact
/// present in the real PL011 fixture source.
#[test]
fn specimen_fidelity_against_fixture() {
    let fixture = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/pl011-full-spec/src/driver.rs"
    ))
    .expect("fixture source must be readable");

    // -- init callback body fidelity --
    // The fixture's init body contains exactly these lines in order:
    assert!(
        fixture.contains("self.toggle_reset_if_present()?;"),
        "fixture must contain toggle_reset_if_present()? call"
    );
    assert!(
        fixture.contains("self.apply_default_pins_if_present()?;"),
        "fixture must contain apply_default_pins_if_present()? call"
    );
    assert!(
        fixture.contains("self.enable_clock_if_present()?;"),
        "fixture must contain enable_clock_if_present()? call"
    );
    assert!(
        fixture.contains("self.program_polling_port(self.uart_clock_hz(), self.current_speed)"),
        "fixture must contain program_polling_port call with exact args"
    );

    // Verify init signature
    assert!(
        fixture.contains("fn init(&mut self) -> Result<(), RegseqError>"),
        "fixture must contain init with exact signature"
    );

    // -- deinit callback body fidelity --
    assert!(
        fixture.contains("self.shutdown_port();"),
        "fixture must contain shutdown_port() call"
    );
    assert!(
        fixture.contains("self.disable_clock_if_present();"),
        "fixture must contain disable_clock_if_present() call"
    );
    assert!(
        fixture.contains("self.apply_sleep_pins_if_present()?;"),
        "fixture must contain apply_sleep_pins_if_present()? call"
    );

    // Verify deinit signature
    assert!(
        fixture.contains("fn deinit(&mut self) -> Result<(), RegseqError>"),
        "fixture must contain deinit with exact signature"
    );

    // Verify trait impl block presence
    assert!(
        fixture.contains("impl Lifecycle for Pl011FullSpec"),
        "fixture must contain Lifecycle impl for Pl011FullSpec"
    );

    // Verify helper fallibility matches the specimen:
    // toggle_reset_if_present returns Result
    assert!(
        fixture.contains("fn toggle_reset_if_present(&self) -> Result<(), RegseqError>"),
        "fixture toggle_reset_if_present must return Result"
    );
    assert!(
        fixture.contains("fn apply_default_pins_if_present(&self) -> Result<(), RegseqError>"),
        "fixture apply_default_pins_if_present must return Result"
    );
    assert!(
        fixture.contains("fn enable_clock_if_present(&self) -> Result<(), RegseqError>"),
        "fixture enable_clock_if_present must return Result"
    );
    // program_polling_port is infallible
    assert!(
        fixture.contains("fn program_polling_port(&mut self, uart_clk_hz: u32, baud_rate: u32)"),
        "fixture program_polling_port must be infallible with exact params"
    );
    // shutdown_port is infallible
    assert!(
        fixture.contains("fn shutdown_port(&mut self)"),
        "fixture shutdown_port must be infallible"
    );
    // disable_clock_if_present is infallible
    assert!(
        fixture.contains("fn disable_clock_if_present(&self)"),
        "fixture disable_clock_if_present must be infallible"
    );
    // apply_sleep_pins_if_present returns Result
    assert!(
        fixture.contains("fn apply_sleep_pins_if_present(&self) -> Result<(), RegseqError>"),
        "fixture apply_sleep_pins_if_present must return Result"
    );

    // Also verify the specimen itself contains the same authored facts
    assert!(
        LIFECYCLE_SPECIMEN.contains("self.toggle_reset_if_present()?;"),
        "specimen must contain toggle_reset_if_present()?"
    );
    assert!(
        LIFECYCLE_SPECIMEN.contains("self.apply_default_pins_if_present()?;"),
        "specimen must contain apply_default_pins_if_present()?"
    );
    assert!(
        LIFECYCLE_SPECIMEN.contains("self.enable_clock_if_present()?;"),
        "specimen must contain enable_clock_if_present()?"
    );
    assert!(
        LIFECYCLE_SPECIMEN
            .contains("self.program_polling_port(self.uart_clock_hz(), self.current_speed)"),
        "specimen must contain program_polling_port with exact args"
    );
    assert!(
        LIFECYCLE_SPECIMEN.contains("self.shutdown_port();"),
        "specimen must contain shutdown_port()"
    );
    assert!(
        LIFECYCLE_SPECIMEN.contains("self.disable_clock_if_present();"),
        "specimen must contain disable_clock_if_present()"
    );
    assert!(
        LIFECYCLE_SPECIMEN.contains("self.apply_sleep_pins_if_present()?;"),
        "specimen must contain apply_sleep_pins_if_present()?"
    );
}

// ---------------------------------------------------------------------------
// Check Group 1: Lifecycle callback identity, trait role, signature, return
// ---------------------------------------------------------------------------

#[test]
fn check_group_1_trait_surface_lifecycle_identity() {
    let surface = compile_and_extract_all(LIFECYCLE_SPECIMEN, "lifecycle_cg1_trait");

    // Trait surface: exactly one Lifecycle impl
    assert_eq!(
        surface.traits.lifecycle_impls.len(),
        1,
        "trait surface must contain exactly one Lifecycle impl"
    );
    assert_eq!(
        surface.traits.lifecycle_impls[0].driver_type, "Pl011FullSpec",
        "Lifecycle impl must be for Pl011FullSpec"
    );
}

#[test]
fn check_group_1_identity_enrichment_lifecycle_impl_block() {
    let surface = compile_and_extract_all(LIFECYCLE_SPECIMEN, "lifecycle_cg1_identity");

    // Identity enrichment: find the Lifecycle trait impl block
    let lifecycle_impl = surface
        .identity
        .impl_blocks
        .iter()
        .find(|ib| ib.trait_name.as_deref() == Some("Lifecycle"))
        .expect("identity surface must contain a Lifecycle impl block");

    assert_eq!(
        lifecycle_impl.self_type_name, "Pl011FullSpec",
        "Lifecycle impl must be for Pl011FullSpec"
    );

    // Must contain init and deinit methods
    let method_names: Vec<&str> = lifecycle_impl
        .methods
        .iter()
        .map(|m| m.name.as_str())
        .collect();
    assert!(
        method_names.contains(&"init"),
        "Lifecycle impl block must contain init method"
    );
    assert!(
        method_names.contains(&"deinit"),
        "Lifecycle impl block must contain deinit method"
    );
}

#[test]
fn check_group_1_body_surface_init_signature() {
    let surface = compile_and_extract_all(LIFECYCLE_SPECIMEN, "lifecycle_cg1_init_sig");

    let init_fn = surface
        .bodies
        .functions
        .iter()
        .find(|f| f.name == "init")
        .expect("body surface must contain init function");

    // Kind must be Callback with trait_name=Lifecycle, method_name=init
    match &init_fn.kind {
        MirFnKind::Callback {
            trait_name,
            method_name,
        } => {
            assert_eq!(trait_name, "Lifecycle", "init must be a Lifecycle callback");
            assert_eq!(method_name, "init", "method_name must be init");
        }
        other => panic!("init must be Callback, got {:?}", other),
    }

    // Receiver must be &mut self
    assert_eq!(
        init_fn.receiver, "ref_mut",
        "init receiver must be &mut self (ref_mut)"
    );

    // Return type must be Result<(), RegseqError>
    let ret = init_fn
        .return_type
        .as_ref()
        .expect("init must have a return type");
    assert!(
        matches!(ret, MirTy::Enum { path, .. } if path.contains("Result")),
        "init return type must be a Result enum, got: {:?}",
        ret
    );

    // No extra parameters (beyond self)
    assert!(
        init_fn.params.is_empty(),
        "init must have no extra parameters"
    );
}

#[test]
fn check_group_1_body_surface_deinit_signature() {
    let surface = compile_and_extract_all(LIFECYCLE_SPECIMEN, "lifecycle_cg1_deinit_sig");

    let deinit_fn = surface
        .bodies
        .functions
        .iter()
        .find(|f| f.name == "deinit")
        .expect("body surface must contain deinit function");

    // Kind must be Callback with trait_name=Lifecycle, method_name=deinit
    match &deinit_fn.kind {
        MirFnKind::Callback {
            trait_name,
            method_name,
        } => {
            assert_eq!(
                trait_name, "Lifecycle",
                "deinit must be a Lifecycle callback"
            );
            assert_eq!(method_name, "deinit", "method_name must be deinit");
        }
        other => panic!("deinit must be Callback, got {:?}", other),
    }

    // Receiver must be &mut self
    assert_eq!(
        deinit_fn.receiver, "ref_mut",
        "deinit receiver must be &mut self (ref_mut)"
    );

    // Return type must be Result<(), RegseqError>
    let ret = deinit_fn
        .return_type
        .as_ref()
        .expect("deinit must have a return type");
    assert!(
        matches!(ret, MirTy::Enum { path, .. } if path.contains("Result")),
        "deinit return type must be a Result enum, got: {:?}",
        ret
    );

    // No extra parameters
    assert!(
        deinit_fn.params.is_empty(),
        "deinit must have no extra parameters"
    );
}

// ---------------------------------------------------------------------------
// Check Group 2: Lifecycle callback bodies -- helper sequencing, error
// propagation, and success return
// ---------------------------------------------------------------------------

/// Extract ordered call names from MIR basic blocks.
/// Walks non-cleanup blocks and collects FnDef call target names in order.
fn extract_call_sequence(body: &MirBody) -> Vec<String> {
    let mut calls = Vec::new();
    for bb in &body.basic_blocks {
        if bb.is_cleanup {
            continue;
        }
        if let Some(ref term) = bb.terminator {
            if let MirTerminatorKind::Call {
                func: MirOperand::Constant { ref constant },
                ..
            } = term.kind
            {
                if let MirTy::Fn { ref path, .. } = constant.ty {
                    // Extract the short function name from the path
                    let short_name = path.rsplit("::").next().unwrap_or(path).to_string();
                    calls.push(short_name);
                }
            }
        }
    }
    calls
}

/// Check if a MIR body contains a `?`-style early error propagation pattern
/// after calling a given function name.
///
/// In post-desugaring MIR, the `?` operator on `Result` becomes:
///   1. `Call(fn)` -> target_bb
///   2. target_bb: `Call(Try::branch)` -> next_bb
///   3. next_bb: `Discriminant` + `SwitchInt` (Continue vs Break)
///   4. Break branch: `Call(FromResidual::from_residual)` -> early return
///
/// We follow the call chain forward up to a bounded depth looking for
/// the `Try::branch` -> discriminant+SwitchInt pattern.
fn has_error_propagation_after_call(body: &MirBody, fn_name: &str) -> bool {
    const MAX_HOPS: usize = 6;

    for bb in &body.basic_blocks {
        if bb.is_cleanup {
            continue;
        }
        let Some(ref term) = bb.terminator else {
            continue;
        };
        let MirTerminatorKind::Call {
            ref func,
            target: Some(target_bb),
            ..
        } = term.kind
        else {
            continue;
        };

        let is_target_fn = match func {
            MirOperand::Constant { ref constant } => match &constant.ty {
                MirTy::Fn { ref path, .. } => path.rsplit("::").next().unwrap_or(path) == fn_name,
                _ => false,
            },
            _ => false,
        };
        if !is_target_fn {
            continue;
        }

        // Walk forward from target_bb following Goto and Call chains.
        let mut cur = target_bb as usize;
        for _ in 0..MAX_HOPS {
            let Some(block) = body.basic_blocks.get(cur) else {
                break;
            };

            let has_discriminant = block.statements.iter().any(|s| {
                matches!(
                    &s.kind,
                    MirStatementKind::Assign {
                        rvalue: box MirRvalue::Discriminant { .. },
                        ..
                    }
                )
            });

            let has_switch = matches!(
                block.terminator.as_ref().map(|t| &t.kind),
                Some(MirTerminatorKind::SwitchInt { .. })
            );

            if has_discriminant && has_switch {
                return true;
            }

            // Follow Goto or Call-to-Try::branch to the next block.
            match block.terminator.as_ref().map(|t| &t.kind) {
                Some(MirTerminatorKind::Goto { target }) => {
                    cur = *target as usize;
                }
                Some(MirTerminatorKind::Call {
                    func: call_func,
                    target: Some(next),
                    ..
                }) => {
                    // Only follow through Try::branch calls, not
                    // arbitrary calls (which would cause false
                    // positives for infallible helpers that happen
                    // to precede a fallible one).
                    let is_try_branch = match call_func {
                        MirOperand::Constant { constant } => match &constant.ty {
                            MirTy::Fn { path, .. } => path.contains("Try::branch"),
                            _ => false,
                        },
                        _ => false,
                    };
                    if is_try_branch {
                        cur = *next as usize;
                    } else {
                        break;
                    }
                }
                _ => break,
            }
        }
    }
    false
}

/// Check if a MIR body has a success-path that returns Ok(()).
/// In MIR this appears as constructing an Adt with variant_index 0
/// (Ok variant) with a unit operand, followed eventually by Return.
fn has_ok_unit_return(body: &MirBody) -> bool {
    for bb in &body.basic_blocks {
        if bb.is_cleanup {
            continue;
        }
        for stmt in &bb.statements {
            if let MirStatementKind::Assign {
                rvalue:
                    box MirRvalue::Aggregate {
                        agg_kind: MirAggregateKind::Adt { variant_index, .. },
                        ..
                    },
                place,
                ..
            } = &stmt.kind
            {
                // variant_index 0 is Ok, and if it's assigned to _0
                // (the return place), it's the success path
                if *variant_index == 0 && place.local == 0 {
                    return true;
                }
            }
        }
    }
    false
}

#[test]
fn check_group_2_init_helper_call_order() {
    let surface = compile_and_extract_all(LIFECYCLE_SPECIMEN, "lifecycle_cg2_init_order");

    let init_fn = surface
        .bodies
        .functions
        .iter()
        .find(|f| f.name == "init")
        .expect("body surface must contain init function");

    // Verify helper_calls contains the expected helpers
    assert!(
        init_fn
            .helper_calls
            .contains(&"toggle_reset_if_present".to_string()),
        "init helper_calls must include toggle_reset_if_present"
    );
    assert!(
        init_fn
            .helper_calls
            .contains(&"apply_default_pins_if_present".to_string()),
        "init helper_calls must include apply_default_pins_if_present"
    );
    assert!(
        init_fn
            .helper_calls
            .contains(&"enable_clock_if_present".to_string()),
        "init helper_calls must include enable_clock_if_present"
    );
    assert!(
        init_fn
            .helper_calls
            .contains(&"program_polling_port".to_string()),
        "init helper_calls must include program_polling_port"
    );
    assert!(
        init_fn.helper_calls.contains(&"uart_clock_hz".to_string()),
        "init helper_calls must include uart_clock_hz"
    );

    // Verify call ordering in the MIR body
    let call_seq = extract_call_sequence(&init_fn.body);

    // Find positions of each helper in the sequence
    let pos_toggle = call_seq
        .iter()
        .position(|c| c == "toggle_reset_if_present")
        .expect("toggle_reset_if_present must appear in call sequence");
    let pos_pins = call_seq
        .iter()
        .position(|c| c == "apply_default_pins_if_present")
        .expect("apply_default_pins_if_present must appear in call sequence");
    let pos_clock = call_seq
        .iter()
        .position(|c| c == "enable_clock_if_present")
        .expect("enable_clock_if_present must appear in call sequence");
    let pos_program = call_seq
        .iter()
        .position(|c| c == "program_polling_port")
        .expect("program_polling_port must appear in call sequence");

    assert!(
        pos_toggle < pos_pins,
        "toggle_reset_if_present must come before apply_default_pins_if_present"
    );
    assert!(
        pos_pins < pos_clock,
        "apply_default_pins_if_present must come before enable_clock_if_present"
    );
    assert!(
        pos_clock < pos_program,
        "enable_clock_if_present must come before program_polling_port"
    );
}

#[test]
fn check_group_2_init_error_propagation() {
    let surface = compile_and_extract_all(LIFECYCLE_SPECIMEN, "lifecycle_cg2_init_errprop");

    let init_fn = surface
        .bodies
        .functions
        .iter()
        .find(|f| f.name == "init")
        .expect("body surface must contain init function");

    // First three calls must have `?` error propagation
    assert!(
        has_error_propagation_after_call(&init_fn.body, "toggle_reset_if_present"),
        "toggle_reset_if_present must have ? error propagation in init"
    );
    assert!(
        has_error_propagation_after_call(&init_fn.body, "apply_default_pins_if_present"),
        "apply_default_pins_if_present must have ? error propagation in init"
    );
    assert!(
        has_error_propagation_after_call(&init_fn.body, "enable_clock_if_present"),
        "enable_clock_if_present must have ? error propagation in init"
    );

    // program_polling_port is infallible -- no error propagation
    assert!(
        !has_error_propagation_after_call(&init_fn.body, "program_polling_port"),
        "program_polling_port must NOT have ? error propagation (infallible)"
    );
}

#[test]
fn check_group_2_init_success_return() {
    let surface = compile_and_extract_all(LIFECYCLE_SPECIMEN, "lifecycle_cg2_init_ok");

    let init_fn = surface
        .bodies
        .functions
        .iter()
        .find(|f| f.name == "init")
        .expect("body surface must contain init function");

    assert!(
        has_ok_unit_return(&init_fn.body),
        "init must have Ok(()) success return path"
    );
}

/// Find the Call terminator for a given function name in non-cleanup blocks.
/// Returns (args, destination) for the first matching call.
fn find_call_terminator(body: &MirBody, fn_name: &str) -> Option<(Vec<MirOperand>, MirPlace)> {
    for bb in &body.basic_blocks {
        if bb.is_cleanup {
            continue;
        }
        if let Some(ref term) = bb.terminator {
            if let MirTerminatorKind::Call {
                func: MirOperand::Constant { ref constant },
                ref args,
                ref destination,
                ..
            } = term.kind
            {
                if let MirTy::Fn { ref path, .. } = constant.ty {
                    let short_name = path.rsplit("::").next().unwrap_or(path);
                    if short_name == fn_name {
                        return Some((args.clone(), destination.clone()));
                    }
                }
            }
        }
    }
    None
}

/// Check whether an operand references a given local index (directly or
/// through projections).
fn operand_references_local(op: &MirOperand, local: u32) -> bool {
    match op {
        MirOperand::Copy { place } | MirOperand::Move { place } => place.local == local,
        MirOperand::Constant { .. } => false,
    }
}

/// Check whether an operand is a field access on a given local (e.g.
/// `self.current_speed` would be local 1 with a Field projection whose
/// field name contains "current_speed" or whose index matches).
fn operand_is_field_on_local(op: &MirOperand, local: u32) -> bool {
    match op {
        MirOperand::Copy { place } | MirOperand::Move { place } => {
            place.local == local
                && place
                    .projection
                    .iter()
                    .any(|p| matches!(p, MirProjectionElem::Field { .. }))
        }
        MirOperand::Constant { .. } => false,
    }
}

#[test]
fn check_group_2_init_argument_flow() {
    let surface = compile_and_extract_all(LIFECYCLE_SPECIMEN, "lifecycle_cg2_init_args");

    let init_fn = surface
        .bodies
        .functions
        .iter()
        .find(|f| f.name == "init")
        .expect("body surface must contain init function");

    // --- Part A: uart_clock_hz() return value flows into program_polling_port ---
    //
    // Find the uart_clock_hz Call terminator to learn which local receives
    // its return value.
    let (_, uart_clk_dest) = find_call_terminator(&init_fn.body, "uart_clock_hz")
        .expect("uart_clock_hz Call terminator must exist in init MIR");

    let uart_clk_local = uart_clk_dest.local;

    // Find the program_polling_port Call terminator and inspect its args.
    // MIR args for a method call are: [self_ref, arg0, arg1, ...]
    // In authored source: program_polling_port(self.uart_clock_hz(), self.current_speed)
    // So MIR args should be: [self_ref, <uart_clk_result>, <current_speed_field>]
    let (ppp_args, _) = find_call_terminator(&init_fn.body, "program_polling_port")
        .expect("program_polling_port Call terminator must exist in init MIR");

    // There should be at least 3 args: self, uart_clk, baud_rate
    assert!(
        ppp_args.len() >= 3,
        "program_polling_port must have at least 3 MIR args (self + 2 params), got {}",
        ppp_args.len()
    );

    // arg[1] (first non-self argument) must reference the local that
    // holds the return value of uart_clock_hz().
    assert!(
        operand_references_local(&ppp_args[1], uart_clk_local),
        "program_polling_port arg 1 must reference the uart_clock_hz() return \
         local (_{uart_clk_local}), but got: {:?}",
        ppp_args[1]
    );

    // --- Part B: self.current_speed flows as arg 2 ---
    //
    // In MIR, `self.current_speed` is local 1 (the self parameter in init,
    // which takes &mut self) with a Field projection. The self local in a
    // method body is local 1 (local 0 is the return place).
    //
    // We verify that arg[2] is a field access on a local that is either
    // local 1 directly (the self reference) or a deref of local 1.
    // After MIR lowering, `self.current_speed` may appear as a Copy/Move
    // of a temporary that was assigned from a field projection. We check
    // two strategies:
    //   (a) arg[2] is directly a field access on self (local 1)
    //   (b) arg[2] references a local that was assigned from a field on self

    let arg2 = &ppp_args[2];
    let arg2_is_field_on_self = operand_is_field_on_local(arg2, 1);

    // Strategy (b): if arg2 references a temporary, check if that
    // temporary was assigned from a field projection on self.
    let arg2_is_indirect_field = if !arg2_is_field_on_self {
        match arg2 {
            MirOperand::Copy { place } | MirOperand::Move { place } => {
                let target_local = place.local;
                // Search statements for an assignment to this local from
                // a field access on self (local 1 or a deref thereof).
                init_fn.body.basic_blocks.iter().any(|bb| {
                    if bb.is_cleanup {
                        return false;
                    }
                    bb.statements.iter().any(|stmt| {
                        if let MirStatementKind::Assign {
                            place: assign_place,
                            rvalue,
                        } = &stmt.kind
                        {
                            if assign_place.local != target_local {
                                return false;
                            }
                            match rvalue.as_ref() {
                                MirRvalue::Use { operand } => operand_is_field_on_local(operand, 1),
                                MirRvalue::CopyForDeref { place: src } => {
                                    src.local == 1
                                        && src
                                            .projection
                                            .iter()
                                            .any(|p| matches!(p, MirProjectionElem::Field { .. }))
                                }
                                _ => false,
                            }
                        } else {
                            false
                        }
                    })
                })
            }
            MirOperand::Constant { .. } => false,
        }
    } else {
        false
    };

    assert!(
        arg2_is_field_on_self || arg2_is_indirect_field,
        "program_polling_port arg 2 must reference self.current_speed \
         (a field on the self parameter, local 1), but got: {:?}",
        arg2
    );
}

#[test]
fn check_group_2_deinit_helper_call_order() {
    let surface = compile_and_extract_all(LIFECYCLE_SPECIMEN, "lifecycle_cg2_deinit_order");

    let deinit_fn = surface
        .bodies
        .functions
        .iter()
        .find(|f| f.name == "deinit")
        .expect("body surface must contain deinit function");

    // Verify helper_calls contains the expected helpers
    assert!(
        deinit_fn
            .helper_calls
            .contains(&"shutdown_port".to_string()),
        "deinit helper_calls must include shutdown_port"
    );
    assert!(
        deinit_fn
            .helper_calls
            .contains(&"disable_clock_if_present".to_string()),
        "deinit helper_calls must include disable_clock_if_present"
    );
    assert!(
        deinit_fn
            .helper_calls
            .contains(&"apply_sleep_pins_if_present".to_string()),
        "deinit helper_calls must include apply_sleep_pins_if_present"
    );

    // Verify call ordering in the MIR body
    let call_seq = extract_call_sequence(&deinit_fn.body);

    let pos_shutdown = call_seq
        .iter()
        .position(|c| c == "shutdown_port")
        .expect("shutdown_port must appear in deinit call sequence");
    let pos_disable_clk = call_seq
        .iter()
        .position(|c| c == "disable_clock_if_present")
        .expect("disable_clock_if_present must appear in deinit call sequence");
    let pos_sleep_pins = call_seq
        .iter()
        .position(|c| c == "apply_sleep_pins_if_present")
        .expect("apply_sleep_pins_if_present must appear in deinit call sequence");

    assert!(
        pos_shutdown < pos_disable_clk,
        "shutdown_port must come before disable_clock_if_present"
    );
    assert!(
        pos_disable_clk < pos_sleep_pins,
        "disable_clock_if_present must come before apply_sleep_pins_if_present"
    );
}

#[test]
fn check_group_2_deinit_error_propagation() {
    let surface = compile_and_extract_all(LIFECYCLE_SPECIMEN, "lifecycle_cg2_deinit_errprop");

    let deinit_fn = surface
        .bodies
        .functions
        .iter()
        .find(|f| f.name == "deinit")
        .expect("body surface must contain deinit function");

    // shutdown_port is infallible -- no error propagation
    assert!(
        !has_error_propagation_after_call(&deinit_fn.body, "shutdown_port"),
        "shutdown_port must NOT have ? error propagation (infallible)"
    );

    // disable_clock_if_present is infallible -- no error propagation
    assert!(
        !has_error_propagation_after_call(&deinit_fn.body, "disable_clock_if_present"),
        "disable_clock_if_present must NOT have ? error propagation (infallible)"
    );

    // apply_sleep_pins_if_present is fallible -- must have error propagation
    assert!(
        has_error_propagation_after_call(&deinit_fn.body, "apply_sleep_pins_if_present"),
        "apply_sleep_pins_if_present must have ? error propagation in deinit"
    );
}

#[test]
fn check_group_2_deinit_success_return() {
    let surface = compile_and_extract_all(LIFECYCLE_SPECIMEN, "lifecycle_cg2_deinit_ok");

    let deinit_fn = surface
        .bodies
        .functions
        .iter()
        .find(|f| f.name == "deinit")
        .expect("body surface must contain deinit function");

    assert!(
        has_ok_unit_return(&deinit_fn.body),
        "deinit must have Ok(()) success return path"
    );
}
