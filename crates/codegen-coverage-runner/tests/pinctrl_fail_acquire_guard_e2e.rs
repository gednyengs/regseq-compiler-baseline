//! T18 fail-loud guard (Codex T18 diff-review rec): an injected
//! `pinctrl.fail-acquire` id that matches NO resolved pinctrl resource must fail
//! emission LOUD (a fixture typo must not silently become "no failure injected").
//! Bridges the required-acquire-failure fixture (which DOES have a resolved
//! `uart_pins`) but emits with a BOGUS `pinctrl.fail-acquire` override → the
//! synthesizer returns `EmitError::BackendOption`. Sole test in its binary.

#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::{Path, PathBuf};

use codegen_coverage_runner::{manifest, pipeline};
use test_ir_fixtures::{try_bridge_fixture_path, BridgeOutcome};

fn pinctrl_corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/_pinctrl")
}

#[test]
fn pinctrl_fail_acquire_unmatched_id_fails_loud() {
    let fixtures = manifest::discover(&pinctrl_corpus()).expect("discover pinctrl corpus");
    let fixture = fixtures
        .iter()
        .find(|f| f.manifest.name == "pinctrl-required-acquire-failure")
        .expect("pinctrl-required-acquire-failure fixture discovered");

    let mut model = match try_bridge_fixture_path(&fixture.input) {
        BridgeOutcome::Ok(m) => m,
        other => panic!("bridge failed: {other:?}"),
    };
    let identity = pipeline::bind_l2_platform(&mut model, fixture).expect("L2 bind");

    // A BOGUS fail-acquire id (the real resource is "uart_pins").
    let options =
        regseq_backends::BackendOptions::from_backend_params([("pinctrl.fail-acquire", "nope")])
            .expect("parse options");
    let context = regseq_backends::BackendContext::new(options, identity);
    let result = regseq_backends::emit_generic_c_for_entry_with_context(
        &model,
        &fixture.manifest.entry,
        &context,
    );
    match result {
        Err(regseq_backends::EmitError::BackendOption(msg)) => {
            assert!(
                msg.contains("nope") && msg.contains("no resolved pinctrl resource"),
                "the guard message must name the unmatched id; got: {msg}"
            );
        }
        other => panic!("expected a fail-loud BackendOption error for the unmatched id; got {other:?}"),
    }
}
