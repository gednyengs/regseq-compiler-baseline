//! Cross-crate attribute visibility gate test (regseq-driver-26).
//!
//! Validates that `#[rseq::intrinsic("...")]`, `#[rseq::framework_type("...")]`,
//! `#[rseq::abstraction("...")]`, and `#[rseq::pin_state(id = "...")]`
//! attributes placed on facade crate types are visible via
//! `tcx.get_all_attrs(def_id)` when extracting a dependent crate.

#![feature(rustc_private)]

extern crate rustc_ast;
extern crate rustc_driver;
extern crate rustc_hir;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_session;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, Once};

#[path = "../src/extract/attr_validation.rs"]
mod attr_validation;

use attr_validation::{AttrValidationReport, FoundAttr};

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

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

/// Build the `regseq` crate and return the path to its rlib.
fn build_regseq_rlib() -> PathBuf {
    let root = workspace_root();

    // Build the regseq crate via cargo.
    let output = std::process::Command::new("cargo")
        .args(["build", "-p", "regseq", "--message-format=json"])
        .current_dir(&root)
        .output()
        .expect("failed to run cargo build -p regseq");

    assert!(
        output.status.success(),
        "cargo build -p regseq failed:\n{}",
        String::from_utf8_lossy(&output.stderr),
    );

    // Parse JSON output to find the regseq rlib artifact.
    let stdout = String::from_utf8(output.stdout).expect("cargo output is valid UTF-8");
    for line in stdout.lines() {
        let Ok(msg) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if msg.get("reason").and_then(|v| v.as_str()) != Some("compiler-artifact") {
            continue;
        }
        let Some(target) = msg.get("target") else {
            continue;
        };
        if target.get("name").and_then(|v| v.as_str()) != Some("regseq") {
            continue;
        }
        // Find the rlib in filenames.
        if let Some(filenames) = msg.get("filenames").and_then(|v| v.as_array()) {
            for f in filenames {
                let path = f.as_str().unwrap_or("");
                if path.ends_with(".rlib") {
                    return PathBuf::from(path);
                }
            }
        }
    }

    panic!("could not find regseq rlib in cargo build output");
}

/// Compile a test source against the regseq crate and run the validation pass.
fn compile_and_validate(source: &str, test_name: &str, regseq_rlib: &Path) -> AttrValidationReport {
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
    let deps_dir = workspace_root().join("target/debug/deps");

    let result: Arc<Mutex<Option<AttrValidationReport>>> = Arc::new(Mutex::new(None));

    struct ValidateCallbacks {
        result: Arc<Mutex<Option<AttrValidationReport>>>,
    }

    impl rustc_driver::Callbacks for ValidateCallbacks {
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
                .push("allow(unused)".to_string());
        }

        fn after_analysis<'tcx>(
            &mut self,
            _compiler: &rustc_interface::interface::Compiler,
            tcx: rustc_middle::ty::TyCtxt<'tcx>,
        ) -> rustc_driver::Compilation {
            *self.result.lock().unwrap() = Some(attr_validation::validate_cross_crate_attrs(tcx));
            rustc_driver::Compilation::Stop
        }
    }

    let mut callbacks = ValidateCallbacks {
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
        "--extern".to_string(),
        format!("regseq={}", regseq_rlib.display()),
        "-L".to_string(),
        format!("dependency={}", deps_dir.display()),
    ];

    rustc_driver::run_compiler(&args, &mut callbacks);

    let report = result
        .lock()
        .unwrap()
        .take()
        .expect("after_analysis should have been called");

    std::fs::remove_dir_all(&test_dir).ok();

    report
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn find_by_value<'a>(attrs: &'a [FoundAttr], value: &str) -> Option<&'a FoundAttr> {
    attrs.iter().find(|a| a.value.as_deref() == Some(value))
}

fn find_by_path_and_value<'a>(
    attrs: &'a [FoundAttr],
    path_suffix: &str,
    value: &str,
) -> Option<&'a FoundAttr> {
    attrs
        .iter()
        .find(|a| a.def_path.ends_with(path_suffix) && a.value.as_deref() == Some(value))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Build the regseq rlib once and reuse across tests.
fn regseq_rlib() -> &'static PathBuf {
    use std::sync::OnceLock;
    static RLIB: OnceLock<PathBuf> = OnceLock::new();
    RLIB.get_or_init(build_regseq_rlib)
}

/// Minimal test source that depends on regseq.
const CROSS_CRATE_SOURCE: &str = r#"
    extern crate regseq;

    #[rseq::pin_state(id = "default")]
    pub struct DefaultPins;

    #[rseq::pin_state(id = "sleep")]
    pub struct SleepPins;
"#;

#[test]
fn cross_crate_intrinsic_attrs_visible() {
    let rlib = regseq_rlib();
    let report = compile_and_validate(CROSS_CRATE_SOURCE, "cross_crate_intrinsics", rlib);

    report.log();

    // Spec requirement: Mmio::read has rseq::intrinsic("mmio_read").
    // Verify the attribute appears on the correct item via def_path.
    assert!(
        find_by_path_and_value(&report.intrinsics, "Mmio::<Block>::read", "mmio_read").is_some(),
        "Mmio::read should carry mmio_read intrinsic"
    );
    assert!(
        find_by_path_and_value(&report.intrinsics, "Mmio::<Block>::write", "mmio_write").is_some(),
        "Mmio::write should carry mmio_write intrinsic"
    );
    assert!(
        find_by_path_and_value(
            &report.intrinsics,
            "Mmio::<Block>::read_field",
            "mmio_read_field"
        )
        .is_some(),
        "Mmio::read_field should carry mmio_read_field intrinsic"
    );
    assert!(
        find_by_path_and_value(
            &report.intrinsics,
            "Mmio::<Block>::write_field",
            "mmio_write_field"
        )
        .is_some(),
        "Mmio::write_field should carry mmio_write_field intrinsic"
    );

    // Spec requirement: at least 1 intrinsic from clock.rs.
    assert!(
        find_by_path_and_value(&report.intrinsics, "Clock::enable", "clock_enable").is_some(),
        "Clock::enable should carry clock_enable intrinsic"
    );

    // Spec requirement: at least 1 intrinsic from reset.rs.
    assert!(
        find_by_path_and_value(&report.intrinsics, "ResetLine::toggle", "reset_toggle").is_some(),
        "ResetLine::toggle should carry reset_toggle intrinsic"
    );

    // Spec requirement: at least 1 intrinsic from pinctrl.rs.
    assert!(
        find_by_path_and_value(&report.intrinsics, "PinCtrl::apply", "pinctrl_apply").is_some(),
        "PinCtrl::apply should carry pinctrl_apply intrinsic"
    );

    // Spec requirement: at least 1 intrinsic from intrinsics/ submodules.
    let has_intrinsics_submod = find_by_value(&report.intrinsics, "busy_wait").is_some()
        || find_by_value(&report.intrinsics, "sleep").is_some()
        || find_by_value(&report.intrinsics, "poll_until").is_some();
    assert!(
        has_intrinsics_submod,
        "at least one intrinsic from intrinsics/ submodules should be visible"
    );
}

#[test]
fn cross_crate_abstraction_attrs_visible() {
    let rlib = regseq_rlib();
    let report = compile_and_validate(CROSS_CRATE_SOURCE, "cross_crate_abstractions", rlib);

    // Spec requirement: Clock struct has rseq::abstraction("clock").
    assert!(
        find_by_path_and_value(&report.abstractions, "Clock", "clock").is_some(),
        "Clock should carry abstraction(\"clock\")"
    );

    // Mmio struct has rseq::abstraction("mmio").
    assert!(
        find_by_path_and_value(&report.abstractions, "Mmio", "mmio").is_some(),
        "Mmio should carry abstraction(\"mmio\")"
    );
}

#[test]
fn cross_crate_framework_type_attrs_visible() {
    let rlib = regseq_rlib();
    let report = compile_and_validate(CROSS_CRATE_SOURCE, "cross_crate_framework_types", rlib);

    // Spec requirement: SerialConfig struct has rseq::framework_type("serial-config").
    assert!(
        find_by_path_and_value(&report.framework_types, "SerialConfig", "serial-config").is_some(),
        "SerialConfig should carry framework_type(\"serial-config\")"
    );

    // At least one framework type total.
    assert!(
        !report.framework_types.is_empty(),
        "at least one framework_type should be visible cross-crate"
    );
}

#[test]
fn local_pin_state_attrs_visible() {
    let rlib = regseq_rlib();
    let report = compile_and_validate(CROSS_CRATE_SOURCE, "local_pin_states", rlib);

    // Local pin_state attrs from the test source.
    assert!(
        find_by_value(&report.pin_states, "default").is_some(),
        "default pin_state should be visible"
    );
    assert!(
        find_by_value(&report.pin_states, "sleep").is_some(),
        "sleep pin_state should be visible"
    );
}

#[test]
fn cross_crate_validation_summary() {
    let rlib = regseq_rlib();
    let report = compile_and_validate(CROSS_CRATE_SOURCE, "cross_crate_summary", rlib);

    report.log();

    eprintln!(
        "\n=== Cross-crate attribute validation summary ===\n\
         Intrinsics:      {}\n\
         Abstractions:    {}\n\
         Framework types: {}\n\
         Pin states:      {}\n\
         Total:           {}\n\
         ================================================",
        report.intrinsics.len(),
        report.abstractions.len(),
        report.framework_types.len(),
        report.pin_states.len(),
        report.total(),
    );

    if report.total() == 0 {
        eprintln!(
            "FALLBACK: Cross-crate attribute visibility failed.\n\
             Path-based heuristics remain the primary recognition mechanism.\n\
             Phase 4 tasks 27-29 should use path-based registry builders."
        );
    } else {
        eprintln!(
            "SUCCESS: Cross-crate attributes are visible.\n\
             Attribute-based registries (tasks 27-29) are viable."
        );
    }

    // Gate assertion: we expect at least some attributes to be visible.
    // If this fails, the fallback strategy applies.
    assert!(
        report.total() > 0,
        "Gate test: expected at least some cross-crate attributes to be visible"
    );
}
