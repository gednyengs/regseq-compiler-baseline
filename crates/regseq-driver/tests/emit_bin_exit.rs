//! regseq-driver-90 Test Plan: `cargo-regseq --emit bin` exit-adapter parity.
//!
//! The `EmitSpec::Bin` arm of the emit dispatcher consumes the backend's
//! entry-aware shim shape (`emit_generic_c_for_entry`, regseq-backends-27) as
//! the single source of truth (contract C04): an error-returning entry
//! (`fn regseq_main() -> Result<(), RegseqError>`) gets the int shim
//! (`int main(void) { return <entry>__regseq_exit(); }`) plus the in-TU errno
//! adapter (regseq-backends-28), so a built binary's exit code maps per C02.
//!
//! These tests drive the production `RegseqCallbacks` end-to-end (bridge +
//! `framework_type` recognition + validate + emit) -- the same pipeline
//! `cargo-regseq` runs -- then link the two emitted halves (`generic.c` +
//! `generic_main.c`) with a host `cc` and assert the binary's exit code. The
//! `cc`-dependent legs skip cleanly when no host compiler is present.

#![feature(rustc_private)]

extern crate rustc_driver;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_session;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, Once};
use std::time::{SystemTime, UNIX_EPOCH};

use regseq_config::{ColorChoice, CompilerOptions, EmitSpec, LogLevel, RegseqParam, TargetOs};
use regseq_driver::RegseqCallbacks;

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
    std::env::temp_dir().join(format!("regseq-emit-bin-exit-{label}-{nanos}"))
}

/// Locate a host C compiler (`$CC` or `cc` on PATH). Returns `None` when none
/// is available, so the link/run legs degrade to a skip rather than a spurious
/// failure on a host without a `cc`. Mirrors
/// `crates/cargo-regseq/tests/generic_c_emit.rs::host_cc`.
fn host_cc() -> Option<PathBuf> {
    if let Some(cc) = std::env::var_os("CC") {
        return Some(PathBuf::from(cc));
    }
    let out = Command::new("sh")
        .args(["-c", "command -v cc"])
        .output()
        .ok()?;
    if out.status.success() {
        let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if !path.is_empty() {
            return Some(PathBuf::from(path));
        }
    }
    None
}

/// Resolve a symbolic errno identifier (e.g. `EINVAL`) to its numeric value on
/// the SAME `cc`/libc that links the fixture, by compiling and running a tiny
/// `printf("%d", <symbol>)` probe over `<errno.h>`. Mirrors the harness oracle
/// in `crates/codegen-coverage-runner/src/toolchain.rs::resolve_errno`, so the
/// C02 mapping is asserted against the host libc rather than a hardcoded
/// numeric (which is glibc-specific).
fn resolve_errno(symbol: &str, cc: &Path, dir: &Path) -> i32 {
    let probe = dir.join("errno_probe.c");
    let bin = dir.join("errno_probe_bin");
    std::fs::write(
        &probe,
        format!("#include <errno.h>\n#include <stdio.h>\nint main(void) {{ printf(\"%d\\n\", {symbol}); return 0; }}\n"),
    )
    .expect("write errno probe");
    let compile = Command::new(cc)
        .arg("-o")
        .arg(&bin)
        .arg(&probe)
        .output()
        .expect("spawn cc for errno probe");
    assert!(
        compile.status.success(),
        "errno probe failed to compile for {symbol}: {}",
        String::from_utf8_lossy(&compile.stderr),
    );
    let run = Command::new(&bin).output().expect("run errno probe");
    String::from_utf8_lossy(&run.stdout)
        .trim()
        .parse::<i32>()
        .unwrap_or_else(|e| panic!("errno probe for {symbol} printed a non-integer: {e}"))
}

/// The `CompilerOptions` the production CLI builds for a `--emit bin` run, with
/// `output_dir` pointed at the test's scratch directory. Mirrors
/// `crates/regseq-driver/tests/emit_dispatch.rs::base_options`.
fn bin_options(output_dir: PathBuf) -> CompilerOptions {
    CompilerOptions {
        target_os: TargetOs::Linux,
        platform_file: PathBuf::from("/dev/null"),
        output_dir,
        emit: vec![EmitSpec::Bin { name: None }],
        check: false,
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

/// Drive the production `RegseqCallbacks` over inline `source` with
/// `--emit bin`, writing the two emitted halves (`generic.c` + the entry shim
/// `generic_main.c`) into `out_dir`. Mirrors the in-process rustc-driver
/// pattern in `emit_dispatch.rs`, but feeds inline source so the entry's return
/// shape is under the test's control.
fn drive_emit_bin(crate_name: &str, source: &str, out_dir: &Path) {
    let _lock = COMPILER_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    init_rustc();

    let src_dir = fresh_dir(&format!("src-{crate_name}"));
    std::fs::create_dir_all(&src_dir).unwrap();
    let src_path = src_dir.join("lib.rs");
    std::fs::write(&src_path, source).unwrap();
    std::fs::create_dir_all(out_dir).unwrap();

    let rustc_out = fresh_dir(&format!("rustc-out-{crate_name}"));
    std::fs::create_dir_all(&rustc_out).unwrap();

    let sysroot = host_sysroot();
    let args = vec![
        "rustc".to_string(),
        src_path.to_str().expect("src path is UTF-8").to_string(),
        "--sysroot".to_string(),
        sysroot,
        "--edition".to_string(),
        "2021".to_string(),
        "--crate-type".to_string(),
        "rlib".to_string(),
        "--crate-name".to_string(),
        crate_name.to_string(),
        "--out-dir".to_string(),
        rustc_out.to_str().unwrap().to_string(),
    ];

    let mut callbacks = RegseqCallbacks::new(bin_options(out_dir.to_path_buf()));
    rustc_driver::run_compiler(&args, &mut callbacks);

    std::fs::remove_dir_all(&src_dir).ok();
    std::fs::remove_dir_all(&rustc_out).ok();
}

/// Link the driver-emitted TU (`generic.c`) and entry shim
/// (`generic_main.c`) with `cc` and run the binary, returning its exit code
/// (`None` if it died on a signal).
fn link_and_run(out_dir: &Path, cc: &Path, label: &str) -> Option<i32> {
    let tu = out_dir.join("generic.c");
    let shim = out_dir.join("generic_main.c");
    assert!(tu.exists(), "TU generic.c was not emitted ({label})");
    assert!(
        shim.exists(),
        "entry shim generic_main.c was not emitted ({label})"
    );
    let bin = out_dir.join(format!("{label}_bin"));
    let compile = Command::new(cc)
        .arg("-std=gnu11")
        .arg("-o")
        .arg(&bin)
        .arg(&tu)
        .arg(&shim)
        .output()
        .expect("spawn cc to link the emitted halves");
    assert!(
        compile.status.success(),
        "cc failed to link the emitted halves ({label}): {}",
        String::from_utf8_lossy(&compile.stderr),
    );
    let run = Command::new(&bin).output().expect("run the linked binary");
    run.status.code()
}

/// A `RegseqError` enum marking `#[rseq::framework_type("regseq-error")]` over
/// the full nine-variant C02 table, plus an error-returning `regseq_main`
/// entry that unconditionally returns the supplied variant. `no_std` keeps the
/// emitted TU minimal; the bridge stops before codegen so no panic handler is
/// needed.
fn err_entry_source(returned_variant: &str) -> String {
    format!(
        r#"#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

#[rseq::framework_type("regseq-error")]
pub enum RegseqError {{
    InvalidParameter,
    OutOfMemory,
    HardwareError,
    Timeout,
    Unsupported,
    DeviceBusy,
    PermissionDenied,
    DeviceNotFound,
    GenericFailure,
}}

pub fn regseq_main() -> Result<(), RegseqError> {{
    Err(RegseqError::{returned_variant})
}}
"#
    )
}

/// Test Plan 1 (C04/R10): `--emit bin` of an error-returning entry produces a
/// binary whose exit code maps per C02. `InvalidParameter` -> `EINVAL`; the
/// expected numeric is resolved against the same host libc via an errno probe.
#[test]
fn emit_bin_err_result_entry_exit_maps_per_c02() {
    let Some(cc) = host_cc() else {
        eprintln!("note: no host `cc` found; skipping the `--emit bin` exit-adapter parity test");
        return;
    };
    let out_dir = fresh_dir("err-result");
    std::fs::create_dir_all(&out_dir).unwrap();

    drive_emit_bin(
        "regseq_bin_exit_err",
        &err_entry_source("InvalidParameter"),
        &out_dir,
    );

    let expected = resolve_errno("EINVAL", &cc, &out_dir);
    let exit = link_and_run(&out_dir, &cc, "err_result");
    assert_eq!(
        exit,
        Some(expected),
        "an `Err(RegseqError::InvalidParameter)` entry must exit EINVAL ({expected}) per C02",
    );

    std::fs::remove_dir_all(&out_dir).ok();
}

/// Test Plan 1, a second C02 row (`Timeout` -> `ETIMEDOUT`) to confirm the
/// adapter switch maps the variant the entry actually returns, not a fixed
/// code.
#[test]
fn emit_bin_err_result_entry_maps_each_variant() {
    let Some(cc) = host_cc() else {
        eprintln!("note: no host `cc` found; skipping the `--emit bin` per-variant exit test");
        return;
    };
    let out_dir = fresh_dir("err-result-timeout");
    std::fs::create_dir_all(&out_dir).unwrap();

    drive_emit_bin(
        "regseq_bin_exit_timeout",
        &err_entry_source("Timeout"),
        &out_dir,
    );

    let expected = resolve_errno("ETIMEDOUT", &cc, &out_dir);
    let exit = link_and_run(&out_dir, &cc, "err_timeout");
    assert_eq!(
        exit,
        Some(expected),
        "an `Err(RegseqError::Timeout)` entry must exit ETIMEDOUT ({expected}) per C02",
    );

    std::fs::remove_dir_all(&out_dir).ok();
}

/// Test Plan 2 (I02): the unit-void `--emit bin` path is unchanged. The entry
/// shim is the byte-identical void shim and the linked binary exits 0.
#[test]
fn emit_bin_unit_void_entry_unchanged() {
    let source = r#"#![no_std]
#[no_mangle]
pub extern "C" fn regseq_main() {}
"#;
    let out_dir = fresh_dir("unit-void");
    std::fs::create_dir_all(&out_dir).unwrap();

    drive_emit_bin("regseq_bin_exit_void", source, &out_dir);

    // I02/R05: the driver must write exactly the backend's void shim, unchanged.
    // Assert byte-for-byte against the backend single-source-of-truth
    // (`generate_main_c(EntryShim::UnitVoid)`) for the crate-qualified entry
    // symbol the backend resolves (`sanitize("regseq_bin_exit_void::regseq_main")`,
    // mirroring `crates/cargo-regseq/tests/generic_c_emit.rs`'s
    // `regseq_gc_smoke_regseq_main`). A substring check would pass through a
    // changed shim; this pins the whole shim and the absence of the errno
    // adapter.
    let shim = std::fs::read_to_string(out_dir.join("generic_main.c"))
        .expect("entry shim generic_main.c must be emitted");
    let expected = regseq_backends::generate_main_c(&regseq_backends::EntryShim::unit_void(
        "regseq_bin_exit_void_regseq_main",
    ));
    assert_eq!(
        shim, expected,
        "the unit-void `--emit bin` shim must be byte-identical to the backend void shim (I02)",
    );

    if let Some(cc) = host_cc() {
        let exit = link_and_run(&out_dir, &cc, "unit_void");
        assert_eq!(exit, Some(0), "a unit-void entry must exit 0");
    } else {
        eprintln!("note: no host `cc` found; skipping the unit-void link/run leg");
    }

    std::fs::remove_dir_all(&out_dir).ok();
}
