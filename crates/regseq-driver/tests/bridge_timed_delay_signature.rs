//! Bridge integration for the timed-cpu-delay-and-sleep `TCD-ERR-02`
//! (intrinsic argument-type mismatch) signature check.
//!
//! Recognition of `busy_wait`/`sleep` is call-driven and runs after rustc's own
//! type-check, so a mistyped *call* through the canonical typed facade is
//! rejected by rustc first. `TCD-ERR-02`'s residual reachable case is a
//! **mis-declared** facade — a function tagged `#[rseq::intrinsic("busy_wait")]`
//! whose parameter is not a by-value `Duration`. These tests construct exactly
//! that, mirroring the device-interrupt `IRQ-ERR-ISR-SIGNATURE` reject tests.
//!
//! Covered: a correctly-typed `busy_wait(Duration)` facade is accepted; a
//! `u32` parameter, a `&Duration` (by-reference) parameter, and a two-parameter
//! declaration are each rejected with `TCD-ERR-02`.

#![feature(rustc_private)]

extern crate rustc_driver;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_session;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex, Once};
use std::time::{SystemTime, UNIX_EPOCH};

use regseq_driver::bridge::{run_bridge, BridgeInput};
use regseq_model::RegseqModel;
use rustc_middle::ty::TyCtxt;

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
    let output = Command::new("rustc")
        .arg("--print")
        .arg("sysroot")
        .output()
        .expect("failed to run rustc --print sysroot");
    String::from_utf8(output.stdout).unwrap().trim().to_string()
}

fn fresh_test_dir(crate_name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("regseq-bridge-tcd-sig-{crate_name}-{nanos}"))
}

/// Run the full bridge over `source`; on success apply `f` to the model, on
/// rejection return the rendered `BridgeError` for diagnostic assertions.
fn with_model<R, F>(crate_name: &str, source: &str, f: F) -> Result<R, String>
where
    R: Send + 'static,
    F: FnOnce(&RegseqModel) -> R + Send + 'static,
{
    let _lock = COMPILER_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    init_rustc();

    let test_dir = fresh_test_dir(crate_name);
    std::fs::create_dir_all(&test_dir).unwrap();
    let src_path = test_dir.join("lib.rs");
    std::fs::write(&src_path, source).unwrap();

    let result: Arc<Mutex<Option<Result<R, String>>>> = Arc::new(Mutex::new(None));
    let callback = Arc::new(Mutex::new(Some(
        Box::new(f) as Box<dyn FnOnce(&RegseqModel) -> R + Send>
    )));

    struct Callbacks<R: Send + 'static> {
        result: Arc<Mutex<Option<Result<R, String>>>>,
        callback: Arc<Mutex<Option<Box<dyn FnOnce(&RegseqModel) -> R + Send>>>>,
    }

    impl<R: Send + 'static> rustc_driver::Callbacks for Callbacks<R> {
        fn config(&mut self, config: &mut rustc_interface::interface::Config) {
            config.opts.unstable_opts.no_codegen = true;
            config.opts.unstable_opts.mir_opt_level = Some(1);
            config
                .opts
                .unstable_opts
                .crate_attr
                .push("allow(unused, non_snake_case)".to_string());
        }

        fn after_analysis<'tcx>(
            &mut self,
            _compiler: &rustc_interface::interface::Compiler,
            tcx: TyCtxt<'tcx>,
        ) -> rustc_driver::Compilation {
            let outcome = match run_bridge(BridgeInput::new(tcx)) {
                Ok(model) => {
                    let f = self.callback.lock().unwrap().take().expect("callback once");
                    Ok(f(&model))
                }
                Err(err) => Err(err.to_string()),
            };
            *self.result.lock().unwrap() = Some(outcome);
            rustc_driver::Compilation::Stop
        }
    }

    let sysroot = get_sysroot();
    let args = vec![
        "rustc".to_string(),
        src_path.to_str().unwrap().to_string(),
        "--sysroot".to_string(),
        sysroot,
        "--edition".to_string(),
        "2021".to_string(),
        "--crate-type".to_string(),
        "rlib".to_string(),
        "--crate-name".to_string(),
        crate_name.to_string(),
        "--out-dir".to_string(),
        test_dir.to_str().unwrap().to_string(),
    ];

    let mut callbacks = Callbacks {
        result: result.clone(),
        callback,
    };
    rustc_driver::run_compiler(&args, &mut callbacks);
    let output = result
        .lock()
        .unwrap()
        .take()
        .expect("after_analysis should produce a result");
    std::fs::remove_dir_all(Path::new(&test_dir)).ok();
    output
}

/// The shared author surfaces (lifecycle trait + RegseqError; Interrupt +
/// IrqResult), included exactly as the fixtures include them. A `busy_wait`
/// call site must live in a materialized body; the ISR handler body is
/// materialized (force-bound to its interrupt resource), so each case calls
/// the facade from there.
const LIFECYCLE_SURFACE: &str =
    include_str!("../../codegen-coverage-runner/fixtures/_lifecycle_surface.rs");
const INTERRUPT_SURFACE: &str =
    include_str!("../../codegen-coverage-runner/fixtures/_interrupt_surface.rs");

/// A minimal `Duration` framework type (the surfaces do not define one).
const DURATION: &str = r#"
#[rseq::framework_type("duration")]
pub struct Duration { ns: u64 }
impl Duration {
    pub const fn from_us(us: u64) -> Self { Self { ns: us.saturating_mul(1_000) } }
}
"#;

/// Build a bridgeable crate: the shared surfaces, an optional `Duration`, the
/// `busy_wait` facade declaration `facade`, and a one-interrupt driver whose
/// ISR body runs `call`.
fn source(define_duration: bool, facade: &str, call: &str) -> String {
    let duration = if define_duration { DURATION } else { "" };
    format!(
        "#![feature(register_tool)]\n#![register_tool(rseq)]\n\
         {LIFECYCLE_SURFACE}\n{INTERRUPT_SURFACE}\n{duration}\n{facade}\n\
         #[rseq::driver(name = \"d\", compatible = [\"acme,d\"])]\n\
         pub struct D {{ #[rseq::interrupt(id = \"irq\")] pub irq: Interrupt }}\n\
         impl D {{\n    #[rseq::isr]\n    pub fn handle(&mut self) -> IrqResult {{\n        {call}\n        IrqResult::Handled\n    }}\n}}\n\
         pub fn make() -> D {{ loop {{}} }}\npub fn entry() {{}}\n"
    )
}

/// Both correctly-typed delay facades + a `Duration`, for the TCD-ERR-01
/// context-legality tests (here recognition always succeeds; the question is
/// the call's *context*, not its signature).
const DELAY_FACADES: &str = r#"
#[rseq::framework_type("duration")]
pub struct Duration { ns: u64 }
impl Duration {
    pub const fn from_us(us: u64) -> Self { Self { ns: us.saturating_mul(1_000) } }
    pub const fn from_ms(ms: u64) -> Self { Self { ns: ms.saturating_mul(1_000_000) } }
}
#[rseq::intrinsic("busy_wait")]
pub fn busy_wait(_d: Duration) {}
#[rseq::intrinsic("sleep")]
pub fn sleep(_d: Duration) {}
"#;

/// A one-interrupt driver whose `#[rseq::isr]` handler body runs `isr_body` and
/// whose process-context `Lifecycle::on_init` runs `init_body` — so a test can
/// place a delay call in IRQ context vs process context.
fn isr_source(isr_body: &str, init_body: &str) -> String {
    format!(
        "#![feature(register_tool)]\n#![register_tool(rseq)]\n\
         {LIFECYCLE_SURFACE}\n{INTERRUPT_SURFACE}\n{DELAY_FACADES}\n\
         #[rseq::driver(name = \"d\", compatible = [\"acme,d\"])]\n\
         pub struct D {{ #[rseq::interrupt(id = \"irq\")] pub irq: Interrupt }}\n\
         impl D {{\n    #[rseq::isr]\n    pub fn handle(&mut self) -> IrqResult {{\n        {isr_body}\n        IrqResult::Handled\n    }}\n}}\n\
         impl Lifecycle for D {{\n    fn on_init(&mut self) -> Result<(), RegseqError> {{\n        {init_body}\n        Ok(())\n    }}\n}}\n\
         pub fn make() -> D {{ loop {{}} }}\npub fn entry() {{}}\n"
    )
}

#[test]
fn sleep_in_isr_body_is_rejected_verbatim() {
    // TCD-ERR-01 (C05): a `sleep` call lexically within the `#[rseq::isr]`
    // handler body is rejected with the VERBATIM diagnostic.
    let err = with_model(
        "tcd_sleep_isr",
        &isr_source("sleep(Duration::from_us(5));", ""),
        |_| (),
    )
    .expect_err("sleep in an ISR body must reject");
    assert!(
        err.contains(
            "TCD / Timed CPU Delay And Sleep: 'regseq::delay::sleep' is not legal in IRQ context \
             (call site reachable from rseq::isr per the callback-context model); \
             use 'regseq::delay::busy_wait' instead"
        ),
        "TCD-ERR-01 must render verbatim (C05); got: {err}"
    );
}

#[test]
fn busy_wait_in_isr_body_is_accepted() {
    // I07: busy_wait is legal in ANY context, including IRQ.
    let ok = with_model(
        "tcd_busy_isr",
        &isr_source("busy_wait(Duration::from_us(5));", ""),
        |_| (),
    );
    assert!(ok.is_ok(), "busy_wait in an ISR body must be accepted: {ok:?}");
}

#[test]
fn sleep_in_process_context_is_accepted() {
    // Only the marked IRQ region is restricted: a `sleep` in process-context
    // `on_init` is accepted (the ISR here legally busy-waits).
    let ok = with_model(
        "tcd_sleep_proc",
        &isr_source(
            "busy_wait(Duration::from_us(5));",
            "sleep(Duration::from_ms(5));",
        ),
        |_| (),
    );
    assert!(
        ok.is_ok(),
        "sleep in process-context on_init must be accepted: {ok:?}"
    );
}

#[test]
fn busy_wait_with_duration_arg_is_accepted() {
    // The canonical signature: exactly one by-value `Duration`. Accepted.
    let ok = with_model(
        "tcd_ok",
        &source(
            true,
            "#[rseq::intrinsic(\"busy_wait\")]\npub fn busy_wait(_d: Duration) {}",
            "busy_wait(Duration::from_us(10));",
        ),
        |_| (),
    );
    assert!(ok.is_ok(), "canonical busy_wait(Duration) must bridge: {ok:?}");
}

#[test]
fn busy_wait_with_non_duration_arg_is_rejected() {
    // A mis-declared facade taking `u32`: TCD-ERR-02.
    let err = with_model(
        "tcd_u32",
        &source(
            false,
            "#[rseq::intrinsic(\"busy_wait\")]\npub fn busy_wait(_x: u32) {}",
            "busy_wait(5);",
        ),
        |_| (),
    )
    .expect_err("u32 param must reject");
    assert!(err.contains("TCD-ERR-02"), "got: {err}");
}

#[test]
fn busy_wait_with_by_reference_duration_is_rejected() {
    // `&Duration` is not by-value Duration: `ty_adt_def()` is None for a
    // reference, so the framework-type match fails -> TCD-ERR-02.
    let err = with_model(
        "tcd_ref",
        &source(
            true,
            "#[rseq::intrinsic(\"busy_wait\")]\npub fn busy_wait(_d: &Duration) {}",
            "busy_wait(&Duration::from_us(10));",
        ),
        |_| (),
    )
    .expect_err("&Duration param must reject");
    assert!(err.contains("TCD-ERR-02"), "got: {err}");
}

#[test]
fn busy_wait_with_two_parameters_is_rejected() {
    // Arity is part of the contract: a second parameter -> TCD-ERR-02.
    let err = with_model(
        "tcd_arity",
        &source(
            true,
            "#[rseq::intrinsic(\"busy_wait\")]\npub fn busy_wait(_d: Duration, _n: u32) {}",
            "busy_wait(Duration::from_us(10), 5);",
        ),
        |_| (),
    )
    .expect_err("two-parameter busy_wait must reject");
    assert!(err.contains("TCD-ERR-02"), "got: {err}");
}
