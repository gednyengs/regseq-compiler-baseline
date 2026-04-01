#![feature(rustc_private)]

extern crate rustc_ast;
extern crate rustc_driver;
extern crate rustc_hir;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_session;

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

    pub struct DefaultState;
    impl PinState for DefaultState {}

    pub struct SleepState;
    impl PinState for SleepState {}
"#;

// ---------------------------------------------------------------------------
// Test 1: PinCtrl and Option<PinCtrl> fields recognized
// ---------------------------------------------------------------------------

#[test]
fn recognize_pinctrl_and_option_pinctrl_fields() {
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

// ---------------------------------------------------------------------------
// Test 5: non-generic or unrelated apply methods not misclassified
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
