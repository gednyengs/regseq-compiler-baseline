//! The `pl011-full-spec` interrupt-surface convergence pin (construct #6,
//! PLAN T13 — the achievable scope, with the residual RECORDED).
//!
//! The PLAN's T13 expected the spec-convergence unit to leave a full-spec
//! model-JSON suite in place; the landed unit converged `pl011-composed`
//! instead and the full-spec fixture's `expected/regseq-model.json` +
//! `expected/ir/*` are ORPHANED legacy artifacts (their generator died with
//! the `*-old` crates, deleted 2026-06-10). The full spec also CANNOT bridge
//! end-to-end yet: its `Option<Clock>` / `Option<ResetLine>` /
//! `Option<PinCtrl>` fields await their constructs (the shells are not
//! `Copy`, so the device-lifecycle D3 user-data rule rejects them — by
//! design, fail-loud).
//!
//! What T13 DELIVERS here, pinned by this test:
//!   1. the ratified fixture migrations — `trigger_polarity` moved
//!      `"high"` -> `"active_high"` (D10/R12: the binder rejects
//!      non-canonical; the FIXTURE moves, never the binder) — by parsing the
//!      REAL `platform.toml` through the production config + L2 tenant path,
//!      and the no-`Debug` irq.rs rule;
//!   2. the blocked-on residual, RECORDED: the fixture is a cargo crate the
//!      single-file bridge harness cannot compile, and its clock/reset/
//!      pinctrl fields await their constructs — the source-level pins below
//!      keep the donor surface honest until a successor unit flips this
//!      test into the true full-spec convergence run (the scheduled idiom).

#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::{Path, PathBuf};

use regseq_model::{InterruptForm, InterruptResource, ItemId, RegseqModel, StableCrateId};

fn full_spec() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/pl011-full-spec")
}

#[test]
fn full_spec_interrupt_section_is_canonical_through_the_real_join() {
    // Parse the fixture's REAL platform.toml and join its [interrupt.uart_irq]
    // through the production L2 tenant onto a minimal model carrying the
    // fixture's interrupt resource shape. A non-canonical spelling (the
    // pre-T13 "high") fails the bind with IRQ-L2-002.
    let file = regseq_config::parse_platform_file_path(&full_spec().join("platform.toml"))
        .expect("the fixture platform.toml parses");
    let mut model = RegseqModel::with_primary("pl011-full-spec", StableCrateId(1));
    let name = model.strings.intern("irq");
    let id = model.strings.intern("uart_irq");
    model.interrupt.drivers.insert(
        ItemId::from_u32(0),
        regseq_model::DriverInterrupts {
            resources: vec![InterruptResource {
                name,
                id,
                form: InterruptForm::Single,
                binding: Default::default(),
            }],
            handlers: Vec::new(),
        },
    );
    // The fixture's platform.toml also carries a `[pinctrl.uart_pins]` section;
    // the pinctrl tenant reverse-join (PIN-L2-006, M2/T11, parallel to clock) now
    // rejects a section with no matching L1 resource, so the model must declare the
    // pinctrl resource the full-spec driver actually has (here it joins cleanly).
    let pin_name = model.strings.intern("pins");
    let pinctrl_id = model.strings.intern("uart_pins");
    model.pinctrl.drivers.insert(
        ItemId::from_u32(0),
        regseq_model::DriverPinctrls {
            resources: vec![regseq_model::PinctrlResource {
                name: pin_name,
                pinctrl_id,
                required: false,
                binding: Default::default(),
            }],
        },
    );
    regseq_l2::bind_all(&mut model, &file.bindings)
        .expect("the migrated canonical spellings must bind (T13: active_high)");
    regseq_l2::validate_l2_surface(&model).expect("the joined surface validates");
    let binding = &model.interrupt.drivers[&ItemId::from_u32(0)].resources[0].binding;
    assert_eq!(binding.irq_number, Some(37));
    assert_eq!(
        binding.trigger_polarity,
        Some(regseq_model::TriggerPolarity::ActiveHigh),
        "the fixture must carry the canonical spelling (D10/R12)"
    );
    assert!(!binding.shared, "QEMU-virt PL011 is unshared (D05 board fact)");
    assert!(binding.resolved);
}

#[test]
fn full_spec_interrupt_surface_carries_the_converged_spellings() {
    // The fixture is a CARGO CRATE (it uses the `regseq_macros` proc-macros
    // and a multi-file layout), so the single-file bridge harness cannot
    // compile it; until the clock/reset/pinctrl constructs land (their
    // `Option<Clock>`-style fields D3-reject under the current user-data
    // rule by design) the full-spec pipeline run stays blocked — the
    // RECORDED T13 residual. What converges NOW is the interrupt surface's
    // authored spellings, pinned at source level: the recognized attribute
    // forms, the marked framework type, and the no-`Debug` rule the bridge's
    // fn-sig translation demands.
    let driver = std::fs::read_to_string(full_spec().join("src/driver.rs")).expect("driver.rs");
    assert!(
        driver.contains("#[rseq::interrupt(id = \"uart_irq\")]"),
        "the resource attr must carry the mandatory id (R01/D08)"
    );
    assert!(
        driver.contains("#[rseq::isr]"),
        "the bare ISR binding must be present (C04)"
    );
    let irq_rs = std::fs::read_to_string(full_spec().join("src/regseq/irq.rs")).expect("irq.rs");
    assert!(
        irq_rs.contains("#[rseq::framework_type(\"irq-result\")]"),
        "IrqResult must carry the framework-type marker"
    );
    assert!(
        !irq_rs.contains("derive(Debug"),
        "no Debug derive on IrqResult (T13: dyn fmt::Write breaks the fn-sig \
         layout query)"
    );
    // The blocked-on constructs are still authored (the donor surface is the
    // spec): when one of them lands and routes its fields, the successor
    // unit flips this test into the true full-spec convergence run.
    for waiting in ["Option<Clock>", "Option<ResetLine>", "Option<PinCtrl>"] {
        assert!(
            driver.contains(waiting),
            "the donor surface must keep its `{waiting}` field (the recorded \
             blocked-on residual)"
        );
    }
}
