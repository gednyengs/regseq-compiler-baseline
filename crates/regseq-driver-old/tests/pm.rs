#![feature(rustc_private)]

extern crate rustc_ast;
extern crate rustc_driver;
extern crate rustc_hir;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_session;

use std::collections::HashSet;
use std::sync::{Arc, Mutex, Once};

#[path = "../src/extract/callback_context.rs"]
mod callback_context;

#[path = "../src/extract/traits.rs"]
mod traits;

#[path = "../src/extract/pm.rs"]
mod pm;

use pm::*;

/// Combined extraction result: PM gate surface + set of driver types with PM impls.
struct CombinedResult {
    pm_result: Result<PmGateSurface, PmError>,
}

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

fn compile_and_extract(source: &str, test_name: &str) -> CombinedResult {
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

    let result: Arc<Mutex<Option<CombinedResult>>> = Arc::new(Mutex::new(None));

    struct ExtractCallbacks {
        result: Arc<Mutex<Option<CombinedResult>>>,
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
            // First extract trait surface to get PM impl types.
            let trait_surface = traits::extract_trait_surface(tcx);
            let pm_impl_types: HashSet<String> = match &trait_surface {
                Ok(s) => s
                    .power_management_impls
                    .iter()
                    .map(|i| i.driver_type.clone())
                    .collect(),
                Err(_) => HashSet::new(),
            };

            let pm_result = extract_pm_surface(tcx, &pm_impl_types);

            *self.result.lock().unwrap() = Some(CombinedResult { pm_result });
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

    let extracted = result
        .lock()
        .unwrap()
        .take()
        .expect("after_analysis should have been called");

    std::fs::remove_dir_all(&test_dir).ok();

    extracted
}

// ---------------------------------------------------------------------------
// Common fixture preamble: fake regseq module with PowerManagement
// ---------------------------------------------------------------------------

const FIXTURE_TYPES: &str = r#"
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum RegseqError {
        HardwareError,
    }
"#;

const PM_TRAIT: &str = r#"
    pub trait PowerManagement {
        fn suspend(&mut self) -> Result<(), RegseqError>;
        fn resume(&mut self) -> Result<(), RegseqError>;
    }
"#;

fn regseq_module() -> String {
    format!(
        r#"
        pub mod regseq {{
            {FIXTURE_TYPES}
            {PM_TRAIT}
        }}
        use regseq::*;
    "#
    )
}

// ---------------------------------------------------------------------------
// Test 1: PM gate recognized with correct booleans
// ---------------------------------------------------------------------------

#[test]
fn recognize_pm_gate_with_properties() {
    let regseq = regseq_module();
    let source = format!(
        r#"
        {regseq}

        #[rseq::pm(idle = false, wakeup_capable = false)]
        pub struct Pl011;

        impl regseq::PowerManagement for Pl011 {{
            fn suspend(&mut self) -> Result<(), RegseqError> {{ Ok(()) }}
            fn resume(&mut self) -> Result<(), RegseqError> {{ Ok(()) }}
        }}
    "#
    );

    let combined = compile_and_extract(&source, "pm_basic");
    let surface = combined.pm_result.unwrap();

    assert_eq!(surface.gates.len(), 1);
    let gate = &surface.gates[0];
    assert_eq!(gate.driver_type, "Pl011");
    assert!(!gate.properties.idle);
    assert!(!gate.properties.wakeup_capable);
    assert!(gate.has_pm_impl);
}

#[test]
fn recognize_pm_gate_with_true_values() {
    let regseq = regseq_module();
    let source = format!(
        r#"
        {regseq}

        #[rseq::pm(idle = true, wakeup_capable = true)]
        pub struct Dev;

        impl regseq::PowerManagement for Dev {{
            fn suspend(&mut self) -> Result<(), RegseqError> {{ Ok(()) }}
            fn resume(&mut self) -> Result<(), RegseqError> {{ Ok(()) }}
        }}
    "#
    );

    let combined = compile_and_extract(&source, "pm_true_values");
    let surface = combined.pm_result.unwrap();

    assert_eq!(surface.gates.len(), 1);
    assert!(surface.gates[0].properties.idle);
    assert!(surface.gates[0].properties.wakeup_capable);
}

// ---------------------------------------------------------------------------
// Test 2: Malformed PM gate attributes rejected
// ---------------------------------------------------------------------------

#[test]
fn reject_malformed_pm_attribute() {
    let regseq = regseq_module();
    let source = format!(
        r#"
        {regseq}

        #[rseq::pm("not-key-value")]
        pub struct Dev;

        impl regseq::PowerManagement for Dev {{
            fn suspend(&mut self) -> Result<(), RegseqError> {{ Ok(()) }}
            fn resume(&mut self) -> Result<(), RegseqError> {{ Ok(()) }}
        }}
    "#
    );

    let combined = compile_and_extract(&source, "pm_malformed");
    assert!(
        matches!(combined.pm_result, Err(PmError::MalformedAttribute(_))),
        "expected MalformedAttribute, got: {:?}",
        combined.pm_result
    );
}

// ---------------------------------------------------------------------------
// Test 3: Duplicate keys and duplicate gates rejected
// ---------------------------------------------------------------------------

#[test]
fn reject_duplicate_key() {
    let regseq = regseq_module();
    let source = format!(
        r#"
        {regseq}

        #[rseq::pm(idle = false, idle = true, wakeup_capable = false)]
        pub struct Dev;

        impl regseq::PowerManagement for Dev {{
            fn suspend(&mut self) -> Result<(), RegseqError> {{ Ok(()) }}
            fn resume(&mut self) -> Result<(), RegseqError> {{ Ok(()) }}
        }}
    "#
    );

    let combined = compile_and_extract(&source, "pm_dup_key");
    assert!(
        matches!(
            &combined.pm_result,
            Err(PmError::DuplicateKey { key, .. }) if key == "idle"
        ),
        "expected DuplicateKey, got: {:?}",
        combined.pm_result
    );
}

#[test]
fn reject_duplicate_pm_gate() {
    let regseq = regseq_module();
    let source = format!(
        r#"
        {regseq}

        #[rseq::pm(idle = false, wakeup_capable = false)]
        #[rseq::pm(idle = true, wakeup_capable = true)]
        pub struct Dev;

        impl regseq::PowerManagement for Dev {{
            fn suspend(&mut self) -> Result<(), RegseqError> {{ Ok(()) }}
            fn resume(&mut self) -> Result<(), RegseqError> {{ Ok(()) }}
        }}
    "#
    );

    let combined = compile_and_extract(&source, "pm_dup_gate");
    assert!(
        matches!(&combined.pm_result, Err(PmError::DuplicateGate { .. })),
        "expected DuplicateGate, got: {:?}",
        combined.pm_result
    );
}

// ---------------------------------------------------------------------------
// Test 4: PM gate without PowerManagement impl rejected
// ---------------------------------------------------------------------------

#[test]
fn reject_pm_gate_without_impl() {
    let source = r#"
        #[rseq::pm(idle = false, wakeup_capable = false)]
        pub struct Dev;
    "#;

    let combined = compile_and_extract(source, "pm_no_impl");
    assert!(
        matches!(
            &combined.pm_result,
            Err(PmError::MissingPmImpl { driver_type }) if driver_type == "Dev"
        ),
        "expected MissingPmImpl, got: {:?}",
        combined.pm_result
    );
}

// ---------------------------------------------------------------------------
// Test 5: Valid PM gate paired with valid PowerManagement impl accepted
// ---------------------------------------------------------------------------

#[test]
fn accept_valid_pm_gate_with_impl() {
    let regseq = regseq_module();
    let source = format!(
        r#"
        {regseq}

        #[rseq::pm(idle = false, wakeup_capable = false)]
        pub struct Pl011;

        impl regseq::PowerManagement for Pl011 {{
            fn suspend(&mut self) -> Result<(), RegseqError> {{ Ok(()) }}
            fn resume(&mut self) -> Result<(), RegseqError> {{ Ok(()) }}
        }}
    "#
    );

    let combined = compile_and_extract(&source, "pm_valid_pair");
    let surface = combined.pm_result.unwrap();
    assert_eq!(surface.gates.len(), 1);
    assert!(surface.gates[0].has_pm_impl);
}

// ---------------------------------------------------------------------------
// Test 6: PowerManagement impl without PM gate remains allowed
// ---------------------------------------------------------------------------

#[test]
fn allow_pm_impl_without_gate() {
    let regseq = regseq_module();
    let source = format!(
        r#"
        {regseq}

        pub struct Dev;

        impl regseq::PowerManagement for Dev {{
            fn suspend(&mut self) -> Result<(), RegseqError> {{ Ok(()) }}
            fn resume(&mut self) -> Result<(), RegseqError> {{ Ok(()) }}
        }}
    "#
    );

    let combined = compile_and_extract(&source, "pm_impl_no_gate");
    let surface = combined.pm_result.unwrap();
    // No gates found -- that's fine. The trait impl exists but no gate requires it.
    assert!(surface.gates.is_empty());
}
