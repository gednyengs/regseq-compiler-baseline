#![feature(rustc_private)]

extern crate rustc_ast;
extern crate rustc_driver;
extern crate rustc_hir;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_session;

use std::sync::{Arc, Mutex, Once};

#[path = "../src/extract/clock.rs"]
mod clock;

use clock::*;

type ExtractionResult = Result<ClockSurface, ClockError>;

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
            *self.result.lock().unwrap() = Some(extract_clock_surface(tcx));
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
    pub enum RegseqError { HardwareError }

    pub struct Clock { _opaque: () }
    impl Clock {
        pub fn enable(&self) -> Result<(), RegseqError> { unimplemented!() }
        pub fn disable(&self) { unimplemented!() }
        pub fn get_rate(&self) -> u32 { unimplemented!() }
    }
"#;

// ---------------------------------------------------------------------------
// Test 1: Clock and Option<Clock> fields recognized
// ---------------------------------------------------------------------------

#[test]
fn recognize_clock_and_option_clock_fields() {
    let source = format!(
        r#"
        {preamble}

        pub struct Pl011 {{
            #[rseq::clock(id = "uart_clk")]
            pub clock: Clock,
        }}

        pub struct OptionalClockDev {{
            #[rseq::clock(id = "aux_clk")]
            pub aux: Option<Clock>,
        }}
    "#,
        preamble = FIXTURE_PREAMBLE
    );

    let surface = compile_and_extract(&source, "clock_fields").unwrap();

    assert_eq!(surface.fields.len(), 2);

    let required = surface
        .fields
        .iter()
        .find(|f| f.field_name == "clock")
        .unwrap();
    assert_eq!(required.clock_id, "uart_clk");
    assert!(required.required);

    let optional = surface
        .fields
        .iter()
        .find(|f| f.field_name == "aux")
        .unwrap();
    assert_eq!(optional.clock_id, "aux_clk");
    assert!(!optional.required);
}

// ---------------------------------------------------------------------------
// Test 2: required vs optional shape preserved
// ---------------------------------------------------------------------------

#[test]
fn required_vs_optional_shape_preserved() {
    let source = format!(
        r#"
        {preamble}

        pub struct Dev {{
            #[rseq::clock(id = "main_clk")]
            pub main_clk: Clock,
            #[rseq::clock(id = "opt_clk")]
            pub opt_clk: Option<Clock>,
        }}
    "#,
        preamble = FIXTURE_PREAMBLE
    );

    let surface = compile_and_extract(&source, "clock_shapes").unwrap();

    let main_clk = surface
        .fields
        .iter()
        .find(|f| f.field_name == "main_clk")
        .unwrap();
    assert!(main_clk.required);

    let opt_clk = surface
        .fields
        .iter()
        .find(|f| f.field_name == "opt_clk")
        .unwrap();
    assert!(!opt_clk.required);
}

// ---------------------------------------------------------------------------
// Test 3: wrapper-method call sites recognized
// ---------------------------------------------------------------------------

#[test]
fn recognize_clock_wrapper_call_sites() {
    let source = format!(
        r#"
        {preamble}

        pub struct Dev {{
            #[rseq::clock(id = "uart_clk")]
            pub clock: Clock,
        }}

        impl Dev {{
            pub fn setup(&self) {{
                let _ = self.clock.enable();
                let _rate = self.clock.get_rate();
            }}

            pub fn teardown(&self) {{
                self.clock.disable();
            }}
        }}
    "#,
        preamble = FIXTURE_PREAMBLE
    );

    let surface = compile_and_extract(&source, "clock_call_sites").unwrap();

    assert_eq!(surface.call_sites.len(), 3);

    assert!(surface
        .call_sites
        .iter()
        .any(|s| s.method == ClockMethod::Enable && s.containing_fn == "setup"));
    assert!(surface
        .call_sites
        .iter()
        .any(|s| s.method == ClockMethod::GetRate && s.containing_fn == "setup"));
    assert!(surface
        .call_sites
        .iter()
        .any(|s| s.method == ClockMethod::Disable && s.containing_fn == "teardown"));
}

// ---------------------------------------------------------------------------
// Test 4: malformed or missing clock_id rejected
// ---------------------------------------------------------------------------

#[test]
fn reject_missing_clock_id() {
    let source = format!(
        r#"
        {preamble}

        pub struct Dev {{
            #[rseq::clock(foo = "bar")]
            pub clock: Clock,
        }}
    "#,
        preamble = FIXTURE_PREAMBLE
    );

    let result = compile_and_extract(&source, "clock_missing_id");
    assert!(matches!(result, Err(ClockError::MissingClockId(_))));
}

#[test]
fn reject_clock_field_without_attribute() {
    let source = format!(
        r#"
        {preamble}

        pub struct Dev {{
            pub clock: Clock,
        }}
    "#,
        preamble = FIXTURE_PREAMBLE
    );

    let result = compile_and_extract(&source, "clock_no_attr");
    assert!(matches!(result, Err(ClockError::MissingClockId(_))));
}

#[test]
fn reject_malformed_clock_attribute() {
    let source = format!(
        r#"
        {preamble}

        pub struct Dev {{
            #[rseq::clock("not-key-value")]
            pub clock: Clock,
        }}
    "#,
        preamble = FIXTURE_PREAMBLE
    );

    let result = compile_and_extract(&source, "clock_malformed");
    assert!(matches!(result, Err(ClockError::MalformedAttribute(_))));
}

// ---------------------------------------------------------------------------
// Test 5: unrelated methods not misclassified
// ---------------------------------------------------------------------------

#[test]
fn unrelated_methods_not_misclassified() {
    let source = format!(
        r#"
        {preamble}

        pub struct FakeDevice;
        impl FakeDevice {{
            pub fn enable(&self) {{}}
            pub fn disable(&self) {{}}
            pub fn get_rate(&self) -> u32 {{ 0 }}
        }}

        pub struct Dev {{
            #[rseq::clock(id = "clk")]
            pub clock: Clock,
            pub fake: FakeDevice,
        }}

        impl Dev {{
            pub fn do_stuff(&self) {{
                self.fake.enable();
                self.fake.disable();
                let _ = self.fake.get_rate();
            }}
        }}
    "#,
        preamble = FIXTURE_PREAMBLE
    );

    let surface = compile_and_extract(&source, "clock_unrelated").unwrap();

    // No clock call sites should be detected -- the receiver is FakeDevice, not Clock.
    assert!(surface.call_sites.is_empty());
}
