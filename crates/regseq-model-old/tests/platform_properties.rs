use regseq_model_old::platform::PlatformValue;
use regseq_model_old::platform_parser::{parse_platform_file_str, PlatformParseError};
use std::collections::BTreeMap;

fn wrap_properties(props: &str) -> String {
    format!(
        r#"
version = "1"

[platform]
os = "linux"
board = "test-board"
arch = "aarch64"

[properties]
{props}
"#
    )
}

#[test]
fn scalar_integer_property_parses() {
    let input = wrap_properties("reg-io-width = 4");
    let file = parse_platform_file_str(&input).expect("should parse");
    assert_eq!(file.properties.len(), 1);
    assert_eq!(file.properties[0].key, "reg-io-width");
    assert_eq!(file.properties[0].value, PlatformValue::Integer(4));
}

#[test]
fn scalar_boolean_property_parses() {
    let input = wrap_properties("fifo-enabled = true");
    let file = parse_platform_file_str(&input).expect("should parse");
    assert_eq!(file.properties.len(), 1);
    assert_eq!(file.properties[0].key, "fifo-enabled");
    assert_eq!(file.properties[0].value, PlatformValue::Boolean(true));
}

#[test]
fn scalar_string_property_parses() {
    let input = wrap_properties(r#"compat-string = "arm,pl011""#);
    let file = parse_platform_file_str(&input).expect("should parse");
    assert_eq!(file.properties.len(), 1);
    assert_eq!(file.properties[0].key, "compat-string");
    assert_eq!(
        file.properties[0].value,
        PlatformValue::String("arm,pl011".into())
    );
}

#[test]
fn aggregate_list_property_parses() {
    let input = wrap_properties(r#"pin-states = ["default", "sleep"]"#);
    let file = parse_platform_file_str(&input).expect("should parse");
    assert_eq!(file.properties.len(), 1);
    assert_eq!(file.properties[0].key, "pin-states");
    assert_eq!(
        file.properties[0].value,
        PlatformValue::List(vec![
            PlatformValue::String("default".into()),
            PlatformValue::String("sleep".into()),
        ])
    );
}

#[test]
fn aggregate_table_property_parses() {
    let input = wrap_properties(
        r#"
[properties.clock-config]
frequency = 48000000
source = "pll"
"#,
    );
    let file = parse_platform_file_str(&input).expect("should parse");
    let prop = file
        .properties
        .iter()
        .find(|p| p.key == "clock-config")
        .expect("clock-config property should exist");
    let mut expected = BTreeMap::new();
    expected.insert("frequency".into(), PlatformValue::Integer(48000000));
    expected.insert("source".into(), PlatformValue::String("pll".into()));
    assert_eq!(prop.value, PlatformValue::Table(expected));
}

#[test]
fn float_property_value_is_rejected() {
    let input = wrap_properties("voltage = 3.3");
    let err = parse_platform_file_str(&input).unwrap_err();
    assert_eq!(
        err,
        PlatformParseError::UnsupportedPropertyValue {
            key: "voltage".into(),
            toml_type: "float",
        }
    );
}

#[test]
fn datetime_property_value_is_rejected() {
    let input = wrap_properties("manufactured = 2026-01-15T10:00:00Z");
    let err = parse_platform_file_str(&input).unwrap_err();
    assert_eq!(
        err,
        PlatformParseError::UnsupportedPropertyValue {
            key: "manufactured".into(),
            toml_type: "datetime",
        }
    );
}

#[test]
fn float_inside_property_list_is_rejected() {
    let input = wrap_properties("thresholds = [1.5, 2.5]");
    let err = parse_platform_file_str(&input).unwrap_err();
    assert_eq!(
        err,
        PlatformParseError::UnsupportedPropertyValue {
            key: "thresholds".into(),
            toml_type: "float",
        }
    );
}

#[test]
fn property_keys_preserved_exactly() {
    let input = wrap_properties(
        r#"
reg-io-width = 4
current-speed = 115200
arm-primecell-periphid = 255
"#,
    );
    let file = parse_platform_file_str(&input).expect("should parse");
    let keys: Vec<&str> = file.properties.iter().map(|p| p.key.as_str()).collect();
    assert!(keys.contains(&"reg-io-width"));
    assert!(keys.contains(&"current-speed"));
    assert!(keys.contains(&"arm-primecell-periphid"));
}

#[test]
fn parsed_properties_are_serializable() {
    let input = wrap_properties(
        r#"
reg-io-width = 4
fifo-enabled = true
compat = "arm,pl011"
"#,
    );
    let file = parse_platform_file_str(&input).expect("should parse");

    // Verify Debug formatting works.
    let debug_str = format!("{:?}", file.properties);
    assert!(debug_str.contains("reg-io-width"));
    assert!(debug_str.contains("fifo-enabled"));

    // Verify serde round-trip works.
    let json = serde_json::to_string(&file.properties).expect("serialize should succeed");
    let deserialized: Vec<regseq_model_old::platform::PropertyEntry> =
        serde_json::from_str(&json).expect("deserialize should succeed");
    assert_eq!(file.properties, deserialized);
}
