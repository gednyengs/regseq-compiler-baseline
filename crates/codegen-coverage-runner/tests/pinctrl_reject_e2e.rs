//! The pin-controller-and-GPIO-configuration NEGATIVE corpus gate (pinctrl
//! construct, M3/T17): every `expect = "reject"` fixture under
//! `fixtures/_pinctrl/` is GREEN as a reject — the pipeline fails at exactly the
//! fixture's `[expected].stage` with its `[expected].diagnostic` substring,
//! covering the full C06 negative family:
//!
//!   * acquire-stage recognition (`PIN-ERR-*`, the bridge): missing/malformed id,
//!     attr-on-non-pinctrl, unsupported form, duplicate id, malformed/non-ZST/
//!     duplicate state markers, spoofed receiver;
//!   * bind-stage semantic (`PIN-ERR-*`, the post-L2 `validate_pinctrl_semantics`
//!     pass): author-managed field, unresolved state param, invalid state marker
//!     (the last asserted through the plain `if let Some(p)` path — proving the
//!     shared receiver recovery validates membership on BOTH call paths, no
//!     split-brain);
//!   * bind-stage L2 CONTENT (`PIN-L2-00x`): no-inventory, no-default,
//!     empty-states, empty-state-name, duplicate-state, orphan-section;
//!   * bind-stage parser SHAPE (`BadPinctrl`): missing `pin_states` key,
//!     non-array value, non-string entry (the SHAPE-vs-CONTENT split, SF6/C01).
//!
//! NOT fixtured (no input can reach them): `PIN-L2-MANY-MATCH` is enforced by
//! construction (explicit-id join + TOML duplicate-section rejection +
//! `PIN-ERR-DUPLICATE-ID` leave no field resolving to >1 section), and
//! `PIN-ERR-CONTEXT` (atomic-context `apply`) is design-in pending the
//! cross-construct context-detection seam (GRILL C06/D11). No emit output is
//! byte-snapshotted, so a shared process is fine; it is its OWN binary (it
//! bridges).

#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::{Path, PathBuf};

use codegen_coverage_runner::manifest::{self, Expect};
use codegen_coverage_runner::pipeline;

/// The host `cc` (never invoked — rejects fail before stage 4).
fn cc() -> &'static Path {
    Path::new("cc")
}

fn pinctrl_corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/_pinctrl")
}

#[test]
fn every_pinctrl_reject_fixture_rejects_exactly_as_declared() {
    let fixtures = manifest::discover(&pinctrl_corpus()).expect("discover pinctrl corpus");
    let rejects: Vec<_> = fixtures
        .iter()
        .filter(|f| matches!(f.manifest.expect, Expect::Reject))
        .collect();
    assert!(
        rejects.len() >= 21,
        "expected the full T17 reject corpus (>= 21: 9 recognition + 3 semantic + 6 L2 CONTENT \
         + 3 parser SHAPE), found {}",
        rejects.len()
    );
    for fixture in rejects {
        let result = pipeline::run_fixture(fixture, cc());
        assert!(
            result.is_green(),
            "reject fixture {:?} must fail exactly as declared: {:?}",
            fixture.manifest.name,
            result.verdict
        );
    }
}
