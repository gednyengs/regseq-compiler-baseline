#![feature(rustc_private)]

extern crate rustc_ast;
extern crate rustc_driver;
extern crate rustc_hir;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_session;

use std::sync::{Arc, Mutex, Once};

#[path = "../src/extract/callback_context.rs"]
mod callback_context;

#[path = "../src/extract/timed_delay.rs"]
mod timed_delay;

use timed_delay::*;

type ExtractionResult = Result<TimedDelaySurface, TimedDelayError>;

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
            *self.result.lock().unwrap() = Some(extract_timed_delay_surface(tcx));
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
        pub const fn from_s(s: u64) -> Self { Self { us: s * 1_000_000 } }
    }

    pub mod intrinsics {
        pub mod timed_delay {
            use super::super::Duration;
            pub fn busy_wait(_d: Duration) {}
            pub fn sleep(_d: Duration) {}
        }
    }

    pub mod delay {
        pub use super::intrinsics::timed_delay::busy_wait;
        pub use super::intrinsics::timed_delay::sleep;
    }

    pub enum IrqResult { Handled, None }
"#;

// ---------------------------------------------------------------------------
// Test 1: canonical and compatibility paths recognized
// ---------------------------------------------------------------------------

#[test]
fn recognize_canonical_and_compatibility_paths() {
    let source = format!(
        r#"
        {preamble}

        pub struct Dev;

        impl Dev {{
            pub fn do_canonical(&self) {{
                intrinsics::timed_delay::busy_wait(Duration::from_us(10));
                intrinsics::timed_delay::sleep(Duration::from_ms(100));
            }}

            pub fn do_compat(&self) {{
                delay::busy_wait(Duration::from_us(5));
                delay::sleep(Duration::from_ms(50));
            }}
        }}
    "#,
        preamble = FIXTURE_PREAMBLE
    );

    let surface = compile_and_extract(&source, "timed_delay_paths").unwrap();

    assert_eq!(surface.call_sites.len(), 4);

    assert!(surface
        .call_sites
        .iter()
        .any(|s| s.intrinsic == DelayIntrinsic::BusyWait
            && s.path_kind == DelayPathKind::Canonical
            && s.containing_fn == "do_canonical"));
    assert!(surface
        .call_sites
        .iter()
        .any(|s| s.intrinsic == DelayIntrinsic::Sleep
            && s.path_kind == DelayPathKind::Canonical
            && s.containing_fn == "do_canonical"));
    assert!(surface
        .call_sites
        .iter()
        .any(|s| s.intrinsic == DelayIntrinsic::BusyWait
            && s.path_kind == DelayPathKind::Compatibility
            && s.containing_fn == "do_compat"));
    assert!(surface
        .call_sites
        .iter()
        .any(|s| s.intrinsic == DelayIntrinsic::Sleep
            && s.path_kind == DelayPathKind::Compatibility
            && s.containing_fn == "do_compat"));
}

// ---------------------------------------------------------------------------
// Test 2: Duration constructor sites recognized
// ---------------------------------------------------------------------------

#[test]
fn recognize_duration_constructors() {
    let source = format!(
        r#"
        {preamble}

        pub struct Dev;

        impl Dev {{
            pub fn use_durations(&self) {{
                let _a = Duration::from_us(10);
                let _b = Duration::from_ms(20);
                let _c = Duration::from_s(1);
            }}
        }}
    "#,
        preamble = FIXTURE_PREAMBLE
    );

    let surface = compile_and_extract(&source, "timed_delay_ctors").unwrap();

    assert_eq!(surface.duration_ctors.len(), 3);

    assert!(surface
        .duration_ctors
        .iter()
        .any(|s| s.ctor == DurationCtor::FromUs && s.containing_fn == "use_durations"));
    assert!(surface
        .duration_ctors
        .iter()
        .any(|s| s.ctor == DurationCtor::FromMs && s.containing_fn == "use_durations"));
    assert!(surface
        .duration_ctors
        .iter()
        .any(|s| s.ctor == DurationCtor::FromS && s.containing_fn == "use_durations"));
}

// ---------------------------------------------------------------------------
// Test 3: sleep accepted in sleepable trait-callback contexts
// ---------------------------------------------------------------------------

#[test]
fn sleep_accepted_in_sleepable_contexts() {
    let source = format!(
        r#"
        {preamble}

        pub trait Lifecycle {{
            fn init(&mut self) -> Result<(), RegseqError>;
        }}

        pub struct Dev;

        impl Lifecycle for Dev {{
            fn init(&mut self) -> Result<(), RegseqError> {{
                delay::sleep(Duration::from_ms(10));
                Ok(())
            }}
        }}
    "#,
        preamble = FIXTURE_PREAMBLE
    );

    let surface = compile_and_extract(&source, "timed_delay_sleepable").unwrap();

    assert_eq!(surface.call_sites.len(), 1);
    assert_eq!(surface.call_sites[0].intrinsic, DelayIntrinsic::Sleep);
    assert_eq!(surface.call_sites[0].containing_fn, "init");
}

// ---------------------------------------------------------------------------
// Test 4: sleep rejected in ISR context
// ---------------------------------------------------------------------------

#[test]
fn sleep_rejected_in_isr_context() {
    let source = format!(
        r#"
        {preamble}

        pub struct Dev;

        impl Dev {{
            #[rseq::isr]
            pub fn handle_irq(&self) -> IrqResult {{
                delay::sleep(Duration::from_ms(10));
                IrqResult::Handled
            }}
        }}
    "#,
        preamble = FIXTURE_PREAMBLE
    );

    let result = compile_and_extract(&source, "timed_delay_isr_sleep");
    assert!(matches!(
        result,
        Err(TimedDelayError::SleepInHandlerContext(_))
    ));
}

// ---------------------------------------------------------------------------
// Test 5: busy_wait accepted in both sleepable and ISR contexts
// ---------------------------------------------------------------------------

#[test]
fn busy_wait_accepted_in_all_contexts() {
    let source = format!(
        r#"
        {preamble}

        pub trait Lifecycle {{
            fn init(&mut self) -> Result<(), RegseqError>;
        }}

        pub struct Dev;

        impl Lifecycle for Dev {{
            fn init(&mut self) -> Result<(), RegseqError> {{
                delay::busy_wait(Duration::from_us(10));
                Ok(())
            }}
        }}

        impl Dev {{
            #[rseq::isr]
            pub fn handle_irq(&self) -> IrqResult {{
                delay::busy_wait(Duration::from_us(5));
                IrqResult::Handled
            }}
        }}
    "#,
        preamble = FIXTURE_PREAMBLE
    );

    let surface = compile_and_extract(&source, "timed_delay_busywait_all").unwrap();

    assert_eq!(surface.call_sites.len(), 2);
    assert!(surface
        .call_sites
        .iter()
        .all(|s| s.intrinsic == DelayIntrinsic::BusyWait));
    assert!(surface.call_sites.iter().any(|s| s.containing_fn == "init"));
    assert!(surface
        .call_sites
        .iter()
        .any(|s| s.containing_fn == "handle_irq"));
}
