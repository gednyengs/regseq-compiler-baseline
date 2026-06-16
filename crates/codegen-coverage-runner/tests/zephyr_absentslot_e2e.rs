//! The OS-absent-slot snapshot gate (os-capability-disposition T09, GRILL
//! D13): `pl011-absentslot-zephyr` lowered by `emit_zephyr_c`, its THREE
//! artifacts byte-locked via this sole binary's `BLESS=1` path —
//! `golden.zephyr.c` (the `RSEQ-ABSENT-SLOT` block visible),
//! `golden.zephyr.binding.yaml`, and `conformance.zephyr.json` (the
//! absent_dispatch row) — plus named spot assertions on the audit block,
//! the R05 summary line, and an in-process double-run determinism check
//! (I05).
//!
//! The fixture is OUTSIDE the C02 identity invariant, gets NO generic_c
//! oracle bless, and has NO L3 leg (the initfail-variant precedent). Its
//! matrix row (`OS-1`) stays `deferred`; the covers aggregation lives in
//! `os_capability_covers.rs` (I08).
//!
//! SOLE test in this binary (clean-process interner isolation, like the
//! other snapshot e2es).

#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::{Path, PathBuf};

use codegen_coverage_runner::{conformance, manifest, pipeline};
use regseq_backends::capability;
use test_ir_fixtures::{try_bridge_fixture_path, BridgeOutcome};

fn composed_corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/_composed")
}

/// One full producer run (bridge -> bind -> emit), returning the three
/// artifacts as rendered bytes.
fn produce(fixture: &manifest::Fixture) -> (String, String, String, regseq_backends::ZephyrEmit) {
    let mut model = match try_bridge_fixture_path(&fixture.input) {
        BridgeOutcome::Ok(m) => m,
        other => panic!("bridge failed: {other:?}"),
    };
    pipeline::bind_l2_platform(&mut model, fixture).expect("L2 bind");
    let emit = regseq_backends::emit_zephyr_c(&model).expect("emit_zephyr_c must succeed");
    let json = conformance::render_json(&emit.conformance);
    (emit.tu.clone(), emit.binding_yaml.clone(), json, emit)
}

#[test]
fn zephyr_absentslot_snapshot_audit_block_and_determinism() {
    let fixtures = manifest::discover(&composed_corpus()).expect("discover composed corpus");
    let fixture = fixtures
        .iter()
        .find(|f| f.manifest.name == "pl011-absentslot-zephyr")
        .expect("pl011-absentslot-zephyr fixture discovered");

    if std::env::var_os("BLESS").is_some() {
        let (tu, yaml, json, _) = produce(fixture);
        std::fs::write(pipeline::zephyr_golden_path(fixture), tu).expect("write golden.zephyr.c");
        std::fs::write(pipeline::zephyr_binding_path(fixture), yaml)
            .expect("write golden.zephyr.binding.yaml");
        std::fs::write(pipeline::zephyr_conformance_path(fixture), json)
            .expect("write conformance.zephyr.json");
        return;
    }

    // The zephyr OS row (stage-attributed), byte-locking all THREE artifacts.
    let result = pipeline::run_zephyr_fixture(fixture);
    assert!(
        result.is_green(),
        "pl011-absentslot-zephyr must pass bridge -> emit_zephyr_c -> snapshot: {:?}",
        result.verdict
    );

    let tu = std::fs::read_to_string(pipeline::zephyr_golden_path(fixture))
        .expect("read golden.zephyr.c");
    let pos = |needle: &str| {
        tu.find(needle)
            .unwrap_or_else(|| panic!("TU must contain `{needle}`"))
    };
    let row = &capability::CAP_ZEPHYR_DT_SURPRISE_1;

    // -- exactly one C04 audit block, grammar verbatim, citing the ratified
    //    row (R02; generated from the same const as the report row, I03).
    assert_eq!(tu.matches("RSEQ-ABSENT-SLOT:").count(), 1, "{tu}");
    pos("/* RSEQ-ABSENT-SLOT: on_surprise_remove");
    pos(&format!("* disposition: AbsentDispatch row: {}", row.id));
    pos("body checked, not delivered: dispatch cannot occur on this target. */");

    // -- non-emission: no surprise symbol, no call site; the delivered
    //    on_init is still fully present.
    assert!(!tu.contains("pl011_absentslot_on_surprise_remove"), "{tu}");
    pos("pl011_absentslot_on_init(&data->inner)");
    // The absent body's LOG_WRN never reaches the TU (D06: the discarded
    // lowering has no observable effect); the delivered LOG_INF does.
    pos("LOG_INF(\"pl011 absent-slot witness up\");");
    assert!(
        !tu.contains("LOG_WRN") && !tu.contains("witness: surprise removal"),
        "the absent body's log line leaked into the TU: {tu}"
    );

    // -- the single-instance guard stands (fieldless: no map, no A-1).
    pos("BUILD_ASSERT(DT_NUM_INST_STATUS_OKAY(DT_DRV_COMPAT) == 1");
    assert!(!tu.contains("int main("), "no main in a Zephyr driver TU");

    // -- the conformance JSON golden: the absent_dispatch row (R04) with
    //    block row ID == report row ID (I03).
    let conf = std::fs::read_to_string(pipeline::zephyr_conformance_path(fixture))
        .expect("read conformance.zephyr.json");
    assert!(
        conf.contains(&format!(
            "\"slot\": \"on_surprise_remove\", \"row\": \"{}\"",
            row.id
        )),
        "{conf}"
    );
    assert!(conf.contains("\"rejected_slots\": []"), "{conf}");

    // -- the R05 summary line, plan-fixed wording, printed and asserted
    //    from the producer's own struct; then the I05 double-run: produce
    //    twice in-process, byte-compare all three artifacts.
    let (tu1, yaml1, json1, emit1) = produce(fixture);
    let line = emit1.conformance.summary_line();
    println!("{line}");
    assert_eq!(
        line,
        "RSEQ-CONFORMANCE [zephyr_c @ v4.4.0 (684c9e8f32e4)] delivered=[on_init] \
         absent_dispatch=[on_surprise_remove:CAP-ZEPHYR-DT-SURPRISE-1] structurally_absent=[] rejected=[] ignored=[]"
    );
    let (tu2, yaml2, json2, _) = produce(fixture);
    assert_eq!(tu1, tu2, "TU must be byte-stable across producer runs");
    assert_eq!(yaml1, yaml2, "binding yaml must be byte-stable");
    assert_eq!(json1, json2, "conformance JSON must be byte-stable");
}
