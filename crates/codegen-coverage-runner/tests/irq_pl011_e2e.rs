//! The device-interrupt M1 vertical-slice gate (construct #6, PLAN T07): the
//! `irq-pl011` fixture passes the six-stage dual oracle in a DEDICATED clean
//! process (the emit's MIR is sensitive to rustc's process-global interner
//! history — D17). The blessed `expected.stdout` is the C01 PLACEMENT GRAMMAR
//! MADE OBSERVABLE (I07): registration (line disabled) after the bus
//! acquisition + map_region keystone, the pre-init fire inert (O1), the
//! enable only after `on_init` Ok, the masked fire's `NotHandled`, the
//! cause-poked fire's `Handled` with the ICR W1C ack in the MMIO trace, the
//! deinit disable→deregister before the resource postambles, and the
//! post-deinit fire inert again.
//!
//! `BLESS=1` writes `golden.c` + `expected.stdout` from a single clean
//! bind+emit+run, then returns without also verifying (the keystone pattern).
//!
//! SOLE test in this binary (like `lifecycle_pl011_e2e` / `pl011_composed_e2e`).

#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::{Path, PathBuf};

use codegen_coverage_runner::{manifest, pipeline, toolchain};
use regseq_model::TargetOs;
use test_ir_fixtures::{try_bridge_fixture_path, BridgeOutcome};

/// The host `cc`, matching the runner's default.
fn cc() -> &'static Path {
    Path::new("cc")
}

fn irq_corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/_irq")
}

#[test]
fn irq_pl011_six_stage_oracle() {
    let dir = irq_corpus().join("irq-pl011");
    let fixtures = manifest::discover(&irq_corpus()).expect("discover irq corpus");
    let fixture = fixtures
        .iter()
        .find(|f| f.manifest.name == "irq-pl011")
        .expect("irq-pl011 fixture discovered")
        .with_binding(manifest::OsBinding::Linux)
        .expect("irq-pl011 resolves the linux binding (C05 layout)");
    let fixture = &fixture;

    assert!(
        fixture.platform.is_some(),
        "irq-pl011 must resolve platform.linux.toml (the [interrupt.<id>] join, C03)"
    );

    if std::env::var_os("BLESS").is_some() {
        let options = pipeline::backend_options_for(fixture).expect("backend options");
        let mut model = match try_bridge_fixture_path(&fixture.input) {
            BridgeOutcome::Ok(m) => m,
            other => panic!("bless bridge failed: {other:?}"),
        };
        let identity = pipeline::bind_l2_platform(&mut model, fixture).expect("L2 bind (bless)");
        assert_eq!(identity.os, Some(TargetOs::Linux));
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

    // Stages 1-6 (the process's first — interner-clean — bridge).
    let result = pipeline::run_fixture(fixture, cc());
    assert!(
        result.is_green(),
        "irq-pl011 must pass all six stages: {:?}",
        result.verdict
    );

    // PLACEMENT GATES on the committed golden (C01 structural form):
    // registration with the line DISABLED rides the preamble AFTER
    // map_region; the enable sits INSIDE the on_init-Ok branch; the deinit
    // disable→deregister follows the on_deinit call.
    let golden = std::fs::read_to_string(dir.join("golden.c")).expect("read golden.c");
    let map_at = golden
        .find("regseq_bus_map_region(0x9000000ull")
        .expect("golden.c maps the AMBA region");
    let register_at = golden
        .find("regseq_irq_register(37u, rseq_isr_pl011irq, (void *)&inst);")
        .expect("golden.c registers the line with the ISR wrapper");
    assert!(
        map_at < register_at,
        "the interrupt registration must follow the map_region keystone (D14)"
    );
    assert!(
        golden.contains("regseq_irq_binding(37u, \"level\", \"active_high\", 0, -1ll);"),
        "golden.c must trace the resolved [interrupt.uart_irq] facts verbatim (R12)"
    );
    assert!(
        golden.contains("if (__disc == ") && golden.contains("regseq_irq_enable(37u);"),
        "golden.c must gate the enable on the captured on_init Ok discriminant (D02/R05)"
    );
    assert!(
        golden.contains("regseq_irq_disable(37u);")
            && golden.contains("regseq_irq_unregister(37u);"),
        "golden.c must emit the deinit disable->deregister pair (D09/R06)"
    );
    assert!(
        golden.contains("static void rseq_isr_pl011irq(void *__ctx)"),
        "golden.c must define the void ISR wrapper (C04)"
    );

    // BEHAVIORAL GATES (dual oracle) on the C01 interleaving: the four fire
    // points trace in lifecycle order — inert, NotHandled, Handled (with the
    // ICR ack at base+0x44 right after the MIS read at base+0x40), inert.
    let stdout =
        std::fs::read_to_string(dir.join("expected.stdout")).expect("read expected.stdout");
    let pos = |needle: &str| -> usize {
        stdout
            .find(needle)
            .unwrap_or_else(|| panic!("expected.stdout must contain {needle:?}; got:\n{stdout}"))
    };
    let register = pos("IRQ register line=37");
    let inert_pre = pos("IRQ fire line=37 inert");
    let on_init = pos("LC call on_init");
    let enable = pos("IRQ enable line=37");
    let not_handled = pos("IRQ result=NotHandled");
    let poke = pos("IRQ poke @0x040 = 0x00000010");
    let handled = pos("IRQ result=Handled");
    let ack = pos("W32 @0x9000044 = 0x00000010");
    let on_deinit = pos("LC call on_deinit");
    let disable = pos("IRQ disable line=37");
    let unregister = pos("IRQ unregister line=37");
    assert!(
        register < inert_pre && inert_pre < on_init,
        "pre-init fire must be inert between registration and on_init"
    );
    assert!(
        on_init < enable && enable < not_handled,
        "enable must follow on_init Ok and precede the masked fire's NotHandled"
    );
    assert!(
        not_handled < poke && poke < handled && handled < ack
            || not_handled < poke && poke < ack && ack < handled,
        "the cause poke must precede the Handled dispatch and its ICR ack"
    );
    assert!(
        on_deinit < disable && disable < unregister,
        "teardown must run on_deinit -> disable -> unregister (D09)"
    );
    let inert_post = stdout
        .rfind("IRQ fire line=37 inert")
        .expect("a post-deinit inert fire must trace");
    assert!(
        inert_post > unregister,
        "the post-deinit fire must trace inert after the unregister"
    );
}
