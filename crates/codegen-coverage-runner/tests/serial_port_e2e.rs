#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::{Path, PathBuf};

use codegen_coverage_runner::{manifest, pipeline, toolchain};
use test_ir_fixtures::{try_bridge_fixture_path, BridgeOutcome};

fn corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/_serial")
}

#[test]
fn empty_serial_port_impl_preserves_every_capability_as_absent() {
    let input =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/_lifecycle/lifecycle-pl011/input.rs");
    let model = match try_bridge_fixture_path(&input) {
        BridgeOutcome::Ok(model) => model,
        other => panic!("empty serial-port bridge failed: {other:?}"),
    };
    assert_eq!(model.serial_port.drivers.len(), 1);
    assert!(model
        .serial_port
        .drivers
        .values()
        .next()
        .expect("one serial port")
        .ops
        .is_empty());
}

#[test]
fn serial_port_all_ops_generic_c_behavior() {
    let fixtures = manifest::discover(&corpus()).expect("discover serial corpus");
    let fixture = fixtures
        .iter()
        .find(|fixture| fixture.manifest.name == "serial-port-all")
        .expect("serial-port-all fixture");
    let mut model = match try_bridge_fixture_path(&fixture.input) {
        BridgeOutcome::Ok(model) => model,
        other => panic!("serial-port bridge failed: {other:?}"),
    };
    assert_eq!(model.serial_port.drivers.len(), 1);
    let lifecycle = model.device_lifecycle.drivers.values().next().unwrap();
    assert_eq!(
        lifecycle
            .handle
            .user_data_fields
            .iter()
            .map(|field| model.strings.resolve(field.name))
            .collect::<Vec<_>>(),
        vec!["state", "tx_byte", "rx_byte"]
    );
    let serial = model.serial_port.drivers.values().next().unwrap();
    assert_eq!(
        serial.ops.capabilities().collect::<Vec<_>>(),
        vec![
            "configure",
            "get_config",
            "transmit",
            "receive",
            "set_signals",
            "get_signals",
            "subscribe_events",
            "open",
            "close",
            "purge",
            "await_drain",
            "abort",
        ]
    );
    let identity = pipeline::bind_l2_platform(&mut model, fixture).expect("no-op L2 bind");
    let options = pipeline::backend_options_for(fixture).expect("backend options");
    let context = regseq_backends::BackendContext::new(options, identity);
    let emit = regseq_backends::emit_generic_c_for_entry_with_context(
        &model,
        &fixture.manifest.entry,
        &context,
    )
    .expect("generic C serial emit");
    assert!(emit.tu.contains("RSEQ-SERIAL-PORT serial_port_all"));
    assert!(emit.tu.contains("capability: purge"));
    assert!(emit.tu.contains("capability: abort"));
    assert!(emit.tu.contains("no terminal event is implied"));
    let run = toolchain::compile_and_run(&emit.tu, &emit.shim, Path::new("cc"))
        .expect("compile and run generic C serial behavior");
    assert_eq!(run.exit_code, Some(0), "stderr: {}", run.stderr);
}

#[test]
fn serial_capability_absence_has_no_fabricated_event_or_success() {
    let fixtures = manifest::discover(&corpus()).expect("discover serial corpus");
    let fixture = fixtures
        .iter()
        .find(|fixture| fixture.manifest.name == "serial-port-minimal")
        .expect("serial-port-minimal fixture");
    let mut model = match try_bridge_fixture_path(&fixture.input) {
        BridgeOutcome::Ok(model) => model,
        other => panic!("minimal serial bridge failed: {other:?}"),
    };
    let serial = model.serial_port.drivers.values().next().unwrap();
    assert_eq!(
        serial.ops.capabilities().collect::<Vec<_>>(),
        vec!["transmit"]
    );
    assert!(serial.ops.subscribe_events.is_none());
    assert!(serial.ops.abort.is_none());
    assert!(serial.ops.await_drain.is_none());

    let identity = pipeline::bind_l2_platform(&mut model, fixture).expect("no-op L2 bind");
    let options = pipeline::backend_options_for(fixture).expect("backend options");
    let context = regseq_backends::BackendContext::new(options, identity);
    let emit = regseq_backends::emit_generic_c_for_entry_with_context(
        &model,
        &fixture.manifest.entry,
        &context,
    )
    .expect("minimal generic C serial emit");
    assert!(emit.tu.contains("capability: transmit"));
    assert!(!emit.tu.contains("capability: subscribe_events"));
    assert!(!emit.tu.contains("capability: abort"));
    assert!(emit
        .tu
        .contains("absent operations are unsupported; no terminal event is implied"));
    let run = toolchain::compile_and_run(&emit.tu, &emit.shim, Path::new("cc"))
        .expect("compile and run minimal generic C serial behavior");
    assert_eq!(run.exit_code, Some(0), "stderr: {}", run.stderr);
}
