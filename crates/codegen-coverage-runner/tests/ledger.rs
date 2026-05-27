//! Ledger aggregation + verdict + drift unit tests (Doc 3 sections 5-6),
//! over synthetic (matrix rows x fixture results): the verdict rule, all
//! three drift directions, and deterministic (sorted) row order.
//!
//! The `rustc_private` anchor is required because the crate under test
//! transitively links the rustc-backed bridge.

#![feature(rustc_private)]

extern crate rustc_driver;

use codegen_coverage_runner::ledger::{self, LedgerError, PhaseVerdict, RowResult, ToolchainInfo};
use codegen_coverage_runner::matrix::{MatrixRow, Status};
use codegen_coverage_runner::pipeline::{FixtureResult, FixtureVerdict, Stage};

fn row(id: &str, status: Status) -> MatrixRow {
    MatrixRow {
        id: id.to_string(),
        status,
    }
}

fn covers(ids: &[&str]) -> Vec<String> {
    ids.iter().map(|s| s.to_string()).collect()
}

fn green(name: &str, ids: &[&str]) -> FixtureResult {
    FixtureResult {
        name: name.to_string(),
        covers: covers(ids),
        verdict: FixtureVerdict::Pass,
    }
}

fn failed(name: &str, ids: &[&str]) -> FixtureResult {
    FixtureResult {
        name: name.to_string(),
        covers: covers(ids),
        verdict: FixtureVerdict::Fail {
            stage: Stage::Emit,
            detail: "stubbed emit".to_string(),
        },
    }
}

fn tc() -> ToolchainInfo {
    ToolchainInfo {
        cc: "cc".to_string(),
        cc_version: "test-cc 1.0".to_string(),
    }
}

#[test]
fn empty_corpus_with_a_covered_row_is_not_pass() {
    let rows = vec![row("IT-1", Status::Covered)];
    let ledger = ledger::build(&rows, &[], tc()).expect("must aggregate");
    assert_eq!(ledger.verdict, PhaseVerdict::NotPass);
    // covered-without-green drift names the uncovered row.
    assert_eq!(ledger.drift.covered_without_green, vec!["IT-1".to_string()]);
    assert_eq!(ledger.rows[0].result, RowResult::Fail);
}

#[test]
fn covers_id_absent_from_matrix_is_a_hard_error() {
    let rows = vec![row("IT-1", Status::Covered)];
    let results = vec![green("invented", &["ZZ-99"])];
    match ledger::build(&rows, &results, tc()) {
        Err(LedgerError::AbsentRowId { fixture, id }) => {
            assert_eq!(fixture, "invented");
            assert_eq!(id, "ZZ-99");
        }
        other => panic!("expected AbsentRowId, got {other:?}"),
    }
}

#[test]
fn green_covering_a_non_covered_row_is_drift_and_not_pass() {
    // No unspecced rows, the covered row is green; the only defect is a
    // green fixture covering a `deferred` row -- isolates direction 3.
    let rows = vec![row("IT-1", Status::Covered), row("TM-13", Status::Deferred)];
    let results = vec![green("f", &["IT-1", "TM-13"])];
    let ledger = ledger::build(&rows, &results, tc()).expect("must aggregate");
    assert_eq!(ledger.verdict, PhaseVerdict::NotPass);
    assert_eq!(ledger.drift.green_without_covered.len(), 1);
    let d = &ledger.drift.green_without_covered[0];
    assert_eq!(d.fixture, "f");
    assert_eq!(d.id, "TM-13");
    assert_eq!(d.status, Status::Deferred);
}

#[test]
fn remaining_unspecced_row_forces_not_pass() {
    // The covered row is green and there is no drift, but an open
    // `unspecced` row remains -- not gated away like deferred/out-of-scope.
    let rows = vec![row("IT-1", Status::Covered), row("RV-1", Status::Unspecced)];
    let results = vec![green("f", &["IT-1"])];
    let ledger = ledger::build(&rows, &results, tc()).expect("must aggregate");
    assert_eq!(ledger.verdict, PhaseVerdict::NotPass);
    // The unspecced row is n/a as a row result (not gated per-row)...
    let rv1 = ledger.rows.iter().find(|r| r.id == "RV-1").unwrap();
    assert_eq!(rv1.result, RowResult::NotApplicable);
}

#[test]
fn deferred_and_out_of_scope_rows_never_gate() {
    // Only non-covered, non-unspecced rows + one green-covered row.
    let rows = vec![
        row("IT-1", Status::Covered),
        row("TM-13", Status::Deferred),
        row("IT-10", Status::OutOfScope),
    ];
    let results = vec![green("f", &["IT-1"])];
    let ledger = ledger::build(&rows, &results, tc()).expect("must aggregate");
    assert_eq!(ledger.verdict, PhaseVerdict::Pass);
}

#[test]
fn all_covered_green_no_unspecced_no_fail_is_pass() {
    let rows = vec![row("IT-1", Status::Covered), row("IT-3", Status::Covered)];
    let results = vec![green("a", &["IT-1"]), green("b", &["IT-3"])];
    let ledger = ledger::build(&rows, &results, tc()).expect("must aggregate");
    assert_eq!(ledger.verdict, PhaseVerdict::Pass);
    assert!(ledger.drift.covered_without_green.is_empty());
    assert!(ledger.drift.green_without_covered.is_empty());
    assert!(ledger.rows.iter().all(|r| r.result == RowResult::Pass));
}

#[test]
fn a_fixture_fail_forces_not_pass() {
    let rows = vec![row("IT-1", Status::Covered)];
    let results = vec![failed("f", &["IT-1"])];
    let ledger = ledger::build(&rows, &results, tc()).expect("must aggregate");
    assert_eq!(ledger.verdict, PhaseVerdict::NotPass);
    assert_eq!(ledger.rows[0].result, RowResult::Fail);
}

#[test]
fn covered_row_with_a_green_and_a_failing_fixture_is_not_covered_without_green() {
    // The row HAS a green fixture, so it is not "covered-without-green"
    // drift; the failing fixture is the separate fixture-FAIL gate. The
    // verdict is still not-pass (a fixture FAILed), but the drift list
    // must stay precise.
    let rows = vec![row("IT-1", Status::Covered)];
    let results = vec![green("ok", &["IT-1"]), failed("bad", &["IT-1"])];
    let ledger = ledger::build(&rows, &results, tc()).expect("must aggregate");
    assert!(
        ledger.drift.covered_without_green.is_empty(),
        "a row with a green fixture is not covered-without-green"
    );
    assert_eq!(ledger.rows[0].result, RowResult::Fail);
    assert_eq!(ledger.verdict, PhaseVerdict::NotPass);
}

#[test]
fn rows_are_sorted_in_the_serialized_json() {
    // Build with rows out of order; the ledger + JSON must be sorted.
    let rows = vec![
        row("TY-18", Status::Unspecced),
        row("AG-1", Status::Unspecced),
        row("IT-1", Status::Covered),
    ];
    let ledger = ledger::build(&rows, &[], tc()).expect("must aggregate");
    let ids: Vec<&str> = ledger.rows.iter().map(|r| r.id.as_str()).collect();
    assert_eq!(ids, vec!["AG-1", "IT-1", "TY-18"]);

    let json = serde_json::to_string(&ledger).expect("serializes");
    let p_ag = json.find("AG-1").unwrap();
    let p_it = json.find("IT-1").unwrap();
    let p_ty = json.find("TY-18").unwrap();
    assert!(p_ag < p_it && p_it < p_ty, "JSON rows must be id-sorted");
}
