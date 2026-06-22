//! Pin-controller-and-GPIO-configuration coverage aggregation (pinctrl construct,
//! PLAN T24): the PIN-* matrix rows are fully covered by the `_pinctrl` fixtures,
//! and no pinctrl `covers` names a row absent from the matrix (the OI-4 "ledger
//! resolves every covers" check). Mirrors `clock_covers.rs` / `irq_covers.rs`: the
//! `_pinctrl` fixtures live under `fixtures/_pinctrl/` (excluded from the top-level
//! `corpus_e2e` discovery to keep the interner clean); their behavioral pass /
//! reject-stage assertions live in the dedicated `pinctrl_*_e2e` /
//! `pinctrl_reject_e2e` binaries. This test is the COVERS AGGREGATION half: a
//! lightweight, dedicated check over `fixtures/_pinctrl` (it does NOT re-run the
//! six-stage, nor the cross-construct `ledger::build` — whose drift model spans the
//! whole corpus and would mis-flag the interner-isolated pinctrl fixtures; the
//! PIN-* rows are `deferred` in the matrix for the same reason).
//!
//! Note: `PIN-7` is intentionally unminted (reserved) — pinctrl's behavioral
//! surface is narrower than clock's (no managed/static duality), so the real-backend
//! Linux/Zephyr lowering rides PIN-1..PIN-4 + PIN-6 rather than a separate parity
//! row, and the rejects stay at their as-built PIN-8/PIN-9 ids. The matrix
//! therefore carries PIN-1..PIN-6 + PIN-8/PIN-9 (8 rows, no PIN-7).

#![feature(rustc_private)]

extern crate rustc_driver;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use codegen_coverage_runner::{manifest, matrix};

fn pinctrl_corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/_pinctrl")
}

fn matrix_doc() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/discussions/generic-c-backend/phase-1/01-construct-surface-and-matrix.md")
}

#[test]
fn pinctrl_fixtures_cover_every_pin_matrix_row() {
    let rows = matrix::parse(&matrix_doc()).expect("parse the Doc-1 matrix");
    let row_ids: BTreeSet<&str> = rows.iter().map(|r| r.id.as_str()).collect();
    let pin_rows: Vec<&str> = rows
        .iter()
        .map(|r| r.id.as_str())
        .filter(|id| id.starts_with("PIN-"))
        .collect();
    assert!(
        pin_rows.len() >= 8,
        "expected >= 8 PIN-* matrix rows (PIN-1..PIN-6 behavioral/composed + \
         PIN-8/PIN-9 reject families; PIN-7 reserved), found {}",
        pin_rows.len()
    );
    // The PIN-7-reserved decision is load-bearing (the real-backend witnesses ride
    // PIN-1..PIN-4 + PIN-6, not a parity row). Pin the matrix universe to EXACTLY
    // {PIN-1..PIN-6, PIN-8, PIN-9} so accidental minting of PIN-7 (or any stray
    // PIN-* row) is caught here, not silently absorbed by the `>= 8` floor.
    let expected_rows: BTreeSet<&str> = [
        "PIN-1", "PIN-2", "PIN-3", "PIN-4", "PIN-5", "PIN-6", "PIN-8", "PIN-9",
    ]
    .into_iter()
    .collect();
    let actual_rows: BTreeSet<&str> = pin_rows.iter().copied().collect();
    assert_eq!(
        actual_rows, expected_rows,
        "the PIN-* matrix universe must be exactly {{PIN-1..PIN-6, PIN-8, PIN-9}} \
         (PIN-7 is reserved/unminted); got {actual_rows:?}"
    );

    let fixtures = manifest::discover(&pinctrl_corpus()).expect("discover the pinctrl corpus");
    assert!(
        fixtures.len() >= 29,
        "expected the full pinctrl corpus (8 behavioral oracle/composed + 21 rejects), \
         found {}",
        fixtures.len()
    );

    // (1) OI-4 / AbsentRowId: every pinctrl `covers` names a real matrix row, AND
    //     every pinctrl `covers` is a PIN-* row (a pinctrl fixture only covers
    //     pinctrl rows). Together with the completeness check below, this pins the
    //     covered set to exactly the PIN-* matrix universe — it catches both a cover
    //     naming a non-pinctrl row and a cover naming an invented row.
    for f in &fixtures {
        for c in &f.manifest.covers {
            assert!(
                row_ids.contains(c.as_str()),
                "pinctrl fixture {:?} covers row {c:?}, which is ABSENT from the matrix",
                f.manifest.name
            );
            assert!(
                c.starts_with("PIN-"),
                "pinctrl fixture {:?} covers non-pinctrl row {c:?}; a `_pinctrl` fixture \
                 must only cover PIN-* rows",
                f.manifest.name
            );
        }
    }

    // (2) Completeness: every PIN-* matrix row is covered by >= 1 pinctrl fixture.
    let covered: BTreeSet<&str> = fixtures
        .iter()
        .flat_map(|f| f.manifest.covers.iter().map(String::as_str))
        .collect();
    let uncovered: Vec<&str> = pin_rows
        .iter()
        .copied()
        .filter(|id| !covered.contains(id))
        .collect();
    assert!(
        uncovered.is_empty(),
        "pin-controller corpus is not green: uncovered PIN-* matrix rows: {uncovered:?}"
    );
}
