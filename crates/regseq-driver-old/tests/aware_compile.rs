//! End-to-end test for `DriverMode::RegseqAware`.
//!
//! Builds the `regseq-driver` binary and uses it as `RUSTC_WRAPPER` for
//! a `cargo build -p regseq` run against the real workspace. The
//! `regseq` facade declares `[package.metadata.regseq]`, so under the
//! dispatch tier its compilation must route through
//! `RegseqAwareCallbacks` rather than `PassthroughCallbacks`.
//!
//! This test is slow (full cargo build) so the end-to-end variant is
//! gated behind `#[ignore]`. Run with:
//!
//!     cargo test -p regseq-driver --test aware_compile -- --ignored
//!
//! In the default test run the lighter-weight smoke test
//! `regseq_aware_callbacks_compile_simple_no_std_lib` exercises the
//! same callback configuration directly against an in-process compile,
//! providing fast coverage that the callback configuration itself is
//! valid.

#![feature(rustc_private)]

extern crate rustc_driver;
extern crate rustc_interface;
extern crate rustc_session;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, Once};

use regseq_driver_old::RegseqAwareCallbacks;

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

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

/// Smoke check: the callback configuration produces a valid compile
/// for a self-contained `#![no_std]` library and emits a non-trivial
/// rlib. This is the fast (in-process) coverage that runs by default.
#[test]
fn regseq_aware_callbacks_compile_simple_no_std_lib() {
    let _lock = COMPILER_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    init_rustc();

    let test_dir = std::env::temp_dir().join("regseq-aware-compile-smoke");
    if test_dir.exists() {
        std::fs::remove_dir_all(&test_dir).unwrap();
    }
    std::fs::create_dir_all(&test_dir).unwrap();

    // Self-contained source mirroring how a regseq-aware library crate
    // looks: `#![no_std]`, declares `feature(register_tool)` and
    // `register_tool(rseq)` itself (the symmetric rule), and exposes a
    // simple helper function whose MIR should be encoded.
    let source = r#"#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

#[inline(never)]
pub fn add(a: u32, b: u32) -> u32 {
    a.wrapping_add(b)
}
"#;
    let src_path = test_dir.join("aware_lib.rs");
    std::fs::write(&src_path, source).unwrap();

    let sysroot = get_sysroot();
    let out_dir = test_dir.clone();
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
        "aware_lib".to_string(),
        "--out-dir".to_string(),
        out_dir.to_str().unwrap().to_string(),
    ];

    rustc_driver::run_compiler(&args, &mut RegseqAwareCallbacks);

    let rlib_path = out_dir.join("libaware_lib.rlib");
    assert!(
        rlib_path.exists(),
        "expected rlib at {} after RegseqAware compile",
        rlib_path.display(),
    );
    let metadata = std::fs::metadata(&rlib_path).expect("rlib metadata");
    assert!(metadata.len() > 0, "rlib at {} is empty", rlib_path.display());

    std::fs::remove_dir_all(&test_dir).ok();
}

/// End-to-end: build the regseq-driver wrapper, then run
/// `cargo build -p regseq` with `RUSTC_WRAPPER` pointing at it. The
/// facade declares `[package.metadata.regseq]` (see
/// `crates/regseq/Cargo.toml`), so its rustc invocation must route
/// through `RegseqAwareCallbacks` and produce a valid rlib.
///
/// `#[ignore]` because this drives a real cargo build and is slow.
#[test]
#[ignore]
fn cargo_build_facade_under_regseq_aware_wrapper() {
    let workspace = workspace_root();

    // Step 1: build regseq-driver so we can use it as RUSTC_WRAPPER.
    let status = Command::new("cargo")
        .args(["build", "-p", "regseq-driver"])
        .current_dir(&workspace)
        .status()
        .expect("cargo build -p regseq-driver");
    assert!(status.success(), "failed to build regseq-driver");

    let driver = workspace.join("target/debug/regseq-driver");
    assert!(driver.exists(), "driver binary not at {}", driver.display());

    // Step 2: wipe the regseq facade fingerprint so the wrapper is
    // actually invoked on the next build.
    let fingerprint_dir = workspace.join("target/debug/.fingerprint");
    if fingerprint_dir.exists() {
        for entry in std::fs::read_dir(&fingerprint_dir).expect("read fingerprint dir") {
            let entry = entry.expect("fingerprint dir entry");
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.starts_with("regseq-") && !name_str.contains("regseq-driver") {
                let _ = std::fs::remove_dir_all(entry.path());
            }
        }
    }

    // Step 3: build the facade with the wrapper.
    let mut cmd = Command::new("cargo");
    cmd.args(["build", "-p", "regseq"])
        .current_dir(&workspace)
        .env("RUSTC_WRAPPER", &driver)
        // REGSEQ_AWARE_CRATES is normally set by cargo-regseq. Set it
        // here so the wrapper picks RegseqAware for the facade.
        .env("REGSEQ_AWARE_CRATES", "regseq");

    let output = cmd.output().expect("cargo build -p regseq under wrapper");
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "cargo build -p regseq failed under RegseqAware wrapper.\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );

    // Step 4: confirm the facade rlib exists.
    let mut found_rlib = None;
    let deps_dir = workspace.join("target/debug/deps");
    for entry in std::fs::read_dir(&deps_dir).expect("read deps dir") {
        let entry = entry.expect("deps entry");
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str.starts_with("libregseq-") && name_str.ends_with(".rlib") {
            found_rlib = Some(entry.path());
            break;
        }
    }
    let rlib = found_rlib.unwrap_or_else(|| {
        panic!(
            "no libregseq-*.rlib produced under {} after RegseqAware compile",
            deps_dir.display(),
        )
    });
    let meta = std::fs::metadata(&rlib).expect("rlib metadata");
    assert!(meta.len() > 0, "facade rlib {} is empty", rlib.display());
}
