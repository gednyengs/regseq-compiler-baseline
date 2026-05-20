//! regseq-driver-59 Test Plan B8: foreign-decl rendering snapshot.
//!
//! Bridges `crates/test-ir-fixtures/fixtures/foreign_rendering.rs`
//! through the full phase-1a `regseq-driver` pipeline and asserts the
//! rendered output contains the `#[foreign]\nfn ...;` (pretty-printer
//! Specimen 113 at `pretty-printer.md:2098-2117`) and
//! `#[foreign]\nstatic ...;` (Specimen 120 at
//! `pretty-printer.md:2236-2254`) declaration shapes for the foreign
//! function and foreign static respectively. The source-defined
//! `entry` fn must NOT carry the `#[foreign]` attribute.
//!
//! Regression target: a pretty-printer change that dropped the
//! `#[foreign]` attribute, mis-applied it to a source-defined fn, or
//! changed the body-less declaration shape would fail this snapshot.
//! The structural foreign-Instance assertions covered by
//! `crates/regseq-driver/tests/bridge_eager_hir_enumeration.rs` (B3)
//! verify that the bridge constructs the right Instance shape; this
//! test pins the *rendered* form on top.

#![feature(rustc_private)]

extern crate rustc_driver;

use test_ir_fixtures::{bridge_fixture, fixture_names, render_default};

#[test]
fn bridged_foreign_rendering_pins_foreign_attribute_form() {
    let model = bridge_fixture(fixture_names::FOREIGN_RENDERING);

    let rendered = render_default(&model);

    // B8 content checks: the rendered output must carry the
    // `#[foreign]\nextern "C" fn` and `#[foreign]\nstatic`
    // declaration shapes per Specimens 113 and 120 at
    // `pretty-printer.md:2108-2110` and `:2246-2247`.
    assert!(
        rendered.contains("#[foreign]\nextern \"C\" fn"),
        "rendered output must contain `#[foreign]\\nextern \"C\" fn` per Specimen 113 (foreign function); got:\n{rendered}"
    );
    assert!(
        rendered.contains("#[foreign]\nstatic"),
        "rendered output must contain `#[foreign]\\nstatic` per Specimen 120 (foreign static); got:\n{rendered}"
    );
    assert!(
        rendered.contains("extern_helper"),
        "rendered output must mention the foreign fn `extern_helper`; got:\n{rendered}"
    );
    assert!(
        rendered.contains("EXTERN_VALUE"),
        "rendered output must mention the foreign static `EXTERN_VALUE`; got:\n{rendered}"
    );
    assert!(
        rendered.contains("entry"),
        "rendered output must mention the source-defined fn `entry`; got:\n{rendered}"
    );

    // Negative assertion: the source-defined `entry` fn must NOT
    // carry the `#[foreign]` attribute (regression guard against
    // accidental attribute over-application by the pretty-printer).
    // Per `pretty-printer.md:3640-3698` the `#[foreign]` line is
    // emitted only for Items whose Linkage is `Foreign`; the
    // source-defined `entry` fn carries `Linkage::Default` and must
    // render as a full-body definition.
    for chunk in rendered.split("\n\n") {
        if chunk.contains("entry") && chunk.contains("fn ") {
            assert!(
                !chunk.contains("#[foreign]"),
                "source-defined `entry` fn must not carry #[foreign]; got chunk:\n{chunk}"
            );
        }
    }

    insta::assert_snapshot!("render", rendered);
}
