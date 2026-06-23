//! The deferred-work-item-scheduling NEGATIVE corpus gate (M2, PLAN T12/T13):
//! every `expect = "reject"` fixture under `fixtures/_deferred_work/` is GREEN as
//! a reject — the pipeline fails at exactly the fixture's `[expected].stage`
//! (`acquire`, the bridge) with its `[expected].diagnostic` substring, covering
//! the full C06 BUILT frontend family:
//!   * handler-signature: bad return + bad receiver (`DWS-ERR-HANDLER-SIGNATURE`),
//!   * required `Work` with no handler (`DWS-ERR-REQUIRED-WORK-NO-HANDLER`),
//!   * two handlers on one `Work` (`DWS-ERR-DUPLICATE-HANDLER`, single-resource),
//!   * bare handler with zero `Work` (`DWS-ERR-BARE-WH-ZERO-WORK`),
//!   * submit spoof: callee not on `Work` + alias receiver (`DWS-ERR-SPOOFED-RECEIVER`),
//!   * unsupported field forms `Option<Work>`/`[Work;N]`/`Vec<Work>`/`&Work`
//!     (`DWS-ERR-UNSUPPORTED-FORM`),
//!   * the explicit `#[rseq::work_handler(field="…")]` arg (SF-1) + multi-`Work`
//!     (SF-2) — both rejected via the locked built diagnostics (no design-in id).
//!
//! There is **no L2** for this construct (0 L1 / 0 L2 properties, GRILL D02), so
//! the family is wholly bridge-stage (`acquire`); there is no `DWS-L2-*` half.
//! No emit output is byte-snapshotted, so a shared process is fine; it is
//! nonetheless its OWN binary (it bridges).

#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::{Path, PathBuf};

use codegen_coverage_runner::manifest::{self, Expect};
use codegen_coverage_runner::pipeline;

/// The host `cc` (never invoked — rejects fail before stage 4).
fn cc() -> &'static Path {
    Path::new("cc")
}

fn dwork_corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/_deferred_work")
}

#[test]
fn every_deferred_work_reject_fixture_rejects_exactly_as_declared() {
    let fixtures = manifest::discover(&dwork_corpus()).expect("discover deferred-work corpus");
    let rejects: Vec<_> = fixtures
        .iter()
        .filter(|f| matches!(f.manifest.expect, Expect::Reject))
        .collect();
    // Pin the EXACT count (not `>=`): the 15 M2 built-C06 rejects + the M3 T14
    // unmarked-`Work` recognition negative. An exact count catches a fixture
    // accidentally flipped from `reject` to `pass` (which a `>=` lower bound would
    // silently tolerate).
    assert_eq!(
        rejects.len(),
        16,
        "expected exactly 16 deferred-work reject fixtures (15 M2 C06 + 1 M3 \
         unmarked-Work), found {}",
        rejects.len()
    );
    assert!(
        rejects
            .iter()
            .any(|f| f.manifest.name == "dws-unmarked-work-reject"),
        "the M3 T14 unmarked-`Work` recognition negative must be in the reject corpus"
    );
    for fixture in rejects {
        let result = pipeline::run_fixture(fixture, cc());
        assert!(
            result.is_green(),
            "reject fixture {:?} must fail exactly as declared: {:?}",
            fixture.manifest.name,
            result.verdict
        );
    }
}
