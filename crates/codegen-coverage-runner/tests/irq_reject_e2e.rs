//! The device-interrupt L2 NEGATIVE gate (construct #6, the T04 reject
//! witness): the `irq-missing-section-reject` fixture is GREEN as a reject —
//! the pipeline fails at `Stage::Bind` with IRQ-L2-003 (a non-Option
//! `Interrupt` whose platform file lacks `[interrupt.<id>]`, C03), per the
//! fixture's `[expected]` declaration. No emit is reached, so this is robust
//! to interner history; it is nonetheless its OWN binary (it bridges).

#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::{Path, PathBuf};

use codegen_coverage_runner::{manifest, pipeline};

/// The host `cc`, matching the runner's default (never invoked — we fail at Bind).
fn cc() -> &'static Path {
    Path::new("cc")
}

fn irq_corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/_irq")
}

#[test]
fn missing_interrupt_section_rejects_at_bind_with_irq_l2_003() {
    let fixtures = manifest::discover(&irq_corpus()).expect("discover irq corpus");
    let fixture = fixtures
        .iter()
        .find(|f| f.manifest.name == "irq-missing-section-reject")
        .expect("irq-missing-section-reject fixture discovered")
        .with_binding(manifest::OsBinding::Linux)
        .expect("the reject fixture resolves the linux binding");

    let result = pipeline::run_fixture(&fixture, cc());
    assert!(
        result.is_green(),
        "the reject fixture must be GREEN (bind-stage IRQ-L2-003 per its \
         [expected] declaration): {:?}",
        result.verdict
    );
}
