//! Device-lifecycle M8 diagnostics (T20): the negative cases reject with the
//! PRECISE diagnostic. Bridged via `try_bridge_fixture_path` (which exposes
//! `BridgeOutcome::Rejected(BridgeError)`, unlike the bridge-unit `with_model`
//! that erases the error to `None`):
//!   - D1 (missing-match): a `#[rseq::driver]` with no device-match attribute.
//!   - D2 (empty-match): a `#[rseq::driver]` whose match attribute is empty.
//!   - D3 (bad user-data): a non-scalar (`[u8; 4]`) user-data field.
//!   - CR-5 (unsupported-bus): a deferred bus family (`acpi`) in the match sugar.
//!
//! Each test bridges an inline snippet; `try_bridge_fixture_path` serializes on
//! its own compiler lock, and these are all rejections (no byte-snapshot), so
//! they coexist in one binary.

#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::PathBuf;

use regseq_driver::bridge::{BridgeError, DeviceLifecycleDiagnostic};
use test_ir_fixtures::{try_bridge_fixture_path, BridgeOutcome};

/// A temp `.rs` snippet file removed on drop.
struct TempSnippet {
    path: PathBuf,
}

impl TempSnippet {
    fn new(label: &str, src: &str) -> TempSnippet {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path =
            std::env::temp_dir().join(format!("lc-diag-{label}-{}-{nanos}.rs", std::process::id()));
        std::fs::write(&path, src).expect("write temp snippet");
        TempSnippet { path }
    }
}

impl Drop for TempSnippet {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

const HEAD: &str = "#![feature(register_tool)]\n#![register_tool(rseq)]\n";

#[test]
fn missing_match_rejects_with_d1() {
    let src = format!("{HEAD}\n#[rseq::driver(name = \"d1\")]\npub struct D1;\n");
    let snip = TempSnippet::new("d1", &src);
    match try_bridge_fixture_path(&snip.path) {
        BridgeOutcome::Rejected(BridgeError::DeviceLifecycleValidation(
            DeviceLifecycleDiagnostic::MissingMatch { driver },
        )) => assert_eq!(driver, "d1"),
        other => panic!("expected D1 MissingMatch, got {other:?}"),
    }
}

#[test]
fn empty_match_rejects_with_d2() {
    let src = format!("{HEAD}\n#[rseq::driver(name = \"d2\", compatible = [])]\npub struct D2;\n");
    let snip = TempSnippet::new("d2", &src);
    match try_bridge_fixture_path(&snip.path) {
        BridgeOutcome::Rejected(BridgeError::DeviceLifecycleValidation(
            DeviceLifecycleDiagnostic::EmptyMatch { driver },
        )) => assert_eq!(driver, "d2"),
        other => panic!("expected D2 EmptyMatch, got {other:?}"),
    }
}

#[test]
fn non_scalar_user_data_rejects_with_d3() {
    let src = format!(
        "{HEAD}\n#[rseq::driver(name = \"d3\", compatible = [\"acme,d3\"])]\n\
         pub struct D3 {{ buf: [u8; 4] }}\n"
    );
    let snip = TempSnippet::new("d3", &src);
    match try_bridge_fixture_path(&snip.path) {
        BridgeOutcome::Rejected(BridgeError::DeviceLifecycleValidation(
            DeviceLifecycleDiagnostic::UserDataNotInitializable { driver, field, .. },
        )) => {
            assert_eq!(driver, "d3");
            assert_eq!(field, "buf");
        }
        other => panic!("expected D3 UserDataNotInitializable, got {other:?}"),
    }
}

#[test]
fn unsupported_bus_sugar_rejects_cr5() {
    let src = format!("{HEAD}\n#[rseq::driver(name = \"d5\", acpi = [])]\npub struct D5;\n");
    let snip = TempSnippet::new("cr5", &src);
    match try_bridge_fixture_path(&snip.path) {
        BridgeOutcome::Rejected(BridgeError::DeviceLifecycleUnsupportedBus { bus }) => {
            assert_eq!(bus, "acpi")
        }
        other => panic!("expected CR-5 DeviceLifecycleUnsupportedBus, got {other:?}"),
    }
}
