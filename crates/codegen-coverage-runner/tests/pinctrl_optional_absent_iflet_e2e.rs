//! M3 hardening (Codex diff-review rec #3): the optional-ABSENT-via-IF-LET twin of
//! `pinctrl_optional_absent_e2e`. The optional `Option<PinCtrl>` is absent AND the
//! author uses the plain `if let Some(p) = &self.pinctrl { p.apply::<S>() }` path —
//! so the `Some` branch is statically dead. The dead branch must compile (fold to
//! a discarded `Err(PinctrlAbsent)`), the absent-only TU must stay helper-free (no
//! `regseq_pinctrl_*`), and the run must show NO `PIN` lines. Sole test in its
//! binary (interner isolation). `BLESS=1` writes the goldens.

#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::{Path, PathBuf};

use codegen_coverage_runner::{manifest, pipeline, toolchain};
use test_ir_fixtures::{try_bridge_fixture_path, BridgeOutcome};

fn cc() -> &'static Path {
    Path::new("cc")
}

fn pinctrl_corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/_pinctrl")
}

#[test]
fn pinctrl_pl011_optional_absent_iflet_oracle() {
    let dir = pinctrl_corpus().join("pinctrl-pl011-optional-absent-iflet");
    let fixtures = manifest::discover(&pinctrl_corpus()).expect("discover pinctrl corpus");
    let fixture = fixtures
        .iter()
        .find(|f| f.manifest.name == "pinctrl-pl011-optional-absent-iflet")
        .expect("pinctrl-pl011-optional-absent-iflet fixture discovered");

    if std::env::var_os("BLESS").is_some() {
        let options = pipeline::backend_options_for(fixture).expect("backend options");
        let mut model = match try_bridge_fixture_path(&fixture.input) {
            BridgeOutcome::Ok(m) => m,
            other => panic!("bless bridge failed: {other:?}"),
        };
        let identity = pipeline::bind_l2_platform(&mut model, fixture).expect("L2 bind (bless)");
        let context = regseq_backends::BackendContext::new(options, identity);
        let emit = regseq_backends::emit_generic_c_for_entry_with_context(
            &model,
            &fixture.manifest.entry,
            &context,
        )
        .expect("emit must succeed");
        let run = toolchain::compile_and_run(&emit.tu, &emit.shim, cc())
            .expect("compile + run must succeed");
        std::fs::write(&fixture.golden_c, &emit.tu).expect("write golden.c");
        std::fs::write(&fixture.expected_stdout, &run.stdout).expect("write expected.stdout");
        return;
    }

    let result = pipeline::run_fixture(fixture, cc());
    assert!(
        result.is_green(),
        "pinctrl-pl011-optional-absent-iflet must pass all six stages: {:?}",
        result.verdict
    );

    let golden = std::fs::read_to_string(dir.join("golden.c")).expect("read golden.c");
    // The dead Some branch must NOT emit any pinctrl hook (the gate did not fire).
    for needle in [
        "regseq_pinctrl_acquire(",
        "regseq_pinctrl_apply(",
        "regseq_pinctrl_release(",
        "REGSEQ_PINCTRL_SLOTS",
    ] {
        assert!(
            !golden.contains(needle),
            "the if-let absent path must NOT emit {needle:?} (the dead Some branch stays hook-free)"
        );
    }
    // The fold is present (the dead branch folds to Err(PinctrlAbsent)).
    assert!(
        golden.contains("Err(RegseqError::PinctrlAbsent) -> ENODEV"),
        "the dead Some branch folds to Err(PinctrlAbsent)"
    );
    // No Some-materialization (the resource is absent → stays None).
    assert!(
        !golden.contains("present materialization"),
        "an absent optional resource must NOT materialize a Some tag"
    );

    let stdout =
        std::fs::read_to_string(dir.join("expected.stdout")).expect("read expected.stdout");
    assert!(
        !stdout.contains("PIN "),
        "the absent if-let path emits NO PIN trace lines; got:\n{stdout}"
    );
    assert!(
        stdout.contains("W32 @0x9000000 = 0x00000042"),
        "the run is non-vacuous (the MMIO write still traces)"
    );
}
