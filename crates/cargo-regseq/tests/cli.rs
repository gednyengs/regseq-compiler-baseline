use std::path::{Path, PathBuf};

use cargo_regseq::cli::{CargoCli, Cli};
use cargo_regseq::Error;
use clap::{CommandFactory, Parser};
use regseq_config::{ColorChoice, DumpStage, EmitSpec, LogLevel, ParamScope, TargetOs};
use tempfile::NamedTempFile;

fn base_args() -> (NamedTempFile, Vec<String>) {
    let platform = NamedTempFile::new().expect("temporary platform file");
    let args = base_args_for_path(platform.path());
    (platform, args)
}

fn base_args_for_path(path: &Path) -> Vec<String> {
    vec![
        "cargo-regseq".to_string(),
        "regseq".to_string(),
        "--target-os".to_string(),
        "linux".to_string(),
        "--platform-file".to_string(),
        path.display().to_string(),
    ]
}

fn parse_ok(args: Vec<String>) -> Cli {
    let parsed = CargoCli::try_parse_from(args).expect("arguments should parse");
    let CargoCli::Regseq(cli) = parsed;
    cli
}

#[test]
fn parse_base_args_linux() {
    let (platform, args) = base_args();
    let cli = parse_ok(args);
    assert_eq!(cli.target_os, TargetOs::Linux);
    assert_eq!(cli.platform_file, platform.path());
}

#[test]
fn parse_target_os_zephyr() {
    let (_platform, mut args) = base_args();
    args[3] = "zephyr".to_string();
    let cli = parse_ok(args);
    assert_eq!(cli.target_os, TargetOs::Zephyr);
}

#[test]
fn parse_target_os_case_insensitive() {
    let (_platform, mut args) = base_args();
    args[3] = "LINUX".to_string();
    let cli = parse_ok(args);
    assert_eq!(cli.target_os, TargetOs::Linux);
}

#[test]
fn parse_output_dir_override() {
    let (_platform, mut args) = base_args();
    args.extend(["--output-dir".to_string(), "/tmp/out".to_string()]);
    let cli = parse_ok(args);
    assert_eq!(cli.output_dir, PathBuf::from("/tmp/out"));
}

#[test]
fn parse_output_dir_default() {
    let (_platform, args) = base_args();
    let cli = parse_ok(args);
    assert_eq!(cli.output_dir, PathBuf::from("./regseq-out/"));
}

#[test]
fn parse_missing_target_os_fails() {
    let (platform, _args) = base_args();
    let args = vec![
        "cargo-regseq".to_string(),
        "regseq".to_string(),
        "--platform-file".to_string(),
        platform.path().display().to_string(),
    ];
    assert!(CargoCli::try_parse_from(args).is_err());
}

#[test]
fn parse_missing_platform_file_fails() {
    let args = vec![
        "cargo-regseq".to_string(),
        "regseq".to_string(),
        "--target-os".to_string(),
        "linux".to_string(),
    ];
    assert!(CargoCli::try_parse_from(args).is_err());
}

#[test]
fn parse_cargo_args_after_double_dash() {
    let (_platform, mut args) = base_args();
    args.extend(["--".to_string(), "--release".to_string(), "-v".to_string()]);
    let cli = parse_ok(args);
    assert_eq!(cli.cargo_args, vec!["--release", "-v"]);
}

#[test]
fn emit_defaults_to_model_in_compiler_options() {
    let (_platform, args) = base_args();
    let options = parse_ok(args).into_compiler_options();
    assert_eq!(options.emit, vec![EmitSpec::Model { path: None }]);
}

#[test]
fn help_documents_model_as_default_emit() {
    let help = Cli::command().render_help().to_string();
    assert!(
        help.contains("default: model"),
        "help must document model emission as the default:\n{help}"
    );
    assert!(
        !help.contains("default: driver"),
        "help must not document driver emission as the default:\n{help}"
    );
    assert!(
        help.contains("model[=PATH]"),
        "help must document the model emit spec spelling:\n{help}"
    );
}

#[test]
fn parses_emit_json_with_and_without_path() {
    {
        let (_platform, mut args) = base_args();
        args.extend(["--emit".to_string(), "json".to_string()]);
        let cli = parse_ok(args);
        assert_eq!(cli.emit, vec![EmitSpec::Json { path: None }]);
    }
    {
        let (_platform, mut args) = base_args();
        args.extend(["--emit".to_string(), "json=foo.json".to_string()]);
        let cli = parse_ok(args);
        assert_eq!(
            cli.emit,
            vec![EmitSpec::Json {
                path: Some(PathBuf::from("foo.json"))
            }]
        );
    }
}

#[test]
fn help_lists_json_emit_spec() {
    let help = Cli::command().render_help().to_string();
    assert!(
        help.contains("json[=PATH]"),
        "help must document the json emit spec spelling:\n{help}"
    );
}

#[test]
fn parse_emit_analysis() {
    let (_platform, mut args) = base_args();
    args.extend(["--emit".to_string(), "analysis".to_string()]);
    let cli = parse_ok(args);
    assert_eq!(
        cli.emit,
        vec![EmitSpec::Analysis {
            analysis_name: None,
            path: None
        }]
    );
}

#[test]
fn parse_multiple_emit_values() {
    let (_platform, mut args) = base_args();
    args.extend([
        "--emit".to_string(),
        "driver".to_string(),
        "--emit".to_string(),
        "analysis.resources".to_string(),
    ]);
    let cli = parse_ok(args);
    assert_eq!(
        cli.emit,
        vec![
            EmitSpec::Driver { match_id: None },
            EmitSpec::Analysis {
                analysis_name: Some("resources".to_string()),
                path: None
            }
        ]
    );
}

#[test]
fn parse_emit_analysis_with_path() {
    let (_platform, mut args) = base_args();
    args.extend([
        "--emit".to_string(),
        "analysis.resources=report.json".to_string(),
    ]);
    let cli = parse_ok(args);
    assert_eq!(
        cli.emit,
        vec![EmitSpec::Analysis {
            analysis_name: Some("resources".to_string()),
            path: Some(PathBuf::from("report.json"))
        }]
    );
}

#[test]
fn parse_check_true_when_present() {
    let (_platform, mut args) = base_args();
    args.push("--check".to_string());
    let cli = parse_ok(args);
    assert!(cli.check);
}

#[test]
fn parse_check_false_by_default() {
    let (_platform, args) = base_args();
    let cli = parse_ok(args);
    assert!(!cli.check);
}

#[test]
fn parse_add_analysis_single() {
    let (_platform, mut args) = base_args();
    args.extend(["-a".to_string(), "foo.bar".to_string()]);
    let cli = parse_ok(args);
    assert_eq!(cli.add_analysis, vec!["foo.bar"]);
}

#[test]
fn parse_add_analysis_multiple() {
    let (_platform, mut args) = base_args();
    args.extend([
        "-a".to_string(),
        "foo".to_string(),
        "-a".to_string(),
        "bar.baz".to_string(),
    ]);
    let cli = parse_ok(args);
    assert_eq!(cli.add_analysis, vec!["foo", "bar.baz"]);
}

#[test]
fn parse_skip_analysis_single() {
    let (_platform, mut args) = base_args();
    args.extend(["-s".to_string(), "staging".to_string()]);
    let cli = parse_ok(args);
    assert_eq!(cli.skip_analysis, vec!["staging"]);
}

#[test]
fn validate_add_skip_conflict_fails() {
    let (_platform, mut args) = base_args();
    args.extend([
        "-a".to_string(),
        "same.name".to_string(),
        "-s".to_string(),
        "same.name".to_string(),
    ]);
    let error = parse_ok(args).validate().expect_err("must fail validation");
    assert!(matches!(error, Error::Validation(message) if message.contains("same.name")));
}

#[test]
fn parse_deny_value() {
    let (_platform, mut args) = base_args();
    args.extend(["--deny".to_string(), "shadow.infer".to_string()]);
    let cli = parse_ok(args);
    assert_eq!(cli.deny, vec!["shadow.infer"]);
}

#[test]
fn parse_allow_value() {
    let (_platform, mut args) = base_args();
    args.extend(["--allow".to_string(), "staging.warn".to_string()]);
    let cli = parse_ok(args);
    assert_eq!(cli.allow, vec!["staging.warn"]);
}

#[test]
fn validate_deny_allow_conflict_fails() {
    let (_platform, mut args) = base_args();
    args.extend([
        "--deny".to_string(),
        "dup.name".to_string(),
        "--allow".to_string(),
        "dup.name".to_string(),
    ]);
    let error = parse_ok(args).validate().expect_err("must fail validation");
    assert!(matches!(error, Error::Validation(message) if message.contains("dup.name")));
}

#[test]
fn parse_param_analysis_scope() {
    let (_platform, mut args) = base_args();
    args.extend([
        "-P".to_string(),
        "analysis.staging.enabled=true".to_string(),
    ]);
    let cli = parse_ok(args);
    let param = &cli.params[0];
    assert_eq!(param.scope, ParamScope::Analysis);
    assert_eq!(param.name, "staging.enabled");
    assert_eq!(param.value, "true");
}

#[test]
fn parse_param_general_scope_without_prefix() {
    let (_platform, mut args) = base_args();
    args.extend(["-P".to_string(), "debug=true".to_string()]);
    let cli = parse_ok(args);
    let param = &cli.params[0];
    assert_eq!(param.scope, ParamScope::General);
    assert_eq!(param.name, "debug");
    assert_eq!(param.value, "true");
}

#[test]
fn parse_param_invalid_name_fails() {
    let (_platform, mut args) = base_args();
    args.extend(["-P".to_string(), "bad-name=value".to_string()]);
    assert!(CargoCli::try_parse_from(args).is_err());
}

#[test]
fn parse_log_level_default_warn() {
    let (_platform, args) = base_args();
    let cli = parse_ok(args);
    assert_eq!(cli.log_level, LogLevel::Warn);
}

#[test]
fn parse_log_level_debug() {
    let (_platform, mut args) = base_args();
    args.extend(["--log-level".to_string(), "debug".to_string()]);
    let cli = parse_ok(args);
    assert_eq!(cli.log_level, LogLevel::Debug);
}

#[test]
fn parse_log_level_case_insensitive() {
    let (_platform, mut args) = base_args();
    args.extend(["--log-level".to_string(), "TRACE".to_string()]);
    let cli = parse_ok(args);
    assert_eq!(cli.log_level, LogLevel::Trace);
}

#[test]
fn parse_verbose_flag() {
    let (_platform, mut args) = base_args();
    args.push("-v".to_string());
    let cli = parse_ok(args);
    assert!(cli.verbose);
}

#[test]
fn resolved_log_level_verbose_only_is_debug() {
    let (_platform, mut args) = base_args();
    args.push("-v".to_string());
    let cli = parse_ok(args);
    assert_eq!(cli.resolved_log_level(), LogLevel::Debug);
}

#[test]
fn resolved_log_level_explicit_trace_wins_over_verbose() {
    let (_platform, mut args) = base_args();
    args.extend([
        "-v".to_string(),
        "--log-level".to_string(),
        "trace".to_string(),
    ]);
    let cli = parse_ok(args);
    assert_eq!(cli.resolved_log_level(), LogLevel::Trace);
}

#[test]
fn parse_dump_stage_default_none() {
    let (_platform, args) = base_args();
    let cli = parse_ok(args);
    assert_eq!(cli.dump_stage, None);
}

#[test]
fn parse_dump_stage_hir() {
    let (_platform, mut args) = base_args();
    args.extend(["--dump-stage".to_string(), "hir".to_string()]);
    let cli = parse_ok(args);
    assert_eq!(cli.dump_stage, Some(DumpStage::Hir));
}

#[test]
fn parse_dump_stage_post_analysis() {
    let (_platform, mut args) = base_args();
    args.extend(["--dump-stage".to_string(), "post-analysis".to_string()]);
    let cli = parse_ok(args);
    assert_eq!(cli.dump_stage, Some(DumpStage::PostAnalysis));
}

#[test]
fn parse_color_default_auto() {
    let (_platform, args) = base_args();
    let cli = parse_ok(args);
    assert_eq!(cli.color, ColorChoice::Auto);
}

#[test]
fn parse_color_always() {
    let (_platform, mut args) = base_args();
    args.extend(["--color".to_string(), "always".to_string()]);
    let cli = parse_ok(args);
    assert_eq!(cli.color, ColorChoice::Always);
}

#[test]
fn parse_color_never() {
    let (_platform, mut args) = base_args();
    args.extend(["--color".to_string(), "never".to_string()]);
    let cli = parse_ok(args);
    assert_eq!(cli.color, ColorChoice::Never);
}

#[test]
fn validate_add_analysis_requires_dotted_ident() {
    let (_platform, mut args) = base_args();
    args.extend(["-a".to_string(), "123bad".to_string()]);
    let error = parse_ok(args).validate().expect_err("must fail validation");
    assert!(matches!(error, Error::Validation(message) if message.contains("123bad")));
}

#[test]
fn validate_deny_requires_dotted_ident() {
    let (_platform, mut args) = base_args();
    args.extend(["--deny".to_string(), "bad-name".to_string()]);
    let error = parse_ok(args).validate().expect_err("must fail validation");
    assert!(matches!(error, Error::Validation(message) if message.contains("bad-name")));
}

#[test]
fn into_compiler_options_base_defaults() {
    let (_platform, args) = base_args();
    let options = parse_ok(args).into_compiler_options();
    assert_eq!(options.emit, vec![EmitSpec::Model { path: None }]);
    assert_eq!(options.log_level, LogLevel::Warn);
}

#[test]
fn into_compiler_options_verbose_sets_debug() {
    let (_platform, mut args) = base_args();
    args.push("-v".to_string());
    let options = parse_ok(args).into_compiler_options();
    assert_eq!(options.log_level, LogLevel::Debug);
}

#[test]
fn into_compiler_options_deduplicates_consecutive_emit_values() {
    let (_platform, mut args) = base_args();
    args.extend([
        "--emit".to_string(),
        "driver".to_string(),
        "--emit".to_string(),
        "driver".to_string(),
        "--emit".to_string(),
        "analysis".to_string(),
    ]);
    let options = parse_ok(args).into_compiler_options();
    assert_eq!(
        options.emit,
        vec![
            EmitSpec::Driver { match_id: None },
            EmitSpec::Analysis {
                analysis_name: None,
                path: None
            }
        ]
    );
}

#[test]
fn validate_requires_existing_platform_file() {
    let missing = std::env::temp_dir().join("regseq-cargo-cli-missing.toml");
    let args = base_args_for_path(&missing);
    let error = parse_ok(args).validate().expect_err("must fail validation");
    assert!(matches!(error, Error::Validation(message) if message.contains("platform file")));
}

#[test]
fn validate_output_dir_rejects_regular_file() {
    let (_platform, mut args) = base_args();
    let out_file = NamedTempFile::new().expect("temporary output file");
    args.extend([
        "--output-dir".to_string(),
        out_file.path().display().to_string(),
    ]);
    let error = parse_ok(args).validate().expect_err("must fail validation");
    assert!(matches!(error, Error::Validation(message) if message.contains("regular file")));
}
