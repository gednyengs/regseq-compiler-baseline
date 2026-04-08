//! source-to-extraction-05: PL011 pin-state markers authored->extraction boundary
//! validation.
//!
//! Validates that the authored pin-state marker facts for PL011 survive at the
//! compiler-side boundary between the authored PL011 source and the current
//! Regseq extraction state.
//!
//! Extraction surfaces under test:
//! - `IdentityEnrichmentSurface.structs` (type identity)
//! - `IdentityEnrichmentSurface.impl_blocks` (trait implementation evidence)
//! - `PinCtrlSurface.call_sites` (usage-site distinction)

#![feature(rustc_private)]

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

#[path = "../src/extract/pinctrl.rs"]
mod pinctrl;

use identity_enrichment::*;
use pinctrl::*;

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

struct ExtractionBoth {
    identity: IdentityEnrichmentSurface,
    pinctrl: PinCtrlSurface,
}

fn compile_and_extract_both(source: &str, test_name: &str) -> ExtractionBoth {
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
    type PinCtrlResult = Result<PinCtrlSurface, PinCtrlError>;

    let identity_result: Arc<Mutex<Option<IdentityResult>>> = Arc::new(Mutex::new(None));
    let pinctrl_result: Arc<Mutex<Option<PinCtrlResult>>> = Arc::new(Mutex::new(None));

    struct ExtractCallbacks {
        identity_result: Arc<Mutex<Option<IdentityResult>>>,
        pinctrl_result: Arc<Mutex<Option<PinCtrlResult>>>,
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
            *self.pinctrl_result.lock().unwrap() = Some(extract_pinctrl_surface(tcx));
            rustc_driver::Compilation::Stop
        }
    }

    let mut callbacks = ExtractCallbacks {
        identity_result: identity_result.clone(),
        pinctrl_result: pinctrl_result.clone(),
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
        .expect("after_analysis should have been called")
        .expect("identity enrichment extraction should succeed");

    let pinctrl = pinctrl_result
        .lock()
        .unwrap()
        .take()
        .expect("after_analysis should have been called")
        .expect("pinctrl extraction should succeed");

    std::fs::remove_dir_all(&test_dir).ok();

    ExtractionBoth { identity, pinctrl }
}

// ---------------------------------------------------------------------------
// Inline specimen: reduced PL011 pin-state marker surface
// ---------------------------------------------------------------------------

/// Reduced specimen covering all authored pin-state marker facts:
/// - `DefaultState` and `SleepState` as zero-sized marker types
/// - `impl PinState for DefaultState` and `impl PinState for SleepState`
/// - `pinctrl.apply::<DefaultState>()` in `apply_default_pins_if_present`
/// - `pinctrl.apply::<SleepState>()` in `apply_sleep_pins_if_present`
const SPECIMEN: &str = r#"
    #![allow(dead_code)]

    pub enum RegseqError { HardwareError }

    pub trait PinState {}

    pub struct PinCtrl { _opaque: () }
    impl PinCtrl {
        pub fn apply<S: PinState>(&self) -> Result<(), RegseqError> { unimplemented!() }
    }

    pub struct DefaultState;
    impl PinState for DefaultState {}

    pub struct SleepState;
    impl PinState for SleepState {}

    #[rseq::driver(name = "pl011-full-spec", compatible = ["arm,pl011"])]
    pub struct Pl011Device {
        #[rseq::pinctrl(id = "uart_pins")]
        pub pinctrl: Option<PinCtrl>,
    }

    impl Pl011Device {
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

    // Fact 1: DefaultState is a zero-sized marker struct
    assert!(
        fixture.contains("pub struct DefaultState;"),
        "fixture must contain `pub struct DefaultState;`"
    );

    // Fact 2: SleepState is a zero-sized marker struct
    assert!(
        fixture.contains("pub struct SleepState;"),
        "fixture must contain `pub struct SleepState;`"
    );

    // Fact 3: DefaultState implements PinState
    assert!(
        fixture.contains("impl PinState for DefaultState {}"),
        "fixture must contain `impl PinState for DefaultState {{}}`"
    );

    // Fact 4: SleepState implements PinState
    assert!(
        fixture.contains("impl PinState for SleepState {}"),
        "fixture must contain `impl PinState for SleepState {{}}`"
    );

    // Fact 5: apply_default_pins_if_present uses pinctrl.apply::<DefaultState>()
    assert!(
        fixture.contains("pinctrl.apply::<DefaultState>()"),
        "fixture must contain `pinctrl.apply::<DefaultState>()`"
    );

    // Fact 6: apply_sleep_pins_if_present uses pinctrl.apply::<SleepState>()
    assert!(
        fixture.contains("pinctrl.apply::<SleepState>()"),
        "fixture must contain `pinctrl.apply::<SleepState>()`"
    );

    // Fact 7: apply_default_pins_if_present function exists
    assert!(
        fixture.contains("fn apply_default_pins_if_present"),
        "fixture must contain `fn apply_default_pins_if_present`"
    );

    // Fact 8: apply_sleep_pins_if_present function exists
    assert!(
        fixture.contains("fn apply_sleep_pins_if_present"),
        "fixture must contain `fn apply_sleep_pins_if_present`"
    );

    // Verify specimen faithfully reproduces these facts
    assert!(
        SPECIMEN.contains("pub struct DefaultState;"),
        "specimen must contain `pub struct DefaultState;`"
    );
    assert!(
        SPECIMEN.contains("pub struct SleepState;"),
        "specimen must contain `pub struct SleepState;`"
    );
    assert!(
        SPECIMEN.contains("impl PinState for DefaultState {}"),
        "specimen must contain `impl PinState for DefaultState {{}}`"
    );
    assert!(
        SPECIMEN.contains("impl PinState for SleepState {}"),
        "specimen must contain `impl PinState for SleepState {{}}`"
    );
    assert!(
        SPECIMEN.contains("pinctrl.apply::<DefaultState>()"),
        "specimen must contain `pinctrl.apply::<DefaultState>()`"
    );
    assert!(
        SPECIMEN.contains("pinctrl.apply::<SleepState>()"),
        "specimen must contain `pinctrl.apply::<SleepState>()`"
    );
    assert!(
        SPECIMEN.contains("fn apply_default_pins_if_present"),
        "specimen must contain `fn apply_default_pins_if_present`"
    );
    assert!(
        SPECIMEN.contains("fn apply_sleep_pins_if_present"),
        "specimen must contain `fn apply_sleep_pins_if_present`"
    );
}

// ---------------------------------------------------------------------------
// Check Group 1: Marker-type identity remains distinct
// ---------------------------------------------------------------------------

/// Check 1a: Both DefaultState and SleepState are extracted as distinct structs
/// with distinct canonical paths in the identity enrichment surface.
#[test]
fn check1a_marker_types_are_distinct_structs() {
    let result = compile_and_extract_both(SPECIMEN, "pin_state_check1a");

    let default_state = result
        .identity
        .structs
        .iter()
        .find(|s| s.name == "DefaultState");
    let sleep_state = result
        .identity
        .structs
        .iter()
        .find(|s| s.name == "SleepState");

    // Both marker types must be present
    assert!(
        default_state.is_some(),
        "DefaultState must be present in identity enrichment structs"
    );
    assert!(
        sleep_state.is_some(),
        "SleepState must be present in identity enrichment structs"
    );

    let default_state = default_state.unwrap();
    let sleep_state = sleep_state.unwrap();

    // Their canonical paths must be distinct
    assert_ne!(
        default_state.canonical_path, sleep_state.canonical_path,
        "DefaultState and SleepState must have distinct canonical paths"
    );

    // Canonical paths must contain the type names
    assert!(
        default_state.canonical_path.0.contains("DefaultState"),
        "DefaultState canonical path must contain 'DefaultState', got: {}",
        default_state.canonical_path.0
    );
    assert!(
        sleep_state.canonical_path.0.contains("SleepState"),
        "SleepState canonical path must contain 'SleepState', got: {}",
        sleep_state.canonical_path.0
    );
}

/// Check 1b: Closed-world evidence that the markers do not collapse --
/// the complete set of structs named DefaultState or SleepState contains
/// exactly one of each.
#[test]
fn check1b_no_marker_collapse() {
    let result = compile_and_extract_both(SPECIMEN, "pin_state_check1b");

    let default_count = result
        .identity
        .structs
        .iter()
        .filter(|s| s.name == "DefaultState")
        .count();
    let sleep_count = result
        .identity
        .structs
        .iter()
        .filter(|s| s.name == "SleepState")
        .count();

    assert_eq!(
        default_count, 1,
        "exactly one DefaultState struct must exist (closed-world)"
    );
    assert_eq!(
        sleep_count, 1,
        "exactly one SleepState struct must exist (closed-world)"
    );

    // Additionally verify the total marker set by checking that both names
    // appear in the struct list -- neither was dropped
    let marker_names: Vec<&str> = result
        .identity
        .structs
        .iter()
        .filter(|s| s.name == "DefaultState" || s.name == "SleepState")
        .map(|s| s.name.as_str())
        .collect();
    assert_eq!(
        marker_names.len(),
        2,
        "exactly two pin-state marker structs expected, got: {:?}",
        marker_names
    );
}

// ---------------------------------------------------------------------------
// Check Group 2: PinState implementation evidence survives
// ---------------------------------------------------------------------------

/// Check 2a: Trait impl blocks for `PinState` exist for both marker types
/// in the identity enrichment surface.
#[test]
fn check2a_pinstate_trait_impl_evidence() {
    let result = compile_and_extract_both(SPECIMEN, "pin_state_check2a");

    // Find trait impl blocks where trait_name is "PinState"
    let pinstate_impls: Vec<_> = result
        .identity
        .impl_blocks
        .iter()
        .filter(|ib| ib.trait_name.as_deref() == Some("PinState"))
        .collect();

    // There must be exactly two: one for DefaultState, one for SleepState
    assert_eq!(
        pinstate_impls.len(),
        2,
        "expected exactly 2 PinState trait impls, got {}: {:?}",
        pinstate_impls.len(),
        pinstate_impls
            .iter()
            .map(|ib| &ib.self_type_name)
            .collect::<Vec<_>>()
    );

    // Verify both marker types are covered
    let impl_self_types: Vec<&str> = pinstate_impls
        .iter()
        .map(|ib| ib.self_type_name.as_str())
        .collect();

    assert!(
        impl_self_types.contains(&"DefaultState"),
        "PinState impl for DefaultState must exist; found: {:?}",
        impl_self_types
    );
    assert!(
        impl_self_types.contains(&"SleepState"),
        "PinState impl for SleepState must exist; found: {:?}",
        impl_self_types
    );

    // Verify is_inherent is false for both (they are trait impls)
    for ib in &pinstate_impls {
        assert!(
            !ib.is_inherent,
            "PinState impl for {} must not be inherent",
            ib.self_type_name
        );
    }
}

/// Check 2b: Usage-site distinction -- `apply::<DefaultState>()` and
/// `apply::<SleepState>()` call sites are extracted with distinct state_type
/// in the PinCtrl surface.
#[test]
fn check2b_usage_site_distinction() {
    let result = compile_and_extract_both(SPECIMEN, "pin_state_check2b");

    // Verify apply::<DefaultState>() in apply_default_pins_if_present
    let default_site = result.pinctrl.call_sites.iter().find(|cs| {
        cs.state_type == "DefaultState" && cs.containing_fn == "apply_default_pins_if_present"
    });
    assert!(
        default_site.is_some(),
        "apply::<DefaultState>() in apply_default_pins_if_present must be extracted; \
         call_sites: {:?}",
        result.pinctrl.call_sites
    );

    // Verify apply::<SleepState>() in apply_sleep_pins_if_present
    let sleep_site = result.pinctrl.call_sites.iter().find(|cs| {
        cs.state_type == "SleepState" && cs.containing_fn == "apply_sleep_pins_if_present"
    });
    assert!(
        sleep_site.is_some(),
        "apply::<SleepState>() in apply_sleep_pins_if_present must be extracted; \
         call_sites: {:?}",
        result.pinctrl.call_sites
    );

    // Closed-world: exactly two pinctrl call sites, no duplication or loss
    assert_eq!(
        result.pinctrl.call_sites.len(),
        2,
        "exactly 2 pinctrl call sites expected (closed-world); got: {:?}",
        result.pinctrl.call_sites
    );
}

/// Check 2c: The state_type values at the call sites are distinct, proving
/// downstream pinctrl state application can still distinguish the two markers.
#[test]
fn check2c_call_site_state_types_are_distinct() {
    let result = compile_and_extract_both(SPECIMEN, "pin_state_check2c");

    let state_types: Vec<&str> = result
        .pinctrl
        .call_sites
        .iter()
        .map(|cs| cs.state_type.as_str())
        .collect();

    assert!(
        state_types.contains(&"DefaultState"),
        "DefaultState must appear in call site state_types"
    );
    assert!(
        state_types.contains(&"SleepState"),
        "SleepState must appear in call site state_types"
    );

    // Distinct: no two call sites share the same state_type
    let unique_count = {
        let mut v = state_types.clone();
        v.sort();
        v.dedup();
        v.len()
    };
    assert_eq!(
        unique_count,
        state_types.len(),
        "all call site state_types must be distinct; got: {:?}",
        state_types
    );
}
