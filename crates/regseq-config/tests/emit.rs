use std::path::PathBuf;
use std::str::FromStr;

use regseq_config::{EmitParseError, EmitSpec};

#[test]
fn emit_parses_driver_without_match_id() {
    assert_eq!(
        EmitSpec::from_str("driver").unwrap(),
        EmitSpec::Driver { match_id: None }
    );
}

#[test]
fn emit_parses_driver_with_match_id() {
    assert_eq!(
        EmitSpec::from_str("driver=pl011").unwrap(),
        EmitSpec::Driver {
            match_id: Some("pl011".to_string())
        }
    );
}

#[test]
fn emit_parses_analysis_without_details() {
    assert_eq!(
        EmitSpec::from_str("analysis").unwrap(),
        EmitSpec::Analysis {
            analysis_name: None,
            path: None
        }
    );
}

#[test]
fn emit_parses_analysis_with_subtype() {
    assert_eq!(
        EmitSpec::from_str("analysis.resources").unwrap(),
        EmitSpec::Analysis {
            analysis_name: Some("resources".to_string()),
            path: None
        }
    );
}

#[test]
fn emit_parses_analysis_with_subtype_and_path() {
    assert_eq!(
        EmitSpec::from_str("analysis.resources=report.json").unwrap(),
        EmitSpec::Analysis {
            analysis_name: Some("resources".to_string()),
            path: Some(PathBuf::from("report.json"))
        }
    );
}

#[test]
fn emit_parses_analysis_with_path_only() {
    assert_eq!(
        EmitSpec::from_str("analysis=report.json").unwrap(),
        EmitSpec::Analysis {
            analysis_name: None,
            path: Some(PathBuf::from("report.json"))
        }
    );
}

#[test]
fn emit_parses_raw_artifact() {
    assert_eq!(
        EmitSpec::from_str("raw-artifact").unwrap(),
        EmitSpec::RawArtifact,
    );
}

#[test]
fn emit_parses_model_without_path() {
    assert_eq!(
        EmitSpec::from_str("model").unwrap(),
        EmitSpec::Model { path: None }
    );
}

#[test]
fn emit_parses_model_with_path() {
    assert_eq!(
        EmitSpec::from_str("model=out/pl011.model.json").unwrap(),
        EmitSpec::Model {
            path: Some(PathBuf::from("out/pl011.model.json"))
        }
    );
}

#[test]
fn emit_parse_unknown_type_fails() {
    assert!(matches!(
        EmitSpec::from_str("unknown"),
        Err(EmitParseError::UnknownType(_))
    ));
}

#[test]
fn emit_parse_empty_input_fails() {
    assert!(matches!(
        EmitSpec::from_str(""),
        Err(EmitParseError::UnknownType(_))
    ));
}

#[test]
fn emit_display_round_trip_for_supported_forms() {
    let cases = [
        "driver",
        "driver=pl011",
        "analysis",
        "analysis.resources",
        "analysis.resources=report.json",
        "analysis=report.json",
        "raw-artifact",
        "model",
        "model=out/pl011.model.json",
    ];

    for input in cases {
        let parsed = EmitSpec::from_str(input).unwrap();
        assert_eq!(parsed.to_string(), input);
    }
}
