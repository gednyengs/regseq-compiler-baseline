//! End-to-end test: regseq-driver as RUSTC_WRAPPER.
//!
//! Builds the regseq-driver binary, then runs `cargo check` with
//! `RUSTC_WRAPPER` pointing at it against the fixture project at
//! `tests/fixtures/minimal-driver`. Verifies the driver reaches
//! `after_analysis` for the target crate while dependencies compile
//! normally via passthrough.
//!
//! This test is slow (full cargo build) so it is gated behind `#[ignore]`.
//! Run with: `cargo test -p regseq-e2e -- --ignored`

use std::path::{Path, PathBuf};
use std::process::Command;

fn workspace_root() -> PathBuf {
    // CARGO_MANIFEST_DIR is crates/regseq-e2e, go up two levels.
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

fn fixture_path() -> PathBuf {
    workspace_root().join("tests/fixtures/minimal-driver")
}

/// Build the regseq-driver binary and return its path.
fn build_driver() -> PathBuf {
    let status = Command::new("cargo")
        .args(["build", "-p", "regseq-driver"])
        .current_dir(workspace_root())
        .status()
        .expect("failed to run cargo build");
    assert!(
        status.success(),
        "cargo build -p regseq-driver failed"
    );

    workspace_root().join("target/debug/regseq-driver")
}

/// Detect sysroot from rustc.
fn detect_sysroot() -> String {
    let output = Command::new("rustc")
        .args(["--print", "sysroot"])
        .output()
        .expect("failed to run rustc --print sysroot");
    assert!(output.status.success());
    String::from_utf8(output.stdout)
        .expect("sysroot not utf-8")
        .trim()
        .to_string()
}

/// Detect host triple from rustc.
fn detect_host() -> String {
    let output = Command::new("rustc")
        .arg("-vV")
        .output()
        .expect("failed to run rustc -vV");
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("not utf-8");
    stdout
        .lines()
        .find_map(|line| line.strip_prefix("host: "))
        .expect("missing host line")
        .trim()
        .to_string()
}

/// Build a minimal CompilerOptions JSON for the e2e test.
fn make_options_json(fixture: &Path) -> String {
    let platform_file = fixture.join("platform.toml");
    let output_dir = fixture.join("regseq-out");
    serde_json::json!({
        "target_os": "linux",
        "platform_file": platform_file,
        "output_dir": output_dir,
        "emit": [{"type": "driver", "match_id": null}],
        "check": false,
        "add_analysis": [],
        "skip_analysis": [],
        "deny": [],
        "allow": [],
        "params": [],
        "log_level": "warn",
        "dump_stage": null,
        "color": "auto"
    })
    .to_string()
}

#[test]
#[ignore]
fn driver_reaches_after_analysis_for_target_crate() {
    let driver = build_driver();
    assert!(
        driver.exists(),
        "regseq-driver binary not found at {}",
        driver.display()
    );

    let sysroot = detect_sysroot();
    let host = detect_host();
    let fixture = fixture_path();
    let options_json = make_options_json(&fixture);

    // Use a separate target dir so we don't interfere with the workspace build.
    let fixture_target = fixture.join("target");

    let output = Command::new("cargo")
        .arg("check")
        .arg("--target")
        .arg(&host)
        .arg("--color")
        .arg("never")
        .env("RUSTC_WRAPPER", &driver)
        .env("REGSEQ_OPTIONS", &options_json)
        .env("REGSEQ_SYSROOT", &sysroot)
        .env("CARGO_TARGET_DIR", &fixture_target)
        .current_dir(&fixture)
        .output()
        .expect("failed to run cargo check with regseq-driver");

    let stderr = String::from_utf8_lossy(&output.stderr);

    // The driver should have reached after_analysis for the target crate.
    assert!(
        stderr.contains("regseq-driver: reached after_analysis for target_os="),
        "Expected 'reached after_analysis' in stderr, got:\n{stderr}"
    );

    // The process should succeed (driver returns Compilation::Stop, which is
    // not an error).
    assert!(
        output.status.success(),
        "cargo check failed with status {}:\n{stderr}",
        output.status
    );
}
