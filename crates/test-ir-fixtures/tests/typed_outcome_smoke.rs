//! Typed-outcome smoke tests for the F0 prerequisite of the
//! bridge-coverage-runner strategy (regseq-driver-63).
//!
//! Verifies that:
//! - `try_bridge_fixture("no_op")` returns `BridgeOutcome::Ok(_)`.
//! - `try_bridge_fixture_path` returns `BridgeOutcome::Ok(_)` on
//!   the same source via the path-based entry point.
//! - `try_bridge_fixture_path` lifts a hyphenated stem (e.g.
//!   `TY-001.rs`) to a rustc-valid crate name and still returns
//!   `BridgeOutcome::Ok(_)`. This is the load-bearing assertion
//!   for the F1 runner's snippet naming convention.
//! - `try_bridge_fixture` panics on an unknown fixture name with
//!   the existing "is not shipped" wording (caller-bug panic).
//! - `bridge_fixture` continues to return `RegseqModel` on the
//!   happy path and panic with the existing wording on unknown
//!   fixture names (Change C backwards-compat).

#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::PathBuf;

use test_ir_fixtures::{
    bridge_fixture, fixture_names, try_bridge_fixture, try_bridge_fixture_path, BridgeOutcome,
};

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join(format!("{name}.rs"))
}

fn unique_tmp_dir(label: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("test-ir-fixtures-{label}-{nanos}"));
    std::fs::create_dir_all(&dir).expect("must create tmp dir");
    dir
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
fn try_bridge_fixture_returns_ok_on_no_op() {
    match try_bridge_fixture(fixture_names::NO_OP) {
        BridgeOutcome::Ok(_) => {}
        other => panic!("expected BridgeOutcome::Ok on no_op, got {other:?}"),
    }
}

#[test]
fn try_bridge_fixture_path_returns_ok_on_no_op_path() {
    let path = fixture_path(fixture_names::NO_OP);
    match try_bridge_fixture_path(&path) {
        BridgeOutcome::Ok(_) => {}
        other => panic!("expected BridgeOutcome::Ok on no_op path, got {other:?}"),
    }
}

#[test]
fn try_bridge_fixture_path_lifts_hyphenated_stem() {
    let src = std::fs::read_to_string(fixture_path(fixture_names::NO_OP))
        .expect("no_op fixture source must be readable");
    let dir = unique_tmp_dir("hyphenated-stem");
    let path = dir.join("TY-001.rs");
    std::fs::write(&path, &src).expect("must write hyphenated-stem snippet");
    let outcome = try_bridge_fixture_path(&path);
    let _ = std::fs::remove_dir_all(&dir);
    match outcome {
        BridgeOutcome::Ok(_) => {}
        other => panic!("expected BridgeOutcome::Ok on hyphenated stem TY-001, got {other:?}"),
    }
}

#[test]
fn try_bridge_fixture_panics_on_unknown_fixture_name() {
    let missing = "__regseq_driver_63_missing_fixture__";
    let result = std::panic::catch_unwind(|| {
        try_bridge_fixture(missing);
    });
    let payload = result.expect_err("try_bridge_fixture must panic on unknown fixture name");
    let message = panic_message(payload);
    assert!(
        message.contains(missing),
        "panic must name the missing fixture; got: {message:?}"
    );
    assert!(
        message.contains("is not shipped"),
        "panic must use the existing 'is not shipped' wording; got: {message:?}"
    );
}

#[test]
fn bridge_fixture_still_returns_model_on_no_op() {
    // Backwards-compat: bridge_fixture preserves its non-panicking
    // happy path under the typed rewire (Change C).
    let _model = bridge_fixture(fixture_names::NO_OP);
}

#[test]
fn bridge_fixture_panics_on_unknown_fixture_with_descriptive_message() {
    let missing = "__regseq_driver_63_legacy_missing__";
    let result = std::panic::catch_unwind(|| {
        bridge_fixture(missing);
    });
    let payload = result.expect_err("bridge_fixture must panic on unknown fixture name");
    let message = panic_message(payload);
    assert!(
        message.contains(missing),
        "panic must name the missing fixture; got: {message:?}"
    );
    assert!(
        message.contains("is not shipped"),
        "panic must use the existing 'is not shipped' wording; got: {message:?}"
    );
}
