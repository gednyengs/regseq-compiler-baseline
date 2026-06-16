//! The device-interrupt zephyr_c gate (construct #6, PLAN T11 + the T12
//! zephyr leg): the `irq-pl011` fixture's zephyr lowering is byte-locked as
//! `golden.zephyr.c` (+ binding.yaml + conformance.zephyr.json), with the C01
//! placement made STRUCTURAL in the emitted driver:
//!
//!   * the void ISR wrapper `rseq_isr_<drv>` precedes the init fn, recovers
//!     the device from the build-time-constant param and DISCARDS the
//!     IrqResult (R08 zephyr leg; no locks, R18);
//!   * init: `IRQ_CONNECT(DT_INST_IRQN(0), …)` after the MMIO preamble
//!     (D14) WITHOUT the stock immediate `irq_enable` (R04); the enable is
//!     SPLIT to after the on_init-Ok adapter (D02/R05/I01) — the Err path
//!     has already returned, leaving the line connected-but-masked;
//!   * the L2 `irq_number` renders as the ADVISORY comment only (R10); the
//!     L2 `priority` is absent, so the DT cell is authoritative (D10/R12);
//!   * deinit: `irq_disable` after on_deinit, then EXACTLY ONE
//!     `RSEQ-ABSENT-OP` audit block at the deregister-slot position, from
//!     the same const as the conformance row (R07/C02/I04);
//!   * conformance: `structurally_absent_slots` carries
//!     CAP-ZEPHYR-STATICIRQ-DEREGISTER-1 with the connected-but-masked
//!     citation, and the RSEQ-CONFORMANCE line cites the row.
//!
//! `BLESS=1` writes the three artifacts from a single clean bridge+bind+emit.
//! SOLE test in this binary (interner isolation, D17).

#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::{Path, PathBuf};

use codegen_coverage_runner::{manifest, pipeline};
use test_ir_fixtures::{try_bridge_fixture_path, BridgeOutcome};

fn irq_corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/_irq")
}

#[test]
fn zephyr_irq_pl011_snapshot_and_l0_structure() {
    let fixtures = manifest::discover(&irq_corpus()).expect("discover irq corpus");
    let fixture = fixtures
        .iter()
        .find(|f| f.manifest.name == "irq-pl011")
        .expect("irq-pl011 fixture discovered")
        .with_binding(manifest::OsBinding::Zephyr)
        .expect("irq-pl011 resolves the zephyr binding");
    let fixture = &fixture;

    if std::env::var_os("BLESS").is_some() {
        let mut model = match try_bridge_fixture_path(&fixture.input) {
            BridgeOutcome::Ok(m) => m,
            other => panic!("bless bridge failed: {other:?}"),
        };
        pipeline::bind_l2_platform(&mut model, fixture).expect("L2 bind (bless)");
        let emit = regseq_backends::emit_zephyr_c(&model).expect("emit_zephyr_c must succeed");
        std::fs::write(pipeline::zephyr_golden_path(fixture), &emit.tu)
            .expect("write golden.zephyr.c");
        std::fs::write(pipeline::zephyr_binding_path(fixture), &emit.binding_yaml)
            .expect("write golden.zephyr.binding.yaml");
        std::fs::write(
            pipeline::zephyr_conformance_path(fixture),
            codegen_coverage_runner::conformance::render_json(&emit.conformance),
        )
        .expect("write conformance.zephyr.json");
        return;
    }

    let result = pipeline::run_zephyr_fixture(fixture);
    assert!(
        result.is_green(),
        "irq-pl011 (zephyr binding) must pass bridge -> emit_zephyr_c -> snapshot: {:?}",
        result.verdict
    );

    let tu = std::fs::read_to_string(pipeline::zephyr_golden_path(fixture))
        .expect("read golden.zephyr.c");
    let pos = |needle: &str| {
        tu.find(needle)
            .unwrap_or_else(|| panic!("TU must contain `{needle}`; got:\n{tu}"))
    };

    // C01 init order, structurally: wrapper -> init fn -> MMIO preamble ->
    // IRQ_CONNECT (no immediate enable) -> on_init adapter -> irq_enable.
    let wrapper = pos("static void rseq_isr_pl011irq(const void *arg)");
    let init_fn = pos("static int pl011irq_init(const struct device *dev)");
    let mmio_map = pos("DEVICE_MMIO_MAP(dev, K_MEM_CACHE_NONE);");
    let advisory = pos("irq_number=40 is ADVISORY on zephyr");
    let connect = pos("IRQ_CONNECT(DT_INST_IRQN(0), DT_INST_IRQ(0, priority), rseq_isr_pl011irq,");
    let on_init_call = pos("pl011irq_on_init(&data->inner)");
    let enable = pos("irq_enable(DT_INST_IRQN(0));");
    assert!(
        wrapper < init_fn && init_fn < mmio_map && mmio_map < advisory && advisory < connect,
        "the static connect must follow the MMIO preamble (D14), wrapper first"
    );
    assert!(
        connect < on_init_call && on_init_call < enable,
        "the enable must SPLIT to after the on_init-Ok adapter (D02/R05), never ride \
         the connect"
    );

    // Deinit order: on_deinit -> irq_disable -> the ONE deregister audit block.
    let deinit_fn = pos("static int pl011irq_deinit(const struct device *dev)");
    let on_deinit_call = pos("pl011irq_on_deinit(&data->inner)");
    let disable = pos("irq_disable(DT_INST_IRQN(0));");
    let audit = pos("RSEQ-ABSENT-OP: interrupt.register_handler.deregister");
    assert!(
        deinit_fn < on_deinit_call && on_deinit_call < disable && disable < audit,
        "deinit must order on_deinit -> irq_disable -> the deregister-slot audit (D09/R07)"
    );
    assert_eq!(
        tu.matches("RSEQ-ABSENT-OP").count(),
        1,
        "exactly one structurally-absent audit block (I04)"
    );
    assert!(
        tu.contains("disposition: StructurallyAbsent row: CAP-ZEPHYR-STATICIRQ-DEREGISTER-1"),
        "the audit block must cite the ratified row (C02)"
    );
    // The audit block sits AT the deregister-slot position: immediately
    // before the deinit postamble comment (Codex M4: broad ordering is not
    // position). The audit block's closing line must be adjacent to the
    // postamble comment up to whitespace.
    let audit_end_marker = "structurally absent on this registration form. */";
    let audit_end = pos(audit_end_marker) + audit_end_marker.len();
    let postamble = pos("postamble (integration \u{a7}5.2)");
    assert!(audit < postamble, "audit precedes the postamble");
    let between = &tu[audit_end..postamble];
    assert!(
        between.trim_matches(|c: char| c.is_whitespace() || c == '/' || c == '*').is_empty(),
        "nothing may sit between the audit block and the postamble; got {between:?}"
    );
    // The COMPLETE connect shape (Codex M4): build-time param + flags 0 —
    // and NO trigger/flag lowering anywhere (R12/D10: the platform file's
    // trigger facts are validation inputs; the DT is authoritative).
    pos("IRQ_CONNECT(DT_INST_IRQN(0), DT_INST_IRQ(0, priority), rseq_isr_pl011irq,\n\t\t    DEVICE_DT_INST_GET(0), 0);");
    for forbidden in ["IRQ_TYPE", "trigger", "active_high", "IRQF_", "level"] {
        assert!(
            !tu.contains(forbidden),
            "no trigger/flag spelling may lower into the TU (R12); found `{forbidden}`"
        );
    }
    // The result is discarded by the void wrapper; the IrqResult never maps
    // to a return channel on zephyr (R08).
    pos("(void)pl011irq_handle_irq(&data->inner);");

    // T12 zephyr leg: the conformance JSON carries the row + the
    // connected-but-masked statement, and the summary line cites it.
    let conf = std::fs::read_to_string(pipeline::zephyr_conformance_path(fixture))
        .expect("read conformance.zephyr.json");
    assert!(
        conf.contains("\"slot\": \"interrupt.register_handler.deregister\", \"row\": \"CAP-ZEPHYR-STATICIRQ-DEREGISTER-1\""),
        "conformance must carry the structurally-absent row (I04); got:\n{conf}"
    );
    assert!(
        conf.contains("connected-but-masked"),
        "the connected-but-masked statement must ride the citation (R07); got:\n{conf}"
    );
    // The RSEQ-CONFORMANCE log line cites the row (T12).
    let mut model = match try_bridge_fixture_path(&fixture.input) {
        BridgeOutcome::Ok(m) => m,
        other => panic!("re-bridge failed: {other:?}"),
    };
    pipeline::bind_l2_platform(&mut model, fixture).expect("L2 bind");
    let emit = regseq_backends::emit_zephyr_c(&model).expect("emit");
    assert!(
        emit.conformance.summary_line().contains(
            "structurally_absent=[interrupt.register_handler.deregister:CAP-ZEPHYR-STATICIRQ-DEREGISTER-1]"
        ),
        "the RSEQ-CONFORMANCE line must cite the row; got: {}",
        emit.conformance.summary_line()
    );
}
