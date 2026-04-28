use regseq_model_old::platform::{
    ConstructSection, PlatformFile, PlatformHeader, PlatformValue, PropertyEntry, UnknownSection,
};
use std::collections::BTreeMap;

#[test]
fn core_schema_types_construct_and_import() {
    let header = PlatformHeader {
        os: "linux".into(),
        board: "raspberry-pi-4".into(),
        arch: "aarch64".into(),
    };
    assert_eq!(header.os, "linux");

    let entry = PropertyEntry {
        key: "reg-io-width".into(),
        value: PlatformValue::Integer(4),
    };
    assert_eq!(entry.key, "reg-io-width");

    let section = ConstructSection {
        section_kind: "mmio".into(),
        section_key: Some("uart_regs".into()),
        values: BTreeMap::new(),
    };
    assert_eq!(section.section_kind, "mmio");

    let unknown = UnknownSection {
        section_path: vec!["custom".into()],
        values: BTreeMap::new(),
    };
    assert_eq!(unknown.section_path.len(), 1);
}

#[test]
fn platform_value_supports_all_accepted_kinds() {
    // Scalar kinds
    let int_val = PlatformValue::Integer(42);
    assert_eq!(int_val, PlatformValue::Integer(42));

    let bool_val = PlatformValue::Boolean(true);
    assert_eq!(bool_val, PlatformValue::Boolean(true));

    let str_val = PlatformValue::String("hello".into());
    assert_eq!(str_val, PlatformValue::String("hello".into()));

    // Aggregate kinds
    let list_val = PlatformValue::List(vec![
        PlatformValue::Integer(1),
        PlatformValue::String("two".into()),
        PlatformValue::Boolean(false),
    ]);
    match &list_val {
        PlatformValue::List(items) => assert_eq!(items.len(), 3),
        _ => panic!("expected List variant"),
    }

    let mut table = BTreeMap::new();
    table.insert("nested_key".into(), PlatformValue::Integer(99));
    let table_val = PlatformValue::Table(table);
    match &table_val {
        PlatformValue::Table(map) => {
            assert_eq!(map.get("nested_key"), Some(&PlatformValue::Integer(99)));
        }
        _ => panic!("expected Table variant"),
    }
}

#[test]
fn representative_platform_file_preserves_all_sections() {
    let mut construct_values = BTreeMap::new();
    construct_values.insert("base".into(), PlatformValue::String("0x09000000".into()));
    construct_values.insert("size".into(), PlatformValue::Integer(0x1000));

    let mut unknown_values = BTreeMap::new();
    unknown_values.insert("custom_flag".into(), PlatformValue::Boolean(true));

    let file = PlatformFile {
        version: "1".into(),
        platform: PlatformHeader {
            os: "linux".into(),
            board: "raspberry-pi-4".into(),
            arch: "aarch64".into(),
        },
        properties: vec![
            PropertyEntry {
                key: "reg-io-width".into(),
                value: PlatformValue::Integer(4),
            },
            PropertyEntry {
                key: "current-speed".into(),
                value: PlatformValue::Integer(115200),
            },
        ],
        bus: None,
        construct_sections: vec![ConstructSection {
            section_kind: "mmio".into(),
            section_key: Some("uart_regs".into()),
            values: construct_values,
        }],
        unknown_sections: vec![UnknownSection {
            section_path: vec!["vendor_ext".into(), "debug".into()],
            values: unknown_values,
        }],
    };

    // version
    assert_eq!(file.version, "1");

    // platform header
    assert_eq!(file.platform.os, "linux");
    assert_eq!(file.platform.board, "raspberry-pi-4");
    assert_eq!(file.platform.arch, "aarch64");

    // property entries
    assert_eq!(file.properties.len(), 2);
    assert_eq!(file.properties[0].key, "reg-io-width");
    assert_eq!(file.properties[0].value, PlatformValue::Integer(4));
    assert_eq!(file.properties[1].key, "current-speed");
    assert_eq!(file.properties[1].value, PlatformValue::Integer(115200));

    // construct-owned section
    assert_eq!(file.construct_sections.len(), 1);
    let cs = &file.construct_sections[0];
    assert_eq!(cs.section_kind, "mmio");
    assert_eq!(cs.section_key, Some("uart_regs".into()));
    assert_eq!(
        cs.values.get("base"),
        Some(&PlatformValue::String("0x09000000".into()))
    );

    // unknown section
    assert_eq!(file.unknown_sections.len(), 1);
    let us = &file.unknown_sections[0];
    assert_eq!(us.section_path, vec!["vendor_ext", "debug"]);
    assert_eq!(
        us.values.get("custom_flag"),
        Some(&PlatformValue::Boolean(true))
    );
}

#[test]
fn serialization_round_trip() {
    let file = PlatformFile {
        version: "1".into(),
        platform: PlatformHeader {
            os: "zephyr".into(),
            board: "nrf52840dk".into(),
            arch: "arm".into(),
        },
        properties: vec![PropertyEntry {
            key: "reg-io-width".into(),
            value: PlatformValue::Integer(4),
        }],
        bus: None,
        construct_sections: vec![],
        unknown_sections: vec![],
    };

    let json = serde_json::to_string(&file).expect("serialization should succeed");
    let deserialized: PlatformFile =
        serde_json::from_str(&json).expect("deserialization should succeed");

    assert_eq!(file, deserialized);
}
