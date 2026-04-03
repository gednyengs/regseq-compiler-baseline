//! PL011 lowering-input stability proof.
//!
//! Proves that the current PL011 declaration model, joined artifact, and
//! analysis bootstrap together form a stable, deterministic lowering-input
//! contract for Phase D.
//!
//! This is the last Phase C gate. It covers the analysis/readiness output
//! over real PL011-shaped inputs, not just the join correctness already
//! proved by track-a-validation-05.

use std::path::{Path, PathBuf};

use regseq_analysis::{check_readiness, AnalysisInput};
use regseq_model::declaration::callbacks::CallbackFamily;
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

/// Load the real PL011 spec-slice declaration model.
fn load_pl011_decl() -> declaration::DeclarationModel {
    let path = workspace_root().join("tests/fixtures/pl011-spec-slice/expected/raw-frontend.json");
    let json =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("failed to read artifact: {e}"));
    declaration::normalize_from_json(&json).expect("normalization should succeed")
}

/// Enriched PL011 Layer 2 platform TOML exercising all accepted construct
/// families.
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

fn load_pl011_binding() -> binding::BindingModel {
    let pf = platform_parser::parse_platform_file_str(PL011_ENRICHED_PLATFORM_TOML)
        .expect("platform TOML should parse");
    binding::normalize(&pf, Some("pl011-spec-slice/platform.toml"))
        .expect("binding normalization should succeed")
}

fn make_pl011_analysis_input() -> AnalysisInput {
    let decl = load_pl011_decl();
    let binding = load_pl011_binding();
    let joined = perform_join(&decl, &binding).expect("join should succeed");
    AnalysisInput::new(decl, joined)
}

// ---------------------------------------------------------------------------
// Test 1: deterministic output
// ---------------------------------------------------------------------------

#[test]
fn pl011_lowering_input_proof_is_deterministic() {
    let input1 = make_pl011_analysis_input();
    let input2 = make_pl011_analysis_input();

    let report1 = check_readiness(&input1);
    let report2 = check_readiness(&input2);

    assert_eq!(report1.findings, report2.findings);
    assert_eq!(report1.join_summaries, report2.join_summaries);

    // Callback families are deterministic.
    assert_eq!(
        input1.callback_families_present(),
        input2.callback_families_present()
    );
}

// ---------------------------------------------------------------------------
// Test 2: callback-family visibility
// ---------------------------------------------------------------------------

#[test]
fn pl011_serial_and_lifecycle_callback_families_visible() {
    let input = make_pl011_analysis_input();
    let families = input.callback_families_present();

    // PL011 is a serial_uart device, should have lifecycle + serial.
    assert!(
        families.contains(&CallbackFamily::Lifecycle),
        "lifecycle callback family should be present"
    );
    assert!(
        families.contains(&CallbackFamily::Serial),
        "serial callback family should be present"
    );
}

#[test]
fn pl011_readiness_callback_integrity_passes() {
    let input = make_pl011_analysis_input();
    let report = check_readiness(&input);

    // No callback integrity errors for the real PL011 spec-slice.
    let callback_errors: Vec<_> = report
        .errors()
        .into_iter()
        .filter(|f| f.category == "callback_integrity")
        .collect();
    assert!(
        callback_errors.is_empty(),
        "expected no callback integrity errors, got: {callback_errors:?}"
    );
}

// ---------------------------------------------------------------------------
// Test 3: property-lint state visible in reviewer-facing output
// ---------------------------------------------------------------------------

#[test]
fn pl011_property_lints_visible_in_readiness() {
    let input = make_pl011_analysis_input();
    let report = check_readiness(&input);

    // The real PL011 spec-slice has "reg-io-width" marked as required with a
    // default value. This should produce a caution finding.
    let lint_cautions: Vec<_> = report
        .cautions()
        .into_iter()
        .filter(|f| f.category == "property_lint")
        .collect();

    // At least the required-with-default lint should appear.
    assert!(
        !lint_cautions.is_empty(),
        "expected at least one property lint caution, got none"
    );

    // Verify the caution is for the expected property.
    assert!(lint_cautions
        .iter()
        .any(|f| f.message.contains("reg-io-width")));
}

// ---------------------------------------------------------------------------
// Test 4: partial join outcomes visible in readiness
// ---------------------------------------------------------------------------

#[test]
fn pl011_join_summaries_present_in_report() {
    let input = make_pl011_analysis_input();
    let report = check_readiness(&input);

    assert_eq!(report.join_summaries.len(), 6);
    let families: Vec<&str> = report.join_summaries.iter().map(|s| s.family).collect();
    assert_eq!(
        families,
        &[
            "properties",
            "mmio",
            "interrupts",
            "clocks",
            "resets",
            "pinctrl"
        ]
    );
}

// ---------------------------------------------------------------------------
// Test 5: unmatched facts visible
// ---------------------------------------------------------------------------

#[test]
fn pl011_unmatched_facts_visible_in_readiness() {
    let input = make_pl011_analysis_input();
    let report = check_readiness(&input);

    // The PL011 spec-slice declares no interrupt resources, but the enriched
    // binding provides [interrupt.uart_irq]. This should appear as a
    // binding-only entry and generate a caution.
    let interrupt_summary = report
        .join_summaries
        .iter()
        .find(|s| s.family == "interrupts")
        .unwrap();

    // Binding-only interrupt is visible.
    assert_eq!(
        interrupt_summary.binding_only, 1,
        "expected 1 binding-only interrupt entry"
    );

    // Corresponding caution finding exists.
    let binding_only_cautions: Vec<_> = report
        .cautions()
        .into_iter()
        .filter(|f| {
            f.category == "join_completeness"
                && f.message.contains("interrupts")
                && f.message.contains("binding-only")
        })
        .collect();
    assert!(
        !binding_only_cautions.is_empty(),
        "expected caution for binding-only interrupt"
    );
}

// ---------------------------------------------------------------------------
// Test 6: declaration-only constructs accessible through bootstrap
// ---------------------------------------------------------------------------

#[test]
fn pl011_register_blocks_accessible() {
    let input = make_pl011_analysis_input();
    // The PL011 spec-slice has register block declarations.
    assert!(
        !input.register_blocks().is_empty(),
        "register blocks should be accessible"
    );
}

#[test]
fn pl011_register_descriptors_accessible() {
    let input = make_pl011_analysis_input();
    assert!(
        !input.register_descriptors().is_empty(),
        "register descriptors should be accessible"
    );
}

#[test]
fn pl011_wrapper_call_sites_accessible() {
    let input = make_pl011_analysis_input();
    assert!(
        !input.wrapper_call_sites().is_empty(),
        "wrapper call sites should be accessible"
    );
}

#[test]
fn pl011_poll_call_sites_accessible() {
    let input = make_pl011_analysis_input();
    // Poll call sites may or may not be present in the spec-slice.
    // This test verifies the accessor is reachable; empty is acceptable.
    let _ = input.poll_call_sites();
}

#[test]
fn pl011_synchronization_accessible() {
    let input = make_pl011_analysis_input();
    let sync = input.synchronization();
    // Accessor must work; content depends on spec-slice coverage.
    let _ = sync.sync_fields.len();
    let _ = input.lock_operations();
    let _ = input.completion_operations();
    let _ = input.guard_scopes();
}

#[test]
fn pl011_timed_delay_accessible() {
    let input = make_pl011_analysis_input();
    // Timed delay may or may not be present, but accessor must work.
    let _ = input.timed_delay_call_sites();
    let _ = input.duration_ctor_sites();
}

#[test]
fn pl011_logging_accessible() {
    let input = make_pl011_analysis_input();
    let _ = input.log_call_sites();
}

#[test]
fn pl011_pm_gate_accessible() {
    let input = make_pl011_analysis_input();
    let _ = input.pm_gates();
}

// ---------------------------------------------------------------------------
// Test 7: stable Phase C gate
// ---------------------------------------------------------------------------

#[test]
fn pl011_readiness_has_no_errors() {
    let input = make_pl011_analysis_input();
    let report = check_readiness(&input);

    assert!(
        !report.has_errors(),
        "PL011 lowering-input baseline should have no errors; Phase D can proceed. \
         Errors: {:?}",
        report.errors()
    );
}

#[test]
fn pl011_readiness_report_snapshot() {
    let input = make_pl011_analysis_input();
    let report = check_readiness(&input);

    // Build a reviewer-facing snapshot string.
    let mut snapshot = String::new();
    snapshot.push_str("=== PL011 Lowering-Input Readiness Report ===\n\n");

    snapshot.push_str("-- Callback families present --\n");
    for family in &input.callback_families_present() {
        snapshot.push_str(&format!("  {}\n", family.as_str()));
    }
    snapshot.push('\n');

    snapshot.push_str("-- Join summaries --\n");
    for s in &report.join_summaries {
        snapshot.push_str(&format!(
            "  {}: complete={} incomplete={} decl_only={} binding_only={}\n",
            s.family, s.joined_complete, s.joined_incomplete, s.decl_only, s.binding_only,
        ));
    }
    snapshot.push('\n');

    snapshot.push_str("-- Findings --\n");
    for f in &report.findings {
        snapshot.push_str(&format!(
            "  [{:?}] {}: {}\n",
            f.severity, f.category, f.message,
        ));
    }

    // Determinism: re-running should produce the same snapshot.
    let input2 = make_pl011_analysis_input();
    let report2 = check_readiness(&input2);

    let mut snapshot2 = String::new();
    snapshot2.push_str("=== PL011 Lowering-Input Readiness Report ===\n\n");

    snapshot2.push_str("-- Callback families present --\n");
    for family in &input2.callback_families_present() {
        snapshot2.push_str(&format!("  {}\n", family.as_str()));
    }
    snapshot2.push('\n');

    snapshot2.push_str("-- Join summaries --\n");
    for s in &report2.join_summaries {
        snapshot2.push_str(&format!(
            "  {}: complete={} incomplete={} decl_only={} binding_only={}\n",
            s.family, s.joined_complete, s.joined_incomplete, s.decl_only, s.binding_only,
        ));
    }
    snapshot2.push('\n');

    snapshot2.push_str("-- Findings --\n");
    for f in &report2.findings {
        snapshot2.push_str(&format!(
            "  [{:?}] {}: {}\n",
            f.severity, f.category, f.message,
        ));
    }

    assert_eq!(
        snapshot, snapshot2,
        "readiness report snapshot is not deterministic"
    );

    // Sanity: snapshot is non-empty and contains expected markers.
    assert!(snapshot.contains("lifecycle"));
    assert!(snapshot.contains("serial"));
    assert!(snapshot.contains("properties"));
    assert!(snapshot.contains("mmio"));
}
