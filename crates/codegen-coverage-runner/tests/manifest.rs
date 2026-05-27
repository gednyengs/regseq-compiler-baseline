//! Manifest schema unit tests (Doc 3 section 3): parse valid rust / ir /
//! panic manifests, the `entry` default, and reject a malformed one.
//!
//! The `rustc_private` anchor is required because the crate under test
//! transitively links the rustc-backed bridge.

#![feature(rustc_private)]

extern crate rustc_driver;

use codegen_coverage_runner::manifest::{parse_manifest, Expect, FixtureKind};

#[test]
fn parses_rust_pass_manifest_with_default_entry() {
    let text = r#"
name = "example_add"
kind = "rust"
covers = ["BO-1..5", "RV-6"]
expect = "pass"
"#;
    let m = parse_manifest(text).expect("valid rust manifest must parse");
    assert_eq!(m.name, "example_add");
    assert_eq!(m.kind, FixtureKind::Rust);
    assert_eq!(m.entry, "regseq_main", "entry must default to regseq_main");
    assert_eq!(m.covers, vec!["BO-1..5".to_string(), "RV-6".to_string()]);
    assert_eq!(m.expect, Expect::Pass);
}

#[test]
fn parses_ir_manifest() {
    let text = r#"
name = "niche_enum"
kind = "ir"
covers = ["TY-18"]
expect = "pass"
"#;
    let m = parse_manifest(text).expect("valid ir manifest must parse");
    assert_eq!(m.kind, FixtureKind::Ir);
}

#[test]
fn parses_panic_manifest_with_explicit_entry() {
    let text = r#"
name = "div_by_zero"
kind = "rust"
entry = "driver_main"
covers = ["AK-3"]
expect = "panic"
"#;
    let m = parse_manifest(text).expect("valid panic manifest must parse");
    assert_eq!(m.entry, "driver_main");
    assert_eq!(m.expect, Expect::Panic);
}

#[test]
fn rejects_manifest_missing_required_field() {
    // `kind` is required and absent.
    let text = r#"
name = "broken"
covers = ["IT-1"]
expect = "pass"
"#;
    assert!(
        parse_manifest(text).is_err(),
        "manifest missing `kind` must fail to parse"
    );
}

#[test]
fn rejects_manifest_with_unknown_kind() {
    let text = r#"
name = "broken"
kind = "wasm"
covers = ["IT-1"]
expect = "pass"
"#;
    assert!(
        parse_manifest(text).is_err(),
        "unknown kind value must fail to parse"
    );
}
