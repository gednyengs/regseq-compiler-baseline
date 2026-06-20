//! The peripheral-clock-management NEGATIVE corpus gate (clock construct, T11):
//! every `expect = "reject"` fixture under `fixtures/_clock/` is GREEN as a
//! reject — the pipeline fails at exactly the fixture's `[expected].stage` with
//! its `[expected].diagnostic` substring, covering the full C06 frontend family:
//! the acquire-stage recognition set (`CLK-ERR-*`: missing/malformed/forbidden
//! id, attr-on-non-Clock, unsupported form, duplicate id, spoofed receiver) and
//! the bind-stage binding set (`CLK-L2-*`: missing section, bad mode, missing
//! frequency, managed-without-provider, static-with-provider, orphan section).
//!
//! No emit output is byte-snapshotted, so a shared process is fine; it is
//! nonetheless its OWN binary (it bridges). Bind-stage fixtures carry a legacy
//! `platform.toml`, so `manifest::discover` sets `fixture.platform` directly and
//! `run_fixture` binds it — no per-OS `with_binding` is needed.

#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::{Path, PathBuf};

use codegen_coverage_runner::manifest::{self, Expect};
use codegen_coverage_runner::pipeline;

/// The host `cc` (never invoked — rejects fail before stage 4).
fn cc() -> &'static Path {
    Path::new("cc")
}

fn clock_corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/_clock")
}

#[test]
fn every_clock_reject_fixture_rejects_exactly_as_declared() {
    let fixtures = manifest::discover(&clock_corpus()).expect("discover clock corpus");
    let rejects: Vec<_> = fixtures
        .iter()
        .filter(|f| matches!(f.manifest.expect, Expect::Reject))
        .collect();
    assert!(
        rejects.len() >= 14,
        "expected the full T11 reject corpus (>= 14: 7 recognition + 7 binding), found {}",
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
