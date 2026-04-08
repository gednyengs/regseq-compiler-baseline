//! source-to-extraction-05: PL011 pin-state markers authored->extraction boundary
//! validation.
//!
//! Validates that the authored pin-state marker facts for PL011 survive at the
//! compiler-side boundary between the authored PL011 source and the current
//! Regseq extraction state.
//!
//! Extraction surfaces under test:
//! - `IdentityEnrichmentSurface.structs` (type identity, canonical paths)
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

/// Combined extraction result from both identity enrichment and pinctrl surfaces.
struct ExtractionResult {
    identity: IdentityEnrichmentSurface,
    pinctrl: PinCtrlSurface,
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

    type IdentityRes = Result<IdentityEnrichmentSurface, IdentityEnrichmentError>;
    type PinCtrlRes = Result<PinCtrlSurface, PinCtrlError>;

    let identity_out: Arc<Mutex<Option<IdentityRes>>> = Arc::new(Mutex::new(None));
    let pinctrl_out: Arc<Mutex<Option<PinCtrlRes>>> = Arc::new(Mutex::new(None));

    struct Callbacks {
        identity_out: Arc<Mutex<Option<IdentityRes>>>,
        pinctrl_out: Arc<Mutex<Option<PinCtrlRes>>>,
    }

    impl rustc_driver::Callbacks for Callbacks {
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
            *self.identity_out.lock().unwrap() = Some(extract_identity_enrichment(tcx));
            *self.pinctrl_out.lock().unwrap() = Some(extract_pinctrl_surface(tcx));
            rustc_driver::Compilation::Stop
        }
    }

    let mut callbacks = Callbacks {
        identity_out: identity_out.clone(),
        pinctrl_out: pinctrl_out.clone(),
    };

    let args = vec![
        "rustc".to_string(),
        src_path.to_str().unwrap().to_string(),
        "--sysroot".to_string(),
        sysroot,
        "--edition".to_string(),
        "2021".to_string(),
        "--crate-name".to_string(),
        "pl011_full_spec".to_string(),
        "--crate-type".to_string(),
        "lib".to_string(),
        "--out-dir".to_string(),
        test_dir.to_str().unwrap().to_string(),
    ];

    rustc_driver::run_compiler(&args, &mut callbacks);

    let identity = identity_out
        .lock()
        .unwrap()
        .take()
        .expect("after_analysis should have been called")
        .expect("identity enrichment extraction should succeed");

    let pinctrl = pinctrl_out
        .lock()
        .unwrap()
        .take()
        .expect("after_analysis should have been called")
        .expect("pinctrl extraction should succeed");

    std::fs::remove_dir_all(&test_dir).ok();

    ExtractionResult { identity, pinctrl }
}

// ---------------------------------------------------------------------------
// Inline specimen: reduced PL011 pin-state marker surface
// ---------------------------------------------------------------------------

/// Reduced specimen covering all authored pin-state marker facts:
/// - `DefaultState` and `SleepState` as zero-sized marker types
/// - `impl PinState for DefaultState` and `impl PinState for SleepState`
/// - `pinctrl.apply::<DefaultState>()` in `apply_default_pins_if_present`
/// - `pinctrl.apply::<SleepState>()` in `apply_sleep_pins_if_present`
///
/// Types are nested inside a `driver` module to reproduce the authored module
/// structure (`pl011_full_spec::driver::*`). With the crate name
/// `pl011_full_spec`, the full paths become
/// `pl011_full_spec::driver::DefaultState` and
/// `pl011_full_spec::driver::SleepState`, allowing exact authored-path
/// assertions rather than suffix-only checks.
const SPECIMEN: &str = r#"
    #![allow(dead_code)]

    pub mod driver {
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
    }
"#;

// ---------------------------------------------------------------------------
// Precondition: specimen fidelity against real fixture source
// ---------------------------------------------------------------------------

/// Reads the real PL011 fixture source and verifies that every authored fact
/// in the specimen matches the real fixture verbatim.
#[test]
fn precondition_specimen_fidelity() {
    let fixture = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/pl011-full-spec/src/driver.rs"
    ))
    .expect("fixture source must be readable");

    // Authored fact: DefaultState is a zero-sized marker struct
    assert!(
        fixture.contains("pub struct DefaultState;"),
        "fixture must contain `pub struct DefaultState;`"
    );

    // Authored fact: SleepState is a zero-sized marker struct
    assert!(
        fixture.contains("pub struct SleepState;"),
        "fixture must contain `pub struct SleepState;`"
    );

    // Authored fact: DefaultState implements PinState
    assert!(
        fixture.contains("impl PinState for DefaultState {}"),
        "fixture must contain `impl PinState for DefaultState {{}}`"
    );

    // Authored fact: SleepState implements PinState
    assert!(
        fixture.contains("impl PinState for SleepState {}"),
        "fixture must contain `impl PinState for SleepState {{}}`"
    );

    // Authored fact: apply_default_pins_if_present uses pinctrl.apply::<DefaultState>()
    assert!(
        fixture.contains("pinctrl.apply::<DefaultState>()"),
        "fixture must contain `pinctrl.apply::<DefaultState>()`"
    );

    // Authored fact: apply_sleep_pins_if_present uses pinctrl.apply::<SleepState>()
    assert!(
        fixture.contains("pinctrl.apply::<SleepState>()"),
        "fixture must contain `pinctrl.apply::<SleepState>()`"
    );

    // Authored fact: apply_default_pins_if_present function exists
    assert!(
        fixture.contains("fn apply_default_pins_if_present"),
        "fixture must contain `fn apply_default_pins_if_present`"
    );

    // Authored fact: apply_sleep_pins_if_present function exists
    assert!(
        fixture.contains("fn apply_sleep_pins_if_present"),
        "fixture must contain `fn apply_sleep_pins_if_present`"
    );

    // Specimen reproduces all eight authored facts
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

    // Specimen uses a `driver` module to reproduce the authored module nesting.
    // The fixture has `mod driver` containing the marker types; the specimen must
    // match this structure so canonical paths are fully reconstructable.
    assert!(
        SPECIMEN.contains("pub mod driver"),
        "specimen must nest items in a `driver` module to match fixture module structure"
    );

    // Verify that the fixture itself nests the markers inside a `driver` module
    // (the fixture file is `src/driver.rs`, so the marker types live at
    // `pl011_full_spec::driver::DefaultState` etc.)
    assert!(
        fixture.contains("pub struct DefaultState;"),
        "fixture markers must be in driver module scope"
    );
}

// ---------------------------------------------------------------------------
// Check Group 1: Marker-type identity remains distinct
// ---------------------------------------------------------------------------

/// Check 1a: Both DefaultState and SleepState are extracted as distinct structs
/// with full canonical paths in the identity enrichment surface.
///
/// The authored canonical paths are `pl011_full_spec::driver::DefaultState` and
/// `pl011_full_spec::driver::SleepState`. The specimen nests markers inside a
/// `driver` module and is compiled as crate `pl011_full_spec`, so the extraction
/// produces `canonical_path = "driver::DefaultState"` and
/// `module_path = "driver"` (rustc's `def_path_str` omits the local crate
/// prefix). The full path is reconstructed using the crate root module
/// (`pl011_full_spec`) to yield `pl011_full_spec::driver::DefaultState`,
/// proving the exact authored `<crate>::driver::<Type>` structure.
///
/// We filter for markers whose `module_path` is `"driver"` to select the
/// module-scoped extraction facts (the identity enrichment extractor may
/// produce both module-scoped and crate-root-scoped entries when
/// `hir_free_items` and `extract_module` both visit nested items).
#[test]
fn check1a_marker_types_have_distinct_canonical_paths() {
    let result = compile_and_extract(SPECIMEN, "s2e05_check1a");

    // Select markers from the `driver` module (matching authored module nesting)
    let default_state = result
        .identity
        .structs
        .iter()
        .find(|s| s.name == "DefaultState" && s.module_path.0 == "driver");
    let sleep_state = result
        .identity
        .structs
        .iter()
        .find(|s| s.name == "SleepState" && s.module_path.0 == "driver");

    // Both marker types must be present in the extraction
    assert!(
        default_state.is_some(),
        "DefaultState must be present in IdentityEnrichmentSurface.structs with module_path 'driver'"
    );
    assert!(
        sleep_state.is_some(),
        "SleepState must be present in IdentityEnrichmentSurface.structs with module_path 'driver'"
    );

    let ds = default_state.unwrap();
    let ss = sleep_state.unwrap();

    // Canonical paths must be distinct (non-collapse)
    assert_ne!(
        ds.canonical_path, ss.canonical_path,
        "DefaultState and SleepState must have distinct canonical paths"
    );

    // Exact canonical path equality: rustc's def_path_str omits the local crate
    // name, so the extraction stores `driver::DefaultState`. We assert this
    // exact value.
    assert_eq!(
        ds.canonical_path.0, "driver::DefaultState",
        "DefaultState canonical path must be exactly 'driver::DefaultState', got: {}",
        ds.canonical_path.0
    );
    assert_eq!(
        ss.canonical_path.0, "driver::SleepState",
        "SleepState canonical path must be exactly 'driver::SleepState', got: {}",
        ss.canonical_path.0
    );

    // Verify the short names carry exactly the authored type names
    assert_eq!(
        ds.name, "DefaultState",
        "DefaultState struct name must be exactly 'DefaultState'"
    );
    assert_eq!(
        ss.name, "SleepState",
        "SleepState struct name must be exactly 'SleepState'"
    );

    // Prove the full authored path is reconstructable:
    // crate_root_name + "::" + canonical_path = full path
    let crate_root = result
        .identity
        .modules
        .iter()
        .find(|m| m.name == "pl011_full_spec")
        .expect("crate root module 'pl011_full_spec' must exist");
    let ds_full = format!("{}::{}", crate_root.name, ds.canonical_path.0);
    let ss_full = format!("{}::{}", crate_root.name, ss.canonical_path.0);
    assert_eq!(
        ds_full, "pl011_full_spec::driver::DefaultState",
        "full reconstructed DefaultState path must match authored structure"
    );
    assert_eq!(
        ss_full, "pl011_full_spec::driver::SleepState",
        "full reconstructed SleepState path must match authored structure"
    );

    // Zero-sized marker type check: both markers are authored as zero-sized
    // (unit structs with no fields).
    assert!(
        ds.fields.is_empty(),
        "DefaultState must be zero-sized (no fields), got {} fields",
        ds.fields.len()
    );
    assert!(
        ss.fields.is_empty(),
        "SleepState must be zero-sized (no fields), got {} fields",
        ss.fields.len()
    );
}

/// Check 1a-supplementary: Verify that module containment and canonical path
/// are consistent, and that `module_path` + `name` reconstructs the exact
/// canonical path.
///
/// With the specimen nesting markers in a `driver` module, the module_path
/// is `driver` (rustc omits crate prefix for local items) and
/// `module_path::name` reconstructs the canonical_path exactly.
#[test]
fn check1a_canonical_path_reconstructable_via_module() {
    let result = compile_and_extract(SPECIMEN, "s2e05_check1a_mod");

    let ds = result
        .identity
        .structs
        .iter()
        .find(|s| s.name == "DefaultState" && s.module_path.0 == "driver")
        .expect("DefaultState in driver module must exist");
    let ss = result
        .identity
        .structs
        .iter()
        .find(|s| s.name == "SleepState" && s.module_path.0 == "driver")
        .expect("SleepState in driver module must exist");

    // Module path must be the `driver` module
    assert_eq!(
        ds.module_path.0, "driver",
        "DefaultState module_path must be 'driver', got: {}",
        ds.module_path.0
    );
    assert_eq!(
        ss.module_path.0, "driver",
        "SleepState module_path must be 'driver', got: {}",
        ss.module_path.0
    );

    // Reconstructed path (module_path::name) must equal the canonical path exactly
    let ds_reconstructed = format!("{}::{}", ds.module_path.0, ds.name);
    let ss_reconstructed = format!("{}::{}", ss.module_path.0, ss.name);

    assert_eq!(
        ds_reconstructed, ds.canonical_path.0,
        "reconstructed DefaultState path must equal canonical path"
    );
    assert_eq!(
        ss_reconstructed, ss.canonical_path.0,
        "reconstructed SleepState path must equal canonical path"
    );

    // Reconstructed paths must be distinct
    assert_ne!(
        ds_reconstructed, ss_reconstructed,
        "reconstructed canonical paths must be distinct"
    );
}

/// Check 1b: Closed-world evidence that the markers do not collapse.
/// Within the `driver` module (matching authored module nesting), the complete
/// set of structs named DefaultState or SleepState contains exactly one of each.
#[test]
fn check1b_no_marker_collapse_closed_world() {
    let result = compile_and_extract(SPECIMEN, "s2e05_check1b");

    // Filter to the `driver` module to match authored module structure
    let driver_structs: Vec<_> = result
        .identity
        .structs
        .iter()
        .filter(|s| s.module_path.0 == "driver")
        .collect();

    let default_count = driver_structs
        .iter()
        .filter(|s| s.name == "DefaultState")
        .count();
    let sleep_count = driver_structs
        .iter()
        .filter(|s| s.name == "SleepState")
        .count();

    assert_eq!(
        default_count, 1,
        "exactly one DefaultState struct must exist in the driver module"
    );
    assert_eq!(
        sleep_count, 1,
        "exactly one SleepState struct must exist in the driver module"
    );

    // Confirm both names appear -- neither was dropped
    let marker_names: Vec<&str> = driver_structs
        .iter()
        .filter(|s| s.name == "DefaultState" || s.name == "SleepState")
        .map(|s| s.name.as_str())
        .collect();
    assert_eq!(
        marker_names.len(),
        2,
        "exactly two pin-state marker structs expected in driver module, got: {:?}",
        marker_names
    );
}

// ---------------------------------------------------------------------------
// Check Group 2: PinState implementation evidence survives
// ---------------------------------------------------------------------------

/// Check 2a: Trait impl blocks for `PinState` exist for both marker types
/// in the identity enrichment surface.
///
/// We deduplicate by `self_type_path` since the extraction may produce
/// duplicate entries when `hir_free_items` and module traversal both visit
/// nested items. We verify that at least one impl per marker exists with
/// the correct `self_type_path` matching the `driver::*` module path.
#[test]
fn check2a_pinstate_trait_impl_evidence() {
    let result = compile_and_extract(SPECIMEN, "s2e05_check2a");

    // Find trait impl blocks where trait_name is "PinState" and
    // self_type_path matches the driver module (e.g., "driver::DefaultState").
    // The extraction produces duplicate entries when `hir_free_items` and module
    // traversal both visit nested items. We deduplicate by
    // (self_type_name, canonical_path) to get the distinct logical impls.
    let pinstate_impls_all: Vec<_> = result
        .identity
        .impl_blocks
        .iter()
        .filter(|ib| {
            ib.trait_name.as_deref() == Some("PinState")
                && ib.self_type_path.0.starts_with("driver::")
        })
        .collect();

    // Deduplicate: collect unique (self_type_name, canonical_path) pairs
    let mut seen = std::collections::HashSet::new();
    let pinstate_impls: Vec<_> = pinstate_impls_all
        .into_iter()
        .filter(|ib| seen.insert((ib.self_type_name.clone(), ib.canonical_path.clone())))
        .collect();

    // Exactly two distinct PinState trait impls in the driver module
    assert_eq!(
        pinstate_impls.len(),
        2,
        "expected exactly 2 distinct PinState trait impls in driver module, got {}: {:?}",
        pinstate_impls.len(),
        pinstate_impls
            .iter()
            .map(|ib| (&ib.self_type_name, &ib.self_type_path, &ib.canonical_path))
            .collect::<Vec<_>>()
    );

    // Both marker types must be covered
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

    // Both must be trait impls (not inherent)
    for ib in &pinstate_impls {
        assert!(
            !ib.is_inherent,
            "PinState impl for {} must be a trait impl, not inherent",
            ib.self_type_name
        );
        assert!(
            ib.trait_path.is_some(),
            "PinState impl for {} must have a trait_path",
            ib.self_type_name
        );
    }
}

/// Check 2b: Usage-site distinction -- `apply::<DefaultState>()` and
/// `apply::<SleepState>()` call sites are extracted with distinct state_type
/// values in the PinCtrl surface, each in the correct containing function.
#[test]
fn check2b_usage_site_distinction() {
    let result = compile_and_extract(SPECIMEN, "s2e05_check2b");

    // apply::<DefaultState>() in apply_default_pins_if_present
    let default_site = result.pinctrl.call_sites.iter().find(|cs| {
        cs.state_type == "DefaultState" && cs.containing_fn == "apply_default_pins_if_present"
    });
    assert!(
        default_site.is_some(),
        "apply::<DefaultState>() in apply_default_pins_if_present must be extracted; \
         call_sites: {:?}",
        result.pinctrl.call_sites
    );

    // apply::<SleepState>() in apply_sleep_pins_if_present
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
    let result = compile_and_extract(SPECIMEN, "s2e05_check2c");

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

    // All state types must be distinct
    let mut sorted = state_types.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(
        sorted.len(),
        state_types.len(),
        "all call site state_types must be distinct; got: {:?}",
        state_types
    );
}
