//! End-to-end test: PL011 spec-slice fixture through the Regseq compiler path.
//!
//! Builds `regseq-driver`, then runs `cargo check` with `RUSTC_WRAPPER`
//! pointing at it against `tests/fixtures/pl011-spec-slice`. Verifies the
//! driver reaches `after_analysis` for the target crate.
//!
//! Also verifies the fixture's checked-in `platform.toml` parses under the
//! current B-07 parser contract.
//!
//! This test is slow (full cargo build) so it is gated behind `#[ignore]`.
//! Run with: `cargo test -p regseq-e2e --test pl011_spec_slice -- --ignored`

use std::path::{Path, PathBuf};
use std::process::Command;

use regseq_model::platform_parser::parse_platform_file_path;

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

fn fixture_path() -> PathBuf {
    workspace_root().join("tests/fixtures/pl011-spec-slice")
}

/// Build the regseq-driver binary and return its path.
fn build_driver() -> PathBuf {
    let status = Command::new("cargo")
        .args(["build", "-p", "regseq-driver"])
        .current_dir(workspace_root())
        .status()
        .expect("failed to run cargo build");
    assert!(status.success(), "cargo build -p regseq-driver failed");

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

/// Build a CompilerOptions JSON for the pl011-spec-slice fixture.
fn make_options_json(fixture: &Path) -> String {
    make_options_json_with_emit(
        fixture,
        serde_json::json!([{"type": "driver", "match_id": null}]),
    )
}

/// Build a CompilerOptions JSON that also requests the raw-artifact emit kind.
fn make_options_json_with_raw_artifact(fixture: &Path) -> String {
    make_options_json_with_emit(
        fixture,
        serde_json::json!([
            {"type": "driver", "match_id": null},
            {"type": "raw-artifact"}
        ]),
    )
}

fn make_options_json_with_emit(fixture: &Path, emit: serde_json::Value) -> String {
    let platform_file = fixture.join("platform.toml");
    let output_dir = fixture.join("regseq-out");
    serde_json::json!({
        "target_os": "linux",
        "platform_file": platform_file,
        "output_dir": output_dir,
        "emit": emit,
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

/// Run the fixture through regseq-driver as RUSTC_WRAPPER and return stderr.
fn run_regseq_driver() -> String {
    run_regseq_driver_with_options(&make_options_json(&fixture_path()))
}

/// Run the fixture with the raw-artifact emit kind and return stderr.
fn run_regseq_driver_with_raw_artifact() -> String {
    run_regseq_driver_with_options(&make_options_json_with_raw_artifact(&fixture_path()))
}

fn run_regseq_driver_with_options(options_json: &str) -> String {
    let driver = build_driver();
    assert!(
        driver.exists(),
        "regseq-driver binary not found at {}",
        driver.display()
    );

    let sysroot = detect_sysroot();
    let host = detect_host();
    let fixture = fixture_path();

    let fixture_target = fixture.join("target");

    let output = Command::new("cargo")
        .arg("check")
        .arg("--target")
        .arg(&host)
        .arg("--color")
        .arg("never")
        .env("RUSTC_WRAPPER", &driver)
        .env("REGSEQ_OPTIONS", options_json)
        .env("REGSEQ_SYSROOT", &sysroot)
        .env("CARGO_TARGET_DIR", &fixture_target)
        .current_dir(&fixture)
        .output()
        .expect("failed to run cargo check with regseq-driver");

    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    assert!(
        output.status.success(),
        "cargo check failed with status {}:\n{stderr}",
        output.status
    );

    stderr
}

#[test]
#[ignore]
fn pl011_fixture_compiles_through_regseq_driver() {
    let stderr = run_regseq_driver();

    assert!(
        stderr.contains("regseq-driver: reached after_analysis for target_os="),
        "Expected 'reached after_analysis' in stderr, got:\n{stderr}"
    );
}

#[test]
#[ignore]
fn pl011_fixture_register_block_is_extracted() {
    let stderr = run_regseq_driver();

    assert!(
        stderr.contains("Pl011Registers"),
        "Expected register block name 'Pl011Registers' in extraction output, got:\n{stderr}"
    );
}

#[test]
#[ignore]
fn pl011_fixture_mmio_field_is_extracted() {
    let stderr = run_regseq_driver();

    assert!(
        stderr.contains("regs") && stderr.contains("Pl011Registers"),
        "Expected MMIO field 'regs' with 'Pl011Registers' in extraction output, got:\n{stderr}"
    );

    assert!(
        stderr.contains("uart_regs"),
        "Expected MMIO id 'uart_regs' in extraction output, got:\n{stderr}"
    );
}

#[test]
#[ignore]
fn pl011_fixture_register_descriptors_are_extracted() {
    let stderr = run_regseq_driver();

    assert!(
        stderr.contains("DR_REG"),
        "Expected register descriptor 'DR_REG' in extraction output, got:\n{stderr}"
    );
    assert!(
        stderr.contains("FR_REG"),
        "Expected register descriptor 'FR_REG' in extraction output, got:\n{stderr}"
    );
    assert!(
        stderr.contains("LCRH_REG"),
        "Expected register descriptor 'LCRH_REG' in extraction output, got:\n{stderr}"
    );
}

#[test]
#[ignore]
fn pl011_fixture_driver_identity_is_extracted() {
    let stderr = run_regseq_driver();

    assert!(
        stderr.contains("pl011-spec-slice"),
        "Expected driver name 'pl011-spec-slice' in extraction output, got:\n{stderr}"
    );
}

#[test]
#[ignore]
fn pl011_fixture_wrapper_call_sites_from_callbacks() {
    let stderr = run_regseq_driver();

    // Wrapper call sites are tracked by the function that directly contains
    // the register read/write. The trait callbacks delegate to helpers.
    for fn_name in &[
        "disable_uart",
        "enable_uart",
        "set_baud_rate",
        "configure_line_control",
        "tx_byte",
        "rx_char",
        "tx_ready",
        "tx_empty",
        "flush_rx_fifo",
        "flush_tx_fifo",
        "send_break",
    ] {
        assert!(
            stderr.contains(&format!("containing_fn: \"{fn_name}\"")),
            "Expected wrapper call-site evidence from '{fn_name}', got:\n{stderr}"
        );
    }
}

#[test]
fn pl011_fixture_platform_toml_parses() {
    let path = fixture_path().join("platform.toml");
    let file =
        parse_platform_file_path(&path).expect("pl011-spec-slice platform.toml should parse");

    assert_eq!(file.version, "1");
    assert_eq!(file.platform.os, "linux");
    assert_eq!(file.platform.board, "raspberry-pi-4");
    assert_eq!(file.platform.arch, "aarch64");
    assert_eq!(file.properties.len(), 2);

    let keys: Vec<&str> = file.properties.iter().map(|p| p.key.as_str()).collect();
    assert!(keys.contains(&"reg-io-width"));
    assert!(keys.contains(&"current-speed"));
}

#[test]
fn pl011_fixture_has_isolated_workspace() {
    let cargo_toml = std::fs::read_to_string(fixture_path().join("Cargo.toml"))
        .expect("pl011-spec-slice Cargo.toml should exist");
    assert!(
        cargo_toml.contains("[workspace]"),
        "fixture Cargo.toml must include an empty [workspace] table"
    );
}

#[test]
#[ignore]
fn pl011_fixture_emits_raw_frontend_artifact() {
    let stderr = run_regseq_driver_with_raw_artifact();

    assert!(
        stderr.contains("regseq-driver: raw artifact emitted to"),
        "Expected raw artifact emission message in stderr, got:\n{stderr}"
    );

    let artifact_path = fixture_path().join("regseq-out/drivers/pl011-spec-slice.raw.json");
    assert!(
        artifact_path.exists(),
        "Raw artifact file not found at {}",
        artifact_path.display()
    );
}

#[test]
#[ignore]
fn pl011_fixture_raw_artifact_matches_checked_in_snapshot() {
    let _stderr = run_regseq_driver_with_raw_artifact();

    let artifact_path = fixture_path().join("regseq-out/drivers/pl011-spec-slice.raw.json");
    let expected_path = fixture_path().join("expected/raw-frontend.json");

    let emitted: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(&artifact_path)
            .unwrap_or_else(|e| panic!("failed to read emitted artifact: {e}")),
    )
    .expect("emitted artifact is valid JSON");

    let expected: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(&expected_path)
            .unwrap_or_else(|e| panic!("failed to read expected snapshot: {e}")),
    )
    .expect("expected snapshot is valid JSON");

    assert_eq!(
        emitted, expected,
        "Emitted raw artifact does not match checked-in snapshot.\n\
         Re-run the compiler on the PL011 fixture and update \
         tests/fixtures/pl011-spec-slice/expected/raw-frontend.json if the \
         change is intentional."
    );
}
