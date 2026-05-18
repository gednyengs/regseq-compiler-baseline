//! regseq-driver-54 spec-contract lock for `Ty::Enum` layout.
//!
//! Bridges `crates/test-ir-fixtures/fixtures/enum_round_trip.rs`
//! through the full phase-1a `regseq-driver` pipeline and asserts:
//!
//!   1. `model.validate(...)` passes -- `validate_layouts`
//!      (`crates/regseq-model/src/model.rs`) enforces that every
//!      `Ty::Enum` carries `LayoutKind::Enum { shape: EnumShape }`.
//!   2. `PrettyPrinter::new(&model).render()` produces a non-empty
//!      string without panicking -- the four enum-shape pretty.rs
//!      panic sites (instance / aggregate / field / variant-name
//!      rendering) only fire on rendered output, not via `validate`.
//!
//! Together these two assertions lock the spec-contract end-to-end:
//! per regseq-driver-54 Test Plan item C the bridge produces enums
//! whose layout passes both the validator AND the renderer.

#![feature(rustc_private)]

extern crate rustc_driver;

use test_ir_fixtures::{bridge_fixture, fixture_names, render_default};

#[test]
fn bridged_enum_round_trip_passes_model_validate_and_renders() {
    // `bridge_fixture` already invokes `model.validate(...)` after
    // bridging (see `compile_and_bridge` in test-ir-fixtures), so a
    // successful return here means validation passed. Re-invoking
    // `validate` explicitly here is unnecessary; the helper is the
    // source of truth.
    let model = bridge_fixture(fixture_names::ENUM_ROUND_TRIP);

    // Pretty-printer panic-coverage lock: the four enum-related
    // panic sites in `crates/regseq-model/src/pretty.rs` only fire
    // during render; if the bridge regresses to producing a wrong
    // EnumShape, this call panics rather than returning an empty
    // string. Mirrors the pattern in `e2e_diag_logging.rs` and
    // `e2e_synthesis.rs`.
    let rendered = render_default(&model);
    assert!(
        !rendered.is_empty(),
        "rendered model must be non-empty for the enum_round_trip fixture"
    );
}
