//! DMA-management coverage aggregation (dma-management construct, PLAN T24): the
//! DMA-* matrix rows are classified into three HONEST coverage buckets and the
//! `_dma` fixture `covers` cannot drift. Mirrors `clock_covers.rs` /
//! `pinctrl_covers.rs` in shape (the `_dma` fixtures are interner-isolated under
//! `fixtures/_dma/`, excluded from the top-level `corpus_e2e` discovery, so the
//! DMA-* rows are `deferred` in the matrix and this dedicated test is the real
//! coverage gate — NOT `ledger::build`, whose drift model spans the whole corpus).
//!
//! Unlike clock/pinctrl (where EVERY row is fixture-covered), the DMA surface is
//! ASYMMETRIC: the M4/M5 generic-c slice is `candidate-prototype`, so some built
//! rows are witnessed by NON-fixture tests (a gc-shape test, or bridge/L2 unit
//! tests) and some rows are DESIGNED-NOT-BUILT (an honest deferral with no witness
//! this cycle). So this test partitions the DMA-* universe into three EXPLICIT sets
//! (the Codex-recommended option A) and pins the union to the exact matrix universe
//! — so adding a DMA-* row FORCES a conscious classification, and a fixture claiming
//! a TEST-VERIFIED / DESIGNED-NOT-BUILT row (overclaiming coverage) is caught:
//!
//!   * FIXTURE-COVERED — a `_dma` six-stage fixture exercises it (`covers=[…]`);
//!     every such row MUST have ≥1 fixture cover, and every fixture cover MUST be in
//!     this set.
//!   * TEST-VERIFIED — built + witnessed by a NON-fixture test (a gc-shape test or a
//!     bridge/L2 unit test), recorded with its named witness; a `_dma` fixture must
//!     NOT claim to cover it (it is not a six-stage fixture row).
//!   * DESIGNED-NOT-BUILT — an honest deferral with NO witness this cycle (D06/D09/
//!     S08/Out-of-Scope); explicitly listed so it cannot be silently mistaken for
//!     coverage, and a `_dma` fixture must NOT cover it.

#![feature(rustc_private)]

extern crate rustc_driver;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use codegen_coverage_runner::{manifest, matrix};

fn dma_corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/_dma")
}

fn matrix_doc() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/discussions/generic-c-backend/phase-1/01-construct-surface-and-matrix.md")
}

/// FIXTURE-COVERED: a `_dma` six-stage fixture exercises the row. Every row here
/// MUST have ≥1 `_dma` fixture cover, and every `_dma` fixture cover MUST be here.
const FIXTURE_COVERED: &[&str] = &[
    "DMA-1",  // region alloc/free lifecycle
    "DMA-2",  // region A2 addressing (device_addr/_of/element_count)
    "DMA-3",  // region element_access verbs (read/write/update)
    "DMA-4",  // store_order fence
    "DMA-5",  // channel transport + lifecycle
    "DMA-6",  // span endpoint
    "DMA-7",  // completion registration shape
    "DMA-12", // generic-c host DMA model
];

/// TEST-VERIFIED: built + witnessed by a NON-fixture test (recorded with the
/// witness). A `_dma` fixture must NOT claim to cover these.
const TEST_VERIFIED: &[(&str, &str)] = &[
    ("DMA-8", "dma_streaming_gc_shape (map→release→acquire→unmap order)"),
    ("DMA-9", "bridge_dma_recognition (C03 layout-verify reject family; neuter-witnessed)"),
    ("DMA-10", "bridge_dma_recognition (C10 fallibility discarded-Result rejects)"),
    ("DMA-11", "regseq-l2 dma tests (T21 capability absent-in-template reject; neuter-witnessed)"),
];

/// DESIGNED-NOT-BUILT: an honest deferral with NO witness this cycle. Explicitly
/// listed so it cannot be mistaken for coverage; a `_dma` fixture must NOT cover it.
const DESIGNED_NOT_BUILT: &[&str] = &[
    "DMA-13", // completion delivery/firing (D06 — only registration built)
    "DMA-14", // completion_marker_order / regseq_dma_rmb (D09 — only store_order built)
    "DMA-15", // capability ops + concrete-absent cap-op reject (S08 — only the L2 contract built)
    "DMA-16", // Linux/Zephyr lowering + Tier-2 (designed, generic-c only this cycle)
];

#[test]
fn dma_matrix_rows_have_an_honest_coverage_classification() {
    let rows = matrix::parse(&matrix_doc()).expect("parse the Doc-1 matrix");
    let row_ids: BTreeSet<&str> = rows.iter().map(|r| r.id.as_str()).collect();
    let dma_rows: BTreeSet<&str> = rows
        .iter()
        .map(|r| r.id.as_str())
        .filter(|id| id.starts_with("DMA-"))
        .collect();

    // (0) The DMA-* matrix universe is EXACTLY the union of the three buckets. This
    //     is the load-bearing invariant: minting a new DMA-* row (or dropping one)
    //     without classifying it here breaks the test — coverage cannot drift.
    let classified: BTreeSet<&str> = FIXTURE_COVERED
        .iter()
        .copied()
        .chain(TEST_VERIFIED.iter().map(|(id, _)| *id))
        .chain(DESIGNED_NOT_BUILT.iter().copied())
        .collect();
    assert_eq!(
        dma_rows, classified,
        "the DMA-* matrix universe must equal the union of FIXTURE_COVERED + \
         TEST_VERIFIED + DESIGNED_NOT_BUILT (a new/removed DMA-* row must be \
         classified here); matrix={dma_rows:?}, classified={classified:?}"
    );

    // (0b) The three buckets are disjoint (no row is double-classified).
    let n_total = FIXTURE_COVERED.len() + TEST_VERIFIED.len() + DESIGNED_NOT_BUILT.len();
    assert_eq!(
        classified.len(),
        n_total,
        "the three coverage buckets must be DISJOINT (a row is classified once); \
         {} unique vs {n_total} listed",
        classified.len()
    );

    // (0c) Every DMA-* row is `deferred` (the interner-isolation precedent — a
    //      `covered` status would mint `covered_without_green` ledger drift; this
    //      dedicated test is the real gate). Catches an accidental status flip.
    for r in rows.iter().filter(|r| r.id.starts_with("DMA-")) {
        assert_eq!(
            r.status,
            matrix::Status::Deferred,
            "DMA-* row {} must be `deferred` (the `_dma` fixtures are interner-isolated; \
             coverage is gated by this test, not the matrix status column)",
            r.id
        );
    }

    let fixtures = manifest::discover(&dma_corpus()).expect("discover the dma corpus");
    assert!(
        fixtures.len() >= 2,
        "expected the `_dma` six-stage corpus (E1000 coherent + PL011 channel halves), \
         found {}",
        fixtures.len()
    );

    let fixture_covered: BTreeSet<&str> = FIXTURE_COVERED.iter().copied().collect();
    let test_verified: BTreeSet<&str> = TEST_VERIFIED.iter().map(|(id, _)| *id).collect();
    let designed_not_built: BTreeSet<&str> = DESIGNED_NOT_BUILT.iter().copied().collect();

    // (1) Every `_dma` fixture cover names a real matrix row, is a DMA-* row, AND is
    //     in the FIXTURE_COVERED set — a fixture must NOT claim a TEST_VERIFIED or
    //     DESIGNED_NOT_BUILT row (that would overclaim six-stage coverage).
    for f in &fixtures {
        for c in &f.manifest.covers {
            assert!(
                row_ids.contains(c.as_str()),
                "dma fixture {:?} covers row {c:?}, which is ABSENT from the matrix",
                f.manifest.name
            );
            assert!(
                c.starts_with("DMA-"),
                "dma fixture {:?} covers non-dma row {c:?}; a `_dma` fixture must only \
                 cover DMA-* rows",
                f.manifest.name
            );
            assert!(
                fixture_covered.contains(c.as_str()),
                "dma fixture {:?} covers row {c:?}, which is NOT a FIXTURE-COVERED row \
                 (it is TEST-VERIFIED or DESIGNED-NOT-BUILT — a `_dma` fixture must not \
                 overclaim coverage of it)",
                f.manifest.name
            );
        }
    }

    // (2) Completeness: every FIXTURE_COVERED row is covered by ≥1 `_dma` fixture.
    let covered: BTreeSet<&str> = fixtures
        .iter()
        .flat_map(|f| f.manifest.covers.iter().map(String::as_str))
        .collect();
    let uncovered: Vec<&str> = fixture_covered
        .iter()
        .copied()
        .filter(|id| !covered.contains(id))
        .collect();
    assert!(
        uncovered.is_empty(),
        "dma corpus is not green: FIXTURE-COVERED rows with NO `_dma` fixture cover: \
         {uncovered:?}"
    );

    // (3) No `_dma` fixture covers a TEST_VERIFIED or DESIGNED_NOT_BUILT row (the
    //     converse of (1), stated over the actual covered set for clarity).
    for id in covered.iter() {
        assert!(
            !test_verified.contains(id) && !designed_not_built.contains(id),
            "a `_dma` fixture covers {id:?}, which is TEST-VERIFIED / DESIGNED-NOT-BUILT \
             (not a six-stage fixture row)"
        );
    }

    // (4) Every TEST_VERIFIED row carries an explicit, non-empty named witness (so
    //     a built-but-test-verified row cannot silently lose its recorded witness).
    for (id, witness) in TEST_VERIFIED {
        assert!(
            !witness.trim().is_empty(),
            "TEST-VERIFIED row {id} must name its witness test"
        );
    }
}
