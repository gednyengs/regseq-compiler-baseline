use std::path::PathBuf;

use regseq_config::{
    ColorChoice, CompilerOptions, DumpStage, EmitSpec, LogLevel, ParamScope, RegseqParam,
};

// `serde_round_trip_target_os_linux` was removed with the `TargetOs` relocation
// to `regseq-model` (R5/D6/D8); serde round-trips of the typed identity are
// covered by `regseq-l2::resolve_platform_identity` tests.

#[test]
fn serde_round_trip_dump_stage_post_analysis() {
    let encoded = serde_json::to_string(&DumpStage::PostAnalysis).unwrap();
    assert_eq!(encoded, "\"post-analysis\"");
    let decoded: DumpStage = serde_json::from_str(&encoded).unwrap();
    assert_eq!(decoded, DumpStage::PostAnalysis);
}

#[test]
fn serde_round_trip_color_choice_auto() {
    let encoded = serde_json::to_string(&ColorChoice::Auto).unwrap();
    assert_eq!(encoded, "\"auto\"");
    let decoded: ColorChoice = serde_json::from_str(&encoded).unwrap();
    assert_eq!(decoded, ColorChoice::Auto);
}

#[test]
fn serde_round_trip_emit_spec_driver() {
    let value = EmitSpec::Driver { match_id: None };
    let encoded = serde_json::to_string(&value).unwrap();
    let decoded: EmitSpec = serde_json::from_str(&encoded).unwrap();
    assert_eq!(decoded, value);
}

#[test]
fn serde_round_trip_emit_spec_model() {
    let value = EmitSpec::Model { path: None };
    let encoded = serde_json::to_string(&value).unwrap();
    let decoded: EmitSpec = serde_json::from_str(&encoded).unwrap();
    assert_eq!(decoded, value);
}

#[test]
fn serde_round_trip_emit_spec_json_without_path() {
    let value = EmitSpec::Json { path: None };
    let encoded = serde_json::to_string(&value).unwrap();
    let decoded: EmitSpec = serde_json::from_str(&encoded).unwrap();
    assert_eq!(decoded, value);
}

#[test]
fn serde_round_trip_emit_spec_json_with_path() {
    let value = EmitSpec::Json {
        path: Some(PathBuf::from("out.json")),
    };
    let encoded = serde_json::to_string(&value).unwrap();
    let decoded: EmitSpec = serde_json::from_str(&encoded).unwrap();
    assert_eq!(decoded, value);
}

#[test]
fn serde_round_trip_emit_spec_analysis_with_name_and_path() {
    let value = EmitSpec::Analysis {
        analysis_name: Some("resources".to_string()),
        path: Some(PathBuf::from("out.json")),
    };
    let encoded = serde_json::to_string(&value).unwrap();
    let decoded: EmitSpec = serde_json::from_str(&encoded).unwrap();
    assert_eq!(decoded, value);
}

#[test]
fn serde_round_trip_regseq_param() {
    let value = RegseqParam {
        scope: ParamScope::Backend,
        name: "linux.header_path".to_string(),
        value: "/usr/include".to_string(),
    };
    let encoded = serde_json::to_string(&value).unwrap();
    let decoded: RegseqParam = serde_json::from_str(&encoded).unwrap();
    assert_eq!(decoded, value);
}

#[test]
fn serde_round_trip_full_compiler_options() {
    let options = CompilerOptions {
        platform_file: PathBuf::from("platform.toml"),
        output_dir: PathBuf::from("out"),
        emit: vec![
            EmitSpec::Driver {
                match_id: Some("pl011".to_string()),
            },
            EmitSpec::Analysis {
                analysis_name: Some("resources".to_string()),
                path: Some(PathBuf::from("out/resources.json")),
            },
        ],
        check: true,
        add_analysis: vec!["resources".to_string()],
        skip_analysis: vec!["dead-code".to_string()],
        deny: vec!["lint.critical".to_string()],
        allow: vec!["lint.non_blocking".to_string()],
        params: vec![
            RegseqParam {
                scope: ParamScope::General,
                name: "debug".to_string(),
                value: "true".to_string(),
            },
            RegseqParam {
                scope: ParamScope::Analysis,
                name: "staging.enabled".to_string(),
                value: "false".to_string(),
            },
        ],
        log_level: LogLevel::Info,
        dump_stage: Some(DumpStage::Mir),
        color: ColorChoice::Auto,
    };

    let encoded = serde_json::to_string(&options).unwrap();
    let decoded: CompilerOptions = serde_json::from_str(&encoded).unwrap();
    assert_eq!(decoded, options);
}
