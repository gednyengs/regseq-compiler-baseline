//! The pin-controller-and-GPIO-configuration M3 optional-ABSENT POSITIVE oracle
//! (PLAN T16 / GRILL S12): the `pinctrl-pl011-optional-absent` fixture proves the
//! D06 compile-time branch-fold. With NO `[pinctrl.<id>]` section the optional
//! `Option<PinCtrl>` binds to `None` (`resolved = false`), and the author's
//! `OptionalPinCtrl::apply` folds to a direct `Err(RegseqError::PinctrlAbsent)` —
//! no hook, no `PIN apply` trace. The oracle is POSITIVE-not-vacuous: it asserts
//! ALL of (1) no pinctrl platform section, (2) the bound model carries the
//! optional resource `required=false`/`resolved=false`/empty `pin_states`, (3) NO
//! `PINCTRL_HELPERS`/`regseq_pinctrl_*` emitted (the gate did not fire), (4) the
//! `ENODEV`/`PinctrlAbsent` fold IS present in the generated source, (5) a clean
//! run with NO `PIN` trace lines.
//!
//! Sole test in its binary (interner isolation). `BLESS=1` writes the goldens.

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
fn pinctrl_pl011_optional_absent_oracle() {
    let dir = pinctrl_corpus().join("pinctrl-pl011-optional-absent");
    let fixtures = manifest::discover(&pinctrl_corpus()).expect("discover pinctrl corpus");
    let fixture = fixtures
        .iter()
        .find(|f| f.manifest.name == "pinctrl-pl011-optional-absent")
        .expect("pinctrl-pl011-optional-absent fixture discovered");

    // (1) The platform.toml carries the MMIO join but NO [pinctrl.<id>] section.
    let platform_src = std::fs::read_to_string(
        fixture
            .platform
            .as_ref()
            .expect("optional-absent carries platform.toml"),
    )
    .expect("read platform.toml");
    assert!(
        !platform_src.contains("[pinctrl."),
        "the absent fixture's platform.toml must have NO [pinctrl.<id>] section"
    );

    // Bridge + bind once: inspect the bound MODEL for the positive recognition
    // assertion (2) — a present optional resource that resolved to None.
    let mut model = match try_bridge_fixture_path(&fixture.input) {
        BridgeOutcome::Ok(m) => m,
        other => panic!("bridge failed: {other:?}"),
    };
    let identity = pipeline::bind_l2_platform(&mut model, fixture).expect("L2 bind");

    let resource = model
        .pinctrl
        .drivers
        .values()
        .flat_map(|d| &d.resources)
        .find(|r| model.strings.resolve(r.pinctrl_id) == "uart_pins")
        .expect("the optional pinctrl resource must be RECOGNIZED (not silently missed)");
    assert!(
        !resource.required,
        "(2) the resource must be the optional form (required=false)"
    );
    assert!(
        !resource.binding.resolved,
        "(2) the absent optional resource must bind to None (resolved=false)"
    );
    assert!(
        resource.binding.pin_states.is_empty(),
        "(2) an absent binding carries no inventory (empty pin_states)"
    );
    assert!(
        !resource.binding.is_resolved(),
        "(2) is_resolved() is false for the absent binding"
    );

    if std::env::var_os("BLESS").is_some() {
        let options = pipeline::backend_options_for(fixture).expect("backend options");
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
        "pinctrl-pl011-optional-absent must pass all six stages: {:?}",
        result.verdict
    );

    let golden = std::fs::read_to_string(dir.join("golden.c")).expect("read golden.c");

    // (3) The gate did NOT fire: no PINCTRL_HELPERS / regseq_pinctrl_* hook
    // definitions or calls. (The only `regseq_pinctrl_*` mention allowed is inside
    // the fold's explanatory comment.)
    for needle in [
        "regseq_pinctrl_acquire(",
        "regseq_pinctrl_apply(",
        "regseq_pinctrl_release(",
        "regseq_pinctrl_slot",
        "REGSEQ_PINCTRL_SLOTS",
    ] {
        assert!(
            !golden.contains(needle),
            "(3) an absent-only TU must NOT emit {needle:?} (the prelude gate must not fire)"
        );
    }

    // (4) The ENODEV/PinctrlAbsent fold IS present in the generated source.
    assert!(
        golden.contains("Err(RegseqError::PinctrlAbsent) -> ENODEV"),
        "(4) the optional-absent fold to Err(PinctrlAbsent)/ENODEV must be in the source"
    );

    // (5) A clean run with NO `PIN` trace lines (but the MMIO write still runs —
    // a non-vacuous run).
    let stdout =
        std::fs::read_to_string(dir.join("expected.stdout")).expect("read expected.stdout");
    assert!(
        !stdout.contains("PIN "),
        "(5) an absent resource emits NO PIN trace lines; got:\n{stdout}"
    );
    assert!(
        stdout.contains("W32 @0x9000000 = 0x00000042"),
        "(5) the run must be non-vacuous (the anchoring MMIO write still traces)"
    );
}
