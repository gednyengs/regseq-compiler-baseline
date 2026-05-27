//! Stage-1 smoke test (Change J): a trivial `#![no_std]` snippet bridges
//! through `try_bridge_fixture_path` to `BridgeOutcome::Ok(model)`,
//! proving the stage-1 acquire path is wired. The bridge front-end is
//! feature-complete (gate `1a6a996`), so a valid snippet validates.
//!
//! `#![feature(rustc_private)]` + `extern crate rustc_driver;` link the
//! rustc-backed bridge into the test binary (sysroot rpath from
//! `build.rs`), matching `test-ir-fixtures/tests/no_op_smoke.rs`.

#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::PathBuf;

use test_ir_fixtures::{try_bridge_fixture_path, BridgeOutcome};

/// A temp `.rs` file removed on drop.
struct TempSnippet {
    path: PathBuf,
}

impl TempSnippet {
    fn new(label: &str, src: &str) -> TempSnippet {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path = std::env::temp_dir().join(format!(
            "ccr-bridge-{label}-{}-{nanos}.rs",
            std::process::id()
        ));
        std::fs::write(&path, src).expect("must write temp snippet");
        TempSnippet { path }
    }
}

impl Drop for TempSnippet {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

#[test]
fn trivial_snippet_bridges_to_ok_model() {
    let snippet = TempSnippet::new("no_op", "#![no_std]\npub fn no_op() {}\n");
    match try_bridge_fixture_path(&snippet.path) {
        BridgeOutcome::Ok(model) => {
            assert!(
                !model.instances.is_empty(),
                "bridged model must carry at least the no_op instance"
            );
        }
        other => panic!("expected BridgeOutcome::Ok, got {other:?}"),
    }
}
