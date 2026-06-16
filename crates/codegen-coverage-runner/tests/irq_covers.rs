//! Device-interrupt coverage aggregation (construct #6): the IRQ-* matrix
//! rows are fully covered by the `_irq` fixtures, and no irq `covers` names a
//! row absent from the matrix (the OI-4 "ledger resolves every covers"
//! check). Mirrors `lifecycle_covers.rs`: the `_irq` fixtures live under
//! `fixtures/_irq/` (excluded from the top-level `corpus_e2e` discovery to
//! keep the interner clean — D17); their six-stage pass is asserted in the
//! dedicated `irq_*_e2e` binaries. This test is the COVERS AGGREGATION half.

#![feature(rustc_private)]

extern crate rustc_driver;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use codegen_coverage_runner::{manifest, matrix};

fn irq_corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/_irq")
}

fn matrix_doc() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/discussions/generic-c-backend/phase-1/01-construct-surface-and-matrix.md")
}

#[test]
fn irq_fixtures_cover_every_irq_matrix_row() {
    let rows = matrix::parse(&matrix_doc()).expect("parse the Doc-1 matrix");
    let row_ids: BTreeSet<&str> = rows.iter().map(|r| r.id.as_str()).collect();
    let irq_rows: Vec<&str> = rows
        .iter()
        .map(|r| r.id.as_str())
        .filter(|id| id.starts_with("IRQ-"))
        .collect();
    assert!(
        irq_rows.len() >= 13,
        "expected >= 13 IRQ-* matrix rows (IRQ-1..IRQ-13), found {}",
        irq_rows.len()
    );

    let fixtures = manifest::discover(&irq_corpus()).expect("discover the irq corpus");
    assert!(
        fixtures.len() >= 14,
        "expected the full irq corpus (3 pass/reject originals + 11 T08 rejects), found {}",
        fixtures.len()
    );

    // (1) OI-4 / AbsentRowId: every irq `covers` names a real matrix row.
    for f in &fixtures {
        for c in &f.manifest.covers {
            assert!(
                row_ids.contains(c.as_str()),
                "irq fixture {:?} covers row {c:?}, which is ABSENT from the matrix",
                f.manifest.name
            );
        }
    }

    // (2) Completeness: every IRQ-* matrix row is covered by >= 1 irq fixture.
    let covered: BTreeSet<&str> = fixtures
        .iter()
        .flat_map(|f| f.manifest.covers.iter().map(String::as_str))
        .collect();
    for row in irq_rows {
        assert!(
            covered.contains(row),
            "IRQ matrix row {row:?} is covered by NO irq fixture"
        );
    }
}
