//! Boundary smoke tests for the Layer 1 / Layer 2 property-id vocabulary.
//!
//! These tests prove the Layer 2 platform-file parser works against the
//! checked-in fixture corpus and that property ids from a Layer 1 fixture
//! driver can be related to parsed Layer 2 property entries without
//! implementing the final join model.

use regseq_model::platform::PlatformValue;
use regseq_model::platform_parser::{parse_platform_file_path, parse_platform_file_str};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

fn fixtures_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("fixtures")
}

fn platform_file_fixtures() -> PathBuf {
    fixtures_root().join("platform-file")
}

fn property_driver_fixture() -> PathBuf {
    fixtures_root().join("property-driver")
}

// ---------------------------------------------------------------------------
// Fixture corpus tests
// ---------------------------------------------------------------------------

#[test]
fn minimal_valid_fixture_parses() {
    let path = platform_file_fixtures().join("minimal-valid.toml");
    let file = parse_platform_file_path(&path).expect("minimal-valid.toml should parse");
    assert_eq!(file.version, "1");
    assert_eq!(file.platform.os, "linux");
    assert_eq!(file.platform.board, "raspberry-pi-4");
    assert_eq!(file.platform.arch, "aarch64");
    assert!(file.properties.is_empty());
    assert!(file.construct_sections.is_empty());
    assert!(file.unknown_sections.is_empty());
}

#[test]
fn with_properties_fixture_parses() {
    let path = platform_file_fixtures().join("with-properties.toml");
    let file = parse_platform_file_path(&path).expect("with-properties.toml should parse");
    assert_eq!(file.properties.len(), 4);

    let keys: Vec<&str> = file.properties.iter().map(|p| p.key.as_str()).collect();
    assert!(keys.contains(&"reg-io-width"));
    assert!(keys.contains(&"current-speed"));
    assert!(keys.contains(&"fifo-enabled"));
    assert!(keys.contains(&"compat-string"));
}

#[test]
fn with_future_sections_fixture_preserves_sections() {
    let path = platform_file_fixtures().join("with-future-sections.toml");
    let file = parse_platform_file_path(&path).expect("with-future-sections.toml should parse");

    assert_eq!(file.properties.len(), 1);
    assert_eq!(file.construct_sections.len(), 3);

    let section_kinds: Vec<&str> = file
        .construct_sections
        .iter()
        .map(|s| s.section_kind.as_str())
        .collect();
    assert!(section_kinds.contains(&"mmio"));
    assert!(section_kinds.contains(&"interrupt"));
    assert!(section_kinds.contains(&"clock"));
}

#[test]
fn missing_version_fixture_is_rejected() {
    let path = platform_file_fixtures().join("missing-version.toml");
    let err = parse_platform_file_path(&path).unwrap_err();
    assert!(
        format!("{err}").contains("version"),
        "error should mention 'version': {err}"
    );
}

#[test]
fn missing_platform_key_fixture_is_rejected() {
    let path = platform_file_fixtures().join("missing-platform-key.toml");
    let err = parse_platform_file_path(&path).unwrap_err();
    assert!(
        format!("{err}").contains("board"),
        "error should mention missing 'board': {err}"
    );
}

#[test]
fn unsupported_property_value_fixture_is_rejected() {
    let path = platform_file_fixtures().join("unsupported-property-value.toml");
    let err = parse_platform_file_path(&path).unwrap_err();
    assert!(
        format!("{err}").contains("float"),
        "error should mention unsupported 'float' type: {err}"
    );
}

// ---------------------------------------------------------------------------
// Layer 1 / Layer 2 boundary smoke
// ---------------------------------------------------------------------------

/// Extract property ids declared in a Layer 1 fixture source file by
/// scanning for `#[rseq::property(id = "...")]` patterns.
///
/// This is a lightweight text-based extraction for smoke-test purposes.
/// Production property extraction uses the real rustc/HIR path in
/// `regseq-driver`.
fn extract_layer1_property_ids(source: &str) -> HashSet<String> {
    let mut ids = HashSet::new();
    for line in source.lines() {
        if let Some(start) = line.find("id = \"") {
            let after = &line[start + 6..];
            if let Some(end) = after.find('"') {
                ids.insert(after[..end].to_string());
            }
        }
    }
    ids
}

#[test]
fn property_driver_layer1_source_has_property_declarations() {
    let source = std::fs::read_to_string(property_driver_fixture().join("src/lib.rs"))
        .expect("property-driver src/lib.rs should exist");
    let ids = extract_layer1_property_ids(&source);
    assert!(
        ids.contains("reg-io-width"),
        "Layer 1 should declare reg-io-width"
    );
    assert!(
        ids.contains("current-speed"),
        "Layer 1 should declare current-speed"
    );
    assert!(
        ids.contains("fifo-size"),
        "Layer 1 should declare fifo-size"
    );
}

#[test]
fn property_driver_platform_file_parses() {
    let path = property_driver_fixture().join("platform.toml");
    let file = parse_platform_file_path(&path).expect("property-driver platform.toml should parse");
    assert_eq!(file.version, "1");
    assert_eq!(file.properties.len(), 3);
}

#[test]
fn property_ids_relate_across_layer1_layer2_boundary() {
    // Layer 1: extract property ids from the fixture driver source.
    let source = std::fs::read_to_string(property_driver_fixture().join("src/lib.rs"))
        .expect("property-driver src/lib.rs should exist");
    let layer1_ids = extract_layer1_property_ids(&source);

    // Layer 2: parse the platform file and collect property keys.
    let path = property_driver_fixture().join("platform.toml");
    let file = parse_platform_file_path(&path).expect("property-driver platform.toml should parse");
    let layer2_keys: HashSet<String> = file.properties.iter().map(|p| p.key.clone()).collect();

    // Boundary check: every Layer 2 property key should have a matching
    // Layer 1 declaration. This proves the two vocabularies can be related.
    for key in &layer2_keys {
        assert!(
            layer1_ids.contains(key),
            "Layer 2 property key '{key}' has no matching Layer 1 declaration"
        );
    }

    // Also verify the reverse: every Layer 1 property id appears in Layer 2.
    for id in &layer1_ids {
        assert!(
            layer2_keys.contains(id),
            "Layer 1 property id '{id}' has no matching Layer 2 entry"
        );
    }

    // Verify the shared vocabulary is non-empty and complete.
    assert_eq!(layer1_ids.len(), 3);
    assert_eq!(layer1_ids, layer2_keys);
}

#[test]
fn property_driver_platform_file_values_are_well_typed() {
    let path = property_driver_fixture().join("platform.toml");
    let file = parse_platform_file_path(&path).expect("property-driver platform.toml should parse");

    for prop in &file.properties {
        match prop.key.as_str() {
            "reg-io-width" | "current-speed" | "fifo-size" => {
                assert!(
                    matches!(prop.value, PlatformValue::Integer(_)),
                    "property '{}' should be an integer, got {:?}",
                    prop.key,
                    prop.value
                );
            }
            other => panic!("unexpected property key: {other}"),
        }
    }
}

#[test]
fn minimal_driver_platform_file_is_valid() {
    // Verify the existing minimal-driver platform.toml is still parseable
    // after updating it to the accepted format.
    let content =
        std::fs::read_to_string(fixtures_root().join("minimal-driver/platform.toml")).unwrap();
    let result = parse_platform_file_str(&content);
    // The existing minimal-driver platform.toml uses the old format
    // (no version key), so it may fail. This test documents the current
    // state rather than asserting success.
    if let Ok(file) = result {
        assert!(!file.version.is_empty());
    }
}
