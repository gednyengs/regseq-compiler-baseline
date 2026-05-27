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
