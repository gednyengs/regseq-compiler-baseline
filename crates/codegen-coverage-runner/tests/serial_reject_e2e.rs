#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::{Path, PathBuf};

use codegen_coverage_runner::manifest::{self, Expect};
use codegen_coverage_runner::pipeline;

fn corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/_serial")
}

#[test]
fn locked_serial_negative_diagnostics() {
    let fixtures = manifest::discover(&corpus()).expect("discover serial corpus");
    let rejects = fixtures
        .iter()
        .filter(|fixture| matches!(fixture.manifest.expect, Expect::Reject))
        .collect::<Vec<_>>();
    assert_eq!(rejects.len(), 6, "the locked negative corpus is exact");
    for fixture in rejects {
        let result = pipeline::run_fixture(fixture, Path::new("cc"));
        assert!(
            result.is_green(),
            "{} must reject as declared: {:?}",
            fixture.manifest.name,
            result.verdict
        );
    }
}
