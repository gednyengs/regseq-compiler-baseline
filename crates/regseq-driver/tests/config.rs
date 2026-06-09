#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::PathBuf;

use regseq_config::{
    ColorChoice, CompilerOptions, EmitSpec, LogLevel, ParamScope, RegseqParam,
};
use regseq_driver::{regseq_mir_passes, RegseqCallbacks};

fn sample_options() -> CompilerOptions {
    CompilerOptions {
        platform_file: PathBuf::from("platforms/test.toml"),
        output_dir: PathBuf::from("out"),
        emit: vec![
            EmitSpec::Driver { match_id: None },
            EmitSpec::Analysis {
                analysis_name: Some("resources".to_string()),
                path: Some(PathBuf::from("reports/resources.json")),
            },
        ],
        check: false,
        add_analysis: vec!["resources".to_string()],
        skip_analysis: vec!["lint.foo".to_string()],
        deny: vec!["diag.a".to_string()],
        allow: vec!["diag.b".to_string()],
        params: vec![RegseqParam {
            scope: ParamScope::Analysis,
            name: "resource_limit".to_string(),
            value: "8".to_string(),
        }],
        log_level: LogLevel::Warn,
        dump_stage: None,
        color: ColorChoice::Auto,
    }
}

#[test]
fn deserialize_valid_regseq_options_json() {
    let json = r#"{
        "platform_file":"platforms/test.toml",
        "output_dir":"out",
        "emit":[
            {"type":"driver","match_id":null},
            {"type":"analysis","analysis_name":"resources","path":"reports/resources.json"}
        ],
        "check":false,
        "add_analysis":["resources"],
        "skip_analysis":["lint.foo"],
        "deny":["diag.a"],
        "allow":["diag.b"],
        "params":[{"scope":"analysis","name":"resource_limit","value":"8"}],
        "log_level":"warn",
        "dump_stage":null,
        "color":"auto"
    }"#;

    let parsed: CompilerOptions = serde_json::from_str(json).expect("JSON should deserialize");
    assert_eq!(parsed, sample_options());
}

#[test]
fn deserialize_invalid_regseq_options_json_returns_error() {
    let invalid = r#"{"platform_file":}"#;
    assert!(serde_json::from_str::<CompilerOptions>(invalid).is_err());
}

#[test]
fn regseq_callbacks_new_stores_options() {
    let options = sample_options();
    let callbacks = RegseqCallbacks::new(options.clone());
    assert_eq!(callbacks.options(), &options);
}

#[test]
fn compiler_options_round_trip_through_json() {
    let options = sample_options();
    let json = serde_json::to_string(&options).expect("serialize options");
    let decoded: CompilerOptions = serde_json::from_str(&json).expect("deserialize options");
    assert_eq!(decoded, options);
}

/// The whitelist returned by `regseq_mir_passes` is the canonical
/// clean-MIR shape shared by spec and regseq-aware crate compilation.
#[test]
fn regseq_mir_passes_whitelist_matches_contract() {
    let passes = regseq_mir_passes();
    let expected: Vec<(String, bool)> = vec![
        ("RemoveStorageMarkers".to_string(), true),
        ("RemoveZsts".to_string(), true),
        ("SimplifyLocals-final".to_string(), true),
        ("ReorderBasicBlocks".to_string(), true),
        ("ReorderLocals".to_string(), true),
        ("CheckAlignment".to_string(), false),
        ("CheckNull".to_string(), false),
        ("CheckEnums".to_string(), false),
    ];
    assert_eq!(passes, expected);
}

/// Regression guard: the `RegseqCallbacks::config` source must not
/// inject `feature(register_tool)` or `register_tool(rseq)`. Driver
/// spec fixtures declare these at their crate roots so the same
/// source compiles uniformly under the wrapper, rust-analyzer, and
/// any other non-wrapper compilation path. End-to-end tests
/// (`model_emit`, `pl011_construction`) compile fixtures that
/// self-declare `register_tool(rseq)`; if `RegseqCallbacks::config`
/// regressed and re-injected the same registration, those compiles
/// would fail with a duplicate-registration error. This text-level
/// check is the lightweight first line of defense.
#[test]
fn regseq_callbacks_config_source_does_not_inject_register_tool() {
    let source = include_str!("../src/callbacks.rs");

    // Locate the RegseqCallbacks::config function body.
    let config_start = source
        .find("impl rustc_driver::Callbacks for RegseqCallbacks")
        .expect("RegseqCallbacks impl not found in callbacks.rs");
    let after_impl = &source[config_start..];
    let fn_start = after_impl
        .find("fn config")
        .expect("RegseqCallbacks::config not found in callbacks.rs");
    let body_start = after_impl[fn_start..]
        .find('{')
        .expect("config fn body open brace not found")
        + fn_start;

    // Find matching closing brace for the function body.
    let bytes = after_impl.as_bytes();
    let mut depth = 0usize;
    let mut body_end = body_start;
    for i in body_start..bytes.len() {
        match bytes[i] {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    body_end = i + 1;
                    break;
                }
            }
            _ => {}
        }
    }
    let body = &after_impl[body_start..body_end];
    let executable_body = body
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");

    assert!(
        !executable_body.contains("register_tool"),
        "RegseqCallbacks::config must not reference register_tool; \
         found in body:\n{body}",
    );
    assert!(
        !executable_body.contains("feature(register_tool)"),
        "RegseqCallbacks::config must not inject feature(register_tool); \
         found in body:\n{body}",
    );
}
