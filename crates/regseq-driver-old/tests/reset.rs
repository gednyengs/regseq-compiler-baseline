#![feature(rustc_private)]

extern crate rustc_ast;
extern crate rustc_driver;
extern crate rustc_hir;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_session;

use std::sync::{Arc, Mutex, Once};

#[path = "../src/extract/reset.rs"]
mod reset;

use reset::*;

type ExtractionResult = Result<ResetSurface, ResetError>;

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
            config
                .opts
                .unstable_opts
                .crate_attr
                .push("allow(dead_code)".to_string());
        }

        fn after_analysis<'tcx>(
            &mut self,
            _compiler: &rustc_interface::interface::Compiler,
            tcx: rustc_middle::ty::TyCtxt<'tcx>,
        ) -> rustc_driver::Compilation {
            *self.result.lock().unwrap() = Some(extract_reset_surface(tcx));
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

    #[derive(Clone, Copy)]
    pub struct Duration { us: u64 }
    impl Duration {
        pub const fn from_us(us: u64) -> Self { Self { us } }
        pub const fn from_ms(ms: u64) -> Self { Self { us: ms * 1_000 } }
    }

    pub struct ResetLine { _opaque: () }
    impl ResetLine {
        pub fn assert(&self) -> Result<(), RegseqError> { unimplemented!() }
        pub fn deassert(&self) -> Result<(), RegseqError> { unimplemented!() }
        pub fn toggle(&self, _delay: Duration) -> Result<(), RegseqError> { unimplemented!() }
    }
"#;

// ---------------------------------------------------------------------------
// Test 1: ResetLine and Option<ResetLine> fields recognized
// ---------------------------------------------------------------------------

#[test]
fn recognize_resetline_and_option_resetline_fields() {
    let source = format!(
        r#"
        {preamble}

        pub struct Dev {{
            #[rseq::reset(id = "main_rst")]
            pub reset: ResetLine,
        }}

        pub struct OptDev {{
            #[rseq::reset(id = "aux_rst")]
            pub reset: Option<ResetLine>,
        }}
    "#,
        preamble = FIXTURE_PREAMBLE
    );

    let surface = compile_and_extract(&source, "reset_fields").unwrap();

    assert_eq!(surface.fields.len(), 2);

    let required = surface.fields.iter().find(|f| f.required).unwrap();
    assert_eq!(required.field_name, "reset");

    let optional = surface.fields.iter().find(|f| !f.required).unwrap();
    assert_eq!(optional.field_name, "reset");
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
            #[rseq::reset(id = "main_rst")]
            pub main_rst: ResetLine,
            #[rseq::reset(id = "aux_rst")]
            pub aux_rst: Option<ResetLine>,
        }}
    "#,
        preamble = FIXTURE_PREAMBLE
    );

    let surface = compile_and_extract(&source, "reset_shapes").unwrap();

    let main_rst = surface
        .fields
        .iter()
        .find(|f| f.field_name == "main_rst")
        .unwrap();
    assert!(main_rst.required);

    let aux_rst = surface
        .fields
        .iter()
        .find(|f| f.field_name == "aux_rst")
        .unwrap();
    assert!(!aux_rst.required);
}

// ---------------------------------------------------------------------------
// Test 3: wrapper-method call sites recognized
// ---------------------------------------------------------------------------

#[test]
fn recognize_reset_wrapper_call_sites() {
    let source = format!(
        r#"
        {preamble}

        pub struct Dev {{
            #[rseq::reset(id = "main_rst")]
            pub reset: ResetLine,
        }}

        impl Dev {{
            pub fn hard_reset(&self) {{
                let _ = self.reset.assert();
                let _ = self.reset.deassert();
            }}

            pub fn toggle_reset(&self) {{
                let _ = self.reset.toggle(Duration::from_us(100));
            }}
        }}
    "#,
        preamble = FIXTURE_PREAMBLE
    );

    let surface = compile_and_extract(&source, "reset_call_sites").unwrap();

    assert_eq!(surface.call_sites.len(), 3);

    assert!(surface
        .call_sites
        .iter()
        .any(|s| s.method == ResetMethod::Assert && s.containing_fn == "hard_reset"));
    assert!(surface
        .call_sites
        .iter()
        .any(|s| s.method == ResetMethod::Deassert && s.containing_fn == "hard_reset"));
    assert!(surface
        .call_sites
        .iter()
        .any(|s| s.method == ResetMethod::Toggle && s.containing_fn == "toggle_reset"));
}

// ---------------------------------------------------------------------------
// Test 4: toggle rejects non-Duration arguments
// ---------------------------------------------------------------------------

#[test]
fn toggle_rejects_non_duration_arg() {
    let source = format!(
        r#"
        {preamble}

        pub struct BadResetLine {{ _opaque: () }}
        impl BadResetLine {{
            pub fn toggle(&self, _delay: u32) -> Result<(), RegseqError> {{ unimplemented!() }}
        }}

        pub struct Dev {{
            pub reset: BadResetLine,
        }}

        impl Dev {{
            pub fn bad_toggle(&self) {{
                let _ = self.reset.toggle(100u32);
            }}
        }}
    "#,
        preamble = FIXTURE_PREAMBLE
    );

    let surface = compile_and_extract(&source, "reset_bad_toggle").unwrap();
    // BadResetLine is not ResetLine, so no call sites should be detected.
    assert!(surface.call_sites.is_empty());
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
            pub fn assert(&self) {{}}
            pub fn deassert(&self) {{}}
            pub fn toggle(&self, _d: Duration) {{}}
        }}

        pub struct Dev {{
            #[rseq::reset(id = "main_rst")]
            pub reset: ResetLine,
            pub fake: FakeDevice,
        }}

        impl Dev {{
            pub fn do_stuff(&self) {{
                self.fake.assert();
                self.fake.deassert();
                self.fake.toggle(Duration::from_ms(10));
            }}
        }}
    "#,
        preamble = FIXTURE_PREAMBLE
    );

    let surface = compile_and_extract(&source, "reset_unrelated").unwrap();

    // No reset call sites -- receiver is FakeDevice, not ResetLine.
    assert!(surface.call_sites.is_empty());
}
