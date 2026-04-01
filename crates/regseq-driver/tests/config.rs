#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::PathBuf;

use regseq_config::{
    ColorChoice, CompilerOptions, EmitSpec, LogLevel, ParamScope, RegseqParam, TargetOs,
};
use regseq_driver::RegseqCallbacks;

fn sample_options() -> CompilerOptions {
    CompilerOptions {
        target_os: TargetOs::Linux,
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
        "target_os":"linux",
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
    let invalid = r#"{"target_os":"linux","platform_file":}"#;
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

#[test]
fn raw_artifact_emit_spec_round_trips_through_json() {
    let mut options = sample_options();
    options.emit.push(EmitSpec::RawArtifact);
    let json = serde_json::to_string(&options).expect("serialize");
    let decoded: CompilerOptions = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(decoded, options);
    assert!(decoded
        .emit
        .iter()
        .any(|e| matches!(e, EmitSpec::RawArtifact)));
}

#[test]
fn deserialize_raw_artifact_emit_spec_from_json() {
    let json = r#"{"type":"raw-artifact"}"#;
    let spec: EmitSpec = serde_json::from_str(json).expect("deserialize");
    assert_eq!(spec, EmitSpec::RawArtifact);
}
