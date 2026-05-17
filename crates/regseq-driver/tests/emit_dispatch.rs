//! End-to-end integration test for the regseq-driver emit
//! dispatcher (regseq-driver-52 Test Plan D14-D17).
//!
//! Mirrors the in-process rustc-driver invocation pattern in
//! `bridge_vtable_indirect_end_to_end.rs:63-141` but drives the
//! production `RegseqCallbacks::new(options)` so that bridge +
//! validate + warn + emit run in their real order. The test does
//! NOT shell out to `cargo regseq`.

#![feature(rustc_private)]

extern crate rustc_driver;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_session;

use std::path::PathBuf;
use std::process::Command;
use std::sync::{Mutex, Once};
use std::time::{SystemTime, UNIX_EPOCH};

use regseq_config::{ColorChoice, CompilerOptions, EmitSpec, LogLevel, RegseqParam, TargetOs};
use regseq_driver::RegseqCallbacks;
use regseq_model::RegseqModel;
use test_ir_fixtures::fixture_names;

static INIT: Once = Once::new();
static COMPILER_LOCK: Mutex<()> = Mutex::new(());

fn init_rustc() {
    INIT.call_once(|| {
        let early_dcx =
            rustc_session::EarlyDiagCtxt::new(rustc_session::config::ErrorOutputType::default());
        rustc_driver::init_rustc_env_logger(&early_dcx);
    });
}

fn host_sysroot() -> String {
    let output = Command::new("rustc")
        .arg("--print")
        .arg("sysroot")
        .output()
        .expect("failed to run rustc --print sysroot");
    String::from_utf8(output.stdout).unwrap().trim().to_string()
}

fn fresh_dir(label: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time is past UNIX epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("regseq-emit-dispatch-{label}-{nanos}"))
}

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("test-ir-fixtures")
        .join("fixtures")
        .join(format!("{name}.rs"))
}

fn base_options(emit: Vec<EmitSpec>, check: bool, output_dir: PathBuf) -> CompilerOptions {
    CompilerOptions {
        target_os: TargetOs::Linux,
        platform_file: PathBuf::from("/dev/null"),
        output_dir,
        emit,
        check,
        add_analysis: Vec::new(),
        skip_analysis: Vec::new(),
        deny: Vec::new(),
        allow: Vec::new(),
        params: Vec::<RegseqParam>::new(),
        log_level: LogLevel::Warn,
        dump_stage: None,
        color: ColorChoice::Never,
    }
}

fn drive_with_regseq_callbacks(fixture: &str, options: CompilerOptions) {
    let _lock = COMPILER_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    init_rustc();

    let src_path = fixture_path(fixture);
    let out_dir = fresh_dir(&format!("rustc-out-{fixture}"));
    std::fs::create_dir_all(&out_dir).unwrap();

    let sysroot = host_sysroot();
    let args = vec![
        "rustc".to_string(),
        src_path
            .to_str()
            .expect("fixture path is valid UTF-8")
            .to_string(),
        "--sysroot".to_string(),
        sysroot,
        "--edition".to_string(),
        "2021".to_string(),
        "--crate-type".to_string(),
        "rlib".to_string(),
        "--crate-name".to_string(),
        fixture.to_string(),
        "--out-dir".to_string(),
        out_dir.to_str().unwrap().to_string(),
    ];

    let mut callbacks = RegseqCallbacks::new(options);
    rustc_driver::run_compiler(&args, &mut callbacks);

    std::fs::remove_dir_all(&out_dir).ok();
}

#[test]
fn emit_dispatch_model_writes_pretty_print_for_no_op_fixture() {
    let out_dir = fresh_dir("model");
    let dest = out_dir.join("model.txt");
    let options = base_options(
        vec![EmitSpec::Model {
            path: Some(dest.clone()),
        }],
        false,
        out_dir.clone(),
    );
    drive_with_regseq_callbacks(fixture_names::NO_OP, options);

    let contents = std::fs::read_to_string(&dest).expect("model.txt must exist");
    assert!(
        !contents.is_empty(),
        "pretty-printed model must be non-empty"
    );
    assert!(
        contents.contains("fn"),
        "rendered model must contain the `fn` keyword per pretty-printer.md section 10.2.1: {contents}"
    );
    assert!(
        contents.contains("no_op"),
        "rendered model must reference the no_op function: {contents}"
    );
    let _ = std::fs::remove_dir_all(&out_dir);
}

#[test]
fn emit_dispatch_json_writes_deserializable_model_for_no_op_fixture() {
    let out_dir = fresh_dir("json");
    let dest = out_dir.join("model.json");
    let options = base_options(
        vec![EmitSpec::Json {
            path: Some(dest.clone()),
        }],
        false,
        out_dir.clone(),
    );
    drive_with_regseq_callbacks(fixture_names::NO_OP, options);

    let file = std::fs::File::open(&dest).expect("model.json must exist");
    let _decoded: RegseqModel =
        serde_json::from_reader(file).expect("model.json must deserialize into RegseqModel");
    let _ = std::fs::remove_dir_all(&out_dir);
}

#[test]
fn emit_dispatch_check_flag_skips_all_emission() {
    let out_dir = fresh_dir("check");
    let dest = out_dir.join("should_not_exist.txt");
    let options = base_options(
        vec![EmitSpec::Model {
            path: Some(dest.clone()),
        }],
        true,
        out_dir.clone(),
    );
    drive_with_regseq_callbacks(fixture_names::NO_OP, options);
    // `--check` short-circuits before emission; the destination
    // file must not exist. Reaching this assertion at all also
    // confirms the bridge ran without fataling.
    assert!(
        !dest.exists(),
        "--check must skip emission; expected absent: {}",
        dest.display()
    );
    let _ = std::fs::remove_dir_all(&out_dir);
}

#[test]
fn emit_dispatch_unsupported_driver_spec_is_a_silent_no_op() {
    let out_dir = fresh_dir("unsupported");
    let options = base_options(
        vec![EmitSpec::Driver { match_id: None }],
        false,
        out_dir.clone(),
    );
    // Must not panic; the dispatcher returns Ok(()) silently for
    // Driver. The user-facing warning string is asserted in
    // `crates/regseq-driver/src/emit.rs::tests::collect_unsupported_warnings_*`
    // because capturing rustc's `tcx.dcx().warn` output here is
    // brittle in the in-process test harness (per Test Plan D17).
    drive_with_regseq_callbacks(fixture_names::NO_OP, options);
    let _ = std::fs::remove_dir_all(&out_dir);
}
