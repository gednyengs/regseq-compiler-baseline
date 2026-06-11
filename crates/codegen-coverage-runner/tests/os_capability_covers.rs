//! OS-capability-disposition covers aggregation (T08, R11/I08): the `OS-1`
//! matrix row exists, the dedicated absent-slot witness fixture covers it,
//! and every composed `covers` resolves to a real matrix row.
//!
//! The composed fixtures live under `fixtures/_composed/` — OUTSIDE Mode-A
//! `discover` (interner-isolation discipline), so the row stays `deferred`
//! and this test is the REAL coverage gate (the `lifecycle_covers.rs`
//! precedent). It is a manifest/matrix AGGREGATION check ONLY: it never
//! invokes `ledger::build` (feeding a green composed fixture against a
//! `deferred` row into the ledger would itself mint `green_without_covered`
//! drift), and it never re-runs the snapshot pipeline — the fixture's
//! six artifacts are gated by its sole-binary `zephyr_absentslot_e2e`.

#![feature(rustc_private)]

extern crate rustc_driver;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use codegen_coverage_runner::{manifest, matrix};

fn composed_corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/_composed")
}

fn matrix_doc() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/discussions/generic-c-backend/phase-1/01-construct-surface-and-matrix.md")
}

#[test]
fn composed_fixtures_cover_the_os_capability_rows() {
    let rows = matrix::parse(&matrix_doc()).expect("parse the Doc-1 matrix");
    let row_ids: BTreeSet<&str> = rows.iter().map(|r| r.id.as_str()).collect();
    assert!(row_ids.contains("OS-1"), "the OS-1 matrix row must exist");

    let fixtures = manifest::discover(&composed_corpus()).expect("discover the composed corpus");

    // (1) Every composed `covers` names a real matrix row (the OI-4 /
    //     AbsentRowId discipline, extended to `_composed`).
    for f in &fixtures {
        for c in &f.manifest.covers {
            assert!(
                row_ids.contains(c.as_str()),
                "composed fixture {:?} covers row {c:?}, which is ABSENT from the matrix",
                f.manifest.name
            );
        }
    }

    // (2) The absent-slot mechanism row is covered by its dedicated witness.
    let os1_coverers: Vec<&str> = fixtures
        .iter()
        .filter(|f| f.manifest.covers.iter().any(|c| c == "OS-1"))
        .map(|f| f.manifest.name.as_str())
        .collect();
    assert!(
        os1_coverers.contains(&"pl011-absentslot-zephyr"),
        "OS-1 must be covered by pl011-absentslot-zephyr; coverers: {os1_coverers:?}"
    );
}
