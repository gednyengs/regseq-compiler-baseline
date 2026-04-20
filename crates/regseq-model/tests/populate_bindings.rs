//! Integration tests for [`regseq_model::populate_bindings`] (task
//! `regseq-model-59`).
//!
//! Every test:
//! - Loads the PL011 raw-frontend JSON and normalizes it to a
//!   Phase-1 [`RegseqModel`].
//! - Parses the PL011 `platform.toml` fixture.
//! - Mutates one of them to construct the scenario under test.
//! - Calls [`populate_bindings`] and asserts on the result.
//!
//! The tests exercise the full Test Plan from the task spec:
//! happy path, Phase-2 determinism, optional-absent acceptance,
//! every [`JoinErrorKind`] negative case, provenance checks, and
//! mutation-API rollback.

use std::path::PathBuf;

use regseq_model::ir::{LiteralValue, ScalarType};
use regseq_model::model::{ModuleKind, RegseqModel};
use regseq_model::normalize::raw_artifact::RawFrontendArtifact;
use regseq_model::platform::{ConstructSection, PlatformFile, PlatformValue, PropertyEntry};
use regseq_model::platform_parser::parse_platform_file_path;
use regseq_model::verify::{verify_phase_2, VerifierErrorKind};
use regseq_model::{normalize, populate_bindings, JoinErrorKind};

// ---------------------------------------------------------------------
// Fixture loading
// ---------------------------------------------------------------------

fn fixture_root() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("../../tests/fixtures/pl011-full-spec");
    p
}

fn load_raw_artifact() -> RawFrontendArtifact {
    let path = fixture_root().join("expected/raw-frontend.json");
    let raw = std::fs::read_to_string(&path).expect("read raw-frontend.json fixture");
    serde_json::from_str(&raw).expect("deserialize RawFrontendArtifact")
}

fn load_platform() -> PlatformFile {
    parse_platform_file_path(&fixture_root().join("platform.toml"))
        .expect("parse platform.toml fixture")
}

fn build_phase_1_model() -> RegseqModel {
    normalize(&load_raw_artifact()).expect("normalize must succeed")
}

/// Byte-identical JSON snapshot of a [`RegseqModel`].
///
/// [`RegseqModel`] does not itself derive [`serde::Serialize`] (see
/// its rustdoc -- the root struct compares its interner by identity).
/// The Phase-1 determinism test in
/// `crates/regseq-model/tests/normalize.rs` uses the same DTO
/// projection: `symbols.strings()` for the interner pool in insertion
/// order (the backing `HashMap` iteration is intentionally omitted
/// because it is non-deterministic and redundant with the `Vec`),
/// `{:#?}` for the module tree (deterministic: every intermediate
/// container is a `Vec` in surface-visit order or a
/// `BTreeMap`/`BTreeSet`), `{:#?}` for `source_locations` (a
/// [`BTreeMap`] with deterministic iteration), and the `format_version`
/// `u32`. Serializing that DTO with `serde_json::to_string_pretty`
/// yields genuine JSON bytes that can be byte-compared.
///
/// The assertion used against this helper is `sa == sb` over the raw
/// serialized bytes, which is the "true byte-equality check"
/// required by the task spec's Review section.
///
/// [`BTreeMap`]: std::collections::BTreeMap
fn serialize_model_json(model: &RegseqModel) -> String {
    #[derive(serde::Serialize)]
    struct Snapshot<'a> {
        strings: &'a [String],
        root_module_debug: String,
        source_locations_debug: String,
        format_version: u32,
    }

    let snap = Snapshot {
        strings: model.symbols.strings(),
        root_module_debug: format!("{:#?}", model.root_module),
        source_locations_debug: format!("{:#?}", model.source_locations),
        format_version: model.format_version,
    };
    serde_json::to_string_pretty(&snap).expect("Snapshot must serialize to JSON")
}

/// Signature used for rollback byte-equality comparisons. Same
/// projection as [`serialize_model_json`], but formatted as a
/// pipe-delimited plain string to make diff output readable in
/// assertion failure messages.
fn model_signature(model: &RegseqModel) -> String {
    format!(
        "{:?}|{:#?}|{:#?}|{}",
        model.symbols.strings(),
        model.root_module,
        model.source_locations,
        model.format_version,
    )
}

/// Find the construct section with the given (kind, id) and return
/// a mutable reference to it.
fn find_section_mut<'a>(
    platform: &'a mut PlatformFile,
    kind: &str,
    id: &str,
) -> &'a mut ConstructSection {
    platform
        .construct_sections
        .iter_mut()
        .find(|s| s.section_kind == kind && s.section_key.as_deref() == Some(id))
        .expect("fixture must contain the requested construct section")
}

// ---------------------------------------------------------------------
// Happy path
// ---------------------------------------------------------------------

#[test]
fn happy_path_pl011_fixture_populates_every_binding_and_passes_phase_2() {
    let mut model = build_phase_1_model();
    let platform = load_platform();

    populate_bindings(&mut model, &platform).expect("populate_bindings must succeed");

    let driver = model
        .root_module
        .submodules
        .iter()
        .find(|m| m.kind == ModuleKind::Driver)
        .and_then(|m| m.driver.as_ref())
        .expect("driver submodule present");

    // Every MMIO binding (the only Single-shape resource on this
    // fixture) is populated.
    for m in &driver.mmio {
        assert!(m.binding.is_some(), "mmio binding must be populated");
    }
    // Every interrupt binding is populated (Single-shape on PL011).
    for i in &driver.interrupts {
        assert!(i.binding.is_some(), "interrupt binding must be populated");
        let b = i.binding.as_ref().unwrap();
        assert_eq!(b.irq_number, 37);
        assert_eq!(b.trigger_type.as_deref(), Some("level"));
        assert_eq!(b.trigger_polarity.as_deref(), Some("high"));
        assert!(!b.shared);
    }
    // Optional clock/reset/pinctrl on PL011 are platform-supplied,
    // so they must be populated post-join too.
    for c in &driver.clocks {
        assert!(c.binding.is_some(), "clock binding must be populated");
        assert_eq!(c.binding.as_ref().unwrap().default_frequency, 48_000_000);
    }
    for r in &driver.resets {
        assert!(r.binding.is_some(), "reset binding must be populated");
        let b = r.binding.as_ref().unwrap();
        assert_eq!(b.reset_line_id, "uart_rst0");
        assert!(!b.reset_shared);
    }
    for p in &driver.pinctrl {
        assert!(p.binding.is_some(), "pinctrl binding must be populated");
        assert_eq!(
            p.binding.as_ref().unwrap().pin_states,
            vec!["default".to_string(), "sleep".to_string()],
        );
    }
    // Every required property has a bound value.
    for p in &driver.properties {
        assert!(
            p.bound_value.is_some(),
            "property {} must be populated",
            model.symbols.resolve(p.property_id),
        );
    }

    // verify_phase_2 is run inside commit(); this assertion re-asserts
    // it explicitly per the Test Plan.
    verify_phase_2(&model).expect("verify_phase_2 must pass on the joined model");
}

// ---------------------------------------------------------------------
// Phase-2 determinism (byte-identical serialization across runs)
// ---------------------------------------------------------------------

#[test]
fn repeated_populate_bindings_is_byte_identical() {
    // Two runs of populate_bindings on two independently-normalized
    // PL011 models must produce byte-identical JSON serialization.
    // This is a *true* byte-equality check (see `serialize_model_json`).
    let platform = load_platform();
    let mut a = build_phase_1_model();
    let mut b = build_phase_1_model();

    populate_bindings(&mut a, &platform).expect("first populate_bindings");
    populate_bindings(&mut b, &platform).expect("second populate_bindings");

    let sa = serialize_model_json(&a);
    let sb = serialize_model_json(&b);
    assert_eq!(
        sa.len(),
        sb.len(),
        "Phase-2 determinism: JSON output lengths differ"
    );
    assert!(
        sa.as_bytes() == sb.as_bytes(),
        "Phase-2 determinism: repeated populate_bindings produced divergent JSON serialization"
    );
}

// ---------------------------------------------------------------------
// Optional-absent is OK
// ---------------------------------------------------------------------

#[test]
fn optional_reset_absent_is_accepted() {
    // PL011 declares `reset` as `Option<Reset>` (required=false).
    // Removing the platform `[reset.reset]` section must leave the
    // reset binding as `None` and still produce `Ok(())`.
    let mut model = build_phase_1_model();
    let mut platform = load_platform();
    platform
        .construct_sections
        .retain(|s| s.section_kind != "reset");

    populate_bindings(&mut model, &platform).expect("optional-absent must be accepted");

    let driver = model
        .root_module
        .submodules
        .iter()
        .find(|m| m.kind == ModuleKind::Driver)
        .and_then(|m| m.driver.as_ref())
        .expect("driver present");
    for r in &driver.resets {
        assert!(
            r.binding.is_none(),
            "unpopulated optional reset must remain None"
        );
    }
}

// ---------------------------------------------------------------------
// Negative -- unknown platform section id (clock.bogus_clk)
// ---------------------------------------------------------------------

#[test]
fn unknown_clock_section_fails_with_unknown_platform_section() {
    let mut model = build_phase_1_model();
    let mut platform = load_platform();
    platform.construct_sections.push(ConstructSection {
        section_kind: "clock".into(),
        section_key: Some("bogus_clk".into()),
        values: Default::default(),
    });

    let err = populate_bindings(&mut model, &platform).expect_err("bogus clock must fail");
    match &err.kind {
        JoinErrorKind::UnknownPlatformSection { kind, id } => {
            assert_eq!(kind, "clock");
            assert_eq!(id, "bogus_clk");
        }
        other => panic!("expected UnknownPlatformSection, got {other:?}"),
    }
    assert!(
        !err.provenance.platform_section.is_empty(),
        "provenance.platform_section must be non-empty",
    );
    assert_eq!(
        err.provenance.resource_id.as_deref(),
        Some("bogus_clk"),
        "provenance.resource_id must cite the offending id",
    );
    assert!(
        err.provenance.platform_section.contains("clock.bogus_clk"),
        "platform_section header must reference the offending section",
    );
}

// ---------------------------------------------------------------------
// Negative -- required resource without section
// ---------------------------------------------------------------------

#[test]
fn missing_required_mmio_surfaces_via_phase_2_verifier() {
    let mut model = build_phase_1_model();
    let mut platform = load_platform();
    // Remove the mmio section so the required mmio resource is
    // left unbound.
    platform
        .construct_sections
        .retain(|s| s.section_kind != "mmio");

    let err = populate_bindings(&mut model, &platform).expect_err("required mmio missing");
    match &err.kind {
        JoinErrorKind::Phase2VerificationFailed { inner } => match &inner.kind {
            VerifierErrorKind::RequiredBindingMissing { slot, .. } => {
                assert_eq!(*slot, "mmio");
            }
            other => panic!("expected RequiredBindingMissing, got {other:?}"),
        },
        other => panic!("expected Phase2VerificationFailed, got {other:?}"),
    }
    // Provenance must still be populated: the verifier-wrapper
    // branch stamps the sentinel `<post-join verify>` rather than
    // leaving `platform_section` empty.
    assert!(
        !err.provenance.platform_section.is_empty(),
        "Phase2VerificationFailed must still populate platform_section",
    );
    assert_eq!(
        err.provenance.platform_section, "<post-join verify>",
        "sentinel string documents that the error originated at commit-time verification",
    );
}

// ---------------------------------------------------------------------
// Negative -- property type mismatch
// ---------------------------------------------------------------------

#[test]
fn property_type_mismatch_reports_declared_and_provided_tags() {
    let mut model = build_phase_1_model();
    let mut platform = load_platform();
    // `current-speed` is declared as u32; set it to a string.
    let entry = platform
        .properties
        .iter_mut()
        .find(|p| p.key == "current-speed")
        .expect("fixture has current-speed property");
    entry.value = PlatformValue::String("oops".into());

    let err = populate_bindings(&mut model, &platform).expect_err("string for u32 must fail");
    match &err.kind {
        JoinErrorKind::PropertyTypeMismatch {
            property_id,
            declared,
            provided,
        } => {
            assert_eq!(property_id, "current-speed");
            assert_eq!(declared, "u32");
            assert_eq!(provided, "string");
        }
        other => panic!("expected PropertyTypeMismatch, got {other:?}"),
    }
    assert!(
        !err.provenance.platform_section.is_empty(),
        "provenance.platform_section must be non-empty"
    );
    assert_eq!(err.provenance.resource_id.as_deref(), Some("current-speed"),);
}

// ---------------------------------------------------------------------
// Negative -- unknown property id
// ---------------------------------------------------------------------

#[test]
fn unknown_property_id_raises_unknown_platform_section_with_property_kind() {
    let mut model = build_phase_1_model();
    let mut platform = load_platform();
    platform.properties.push(PropertyEntry {
        key: "bogus-prop".into(),
        value: PlatformValue::Integer(1),
    });

    let err = populate_bindings(&mut model, &platform).expect_err("unknown property must fail");
    match &err.kind {
        JoinErrorKind::UnknownPlatformSection { kind, id } => {
            assert_eq!(kind, "property");
            assert_eq!(id, "bogus-prop");
        }
        other => panic!("expected UnknownPlatformSection, got {other:?}"),
    }
    assert!(
        !err.provenance.platform_section.is_empty(),
        "provenance.platform_section must be non-empty"
    );
    assert_eq!(err.provenance.resource_id.as_deref(), Some("bogus-prop"),);
}

// ---------------------------------------------------------------------
// Mutation API integration -- rollback on error
// ---------------------------------------------------------------------

#[test]
fn rejected_join_leaves_the_model_byte_equal_to_pre_join() {
    let mut model = build_phase_1_model();
    let mut platform = load_platform();
    platform.construct_sections.push(ConstructSection {
        section_kind: "clock".into(),
        section_key: Some("bogus_clk".into()),
        values: Default::default(),
    });

    let pre = model_signature(&model);
    let err = populate_bindings(&mut model, &platform).expect_err("bogus clock must fail");
    let _ = err;
    let post = model_signature(&model);

    assert_eq!(
        pre, post,
        "rejected join must roll the model back byte-for-byte"
    );
}

// ---------------------------------------------------------------------
// Sanity: LiteralValue coercion for a fixture property
// ---------------------------------------------------------------------

#[test]
fn property_bound_value_matches_declared_scalar_type() {
    let mut model = build_phase_1_model();
    let platform = load_platform();
    populate_bindings(&mut model, &platform).expect("happy path");

    let driver = model
        .root_module
        .submodules
        .iter()
        .find(|m| m.kind == ModuleKind::Driver)
        .and_then(|m| m.driver.as_ref())
        .expect("driver present");

    // `current-speed` is declared u32 and the platform file sets it
    // to 115200 -- the stored literal must be U32(115200).
    let prop = driver
        .properties
        .iter()
        .find(|p| model.symbols.resolve(p.property_id) == "current-speed")
        .expect("current-speed property present");
    assert_eq!(prop.field_type, ScalarType::U32);
    assert_eq!(prop.bound_value, Some(LiteralValue::U32(115_200)));
}

// Guard against accidental static use: keep the helper referenced.
#[test]
fn find_section_mut_helper_is_usable() {
    let mut platform = load_platform();
    let s = find_section_mut(&mut platform, "mmio", "uart_regs");
    assert_eq!(s.section_kind, "mmio");
}
