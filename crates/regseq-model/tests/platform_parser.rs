use regseq_model::platform::PlatformValue;
use regseq_model::platform_parser::{parse_platform_file_str, PlatformParseError};

const MINIMAL_VALID: &str = r#"
version = "1"

[platform]
os = "linux"
board = "raspberry-pi-4"
arch = "aarch64"
"#;

#[test]
fn minimal_valid_platform_file_parses() {
    let file = parse_platform_file_str(MINIMAL_VALID).expect("should parse");
    assert_eq!(file.version, "1");
    assert_eq!(file.platform.os, "linux");
    assert_eq!(file.platform.board, "raspberry-pi-4");
    assert_eq!(file.platform.arch, "aarch64");
    assert!(file.properties.is_empty());
    assert!(file.construct_sections.is_empty());
    assert!(file.unknown_sections.is_empty());
}

#[test]
fn valid_file_with_properties_parses() {
    let input = r#"
version = "1"

[platform]
os = "linux"
board = "raspberry-pi-4"
arch = "aarch64"

[properties]
reg-io-width = 4
current-speed = 115200
fifo-size = 16
"#;
    let file = parse_platform_file_str(input).expect("should parse");
    assert_eq!(file.properties.len(), 3);

    let by_key = |k: &str| file.properties.iter().find(|p| p.key == k).unwrap();
    assert_eq!(by_key("reg-io-width").value, PlatformValue::Integer(4));
    assert_eq!(
        by_key("current-speed").value,
        PlatformValue::Integer(115200)
    );
    assert_eq!(by_key("fifo-size").value, PlatformValue::Integer(16));
}

#[test]
fn valid_file_with_construct_sections_preserves_them() {
    let input = r#"
version = "1"

[platform]
os = "linux"
board = "raspberry-pi-4"
arch = "aarch64"

[properties]
reg-io-width = 4

[mmio.uart_regs]
base = "0x09000000"
size = 4096

[interrupt.combined_irq]
trigger_type = "level_high"
shared = false
"#;
    let file = parse_platform_file_str(input).expect("should parse");

    assert_eq!(file.construct_sections.len(), 2);

    let mmio = file
        .construct_sections
        .iter()
        .find(|s| s.section_kind == "mmio")
        .expect("mmio section should exist");
    assert_eq!(mmio.section_key, Some("uart_regs".into()));
    assert_eq!(
        mmio.values.get("base"),
        Some(&PlatformValue::String("0x09000000".into()))
    );
    assert_eq!(mmio.values.get("size"), Some(&PlatformValue::Integer(4096)));

    let irq = file
        .construct_sections
        .iter()
        .find(|s| s.section_kind == "interrupt")
        .expect("interrupt section should exist");
    assert_eq!(irq.section_key, Some("combined_irq".into()));
    assert_eq!(
        irq.values.get("shared"),
        Some(&PlatformValue::Boolean(false))
    );
}

#[test]
fn unknown_simple_section_preserved() {
    let input = r#"
version = "1"

[platform]
os = "linux"
board = "test-board"
arch = "arm"

[custom_vendor]
debug_mode = true
log_level = "verbose"
"#;
    let file = parse_platform_file_str(input).expect("should parse");
    assert_eq!(file.unknown_sections.len(), 1);
    let us = &file.unknown_sections[0];
    assert_eq!(us.section_path, vec!["custom_vendor"]);
    assert_eq!(
        us.values.get("debug_mode"),
        Some(&PlatformValue::Boolean(true))
    );
    assert_eq!(
        us.values.get("log_level"),
        Some(&PlatformValue::String("verbose".into()))
    );
}

#[test]
fn missing_version_is_rejected() {
    let input = r#"
[platform]
os = "linux"
board = "test"
arch = "arm"
"#;
    let err = parse_platform_file_str(input).unwrap_err();
    assert_eq!(err, PlatformParseError::MissingKey("version"));
}

#[test]
fn missing_platform_is_rejected() {
    let input = r#"
version = "1"
"#;
    let err = parse_platform_file_str(input).unwrap_err();
    assert_eq!(err, PlatformParseError::MissingKey("platform"));
}

#[test]
fn missing_platform_os_is_rejected() {
    let input = r#"
version = "1"

[platform]
board = "test"
arch = "arm"
"#;
    let err = parse_platform_file_str(input).unwrap_err();
    assert_eq!(err, PlatformParseError::MissingKey("platform.os"));
}

#[test]
fn missing_platform_board_is_rejected() {
    let input = r#"
version = "1"

[platform]
os = "linux"
arch = "arm"
"#;
    let err = parse_platform_file_str(input).unwrap_err();
    assert_eq!(err, PlatformParseError::MissingKey("platform.board"));
}

#[test]
fn missing_platform_arch_is_rejected() {
    let input = r#"
version = "1"

[platform]
os = "linux"
board = "test"
"#;
    let err = parse_platform_file_str(input).unwrap_err();
    assert_eq!(err, PlatformParseError::MissingKey("platform.arch"));
}

#[test]
fn syntactically_malformed_toml_is_rejected() {
    let input = "this is not valid [[[ toml";
    let err = parse_platform_file_str(input).unwrap_err();
    match err {
        PlatformParseError::InvalidToml(_) => {}
        other => panic!("expected InvalidToml, got {other:?}"),
    }
}

#[test]
fn wrong_type_for_version_is_rejected() {
    let input = r#"
version = 42

[platform]
os = "linux"
board = "test"
arch = "arm"
"#;
    let err = parse_platform_file_str(input).unwrap_err();
    assert_eq!(
        err,
        PlatformParseError::WrongType {
            key: "version",
            expected: "string",
        }
    );
}

#[test]
fn wrong_type_for_platform_field_is_rejected() {
    let input = r#"
version = "1"

[platform]
os = 123
board = "test"
arch = "arm"
"#;
    let err = parse_platform_file_str(input).unwrap_err();
    assert_eq!(
        err,
        PlatformParseError::WrongType {
            key: "platform.os",
            expected: "string",
        }
    );
}

// ---- Bus section parsing tests ----

#[test]
fn bus_section_parses_as_typed_section_not_unknown() {
    let input = r#"
version = "1"

[platform]
os = "linux"
board = "test-board"
arch = "aarch64"

[bus]
type = "amba"
amba_id = 0x00041011
revision = 3
"#;
    let file = parse_platform_file_str(input).expect("should parse");
    assert!(file.bus.is_some(), "bus section should be parsed");
    assert!(
        file.unknown_sections.is_empty(),
        "bus should not appear in unknown sections"
    );
    let bus = file.bus.unwrap();
    assert_eq!(bus.bus_type, "amba");
    assert_eq!(
        bus.values.get("amba_id"),
        Some(&PlatformValue::Integer(0x00041011))
    );
    assert_eq!(bus.values.get("revision"), Some(&PlatformValue::Integer(3)));
}

#[test]
fn bus_section_with_integrated_type_parses() {
    let input = r#"
version = "1"

[platform]
os = "linux"
board = "test-board"
arch = "aarch64"

[bus]
type = "integrated"
firmware = "devicetree"
"#;
    let file = parse_platform_file_str(input).expect("should parse");
    let bus = file.bus.expect("bus should be present");
    assert_eq!(bus.bus_type, "integrated");
    assert_eq!(
        bus.values.get("firmware"),
        Some(&PlatformValue::String("devicetree".into()))
    );
}

#[test]
fn bus_section_with_pci_type_parses() {
    let input = r#"
version = "1"

[platform]
os = "linux"
board = "test-board"
arch = "x86_64"

[bus]
type = "pci"
vendor_id = 0x8086
device_id = 0x100e
"#;
    let file = parse_platform_file_str(input).expect("should parse");
    let bus = file.bus.expect("bus should be present");
    assert_eq!(bus.bus_type, "pci");
    assert_eq!(
        bus.values.get("vendor_id"),
        Some(&PlatformValue::Integer(0x8086))
    );
    assert_eq!(
        bus.values.get("device_id"),
        Some(&PlatformValue::Integer(0x100e))
    );
}

#[test]
fn bus_section_missing_type_is_rejected() {
    let input = r#"
version = "1"

[platform]
os = "linux"
board = "test-board"
arch = "aarch64"

[bus]
amba_id = 0x00041011
"#;
    let err = parse_platform_file_str(input).unwrap_err();
    assert_eq!(err, PlatformParseError::MissingKey("bus.type"));
}

#[test]
fn absent_bus_section_produces_none() {
    let file = parse_platform_file_str(MINIMAL_VALID).expect("should parse");
    assert!(file.bus.is_none());
}

#[test]
fn pl011_fixture_with_bus_section_parses() {
    let input = r#"
version = "1"

[platform]
os = "linux"
board = "raspberry-pi-4"
arch = "aarch64"

[bus]
type = "amba"
amba_id = 0x00041011
revision = 3

[properties]
reg-io-width = 4
current-speed = 115200
"#;
    let file = parse_platform_file_str(input).expect("should parse");
    let bus = file.bus.expect("bus should be present");
    assert_eq!(bus.bus_type, "amba");
    assert_eq!(file.properties.len(), 2);
    assert!(file.unknown_sections.is_empty());
}
