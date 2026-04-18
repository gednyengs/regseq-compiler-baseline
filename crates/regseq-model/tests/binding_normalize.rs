//! Normalization tests for the parsed-platform to binding-model path.
//!
//! Verifies that parsed Layer 2 artifacts normalize into the binding model
//! correctly, preserving typed values, join keys, provenance, and section
//! ownership.

use std::collections::BTreeMap;

use regseq_model::binding::{normalize, BindingModel, BindingNormalizeError, JoinKey};
use regseq_model::platform::{
    ConstructSection, PlatformFile, PlatformHeader, PlatformValue, PropertyEntry, UnknownSection,
};

/// Build a minimal valid PlatformFile for testing.
fn minimal_platform_file() -> PlatformFile {
    PlatformFile {
        version: "1".into(),
        platform: PlatformHeader {
            os: "linux".into(),
            board: "raspberry-pi-4".into(),
            arch: "aarch64".into(),
        },
        properties: vec![],
        bus: None,
        construct_sections: vec![],
        unknown_sections: vec![],
    }
}

/// Build a rich PlatformFile exercising all accepted construct sections.
fn rich_platform_file() -> PlatformFile {
    PlatformFile {
        version: "1".into(),
        platform: PlatformHeader {
            os: "linux".into(),
            board: "raspberry-pi-4".into(),
            arch: "aarch64".into(),
        },
        properties: vec![
            PropertyEntry {
                key: "baud_rate".into(),
                value: PlatformValue::Integer(115200),
            },
            PropertyEntry {
                key: "hw_flow_control".into(),
                value: PlatformValue::Boolean(false),
            },
        ],
        construct_sections: vec![
            ConstructSection {
                section_kind: "mmio".into(),
                section_key: Some("uart_regs".into()),
                values: {
                    let mut m = BTreeMap::new();
                    m.insert("base".into(), PlatformValue::Integer(0x0900_0000));
                    m.insert("size".into(), PlatformValue::Integer(0x1000));
                    m
                },
            },
            ConstructSection {
                section_kind: "interrupt".into(),
                section_key: Some("uart_irq".into()),
                values: {
                    let mut m = BTreeMap::new();
                    m.insert("trigger_type".into(), PlatformValue::String("level".into()));
                    m.insert(
                        "trigger_polarity".into(),
                        PlatformValue::String("high".into()),
                    );
                    m.insert("priority".into(), PlatformValue::String("128".into()));
                    m.insert("shared".into(), PlatformValue::Boolean(false));
                    m
                },
            },
            ConstructSection {
                section_kind: "clock".into(),
                section_key: Some("uart_clk".into()),
                values: {
                    let mut m = BTreeMap::new();
                    m.insert(
                        "default_frequency".into(),
                        PlatformValue::Integer(48_000_000),
                    );
                    m
                },
            },
            ConstructSection {
                section_kind: "reset".into(),
                section_key: Some("uart_reset".into()),
                values: {
                    let mut m = BTreeMap::new();
                    m.insert("reset_line_id".into(), PlatformValue::String("rst0".into()));
                    m.insert("reset_shared".into(), PlatformValue::Boolean(false));
                    m
                },
            },
            ConstructSection {
                section_kind: "pinctrl".into(),
                section_key: Some("uart_pins".into()),
                values: {
                    let mut m = BTreeMap::new();
                    m.insert(
                        "pin_states".into(),
                        PlatformValue::List(vec![
                            PlatformValue::String("default".into()),
                            PlatformValue::String("sleep".into()),
                        ]),
                    );
                    m
                },
            },
        ],
        bus: None,
        unknown_sections: vec![UnknownSection {
            section_path: vec!["custom".into()],
            values: {
                let mut m = BTreeMap::new();
                m.insert("foo".into(), PlatformValue::String("bar".into()));
                m
            },
        }],
    }
}

// ---- Test 1: representative artifact normalizes successfully ----

#[test]
#[ignore = "Phase 9 cleanup: validates legacy pre-RegseqModel schema"]
fn minimal_platform_file_normalizes() {
    let pf = minimal_platform_file();
    let model = normalize(&pf, None, None).expect("normalization should succeed");

    assert_eq!(model.header.version, "1");
    assert_eq!(model.header.os, "linux");
    assert_eq!(model.header.arch, "aarch64");
    assert_eq!(model.header.board, "raspberry-pi-4");
    assert!(model.properties.is_empty());
    assert!(model.mmio.is_empty());
    assert!(model.interrupts.is_empty());
    assert!(model.clocks.is_empty());
    assert!(model.resets.is_empty());
    assert!(model.pinctrl.is_empty());
}

#[test]
#[ignore = "Phase 9 cleanup: validates legacy pre-RegseqModel schema"]
fn rich_platform_file_normalizes() {
    let pf = rich_platform_file();
    let model =
        normalize(&pf, Some("pl011.platform.toml"), None).expect("normalization should succeed");

    assert_eq!(model.properties.len(), 2);
    assert_eq!(model.mmio.len(), 1);
    assert_eq!(model.interrupts.len(), 1);
    assert_eq!(model.clocks.len(), 1);
    assert_eq!(model.resets.len(), 1);
    assert_eq!(model.pinctrl.len(), 1);
}

// ---- Test 2: all accepted sections map into the binding model ----

#[test]
#[ignore = "Phase 9 cleanup: validates legacy pre-RegseqModel schema"]
fn all_accepted_sections_are_mapped() {
    let pf = rich_platform_file();
    let model = normalize(&pf, None, None).unwrap();

    // MMIO
    assert_eq!(
        model.mmio[0].join_key,
        JoinKey::ExplicitId("uart_regs".into())
    );
    assert_eq!(model.mmio[0].base, Some(0x0900_0000));
    assert_eq!(model.mmio[0].size, Some(0x1000));

    // Interrupt
    assert_eq!(
        model.interrupts[0].join_key,
        JoinKey::ExplicitId("uart_irq".into())
    );
    assert_eq!(model.interrupts[0].trigger_type.as_deref(), Some("level"));
    assert_eq!(
        model.interrupts[0].trigger_polarity.as_deref(),
        Some("high")
    );
    assert_eq!(model.interrupts[0].priority.as_deref(), Some("128"));
    assert_eq!(model.interrupts[0].shared, Some(false));

    // Clock
    assert_eq!(
        model.clocks[0].join_key,
        JoinKey::ExplicitId("uart_clk".into())
    );
    assert_eq!(model.clocks[0].default_frequency, Some(48_000_000));

    // Reset (field-name join)
    assert_eq!(
        model.resets[0].join_key,
        JoinKey::FieldName("uart_reset".into())
    );
    assert_eq!(model.resets[0].reset_line_id.as_deref(), Some("rst0"));
    assert_eq!(model.resets[0].reset_shared, Some(false));

    // PinCtrl
    assert_eq!(
        model.pinctrl[0].join_key,
        JoinKey::ExplicitId("uart_pins".into())
    );
    let states = model.pinctrl[0].pin_states.as_ref().unwrap();
    assert_eq!(states, &["default", "sleep"]);
}

// ---- Test 3: typed property values survive normalization ----

#[test]
#[ignore = "Phase 9 cleanup: validates legacy pre-RegseqModel schema"]
fn property_values_survive_normalization() {
    let pf = rich_platform_file();
    let model = normalize(&pf, None, None).unwrap();

    let baud = model
        .properties
        .iter()
        .find(|p| p.key == "baud_rate")
        .unwrap();
    assert_eq!(baud.value, PlatformValue::Integer(115200));

    let flow = model
        .properties
        .iter()
        .find(|p| p.key == "hw_flow_control")
        .unwrap();
    assert_eq!(flow.value, PlatformValue::Boolean(false));
}

#[test]
#[ignore = "Phase 9 cleanup: validates legacy pre-RegseqModel schema"]
fn aggregate_property_values_survive_normalization() {
    let mut pf = minimal_platform_file();
    pf.properties.push(PropertyEntry {
        key: "dma_channels".into(),
        value: PlatformValue::List(vec![
            PlatformValue::String("tx".into()),
            PlatformValue::String("rx".into()),
        ]),
    });

    let model = normalize(&pf, None, None).unwrap();
    let dma = model
        .properties
        .iter()
        .find(|p| p.key == "dma_channels")
        .unwrap();
    assert!(matches!(&dma.value, PlatformValue::List(items) if items.len() == 2));
}

// ---- Test 4: join-relevant identifiers survive normalization ----

#[test]
#[ignore = "Phase 9 cleanup: validates legacy pre-RegseqModel schema"]
fn join_keys_survive_normalization() {
    let pf = rich_platform_file();
    let model = normalize(&pf, None, None).unwrap();

    // Explicit-id join keys
    assert_eq!(model.mmio[0].join_key.value(), "uart_regs");
    assert_eq!(model.interrupts[0].join_key.value(), "uart_irq");
    assert_eq!(model.clocks[0].join_key.value(), "uart_clk");
    assert_eq!(model.pinctrl[0].join_key.value(), "uart_pins");

    // Field-name join key
    assert_eq!(model.resets[0].join_key.value(), "uart_reset");
    assert!(matches!(model.resets[0].join_key, JoinKey::FieldName(_)));
}

// ---- Test 5: section/key provenance survives normalization ----

#[test]
#[ignore = "Phase 9 cleanup: validates legacy pre-RegseqModel schema"]
fn provenance_survives_normalization() {
    let pf = rich_platform_file();
    let model = normalize(&pf, Some("pl011.platform.toml"), None).unwrap();

    // Header provenance
    assert_eq!(model.header.provenance.section_kind, "platform");
    assert_eq!(
        model.header.provenance.source_file.as_deref(),
        Some("pl011.platform.toml")
    );

    // Property provenance
    let baud = model
        .properties
        .iter()
        .find(|p| p.key == "baud_rate")
        .unwrap();
    assert_eq!(baud.provenance.section_kind, "properties");
    assert_eq!(baud.provenance.section_key.as_deref(), Some("baud_rate"));
    assert_eq!(
        baud.provenance.source_file.as_deref(),
        Some("pl011.platform.toml")
    );

    // Resource provenance
    assert_eq!(model.mmio[0].provenance.section_kind, "mmio");
    assert_eq!(
        model.mmio[0].provenance.section_key.as_deref(),
        Some("uart_regs")
    );

    assert_eq!(model.interrupts[0].provenance.section_kind, "interrupt");
    assert_eq!(
        model.interrupts[0].provenance.section_key.as_deref(),
        Some("uart_irq")
    );

    assert_eq!(model.clocks[0].provenance.section_kind, "clock");
    assert_eq!(
        model.clocks[0].provenance.section_key.as_deref(),
        Some("uart_clk")
    );

    assert_eq!(model.resets[0].provenance.section_kind, "reset");
    assert_eq!(
        model.resets[0].provenance.section_key.as_deref(),
        Some("uart_reset")
    );

    assert_eq!(model.pinctrl[0].provenance.section_kind, "pinctrl");
    assert_eq!(
        model.pinctrl[0].provenance.section_key.as_deref(),
        Some("uart_pins")
    );
}

#[test]
#[ignore = "Phase 9 cleanup: validates legacy pre-RegseqModel schema"]
fn provenance_without_source_file_has_none() {
    let pf = rich_platform_file();
    let model = normalize(&pf, None, None).unwrap();

    assert!(model.header.provenance.source_file.is_none());
    assert!(model.mmio[0].provenance.source_file.is_none());
}

// ---- Test 6: absent sections remain absent ----

#[test]
#[ignore = "Phase 9 cleanup: validates legacy pre-RegseqModel schema"]
fn absent_sections_remain_absent() {
    let pf = minimal_platform_file();
    let model = normalize(&pf, None, None).unwrap();

    assert!(model.properties.is_empty());
    assert!(model.mmio.is_empty());
    assert!(model.interrupts.is_empty());
    assert!(model.clocks.is_empty());
    assert!(model.resets.is_empty());
    assert!(model.pinctrl.is_empty());
}

#[test]
#[ignore = "Phase 9 cleanup: validates legacy pre-RegseqModel schema"]
fn unknown_sections_are_not_normalized() {
    let pf = rich_platform_file();
    // The rich fixture has an unknown section; it should not appear in the model.
    let model = normalize(&pf, None, None).unwrap();

    // Only accepted families should be populated.
    assert_eq!(model.mmio.len(), 1);
    assert_eq!(model.interrupts.len(), 1);
    assert_eq!(model.clocks.len(), 1);
    assert_eq!(model.resets.len(), 1);
    assert_eq!(model.pinctrl.len(), 1);
    // No way for unknown sections to enter the binding model.
}

#[test]
#[ignore = "Phase 9 cleanup: validates legacy pre-RegseqModel schema"]
fn unrecognized_construct_section_kinds_are_skipped() {
    let mut pf = minimal_platform_file();
    pf.construct_sections.push(ConstructSection {
        section_kind: "dma".into(),
        section_key: Some("tx_channel".into()),
        values: {
            let mut m = BTreeMap::new();
            m.insert("channel".into(), PlatformValue::Integer(0));
            m
        },
    });

    let model = normalize(&pf, None, None).unwrap();
    // DMA is not an accepted section for this batch.
    assert!(model.mmio.is_empty());
    assert!(model.interrupts.is_empty());
    assert!(model.clocks.is_empty());
    assert!(model.resets.is_empty());
    assert!(model.pinctrl.is_empty());
}

// ---- Error cases ----

#[test]
#[ignore = "Phase 9 cleanup: validates legacy pre-RegseqModel schema"]
fn missing_section_key_is_an_error() {
    let mut pf = minimal_platform_file();
    pf.construct_sections.push(ConstructSection {
        section_kind: "mmio".into(),
        section_key: None,
        values: BTreeMap::new(),
    });

    let err = normalize(&pf, None, None).unwrap_err();
    assert_eq!(
        err,
        BindingNormalizeError::MissingSectionKey {
            section_kind: "mmio".into(),
        }
    );
}

// ---- Normalization is deterministic ----

#[test]
#[ignore = "Phase 9 cleanup: validates legacy pre-RegseqModel schema"]
fn normalization_is_deterministic() {
    let pf = rich_platform_file();
    let model1 = normalize(&pf, Some("pl011.platform.toml"), None).unwrap();
    let model2 = normalize(&pf, Some("pl011.platform.toml"), None).unwrap();

    assert_eq!(model1, model2);

    let json1 = serde_json::to_string_pretty(&model1).unwrap();
    let json2 = serde_json::to_string_pretty(&model2).unwrap();
    assert_eq!(json1, json2);
}

// ---- Fields with absent values normalize to None ----

#[test]
#[ignore = "Phase 9 cleanup: validates legacy pre-RegseqModel schema"]
fn partial_construct_section_normalizes_with_none_fields() {
    let mut pf = minimal_platform_file();
    // An interrupt section with only trigger_type, other fields absent.
    pf.construct_sections.push(ConstructSection {
        section_kind: "interrupt".into(),
        section_key: Some("uart_irq".into()),
        values: {
            let mut m = BTreeMap::new();
            m.insert("trigger_type".into(), PlatformValue::String("edge".into()));
            m
        },
    });

    let model = normalize(&pf, None, None).unwrap();
    assert_eq!(model.interrupts.len(), 1);
    assert_eq!(model.interrupts[0].trigger_type.as_deref(), Some("edge"));
    assert!(model.interrupts[0].trigger_polarity.is_none());
    assert!(model.interrupts[0].priority.is_none());
    assert!(model.interrupts[0].shared.is_none());
}

// ---- Round-trip through parser + normalization ----

#[test]
#[ignore = "Phase 9 cleanup: validates legacy pre-RegseqModel schema"]
fn parser_output_normalizes_end_to_end() {
    let toml_input = r#"
version = "1"

[platform]
os = "linux"
board = "qemu-virt"
arch = "aarch64"

[properties]
baud_rate = 115200

[mmio.uart_regs]
base = 150994944
size = 4096

[interrupt.uart_irq]
trigger_type = "level"
trigger_polarity = "high"
shared = false

[clock.uart_clk]
default_frequency = 48000000

[reset.uart_reset]
reset_line_id = "rst0"
reset_shared = false

[pinctrl.uart_pins]
pin_states = ["default", "sleep"]
"#;

    let pf = regseq_model::platform_parser::parse_platform_file_str(toml_input)
        .expect("parse should succeed");
    let model =
        normalize(&pf, Some("test.platform.toml"), None).expect("normalization should succeed");

    assert_eq!(model.header.os, "linux");
    assert_eq!(model.header.board, "qemu-virt");
    assert_eq!(model.properties.len(), 1);
    assert_eq!(model.mmio.len(), 1);
    assert_eq!(model.interrupts.len(), 1);
    assert_eq!(model.clocks.len(), 1);
    assert_eq!(model.resets.len(), 1);
    assert_eq!(model.pinctrl.len(), 1);

    // Verify round-trip serialization.
    let json = serde_json::to_string_pretty(&model).unwrap();
    let roundtrip: BindingModel = serde_json::from_str(&json).unwrap();
    assert_eq!(model, roundtrip);
}

// ---- Bus section normalization tests ----

#[test]
#[ignore = "Phase 9 cleanup: validates legacy pre-RegseqModel schema"]
fn amba_bus_section_normalizes_successfully() {
    let toml_input = r#"
version = "1"

[platform]
os = "linux"
board = "raspberry-pi-4"
arch = "aarch64"

[bus]
type = "amba"
amba_id = 0x00041011
revision = 3
"#;

    let pf = regseq_model::platform_parser::parse_platform_file_str(toml_input)
        .expect("parse should succeed");
    let model = normalize(&pf, None, None).expect("normalization should succeed");

    let bus = model.bus.expect("bus binding should be present");
    assert_eq!(
        bus.bus_type,
        regseq_model::declaration::driver::BusType::Amba
    );
    assert_eq!(bus.amba_id, Some(0x00041011));
    assert_eq!(bus.revision, Some(3));
    assert_eq!(bus.provenance.section_kind, "bus");
}

#[test]
#[ignore = "Phase 9 cleanup: validates legacy pre-RegseqModel schema"]
fn amba_bus_with_declaration_match_normalizes() {
    let toml_input = r#"
version = "1"

[platform]
os = "linux"
board = "raspberry-pi-4"
arch = "aarch64"

[bus]
type = "amba"
amba_id = 0x00041011
revision = 3
"#;

    let pf = regseq_model::platform_parser::parse_platform_file_str(toml_input)
        .expect("parse should succeed");
    let decl_bus = regseq_model::declaration::driver::BusType::Amba;
    let model = normalize(&pf, None, Some(&decl_bus)).expect("normalization should succeed");

    let bus = model.bus.expect("bus binding should be present");
    assert_eq!(
        bus.bus_type,
        regseq_model::declaration::driver::BusType::Amba
    );
}

#[test]
#[ignore = "Phase 9 cleanup: validates legacy pre-RegseqModel schema"]
fn bus_type_mismatch_is_rejected() {
    let toml_input = r#"
version = "1"

[platform]
os = "linux"
board = "test-board"
arch = "aarch64"

[bus]
type = "amba"
amba_id = 0x00041011
"#;

    let pf = regseq_model::platform_parser::parse_platform_file_str(toml_input)
        .expect("parse should succeed");
    let decl_bus = regseq_model::declaration::driver::BusType::Pci;
    let err = normalize(&pf, None, Some(&decl_bus)).unwrap_err();
    assert_eq!(
        err,
        BindingNormalizeError::BusTypeMismatch {
            layer1: "pci".into(),
            layer2: "amba".into(),
        }
    );
}

#[test]
#[ignore = "Phase 9 cleanup: validates legacy pre-RegseqModel schema"]
fn unrecognized_bus_type_is_rejected() {
    let toml_input = r#"
version = "1"

[platform]
os = "linux"
board = "test-board"
arch = "aarch64"

[bus]
type = "usb"
"#;

    let pf = regseq_model::platform_parser::parse_platform_file_str(toml_input)
        .expect("parse should succeed");
    let err = normalize(&pf, None, None).unwrap_err();
    assert_eq!(
        err,
        BindingNormalizeError::UnrecognizedBusType {
            bus_type: "usb".into(),
        }
    );
}

#[test]
#[ignore = "Phase 9 cleanup: validates legacy pre-RegseqModel schema"]
fn invalid_family_specific_key_is_rejected() {
    let toml_input = r#"
version = "1"

[platform]
os = "linux"
board = "test-board"
arch = "aarch64"

[bus]
type = "amba"
vendor_id = 0x8086
"#;

    let pf = regseq_model::platform_parser::parse_platform_file_str(toml_input)
        .expect("parse should succeed");
    let err = normalize(&pf, None, None).unwrap_err();
    assert_eq!(
        err,
        BindingNormalizeError::InvalidBusKey {
            key: "vendor_id".into(),
            bus_type: "amba".into(),
        }
    );
}

#[test]
#[ignore = "Phase 9 cleanup: validates legacy pre-RegseqModel schema"]
fn pl011_fixture_with_amba_bus_normalizes_deterministically() {
    let toml_input = r#"
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

[mmio.uart_regs]
base = 150994944
size = 4096
"#;

    let pf = regseq_model::platform_parser::parse_platform_file_str(toml_input)
        .expect("parse should succeed");
    let decl_bus = regseq_model::declaration::driver::BusType::Amba;
    let model1 = normalize(&pf, Some("pl011.platform.toml"), Some(&decl_bus))
        .expect("normalization should succeed");
    let model2 = normalize(&pf, Some("pl011.platform.toml"), Some(&decl_bus))
        .expect("normalization should succeed");

    assert_eq!(model1, model2);

    let bus = model1.bus.as_ref().expect("bus should be present");
    assert_eq!(
        bus.bus_type,
        regseq_model::declaration::driver::BusType::Amba
    );
    assert_eq!(bus.amba_id, Some(0x00041011));
    assert_eq!(bus.revision, Some(3));
    assert_eq!(
        bus.provenance.source_file.as_deref(),
        Some("pl011.platform.toml")
    );

    // JSON round-trip.
    let json = serde_json::to_string_pretty(&model1).unwrap();
    let roundtrip: BindingModel = serde_json::from_str(&json).unwrap();
    assert_eq!(model1, roundtrip);
}

#[test]
#[ignore = "Phase 9 cleanup: validates legacy pre-RegseqModel schema"]
fn absent_bus_section_normalizes_to_none() {
    let pf = minimal_platform_file();
    let model = normalize(&pf, None, None).unwrap();
    assert!(model.bus.is_none());
}

#[test]
#[ignore = "Phase 9 cleanup: validates legacy pre-RegseqModel schema"]
fn bus_normalization_without_declaration_skips_consistency_check() {
    let toml_input = r#"
version = "1"

[platform]
os = "linux"
board = "test-board"
arch = "aarch64"

[bus]
type = "pci"
vendor_id = 0x8086
device_id = 0x100e
"#;

    let pf = regseq_model::platform_parser::parse_platform_file_str(toml_input)
        .expect("parse should succeed");
    // No declaration bus provided -- consistency check is skipped.
    let model = normalize(&pf, None, None).expect("normalization should succeed");
    let bus = model.bus.expect("bus should be present");
    assert_eq!(
        bus.bus_type,
        regseq_model::declaration::driver::BusType::Pci
    );
}
