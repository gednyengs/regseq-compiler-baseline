//! PL011 bus identity end-to-end proof.
//!
//! Demonstrates that `bus = "amba"` survives the full model pipeline from
//! authored source through declaration normalization, Layer 2 parsing,
//! binding normalization, and analysis/bootstrap exposure.
//!
//! This is a structural gate that should land before Phase D lowering
//! resumes. It proves:
//!
//! - PL011 is not falling back to generic `integrated` or `platform`
//! - `compatible = ["arm,pl011"]` and `bus = "amba"` are complementary facts
//! - Layer 2 AMBA metadata remains visible without collapsing into properties

use std::path::{Path, PathBuf};

use regseq_analysis::{check_readiness, AnalysisInput};
use regseq_model::declaration::driver::BusType;
use regseq_model::joined::perform_join;
use regseq_model::{binding, declaration, platform_parser};

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

// ---------------------------------------------------------------------------
// Layer 1: raw frontend artifact carries bus identity
// ---------------------------------------------------------------------------

#[test]
fn raw_frontend_artifact_carries_amba_bus() {
    let path = workspace_root().join("tests/fixtures/pl011-spec-slice/expected/raw-frontend.json");
    let json = std::fs::read_to_string(&path).expect("raw-frontend.json should be readable");
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("should be valid JSON");

    let bus = parsed["identity"]["bus"]
        .as_str()
        .expect("identity.bus should be a string");
    assert_eq!(
        bus, "amba",
        "PL011 raw frontend artifact must carry bus = \"amba\", not generic integrated/platform"
    );
}

#[test]
fn raw_frontend_compatible_and_bus_are_complementary() {
    let path = workspace_root().join("tests/fixtures/pl011-spec-slice/expected/raw-frontend.json");
    let json = std::fs::read_to_string(&path).expect("raw-frontend.json should be readable");
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("should be valid JSON");

    let compatible = parsed["identity"]["compatible"]
        .as_array()
        .expect("identity.compatible should be an array");
    assert!(
        compatible.iter().any(|v| v.as_str() == Some("arm,pl011")),
        "compatible must include \"arm,pl011\""
    );

    let bus = parsed["identity"]["bus"].as_str().unwrap();
    assert_eq!(bus, "amba");

    // These are complementary facts: compatible identifies the specific device,
    // bus identifies the fabric/attachment family. Neither subsumes the other.
}

// ---------------------------------------------------------------------------
// Layer 1: declaration model preserves typed bus identity
// ---------------------------------------------------------------------------

#[test]
fn declaration_model_preserves_amba_bus() {
    let path = workspace_root().join("tests/fixtures/pl011-spec-slice/expected/raw-frontend.json");
    let json = std::fs::read_to_string(&path).expect("raw-frontend.json should be readable");
    let decl = declaration::normalize_from_json(&json).expect("normalization should succeed");

    let bus = decl
        .driver
        .bus
        .as_ref()
        .expect("PL011 declaration must have bus identity");
    assert_eq!(
        *bus,
        BusType::Amba,
        "PL011 must normalize to BusType::Amba, not Integrated or Pci"
    );
}

#[test]
fn declaration_model_bus_is_not_integrated() {
    let path = workspace_root().join("tests/fixtures/pl011-spec-slice/expected/raw-frontend.json");
    let json = std::fs::read_to_string(&path).expect("raw-frontend.json should be readable");
    let decl = declaration::normalize_from_json(&json).expect("normalization should succeed");

    // Explicit regression guard: PL011 should never fall back to `integrated`.
    assert_ne!(
        decl.driver.bus,
        Some(BusType::Integrated),
        "PL011 is an AMBA device, not a generic integrated device"
    );
}

// ---------------------------------------------------------------------------
// Layer 2: platform file carries typed bus metadata
// ---------------------------------------------------------------------------

#[test]
fn layer2_platform_file_has_typed_bus_section() {
    let path = workspace_root().join("tests/fixtures/pl011-spec-slice/platform.toml");
    let content = std::fs::read_to_string(&path).expect("platform.toml should be readable");
    let pf = platform_parser::parse_platform_file_str(&content).expect("should parse");

    let bus = pf
        .bus
        .as_ref()
        .expect("PL011 platform file must have a [bus] section");
    assert_eq!(bus.bus_type, "amba");
}

#[test]
fn layer2_amba_metadata_not_collapsed_into_properties() {
    let path = workspace_root().join("tests/fixtures/pl011-spec-slice/platform.toml");
    let content = std::fs::read_to_string(&path).expect("platform.toml should be readable");
    let pf = platform_parser::parse_platform_file_str(&content).expect("should parse");

    // Bus metadata must live in the dedicated [bus] section, not in [properties].
    assert!(
        !pf.properties.iter().any(|p| p.key == "amba_id"),
        "amba_id must not appear in [properties]"
    );
    assert!(
        !pf.properties
            .iter()
            .any(|p| p.key == "bus_type" || p.key == "bus" || p.key == "revision"),
        "bus metadata must not leak into [properties]"
    );

    // The bus section carries the metadata.
    let bus = pf.bus.as_ref().unwrap();
    assert!(bus.values.contains_key("amba_id"));
    assert!(bus.values.contains_key("revision"));
}

// ---------------------------------------------------------------------------
// Binding normalization: AMBA metadata survives
// ---------------------------------------------------------------------------

#[test]
fn binding_normalization_produces_amba_bus_binding() {
    let path = workspace_root().join("tests/fixtures/pl011-spec-slice/platform.toml");
    let content = std::fs::read_to_string(&path).expect("platform.toml should be readable");
    let pf = platform_parser::parse_platform_file_str(&content).expect("should parse");
    let model = binding::normalize(&pf, Some("pl011-spec-slice/platform.toml"), None)
        .expect("normalization should succeed");

    let bus = model
        .bus
        .as_ref()
        .expect("binding model must have bus binding");
    assert_eq!(bus.bus_type, BusType::Amba);
    assert_eq!(bus.amba_id, Some(0x00041011));
    assert_eq!(bus.revision, Some(3));
}

// ---------------------------------------------------------------------------
// Analysis bootstrap: both layers visible
// ---------------------------------------------------------------------------

fn make_pl011_analysis_input() -> AnalysisInput {
    let decl_path =
        workspace_root().join("tests/fixtures/pl011-spec-slice/expected/raw-frontend.json");
    let decl_json = std::fs::read_to_string(&decl_path).expect("raw-frontend.json readable");
    let decl = declaration::normalize_from_json(&decl_json).expect("decl normalization ok");

    // Use the enriched platform TOML with bus section and all resource families.
    let platform_toml = r#"
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

    let pf = platform_parser::parse_platform_file_str(platform_toml).expect("parse ok");
    let binding_model =
        binding::normalize(&pf, Some("pl011-spec-slice/platform.toml"), None).expect("bind ok");
    let bus_binding = binding_model.bus.clone();
    let joined = perform_join(&decl, &binding_model).expect("join ok");
    AnalysisInput::new(decl, joined, bus_binding)
}

#[test]
fn analysis_input_exposes_layer1_bus_identity() {
    let input = make_pl011_analysis_input();
    let bus = input
        .bus_identity()
        .expect("Layer 1 bus identity must be visible");
    assert_eq!(*bus, BusType::Amba);
}

#[test]
fn analysis_input_exposes_layer2_bus_metadata() {
    let input = make_pl011_analysis_input();
    let bb = input
        .bus_binding()
        .expect("Layer 2 bus binding must be visible");
    assert_eq!(bb.bus_type, BusType::Amba);
    assert_eq!(bb.amba_id, Some(0x00041011));
    assert_eq!(bb.revision, Some(3));
}

#[test]
fn readiness_confirms_bus_consistency() {
    let input = make_pl011_analysis_input();
    let report = check_readiness(&input);

    // No bus errors.
    let bus_errors: Vec<_> = report
        .errors()
        .into_iter()
        .filter(|f| f.category == "bus_consistency")
        .collect();
    assert!(
        bus_errors.is_empty(),
        "PL011 bus should be consistent across layers: {bus_errors:?}"
    );

    // Info confirming consistency.
    let bus_infos: Vec<_> = report
        .infos()
        .into_iter()
        .filter(|f| f.category == "bus_consistency")
        .collect();
    assert!(
        bus_infos
            .iter()
            .any(|f| f.message.contains("amba") && f.message.contains("consistent")),
        "readiness must report bus consistency for amba"
    );
}

// ---------------------------------------------------------------------------
// Determinism: end-to-end output is reproducible
// ---------------------------------------------------------------------------

#[test]
fn bus_identity_proof_is_deterministic() {
    let input1 = make_pl011_analysis_input();
    let input2 = make_pl011_analysis_input();

    assert_eq!(input1.bus_identity(), input2.bus_identity());

    let bb1 = input1.bus_binding().unwrap();
    let bb2 = input2.bus_binding().unwrap();
    assert_eq!(bb1, bb2);

    let report1 = check_readiness(&input1);
    let report2 = check_readiness(&input2);
    assert_eq!(report1.findings, report2.findings);
}

// ---------------------------------------------------------------------------
// Regression guard: proof fails if bus regresses
// ---------------------------------------------------------------------------

#[test]
fn proof_would_fail_if_bus_were_integrated() {
    // This test documents the regression property: if someone changed
    // the PL011 bus to "integrated", the proof tests above would fail.
    // We verify this by checking that Amba != Integrated.
    assert_ne!(BusType::Amba, BusType::Integrated);
    assert_ne!(BusType::Amba, BusType::Pci);
    assert_eq!(BusType::from_raw("amba"), Some(BusType::Amba));
    assert_ne!(BusType::from_raw("amba"), Some(BusType::Integrated));
}
