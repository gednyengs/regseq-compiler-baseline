//! The serial device-class NEGATIVE corpus gate (construct #7, PLAN T04): every
//! `expect = "reject"` fixture under `fixtures/_serial/` is GREEN as a reject —
//! the pipeline fails at exactly the fixture's `[expected].stage` (acquire) with
//! its `[expected].diagnostic` substring, covering the full T04 reject matrix:
//!   * `SER-ERR-DYNAMIC-IRQ-SET` — a dynamic author call to `irq_source_set`
//!     (R07/D03);
//!   * `SER-ERR-DEFERRED-MEMBER` — an override of a v1-deferred member inside an
//!     in-scope trait (`SerialRuntimeLineConfig::{line_format_readback,break_set,
//!     flow_auto_set}`, `SerialQueueControl::fifo_purge_hw`) (R10/C03/C05);
//!   * `SER-ERR-DEFERRED-TRAIT` — an impl of a deferred companion trait
//!     (`SerialModemControl`/`SerialConsole`/`SerialBulkTransfer`) (R10/C06-C08).
//! No emit output is snapshotted (rejects fail before emit), so a shared process
//! is fine; it is nonetheless its OWN binary (it bridges).

#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::{Path, PathBuf};

use codegen_coverage_runner::manifest::{self, Expect};
use codegen_coverage_runner::pipeline;

/// The host `cc`, matching the runner's default (never invoked — rejects fail
/// at acquire, before stage 4).
fn cc() -> &'static Path {
    Path::new("cc")
}

fn serial_corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/_serial")
}

#[test]
fn every_serial_reject_fixture_rejects_exactly_as_declared() {
    let fixtures = manifest::discover(&serial_corpus()).expect("discover serial corpus");
    let rejects: Vec<_> = fixtures
        .iter()
        .filter(|f| matches!(f.manifest.expect, Expect::Reject))
        .collect();
    assert!(
        rejects.len() >= 20,
        "expected the full reject matrix (>= 20: T04 = 1 direct dynamic-call + 2 evasion \
         (method-to-fn-pointer, capturing closure) + 4 deferred-member + 3 deferred-trait; \
         O2-hardening = 2 named-const mismatch (cause, rxerror) + 8 body-provenance (raw-literal, \
         device-leak, helper-delegation, mutable-borrow, struct-literal, interior-field-mutation, \
         projection-call-dest, nested-field-mutation)), found {}",
        rejects.len()
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
