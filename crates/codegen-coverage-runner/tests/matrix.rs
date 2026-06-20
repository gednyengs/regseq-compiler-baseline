//! Doc-1 matrix parser unit tests (Change D): parse a small fixed table
//! into (id, status) rows across the three observed ID forms, reject an
//! unknown status string, and lock the GFM-code-span split that protects
//! the one Doc-1 row carrying a raw `|` inside an inline-code cell.
//!
//! The `rustc_private` anchor is required because the crate under test
//! transitively links the rustc-backed bridge.

#![feature(rustc_private)]

extern crate rustc_driver;

use codegen_coverage_runner::matrix::{self, MatrixError, Status};

#[test]
fn parses_plain_suffixed_and_range_ids() {
    let md = "\
| id | construct | C lowering | status | spec ref | notes |\n\
| -- | --------- | ---------- | ------ | -------- | ----- |\n\
| IT-1 | Function | drives emission | covered | gc 7 | x |\n\
| IN-5b | coroutine flavors | -- | out-of-scope | spec 15.1 | y |\n\
| BO-1..5 | Add..Rem | C + - * / % | unspecced | spec 21 | z |\n\
| TM-13 | drop glue | synthetic call | deferred | gc 7 | w |\n\
not a table line\n\
| not-a-row | filler | filler | covered | filler | filler |\n";
    let rows = matrix::parse_str(md).expect("table must parse");

    // Sorted by id; header/separator/non-id rows skipped.
    let ids: Vec<&str> = rows.iter().map(|r| r.id.as_str()).collect();
    assert_eq!(ids, vec!["BO-1..5", "IN-5b", "IT-1", "TM-13"]);

    let status_of = |id: &str| rows.iter().find(|r| r.id == id).unwrap().status;
    assert_eq!(status_of("IT-1"), Status::Covered);
    assert_eq!(status_of("IN-5b"), Status::OutOfScope);
    assert_eq!(status_of("BO-1..5"), Status::Unspecced);
    assert_eq!(status_of("TM-13"), Status::Deferred);
}

#[test]
fn handles_raw_pipe_inside_inline_code_span() {
    // The C-lowering cell holds `C `^ & |`` -- a raw `|` inside backticks.
    // A naive split would misread the status column; the code-span-aware
    // splitter must still read `unspecced`.
    let md = "\
| id | construct | C lowering | status | spec ref | notes |\n\
| -- | --------- | ---------- | ------ | -------- | ----- |\n\
| BO-12..14 | `BinOp::{BitXor,BitAnd,BitOr}` | C `^ & |` | unspecced | spec 21 | |\n";
    let rows = matrix::parse_str(md).expect("code-span row must parse");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id, "BO-12..14");
    assert_eq!(rows[0].status, Status::Unspecced);
}

#[test]
fn rejects_non_legend_status() {
    let md = "\
| id | construct | C lowering | status | spec ref | notes |\n\
| -- | --------- | ---------- | ------ | -------- | ----- |\n\
| IT-1 | Function | drives emission | bogus | gc 7 | x |\n";
    match matrix::parse_str(md) {
        Err(MatrixError::NonLegendStatus { id, status }) => {
            assert_eq!(id, "IT-1");
            assert_eq!(status, "bogus");
        }
        other => panic!("expected NonLegendStatus, got {other:?}"),
    }
}

#[test]
fn committed_matrix_parses_with_device_lifecycle_rows() {
    // The whole committed Doc-1 matrix must still parse after the §14
    // device-lifecycle rows landed (no NonLegendStatus / DuplicateId), and the
    // 13 numeric `LC-<n>` rows must be present and `deferred` at authoring time
    // (T22a/T23/T24 flip them to `covered` as their fixtures land).
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(
        "../../docs/discussions/generic-c-backend/phase-1/01-construct-surface-and-matrix.md",
    );
    let rows = matrix::parse(&path).expect("the committed Doc-1 matrix must parse");
    let lc: std::collections::BTreeSet<&str> = rows
        .iter()
        .map(|r| r.id.as_str())
        .filter(|id| id.starts_with("LC-"))
        .collect();
    for n in 1..=13 {
        let id = format!("LC-{n}");
        assert!(lc.contains(id.as_str()), "matrix must contain {id}");
    }
    assert_eq!(lc.len(), 13, "exactly 13 device-lifecycle rows");
    for r in rows.iter().filter(|r| r.id.starts_with("LC-")) {
        assert_eq!(
            r.status,
            Status::Deferred,
            "{} is `deferred` until its milestone fixture lands",
            r.id
        );
    }
}

#[test]
fn committed_matrix_carries_the_os_capability_row_deferred() {
    // os-capability-disposition T08: the OS-1 absent-slot row is present and
    // `deferred` (the LC precedent — `_composed` is outside Mode-A discovery,
    // so `covered` would mint covered_without_green drift; the real gate is
    // tests/os_capability_covers.rs + the sole-binary absentslot e2e).
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(
        "../../docs/discussions/generic-c-backend/phase-1/01-construct-surface-and-matrix.md",
    );
    let rows = matrix::parse(&path).expect("the committed Doc-1 matrix must parse");
    let os1 = rows
        .iter()
        .find(|r| r.id == "OS-1")
        .expect("matrix must contain OS-1");
    assert_eq!(
        os1.status,
        Status::Deferred,
        "OS-1 stays `deferred`; its coverage gate is the dedicated covers test"
    );
}

#[test]
fn committed_matrix_carries_the_peripheral_clock_rows_deferred() {
    // peripheral-clock-management T18: the CLK-1..CLK-9 rows (7 behavioral + 2
    // reject families) are present and `deferred` (the LC/OS/IRQ precedent —
    // `_clock` is outside Mode-A discovery, so `covered` would mint
    // covered_without_green drift; the real gate is tests/clock_covers.rs + the
    // sole-binary clock e2es).
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(
        "../../docs/discussions/generic-c-backend/phase-1/01-construct-surface-and-matrix.md",
    );
    let rows = matrix::parse(&path).expect("the committed Doc-1 matrix must parse");
    let clk: std::collections::BTreeSet<&str> = rows
        .iter()
        .map(|r| r.id.as_str())
        .filter(|id| id.starts_with("CLK-"))
        .collect();
    for n in 1..=9 {
        let id = format!("CLK-{n}");
        assert!(clk.contains(id.as_str()), "matrix must contain {id}");
    }
    assert_eq!(clk.len(), 9, "exactly 9 peripheral-clock rows");
    for r in rows.iter().filter(|r| r.id.starts_with("CLK-")) {
        assert_eq!(
            r.status,
            Status::Deferred,
            "{} is `deferred`; its coverage gate is the dedicated covers test",
            r.id
        );
    }
}

#[test]
fn rejects_duplicate_ids() {
    let md = "\
| id | construct | C lowering | status | spec ref | notes |\n\
| -- | --------- | ---------- | ------ | -------- | ----- |\n\
| IT-1 | Function | a | covered | gc 7 | x |\n\
| IT-1 | Function | b | covered | gc 7 | y |\n";
    assert!(matches!(
        matrix::parse_str(md),
        Err(MatrixError::DuplicateId { .. })
    ));
}
