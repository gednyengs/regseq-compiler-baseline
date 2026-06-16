//! The device-interrupt NEGATIVE corpus gate (construct #6, T04 + T08): every
//! `expect = "reject"` fixture under `fixtures/_irq/` is GREEN as a reject —
//! the pipeline fails at exactly the fixture's `[expected].stage` with its
//! `[expected].diagnostic` substring, covering the full C06 frontend family
//! (acquire-stage `IRQ-ERR-*`), the bind-stage `IRQ-L2-003`
//! missing-section check, and the two emit-stage I05 guards (Option/array
//! lowering reached; reachable `DeferWork` naming
//! deferred-work-item-scheduling). No emit output is byte-snapshotted, so a
//! shared process is fine; it is nonetheless its OWN binary (it bridges).

#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::{Path, PathBuf};

use codegen_coverage_runner::manifest::{self, Expect};
use codegen_coverage_runner::pipeline;

/// The host `cc`, matching the runner's default (never invoked — rejects
/// fail before stage 4).
fn cc() -> &'static Path {
    Path::new("cc")
}

fn irq_corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/_irq")
}

#[test]
fn every_irq_reject_fixture_rejects_exactly_as_declared() {
    let fixtures = manifest::discover(&irq_corpus()).expect("discover irq corpus");
    let rejects: Vec<_> = fixtures
        .iter()
        .filter(|f| matches!(f.manifest.expect, Expect::Reject))
        .collect();
    assert!(
        rejects.len() >= 11,
        "expected the full T04+T08 reject corpus (>= 11), found {}",
        rejects.len()
    );
    for fixture in rejects {
        // A reject fixture carrying a per-OS platform file binds it (the
        // bind-stage checks need the join); a frontend/emit reject without
        // one runs unbound.
        let bound;
        let fixture = if fixture.dir.join("platform.linux.toml").exists() {
            bound = fixture
                .with_binding(manifest::OsBinding::Linux)
                .expect("reject fixture resolves its linux binding");
            &bound
        } else {
            fixture
        };
        let result = pipeline::run_fixture(fixture, cc());
        assert!(
            result.is_green(),
            "reject fixture {:?} must fail exactly as declared: {:?}",
            fixture.manifest.name,
            result.verdict
        );
    }
}
