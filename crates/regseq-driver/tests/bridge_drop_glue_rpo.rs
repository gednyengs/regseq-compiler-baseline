//! regseq-driver-81: a struct with two or more `Drop` fields (or a
//! Drop type holding a Drop field) produces a multi-block drop-glue
//! shim. rustc's shim MIR pipeline omits `ReorderBasicBlocks`, so the
//! shim body arrived non-RPO and failed the Phase-5 `BRpo` body
//! pre-check at HEAD (`BridgeOutcome::Rejected([BRpo])`). The bridge
//! now normalises every materialised body to DFS-RPO immediately
//! before joint validation, so the multi-Drop witness reaches
//! `Ok`+validate. A single-Drop field was always RPO-trivial and stays
//! `Ok`.
//!
//! Each bridge run is wrapped in `catch_unwind` so a regression that
//! ICEs surfaces as a test failure rather than aborting the binary.

#![feature(rustc_private)]

extern crate rustc_driver;

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use regseq_model::{BodyValidationError, FunctionInstanceFlavor, RegseqModel, Visitor};
use test_ir_fixtures::{try_bridge_fixture_path, BridgeOutcome};

/// Write `source` to a uniquely named temp `.rs` file and return its
/// path. The OS reclaims the temp dir.
fn write_snippet(label: &str, source: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time after unix epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("regseq-drop-glue-rpo-{label}-{nanos}.rs"));
    std::fs::write(&path, source).expect("write temp snippet");
    path
}

/// Bridge `source` and assert the bridge AND `validate()` both
/// succeed, returning the model. At HEAD the multi-Drop witnesses
/// reject; the `catch_unwind` reports a regression ICE as a failure.
fn bridge_ok(label: &str, source: &str) -> RegseqModel {
    let path = write_snippet(label, source);
    let outcome = catch_unwind(AssertUnwindSafe(|| try_bridge_fixture_path(&path)));
    let _ = std::fs::remove_file(&path);
    match outcome {
        Ok(BridgeOutcome::Ok(model)) => model,
        Ok(other) => panic!("expected BridgeOutcome::Ok for {label}, got {other:?}"),
        Err(_) => panic!("bridge panicked (regression ICE?) for {label}"),
    }
}

/// Collects the body-level validation errors emitted by the model's
/// `visit_body` DFS-RPO pre-checks (BNonEmpty / BSuccessorsValid /
/// BReachable / BRpo).
#[derive(Default)]
struct BodyPrecheck {
    errors: Vec<BodyValidationError>,
}

impl Visitor for BodyPrecheck {
    fn emit_body_error(&mut self, error: BodyValidationError) {
        self.errors.push(error);
    }
}

/// Assert every materialised Function-Instance body satisfies the
/// model's body pre-checks, including `BRpo` (DFS reverse-postorder
/// layout). This is the load-bearing property the normalization
/// establishes for ALL bridged bodies.
fn assert_all_bodies_rpo(model: &RegseqModel, label: &str) {
    for instance in model.instances.iter() {
        let Some(function) = instance.kind.as_function() else {
            continue;
        };
        let Some(body) = function.body.as_ref() else {
            continue;
        };
        let mut precheck = BodyPrecheck::default();
        precheck.visit_body(body);
        assert!(
            precheck.errors.is_empty(),
            "{label}: a materialised body failed the model body pre-checks {:?}; \
             every body must be DFS-RPO after normalization",
            precheck.errors,
        );
    }
}

/// Count materialised DropGlue-flavored Function-Instance bodies whose
/// block count meets `min_blocks`.
fn drop_glue_bodies_with_at_least(model: &RegseqModel, min_blocks: usize) -> usize {
    model
        .instances
        .iter()
        .filter(|instance| {
            let Some(function) = instance.kind.as_function() else {
                return false;
            };
            if function.flavor != FunctionInstanceFlavor::DropGlue {
                return false;
            }
            function
                .body
                .as_ref()
                .is_some_and(|body| body.basic_blocks.len() >= min_blocks)
        })
        .count()
}

const TWO_DROP_FIELDS: &str = "#![no_std]\n\
pub struct Inner;\n\
impl Drop for Inner { fn drop(&mut self) {} }\n\
pub struct Outer { pub a: Inner, pub b: Inner }\n\
pub fn f() { let _o = Outer { a: Inner, b: Inner }; }\n";

const NESTED_DROP: &str = "#![no_std]\n\
pub struct Inner;\n\
impl Drop for Inner { fn drop(&mut self) {} }\n\
pub struct Outer { pub a: Inner }\n\
impl Drop for Outer { fn drop(&mut self) {} }\n\
pub fn f() { let _o = Outer { a: Inner }; }\n";

const SINGLE_DROP: &str = "#![no_std]\n\
pub struct Inner;\n\
impl Drop for Inner { fn drop(&mut self) {} }\n\
pub struct Outer { pub a: Inner }\n\
pub fn f() { let _o = Outer { a: Inner }; }\n";

/// B6: the two-Drop-field witness bridges `Ok`+validates, every
/// materialised body is DFS-RPO, and the multi-block drop-glue shim
/// (the construct that rejected at HEAD) is present.
#[test]
fn two_drop_fields_bridges_ok_and_is_rpo() {
    let model = bridge_ok("two-drop-fields", TWO_DROP_FIELDS);
    assert_all_bodies_rpo(&model, "two-drop-fields");
    assert!(
        drop_glue_bodies_with_at_least(&model, 2) >= 1,
        "expected at least one multi-block DropGlue body (the non-RPO shim that \
         rejected at HEAD); got none",
    );
}

/// B7: a Drop type holding a Drop field also bridges `Ok`+validates,
/// with every body DFS-RPO.
#[test]
fn nested_drop_bridges_ok_and_is_rpo() {
    let model = bridge_ok("nested-drop", NESTED_DROP);
    assert_all_bodies_rpo(&model, "nested-drop");
}

/// B8 (control): a single Drop field was always RPO-trivial and stays
/// `Ok`+validates.
#[test]
fn single_drop_field_still_bridges_ok() {
    let model = bridge_ok("single-drop", SINGLE_DROP);
    assert_all_bodies_rpo(&model, "single-drop");
}
