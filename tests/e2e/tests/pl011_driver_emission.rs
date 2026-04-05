//! End-to-end test: PL011 full-spec through the production driver emission path.
//!
//! Builds `regseq-driver`, then runs `cargo check` with `RUSTC_WRAPPER`
//! pointing at it against `tests/fixtures/pl011-full-spec`. Verifies the
//! `EmitSpec::Driver` path produces real Linux artifacts on disk.
//!
//! On-disk artifacts are the primary proof. Stderr is supporting evidence.
//!
//! These tests are slow (full cargo build) so they are gated behind `#[ignore]`.
//! Run with: `cargo test -p regseq-e2e --test pl011_driver_emission -- --ignored`

use std::path::{Path, PathBuf};
use std::process::Command;

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

fn fixture_path() -> PathBuf {
    workspace_root().join("tests/fixtures/pl011-full-spec")
}

fn output_dir() -> PathBuf {
    fixture_path().join("regseq-out")
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

/// Build a CompilerOptions JSON requesting driver emission for pl011-full-spec.
fn make_driver_options_json() -> String {
    let fixture = fixture_path();
    let platform_file = fixture.join("platform.toml");
    let out = output_dir();
    serde_json::json!({
        "target_os": "linux",
        "platform_file": platform_file,
        "output_dir": out,
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

/// Run the pl011-full-spec fixture through the production driver emission path.
/// Returns (stderr, output_dir) for assertions.
fn run_driver_emission() -> (String, PathBuf) {
    let driver = build_driver();
    assert!(
        driver.exists(),
        "regseq-driver binary not found at {}",
        driver.display()
    );

    let sysroot = detect_sysroot();
    let host = detect_host();
    let fixture = fixture_path();
    let out = output_dir();

    // Clean output directory to ensure fresh emission.
    if out.exists() {
        std::fs::remove_dir_all(&out).expect("failed to clean output dir");
    }

    let fixture_target = fixture.join("target");
    let options_json = make_driver_options_json();

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

    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    assert!(
        output.status.success(),
        "cargo check failed with status {}:\n{stderr}",
        output.status
    );

    (stderr, out)
}

// ---------------------------------------------------------------------------
// Test 1: production path produces artifacts on disk
// ---------------------------------------------------------------------------

#[test]
#[ignore]
fn pl011_full_spec_driver_emission_produces_artifacts() {
    let (_stderr, out) = run_driver_emission();

    assert!(
        out.exists(),
        "output directory should exist: {}",
        out.display()
    );

    // At least one .c file and one .h file should exist.
    let c_files: Vec<_> = std::fs::read_dir(&out)
        .expect("read output dir")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "c"))
        .collect();
    assert!(
        !c_files.is_empty(),
        "expected at least one .c file in {}",
        out.display()
    );

    let h_files: Vec<_> = std::fs::read_dir(&out)
        .expect("read output dir")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "h"))
        .collect();
    assert!(
        !h_files.is_empty(),
        "expected at least one .h file in {}",
        out.display()
    );
}

// ---------------------------------------------------------------------------
// Test 2: expected file inventory
// ---------------------------------------------------------------------------

#[test]
#[ignore]
fn pl011_full_spec_emitted_file_inventory() {
    let (_stderr, out) = run_driver_emission();

    // Filenames are derived from the driver name "pl011-full-spec" by the
    // Linux backend planner's snake-case and prefix policies.
    let expected_files = [
        "regseq_pl011-full-spec_module.c",
        "regseq_pl011-full-spec_shared.h",
        "pl011-full-spec_regs.h",
        "Makefile",
        "Kbuild",
    ];

    for filename in &expected_files {
        let path = out.join(filename);
        assert!(path.exists(), "expected file not found: {}", path.display());
        let metadata = std::fs::metadata(&path)
            .unwrap_or_else(|e| panic!("failed to stat {}: {e}", path.display()));
        assert!(
            metadata.len() > 0,
            "expected nonempty file: {}",
            path.display()
        );
    }
}

// ---------------------------------------------------------------------------
// Test 3: output is under the configured root
// ---------------------------------------------------------------------------

#[test]
#[ignore]
fn pl011_full_spec_output_under_configured_root() {
    let (_stderr, out) = run_driver_emission();

    let entries: Vec<_> = std::fs::read_dir(&out)
        .expect("read output dir")
        .filter_map(|e| e.ok())
        .collect();

    assert!(!entries.is_empty(), "output directory should contain files");

    // Every emitted file should be directly under the output root.
    for entry in &entries {
        assert!(
            entry.path().starts_with(&out),
            "artifact {} not under output root {}",
            entry.path().display(),
            out.display()
        );
    }
}

// ---------------------------------------------------------------------------
// Test 4: emitted module.c contains expected structural markers
// ---------------------------------------------------------------------------

#[test]
#[ignore]
fn pl011_full_spec_module_c_contains_expected_structures() {
    let (_stderr, out) = run_driver_emission();

    let module_c = out.join("regseq_pl011-full-spec_module.c");
    let content = std::fs::read_to_string(&module_c)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", module_c.display()));

    // The generated module file should contain key Linux kernel structures.
    assert!(
        content.contains("#include"),
        "module.c should contain #include directives"
    );
    assert!(
        content.contains("MODULE_LICENSE"),
        "module.c should contain MODULE_LICENSE"
    );
    // AMBA driver registration — pl011 is an AMBA device.
    assert!(
        content.contains("module_amba_driver"),
        "module.c should contain module_amba_driver registration"
    );
}

// ---------------------------------------------------------------------------
// Test 5: wrapper path produces output (file count check)
// ---------------------------------------------------------------------------

#[test]
#[ignore]
fn pl011_full_spec_wrapper_path_produces_output() {
    let (_stderr, out) = run_driver_emission();

    let file_count = std::fs::read_dir(&out)
        .expect("read output dir")
        .filter_map(|e| e.ok())
        .count();

    // The Linux backend should emit 5 files for a serial UART driver:
    // module.c, shared.h, regs.h, Makefile, Kbuild
    assert_eq!(file_count, 5, "expected 5 emitted files, got {file_count}");
}
