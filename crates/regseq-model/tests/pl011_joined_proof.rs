//! PL011 joined-artifact proof: validates that the real PL011 spec-slice
//! declaration model and an enriched PL011 binding model join cleanly,
//! producing a reviewable joined artifact with correct join/unmatched
//! structure.

use std::path::{Path, PathBuf};

use regseq_model::binding;
use regseq_model::declaration;
use regseq_model::joined::{perform_join, JoinOutcome, JoinedArtifact};
use regseq_model::platform::PlatformValue;
use regseq_model::platform_parser;

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

/// Load the real PL011 spec-slice declaration model from the checked-in
/// raw frontend artifact JSON.
fn load_pl011_decl() -> regseq_model::declaration::DeclarationModel {
    let path = workspace_root().join("tests/fixtures/pl011-spec-slice/expected/raw-frontend.json");
    let json = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read raw artifact: {e}"));
    declaration::normalize_from_json(&json).expect("declaration normalization should succeed")
}

/// A PL011-enriched Layer 2 platform TOML that exercises all accepted
/// construct-section families. Based on the real fixture header/properties
/// with resource construct sections added for the proof.
const PL011_ENRICHED_PLATFORM_TOML: &str = r#"
version = "1"

[platform]
os = "linux"
board = "raspberry-pi-4"
arch = "aarch64"

[properties]
reg-io-width = 4
current-speed = 115200

[mmio.uart_regs]
base = 150994944
size = 4096

[interrupt.uart_irq]
trigger_type = "level"
trigger_polarity = "high"
shared = false

[clock.uart_clk]
default_frequency = 48000000

[reset.reset]
reset_line_id = "uart_rst0"
reset_shared = false

[pinctrl.uart_pins]
pin_states = ["default", "sleep"]
"#;

/// Build the enriched PL011 binding model by parsing the TOML and normalizing.
fn load_pl011_binding() -> regseq_model::binding::BindingModel {
    let pf = platform_parser::parse_platform_file_str(PL011_ENRICHED_PLATFORM_TOML)
        .expect("platform TOML should parse");
    binding::normalize(&pf, Some("pl011-spec-slice/platform.toml"), None)
        .expect("binding normalization should succeed")
}

// ---------------------------------------------------------------------------
// Test 1: PL011 declaration and binding join successfully
// ---------------------------------------------------------------------------

#[test]
fn pl011_join_succeeds() {
    let decl = load_pl011_decl();
    let binding = load_pl011_binding();
    let artifact = perform_join(&decl, &binding).expect("join should succeed");

    // Header comes from binding.
    assert_eq!(artifact.header.os, "linux");
    assert_eq!(artifact.header.board, "raspberry-pi-4");
    assert_eq!(artifact.header.arch, "aarch64");
    assert_eq!(artifact.header.version, "1");
}

// ---------------------------------------------------------------------------
// Test 2: joined output is deterministic and reviewable
// ---------------------------------------------------------------------------

#[test]
fn pl011_joined_output_is_deterministic() {
    let decl = load_pl011_decl();
    let binding = load_pl011_binding();

    let a1 = perform_join(&decl, &binding).unwrap();
    let a2 = perform_join(&decl, &binding).unwrap();

    assert_eq!(a1, a2);

    let json1 = serde_json::to_string_pretty(&a1).unwrap();
    let json2 = serde_json::to_string_pretty(&a2).unwrap();
    assert_eq!(json1, json2);
}

#[test]
fn pl011_joined_artifact_round_trips_through_json() {
    let decl = load_pl011_decl();
    let binding = load_pl011_binding();
    let artifact = perform_join(&decl, &binding).unwrap();

    let json = serde_json::to_string_pretty(&artifact).unwrap();
    let roundtrip: JoinedArtifact = serde_json::from_str(&json).unwrap();
    assert_eq!(artifact, roundtrip);
}

// ---------------------------------------------------------------------------
// Test 3: expected PL011 resources/properties join to the right entries
// ---------------------------------------------------------------------------

#[test]
fn pl011_property_joins_match_expected_ids() {
    let decl = load_pl011_decl();
    let binding = load_pl011_binding();
    let artifact = perform_join(&decl, &binding).unwrap();

    // The raw artifact declares: reg-io-width (required), current-speed (optional).
    // The binding provides: reg-io-width = 4, current-speed = 115200.
    assert_eq!(artifact.properties_joined.len(), 2);

    let rio = artifact
        .properties_joined
        .iter()
        .find(|p| p.property_id == "reg-io-width")
        .expect("reg-io-width should be joined");
    assert_eq!(rio.binding_value, Some(PlatformValue::Integer(4)));
    assert!(rio.required);
    assert_eq!(rio.outcome, JoinOutcome::Matched);

    let speed = artifact
        .properties_joined
        .iter()
        .find(|p| p.property_id == "current-speed")
        .expect("current-speed should be joined");
    assert_eq!(speed.binding_value, Some(PlatformValue::Integer(115200)));
    assert_eq!(speed.outcome, JoinOutcome::Matched);
}

#[test]
fn pl011_mmio_join_matches() {
    let decl = load_pl011_decl();
    let binding = load_pl011_binding();
    let artifact = perform_join(&decl, &binding).unwrap();

    // Declaration: mmio field "regs" with mmio_id "uart_regs".
    // Binding: [mmio.uart_regs] base = 150994944, size = 4096.
    assert_eq!(artifact.mmio_joined.len(), 1);
    let entry = &artifact.mmio_joined[0];
    assert_eq!(entry.mmio_id, "uart_regs");
    assert_eq!(entry.field_name, "regs");
    assert_eq!(entry.base, Some(150994944));
    assert_eq!(entry.size, Some(4096));
    assert_eq!(entry.outcome, JoinOutcome::Matched);
    assert_eq!(entry.register_block_type.as_deref(), Some("Pl011Registers"));
}

#[test]
fn pl011_clock_join_matches() {
    let decl = load_pl011_decl();
    let binding = load_pl011_binding();
    let artifact = perform_join(&decl, &binding).unwrap();

    // Declaration: clock field "clock" with clock_id "uart_clk".
    // Binding: [clock.uart_clk] default_frequency = 48000000.
    assert_eq!(artifact.clocks_joined.len(), 1);
    let entry = &artifact.clocks_joined[0];
    assert_eq!(entry.clock_id, "uart_clk");
    assert_eq!(entry.field_name, "clock");
    assert_eq!(entry.default_frequency, Some(48_000_000));
    assert_eq!(entry.outcome, JoinOutcome::Matched);
}

#[test]
fn pl011_reset_join_matches_by_field_name() {
    let decl = load_pl011_decl();
    let binding = load_pl011_binding();
    let artifact = perform_join(&decl, &binding).unwrap();

    // Declaration: reset field "reset" (field-name join).
    // Binding: [reset.reset] reset_line_id = "uart_rst0", reset_shared = false.
    assert_eq!(artifact.resets_joined.len(), 1);
    let entry = &artifact.resets_joined[0];
    assert_eq!(entry.field_name, "reset");
    assert_eq!(entry.reset_line_id.as_deref(), Some("uart_rst0"));
    assert_eq!(entry.reset_shared, Some(false));
    assert_eq!(entry.outcome, JoinOutcome::Matched);
}

#[test]
fn pl011_pinctrl_join_matches_by_explicit_id() {
    let decl = load_pl011_decl();
    let binding = load_pl011_binding();
    let artifact = perform_join(&decl, &binding).unwrap();

    // Declaration: pinctrl field "pinctrl" with pinctrl_id "uart_pins".
    // Binding: [pinctrl.uart_pins] pin_states = ["default", "sleep"].
    assert_eq!(artifact.pinctrl_joined.len(), 1);
    let entry = &artifact.pinctrl_joined[0];
    assert_eq!(entry.field_name, "pinctrl");
    let states = entry.pin_states.as_ref().unwrap();
    assert_eq!(states, &["default", "sleep"]);
    assert_eq!(entry.outcome, JoinOutcome::Matched);
}

// ---------------------------------------------------------------------------
// Test 4: unmatched facts remain explicit
// ---------------------------------------------------------------------------

#[test]
fn pl011_unmatched_interrupt_is_decl_only() {
    let decl = load_pl011_decl();
    let binding = load_pl011_binding();
    let artifact = perform_join(&decl, &binding).unwrap();

    // The raw artifact declares NO interrupt resources, so the binding-side
    // interrupt entry has no match.
    assert!(artifact.interrupts_joined.is_empty());
    assert!(artifact.interrupts_decl_only.is_empty());
    // The binding provides [interrupt.uart_irq] which has no declaration match.
    assert_eq!(artifact.interrupts_binding_only.len(), 1);
    assert_eq!(
        artifact.interrupts_binding_only[0].join_key.value(),
        "uart_irq"
    );
}

#[test]
fn pl011_no_silent_drops() {
    let decl = load_pl011_decl();
    let binding = load_pl011_binding();
    let artifact = perform_join(&decl, &binding).unwrap();

    // Every declared property should appear in either joined or decl_only.
    let total_prop_decls = decl.properties.fields.len();
    let prop_accounted = artifact.properties_joined.len() + artifact.properties_decl_only.len();
    assert_eq!(total_prop_decls, prop_accounted);

    // Every binding property should appear in either joined or binding_only.
    let total_prop_bindings = binding.properties.len();
    let prop_binding_accounted =
        artifact.properties_joined.len() + artifact.properties_binding_only.len();
    assert_eq!(total_prop_bindings, prop_binding_accounted);

    // MMIO
    let mmio_decl_total = decl.resources.mmio.len();
    let mmio_accounted = artifact.mmio_joined.len() + artifact.mmio_decl_only.len();
    assert_eq!(mmio_decl_total, mmio_accounted);

    let mmio_binding_total = binding.mmio.len();
    let mmio_binding_accounted = artifact.mmio_joined.len() + artifact.mmio_binding_only.len();
    assert_eq!(mmio_binding_total, mmio_binding_accounted);

    // Clocks
    let clock_decl_total = decl.resources.clock.fields.len();
    let clock_accounted = artifact.clocks_joined.len() + artifact.clocks_decl_only.len();
    assert_eq!(clock_decl_total, clock_accounted);

    // Resets
    let reset_decl_total = decl.resources.reset.fields.len();
    let reset_accounted = artifact.resets_joined.len() + artifact.resets_decl_only.len();
    assert_eq!(reset_decl_total, reset_accounted);

    // PinCtrl
    let pinctrl_decl_total = decl.resources.pinctrl.fields.len();
    let pinctrl_accounted = artifact.pinctrl_joined.len() + artifact.pinctrl_decl_only.len();
    assert_eq!(pinctrl_decl_total, pinctrl_accounted);
}

// ---------------------------------------------------------------------------
// Test 5: partial join outcomes are reviewable
// ---------------------------------------------------------------------------

#[test]
fn pl011_partial_mmio_binding_produces_incomplete_outcome() {
    let decl = load_pl011_decl();
    let mut binding = load_pl011_binding();
    // Remove size to simulate incomplete binding.
    binding.mmio[0].size = None;

    let artifact = perform_join(&decl, &binding).unwrap();
    assert_eq!(artifact.mmio_joined.len(), 1);
    assert_eq!(
        artifact.mmio_joined[0].outcome,
        JoinOutcome::MatchedIncomplete
    );
    assert_eq!(artifact.mmio_joined[0].base, Some(150994944));
    assert!(artifact.mmio_joined[0].size.is_none());
}

#[test]
fn pl011_partial_clock_binding_produces_incomplete_outcome() {
    let decl = load_pl011_decl();
    let mut binding = load_pl011_binding();
    binding.clocks[0].default_frequency = None;

    let artifact = perform_join(&decl, &binding).unwrap();
    assert_eq!(artifact.clocks_joined.len(), 1);
    assert_eq!(
        artifact.clocks_joined[0].outcome,
        JoinOutcome::MatchedIncomplete
    );
}

#[test]
fn pl011_partial_reset_binding_produces_incomplete_outcome() {
    let decl = load_pl011_decl();
    let mut binding = load_pl011_binding();
    binding.resets[0].reset_shared = None;

    let artifact = perform_join(&decl, &binding).unwrap();
    assert_eq!(artifact.resets_joined.len(), 1);
    assert_eq!(
        artifact.resets_joined[0].outcome,
        JoinOutcome::MatchedIncomplete
    );
    assert_eq!(
        artifact.resets_joined[0].reset_line_id.as_deref(),
        Some("uart_rst0")
    );
}

#[test]
fn pl011_partial_pinctrl_binding_produces_incomplete_outcome() {
    let decl = load_pl011_decl();
    let mut binding = load_pl011_binding();
    binding.pinctrl[0].pin_states = None;

    let artifact = perform_join(&decl, &binding).unwrap();
    assert_eq!(artifact.pinctrl_joined.len(), 1);
    assert_eq!(
        artifact.pinctrl_joined[0].outcome,
        JoinOutcome::MatchedIncomplete
    );
}

// ---------------------------------------------------------------------------
// Provenance traceability
// ---------------------------------------------------------------------------

#[test]
fn pl011_joined_entries_carry_both_side_provenance() {
    let decl = load_pl011_decl();
    let binding = load_pl011_binding();
    let artifact = perform_join(&decl, &binding).unwrap();

    // MMIO: decl provenance from register_access, binding from mmio section.
    let mmio = &artifact.mmio_joined[0];
    assert_eq!(mmio.decl_provenance.raw_family, "register_access");
    assert_eq!(mmio.binding_provenance.section_kind, "mmio");
    assert_eq!(
        mmio.binding_provenance.source_file.as_deref(),
        Some("pl011-spec-slice/platform.toml")
    );

    // Clock.
    let clk = &artifact.clocks_joined[0];
    assert_eq!(clk.decl_provenance.raw_family, "clock");
    assert_eq!(clk.binding_provenance.section_kind, "clock");

    // Properties.
    let prop = &artifact.properties_joined[0];
    assert_eq!(prop.decl_provenance.raw_family, "firmware_properties");
    let bp = prop.binding_provenance.as_ref().unwrap();
    assert_eq!(bp.section_kind, "properties");
}
