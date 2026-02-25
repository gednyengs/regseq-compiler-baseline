use std::str::FromStr;

use regseq_config::{validate_dotted_ident, ParamParseError, ParamScope, RegseqParam};

#[test]
fn param_parses_analysis_scope() {
    let parsed = RegseqParam::from_str("analysis.staging.enabled=true").unwrap();
    assert_eq!(parsed.scope, ParamScope::Analysis);
    assert_eq!(parsed.name, "staging.enabled");
    assert_eq!(parsed.value, "true");
}

#[test]
fn param_parses_backend_scope() {
    let parsed = RegseqParam::from_str("backend.linux.header_path=/usr/include").unwrap();
    assert_eq!(parsed.scope, ParamScope::Backend);
    assert_eq!(parsed.name, "linux.header_path");
    assert_eq!(parsed.value, "/usr/include");
}

#[test]
fn param_parses_regseq_scope_as_general() {
    let parsed = RegseqParam::from_str("regseq.debug=true").unwrap();
    assert_eq!(parsed.scope, ParamScope::General);
    assert_eq!(parsed.name, "debug");
    assert_eq!(parsed.value, "true");
}

#[test]
fn param_parses_without_prefix_as_general() {
    let parsed = RegseqParam::from_str("debug=true").unwrap();
    assert_eq!(parsed.scope, ParamScope::General);
    assert_eq!(parsed.name, "debug");
    assert_eq!(parsed.value, "true");
}

#[test]
fn param_splits_on_first_equals_only() {
    let parsed = RegseqParam::from_str("key=val=ue").unwrap();
    assert_eq!(parsed.value, "val=ue");
}

#[test]
fn param_missing_equals_fails() {
    assert!(matches!(
        RegseqParam::from_str("noequals"),
        Err(ParamParseError::MissingEquals(_))
    ));
}

#[test]
fn param_empty_name_fails() {
    assert!(matches!(
        RegseqParam::from_str("=value"),
        Err(ParamParseError::EmptyName)
    ));
}

#[test]
fn param_invalid_name_with_dash_fails() {
    assert!(matches!(
        RegseqParam::from_str("bad-name=value"),
        Err(ParamParseError::InvalidName { .. })
    ));
}

#[test]
fn param_invalid_name_starting_with_digit_fails() {
    assert!(matches!(
        RegseqParam::from_str("123start=value"),
        Err(ParamParseError::InvalidName { .. })
    ));
}

#[test]
fn param_dotted_name_without_prefix_is_general_scope() {
    let parsed = RegseqParam::from_str("a.b.c=value").unwrap();
    assert_eq!(parsed.scope, ParamScope::General);
    assert_eq!(parsed.name, "a.b.c");
}

#[test]
fn validate_dotted_ident_accepts_single_ident() {
    assert!(validate_dotted_ident("foo").is_ok());
}

#[test]
fn validate_dotted_ident_accepts_multiple_segments() {
    assert!(validate_dotted_ident("foo.bar.baz").is_ok());
}

#[test]
fn validate_dotted_ident_accepts_private_prefix() {
    assert!(validate_dotted_ident("_private.field").is_ok());
}

#[test]
fn validate_dotted_ident_rejects_empty_string() {
    assert!(validate_dotted_ident("").is_err());
}

#[test]
fn validate_dotted_ident_rejects_dash() {
    assert!(validate_dotted_ident("bad-name").is_err());
}

#[test]
fn validate_dotted_ident_rejects_leading_dot() {
    assert!(validate_dotted_ident(".leading_dot").is_err());
}

#[test]
fn validate_dotted_ident_rejects_trailing_dot() {
    assert!(validate_dotted_ident("trailing_dot.").is_err());
}

#[test]
fn validate_dotted_ident_rejects_double_dot() {
    assert!(validate_dotted_ident("double..dot").is_err());
}
