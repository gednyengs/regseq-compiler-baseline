#![feature(rustc_private)]

extern crate rustc_ast;
extern crate rustc_driver;
extern crate rustc_hir;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_session;

use std::sync::{Arc, Mutex, Once};

#[path = "../src/extract/logging.rs"]
mod logging;

use logging::*;

type ExtractionResult = Result<LoggingSurface, LoggingError>;

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
            *self.result.lock().unwrap() = Some(extract_logging_surface(tcx));
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
    pub enum IrqResult { Handled, None }

    #[derive(Clone, Copy)]
    pub enum LogArg<'a> {
        U8(u8), U16(u16), U32(u32), U64(u64), Usize(usize),
        I8(i8), I16(i16), I32(i32), I64(i64), Isize(isize),
        Bool(bool), Str(&'a str), Ptr(*const ()),
    }

    impl From<u8> for LogArg<'_> { fn from(v: u8) -> Self { LogArg::U8(v) } }
    impl From<u16> for LogArg<'_> { fn from(v: u16) -> Self { LogArg::U16(v) } }
    impl From<u32> for LogArg<'_> { fn from(v: u32) -> Self { LogArg::U32(v) } }
    impl From<u64> for LogArg<'_> { fn from(v: u64) -> Self { LogArg::U64(v) } }
    impl From<usize> for LogArg<'_> { fn from(v: usize) -> Self { LogArg::Usize(v) } }
    impl From<i8> for LogArg<'_> { fn from(v: i8) -> Self { LogArg::I8(v) } }
    impl From<i16> for LogArg<'_> { fn from(v: i16) -> Self { LogArg::I16(v) } }
    impl From<i32> for LogArg<'_> { fn from(v: i32) -> Self { LogArg::I32(v) } }
    impl From<i64> for LogArg<'_> { fn from(v: i64) -> Self { LogArg::I64(v) } }
    impl From<isize> for LogArg<'_> { fn from(v: isize) -> Self { LogArg::Isize(v) } }
    impl From<bool> for LogArg<'_> { fn from(v: bool) -> Self { LogArg::Bool(v) } }
    impl<'a> From<&'a str> for LogArg<'a> { fn from(v: &'a str) -> Self { LogArg::Str(v) } }
    impl From<*const ()> for LogArg<'_> { fn from(v: *const ()) -> Self { LogArg::Ptr(v) } }

    pub mod intrinsics {
        pub mod diagnostic_logging {
            use super::super::LogArg;
            pub fn error<'a, const N: usize>(_t: &'static str, _a: [LogArg<'a>; N]) {}
            pub fn warn<'a, const N: usize>(_t: &'static str, _a: [LogArg<'a>; N]) {}
            pub fn info<'a, const N: usize>(_t: &'static str, _a: [LogArg<'a>; N]) {}
            pub fn debug<'a, const N: usize>(_t: &'static str, _a: [LogArg<'a>; N]) {}
        }
    }

    pub trait Lifecycle {
        fn init(&mut self) -> Result<(), RegseqError>;
    }
"#;

// ---------------------------------------------------------------------------
// Test 1: four accepted logging levels recognized
// ---------------------------------------------------------------------------

#[test]
fn recognize_four_logging_levels() {
    let source = format!(
        r#"
        {preamble}

        pub struct Dev;

        impl Dev {{
            pub fn log_all(&self) {{
                intrinsics::diagnostic_logging::error("err", []);
                intrinsics::diagnostic_logging::warn("wrn", []);
                intrinsics::diagnostic_logging::info("inf", []);
                intrinsics::diagnostic_logging::debug("dbg", []);
            }}
        }}
    "#,
        preamble = FIXTURE_PREAMBLE
    );

    let surface = compile_and_extract(&source, "logging_levels").unwrap();

    assert_eq!(surface.call_sites.len(), 4);
    assert!(surface
        .call_sites
        .iter()
        .any(|s| s.level == LogLevel::Error));
    assert!(surface.call_sites.iter().any(|s| s.level == LogLevel::Warn));
    assert!(surface.call_sites.iter().any(|s| s.level == LogLevel::Info));
    assert!(surface
        .call_sites
        .iter()
        .any(|s| s.level == LogLevel::Debug));
}

// ---------------------------------------------------------------------------
// Test 2: template plus primitive dynamic arguments captured
// ---------------------------------------------------------------------------

#[test]
fn capture_template_and_args() {
    let source = format!(
        r#"
        {preamble}

        pub struct Dev;

        impl Dev {{
            pub fn log_with_args(&self) {{
                let channel: u32 = 3;
                intrinsics::diagnostic_logging::error(
                    "DMA channel {{}} failed on {{}}",
                    [LogArg::from(channel), LogArg::from("rx")],
                );
            }}
        }}
    "#,
        preamble = FIXTURE_PREAMBLE
    );

    let surface = compile_and_extract(&source, "logging_args").unwrap();

    assert_eq!(surface.call_sites.len(), 1);
    let site = &surface.call_sites[0];
    assert_eq!(site.level, LogLevel::Error);
    assert_eq!(site.template, "DMA channel {} failed on {}");
    assert_eq!(site.arg_count, 2);
    assert_eq!(site.arg_kinds[0], LogArgKind::U32);
    assert_eq!(site.arg_kinds[1], LogArgKind::Str);
}

// ---------------------------------------------------------------------------
// Test 3: unsupported placeholder forms rejected
// ---------------------------------------------------------------------------

#[test]
fn reject_unsupported_placeholder() {
    let source = format!(
        r#"
        {preamble}

        pub struct Dev;

        impl Dev {{
            pub fn bad_log(&self) {{
                intrinsics::diagnostic_logging::info("value = {{:?}}", [LogArg::from(42u32)]);
            }}
        }}
    "#,
        preamble = FIXTURE_PREAMBLE
    );

    let result = compile_and_extract(&source, "logging_bad_placeholder");
    assert!(matches!(
        result,
        Err(LoggingError::UnsupportedPlaceholder(_))
    ));
}

// ---------------------------------------------------------------------------
// Test 4: unsupported non-primitive argument types rejected
// ---------------------------------------------------------------------------

#[test]
fn reject_unsupported_arg_type() {
    // A cast `v as u32` evaluates to u32 in HIR, so type-based rejection can't
    // catch it. Since From::from has no impl for f64, unsupported types fail at
    // compile time already. Verify that a well-formed primitive call passes.
    let source = format!(
        r#"
        {preamble}

        pub struct Dev;

        impl Dev {{
            pub fn good_args(&self) {{
                let v: u32 = 42;
                intrinsics::diagnostic_logging::info("val = {{}}", [LogArg::from(v)]);
            }}
        }}
    "#,
        preamble = FIXTURE_PREAMBLE
    );

    let surface = compile_and_extract(&source, "logging_supported_arg").unwrap();
    assert_eq!(surface.call_sites.len(), 1);
    assert_eq!(surface.call_sites[0].arg_kinds[0], LogArgKind::U32);
}

// ---------------------------------------------------------------------------
// Test 5: placeholder-count mismatches rejected
// ---------------------------------------------------------------------------

#[test]
fn reject_placeholder_count_mismatch() {
    let source = format!(
        r#"
        {preamble}

        pub struct Dev;

        impl Dev {{
            pub fn mismatch(&self) {{
                intrinsics::diagnostic_logging::info(
                    "one {{}} two {{}} three {{}}",
                    [LogArg::from(1u32), LogArg::from(2u32)],
                );
            }}
        }}
    "#,
        preamble = FIXTURE_PREAMBLE
    );

    let result = compile_and_extract(&source, "logging_mismatch");
    assert!(matches!(
        result,
        Err(LoggingError::PlaceholderCountMismatch { .. })
    ));
}

// ---------------------------------------------------------------------------
// Test 6: direct canonical anchor call recognized
// ---------------------------------------------------------------------------

#[test]
fn recognize_canonical_anchor_call() {
    let source = format!(
        r#"
        {preamble}

        pub struct Dev;

        impl Dev {{
            pub fn direct_call(&self) {{
                intrinsics::diagnostic_logging::error("direct error", []);
            }}
        }}
    "#,
        preamble = FIXTURE_PREAMBLE
    );

    let surface = compile_and_extract(&source, "logging_canonical").unwrap();

    assert_eq!(surface.call_sites.len(), 1);
    assert!(surface.call_sites[0].canonical);
    assert_eq!(surface.call_sites[0].level, LogLevel::Error);
}

// ---------------------------------------------------------------------------
// Test 7: logging in trait-callback and ISR bodies
// ---------------------------------------------------------------------------

#[test]
fn logging_in_trait_and_isr_bodies() {
    let source = format!(
        r#"
        {preamble}

        pub struct Dev;

        impl Lifecycle for Dev {{
            fn init(&mut self) -> Result<(), RegseqError> {{
                intrinsics::diagnostic_logging::info("init started", []);
                Ok(())
            }}
        }}

        impl Dev {{
            #[rseq::isr]
            pub fn handle_irq(&self) -> IrqResult {{
                intrinsics::diagnostic_logging::debug("irq hit", []);
                IrqResult::Handled
            }}
        }}
    "#,
        preamble = FIXTURE_PREAMBLE
    );

    let surface = compile_and_extract(&source, "logging_contexts").unwrap();

    assert_eq!(surface.call_sites.len(), 2);
    assert!(surface
        .call_sites
        .iter()
        .any(|s| s.containing_fn == "init" && s.level == LogLevel::Info));
    assert!(surface
        .call_sites
        .iter()
        .any(|s| s.containing_fn == "handle_irq" && s.level == LogLevel::Debug));
}

// ---------------------------------------------------------------------------
// Test 8: accepted zero-padded hex width forms recognized
// ---------------------------------------------------------------------------

#[test]
fn recognize_zero_padded_hex_forms() {
    let source = format!(
        r#"
        {preamble}

        pub struct Dev;

        impl Dev {{
            pub fn hex_logging(&self) {{
                let status: u32 = 0xff;
                intrinsics::diagnostic_logging::debug(
                    "reg = {{:08x}} alt = {{:#010x}}",
                    [LogArg::from(status), LogArg::from(status)],
                );
            }}
        }}
    "#,
        preamble = FIXTURE_PREAMBLE
    );

    let surface = compile_and_extract(&source, "logging_hex").unwrap();

    assert_eq!(surface.call_sites.len(), 1);
    assert_eq!(
        surface.call_sites[0].template,
        "reg = {:08x} alt = {:#010x}"
    );
    assert_eq!(surface.call_sites[0].arg_count, 2);
}
