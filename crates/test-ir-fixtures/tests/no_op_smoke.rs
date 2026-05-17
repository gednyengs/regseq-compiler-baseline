//! P1a-TS-01-E2E-01 smoke fixture: bridge the canonical `fn no_op() {}`
//! Rust program through the full phase-1a `regseq-driver` pipeline,
//! validate the resulting `RegseqModel`, pretty-print it, and lock the
//! rendered output against an `insta` snapshot.

#![feature(rustc_private)]

extern crate rustc_driver;

use regseq_model::BridgeInput;
use test_ir_fixtures::{bridge_fixture, fixture_names, render_default};

#[test]
fn no_op_bridge_validates_and_pretty_prints() {
    let mut model = bridge_fixture(fixture_names::NO_OP);

    // 1. Validation passes (helper would have panicked otherwise,
    //    but assert structurally for clarity).
    let bridge_input = BridgeInput::matching(&model);
    model
        .validate(&bridge_input)
        .expect("RegseqModel::validate must pass on no_op");

    // 2. Exactly one Function Instance is present (the `no_op` fn).
    //    No synthesised Items expected (no closures, no vtables, no
    //    drop-glue, no const-memory statics).
    assert_eq!(
        model.instances.len(),
        1,
        "no_op should produce one Instance"
    );

    // 3. Pretty-printed output is non-empty and snapshot-stable.
    let rendered = render_default(&model);
    assert!(!rendered.is_empty(), "render must be non-empty");
    insta::assert_snapshot!("no_op_render", rendered);
}
