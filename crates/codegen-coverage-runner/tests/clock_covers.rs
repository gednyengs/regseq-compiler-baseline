//! Peripheral-clock coverage aggregation (construct #7, PLAN T18): the CLK-*
//! matrix rows are fully covered by the `_clock` fixtures, and no clock `covers`
//! names a row absent from the matrix (the OI-4 "ledger resolves every covers"
//! check). Mirrors `irq_covers.rs` / `lifecycle_covers.rs`: the `_clock`
//! fixtures live under `fixtures/_clock/` (excluded from the top-level
//! `corpus_e2e` discovery to keep the interner clean — D17); their behavioral
//! pass / reject-stage assertions live in the dedicated `clock_*_e2e` /
//! `clock_reject_e2e` binaries. This test is the COVERS AGGREGATION half: a
//! lightweight, dedicated check over `fixtures/_clock` (it does NOT re-run the
//! six-stage, nor does it run the cross-construct `ledger::build` — whose drift
//! model spans the whole corpus and would mis-flag the interner-isolated clock
//! fixtures; the CLK-* rows are `deferred` in the matrix for the same reason).

#![feature(rustc_private)]

extern crate rustc_driver;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use codegen_coverage_runner::{manifest, matrix};

fn clock_corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/_clock")
}

fn matrix_doc() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/discussions/generic-c-backend/phase-1/01-construct-surface-and-matrix.md")
}

#[test]
fn clock_fixtures_cover_every_clk_matrix_row() {
    let rows = matrix::parse(&matrix_doc()).expect("parse the Doc-1 matrix");
    let row_ids: BTreeSet<&str> = rows.iter().map(|r| r.id.as_str()).collect();
    let clk_rows: Vec<&str> = rows
        .iter()
        .map(|r| r.id.as_str())
        .filter(|id| id.starts_with("CLK-"))
        .collect();
    assert!(
        clk_rows.len() >= 9,
        "expected >= 9 CLK-* matrix rows (CLK-1..CLK-9: 7 behavioral + 2 reject \
         families), found {}",
        clk_rows.len()
    );

    let fixtures = manifest::discover(&clock_corpus()).expect("discover the clock corpus");
    assert!(
        fixtures.len() >= 17,
        "expected the full clock corpus (3 behavioral oracle/composed + 14 rejects), \
         found {}",
        fixtures.len()
    );

    // (1) OI-4 / AbsentRowId: every clock `covers` names a real matrix row, AND
    //     every clock `covers` is a CLK-* row (a clock fixture only covers clock
    //     rows). Together with the completeness check below, this pins the
    //     covered set to exactly the CLK-* matrix universe — it catches both a
    //     cover naming a non-clock row and a cover naming an invented row.
    for f in &fixtures {
        for c in &f.manifest.covers {
            assert!(
                row_ids.contains(c.as_str()),
                "clock fixture {:?} covers row {c:?}, which is ABSENT from the matrix",
                f.manifest.name
            );
            assert!(
                c.starts_with("CLK-"),
                "clock fixture {:?} covers non-clock row {c:?}; a `_clock` fixture \
                 must only cover CLK-* rows",
                f.manifest.name
            );
        }
    }

    // (2) Completeness: every CLK-* matrix row is covered by >= 1 clock fixture.
    let covered: BTreeSet<&str> = fixtures
        .iter()
        .flat_map(|f| f.manifest.covers.iter().map(String::as_str))
        .collect();
    let uncovered: Vec<&str> = clk_rows
        .iter()
        .copied()
        .filter(|id| !covered.contains(id))
        .collect();
    assert!(
        uncovered.is_empty(),
        "peripheral-clock corpus is not green: uncovered CLK-* matrix rows: {uncovered:?}"
    );
}
