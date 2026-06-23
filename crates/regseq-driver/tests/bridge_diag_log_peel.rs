#![feature(rustc_private)]

//! Integration tests for the DiagLogPeel body pass, the format-template
//! grammar parser, and the bridge-side validation diagnostics per
//! spec.md sections 25.7.3, 26.6, 26.7, and 26.8.
//!
//! Scope coverage:
//!
//! - The format-template parser is exercised through its public
//!   surface (`parse_format_template`) -- the same surface the
//!   integration-level peel + validation pipeline routes through.
//! - The validation surface (`validate_log_call`) is exercised via
//!   `with_bridge` so the test calls the public function with a real
//!   `BridgeCtxt` reference.
//! - The peel pass's Phase-5 wiring is exercised by running
//!   `run_bridge` on a fixture that uses the same simplified macro
//!   shape as `bridge_diag_log_recognition.rs` (empty `&[]` slice
//!   arguments). The fixture verifies that the peel pass runs as a
//!   no-op on call sites whose carrier slice is the empty literal
//!   (the macro fixture in this crate does not synthesise `LogArg`
//!   `From` impls; that would require integrating the full LogArg
//!   carrier surface which is out of scope for regseq-driver-45 per
//!   the task spec's Non-Goals).
//!
//! The richer D1-D8 negative-coverage matrix listed in the task
//! spec's Test Plan requires fixtures with the complete `LogArg` +
//! `From` impl surface and exercises operand-type recovery (D2/D4),
//! the pre-lowering MIR Constant shape (D6), and per-target
//! string-arg bounds (D8). Those surfaces are still in flight (the
//! signatures landed with regseq-driver-44 / 45 and the operand-type
//! recovery + target-binding wiring is owned by phase-1b backend
//! tasks); the negative coverage will land alongside those tasks.

extern crate rustc_driver;
extern crate rustc_hir;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_session;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex, Once};
use std::time::{SystemTime, UNIX_EPOCH};

use regseq_driver::bridge::diag_log::{parse_format_template, validate_log_call, FormatSpecifier};
use regseq_driver::bridge::{
    run_bridge, BridgeCtxt, BridgeError, BridgeInput, DiagLogFormatReason,
    DiagLogValidationDiagnostic,
};
use regseq_model::{AttrData, IntrinsicKind, LogLevel};
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
    std::env::temp_dir().join(format!("regseq-bridge-diag-log-peel-{crate_name}-{nanos}"))
}

fn with_bridge<R, F>(crate_name: &str, source: &str, f: F) -> R
where
    R: Send + 'static,
    F: for<'tcx> FnOnce(TyCtxt<'tcx>, &mut BridgeCtxt<'tcx>) -> R + Send + 'static,
{
    let _lock = COMPILER_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    init_rustc();

    let test_dir = fresh_test_dir(crate_name);
    std::fs::create_dir_all(&test_dir).unwrap();
    let src_path = test_dir.join("lib.rs");
    std::fs::write(&src_path, source).unwrap();

    let result = Arc::new(Mutex::new(None));
    let callback =
        Arc::new(Mutex::new(Some(Box::new(f)
            as Box<
                dyn for<'tcx> FnOnce(TyCtxt<'tcx>, &mut BridgeCtxt<'tcx>) -> R + Send,
            >)));

    struct Callbacks<R: Send + 'static> {
        result: Arc<Mutex<Option<R>>>,
        callback: Arc<
            Mutex<
                Option<Box<dyn for<'tcx> FnOnce(TyCtxt<'tcx>, &mut BridgeCtxt<'tcx>) -> R + Send>>,
            >,
        >,
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
            let input = BridgeInput::new(tcx);
            let mut cx = BridgeCtxt::new(input);
            let f = self.callback.lock().unwrap().take().expect("callback once");
            *self.result.lock().unwrap() = Some(f(tcx, &mut cx));
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

/// Build a Regseq driver fixture with the four severity-named log
/// macros, the canonical `log_<sev>` intrinsic stubs, and the
/// `LogArg::Str` carrier. The macros expand to `&[]` slice
/// arguments per the existing `bridge_diag_log_recognition.rs`
/// fixture: this exercises the recognition + peel-pass wiring
/// end-to-end without requiring the full `From` impl matrix.
fn source_with_calls(calls: &str) -> String {
    format!(
        r#"
#![feature(register_tool)]
#![register_tool(rseq)]

pub mod regseq {{
    pub mod log {{
        pub mod intrinsics {{
            pub enum LogArg<'a> {{
                Str(&'a str),
            }}

            #[inline(never)]
            #[rseq::intrinsic("log-debug")]
            pub fn log_debug(_fmt: &'static str, _args: &[LogArg<'_>]) {{ unimplemented!() }}

            #[inline(never)]
            #[rseq::intrinsic("log-info")]
            pub fn log_info(_fmt: &'static str, _args: &[LogArg<'_>]) {{ unimplemented!() }}

            #[inline(never)]
            #[rseq::intrinsic("log-warn")]
            pub fn log_warn(_fmt: &'static str, _args: &[LogArg<'_>]) {{ unimplemented!() }}

            #[inline(never)]
            #[rseq::intrinsic("log-error")]
            pub fn log_error(_fmt: &'static str, _args: &[LogArg<'_>]) {{ unimplemented!() }}
        }}

        macro_rules! debug {{
            ($fmt:literal $(, $arg:expr)* $(,)?) => {{
                $crate::regseq::log::intrinsics::log_debug($fmt, &[])
            }};
        }}
        pub(crate) use debug;

        macro_rules! info {{
            ($fmt:literal $(, $arg:expr)* $(,)?) => {{
                $crate::regseq::log::intrinsics::log_info($fmt, &[])
            }};
        }}
        pub(crate) use info;

        macro_rules! log_warn {{
            ($fmt:literal $(, $arg:expr)* $(,)?) => {{
                $crate::regseq::log::intrinsics::log_warn($fmt, &[])
            }};
        }}
        pub(crate) use log_warn as warn;

        macro_rules! error {{
            ($fmt:literal $(, $arg:expr)* $(,)?) => {{
                $crate::regseq::log::intrinsics::log_error($fmt, &[])
            }};
        }}
        pub(crate) use error;
    }}
}}

#[no_mangle]
pub extern "C" fn caller() {{
    {calls}
}}
"#
    )
}

#[test]
fn parse_format_template_recognises_closed_subset() {
    // `regseq::log::error!("x = {{}}, y = {{:x}}", x, y)` template:
    // assert the parser produces two specifiers (Display, Hex).
    let parsed = parse_format_template("x = {}, y = {:x}").expect("template parses");
    assert_eq!(parsed.specifiers.len(), 2);
    assert_eq!(parsed.specifiers[0], FormatSpecifier::Display);
    assert_eq!(parsed.specifiers[1], FormatSpecifier::Hex);
}

#[test]
fn parse_format_template_recognises_hex_formatted_modifier() {
    // `regseq::log::info!("addr = {{:#010x}}", a)` template: assert
    // the parser produces a HexFormatted specifier with the parsed
    // fill / width / alternate fields.
    let parsed = parse_format_template("addr = {:#010x}").expect("template parses");
    assert_eq!(parsed.specifiers.len(), 1);
    assert_eq!(
        parsed.specifiers[0],
        FormatSpecifier::HexFormatted {
            fill: '0',
            width: 10,
            alternate: true,
            upper: false,
        }
    );
}

#[test]
fn parse_format_template_rejects_debug_formatter_per_d3() {
    let err = parse_format_template("x = {:?}").expect_err("D3 rejects {:?}");
    match err {
        BridgeError::DiagLogFormat { reason, .. } => {
            assert_eq!(reason, DiagLogFormatReason::DebugFormatterRejected);
        }
        other => panic!("expected DiagLogFormat, got {other:?}"),
    }
}

#[test]
fn parse_format_template_rejects_positional_arg() {
    let err = parse_format_template("{0}").expect_err("positional rejected");
    match err {
        BridgeError::DiagLogFormat { reason, .. } => {
            assert_eq!(reason, DiagLogFormatReason::PositionalArgRejected);
        }
        other => panic!("expected DiagLogFormat, got {other:?}"),
    }
}

#[test]
fn validate_log_call_d1_specifier_arg_count_mismatch() {
    let parsed = parse_format_template("x = {}").expect("template parses");
    let template = parsed;
    let validation_result = with_bridge(
        "bridge_diag_log_peel_validate_d1",
        &source_with_calls(r#"regseq::log::debug!("placeholder");"#),
        move |_tcx, cx| {
            // D1: template has 1 specifier, but caller passes 2 args.
            validate_log_call(cx, &template, 2)
        },
    );
    let err = validation_result.expect_err("D1 should fire on count mismatch");
    match err {
        BridgeError::DiagLogValidation { diagnostic, .. } => {
            assert_eq!(diagnostic, DiagLogValidationDiagnostic::D1);
        }
        other => panic!("expected DiagLogValidation, got {other:?}"),
    }
}

#[test]
fn validate_log_call_accepts_matching_counts() {
    let parsed = parse_format_template("x = {}, y = {:x}").expect("template parses");
    let template = parsed;
    let validation_result = with_bridge(
        "bridge_diag_log_peel_validate_ok",
        &source_with_calls(r#"regseq::log::info!("placeholder");"#),
        move |_tcx, cx| validate_log_call(cx, &template, 2),
    );
    validation_result.expect("matching counts validate");
}

#[test]
fn validate_log_call_d7_trailing_newline() {
    let parsed = parse_format_template("hello\n").expect("template parses");
    let template = parsed;
    let validation_result = with_bridge(
        "bridge_diag_log_peel_validate_d7",
        &source_with_calls(r#"regseq::log::warn!("placeholder");"#),
        move |_tcx, cx| validate_log_call(cx, &template, 0),
    );
    let err = validation_result.expect_err("D7 should fire on trailing newline");
    match err {
        BridgeError::DiagLogValidation { diagnostic, .. } => {
            assert_eq!(diagnostic, DiagLogValidationDiagnostic::D7);
        }
        other => panic!("expected DiagLogValidation, got {other:?}"),
    }
}

#[test]
fn run_bridge_phase5_peels_all_four_severities() {
    // All-four-severities fixture (per the task spec Test Plan):
    // one function calling each of debug / info / warn / error.
    // The empty-slice fixture means each Call's carrier is the
    // empty literal; the peel pass is a no-op on bodies whose
    // carrier slice does not trace to an Aggregate(Array, _) (the
    // empty `&[]` literal lowers to a different shape).
    //
    // What this exercises end-to-end:
    // - The peel pass is wired into Phase 5 between the
    //   vtable-indirect rewrite and joint validation.
    // - Running `run_bridge` against a fixture that contains
    //   Log-intrinsic Calls does NOT cause Phase-5 to fail.
    // - The recognition layer (regseq-driver-44) attaches
    //   AttrData::Intrinsic(IntrinsicKind::Log(_)) to each of the
    //   four callee Items so the peel pass sees them.
    let levels = with_bridge(
        "bridge_diag_log_peel_phase5_all_severities",
        &source_with_calls(
            r#"
            regseq::log::debug!("debug");
            regseq::log::info!("info");
            regseq::log::warn!("warn");
            regseq::log::error!("error");
            "#,
        ),
        |tcx, _cx| {
            let model = run_bridge(BridgeInput::new(tcx)).expect("Phase-5 succeeds");
            let mut seen: Vec<LogLevel> = Vec::new();
            for attrs in model.domain_attrs.item_attrs.iter() {
                for attr in attrs.iter() {
                    match attr {
                        AttrData::Intrinsic(IntrinsicKind::Log(level)) => seen.push(level.clone()),
                        // A non-log intrinsic (mmio / poll); skip it.
                        AttrData::Intrinsic(_) => {}
                        // Not a log level; this fixture produces none, so skip.
                        AttrData::FrameworkType(_) => {}
                        // Interrupt-construct markers (#6); this fixture
                        // produces none, so skip.
                        AttrData::InterruptResource { .. } => {}
                        AttrData::IsrBinding { .. } => {}
                        // Atomicity field markers (#7); this fixture produces
                        // none, so skip.
                        AttrData::Atomic { .. } => {}
                        // Synchronization marker; this fixture produces none.
                        AttrData::SyncResource { .. } => {}
                        // Clock-construct marker; this fixture produces none.
                        AttrData::ClockResource { .. } => {}
                        // Pinctrl-construct markers; this fixture produces none.
                        AttrData::PinctrlResource { .. } => {}
                        AttrData::PinStateMarker { .. } => {}
                        // DMA-management markers; this fixture produces none.
                        AttrData::DmaRegionResource { .. } => {}
                        AttrData::DmaChannelResource { .. } => {}
                        AttrData::DmaCompletionBinding { .. } => {}
                        // Deferred-work handler marker; this fixture produces none.
                        AttrData::WorkHandler { .. } => {}
                    }
                }
            }
            seen
        },
    );

    assert!(levels.contains(&LogLevel::Debug));
    assert!(levels.contains(&LogLevel::Info));
    assert!(levels.contains(&LogLevel::Warn));
    assert!(levels.contains(&LogLevel::Error));
}
