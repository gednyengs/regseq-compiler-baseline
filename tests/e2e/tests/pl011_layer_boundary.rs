//! Minimal Layer 1 / Layer 2 boundary-compatibility proof for the real PL011
//! slice.
//!
//! Parses the checked-in `platform.toml` and loads the raw frontend artifact,
//! then verifies they share a coherent boundary vocabulary without performing
//! the final Phase C join.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use regseq_model::platform_parser::parse_platform_file_path;

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

fn fixture_path() -> PathBuf {
    workspace_root().join("tests/fixtures/pl011-spec-slice")
}

fn load_raw_artifact() -> serde_json::Value {
    let path = fixture_path().join("expected/raw-frontend.json");
    let content = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read raw artifact: {e}"));
    serde_json::from_str(&content).expect("raw artifact is valid JSON")
}

// ---------------------------------------------------------------------------
// Layer 2 header contract
// ---------------------------------------------------------------------------

#[test]
fn boundary_platform_file_satisfies_b07_header_contract() {
    let platform = parse_platform_file_path(&fixture_path().join("platform.toml"))
        .expect("platform.toml should parse");

    assert_eq!(platform.version, "1", "version must be '1'");
    assert!(
        !platform.platform.os.is_empty(),
        "platform.os must be present"
    );
    assert!(
        !platform.platform.board.is_empty(),
        "platform.board must be present"
    );
    assert!(
        !platform.platform.arch.is_empty(),
        "platform.arch must be present"
    );
}

// ---------------------------------------------------------------------------
// Target OS coherence
// ---------------------------------------------------------------------------

#[test]
fn boundary_target_os_matches_platform_os() {
    let platform = parse_platform_file_path(&fixture_path().join("platform.toml"))
        .expect("platform.toml should parse");
    let artifact = load_raw_artifact();

    let artifact_os = artifact["target_os"].as_str().expect("target_os string");
    let platform_os = &platform.platform.os;

    assert_eq!(
        artifact_os, platform_os,
        "raw artifact target_os '{artifact_os}' should match platform.os '{platform_os}'"
    );
}

// ---------------------------------------------------------------------------
// Property-id vocabulary boundary
// ---------------------------------------------------------------------------

#[test]
fn boundary_layer2_property_ids_exist_in_layer1() {
    let platform = parse_platform_file_path(&fixture_path().join("platform.toml"))
        .expect("platform.toml should parse");
    let artifact = load_raw_artifact();

    let layer2_ids: HashSet<&str> = platform.properties.iter().map(|p| p.key.as_str()).collect();

    let layer1_ids: HashSet<String> = artifact["firmware_properties"]["fields"]
        .as_array()
        .expect("firmware_properties.fields array")
        .iter()
        .map(|f| f["property_id"].as_str().unwrap().to_string())
        .collect();

    for l2_id in &layer2_ids {
        assert!(
            layer1_ids.contains(*l2_id),
            "Layer 2 property id '{l2_id}' has no matching Layer 1 declaration"
        );
    }
}

#[test]
fn boundary_layer1_property_ids_present_in_layer2_or_optional() {
    let platform = parse_platform_file_path(&fixture_path().join("platform.toml"))
        .expect("platform.toml should parse");
    let artifact = load_raw_artifact();

    let layer2_ids: HashSet<&str> = platform.properties.iter().map(|p| p.key.as_str()).collect();

    let fields = artifact["firmware_properties"]["fields"]
        .as_array()
        .expect("firmware_properties.fields array");

    for field in fields {
        let id = field["property_id"].as_str().unwrap();
        let required = field["required"].as_bool().unwrap();

        if required {
            // Required Layer 1 properties should be present in Layer 2
            // (or have a default -- we don't validate default semantics here,
            // just record presence/absence for boundary awareness).
            let has_default = !field["default_literal"].is_null();
            let in_layer2 = layer2_ids.contains(id);
            assert!(
                in_layer2 || has_default,
                "Required Layer 1 property '{id}' is neither in Layer 2 nor has a default"
            );
        }
        // Optional properties may or may not appear in Layer 2 -- that's fine.
    }
}

#[test]
fn boundary_shared_property_ids_are_exactly_two() {
    let platform = parse_platform_file_path(&fixture_path().join("platform.toml"))
        .expect("platform.toml should parse");
    let artifact = load_raw_artifact();

    let layer2_ids: HashSet<&str> = platform.properties.iter().map(|p| p.key.as_str()).collect();

    let layer1_ids: HashSet<String> = artifact["firmware_properties"]["fields"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["property_id"].as_str().unwrap().to_string())
        .collect();

    let shared: HashSet<&&str> = layer2_ids
        .iter()
        .filter(|id| layer1_ids.contains(**id))
        .collect();

    assert_eq!(
        shared.len(),
        2,
        "expected exactly 2 shared property ids (reg-io-width, current-speed), found: {shared:?}"
    );
    assert!(shared.contains(&&"reg-io-width"));
    assert!(shared.contains(&&"current-speed"));
}
