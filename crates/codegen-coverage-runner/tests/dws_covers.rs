//! Deferred-work-item-scheduling coverage aggregation (construct #8, PLAN T18):
//! the DWS-* matrix rows are fully covered by the `_deferred_work` fixtures, and no
//! deferred-work `covers` names a row absent from the matrix (the OI-4 "ledger
//! resolves every covers" check). Mirrors `clock_covers.rs` / `irq_covers.rs`: the
//! `_deferred_work` fixtures live under `fixtures/_deferred_work/` (excluded from
//! the top-level `corpus_e2e` discovery — `discover` is non-recursive and that dir
//! has no manifest — to keep the interner clean); their behavioral pass /
//! reject-stage / recognition assertions live in the dedicated `dws_reset_e2e` /
//! `dws_nosubmit_e2e` / `dws_alias_recognition_e2e` / `dws_reject_e2e` binaries.
//! This test is the COVERS AGGREGATION half: a lightweight, dedicated check over
//! `fixtures/_deferred_work` (it does NOT re-run the six-stage, nor the
//! cross-construct `ledger::build` — whose drift model spans the whole corpus and
//! would mis-flag the interner-isolated deferred-work fixtures; the DWS-* rows are
//! `deferred` in the matrix for the same reason).

#![feature(rustc_private)]

extern crate rustc_driver;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use codegen_coverage_runner::{manifest, matrix};

fn dwork_corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/_deferred_work")
}

fn matrix_doc() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/discussions/generic-c-backend/phase-1/01-construct-surface-and-matrix.md")
}

#[test]
fn deferred_work_fixtures_cover_every_dws_matrix_row() {
    let rows = matrix::parse(&matrix_doc()).expect("parse the Doc-1 matrix");
    let row_ids: BTreeSet<&str> = rows.iter().map(|r| r.id.as_str()).collect();
    let dws_rows: Vec<&str> = rows
        .iter()
        .map(|r| r.id.as_str())
        .filter(|id| id.starts_with("DWS-"))
        .collect();
    assert!(
        dws_rows.len() >= 6,
        "expected >= 6 DWS-* matrix rows (DWS-1..DWS-3 behavioral oracle, DWS-4 \
         nosubmit, DWS-5 recognition, DWS-6 reject family), found {}",
        dws_rows.len()
    );

    let fixtures = manifest::discover(&dwork_corpus()).expect("discover the deferred-work corpus");
    assert!(
        fixtures.len() >= 19,
        "expected the full deferred-work corpus (reset + nosubmit + alias + 16 \
         rejects), found {}",
        fixtures.len()
    );

    // (1) OI-4 / AbsentRowId: every deferred-work `covers` names a real matrix row,
    //     AND every deferred-work `covers` is a DWS-* row (a deferred-work fixture
    //     only covers deferred-work rows). Together with the completeness check
    //     below, this pins the covered set to exactly the DWS-* matrix universe — it
    //     catches both a cover naming a non-DWS row and a cover naming an invented
    //     row.
    for f in &fixtures {
        for c in &f.manifest.covers {
            assert!(
                row_ids.contains(c.as_str()),
                "deferred-work fixture {:?} covers row {c:?}, which is ABSENT from the matrix",
                f.manifest.name
            );
            assert!(
                c.starts_with("DWS-"),
                "deferred-work fixture {:?} covers non-DWS row {c:?}; a `_deferred_work` \
                 fixture must only cover DWS-* rows",
                f.manifest.name
            );
        }
    }

    // (2) Completeness: every DWS-* matrix row is covered by >= 1 deferred-work fixture.
    let covered: BTreeSet<&str> = fixtures
        .iter()
        .flat_map(|f| f.manifest.covers.iter().map(String::as_str))
        .collect();
    let uncovered: Vec<&str> = dws_rows
        .iter()
        .copied()
        .filter(|id| !covered.contains(id))
        .collect();
    assert!(
        uncovered.is_empty(),
        "deferred-work corpus is not green: uncovered DWS-* matrix rows: {uncovered:?}"
    );
}
