#![feature(rustc_private)]

extern crate rustc_ast;
extern crate rustc_driver;
extern crate rustc_hir;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_session;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, Once};

#[path = "../src/extract/pinctrl.rs"]
mod pinctrl;

use pinctrl::*;

type ExtractionResult = Result<PinCtrlSurface, PinCtrlError>;

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

#[derive(Debug, Clone)]
struct BuildArtifacts {
    regseq_rlib: PathBuf,
    regseq_macros_dylib: PathBuf,
    deps_dir: PathBuf,
}

fn build_deps() -> BuildArtifacts {
    let root = workspace_root();

    let output = std::process::Command::new("cargo")
        .args([
            "build",
            "-p",
            "regseq",
            "-p",
            "regseq-macros",
            "--message-format=json",
        ])
        .current_dir(&root)
        .output()
        .expect("failed to run cargo build for regseq deps");

    assert!(
        output.status.success(),
        "cargo build for regseq deps failed:\n{}",
        String::from_utf8_lossy(&output.stderr),
    );

    let stdout = String::from_utf8(output.stdout).expect("cargo output is valid UTF-8");
    let mut regseq_rlib: Option<PathBuf> = None;
    let mut regseq_macros_dylib: Option<PathBuf> = None;

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
        let Some(name) = target.get("name").and_then(|v| v.as_str()) else {
            continue;
        };
        let Some(filenames) = msg.get("filenames").and_then(|v| v.as_array()) else {
            continue;
        };

        match name {
            "regseq" => {
                for f in filenames {
                    let p = f.as_str().unwrap_or("");
                    if p.ends_with(".rlib") {
                        regseq_rlib = Some(PathBuf::from(p));
                        break;
                    }
                }
            }
            "regseq-macros" | "regseq_macros" => {
                for f in filenames {
                    let p = f.as_str().unwrap_or("");
                    if p.ends_with(".dylib") || p.ends_with(".so") || p.ends_with(".dll") {
                        regseq_macros_dylib = Some(PathBuf::from(p));
                        break;
                    }
                }
            }
            _ => {}
        }
    }

    let regseq_rlib = regseq_rlib.expect("regseq rlib not found in cargo build output");
    let regseq_macros_dylib =
        regseq_macros_dylib.expect("regseq-macros dylib not found in cargo build output");

    let deps_dir = regseq_rlib
        .parent()
        .expect("regseq rlib path must have a parent")
        .to_path_buf();

    BuildArtifacts {
        regseq_rlib,
        regseq_macros_dylib,
        deps_dir,
    }
}

fn artifacts() -> &'static BuildArtifacts {
    use std::sync::OnceLock;
    static A: OnceLock<BuildArtifacts> = OnceLock::new();
    A.get_or_init(build_deps)
}

fn compile_pl011_and_extract(test_name: &str) -> ExtractionResult {
    let artifacts = artifacts();
    let _lock = COMPILER_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    init_rustc();

    let src_path = workspace_root().join("tests/fixtures/pl011-full-spec/src/lib.rs");
    let test_dir = std::env::temp_dir()
        .join("regseq-driver-tests")
        .join(test_name);
    if test_dir.exists() {
        std::fs::remove_dir_all(&test_dir).unwrap();
    }
    std::fs::create_dir_all(&test_dir).unwrap();

    let sysroot = get_sysroot();
    let result: Arc<Mutex<Option<ExtractionResult>>> = Arc::new(Mutex::new(None));

    struct ExtractCallbacks {
        result: Arc<Mutex<Option<ExtractionResult>>>,
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
            *self.result.lock().unwrap() = Some(extract_pinctrl_surface(tcx));
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
        "--crate-name".to_string(),
        "pl011_full_spec".to_string(),
        "--out-dir".to_string(),
        test_dir.to_str().unwrap().to_string(),
        "--extern".to_string(),
        format!("regseq={}", artifacts.regseq_rlib.display()),
        "--extern".to_string(),
        format!("regseq_macros={}", artifacts.regseq_macros_dylib.display()),
        "-L".to_string(),
        format!("dependency={}", artifacts.deps_dir.display()),
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

fn compile_and_extract(source: &str, test_name: &str) -> ExtractionResult {
    let _lock = COMPILER_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
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

    let result: Arc<Mutex<Option<ExtractionResult>>> = Arc::new(Mutex::new(None));

    struct ExtractCallbacks {
        result: Arc<Mutex<Option<ExtractionResult>>>,
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
            *self.result.lock().unwrap() = Some(extract_pinctrl_surface(tcx));
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
// Common fixture preamble
// ---------------------------------------------------------------------------

const FIXTURE_PREAMBLE: &str = r#"
    #![allow(dead_code)]

    pub enum RegseqError { HardwareError }

    pub trait PinState {}

    pub struct PinCtrl { _opaque: () }
    impl PinCtrl {
        pub fn apply<S: PinState>(&self) -> Result<(), RegseqError> { unimplemented!() }
    }

    #[rseq::pin_state(id = "default")]
    pub struct DefaultState;
    impl PinState for DefaultState {}

    #[rseq::pin_state(id = "sleep")]
    pub struct SleepState;
    impl PinState for SleepState {}
"#;

// ---------------------------------------------------------------------------
// Test 1: PinCtrl and Option<PinCtrl> fields recognized
// ---------------------------------------------------------------------------

#[test]
fn recognize_pinctrl_and_option_pinctrl_fields() {
    // The `PinCtrl` fields here are intentionally left unattributed: this test
    // verifies the extractor's default behavior when no
    // `#[rseq::pinctrl(id = "...")]` override is present. The attribute-present
    // case is covered separately by `capture_pinctrl_id_override`.
    let source = format!(
        r#"
        {preamble}

        pub struct Dev {{
            pub pins: PinCtrl,
        }}

        pub struct OptDev {{
            pub pins: Option<PinCtrl>,
        }}
    "#,
        preamble = FIXTURE_PREAMBLE
    );

    let surface = compile_and_extract(&source, "pinctrl_fields").unwrap();

    assert_eq!(surface.fields.len(), 2);

    let required = surface.fields.iter().find(|f| f.required).unwrap();
    assert_eq!(required.field_name, "pins");
    assert!(required.pinctrl_id.is_none());

    let optional = surface.fields.iter().find(|f| !f.required).unwrap();
    assert_eq!(optional.field_name, "pins");
    assert!(optional.pinctrl_id.is_none());
}

// ---------------------------------------------------------------------------
// Test 2: optional #[rseq::pinctrl(id = "...")] override captured
// ---------------------------------------------------------------------------

#[test]
fn capture_pinctrl_id_override() {
    let source = format!(
        r#"
        {preamble}

        pub struct Dev {{
            #[rseq::pinctrl(id = "uart_pins")]
            pub pins: PinCtrl,
        }}
    "#,
        preamble = FIXTURE_PREAMBLE
    );

    let surface = compile_and_extract(&source, "pinctrl_id").unwrap();

    assert_eq!(surface.fields.len(), 1);
    assert_eq!(surface.fields[0].pinctrl_id, Some("uart_pins".to_string()));
}

// ---------------------------------------------------------------------------
// Test 3: apply::<State>() call sites recognized with state marker
// ---------------------------------------------------------------------------

#[test]
fn recognize_apply_call_sites_with_state() {
    let source = format!(
        r#"
        {preamble}

        pub struct Dev {{
            #[rseq::pinctrl(id = "uart_pins")]
            pub pins: PinCtrl,
        }}

        impl Dev {{
            pub fn activate(&self) {{
                let _ = self.pins.apply::<DefaultState>();
            }}

            pub fn sleep(&self) {{
                let _ = self.pins.apply::<SleepState>();
            }}
        }}
    "#,
        preamble = FIXTURE_PREAMBLE
    );

    let surface = compile_and_extract(&source, "pinctrl_call_sites").unwrap();

    assert_eq!(surface.call_sites.len(), 2);

    assert!(surface
        .call_sites
        .iter()
        .any(|s| s.state_type == "DefaultState" && s.containing_fn == "activate"));
    assert!(surface
        .call_sites
        .iter()
        .any(|s| s.state_type == "SleepState" && s.containing_fn == "sleep"));
}

// ---------------------------------------------------------------------------
// Test 4: malformed pinctrl attributes rejected
// ---------------------------------------------------------------------------

#[test]
fn reject_malformed_pinctrl_attribute() {
    let source = format!(
        r#"
        {preamble}

        pub struct Dev {{
            #[rseq::pinctrl("not-key-value")]
            pub pins: PinCtrl,
        }}
    "#,
        preamble = FIXTURE_PREAMBLE
    );

    let result = compile_and_extract(&source, "pinctrl_malformed");
    assert!(matches!(result, Err(PinCtrlError::MalformedAttribute(_))));
}

#[test]
fn reject_pinctrl_with_duplicate_id_keys() {
    let source = format!(
        r#"
        {preamble}

        pub struct Dev {{
            #[rseq::pinctrl(id = "first", id = "second")]
            pub pins: PinCtrl,
        }}
    "#,
        preamble = FIXTURE_PREAMBLE
    );

    let result = compile_and_extract(&source, "pinctrl_dup_id");
    let err = result.expect_err("duplicate id should be rejected");
    let PinCtrlError::MalformedAttribute(msg) = err;
    assert!(
        msg.contains("duplicate"),
        "error should mention 'duplicate', got: {msg}",
    );
}

#[test]
fn reject_pinctrl_with_non_string_id() {
    let source = format!(
        r#"
        {preamble}

        pub struct Dev {{
            #[rseq::pinctrl(id = 42)]
            pub pins: PinCtrl,
        }}
    "#,
        preamble = FIXTURE_PREAMBLE
    );

    let result = compile_and_extract(&source, "pinctrl_non_string_id");
    let err = result.expect_err("non-string id should be rejected");
    let PinCtrlError::MalformedAttribute(msg) = err;
    assert!(
        msg.contains("string literal"),
        "error should distinguish non-string from missing id, got: {msg}",
    );
}

// ---------------------------------------------------------------------------
// Test 5: non-generic or unrelated apply methods not misclassified
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Test 6: #[rseq::pin_state(id = "...")] marker types discovered
// ---------------------------------------------------------------------------

#[test]
fn discover_pin_state_markers() {
    // Uses the FIXTURE_PREAMBLE (which already declares DefaultState/SleepState
    // as pin_state markers per spec 4.2) plus two test-local markers to verify
    // that discovery picks up every annotated marker in the crate.
    let source = format!(
        r#"
        {preamble}

        #[rseq::pin_state(id = "extra_default")]
        pub struct DefaultPins;

        #[rseq::pin_state(id = "extra_sleep")]
        pub struct SleepPins;
    "#,
        preamble = FIXTURE_PREAMBLE
    );

    let surface = compile_and_extract(&source, "pinctrl_pin_states").unwrap();

    let find_by_path = |suffix: &str| {
        surface
            .pin_states
            .iter()
            .find(|s| s.type_path.ends_with(suffix))
            .unwrap_or_else(|| {
                panic!(
                    "{suffix} marker must be discovered in {:?}",
                    surface.pin_states
                )
            })
    };

    let preamble_default = find_by_path("DefaultState");
    assert_eq!(preamble_default.state_id, "default");

    let preamble_sleep = find_by_path("SleepState");
    assert_eq!(preamble_sleep.state_id, "sleep");

    let local_default = find_by_path("DefaultPins");
    assert_eq!(local_default.state_id, "extra_default");

    let local_sleep = find_by_path("SleepPins");
    assert_eq!(local_sleep.state_id, "extra_sleep");
}

// ---------------------------------------------------------------------------
// Test 6b: PL011 full-spec compilation yields the expected 2 pin states
// ---------------------------------------------------------------------------

#[test]
fn pin_states_from_pl011_full_spec_compilation() {
    let surface = compile_pl011_and_extract("pinctrl_pl011_full_spec").unwrap();

    assert_eq!(
        surface.pin_states.len(),
        2,
        "PL011 fixture should expose 2 pin states, got {:?}",
        surface.pin_states,
    );

    let default = surface
        .pin_states
        .iter()
        .find(|s| s.state_id == "default")
        .expect("PL011 default pin state must be discovered");
    assert!(
        default.type_path.ends_with("DefaultState"),
        "default state should map to DefaultState marker, got {}",
        default.type_path,
    );

    let sleep = surface
        .pin_states
        .iter()
        .find(|s| s.state_id == "sleep")
        .expect("PL011 sleep pin state must be discovered");
    assert!(
        sleep.type_path.ends_with("SleepState"),
        "sleep state should map to SleepState marker, got {}",
        sleep.type_path,
    );
}

// ---------------------------------------------------------------------------
// Test 7: pin_state attribute without id is rejected
// ---------------------------------------------------------------------------

#[test]
fn reject_pin_state_without_id() {
    let source = format!(
        r#"
        {preamble}

        #[rseq::pin_state()]
        pub struct BadState;
    "#,
        preamble = FIXTURE_PREAMBLE
    );

    let result = compile_and_extract(&source, "pinctrl_pin_state_no_id");
    assert!(matches!(result, Err(PinCtrlError::MalformedAttribute(_))));
}

#[test]
fn reject_pin_state_with_duplicate_id_keys() {
    let source = format!(
        r#"
        {preamble}

        #[rseq::pin_state(id = "first", id = "second")]
        pub struct DupState;
    "#,
        preamble = FIXTURE_PREAMBLE
    );

    let result = compile_and_extract(&source, "pinctrl_pin_state_dup_id");
    let err = result.expect_err("duplicate id should be rejected");
    let PinCtrlError::MalformedAttribute(msg) = err;
    assert!(
        msg.contains("duplicate"),
        "error should mention 'duplicate', got: {msg}",
    );
}

#[test]
fn reject_pin_state_with_non_string_id() {
    let source = format!(
        r#"
        {preamble}

        #[rseq::pin_state(id = 42)]
        pub struct IntIdState;
    "#,
        preamble = FIXTURE_PREAMBLE
    );

    let result = compile_and_extract(&source, "pinctrl_pin_state_non_string_id");
    let err = result.expect_err("non-string id should be rejected");
    let PinCtrlError::MalformedAttribute(msg) = err;
    assert!(
        msg.contains("string literal"),
        "error should distinguish non-string from missing id, got: {msg}",
    );
}

// ---------------------------------------------------------------------------
// Test 8: unrelated apply methods not misclassified
// ---------------------------------------------------------------------------

#[test]
fn unrelated_apply_not_misclassified() {
    let source = format!(
        r#"
        {preamble}

        pub struct FakeDevice;
        impl FakeDevice {{
            pub fn apply(&self) {{}}
        }}

        pub struct Dev {{
            #[rseq::pinctrl(id = "uart_pins")]
            pub pins: PinCtrl,
            pub fake: FakeDevice,
        }}

        impl Dev {{
            pub fn do_stuff(&self) {{
                self.fake.apply();
            }}
        }}
    "#,
        preamble = FIXTURE_PREAMBLE
    );

    let surface = compile_and_extract(&source, "pinctrl_unrelated").unwrap();

    // No pinctrl call sites -- receiver is FakeDevice, not PinCtrl.
    assert!(surface.call_sites.is_empty());
}
