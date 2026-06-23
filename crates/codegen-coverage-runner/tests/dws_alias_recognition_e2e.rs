//! The deferred-work-item-scheduling ADT-DefId recognition gate — alias half
//! (deferred-work-item-scheduling construct, PLAN T14, the genuinely-new
//! type-driven recognition path): the `deferred-work-alias` fixture declares its
//! `Work` field via a `type WorkAlias = Work;` alias. This binary BRIDGES it once
//! (interner-clean, sole-in-binary) and inspects the resulting `model.deferred_work`
//! to assert the recognition SEMANTICS directly (the model layer the reset golden
//! can only prove indirectly via emitted hooks):
//!   - the aliased field is recognized as exactly one `Work` resource (recognition
//!     keys on the resolved ADT item DefId + its `#[rseq::abstraction]` marker,
//!     GRILL D04 — NOT the syntactic field-type spelling);
//!   - the required form is recorded;
//!   - the bare `#[rseq::work_handler]` binds it (the handler `InstanceRef` is set);
//!   - the alias-routed `self.work.submit()` is tagged + recorded as a submit site.
//!
//! The negative twin — an unmarked local `struct Work` does NOT recognize, so it
//! falls through to device-lifecycle user-data classification and is rejected
//! there ("field … of type `Work` is not a valid user-data field") — is
//! `dws-unmarked-work-reject`, run by `dws_reject_e2e`. Together they pin
//! recognition to the DefId/marker, not the type name.

#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::{Path, PathBuf};

use codegen_coverage_runner::manifest;
use test_ir_fixtures::{try_bridge_fixture_path, BridgeOutcome};

fn dwork_corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/_deferred_work")
}

#[test]
fn deferred_work_alias_field_is_recognized() {
    let fixtures = manifest::discover(&dwork_corpus()).expect("discover deferred-work corpus");
    let fixture = fixtures
        .iter()
        .find(|f| f.manifest.name == "deferred-work-alias")
        .expect("deferred-work-alias fixture discovered");

    let model = match try_bridge_fixture_path(&fixture.input) {
        BridgeOutcome::Ok(m) => m,
        other => panic!("the alias fixture must bridge successfully: {other:?}"),
    };

    // Exactly one deferred-work driver, recognized THROUGH the alias.
    let drivers: Vec<_> = model.deferred_work.drivers.values().collect();
    assert_eq!(
        drivers.len(),
        1,
        "exactly one deferred-work driver must be recognized via the alias"
    );
    let resources = &drivers[0].resources;
    assert_eq!(
        resources.len(),
        1,
        "the aliased `Work` field is recognized as one resource (DefId/marker-based, D04)"
    );

    let r = &resources[0];
    assert!(r.required, "the required `Work` form is recorded");
    assert!(
        r.handler.is_some(),
        "the bare #[rseq::work_handler] binds the aliased Work (handler InstanceRef set)"
    );
    assert!(
        !r.submit_sites.is_empty(),
        "the alias-routed `self.work.submit()` is tagged + recorded as a submit site"
    );
}
