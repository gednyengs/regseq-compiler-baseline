//! C5: one authored PL011 spec and one authored E1000 spec, each unchanged
//! across Linux and Zephyr. The target backends own class and driver scaffold.

#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use codegen_coverage_runner::kbuild::{self, KbuildOutcome};
use codegen_coverage_runner::manifest::{self, OsBinding};
use codegen_coverage_runner::pipeline;
use codegen_coverage_runner::zephyr_build::{
    self, StagedBus, StagedDriver, WestOutcome,
};
use regseq_model::{AttrData, IntrinsicKind, TargetOs};
use serde_json::{json, Value};
use test_ir_fixtures::{try_bridge_fixture_path, BridgeOutcome};

const AUTHORITY_SHA256: &str =
    "708f08fcf18f934041d98e23f9811103fca59bea58418680c5395d2201e3bf17";
const ADDENDUM_LOCK_SHA256: &str =
    "52996affe191766dbbae4cd7a1eaa782a829d541f5ea9572af213d10f619c41a";

fn package_root() -> PathBuf { PathBuf::from(env!("CARGO_MANIFEST_DIR")) }
fn repo_root() -> PathBuf { package_root().join("../..").canonicalize().unwrap() }
fn corpus() -> PathBuf { package_root().join("fixtures/_composition") }
fn generated_dir() -> PathBuf {
    repo_root().join("experiments/os-api-gap-closure/generated/C5-composition")
}

fn fixture(name: &str) -> manifest::Fixture {
    manifest::discover(&corpus())
        .expect("discover C5 composition corpus")
        .into_iter()
        .find(|fixture| fixture.manifest.name == name)
        .unwrap_or_else(|| panic!("missing C5 fixture {name}"))
}

struct NativePair {
    linux: regseq_backends::LinuxEmit,
    zephyr: regseq_backends::ZephyrEmit,
}

fn emit_pair(name: &str) -> NativePair {
    let base = fixture(name);
    let source = std::fs::read(&base.input).expect("read authored input");

    let linux_fixture = base.with_binding(OsBinding::Linux).expect("Linux binding");
    let mut linux_model = match try_bridge_fixture_path(&linux_fixture.input) {
        BridgeOutcome::Ok(model) => model,
        other => panic!("{name} Linux bridge failed: {other:?}"),
    };
    let linux_identity = pipeline::bind_l2_platform(&mut linux_model, &linux_fixture)
        .expect("bind Linux platform");
    assert_eq!(linux_identity.os, Some(TargetOs::Linux));

    let zephyr_fixture = base.with_binding(OsBinding::Zephyr).expect("Zephyr binding");
    let mut zephyr_model = match try_bridge_fixture_path(&zephyr_fixture.input) {
        BridgeOutcome::Ok(model) => model,
        other => panic!("{name} Zephyr bridge failed: {other:?}"),
    };
    let zephyr_identity = pipeline::bind_l2_platform(&mut zephyr_model, &zephyr_fixture)
        .expect("bind Zephyr platform");
    assert_eq!(zephyr_identity.os, Some(TargetOs::Zephyr));

    assert_eq!(
        std::fs::read(&base.input).expect("re-read authored input"),
        source,
        "target selection must not rewrite the authored spec",
    );

    match name {
        "c5-pl011-rich" => {
            assert_eq!(linux_model.serial_port.drivers.len(), 1);
            assert_eq!(linux_model.dma.drivers.len(), 1);
            assert_eq!(zephyr_model.serial_port.drivers.len(), 1);
            assert_eq!(zephyr_model.dma.drivers.len(), 1);
            for model in [&linux_model, &zephyr_model] {
                let serial = model.serial_port.drivers.values().next().unwrap();
                assert_eq!(
                    serial.ops.capabilities().collect::<Vec<_>>(),
                    [
                        "configure",
                        "get_config",
                        "transmit",
                        "receive",
                        "open",
                        "close",
                        "await_drain",
                    ]
                );
                let dma = model.dma.drivers.values().next().unwrap();
                assert!(dma.buffer.is_none(), "PL011 must not invent a DMA buffer");
                assert_eq!(
                    dma.channel
                        .as_ref()
                        .unwrap()
                        .ops
                        .capabilities()
                        .collect::<Vec<_>>(),
                    [
                        "acquire",
                        "configure",
                        "prepare",
                        "submit",
                        "activate",
                        "release",
                        "terminate",
                        "synchronize",
                    ]
                );
            }
        }
        "c5-e1000-rich" => {
            assert_eq!(linux_model.network_interface.drivers.len(), 1);
            assert_eq!(zephyr_model.network_interface.drivers.len(), 1);
            for model in [&linux_model, &zephyr_model] {
                let network = model.network_interface.drivers.values().next().unwrap();
                assert_eq!(
                    network.ops.capabilities().collect::<Vec<_>>(),
                    [
                        "set_admin_enabled",
                        "transmit",
                        "receive",
                        "report_carrier",
                        "query_operational_state",
                        "get_property",
                        "read_counters",
                    ]
                );
            }
            assert!(linux_model
                .domain_attrs
                .item_attrs
                .iter()
                .flatten()
                .any(|attr| matches!(
                    attr,
                    AttrData::Intrinsic(IntrinsicKind::CommandTransportSubmit)
                )));
        }
        _ => unreachable!(),
    }

    NativePair {
        linux: regseq_backends::emit_linux_c(&linux_model).expect("Linux native emit"),
        zephyr: regseq_backends::emit_zephyr_c(&zephyr_model).expect("Zephyr native emit"),
    }
}

fn sha256(bytes: &[u8]) -> String {
    use std::io::Write;

    let mut child = Command::new("sha256sum")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn sha256sum");
    child.stdin.take().unwrap().write_all(bytes).expect("write sha256 input");
    let output = child.wait_with_output().expect("wait for sha256sum");
    assert!(output.status.success(), "sha256sum failed");
    String::from_utf8(output.stdout)
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .to_owned()
}

fn output_path(device: &str, target: &str) -> PathBuf {
    generated_dir().join(format!("{device}.{target}.c"))
}

fn generated_products() -> Vec<(&'static str, &'static str, String)> {
    let pl011 = emit_pair("c5-pl011-rich");
    let e1000 = emit_pair("c5-e1000-rich");
    vec![
        ("pl011", "linux", pl011.linux.tu),
        ("pl011", "zephyr", pl011.zephyr.tu),
        ("e1000", "linux", e1000.linux.tu),
        ("e1000", "zephyr", e1000.zephyr.tu),
    ]
}

fn manifest_value(products: &[(&str, &str, String)]) -> Value {
    let sources = ["c5-pl011-rich", "c5-e1000-rich"]
        .into_iter()
        .map(|name| {
            let path = fixture(name).input;
            let rel = path.strip_prefix(repo_root()).unwrap();
            let bytes = std::fs::read(&path).unwrap();
            json!({"path": rel, "sha256": sha256(&bytes)})
        })
        .collect::<Vec<_>>();
    let outputs = products
        .iter()
        .map(|(device, target, text)| {
            let path = output_path(device, target);
            let rel = path.strip_prefix(repo_root()).unwrap();
            json!({
                "device": device,
                "target": target,
                "path": rel,
                "sha256": sha256(text.as_bytes()),
            })
        })
        .collect::<Vec<_>>();
    json!({
        "schema": "gap-closure-generated-manifest/1",
        "stage": "C5-composition",
        "authority_sha256": AUTHORITY_SHA256,
        "addendum_lock_sha256": ADDENDUM_LOCK_SHA256,
        "sources": sources,
        "outputs": outputs,
    })
}

fn write_or_verify_products(regenerate: bool) {
    if regenerate {
        let products = generated_products();
        let manifest = manifest_value(&products);
        let manifest_bytes = format!("{}\n", serde_json::to_string_pretty(&manifest).unwrap());
        std::fs::create_dir_all(generated_dir()).expect("create generated output directory");
        for (device, target, text) in &products {
            std::fs::write(output_path(device, target), text).expect("write native output");
        }
        std::fs::write(generated_dir().join("MANIFEST.json"), &manifest_bytes)
            .expect("write generated manifest");
    }
    verify_persisted_manifest();
}

fn verify_persisted_manifest() {
    let manifest: Value = serde_json::from_slice(
        &std::fs::read(generated_dir().join("MANIFEST.json")).expect("read generated manifest"),
    )
    .expect("parse generated manifest");
    assert_eq!(manifest["schema"], "gap-closure-generated-manifest/1");
    assert_eq!(manifest["stage"], "C5-composition");
    assert_eq!(manifest["authority_sha256"], AUTHORITY_SHA256);
    assert_eq!(manifest["addendum_lock_sha256"], ADDENDUM_LOCK_SHA256);
    assert_eq!(manifest["sources"].as_array().unwrap().len(), 2);
    assert_eq!(manifest["outputs"].as_array().unwrap().len(), 4);
    for group in ["sources", "outputs"] {
        for entry in manifest[group].as_array().unwrap() {
            let path = repo_root().join(entry["path"].as_str().unwrap());
            let bytes = std::fs::read(&path)
                .unwrap_or_else(|error| panic!("read manifest path {}: {error}", path.display()));
            assert_eq!(sha256(&bytes), entry["sha256"].as_str().unwrap(), "{}", path.display());
        }
    }
}

#[test]
fn regenerate_four_outputs_and_manifest() {
    write_or_verify_products(std::env::var_os("RSEQ_REGENERATE").is_some());
}

#[test]
fn rich_specs_compile_unchanged_and_keep_scaffold_backend_owned() {
    write_or_verify_products(false);
    let pl011_linux = std::fs::read_to_string(output_path("pl011", "linux")).unwrap();
    let pl011_zephyr = std::fs::read_to_string(output_path("pl011", "zephyr")).unwrap();
    let e1000_linux = std::fs::read_to_string(output_path("e1000", "linux")).unwrap();
    let e1000_zephyr = std::fs::read_to_string(output_path("e1000", "zephyr")).unwrap();

    for needle in ["c5_pl011_on_init", "C5Pl011", "PL011_IBRD"] {
        assert!(pl011_linux.contains(needle), "Linux lost authored PL011 logic: {needle}");
        assert!(pl011_zephyr.contains(needle), "Zephyr lost authored PL011 logic: {needle}");
    }
    assert!(pl011_linux.contains("struct uart_ops"));
    assert!(pl011_linux.contains("dma_request_chan("));
    assert!(pl011_linux.contains("module_amba_driver(c5_pl011_driver)"));
    assert!(pl011_zephyr.contains("DEVICE_API(uart,"));
    assert!(pl011_zephyr.contains("dma_request_channel("));
    assert!(pl011_zephyr.contains("DEVICE_DT_INST_DEINIT_DEFINE("));
    assert!(!pl011_linux.contains("DEVICE_DT_INST_DEFINE"));
    assert!(!pl011_zephyr.contains("module_amba_driver"));

    for needle in ["c5_e1000_on_init", "C5E1000", "E1000_TDT"] {
        assert!(e1000_linux.contains(needle), "Linux lost authored E1000 logic: {needle}");
        assert!(e1000_zephyr.contains(needle), "Zephyr lost authored E1000 logic: {needle}");
    }
    assert!(e1000_linux.contains("struct net_device_ops"));
    assert!(e1000_linux.contains("mbox_send_message("));
    assert!(e1000_linux.contains("module_pci_driver(c5_e1000_driver)"));
    assert!(e1000_zephyr.contains("net_if_send_data("));
    assert!(e1000_zephyr.contains("mbox_send("));
    assert!(e1000_zephyr.contains("DEVICE_PCIE_INST_DECLARE("));
    assert!(e1000_zephyr.contains("DEVICE_DT_INST_DEINIT_DEFINE("));
    assert!(!e1000_linux.contains("DEVICE_PCIE_INST_DECLARE"));
    assert!(!e1000_zephyr.contains("module_pci_driver"));
}

#[test]
fn full_coverage_and_negative_capability_matrices() {
    let coverage: Value = serde_json::from_slice(
        &std::fs::read(repo_root().join(
            "experiments/os-api-gap-closure/coverage/C5-composition.json",
        ))
        .unwrap(),
    )
    .unwrap();
    assert_eq!(coverage["schema"], "gap-closure-composition-coverage/1");
    assert_eq!(coverage["authority_sha256"], AUTHORITY_SHA256);
    assert_eq!(coverage["addendum_lock_sha256"], ADDENDUM_LOCK_SHA256);
    assert_eq!(coverage["compiler_baseline"], "canonical-v3");
    assert_eq!(coverage["construct_epochs"]["device-class-serial"], 1);
    assert_eq!(coverage["construct_epochs"]["dma-management"], 1);
    assert_eq!(coverage["construct_epochs"]["device-command-transport"], 2);
    assert_eq!(coverage["construct_epochs"]["network-interface-management"], 1);
    let pl011 = coverage["denominators"]["pl011"].as_array().unwrap();
    let e1000 = coverage["denominators"]["e1000"].as_array().unwrap();
    assert_eq!(pl011.len(), 21);
    assert_eq!(e1000.len(), 60);
    assert_eq!(pl011.first().unwrap(), "PL011-S01");
    assert_eq!(pl011.last().unwrap(), "PL011-S21");
    assert_eq!(e1000.first().unwrap(), "E1000-S01");
    assert_eq!(e1000.last().unwrap(), "E1000-S60");

    let pairs = coverage["target_matrix"].as_array().unwrap();
    assert_eq!(pairs.len(), 4);
    for (device, target, rows) in [
        ("pl011", "linux", 21),
        ("pl011", "zephyr", 21),
        ("e1000", "linux", 60),
        ("e1000", "zephyr", 60),
    ] {
        let pair = pairs
            .iter()
            .find(|pair| pair["device"] == device && pair["target"] == target)
            .unwrap_or_else(|| panic!("missing {device} x {target} coverage pair"));
        assert_eq!(pair["frozen_rows"], rows);
        assert!(repo_root().join(pair["source"].as_str().unwrap()).is_file());
        assert!(repo_root().join(pair["output"].as_str().unwrap()).is_file());
    }

    let frozen: Value = serde_json::from_slice(
        &std::fs::read(
            "/home/nyengele/regseq-eval-worktrees/regseq-gap-closure/docs/experiments/os-api-gap-closure/freeze/FEATURE-COVERAGE-MAP.json",
        )
        .expect("read frozen feature coverage authority"),
    )
    .unwrap();
    assert_eq!(frozen["active_epoch"], 1);
    let lock: Value = serde_json::from_slice(
        &std::fs::read(
            "/home/nyengele/regseq-eval-worktrees/regseq-gap-closure/docs/experiments/os-api-gap-closure/addendum/LOCK.json",
        )
        .expect("read locked construct-epoch authority"),
    )
    .unwrap();
    assert_eq!(lock["schema"], "gap-closure-addendum-lock/1");
    assert_eq!(lock["sha256"], ADDENDUM_LOCK_SHA256);
    assert_eq!(coverage["construct_epochs"], lock["construct_epochs"]);
    for (device, local) in [("pl011", pl011), ("e1000", e1000)] {
        let frozen_ids = frozen["devices"][device]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| row["row_id"].as_str().unwrap())
            .collect::<Vec<_>>();
        let local_ids = local.iter().map(|id| id.as_str().unwrap()).collect::<Vec<_>>();
        assert_eq!(local_ids, frozen_ids, "{device} denominator drift");
    }

    for (stage, count) in [("C1-serial", 21), ("C2-DMA", 15), ("C3-DCT", 6), ("C4-NIM", 20)] {
        let value: Value = serde_json::from_slice(
            &std::fs::read(repo_root().join(format!(
                "experiments/os-api-gap-closure/coverage/{stage}.json"
            )))
            .unwrap(),
        )
        .unwrap();
        assert_eq!(value["rows"].as_array().unwrap().len(), count, "{stage}");
    }

    let dma = fixture_from("_dma", "dma-buffer-e1000");
    let dma_model = bridge(&dma.input);
    let dma_error = regseq_backends::emit_zephyr_c(&dma_model)
        .expect_err("Zephyr must preserve owned-buffer allocation absence");
    assert!(dma_error.to_string().contains("DmaBuffer owned allocation is unsupported"));

    let network = fixture_from("_network", "network-full");
    let network_model = bridge(&network.input);
    let publication_error = regseq_backends::emit_zephyr_c(&network_model)
        .expect_err("Zephyr must preserve runtime publication absence");
    assert!(publication_error.to_string().contains("runtime publication is unsupported"));

    for (_, _, text) in generated_products() {
        for fabricated in ["command_complete", "correlation_id", "packet_canceled", "packet_completed"] {
            assert!(!text.contains(fabricated), "generated output fabricated {fabricated}");
        }
    }
    let negatives = coverage["negative_capabilities"].as_array().unwrap();
    assert_eq!(negatives.len(), 4);
    assert!(negatives.iter().all(|row| row["expected"] == "rejected" || row["expected"] == "absent"));
    let authored_absences = coverage["authored_absence_matrix"].as_array().unwrap();
    assert_eq!(authored_absences.len(), 4);
    assert_eq!(
        authored_absences
            .iter()
            .map(|row| (row["device"].as_str().unwrap(), row["target"].as_str().unwrap()))
            .collect::<Vec<_>>(),
        [
            ("pl011", "linux"),
            ("pl011", "zephyr"),
            ("e1000", "linux"),
            ("e1000", "zephyr"),
        ]
    );
    for row in authored_absences {
        match row["device"].as_str().unwrap() {
            "pl011" => {
                assert_eq!(row["serial"].as_array().unwrap().len(), 5);
                assert_eq!(row["dma"].as_array().unwrap().len(), 8);
            }
            "e1000" => {
                assert_eq!(row["network"].as_array().unwrap().len(), 5);
                assert_eq!(row["transport"].as_array().unwrap().len(), 3);
            }
            device => panic!("unexpected authored-absence device {device}"),
        }
    }
}

fn fixture_from(corpus_name: &str, name: &str) -> manifest::Fixture {
    manifest::discover(&package_root().join("fixtures").join(corpus_name))
        .unwrap()
        .into_iter()
        .find(|fixture| fixture.manifest.name == name)
        .unwrap()
}

fn bridge(path: &Path) -> regseq_model::RegseqModel {
    match try_bridge_fixture_path(path) {
        BridgeOutcome::Ok(model) => model,
        other => panic!("bridge failed for {}: {other:?}", path.display()),
    }
}

fn visible(message: impl AsRef<str>) {
    use std::io::Write;
    let _ = writeln!(std::io::stderr().lock(), "{}", message.as_ref());
}

#[test]
fn rich_linux_outputs_build_against_real_headers() {
    for (label, emit) in [
        ("pl011", emit_pair("c5-pl011-rich").linux),
        ("e1000", emit_pair("c5-e1000-rich").linux),
    ] {
        match kbuild::kbuild_module(&emit.tu, &emit.kbuild, &emit.module_name) {
            Ok(KbuildOutcome::Skipped { notice }) => visible(notice),
            Ok(KbuildOutcome::Built { kernel_version, warnings }) => {
                visible(format!("C5 {label} kbuild PASS against {kernel_version}"));
                if !warnings.trim().is_empty() { visible(warnings); }
            }
            Err(error) => panic!("C5 {label} kbuild failed: {error}"),
        }
    }
}

#[test]
fn rich_zephyr_outputs_build_against_real_headers() {
    let pl011 = emit_pair("c5-pl011-rich").zephyr;
    let e1000 = emit_pair("c5-e1000-rich").zephyr;
    let drivers = [
        StagedDriver {
            tu: &pl011.tu,
            binding_yaml: &pl011.binding_yaml,
            driver_name: &pl011.driver_name,
            compatible: "regseq,c5-pl011",
            bus: StagedBus::Platform { region: (0x0904_0000, 0x1000), interrupts: None },
        },
        StagedDriver {
            tu: &e1000.tu,
            binding_yaml: &e1000.binding_yaml,
            driver_name: &e1000.driver_name,
            compatible: "regseq,c5-e1000",
            bus: StagedBus::Pcie { vendor_id: 0x8086, device_id: 0x100e },
        },
    ];
    for driver in &drivers {
        match zephyr_build::west_build(driver) {
            Ok(WestOutcome::Skipped { notice }) => visible(notice),
            Ok(WestOutcome::Built { zephyr_version, warnings }) => {
                visible(format!("C5 west PASS against Zephyr {zephyr_version}"));
                if !warnings.trim().is_empty() { visible(warnings); }
            }
            Err(error) => panic!("C5 west build failed: {error}"),
        }
    }
}
