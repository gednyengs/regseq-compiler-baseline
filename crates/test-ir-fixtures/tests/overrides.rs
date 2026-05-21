//! Override-surface tests for the F68 D1 helper extension (per
//! `.harness/tasks/regseq-driver/69-test-ir-fixtures-build-flag-override.md`
//! Test Plan B2).
//!
//! Covers:
//!  - `BuildOverrides::default()` carries None / empty fields
//!    (sanity check that the default value preserves pre-D1
//!    helper behavior bit-for-bit).
//!  - `try_bridge_fixture_path_with_overrides` with the default
//!    overrides produces the same `BridgeOutcome` discriminant as
//!    `try_bridge_fixture_path` on an existing fixture (no
//!    behavioral drift on the default path).
//!  - `try_bridge_fixture_path_with_overrides` panics with the
//!    documented diagnostic when `z_flags` carries a value outside
//!    [`test_ir_fixtures::Z_FLAG_ALLOWLIST`]. The runner's matrix
//!    loader is the first line of defense; this panic is the
//!    helper-side defense-in-depth backstop.

#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::PathBuf;

use test_ir_fixtures::{
    fixture_names, try_bridge_fixture_path, try_bridge_fixture_path_with_overrides, BridgeOutcome,
    BuildOverrides,
};

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join(format!("{name}.rs"))
}

fn outcome_discriminant(outcome: &BridgeOutcome) -> &'static str {
    match outcome {
        BridgeOutcome::Ok(_) => "Ok",
        BridgeOutcome::ValidatorFailed { .. } => "ValidatorFailed",
        BridgeOutcome::Rejected(_) => "Rejected",
        BridgeOutcome::RustcRejected { .. } => "RustcRejected",
    }
}

fn panic_message(payload: Box<dyn std::any::Any + Send>) -> String {
    payload
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| {
            payload
                .downcast_ref::<&'static str>()
                .map(|s| s.to_string())
        })
        .unwrap_or_default()
}

#[test]
fn build_overrides_default_is_none_and_empty() {
    let default = BuildOverrides::default();
    assert!(
        default.overflow_checks.is_none(),
        "BuildOverrides::default().overflow_checks must be None"
    );
    assert!(
        default.z_flags.is_empty(),
        "BuildOverrides::default().z_flags must be empty"
    );
}

#[test]
fn override_aware_entry_matches_legacy_entry_on_default_overrides() {
    let path = fixture_path(fixture_names::NO_OP);
    let legacy = try_bridge_fixture_path(&path);
    let with_default = try_bridge_fixture_path_with_overrides(&path, &BuildOverrides::default());
    assert_eq!(
        outcome_discriminant(&legacy),
        outcome_discriminant(&with_default),
        "default BuildOverrides must not alter the BridgeOutcome discriminant on no_op",
    );
}

#[test]
fn override_aware_entry_panics_on_unrecognized_z_flag() {
    let path = fixture_path(fixture_names::NO_OP);
    let overrides = BuildOverrides {
        z_flags: vec!["definitely-not-an-allow-listed-flag".to_string()],
        ..Default::default()
    };
    let result = std::panic::catch_unwind(|| {
        try_bridge_fixture_path_with_overrides(&path, &overrides);
    });
    let payload = result.expect_err("helper must panic on unrecognized -Z override");
    let message = panic_message(payload);
    assert!(
        message.contains("unrecognized -Z override"),
        "panic must surface the documented diagnostic prefix; got: {message:?}",
    );
    assert!(
        message.contains("definitely-not-an-allow-listed-flag"),
        "panic must name the offending value; got: {message:?}",
    );
    assert!(
        message.contains("mir-emit-retag"),
        "panic must surface the allow-list so the user knows the authorized values; got: {message:?}",
    );
}
