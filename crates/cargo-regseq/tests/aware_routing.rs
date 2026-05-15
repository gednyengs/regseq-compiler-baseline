//! Integration test for the `REGSEQ_AWARE_CRATES` env injection in
//! `build_cargo_command`. The cargo-regseq CLI runs `cargo metadata`
//! against the workspace it is invoked from; this test exercises that
//! against the real workspace under the repo root, which currently
//! contains exactly one regseq-aware crate (`regseq`, the facade).
//!
//! If new regseq-aware crates are added later, update the assertion to
//! reflect the new set.

use std::ffi::OsStr;
use std::path::PathBuf;
use std::process::Command;

use cargo_regseq::invoke::build_cargo_command;
use cargo_regseq::toolchain::ToolchainInfo;
use regseq_config::{ColorChoice, CompilerOptions, EmitSpec, LogLevel, TargetOs};
use tempfile::{NamedTempFile, TempDir};

fn env_value(command: &Command, key: &str) -> Option<String> {
    command.get_envs().find_map(|(env_key, env_value)| {
        if env_key == OsStr::new(key) {
            env_value.map(|value| value.to_string_lossy().into_owned())
        } else {
            None
        }
    })
}

fn sample_options_and_toolchain() -> (
    CompilerOptions,
    ToolchainInfo,
    NamedTempFile,
    NamedTempFile,
    TempDir,
) {
    let platform_file = NamedTempFile::new().expect("temporary platform file");
    let driver_file = NamedTempFile::new().expect("temporary driver file");
    let root = TempDir::new().expect("temporary root");
    let output_dir = root.path().join("regseq-out");

    let options = CompilerOptions {
        target_os: TargetOs::Linux,
        platform_file: platform_file.path().to_path_buf(),
        output_dir,
        emit: vec![EmitSpec::Driver { match_id: None }],
        check: false,
        add_analysis: vec![],
        skip_analysis: vec![],
        deny: vec![],
        allow: vec![],
        params: vec![],
        log_level: LogLevel::Warn,
        dump_stage: None,
        color: ColorChoice::Auto,
    };

    let toolchain = ToolchainInfo {
        rustc: PathBuf::from("rustc"),
        sysroot: root.path().to_path_buf(),
        driver: driver_file.path().to_path_buf(),
        host: "x86_64-unknown-linux-gnu".to_string(),
    };

    (options, toolchain, platform_file, driver_file, root)
}

#[test]
fn build_command_injects_regseq_aware_crates_env() {
    let (options, toolchain, _platform_file, _driver_file, _root) = sample_options_and_toolchain();
    let command = build_cargo_command(&options, &toolchain, &[], ColorChoice::Auto)
        .expect("build_cargo_command should succeed against the workspace");

    let aware = env_value(&command, "REGSEQ_AWARE_CRATES")
        .expect("REGSEQ_AWARE_CRATES must be set on the cargo command");

    let names: Vec<&str> = aware.split(',').filter(|s| !s.is_empty()).collect();
    assert!(
        names.contains(&"regseq"),
        "expected the facade crate `regseq` in REGSEQ_AWARE_CRATES, got {aware:?}"
    );
    // Currently only the facade declares `[package.metadata.regseq]`.
    // If you add another regseq-aware crate, update this assertion.
    assert_eq!(
        names,
        vec!["regseq"],
        "expected only `regseq` to be regseq-aware in this workspace, got {aware:?}"
    );
}
