#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::{Path, PathBuf};

use codegen_coverage_runner::manifest::{self, Expect};
use codegen_coverage_runner::{pipeline, toolchain};
use test_ir_fixtures::{try_bridge_fixture_path, BridgeOutcome};

fn corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/_network")
}

fn fixture(name: &str) -> manifest::Fixture {
    manifest::discover(&corpus())
        .expect("discover network corpus")
        .into_iter()
        .find(|fixture| fixture.manifest.name == name)
        .unwrap_or_else(|| panic!("missing network fixture {name}"))
}

fn model(name: &str) -> regseq_model::RegseqModel {
    let fixture = fixture(name);
    match try_bridge_fixture_path(&fixture.input) {
        BridgeOutcome::Ok(model) => model,
        other => panic!("{name} bridge failed: {other:?}"),
    }
}

#[test]
fn locked_network_positive_frontend_has_full_surface_and_composed_owners() {
    let model = model("network-full");
    let interface = model
        .network_interface
        .drivers
        .values()
        .next()
        .expect("one network interface");
    assert_eq!(interface.ops.capabilities().count(), 12);
    assert_eq!(model.dma.drivers.len(), 1, "DMA remains a sibling owner");
    assert_eq!(model.interrupt.drivers.len(), 1, "IRQ remains a sibling owner");
    assert_eq!(
        model.deferred_work.drivers.len(),
        1,
        "deferred work remains a sibling owner"
    );
    assert!(model
        .domain_attrs
        .item_attrs
        .iter()
        .flatten()
        .any(|attr| matches!(attr, regseq_model::AttrData::Intrinsic(
            regseq_model::IntrinsicKind::CommandTransportSubmit
        ))));

    let network_json = serde_json::to_string(&model.network_interface).unwrap();
    for forbidden in [
        "descriptor",
        "producer_index",
        "consumer_index",
        "dma_mapping",
        "interrupt",
        "poll_budget",
        "work_queue",
        "per_packet_completion",
    ] {
        assert!(
            !network_json.contains(forbidden),
            "NIM IR absorbed L2 mechanism {forbidden}"
        );
    }
}

#[test]
fn locked_network_generic_c_executes_packet_and_composed_dma_dct_behavior() {
    let fixture = fixture("network-full");
    let mut model = model("network-full");
    let bodyless_core_defaults = model
        .instances
        .iter()
        .filter_map(|instance| {
            let data = instance.kind.as_function()?;
            let path = model
                .strings
                .resolve(model.items[instance.item].def_path.rendered);
            (data.body.is_none()
                && path.starts_with("core::default::")
                && path.contains("::{impl#")
                && path.ends_with("::default"))
            .then_some(path)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        bodyless_core_defaults.len(),
        1,
        "the full NIM fixture must expose exactly its u64 Default::default backend shim"
    );
    let identity = pipeline::bind_l2_platform(&mut model, &fixture).expect("network L2 bind");
    let context = regseq_backends::BackendContext::new(
        pipeline::backend_options_for(&fixture).expect("backend options"),
        identity,
    );
    let emit = regseq_backends::emit_generic_c_for_entry_with_context(
        &model,
        &fixture.manifest.entry,
        &context,
    )
    .expect("emit generic-C network fixture");
    for needle in [
        "RSEQ-NETWORK-INTERFACE",
        "RSEQ-NIM-CAPABILITY transmit",
        "RSEQ-NIM-ABSENCE no-fabricated-packet-completion",
        "RSEQ-NIM-BOUNDARY packet-layout/rings/indices/DMA/IRQ/work-remain-L2",
        "RSEQ-DMA",
        "regseq_command_transport_submit",
    ] {
        assert!(emit.tu.contains(needle), "missing generic-C proof {needle}");
    }
    let run = toolchain::compile_and_run(&emit.tu, &emit.shim, Path::new("cc"))
        .expect("compile/run generic-C network fixture");
    assert_eq!(run.exit_code, Some(0), "{}", run.stderr);
}

#[test]
fn locked_network_negative_frontend_corpus_is_exact() {
    let fixtures = manifest::discover(&corpus()).expect("discover network corpus");
    let rejects = fixtures
        .iter()
        .filter(|fixture| matches!(fixture.manifest.expect, Expect::Reject))
        .collect::<Vec<_>>();
    assert_eq!(rejects.len(), 3, "the locked NIM negative corpus is exact");
    for fixture in rejects {
        let result = pipeline::run_fixture(fixture, Path::new("cc"));
        assert!(
            result.is_green(),
            "{}: {:?}",
            fixture.manifest.name,
            result.verdict
        );
    }
}

#[test]
fn network_capability_absence_is_data_not_success() {
    let mut model = model("network-full");
    let interface = model.network_interface.drivers.values_mut().next().unwrap();
    interface.ops.set_published = None;
    interface.ops.receive = None;
    interface.ops.report_dormant = None;
    interface.ops.set_property = None;
    interface.ops.register_observer = None;
    interface.ops.unregister_observer = None;
    assert_eq!(
        interface.ops.capabilities().collect::<Vec<_>>(),
        [
            "set_admin_enabled",
            "transmit",
            "report_carrier",
            "query_operational_state",
            "get_property",
            "read_counters",
        ]
    );
}
