#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::{Path, PathBuf};

use codegen_coverage_runner::{manifest, pipeline, toolchain};
use regseq_model::{AttrData, IntrinsicKind};
use test_ir_fixtures::{try_bridge_fixture_path, BridgeOutcome};

fn corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/_transport")
}

fn fixture(name: &str) -> manifest::Fixture {
    manifest::discover(&corpus())
        .expect("discover transport corpus")
        .into_iter()
        .find(|fixture| fixture.manifest.name == name)
        .unwrap_or_else(|| panic!("missing transport fixture {name}"))
}

fn bridge(name: &str) -> regseq_model::RegseqModel {
    let fixture = fixture(name);
    match try_bridge_fixture_path(&fixture.input) {
        BridgeOutcome::Ok(model) => model,
        other => panic!("{name} bridge failed: {other:?}"),
    }
}

#[test]
fn locked_transport_positive_frontend() {
    let model = bridge("transport-admission");
    let submits = model
        .domain_attrs
        .item_attrs
        .iter()
        .flatten()
        .filter(|attr| {
            matches!(
                attr,
                AttrData::Intrinsic(IntrinsicKind::CommandTransportSubmit)
            )
        })
        .count();
    assert_eq!(submits, 1, "the locked surface has exactly one operation");

    let serialized = serde_json::to_string(&model).unwrap();
    for forbidden in [
        "WorkQueue",
        "CommandSlot",
        "reserve_slot",
        "publish",
        "harvest",
        "completion_id",
        "correlation_id",
        "cancel_command",
    ] {
        assert!(
            !serialized.contains(forbidden),
            "locked transport IR must not contain historical surface {forbidden}"
        );
    }
}

#[test]
fn locked_transport_negative_frontend() {
    let fixtures = manifest::discover(&corpus()).expect("discover transport corpus");
    let rejects: Vec<_> = fixtures
        .iter()
        .filter(|fixture| matches!(fixture.manifest.expect, manifest::Expect::Reject))
        .collect();
    assert_eq!(rejects.len(), 3, "the locked negative corpus is exact");
    for fixture in rejects {
        let result = pipeline::run_fixture(fixture, Path::new("cc"));
        assert!(
            result.is_green(),
            "reject fixture {:?} must reject as declared: {:?}",
            fixture.manifest.name,
            result.verdict
        );
    }
}

#[test]
fn locked_transport_generic_c_admission_has_no_terminal_event() {
    let fixture = fixture("transport-admission");
    let model = bridge("transport-admission");
    let options = pipeline::backend_options_for(&fixture).expect("backend options");
    let context =
        regseq_backends::BackendContext::new(options, regseq_model::PlatformIdentity::default());
    let emit = regseq_backends::emit_generic_c_for_entry_with_context(
        &model,
        &fixture.manifest.entry,
        &context,
    )
    .expect("emit locked transport");

    assert!(emit.tu.contains("regseq_command_transport_submit"));
    for forbidden in [
        "reserve_slot",
        "publish_command",
        "harvest_command",
        "command_complete",
        "command_cancel",
        "correlation_id",
    ] {
        assert!(
            !emit.tu.contains(forbidden),
            "generic C must not fabricate {forbidden}"
        );
    }

    let run = toolchain::compile_and_run(&emit.tu, &emit.shim, Path::new("cc"))
        .expect("compile and run locked transport");
    assert_eq!(
        run.exit_code,
        Some(0),
        "generated program failed: {}",
        run.stderr
    );
    assert_eq!(
        run.stdout,
        "LC register dctadmit match=dt:regseq,dct-admit\n\
LC create\n\
LC associate\n\
LC call on_init\n\
DCT admission accepted endpoint=1 payload=17\n\
DCT admission rejected reason=-16 endpoint=0 payload=34\n\
LC teardown present=1\n\
LC call on_deinit\n"
    );
}
