use std::str::FromStr;

use regseq_config::{ColorChoice, DumpStage, LogLevel, TargetOs};

#[test]
fn target_os_parses_linux() {
    assert_eq!(TargetOs::from_str("linux").unwrap(), TargetOs::Linux);
}

#[test]
fn target_os_parses_zephyr_case_insensitive() {
    assert_eq!(TargetOs::from_str("ZEPHYR").unwrap(), TargetOs::Zephyr);
}

#[test]
fn target_os_parse_invalid_fails() {
    assert!(TargetOs::from_str("invalid").is_err());
}

#[test]
fn target_os_display_linux() {
    assert_eq!(TargetOs::Linux.to_string(), "linux");
}

#[test]
fn log_level_parses_debug() {
    assert_eq!(LogLevel::from_str("debug").unwrap(), LogLevel::Debug);
}

#[test]
fn log_level_parses_trace_case_insensitive() {
    assert_eq!(LogLevel::from_str("TRACE").unwrap(), LogLevel::Trace);
}

#[test]
fn log_level_parse_invalid_fails() {
    assert!(LogLevel::from_str("invalid").is_err());
}

#[test]
fn log_level_default_is_warn() {
    assert_eq!(LogLevel::default(), LogLevel::Warn);
}

#[test]
fn dump_stage_parses_hir() {
    assert_eq!(DumpStage::from_str("hir").unwrap(), DumpStage::Hir);
}

#[test]
fn dump_stage_parses_post_analysis() {
    assert_eq!(
        DumpStage::from_str("post-analysis").unwrap(),
        DumpStage::PostAnalysis
    );
}

#[test]
fn dump_stage_parses_pre_codegen() {
    assert_eq!(
        DumpStage::from_str("pre-codegen").unwrap(),
        DumpStage::PreCodegen
    );
}

#[test]
fn dump_stage_parse_invalid_fails() {
    assert!(DumpStage::from_str("invalid").is_err());
}

#[test]
fn color_choice_parses_always() {
    assert_eq!(
        ColorChoice::from_str("always").unwrap(),
        ColorChoice::Always
    );
}

#[test]
fn color_choice_parses_auto_case_insensitive() {
    assert_eq!(ColorChoice::from_str("AUTO").unwrap(), ColorChoice::Auto);
}

#[test]
fn color_choice_parse_invalid_fails() {
    assert!(ColorChoice::from_str("invalid").is_err());
}

#[test]
fn color_choice_default_is_auto() {
    assert_eq!(ColorChoice::default(), ColorChoice::Auto);
}
