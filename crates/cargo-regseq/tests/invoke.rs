use std::ffi::OsStr;
use std::path::PathBuf;
use std::process::Command;

use cargo_regseq::cli::CargoCli;
use cargo_regseq::invoke::{build_cargo_command, invoke_cargo};
use cargo_regseq::toolchain::ToolchainInfo;
use clap::Parser;
use regseq_config::{
    ColorChoice, CompilerOptions, EmitSpec, LogLevel, ParamScope, RegseqParam, TargetOs,
};
use tempfile::{NamedTempFile, TempDir};

fn sample_options_and_toolchain(
    check: bool,
) -> (
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
        output_dir: output_dir.clone(),
        emit: vec![EmitSpec::Driver { match_id: None }],
        check,
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
    };

    (options, toolchain, platform_file, driver_file, root)
}

fn command_args(command: &Command) -> Vec<String> {
    command
        .get_args()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect()
}

fn env_value(command: &Command, key: &str) -> Option<String> {
    command.get_envs().find_map(|(env_key, env_value)| {
        if env_key == OsStr::new(key) {
            env_value.map(|value| value.to_string_lossy().into_owned())
        } else {
            None
        }
    })
}

fn regseq_options_from_env(command: &Command) -> CompilerOptions {
    let json = env_value(command, "REGSEQ_OPTIONS").expect("REGSEQ_OPTIONS must be set");
    serde_json::from_str(&json).expect("REGSEQ_OPTIONS should deserialize")
}

#[test]
fn build_command_sets_cargo_check_and_wrapper_env() {
    let (options, toolchain, _platform_file, _driver_file, _root) =
        sample_options_and_toolchain(false);
    let command = build_cargo_command(&options, &toolchain, &[], ColorChoice::Auto).unwrap();

    assert_eq!(command.get_program(), OsStr::new("cargo"));
    assert_eq!(command_args(&command), vec!["check", "--color", "auto"],);
    assert_eq!(
        env_value(&command, "RUSTC_WRAPPER"),
        Some(toolchain.driver.to_string_lossy().into_owned())
    );
    assert_eq!(
        env_value(&command, "REGSEQ_SYSROOT"),
        Some(toolchain.sysroot.to_string_lossy().into_owned())
    );
}

#[test]
fn build_command_sets_regseq_options_json() {
    let (options, toolchain, _platform_file, _driver_file, _root) =
        sample_options_and_toolchain(false);
    let command = build_cargo_command(&options, &toolchain, &[], ColorChoice::Auto).unwrap();
    assert_eq!(regseq_options_from_env(&command), options);
}

#[test]
fn build_command_preserves_check_flag_in_options_json() {
    let (options, toolchain, _platform_file, _driver_file, _root) =
        sample_options_and_toolchain(true);
    let command = build_cargo_command(&options, &toolchain, &[], ColorChoice::Auto).unwrap();
    let decoded = regseq_options_from_env(&command);
    assert!(decoded.check);
}

#[test]
fn build_command_preserves_emit_specs_in_options_json() {
    let (mut options, toolchain, _platform_file, _driver_file, _root) =
        sample_options_and_toolchain(false);
    options.emit = vec![
        EmitSpec::Driver {
            match_id: Some("pl011".to_string()),
        },
        EmitSpec::Analysis {
            analysis_name: Some("resources".to_string()),
            path: Some(PathBuf::from("report.json")),
        },
    ];

    let command = build_cargo_command(&options, &toolchain, &[], ColorChoice::Auto).unwrap();
    let decoded = regseq_options_from_env(&command);
    assert_eq!(decoded.emit, options.emit);
}

#[test]
fn build_command_preserves_params_in_options_json() {
    let (mut options, toolchain, _platform_file, _driver_file, _root) =
        sample_options_and_toolchain(false);
    options.params = vec![
        RegseqParam {
            scope: ParamScope::Analysis,
            name: "staging.enabled".to_string(),
            value: "true".to_string(),
        },
        RegseqParam {
            scope: ParamScope::General,
            name: "debug".to_string(),
            value: "false".to_string(),
        },
    ];

    let command = build_cargo_command(&options, &toolchain, &[], ColorChoice::Auto).unwrap();
    let decoded = regseq_options_from_env(&command);
    assert_eq!(decoded.params, options.params);
}

#[test]
fn build_command_adds_color_when_not_provided_by_user() {
    let (options, toolchain, _platform_file, _driver_file, _root) =
        sample_options_and_toolchain(false);
    let command = build_cargo_command(&options, &toolchain, &[], ColorChoice::Always).unwrap();
    let args = command_args(&command);
    assert_eq!(args, vec!["check", "--color", "always"]);
}

#[test]
fn build_command_does_not_add_color_when_user_passed_color() {
    let (options, toolchain, _platform_file, _driver_file, _root) =
        sample_options_and_toolchain(false);
    let cargo_args = vec!["--color".to_string(), "never".to_string()];
    let command =
        build_cargo_command(&options, &toolchain, &cargo_args, ColorChoice::Always).unwrap();
    let args = command_args(&command);
    assert_eq!(args, vec!["check", "--color", "never"]);
}

#[test]
fn build_command_forwards_extra_cargo_args() {
    let (options, toolchain, _platform_file, _driver_file, _root) =
        sample_options_and_toolchain(false);
    let cargo_args = vec!["--release".to_string(), "-v".to_string()];
    let command =
        build_cargo_command(&options, &toolchain, &cargo_args, ColorChoice::Always).unwrap();
    let args = command_args(&command);
    assert_eq!(args, vec!["check", "--color", "always", "--release", "-v"]);
}

#[test]
fn invoke_cargo_creates_output_directory() {
    let (options, toolchain, _platform_file, _driver_file, _root) =
        sample_options_and_toolchain(false);
    assert!(!options.output_dir.exists());

    let result = invoke_cargo(
        &options,
        &toolchain,
        &["--help".to_string()],
        ColorChoice::Auto,
    );
    assert!(result.is_ok());
    assert!(options.output_dir.is_dir());
}

#[test]
fn compiler_options_platform_file_is_absolute_after_cli_conversion() {
    let platform_file = NamedTempFile::new().expect("temporary platform file");
    let args = vec![
        "cargo-regseq".to_string(),
        "regseq".to_string(),
        "--target-os".to_string(),
        "linux".to_string(),
        "--platform-file".to_string(),
        platform_file.path().to_string_lossy().into_owned(),
    ];
    let parsed = CargoCli::try_parse_from(args).expect("arguments should parse");
    let CargoCli::Regseq(cli) = parsed;

    let options = cli.into_compiler_options();
    assert!(options.platform_file.is_absolute());
}
