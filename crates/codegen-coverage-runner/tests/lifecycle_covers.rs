//! Device-lifecycle M8 coverage ledger (T26): the LC-* matrix rows are fully
//! covered by the lifecycle fixtures, and no lifecycle `covers` names a row
//! absent from the matrix (the OI-4 "ledger resolves every covers" check).
//!
//! The lifecycle fixtures live under `fixtures/_lifecycle/` (excluded from the
//! top-level `corpus_e2e` discovery to keep the interner clean — D17); their
//! six-stage dual-oracle pass is asserted per-fixture in the dedicated
//! `lifecycle_*_e2e` binaries. This test is the COVERS AGGREGATION half: a
//! lightweight, dedicated check over `fixtures/_lifecycle` (it does NOT re-run
//! the six-stage, nor does it run the cross-construct `ledger::build` — whose
//! drift model spans the whole corpus and would mis-flag the interner-isolated
//! lifecycle fixtures).

#![feature(rustc_private)]

extern crate rustc_driver;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use codegen_coverage_runner::{manifest, matrix};

fn lifecycle_corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/_lifecycle")
}

fn matrix_doc() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/discussions/generic-c-backend/phase-1/01-construct-surface-and-matrix.md")
}

#[test]
fn lifecycle_fixtures_cover_every_lc_matrix_row() {
    let rows = matrix::parse(&matrix_doc()).expect("parse the Doc-1 matrix");
    let row_ids: BTreeSet<&str> = rows.iter().map(|r| r.id.as_str()).collect();
    let lc_rows: Vec<&str> = rows
        .iter()
        .map(|r| r.id.as_str())
        .filter(|id| id.starts_with("LC-"))
        .collect();
    assert!(
        lc_rows.len() >= 13,
        "expected >= 13 LC-* matrix rows (LC-1..LC-13), found {}",
        lc_rows.len()
    );

    let fixtures = manifest::discover(&lifecycle_corpus()).expect("discover the lifecycle corpus");

    // (1) OI-4 / AbsentRowId: every lifecycle `covers` names a real matrix row.
    for f in &fixtures {
        for c in &f.manifest.covers {
            assert!(
                row_ids.contains(c.as_str()),
                "lifecycle fixture {:?} covers row {c:?}, which is ABSENT from the matrix",
                f.manifest.name
            );
        }
    }

    // (2) Completeness: every LC-* matrix row is covered by >= 1 lifecycle fixture.
    let covered: BTreeSet<&str> = fixtures
        .iter()
        .flat_map(|f| f.manifest.covers.iter().map(String::as_str))
        .collect();
    let uncovered: Vec<&str> = lc_rows
        .iter()
        .copied()
        .filter(|id| !covered.contains(id))
        .collect();
    assert!(
        uncovered.is_empty(),
        "device-lifecycle corpus is not green: uncovered LC-* matrix rows: {uncovered:?}"
    );
}
