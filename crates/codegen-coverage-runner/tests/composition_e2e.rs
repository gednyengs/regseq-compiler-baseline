//! C5: one authored PL011 spec and one authored E1000 spec, each unchanged
//! across Linux and Zephyr. The target backends own class and driver scaffold.

#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use codegen_coverage_runner::kbuild::{self, KbuildOutcome};
use codegen_coverage_runner::manifest::{self, OsBinding};
use codegen_coverage_runner::pipeline;
use codegen_coverage_runner::toolchain;
use codegen_coverage_runner::zephyr_build::{self, StagedBus, StagedDriver, WestOutcome};
use regseq_driver::bridge::BridgeError;
use regseq_model::{AttrData, IntrinsicKind, TargetOs};
use serde_json::{json, Value};
use test_ir_fixtures::{try_bridge_fixture_path, BridgeOutcome};

const AUTHORITY_SHA256: &str = "708f08fcf18f934041d98e23f9811103fca59bea58418680c5395d2201e3bf17";
const ADDENDUM_LOCK_SHA256: &str =
    "52996affe191766dbbae4cd7a1eaa782a829d541f5ea9572af213d10f619c41a";
const PL011_ORACLE_SHA256: &str =
    "3dac37719c67f04a5cac23aab2e86c8adb31d78a0a2e15c6e7a95b2d4b66dec6";
const EVAL_ROOT_VAR: &str = "RSEQ_EVAL_ROOT";

fn package_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}
fn repo_root() -> PathBuf {
    package_root().join("../..").canonicalize().unwrap()
}
fn corpus() -> PathBuf {
    package_root().join("fixtures/_composition")
}
fn generated_dir() -> PathBuf {
    repo_root().join("experiments/os-api-gap-closure/generated/C5-composition")
}

fn eval_root() -> PathBuf {
    eval_root_from_pin(std::env::var_os(EVAL_ROOT_VAR))
}

fn eval_root_from_pin(pin: Option<std::ffi::OsString>) -> PathBuf {
    PathBuf::from(
        pin.unwrap_or_else(|| panic!("{EVAL_ROOT_VAR} must name the pinned regseq eval worktree")),
    )
}

fn pl011_spec_dir() -> PathBuf {
    eval_root().join("pl011/spec")
}

fn pl011_producer_model() -> PathBuf {
    pl011_spec_dir().join("rich-pl011.rs")
}

fn e1000_spec_dir() -> PathBuf {
    eval_root().join("e1000/spec")
}

fn e1000_producer_model() -> PathBuf {
    e1000_spec_dir().join("rich-e1000.rs")
}

fn fixture(name: &str) -> manifest::Fixture {
    let mut fixture = manifest::discover(&corpus())
        .expect("discover C5 composition corpus")
        .into_iter()
        .find(|fixture| fixture.manifest.name == name)
        .unwrap_or_else(|| panic!("missing C5 fixture {name}"));
    match name {
        "c5-pl011-rich" => fixture.input = pl011_producer_model(),
        "c5-e1000-rich" => fixture.input = e1000_producer_model(),
        _ => {}
    }
    fixture
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
    let linux_identity =
        pipeline::bind_l2_platform(&mut linux_model, &linux_fixture).expect("bind Linux platform");
    assert_eq!(linux_identity.os, Some(TargetOs::Linux));

    let zephyr_fixture = base
        .with_binding(OsBinding::Zephyr)
        .expect("Zephyr binding");
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
            assert_eq!(zephyr_model.serial_port.drivers.len(), 1);
            for model in [&linux_model, &zephyr_model] {
                let serial = model.serial_port.drivers.values().next().unwrap();
                assert_eq!(
                    serial.ops.capabilities().collect::<Vec<_>>(),
                    [
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
                        "set_property",
                        "read_counters",
                    ]
                );
            }
            assert_eq!(linux_model.interrupt.drivers.len(), 1);
            assert_eq!(zephyr_model.interrupt.drivers.len(), 1);
            assert_eq!(linux_model.deferred_work.drivers.len(), 1);
            assert_eq!(zephyr_model.deferred_work.drivers.len(), 1);
            assert_eq!(linux_model.dma.drivers.len(), 1);
            assert_eq!(zephyr_model.dma.drivers.len(), 1);
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
    child
        .stdin
        .take()
        .unwrap()
        .write_all(bytes)
        .expect("write sha256 input");
    let output = child.wait_with_output().expect("wait for sha256sum");
    assert!(output.status.success(), "sha256sum failed");
    String::from_utf8(output.stdout)
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .to_owned()
}

fn manifest_path(path: &Path) -> String {
    if let Ok(relative) = path.strip_prefix(repo_root()) {
        return relative.to_string_lossy().into_owned();
    }
    if let Ok(relative) = path.strip_prefix(eval_root()) {
        return format!("${EVAL_ROOT_VAR}/{}", relative.to_string_lossy());
    }
    path.to_string_lossy().into_owned()
}

fn resolve_manifest_path(path: &str) -> PathBuf {
    path.strip_prefix(&format!("${EVAL_ROOT_VAR}/"))
        .map(|relative| eval_root().join(relative))
        .unwrap_or_else(|| repo_root().join(path))
}

fn output_path(device: &str, target: &str) -> PathBuf {
    let file_name = match target {
        "linux" => format!("{device}_linux.c"),
        _ => format!("{device}.{target}.c"),
    };
    generated_dir().join(file_name)
}

fn product_output_path(device: &str, target: &str, role: &str) -> PathBuf {
    match role {
        "source" => output_path(device, target),
        "negative_source" => generated_dir().join(format!("{device}_{target}_initfail.c")),
        "binding" => generated_dir().join(format!("{device}_{target}.binding.yaml")),
        "negative_binding" => {
            generated_dir().join(format!("{device}_{target}_initfail.binding.yaml"))
        }
        "oracle" if target == "generic_c" => generated_dir().join(format!("{device}_oracle.trace")),
        _ => panic!("unexpected generated product role {role}"),
    }
}

fn emit_oracle(name: &str) -> String {
    let fixture = fixture(name)
        .with_binding(OsBinding::Linux)
        .expect("Linux oracle binding");
    let options = pipeline::backend_options_for(&fixture).expect("oracle backend options");
    let mut model = match try_bridge_fixture_path(&fixture.input) {
        BridgeOutcome::Ok(model) => model,
        other => panic!("{name} oracle bridge failed: {other:?}"),
    };
    let identity = pipeline::bind_l2_platform(&mut model, &fixture).expect("bind oracle platform");
    let context = regseq_backends::BackendContext::new(options, identity);
    let emitted = regseq_backends::emit_generic_c_for_entry_with_context(
        &model,
        &fixture.manifest.entry,
        &context,
    )
    .expect("generic_c oracle emit");
    let run = toolchain::compile_and_run(&emitted.tu, &emitted.shim, Path::new("cc"))
        .expect("generic_c oracle compile and run");
    assert_eq!(
        run.exit_code,
        Some(0),
        "{name} oracle exited unsuccessfully"
    );
    assert!(
        run.stderr.is_empty(),
        "{name} oracle stderr: {}",
        run.stderr
    );
    run.stdout
}

fn emit_linux(name: &str) -> String {
    let fixture = fixture(name)
        .with_binding(OsBinding::Linux)
        .expect("Linux binding");
    let mut model = match try_bridge_fixture_path(&fixture.input) {
        BridgeOutcome::Ok(model) => model,
        other => panic!("{name} Linux bridge failed: {other:?}"),
    };
    pipeline::bind_l2_platform(&mut model, &fixture).expect("bind Linux platform");
    regseq_backends::emit_linux_c(&model)
        .expect("Linux native emit")
        .tu
}

fn emit_zephyr(name: &str) -> regseq_backends::ZephyrEmit {
    let fixture = fixture(name)
        .with_binding(OsBinding::Zephyr)
        .expect("Zephyr binding");
    let mut model = match try_bridge_fixture_path(&fixture.input) {
        BridgeOutcome::Ok(model) => model,
        other => panic!("{name} Zephyr bridge failed: {other:?}"),
    };
    pipeline::bind_l2_platform(&mut model, &fixture).expect("bind Zephyr platform");
    regseq_backends::emit_zephyr_c(&model).expect("Zephyr native emit")
}

fn emit_composed_pair(name: &str) -> NativePair {
    let base = fixture_from("_composed", name);
    let linux_fixture = base.with_binding(OsBinding::Linux).expect("Linux binding");
    let mut linux_model = match try_bridge_fixture_path(&linux_fixture.input) {
        BridgeOutcome::Ok(model) => model,
        other => panic!("{name} Linux bridge failed: {other:?}"),
    };
    pipeline::bind_l2_platform(&mut linux_model, &linux_fixture).expect("bind Linux platform");

    let zephyr_fixture = base
        .with_binding(OsBinding::Zephyr)
        .expect("Zephyr binding");
    let mut zephyr_model = match try_bridge_fixture_path(&zephyr_fixture.input) {
        BridgeOutcome::Ok(model) => model,
        other => panic!("{name} Zephyr bridge failed: {other:?}"),
    };
    pipeline::bind_l2_platform(&mut zephyr_model, &zephyr_fixture).expect("bind Zephyr platform");

    NativePair {
        linux: regseq_backends::emit_linux_c(&linux_model).expect("Linux native emit"),
        zephyr: regseq_backends::emit_zephyr_c(&zephyr_model).expect("Zephyr native emit"),
    }
}

fn generated_products() -> Vec<(&'static str, &'static str, &'static str, String)> {
    let pl011 = emit_pair("c5-pl011-rich");
    let pl011_initfail_linux = emit_linux("c5-pl011-linux-initfail");
    let pl011_initfail_zephyr = emit_zephyr("c5-pl011-linux-initfail");
    let e1000 = emit_pair("c5-e1000-rich");
    let e1000_initfail = emit_composed_pair("e1000-initfail");
    let pl011_oracle = emit_oracle("c5-pl011-rich");
    let e1000_oracle = emit_oracle("c5-e1000-rich");
    vec![
        ("pl011", "linux", "source", pl011.linux.tu),
        ("pl011", "linux", "negative_source", pl011_initfail_linux),
        ("pl011", "zephyr", "source", pl011.zephyr.tu),
        (
            "pl011",
            "zephyr",
            "negative_source",
            pl011_initfail_zephyr.tu,
        ),
        ("pl011", "zephyr", "binding", pl011.zephyr.binding_yaml),
        ("pl011", "generic_c", "oracle", pl011_oracle),
        ("e1000", "linux", "source", e1000.linux.tu),
        ("e1000", "linux", "negative_source", e1000_initfail.linux.tu),
        ("e1000", "zephyr", "source", e1000.zephyr.tu),
        (
            "e1000",
            "zephyr",
            "negative_source",
            e1000_initfail.zephyr.tu,
        ),
        ("e1000", "zephyr", "binding", e1000.zephyr.binding_yaml),
        (
            "e1000",
            "zephyr",
            "negative_binding",
            e1000_initfail.zephyr.binding_yaml,
        ),
        ("e1000", "generic_c", "oracle", e1000_oracle),
    ]
}

fn manifest_value(products: &[(&str, &str, &str, String)]) -> Value {
    let sources = [
        fixture("c5-pl011-rich").input,
        fixture("c5-pl011-linux-initfail").input,
        fixture("c5-e1000-rich").input,
        fixture_from("_composed", "e1000-initfail").input,
    ]
    .into_iter()
    .map(|path| {
        let bytes = std::fs::read(&path).unwrap();
        json!({"path": manifest_path(&path), "sha256": sha256(&bytes)})
    })
    .collect::<Vec<_>>();
    let producer_models = [
        ("pl011", pl011_producer_model()),
        ("e1000", e1000_producer_model()),
    ]
    .into_iter()
    .map(|(device, path)| {
        let bytes = std::fs::read(&path).unwrap();
        json!({
            "device": device,
            "path": manifest_path(&path),
            "sha256": sha256(&bytes),
        })
    })
    .collect::<Vec<_>>();
    let outputs = products
        .iter()
        .map(|(device, target, role, text)| {
            let path = product_output_path(device, target, role);
            json!({
                "device": device,
                "target": target,
                "role": role,
                "path": manifest_path(&path),
                "sha256": sha256(text.as_bytes()),
            })
        })
        .collect::<Vec<_>>();
    json!({
        "schema": "gap-closure-generated-manifest/1",
        "stage": "C5-composition",
        "authority_sha256": AUTHORITY_SHA256,
        "addendum_lock_sha256": ADDENDUM_LOCK_SHA256,
        "producer_models": producer_models,
        "sources": sources,
        "outputs": outputs,
    })
}

fn write_or_verify_products(regenerate: bool) {
    static PRODUCT_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _guard = PRODUCT_LOCK.lock().expect("lock generated products");
    let regenerate = regenerate || std::env::var_os("RSEQ_REGENERATE").is_some();
    if regenerate {
        let products = generated_products();
        let manifest = manifest_value(&products);
        let manifest_bytes = format!("{}\n", serde_json::to_string_pretty(&manifest).unwrap());
        std::fs::create_dir_all(generated_dir()).expect("create generated output directory");
        for (device, target, role, text) in &products {
            std::fs::write(product_output_path(device, target, role), text)
                .expect("write native output");
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
    assert_eq!(manifest["producer_models"].as_array().unwrap().len(), 2);
    assert_eq!(manifest["sources"].as_array().unwrap().len(), 4);
    assert_eq!(manifest["outputs"].as_array().unwrap().len(), 13);
    for group in ["producer_models", "sources", "outputs"] {
        for entry in manifest[group].as_array().unwrap() {
            let path = resolve_manifest_path(entry["path"].as_str().unwrap());
            let bytes = std::fs::read(&path)
                .unwrap_or_else(|error| panic!("read manifest path {}: {error}", path.display()));
            assert_eq!(
                sha256(&bytes),
                entry["sha256"].as_str().unwrap(),
                "{}",
                path.display()
            );
        }
    }
}

#[test]
fn generated_manifest_records_the_models_actually_consumed() {
    let manifest = manifest_value(&[]);
    let producer_models = manifest["producer_models"].as_array().unwrap();
    assert_eq!(producer_models.len(), 2);

    let pl011 = producer_models
        .iter()
        .find(|entry| entry["device"] == "pl011")
        .unwrap();
    assert_eq!(pl011["path"], manifest_path(&pl011_producer_model()));
    assert_eq!(
        pl011["sha256"],
        sha256(&std::fs::read(pl011_producer_model()).unwrap())
    );

    let e1000 = producer_models
        .iter()
        .find(|entry| entry["device"] == "e1000")
        .unwrap();
    let e1000_path = e1000_producer_model();
    assert_eq!(e1000["path"], manifest_path(&e1000_path));
    assert_eq!(e1000["sha256"], sha256(&std::fs::read(e1000_path).unwrap()));
}

#[test]
#[should_panic(expected = "RSEQ_EVAL_ROOT must name the pinned regseq eval worktree")]
fn eval_spec_consumption_fails_closed_without_pin() {
    let _ = eval_root_from_pin(None);
}

#[test]
fn pl011_composed_bridge_fires_register_block_extraction_check() {
    let source_path = pl011_producer_model();
    assert!(
        matches!(try_bridge_fixture_path(&source_path), BridgeOutcome::Ok(_)),
        "marked PL011 register block must bridge",
    );

    let marker = "#[rseq::register_block(endian = \"little\", access_width = 32)]";
    let source = std::fs::read_to_string(&source_path).expect("read PL011 producer model");
    assert_eq!(
        source.matches(marker).count(),
        1,
        "PL011 producer must carry exactly one canonical register-block marker",
    );

    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system time after Unix epoch")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "regseq-pl011-register-block-check-{}-{nonce}",
        std::process::id(),
    ));
    std::fs::create_dir_all(&dir).expect("create register-block check directory");
    let unmarked_path = dir.join("rich-pl011-unmarked.rs");
    std::fs::write(&unmarked_path, source.replacen(marker, "", 1))
        .expect("write unmarked PL011 producer model");
    std::fs::copy(pl011_spec_dir().join("surface.rs"), dir.join("surface.rs"))
        .expect("copy PL011 facade");

    let outcome = try_bridge_fixture_path(&unmarked_path);
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        matches!(
            outcome,
            BridgeOutcome::Rejected(BridgeError::RegisterAccessValidation {
                ref driver,
                ref field,
                ..
            }) if driver == "rich-pl011" && field == "regs"
        ),
        "removing only the register-block marker must fire the composed extraction check: {outcome:?}",
    );
}

#[test]
fn e1000_composed_bridge_fires_register_block_extraction_check() {
    let source_path = e1000_producer_model();
    assert!(
        matches!(try_bridge_fixture_path(&source_path), BridgeOutcome::Ok(_)),
        "marked E1000 register block must bridge",
    );

    let marker = "#[rseq::register_block(endian = \"little\", access_width = 32)]";
    let source = std::fs::read_to_string(&source_path).expect("read E1000 producer model");
    assert_eq!(
        source.matches(marker).count(),
        1,
        "E1000 producer must carry exactly one canonical register-block marker",
    );

    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system time after Unix epoch")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "regseq-e1000-register-block-check-{}-{nonce}",
        std::process::id(),
    ));
    std::fs::create_dir_all(&dir).expect("create register-block check directory");
    let unmarked_path = dir.join("rich-e1000-unmarked.rs");
    std::fs::write(&unmarked_path, source.replacen(marker, "", 1))
        .expect("write unmarked E1000 producer model");
    std::fs::copy(e1000_spec_dir().join("surface.rs"), dir.join("surface.rs"))
        .expect("copy E1000 facade");

    let outcome = try_bridge_fixture_path(&unmarked_path);
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        matches!(
            outcome,
            BridgeOutcome::Rejected(BridgeError::RegisterAccessValidation {
                ref driver,
                ref field,
                ..
            }) if driver == "rich-e1000" && field == "regs"
        ),
        "removing only the register-block marker must fire the composed extraction check: {outcome:?}",
    );
}

#[test]
fn regenerate_products_and_manifest() {
    write_or_verify_products(std::env::var_os("RSEQ_REGENERATE").is_some());
}

#[test]
fn register_access_port_preserves_device_access_oracles() {
    for (name, device) in [("c5-pl011-rich", "pl011"), ("c5-e1000-rich", "e1000")] {
        let actual = emit_oracle(name);
        let expected = std::fs::read_to_string(product_output_path(device, "generic_c", "oracle"))
            .expect("read persisted oracle trace");
        assert_eq!(
            actual, expected,
            "{device} device-access oracle changed during the register-access port",
        );
    }
}

#[test]
fn pl011_named_bitfields_preserve_locked_mmio_values() {
    let trace = emit_oracle("c5-pl011-rich");
    assert_eq!(sha256(trace.as_bytes()), PL011_ORACLE_SHA256);
    assert_eq!(
        trace
            .lines()
            .filter(|line| line.starts_with('R') || line.starts_with('W'))
            .collect::<Vec<_>>(),
        [
            "W32 @0x9040030 = 0x00000000",
            "W32 @0x9040044 = 0x000007ff",
            "W32 @0x9040034 = 0x00000012",
            "W32 @0x9040038 = 0x00000000",
            "W32 @0x9040044 = 0x000007ff",
            "W32 @0x9040030 = 0x00000000",
        ],
    );
}

#[test]
fn persisted_linux_product_names_are_kbuild_safe() {
    for path in [
        output_path("pl011", "linux"),
        product_output_path("pl011", "linux", "negative_source"),
        output_path("e1000", "linux"),
        product_output_path("e1000", "linux", "negative_source"),
    ] {
        let module_name = path.file_stem().unwrap().to_str().unwrap();
        assert!(
            module_name
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || character == '_'),
            "persisted Linux product basename `{module_name}` is not Kbuild-safe",
        );
    }
}

fn emitted_function<'a>(tu: &'a str, symbol: &str) -> &'a str {
    let needle = format!(" {symbol}(");
    let start = tu
        .match_indices(&needle)
        .map(|(start, _)| start)
        .find(|start| {
            let candidate = &tu[*start..];
            let line_end = candidate.find('\n').unwrap_or(candidate.len());
            candidate[..line_end].ends_with(')')
                || candidate
                    .find("\n{")
                    .is_some_and(|open| !candidate[..open].contains(';'))
        })
        .unwrap_or_else(|| panic!("missing emitted function definition {symbol}"));
    let body = &tu[start..];
    let end = body
        .find("\n}\n")
        .unwrap_or_else(|| panic!("unterminated emitted function {symbol}"));
    &body[..end]
}

fn register_write<'a>(body: &'a str, register: &str) -> &'a str {
    body.lines()
        .find(|line| line.contains(register) && line.contains("write"))
        .unwrap_or_else(|| panic!("missing write to {register} in:\n{body}"))
}

#[test]
fn pl011_native_configure_mmio_consumes_runtime_params_and_state() {
    let pair = emit_pair("c5-pl011-rich");
    for (target, tu) in [
        ("linux", pair.linux.tu.as_str()),
        ("zephyr", pair.zephyr.tu.as_str()),
    ] {
        let configure = emitted_function(&tu, "rich_pl011_serial_configure");
        for field in ["baud_rate", "data_bits", "stop_bits", "parity"] {
            assert!(
                configure.contains(&format!("_2.{field}")),
                "{target} configure does not consume runtime params.{field}:\n{configure}"
            );
        }
        for register in ["RICH_PL011_IBRD", "RICH_PL011_FBRD", "RICH_PL011_LCRH"] {
            let write = register_write(configure, register);
            assert!(
                !write.contains("(0x"),
                "{target} {register} write was constant-folded instead of consuming the runtime calculation: {write}"
            );
        }

        let open = emitted_function(&tu, "rich_pl011_serial_open");
        assert!(open.contains("rich_pl011_set_interrupts"));
        let set_interrupts = emitted_function(&tu, "rich_pl011_set_interrupts");
        let imsc = register_write(set_interrupts, "RICH_PL011_IMSC");
        assert!(
            set_interrupts.contains("imsc_shadow") && !imsc.contains("(0x0u)"),
            "{target} interrupt helper lost the runtime shadow RMW: {imsc}\n{set_interrupts}"
        );
    }
}

#[test]
fn e1000_native_outputs_preserve_the_approved_enrichment_slice() {
    let pair = emit_pair("c5-e1000-rich");
    for (target, tu) in [
        ("linux", pair.linux.tu.as_str()),
        ("zephyr", pair.zephyr.tu.as_str()),
    ] {
        let mac = emitted_function(&tu, "rich_e1000_program_mac");
        for octet in 0..6 {
            assert!(
                mac.contains(&format!(".octet{octet}")),
                "{target} MAC programming lost runtime octet {octet}:\n{mac}"
            );
        }
        for register in ["RICH_E1000_RAL0", "RICH_E1000_RAH0"] {
            assert!(
                mac.contains(register),
                "{target} MAC programming lost {register}"
            );
        }

        let property = emitted_function(&tu, "rich_e1000_set_property");
        for needle in [
            "rich_e1000_program_mac",
            "rich_e1000_receive_control",
            "rich_e1000_set_admin_enabled",
            "0x44u",
            "0x2328u",
        ] {
            assert!(
                property.contains(needle),
                "{target} runtime MTU/MAC/filter property path lost {needle}:\n{property}"
            );
        }

        let transmit = emitted_function(&tu, "rich_e1000_transmit");
        for needle in [
            "admin_enabled",
            "carrier",
            "rich_e1000_native_identity",
            "RICH_E1000_TDT",
        ] {
            assert!(
                transmit.contains(needle),
                "{target} carrier-gated TX path lost {needle}:\n{transmit}"
            );
        }

        let rings = emitted_function(&tu, "rich_e1000_allocate_rings");
        for needle in [
            "0x1000ull",
            "0x80ull",
            "rich_e1000_transfer_address",
            "rich_e1000_program_rings",
            "rich_e1000_post_receive_ring",
        ] {
            assert!(
                rings.contains(needle),
                "{target} validated dual-ring path lost {needle}:\n{rings}"
            );
        }
        assert!(
            !rings.contains("0x2000") && !rings.contains("0x4000"),
            "{target} retained fabricated descriptor-ring addresses:\n{rings}"
        );

        let service = emitted_function(&tu, "rich_e1000_service_interrupt");
        for register in ["RICH_E1000_STATUS", "RICH_E1000_IMS"] {
            assert!(
                service.contains(register),
                "{target} deferred service lost {register}:\n{service}"
            );
        }
        for harvest in ["rich_e1000_harvest_tx", "rich_e1000_harvest_rx"] {
            assert!(
                service.contains(harvest),
                "{target} deferred service lost {harvest}:\n{service}"
            );
        }
        assert!(
            service.contains("rich_e1000_set_itr"),
            "{target} deferred service lost adaptive ITR recalculation:\n{service}"
        );
        assert!(
            !service.contains("RICH_E1000_ICS"),
            "{target} deferred service retained the unconditional ICS self-kick:\n{service}"
        );
        let counters = emitted_function(&tu, "rich_e1000_read_counters");
        assert!(counters.contains("tx_packets") && counters.contains("rx_packets"));

        let irq = emitted_function(&tu, "rich_e1000_handle_irq");
        assert!(
            irq.contains("RICH_E1000_ICR"),
            "{target} ISR lost RICH_E1000_ICR:\n{irq}"
        );
        match target {
            "linux" => {
                assert!(
                    irq.contains("RICH_E1000_IMC"),
                    "Linux ISR lost RICH_E1000_IMC:\n{irq}"
                );
                assert_eq!(
                    tu.matches("struct work_struct __rseq_work_").count(),
                    0,
                    "Linux NAPI lowering retained a generic data work item"
                );
                assert!(
                    irq.contains("rich_e1000_schedule_napi(__rseq_priv, true)"),
                    "Linux ISR did not schedule NAPI:\n{irq}"
                );
            }
            "zephyr" => {
                assert_eq!(
                    tu.matches("struct k_work __rseq_work_").count(),
                    0,
                    "Zephyr's stock-shaped E1000 ISR must not retain a workqueue hop"
                );
                assert!(
                    irq.contains("rich_e1000_harvest_tx(&__rseq_data->inner, 64u)")
                        && irq.contains("rich_e1000_harvest_rx(&__rseq_data->inner, 64u)"),
                    "Zephyr ISR did not harvest the descriptor path synchronously:\n{irq}"
                );
                assert!(
                    !irq.contains("RICH_E1000_IMC")
                        && !irq.contains("rich_e1000_service_interrupt"),
                    "Zephyr ISR retained deferred-service masking or dispatch:\n{irq}"
                );
            }
            _ => unreachable!(),
        }
    }
    assert!(pair
        .linux
        .tu
        .contains("dma_set_mask_and_coherent(&pdev->dev, DMA_BIT_MASK(32))"));
    assert!(pair.linux.tu.contains("request_irq(pdev->irq"));
    assert!(pair.linux.tu.contains("IRQF_SHARED"));
    assert!(pair.zephyr.tu.contains("IRQ_CONNECT(DT_INST_IRQN(0)"));
}

#[test]
fn e1000_zephyr_uses_stock_ring8_geometry_without_shrinking_dma_backing() {
    let pair = emit_pair("c5-e1000-rich");
    let zephyr = pair.zephyr.tu;

    for field in [
        "size_t __rseq_tx_lengths[8];",
        "uint8_t __rseq_tx_buffers[8][2048]",
        "uint8_t __rseq_rx_buffers[8][2048]",
    ] {
        assert!(zephyr.contains(field), "Zephyr ring8 data lost {field}");
    }
    let command = emitted_function(&zephyr, "rich_e1000_descriptor_command");
    assert!(command.contains("__command->slot < 8"));
    for name in [
        "rich_e1000_post_receive_ring",
        "rich_e1000_harvest_tx",
        "rich_e1000_harvest_rx",
        "rich_e1000_transmit",
    ] {
        let function = emitted_function(&zephyr, name);
        assert!(
            function.contains("0x8u"),
            "{name} did not consume ring8:\n{function}"
        );
        assert!(
            !function.contains("0x100u"),
            "{name} retained the shared Linux ring size:\n{function}"
        );
    }
    let program = emitted_function(&zephyr, "rich_e1000_program_rings");
    assert!(program.contains("0x80ull"));
    assert!(!program.contains("0x1000ull"));
    let allocate = emitted_function(&zephyr, "rich_e1000_allocate_rings");
    assert!(allocate.contains("0x1000ull"));
    assert!(zephyr.contains("uint8_t __rseq_dma_cpu[2][4096] __aligned(128);"));

    let itr = emitted_function(&zephyr, "rich_e1000_set_itr");
    assert!(
        itr.contains("0x100u"),
        "adaptive ITR scaling is not ring geometry and must stay unchanged"
    );

    let linux = pair.linux.tu;
    assert!(emitted_function(&linux, "rich_e1000_post_receive_ring").contains("0x100u"));
    assert!(emitted_function(&linux, "rich_e1000_program_rings").contains("0x1000ull"));
}

#[test]
fn e1000_tx_descriptors_are_packed_before_tdt_publication() {
    let pair = emit_pair("c5-e1000-rich");
    for (target, tu, descriptor_writes, barrier) in [
        (
            "linux",
            pair.linux.tu.as_str(),
            [
                "__ring[__data_slot].buffer_address = cpu_to_le64(__address);",
                "__ring[__data_slot].length = cpu_to_le16(__segment_len);",
                "__ring[__data_slot].command =",
                "__ring[__data_slot].status = 0;",
            ],
            "dma_wmb();",
        ),
        (
            "zephyr",
            pair.zephyr.tu.as_str(),
            [
                "__ring[__slot].buffer_address = sys_cpu_to_le64(__address);",
                "__ring[__slot].length = sys_cpu_to_le16(__length);",
                "__ring[__slot].command = 0x0b;",
                "__ring[__slot].status = 0;",
            ],
            "barrier_dmem_fence_full();",
        ),
    ] {
        let command = emitted_function(tu, "rich_e1000_descriptor_command");
        let mut previous = 0;
        for write in descriptor_writes {
            let position = command
                .find(write)
                .unwrap_or_else(|| panic!("{target} descriptor pack lost {write}:\n{command}"));
            assert!(
                position >= previous,
                "{target} descriptor fields are not packed in device layout order"
            );
            previous = position;
        }
        let barrier_position = command[previous..]
            .find(barrier)
            .map(|position| previous + position)
            .unwrap_or_else(|| {
                panic!("{target} descriptor publication lost {barrier}:\n{command}")
            });
        assert!(
            previous < barrier_position,
            "{target} descriptor became publishable before its content was written"
        );

        let transmit = emitted_function(tu, "rich_e1000_transmit");
        let submit = transmit
            .find("rich_e1000_descriptor_command")
            .expect("transmit does not publish through the DCT seam");
        let advance = transmit
            .find(".tx_tail =")
            .expect("transmit does not advance the producer");
        let doorbell = transmit
            .find("RICH_E1000_TDT")
            .expect("transmit does not ring TDT");
        assert!(
            submit < advance && advance < doorbell,
            "{target} TDT was rung before descriptor publication completed:\n{transmit}"
        );
    }

    let transmit = emitted_function(&pair.linux.tu, "rich_e1000_transmit");
    for needle in [
        "!netdev_xmit_more()",
        "RICH_E1000_TX_DOORBELL_BATCH",
        "tx_tail % RICH_E1000_TX_DOORBELL_BATCH",
        "netif_xmit_stopped(netdev_get_tx_queue(__rseq_priv->netdev, 0))",
        "RICH_E1000_TDT",
    ] {
        assert!(
            transmit.contains(needle),
            "Linux stock TDT batching lost {needle}:\n{transmit}"
        );
    }
    let guard = transmit
        .find("!netdev_xmit_more()")
        .expect("Linux TDT end-of-chain guard");
    let doorbell = transmit
        .find("RICH_E1000_TDT")
        .expect("Linux TDT publication");
    assert!(
        guard < doorbell,
        "Linux must evaluate all native batching flush conditions before TDT publication:\n{transmit}"
    );
    assert!(
        !pair.zephyr.tu.contains("netdev_xmit_more"),
        "Linux-native TDT batching leaked into Zephyr"
    );
}

#[test]
fn e1000_linux_tx_checksum_offload_uses_authored_context_and_data_descriptors() {
    let pair = emit_pair("c5-e1000-rich");
    let linux = pair.linux.tu;
    let command = emitted_function(&linux, "rich_e1000_descriptor_command");
    let transmit = emitted_function(&linux, "rich_e1000_transmit");

    for needle in [
        "struct TxChecksumContextDescriptor",
        "struct TxChecksumDataDescriptor",
        "CHECKSUM_PARTIAL",
        "skb_checksum_start_offset(__packet)",
        "skb_headlen(__packet)",
        "skb_shinfo(__packet)->nr_frags",
        "skb_frag_dma_map",
        "dma_unmap_page",
        "__packet->csum_offset",
        "__command->checksum_context",
        "__command->checksum_data",
        "__command->descriptor_count = __count",
        ".command_length = 0x20000000, .segmentation = 0x0",
        ".command_length = 0xab100000, .status = 0x0, .packet_options = 0x2",
    ] {
        assert!(
            linux.contains(needle),
            "Linux checksum-offload lowering lost {needle}"
        );
    }
    let context_write = command
        .find("__context->transport_checksum_start")
        .expect("checksum context start is written");
    let data_write = command
        .find("__data->packet_options")
        .expect("checksum data TXSM option is written");
    let publication = command[data_write..]
        .find("dma_wmb();")
        .map(|position| data_write + position)
        .expect("checksum descriptor publication barrier");
    assert!(
        context_write < data_write && data_write < publication,
        "checksum context and data descriptors must be complete before publication"
    );
    for needle in [
        "transport_checksum_start",
        "transport_checksum_offset",
        "transport_checksum_end",
        "packet_options",
        "__done_slot = (__slot + __count - 1u) % 256u",
        "for (__cleanup = 0u; __cleanup < __count; __cleanup++)",
        "for (__cleanup = 0u; __cleanup < __mapped; __cleanup++)",
        "dma_wmb();",
    ] {
        assert!(
            command.contains(needle),
            "Linux checksum descriptors lost {needle}:\n{command}"
        );
    }
    assert!(
        transmit.contains("descriptor_count") && transmit.contains("RICH_E1000_TDT"),
        "authored transmit path must advance TDT by the accepted descriptor count:\n{transmit}"
    );
    for needle in [
        "priv->netdev->hw_features = NETIF_F_HW_CSUM | NETIF_F_SG;",
        "priv->netdev->vlan_features = NETIF_F_HW_CSUM | NETIF_F_SG;",
        "priv->netdev->features |= NETIF_F_HW_CSUM | NETIF_F_SG;",
    ] {
        assert!(
            linux.contains(needle),
            "feature advertisement lost {needle}"
        );
    }
    assert!(
        !linux.contains("hw_features = NETIF_F_IP_CSUM"),
        "Linux must use the generic checksum capability"
    );
    assert!(
        !pair.zephyr.tu.contains("CHECKSUM_PARTIAL")
            && !pair.zephyr.tu.contains("checksum-offload lowering"),
        "Zephyr WZ has no checksum-offload feature contract"
    );
}

#[test]
fn e1000_rx_buffers_are_posted_and_replenished_before_rdt_publication() {
    let pair = emit_pair("c5-e1000-rich");
    for (target, tu, buffer_address, barrier) in [
        (
            "linux",
            pair.linux.tu.as_str(),
            "__ring[__slot].buffer_address = cpu_to_le64(__address);",
            "dma_wmb();",
        ),
        (
            "zephyr",
            pair.zephyr.tu.as_str(),
            "__ring[__slot].buffer_address = sys_cpu_to_le64(__address);",
            "barrier_dmem_fence_full();",
        ),
    ] {
        let command = emitted_function(tu, "rich_e1000_descriptor_command");
        let rx_post = command
            .find("__command->operation == 3")
            .expect("RX post command is absent");
        let address = command[rx_post..]
            .find(buffer_address)
            .map(|position| position + rx_post)
            .unwrap_or_else(|| panic!("{target} RX post lost its buffer address:\n{command}"));
        let visible = command[address..]
            .find(barrier)
            .map(|position| position + address)
            .unwrap_or_else(|| panic!("{target} RX post lost its DMA visibility barrier"));
        assert!(rx_post < address && address < visible);

        let prepost = emitted_function(tu, "rich_e1000_post_receive_ring");
        assert!(
            prepost.contains("rich_e1000_post_receive") && prepost.contains("RICH_E1000_RDT"),
            "{target} init does not pre-post RX buffers before publishing RDT:\n{prepost}"
        );

        let replenish = emitted_function(tu, "rich_e1000_harvest_rx");
        let harvest = replenish
            .find("rich_e1000_descriptor_command")
            .expect("RX completion is not harvested");
        let post = replenish[harvest + 1..]
            .find("rich_e1000_post_receive")
            .map(|position| position + harvest + 1)
            .expect("RX buffer is not replenished after harvest");
        let doorbell = replenish
            .find("RICH_E1000_RDT")
            .expect("RX replenishment does not publish RDT");
        assert!(
            harvest < post && post < doorbell,
            "{target} RDT was published before RX replenishment completed:\n{replenish}"
        );
    }
}

#[test]
fn e1000_carrier_and_receive_are_composed_without_ics_self_kick() {
    let pair = emit_pair("c5-e1000-rich");
    let service = emitted_function(&pair.linux.tu, "rich_e1000_service_interrupt");
    assert!(service.contains("netif_carrier_on(__rseq_priv->netdev)"));
    assert!(service.contains("netif_carrier_off(__rseq_priv->netdev)"));
    let receive = emitted_function(&pair.linux.tu, "rich_e1000_harvest_rx");
    let classify = receive
        .find("__packet->protocol = eth_type_trans(__packet, __rseq_priv->netdev)")
        .expect("Linux RX handoff did not establish the Ethernet protocol and MAC header");
    let deliver = receive
        .find("napi_gro_receive(&__rseq_priv->napi, __packet)")
        .expect("Linux NAPI RX handoff did not use the NAPI/GRO delivery path");
    assert!(
        !receive.contains("netif_rx(__packet)") && !receive.contains("netif_receive_skb(__packet)"),
        "Linux NAPI RX handoff retained a non-NAPI delivery path:\n{receive}"
    );
    assert!(
        !(pair.linux.tu.contains("schedule_work(") && pair.linux.tu.contains("netif_receive_skb(")),
        "netif_receive_skb must never be emitted from workqueue service context"
    );
    assert!(
        classify < deliver,
        "Linux delivered the RX packet before Ethernet classification:\n{receive}"
    );

    let service = emitted_function(&pair.zephyr.tu, "rich_e1000_service_interrupt");
    assert!(service.contains("net_if_carrier_on(__rseq_data->__rseq_net_iface)"));
    assert!(service.contains("net_if_carrier_off(__rseq_data->__rseq_net_iface)"));
    let receive = emitted_function(&pair.zephyr.tu, "rich_e1000_harvest_rx");
    assert!(receive.contains("net_recv_data(__rseq_data->__rseq_net_iface, __packet)"));

    for (target, tu) in [
        ("linux", pair.linux.tu.as_str()),
        ("zephyr", pair.zephyr.tu.as_str()),
    ] {
        let service = emitted_function(tu, "rich_e1000_service_interrupt");
        assert!(
            !service.contains("RICH_E1000_ICS"),
            "{target} service path retained the unconditional ICS self-kick:\n{service}"
        );
    }
}

#[test]
fn e1000_linux_napi_lowering_is_budgeted_and_rearms_only_after_completion() {
    let pair = emit_pair("c5-e1000-rich");
    let linux = pair.linux.tu;

    for needle in [
        "struct napi_struct napi;",
        "bool __rseq_napi_enabled;",
        "struct delayed_work __rseq_link_work;",
        "u64 __rseq_itr_window_tx_packets;",
        "u64 __rseq_itr_window_rx_packets;",
        "u64 __rseq_itr_window_tx_bytes;",
        "u64 __rseq_itr_window_rx_bytes;",
        "static __always_inline bool rich_e1000_descriptor_command",
        "netif_napi_add(priv->netdev, &priv->napi, rich_e1000_napi_poll);",
        "napi_enable(&priv->napi);",
        "INIT_DELAYED_WORK(&priv->__rseq_link_work, rich_e1000_link_work);",
        "napi_disable(&priv->napi);",
        "netif_napi_del(&priv->napi);",
    ] {
        assert!(linux.contains(needle), "Linux NAPI lowering lost {needle}");
    }
    for forbidden in [
        "struct work_struct __rseq_work_",
        "schedule_work(&__rseq_priv->__rseq_work_",
        "cancel_work_sync(&priv->__rseq_work_",
    ] {
        assert!(
            !linux.contains(forbidden),
            "composed network service retained workqueue lowering {forbidden}"
        );
    }

    let isr = emitted_function(&linux, "rich_e1000_handle_irq");
    let mask = isr
        .find("RICH_E1000_IMC")
        .expect("ISR did not mask device interrupts");
    let flush = isr[mask..]
        .find("readl(__rseq_priv->inner.regs + RICH_E1000_STATUS)")
        .map(|position| mask + position)
        .expect("ISR did not flush the posted IMC write");
    let schedule = isr[flush..]
        .find("rich_e1000_schedule_napi(__rseq_priv, true)")
        .map(|position| flush + position)
        .expect("ISR did not schedule NAPI after flushing IMC");
    let lsc = isr
        .find("schedule_delayed_work(&__rseq_priv->__rseq_link_work, 1)")
        .expect("ICR.LSC did not schedule independent link work");
    assert!(
        lsc < mask && mask < flush && flush < schedule,
        "ISR must dispatch LSC, mask, flush IMC, and only then schedule NAPI:\n{isr}"
    );

    let schedule_napi = emitted_function(&linux, "rich_e1000_schedule_napi");
    let prepare = schedule_napi
        .find("napi_schedule_prep(&priv->napi)")
        .expect("NAPI helper did not acquire ownership");
    let clear = schedule_napi
        .find("priv->__rseq_itr_window_tx_packets = 0")
        .expect("NAPI helper did not open a fresh counter window");
    let schedule_irqoff = schedule_napi
        .find("__napi_schedule_irqoff(&priv->napi)")
        .expect("NAPI helper lost IRQ-context scheduling");
    let rejected_rearm = schedule_napi
        .find("RICH_E1000_IMS")
        .expect("NAPI helper did not restore IMS when ownership was refused");
    for clear_window in [
        "priv->__rseq_itr_window_tx_packets = 0",
        "priv->__rseq_itr_window_rx_packets = 0",
        "priv->__rseq_itr_window_tx_bytes = 0",
        "priv->__rseq_itr_window_rx_bytes = 0",
    ] {
        assert!(schedule_napi.contains(clear_window));
    }
    assert!(
        prepare < clear && clear < schedule_irqoff && schedule_irqoff < rejected_rearm,
        "NAPI counters must clear only after ownership, and refused scheduling must restore IMS:\n{schedule_napi}"
    );

    let link_work = emitted_function(&linux, "rich_e1000_link_work");
    for needle in [
        "rich_e1000_report_carrier(&priv->inner, carrier)",
        "schedule_delayed_work(&priv->__rseq_link_work, 2 * HZ)",
        "netif_carrier_on(priv->netdev)",
        "rich_e1000_tx_desc_unused(priv) >= 32u",
        "netif_queue_stopped(priv->netdev)",
        "netif_wake_queue(priv->netdev)",
        "RICH_E1000_IMC",
        "readl(priv->inner.regs + RICH_E1000_STATUS)",
        "rich_e1000_schedule_napi(priv, false)",
    ] {
        assert!(
            link_work.contains(needle),
            "independent LSC recovery lost {needle}:\n{link_work}"
        );
    }
    assert!(
        !link_work.contains("tx_done"),
        "carrier-return TX wake must not depend on fresh TX completions:\n{link_work}"
    );

    let poll = emitted_function(&linux, "rich_e1000_napi_poll");
    for needle in [
        "min_t(int, budget, 64)",
        "rich_e1000_harvest_tx(&priv->inner, work_budget)",
        "rich_e1000_harvest_rx(&priv->inner, work_budget)",
        "if (tx_done == work_budget || rx_done == work_budget)",
        "return budget;",
        "napi_complete_done(napi, rx_done)",
        "tx_packets = (u32)priv->__rseq_itr_window_tx_packets",
        "rx_packets = (u32)priv->__rseq_itr_window_rx_packets",
        "tx_bytes = (u32)priv->__rseq_itr_window_tx_bytes",
        "rx_bytes = (u32)priv->__rseq_itr_window_rx_bytes",
        "rich_e1000_set_itr(&priv->inner, tx_packets, tx_bytes, rx_packets, rx_bytes)",
        "RICH_E1000_IMS",
    ] {
        assert!(poll.contains(needle), "NAPI poll lost {needle}:\n{poll}");
    }
    let incomplete = poll
        .find("if (tx_done == work_budget || rx_done == work_budget)")
        .expect("incomplete predicate");
    let complete = poll
        .find("napi_complete_done(napi, rx_done)")
        .expect("completion");
    let recalculate = poll[complete..]
        .find("rich_e1000_set_itr(&priv->inner")
        .map(|position| complete + position)
        .expect("adaptive ITR recalculation after completion");
    let rearm = poll[complete..]
        .find("RICH_E1000_IMS")
        .map(|position| complete + position)
        .expect("IMS re-arm after completion");
    assert!(
        incomplete < complete && complete < recalculate && recalculate < rearm,
        "NAPI must retain the service window across re-polls, recalculate adaptive ITR, and re-arm IMS only after completion:\n{poll}"
    );
    assert!(
        !poll.contains("RICH_E1000_ITR")
            && !poll.contains("__rseq_napi_busy")
            && !poll.contains("packets_before")
            && !poll.contains("bytes_before"),
        "NAPI retained the replaced static two-level ITR heuristic:\n{poll}"
    );

    let update_itr = emitted_function(&linux, "rich_e1000_update_itr");
    for threshold in [
        "0x1f40u", "0x200u", "0x2710u", "0x4b0u", "0x7d0u", "0x61a8u", "0x1770u",
    ] {
        assert!(
            update_itr.contains(threshold),
            "stock adaptive ITR state machine lost threshold {threshold}:\n{update_itr}"
        );
    }
    let set_itr = emitted_function(&linux, "rich_e1000_set_itr");
    for target in ["0x4e20u", "0xfa0u"] {
        assert!(
            set_itr.contains(target),
            "stock adaptive ITR state machine lost interrupt-rate target {target}:\n{set_itr}"
        );
    }

    let probe = emitted_function(&linux, "rich_e1000_probe");
    let add = probe.find("netif_napi_add(").expect("NAPI registration");
    let enable = probe.find("napi_enable(").expect("NAPI enable");
    let request = probe.find("request_irq(").expect("IRQ registration");
    let publish = probe.find("register_netdev(").expect("netdev publication");
    assert!(
        add < enable && enable < request && request < publish,
        "NAPI must be live before authored IMS/IRQ activity and before publication:\n{probe}"
    );

    let open = emitted_function(&linux, "rich_e1000_net_open");
    let open_reenable = open
        .find("if (!priv->__rseq_napi_enabled)")
        .expect("open did not recognize a prior stop");
    let open_enable = open.find("napi_enable(").expect("open NAPI re-enable");
    let open_admin = open
        .find("rich_e1000_set_admin_enabled(&priv->inner, true)")
        .expect("authored admin-up call");
    let open_rollback = open
        .find("if (ret && priv->__rseq_napi_enabled)")
        .expect("failed admin-up NAPI rollback");
    assert!(
        open_reenable < open_enable && open_enable < open_admin && open_admin < open_rollback,
        "each down/up flap must re-enable NAPI before authored admin-up and roll it back on failure:\n{open}"
    );

    let stop = emitted_function(&linux, "rich_e1000_net_stop");
    let stop_tx = stop
        .find("netif_tx_disable(dev)")
        .expect("stop did not quiesce TX");
    let stop_admin = stop
        .find("rich_e1000_set_admin_enabled(&priv->inner, false)")
        .expect("authored admin-down call");
    let stop_link = stop
        .find("cancel_delayed_work_sync(&priv->__rseq_link_work)")
        .expect("stop did not drain link recovery work");
    let stop_disable = stop.find("napi_disable(").expect("stop NAPI disable");
    let stop_mark = stop
        .find("priv->__rseq_napi_enabled = false")
        .expect("stop NAPI state update");
    assert!(
        stop_tx < stop_admin
            && stop_admin < stop_link
            && stop_link < stop_disable
            && stop_disable < stop_mark,
        "each up/down flap must quiesce TX and link work before disabling NAPI:\n{stop}"
    );

    let transmit = emitted_function(&linux, "rich_e1000_net_transmit");
    for needle in [
        "rich_e1000_tx_desc_needed(packet)",
        "rich_e1000_maybe_stop_tx(priv, needed + 2u)",
        "rich_e1000_maybe_stop_tx(priv, MAX_SKB_FRAGS + 7u)",
    ] {
        assert!(
            transmit.contains(needle),
            "descriptor-count TX admission lost {needle}:\n{transmit}"
        );
    }
    let stop_tx_queue = emitted_function(&linux, "rich_e1000_maybe_stop_tx");
    let stop_queue = stop_tx_queue
        .find("netif_stop_queue(priv->netdev)")
        .expect("TX admission did not stop the queue");
    let stop_barrier = stop_tx_queue.find("smp_mb()").expect("TX stop barrier");
    let stop_recheck = stop_tx_queue[stop_barrier..]
        .find("rich_e1000_tx_desc_unused(priv) < needed")
        .map(|position| stop_barrier + position)
        .expect("TX admission did not recheck ring space");
    let restart = stop_tx_queue
        .find("netif_start_queue(priv->netdev)")
        .expect("TX admission did not undo an unnecessary stop");
    assert!(stop_queue < stop_barrier && stop_barrier < stop_recheck && stop_recheck < restart);

    let wake_tx = emitted_function(&linux, "rich_e1000_maybe_wake_tx");
    let wake_barrier = wake_tx.find("smp_mb()").expect("TX wake barrier");
    let stopped = wake_tx[wake_barrier..]
        .find("netif_queue_stopped(priv->netdev)")
        .map(|position| wake_barrier + position)
        .expect("TX wake did not recheck stopped state");
    let wake = wake_tx
        .find("netif_wake_queue(priv->netdev)")
        .expect("TX reclaim did not wake the queue");
    assert!(wake_barrier < stopped && stopped < wake);
    let descriptor_command = emitted_function(&linux, "rich_e1000_descriptor_command");
    for needle in [
        "__rseq_priv->__rseq_itr_window_tx_packets++",
        "__rseq_priv->__rseq_itr_window_tx_bytes += __packet->len",
        "__rseq_priv->__rseq_itr_window_rx_packets++",
        "__rseq_priv->__rseq_itr_window_rx_bytes += __length",
    ] {
        assert!(
            descriptor_command.contains(needle),
            "descriptor service lost per-window accounting {needle}"
        );
    }
    assert!(
        !descriptor_command.contains("netif_wake_queue"),
        "transport cleanup woke TX before authored tx_head publication"
    );

    let remove = emitted_function(&linux, "rich_e1000_remove");
    let cancel_link = remove
        .find("cancel_delayed_work_sync(&priv->__rseq_link_work)")
        .expect("remove did not drain link recovery work");
    let disable = remove.find("napi_disable(").expect("NAPI disable");
    let authored = remove
        .find("rich_e1000_on_deinit(")
        .expect("authored teardown");
    let free_irq = remove.find("free_irq(").expect("IRQ deregistration");
    let delete = remove.find("netif_napi_del(").expect("NAPI deletion");
    let free_netdev = remove.find("free_netdev(").expect("netdev free");
    assert!(
        cancel_link < disable
            && disable < authored
            && authored < free_irq
            && free_irq < delete
            && delete < free_netdev,
        "NAPI quiescence/deletion does not bracket authored ring release and netdev free:\n{remove}"
    );
}

#[test]
fn e1000_rar_address_is_published_to_the_native_interface() {
    let pair = emit_pair("c5-e1000-rich");
    for (target, tu) in [
        ("linux", pair.linux.tu.as_str()),
        ("zephyr", pair.zephyr.tu.as_str()),
    ] {
        let synchronize = emitted_function(tu, "rich_e1000_synchronize_mac_from_rar");
        for needle in [
            "RICH_E1000_RAL0",
            "RICH_E1000_RAH0",
            "(*_1).mac_low =",
            "(*_1).mac_high =",
        ] {
            assert!(
                synchronize.contains(needle),
                "{target} RAR synchronization lost {needle}:\n{synchronize}"
            );
        }
    }

    let probe = emitted_function(&pair.linux.tu, "rich_e1000_probe");
    for needle in [
        "priv->inner.mac_low",
        "priv->inner.mac_high",
        "eth_hw_addr_set(priv->netdev, __mac)",
    ] {
        assert!(
            probe.contains(needle),
            "Linux native MAC publication lost {needle}:\n{probe}"
        );
    }
    let initialized = probe
        .find("rich_e1000_on_init(&priv->inner)")
        .expect("Linux probe lost authored initialization");
    let address = probe
        .find("eth_hw_addr_set(priv->netdev, __mac)")
        .expect("Linux probe lost native MAC publication");
    let published = probe
        .find("register_netdev(priv->netdev)")
        .expect("Linux probe lost netdev publication");
    assert!(
        initialized < address && address < published,
        "Linux published the netdev before the RAR-derived address:\n{probe}"
    );

    let iface_init = emitted_function(&pair.zephyr.tu, "rich_e1000_net_iface_init");
    for needle in [
        "data->inner.mac_low",
        "data->inner.mac_high",
        "net_if_set_link_addr(iface, __mac, sizeof(__mac), NET_LINK_ETHERNET)",
    ] {
        assert!(
            iface_init.contains(needle),
            "Zephyr native MAC publication lost {needle}:\n{iface_init}"
        );
    }
    let start = emitted_function(&pair.zephyr.tu, "rich_e1000_net_start");
    for needle in [
        "property = { ._raw = { 8 } }",
        "value._raw[0] = 6",
        "value._raw[1] = 1",
    ] {
        assert!(
            start.contains(needle),
            "Zephyr native all-multicast preparation lost {needle}:\n{start}"
        );
    }
    assert!(
        start.find("rich_e1000_set_property").unwrap()
            < start.find("rich_e1000_set_admin_enabled").unwrap(),
        "Zephyr enabled RX before selecting the stock-parity multicast filter:\n{start}"
    );
}

#[test]
fn pl011_zephyr_boot_config_is_dt_gated_and_reuses_authored_ops() {
    let emit = emit_pair("c5-pl011-rich").zephyr;
    let boot = emitted_function(&emit.tu, "rich_pl011_uart_boot_configure");
    assert!(
        boot.contains("DT_INST_PROP(0, current_speed)"),
        "boot config does not consume the standard DT baud rate:\n{boot}"
    );
    assert!(
        boot.contains("DT_INST_PROP_OR(0, hw_flow_control, 0)"),
        "boot config does not consume the optional DT flow-control setting:\n{boot}"
    );
    assert!(
        boot.contains("rich_pl011_serial_configure(&data->inner, params)")
            && boot.contains("rich_pl011_serial_open(&data->inner)"),
        "boot config must reuse the authored configure/open operations:\n{boot}"
    );

    let init = emitted_function(&emit.tu, "rich_pl011_init");
    let authored_init = init
        .find("rich_pl011_on_init(&data->inner)")
        .expect("missing authored on_init call");
    let property_gate = init
        .find("#if DT_INST_NODE_HAS_PROP(0, current_speed)")
        .expect("missing compile-time current-speed gate");
    let boot_call = init
        .find("rich_pl011_uart_boot_configure(dev)")
        .expect("missing boot-config call");
    let irq_enable = init
        .find("irq_enable(DT_INST_IRQN(0))")
        .expect("missing IRQ enable");
    assert!(
        authored_init < property_gate && property_gate < boot_call && boot_call < irq_enable,
        "boot config must run after successful on_init and before irq_enable:\n{init}"
    );
    assert!(
        emit.binding_yaml.contains("include: uart-controller.yaml"),
        "serial binding must expose Zephyr's standard UART properties:\n{}",
        emit.binding_yaml
    );

    let initfail = emit_zephyr("c5-pl011-linux-initfail");
    assert!(!initfail.tu.contains("current_speed"), "{}", initfail.tu);
    assert!(
        initfail.binding_yaml.contains("include: base.yaml"),
        "serial-free initfail binding changed class:\n{}",
        initfail.binding_yaml
    );
}

#[test]
fn pl011_linux_elides_unchanged_serial_core_mctrl() {
    let tu = emit_pair("c5-pl011-rich").linux.tu;
    assert!(
        tu.contains("unsigned int __rseq_mctrl;"),
        "Linux serial glue lacks its applied DTR/RTS shadow"
    );

    let probe = emitted_function(&tu, "rich_pl011_probe");
    let seed = probe
        .find("priv->__rseq_mctrl = 0;")
        .expect("probe does not seed the authored inactive DTR/RTS state");
    let publish = probe
        .find("uart_add_one_port")
        .expect("probe does not publish the uart port");
    assert!(seed < publish, "mctrl shadow was seeded after publication");

    let set_mctrl = emitted_function(&tu, "rich_pl011_uart_set_mctrl");
    let guard = set_mctrl
        .find("if (driven == priv->__rseq_mctrl)\n\t\treturn;")
        .expect("set_mctrl does not elide an unchanged DTR/RTS request");
    let authored = set_mctrl
        .find("rich_pl011_serial_set_signals")
        .expect("set_mctrl does not retain the authored signal operation");
    let advance = set_mctrl
        .find("if (status._raw[0] == 0)\n\t\tpriv->__rseq_mctrl = driven;")
        .expect("set_mctrl advances its shadow without an authored success");
    assert!(
        guard < authored && authored < advance,
        "set_mctrl no-op guard/signal call/shadow update are misordered"
    );
}

#[test]
fn pl011_zephyr_elides_unchanged_irq_mask_requests() {
    let tu = emit_pair("c5-pl011-rich").zephyr.tu;
    for (symbol, guard) in [
        (
            "rich_pl011_uart_irq_rx_enable",
            "if (data->__rseq_irq_rx_enabled)\n\t\treturn;",
        ),
        (
            "rich_pl011_uart_irq_rx_disable",
            "if (!data->__rseq_irq_rx_enabled)\n\t\treturn;",
        ),
        (
            "rich_pl011_uart_irq_tx_enable",
            "if (data->__rseq_irq_tx_enabled)\n\t\treturn;",
        ),
        (
            "rich_pl011_uart_irq_tx_disable",
            "if (!data->__rseq_irq_tx_enabled)\n\t\treturn;",
        ),
    ] {
        let callback = emitted_function(&tu, symbol);
        let guard = callback
            .find(guard)
            .unwrap_or_else(|| panic!("{symbol} does not elide an unchanged mask request"));
        let apply = callback
            .find("rich_pl011_uart_apply_irq_mask(dev)")
            .unwrap_or_else(|| panic!("{symbol} does not apply a changed mask request"));
        assert!(
            guard < apply,
            "{symbol} applies the mask before its no-op guard"
        );
    }
}

#[test]
fn pl011_zephyr_software_kicks_first_tx_interrupt() {
    let tu = emit_pair("c5-pl011-rich").zephyr.tu;

    for field in [
        "volatile bool __rseq_sw_call_txdrdy;",
        "struct k_spinlock __rseq_irq_callback_lock;",
    ] {
        assert!(tu.contains(field), "Zephyr PL011 data lacks `{field}`");
    }

    let init = emitted_function(&tu, "rich_pl011_init");
    let arm = init
        .find("data->__rseq_sw_call_txdrdy = true;")
        .expect("Zephyr PL011 init does not arm the first TX software kick");
    let line_enable = init
        .find("irq_enable(DT_INST_IRQN(0))")
        .expect("Zephyr PL011 init does not enable the IRQ line");
    assert!(
        arm < line_enable,
        "the first TX kick was armed after IRQ enable"
    );

    let enable = emitted_function(&tu, "rich_pl011_uart_irq_tx_enable");
    let apply = enable
        .find("rich_pl011_uart_apply_irq_mask(dev)")
        .expect("TX enable does not apply the authored TX mask");
    let armed = enable
        .find("if (!data->__rseq_sw_call_txdrdy)")
        .expect("TX enable does not gate the software kick to its armed state");
    let consume = enable
        .find("data->__rseq_sw_call_txdrdy = false;")
        .expect("TX enable does not consume the one-shot software kick");
    let ready = enable
        .find("data->__rseq_irq_tx_ready = true;")
        .expect("TX enable does not present software TX-ready state");
    let lock = enable
        .find("K_SPINLOCK(&data->__rseq_irq_callback_lock)")
        .expect("TX enable does not serialize the software callback");
    let callback = enable
        .find("data->__rseq_irq_callback(dev, data->__rseq_irq_callback_data)")
        .expect("TX enable does not software-call the registered UART callback");
    let clear = enable
        .find("data->__rseq_irq_tx_ready = false;")
        .expect("TX enable does not clear software TX-ready state");
    assert!(
        apply < armed
            && armed < consume
            && consume < ready
            && ready < lock
            && lock < callback
            && callback < clear,
        "first TX software-kick operations are misordered"
    );

    let disable = emitted_function(&tu, "rich_pl011_uart_irq_tx_disable");
    assert!(
        disable.find("data->__rseq_sw_call_txdrdy = true;").unwrap()
            < disable.find("rich_pl011_uart_apply_irq_mask(dev)").unwrap(),
        "TX disable does not re-arm the next first-kick before masking"
    );

    let ready = emitted_function(&tu, "rich_pl011_uart_irq_tx_ready");
    assert!(
        ready.contains("data->__rseq_irq_tx_enabled && data->__rseq_irq_tx_ready"),
        "TX-ready does not require both the enabled mask and ready state"
    );
    let update = emitted_function(&tu, "rich_pl011_uart_irq_update");
    assert!(
        update.contains("return 1;"),
        "irq_update diverged from stock PL011"
    );

    let raise = emitted_function(&tu, "rich_pl011_raise");
    let event_ready = raise
        .find("case 1: data->__rseq_irq_tx_ready = true; break;")
        .expect("hardware TX events do not present TX-ready state");
    let event_lock = raise
        .find("K_SPINLOCK(&data->__rseq_irq_callback_lock)")
        .expect("hardware TX events do not serialize the UART callback");
    let event_callback = raise
        .find("data->__rseq_irq_callback(dev, data->__rseq_irq_callback_data)")
        .expect("hardware TX events do not call the registered UART callback");
    let event_clear = raise
        .find("data->__rseq_irq_tx_ready = false;")
        .expect("hardware TX events do not clear TX-ready state");
    assert!(
        event_ready < event_lock && event_lock < event_callback && event_callback < event_clear,
        "hardware TX-event callback operations are misordered"
    );
}

#[test]
fn composed_products_claim_and_dispatch_interrupts_but_initfail_claims_nothing() {
    let pl011 = emit_pair("c5-pl011-rich");
    let e1000 = emit_pair("c5-e1000-rich");

    for (device, tu) in [("pl011", &pl011.linux.tu), ("e1000", &e1000.linux.tu)] {
        for needle in ["static irqreturn_t rseq_isr_", "request_irq(", "free_irq("] {
            assert!(
                tu.contains(needle),
                "{device} linux composed product lacks interrupt shape `{needle}`"
            );
        }
        assert!(
            tu.contains("handle_irq(&priv->inner)"),
            "{device} linux ISR does not dispatch the authored handler"
        );
    }
    for (device, tu) in [("pl011", &pl011.zephyr.tu), ("e1000", &e1000.zephyr.tu)] {
        for needle in ["static void rseq_isr_", "IRQ_CONNECT(", "irq_enable("] {
            assert!(
                tu.contains(needle),
                "{device} zephyr composed product lacks interrupt shape `{needle}`"
            );
        }
        assert!(
            tu.contains("handle_irq(&data->inner)"),
            "{device} zephyr ISR does not dispatch the authored handler"
        );
    }

    let initfail = [
        emit_linux("c5-pl011-linux-initfail"),
        emit_zephyr("c5-pl011-linux-initfail").tu,
        emit_composed_pair("e1000-initfail").linux.tu,
        emit_composed_pair("e1000-initfail").zephyr.tu,
    ];
    for tu in initfail {
        for forbidden in ["request_irq(", "free_irq(", "IRQ_CONNECT(", "irq_enable("] {
            assert!(
                !tu.contains(forbidden),
                "initfail product unexpectedly claims an interrupt with `{forbidden}`"
            );
        }
    }
}

#[test]
fn negative_products_are_unique_and_induced() {
    write_or_verify_products(false);
    let manifest: Value =
        serde_json::from_slice(&std::fs::read(generated_dir().join("MANIFEST.json")).unwrap())
            .unwrap();
    let outputs = manifest["outputs"].as_array().unwrap();
    for (device, target) in [
        ("pl011", "linux"),
        ("pl011", "zephyr"),
        ("e1000", "linux"),
        ("e1000", "zephyr"),
    ] {
        assert_eq!(
            outputs
                .iter()
                .filter(|output| {
                    output["device"] == device
                        && output["target"] == target
                        && output["role"] == "negative_source"
                })
                .count(),
            1,
            "{device} {target} negative_source must be unique",
        );
    }

    let read_product = |device, target, role| {
        std::fs::read_to_string(product_output_path(device, target, role)).unwrap()
    };
    for tu in [
        read_product("pl011", "linux", "negative_source"),
        read_product("pl011", "zephyr", "negative_source"),
    ] {
        assert!(tu.contains("pl011 init-fail witness: refusing to start"));
        assert!(tu.contains("return -EBUSY;"));
    }
    for tu in [
        read_product("e1000", "linux", "negative_source"),
        read_product("e1000", "zephyr", "negative_source"),
    ] {
        assert!(tu.contains("fails the impossible expectation"));
        assert!(tu.contains("e1000fail: init failed, cleaning up partial state"));
        assert!(tu.contains("return -EIO;"));
    }
    assert!(read_product("pl011", "linux", "negative_source")
        .contains("module_amba_driver(pl011_initfail_driver)"));
    assert!(read_product("e1000", "linux", "negative_source")
        .contains("module_pci_driver(e1000fail_driver)"));
    for tu in [
        read_product("pl011", "linux", "negative_source"),
        read_product("pl011", "zephyr", "negative_source"),
    ] {
        assert!(!tu.contains("uart_add_one_port("));
        assert!(!tu.contains("NET_IF_INIT("));
    }
    for tu in [
        read_product("e1000", "linux", "negative_source"),
        read_product("e1000", "zephyr", "negative_source"),
    ] {
        assert!(!tu.contains("register_netdev("));
        assert!(!tu.contains("NET_IF_INIT("));
    }
}

#[test]
fn rich_specs_compile_unchanged_and_keep_scaffold_backend_owned() {
    write_or_verify_products(false);
    let pl011_linux = std::fs::read_to_string(output_path("pl011", "linux")).unwrap();
    let pl011_zephyr = std::fs::read_to_string(output_path("pl011", "zephyr")).unwrap();
    let e1000_linux = std::fs::read_to_string(output_path("e1000", "linux")).unwrap();
    let e1000_zephyr = std::fs::read_to_string(output_path("e1000", "zephyr")).unwrap();

    for needle in ["rich_pl011_on_init", "RichPl011", "RICH_PL011_IBRD"] {
        assert!(
            pl011_linux.contains(needle),
            "Linux lost authored PL011 logic: {needle}"
        );
        assert!(
            pl011_zephyr.contains(needle),
            "Zephyr lost authored PL011 logic: {needle}"
        );
    }
    assert!(pl011_linux.contains("struct uart_ops"));
    for callback in [
        ".tx_empty = rich_pl011_uart_tx_empty",
        ".set_mctrl = rich_pl011_uart_set_mctrl",
        ".get_mctrl = rich_pl011_uart_get_mctrl",
        ".stop_tx = rich_pl011_uart_stop_tx",
        ".start_tx = rich_pl011_uart_start_tx",
        ".stop_rx = rich_pl011_uart_stop_rx",
        ".break_ctl = rich_pl011_uart_break_ctl",
        ".startup = rich_pl011_uart_startup",
        ".shutdown = rich_pl011_uart_shutdown",
        ".flush_buffer = rich_pl011_uart_flush_buffer",
        ".set_termios = rich_pl011_uart_set_termios",
        ".type = rich_pl011_uart_type",
        ".release_port = rich_pl011_uart_release_port",
        ".request_port = rich_pl011_uart_request_port",
        ".config_port = rich_pl011_uart_config_port",
    ] {
        assert!(
            pl011_linux.contains(callback),
            "persisted PL011 product lacks {callback}"
        );
    }
    assert!(pl011_linux.contains("priv->__rseq_uart_port.membase = priv->inner.regs"));
    assert!(pl011_linux.contains("UPF_BOOT_AUTOCONF | UPF_FIXED_PORT | UPF_FIXED_TYPE"));
    assert!(!pl011_linux.contains("dma_request_chan("));
    assert!(pl011_linux.contains(".id\t= 0x00abc123,"));
    assert!(pl011_linux.contains(".mask\t= 0x00ffffff,"));
    assert!(!pl011_linux.contains(".id\t= 0x00041011,"));
    assert!(pl011_linux.contains("uart_register_driver(&rich_pl011_uart_driver)"));
    assert!(pl011_linux.contains("uart_add_one_port(&rich_pl011_uart_driver"));
    assert!(pl011_linux.contains("uart_remove_one_port(&rich_pl011_uart_driver"));
    let pl011_publish_failure = &pl011_linux[pl011_linux
        .find("ret = uart_add_one_port(&rich_pl011_uart_driver")
        .unwrap()..];
    assert!(
        pl011_publish_failure.find("rich_pl011_on_deinit").unwrap()
            < pl011_publish_failure.find("return ret;").unwrap(),
        "PL011 publication failure did not unwind authored initialization"
    );
    assert!(pl011_linux.contains(".dev_name = \"ttyrich_pl011\""));
    assert!(pl011_linux.contains("module_init(rich_pl011_init)"));
    assert!(pl011_zephyr.contains("DEVICE_API(uart,"));
    assert!(!pl011_zephyr.contains("dma_request_channel("));
    assert!(pl011_zephyr.contains("#define DT_DRV_COMPAT regseq_serialpl011"));
    assert!(pl011_zephyr.contains("DEVICE_DT_INST_DEINIT_DEFINE("));
    assert!(!pl011_linux.contains("DEVICE_DT_INST_DEFINE"));
    assert!(!pl011_zephyr.contains("module_amba_driver"));

    for needle in ["rich_e1000_on_init", "RichE1000", "RICH_E1000_TDT"] {
        assert!(
            e1000_linux.contains(needle),
            "Linux lost authored E1000 logic: {needle}"
        );
        assert!(
            e1000_zephyr.contains(needle),
            "Zephyr lost authored E1000 logic: {needle}"
        );
    }
    assert!(e1000_linux.contains("struct net_device_ops"));
    assert!(e1000_linux.contains("netif_carrier_off(priv->netdev)"));
    assert!(e1000_linux.contains("netif_tx_stop_all_queues(priv->netdev)"));
    assert!(e1000_linux.contains("register_netdev(priv->netdev)"));
    assert!(e1000_linux.contains("unregister_netdev(priv->netdev)"));
    let e1000_publish_failure =
        &e1000_linux[e1000_linux.find("int __publish = register_netdev").unwrap()..];
    assert!(
        e1000_publish_failure.find("rich_e1000_on_deinit").unwrap()
            < e1000_publish_failure.find("return __publish;").unwrap(),
        "E1000 publication failure did not unwind authored initialization"
    );
    assert!(e1000_linux.contains("rich_e1000_descriptor_command("));
    assert!(!e1000_linux.contains("mbox_send_message("));
    assert!(e1000_linux.contains("module_pci_driver(rich_e1000_driver)"));
    assert!(e1000_zephyr.contains("net_if_send_data("));
    assert!(e1000_zephyr.contains("#define DT_DRV_COMPAT regseq_e1000"));
    assert!(e1000_zephyr.contains("static const struct ethernet_api rich_e1000_net_api"));
    assert!(e1000_zephyr.contains("select the already-authored AllMulticast mode"));
    assert!(
        e1000_zephyr.contains("PCIE_CONF_CMDSTAT_MEM |\n\t\t     PCIE_CONF_CMDSTAT_MASTER, true);")
    );
    assert!(e1000_zephyr.contains("static struct net_stats_eth *rich_e1000_net_get_stats"));
    assert!(e1000_zephyr.contains(".get_stats = rich_e1000_net_get_stats,"));
    for field in [
        "errors.rx",
        "errors.tx",
        "tx_dropped",
        "pkts.rx",
        "pkts.tx",
        "bytes.received",
        "bytes.sent",
    ] {
        assert!(
            e1000_zephyr.contains(&format!("__rseq_net_stats.{field}")),
            "Zephyr E1000 statistics glue lost {field}"
        );
    }
    assert!(e1000_zephyr.contains("net_if_flag_set(iface, NET_IF_NO_AUTO_START);"));
    assert!(e1000_zephyr.contains(".start = rich_e1000_net_start,"));
    assert!(e1000_zephyr.contains(".stop = rich_e1000_net_stop,"));
    assert!(e1000_zephyr.contains("NET_IF_INIT("));
    assert!(e1000_zephyr.contains("rich_e1000_descriptor_command("));
    assert!(!e1000_zephyr.contains("mbox_send("));
    assert!(e1000_zephyr.contains("DEVICE_PCIE_INST_DECLARE("));
    assert!(e1000_zephyr.contains("DEVICE_DT_INST_DEINIT_DEFINE("));
    assert!(!e1000_linux.contains("DEVICE_PCIE_INST_DECLARE"));
    assert!(!e1000_zephyr.contains("module_pci_driver"));
}

#[test]
fn generated_pl011_linux_closes_rx_delivery_and_break_control_gaps() {
    let pair = emit_pair("c5-pl011-rich");
    let tu = pair.linux.tu;

    for needle in [
        "rich_pl011_serial_receive(&priv->inner, buffer)",
        "uart_insert_char(port, errors.overrun, true, ch, flag)",
        "tty_flip_buffer_push(&port->state->port)",
        "uart_write_wakeup(port)",
        "uart_handle_cts_change(port, signals.cts)",
        "uart_handle_dcd_change(port, signals.dcd)",
    ] {
        assert!(
            tu.contains(needle),
            "Linux RX/event lowering lacks {needle}"
        );
    }

    let break_lines = tu
        .lines()
        .filter(|line| line.contains("break") || line.contains("BRK"))
        .collect::<Vec<_>>();
    assert!(
        tu.contains("rich_pl011_serial_break_send(&priv->inner, ctl != 0)"),
        "Linux break lowering lacks the authored hook call: {break_lines:?}"
    );
    let send_break = tu
        .rsplit_once("static void rich_pl011_serial_break_send(")
        .map(|(_, tail)| tail.split_once("\n}\n").unwrap().0)
        .expect("generated Linux TU lacks the authored break-send body");
    assert!(
        send_break.contains("readl(_1->regs + RICH_PL011_LCRH)"),
        "unexpected break-send body: {send_break}"
    );
    assert!(send_break.contains("writel("));
    assert_eq!(send_break.matches("RICH_PL011_LCRH").count(), 2);

    let zephyr = pair.zephyr.tu;
    for needle in [
        "static int rich_pl011_uart_fifo_read",
        "rich_pl011_serial_receive(&data->inner, buffer) != 1",
        ".irq_rx_ready = rich_pl011_uart_irq_rx_ready",
        "data->__rseq_irq_callback(dev, data->__rseq_irq_callback_data)",
        "serial-hardware-ops.break-send is unsupported on Zephyr",
    ] {
        assert!(
            zephyr.contains(needle),
            "Zephyr serial event lowering lacks {needle}"
        );
    }
    assert!(!zephyr.contains("rich_pl011_serial_break_send("));
}

#[test]
fn generated_pl011_preserves_unprojected_purge_and_modem_operations() {
    let pair = emit_pair("c5-pl011-rich");
    let linux = pair.linux.tu;

    let linux_ops = &linux[linux
        .find("static const struct uart_ops rich_pl011_uart_ops")
        .expect("Linux rich PL011 uart_ops")..];
    let linux_ops = &linux_ops[..linux_ops.find("\n};").expect("Linux uart_ops end")];
    for supported in [".set_mctrl =", ".get_mctrl ="] {
        assert!(
            linux_ops.contains(supported),
            "Linux rich PL011 lost supported modem hook {supported}:\n{linux_ops}"
        );
    }
    assert!(
        !linux_ops.contains(".enable_ms ="),
        "Linux rich PL011 fabricated a modem-status-enable callback:\n{linux_ops}"
    );
    let linux_set_mctrl = emitted_function(&linux, "rich_pl011_uart_set_mctrl");
    assert!(
        linux_set_mctrl.contains("rich_pl011_serial_set_signals")
            && linux_set_mctrl.contains("TIOCM_DTR")
            && linux_set_mctrl.contains("TIOCM_RTS"),
        "Linux rich PL011 lost authored DTR/RTS projection:\n{linux_set_mctrl}"
    );
    let linux_get_mctrl = emitted_function(&linux, "rich_pl011_uart_get_mctrl");
    for sensed in ["TIOCM_CTS", "TIOCM_DSR", "TIOCM_CAR", "TIOCM_RNG"] {
        assert!(
            linux_get_mctrl.contains(sensed),
            "Linux rich PL011 lost sensed modem input {sensed}:\n{linux_get_mctrl}"
        );
    }

    let linux_flush = emitted_function(&linux, "rich_pl011_uart_flush_buffer");
    assert!(
        linux_flush.contains("direction._raw[0] = 1;")
            && linux_flush.contains("rich_pl011_serial_purge"),
        "Linux serial-core flush must retain its transmit-only unsupported projection:\n{linux_flush}"
    );
    let linux_purge = emitted_function(&linux, "rich_pl011_serial_purge");
    for required in [
        "rich_pl011_ne(_3, _5)",
        "_0._raw[0] = 0x1;",
        "readl(_1->regs + RICH_PL011_FR)",
        "readl(_1->regs)",
    ] {
        assert!(
            linux_purge.contains(required),
            "Linux rich PL011 lost purge rejection/drain behavior {required}:\n{linux_purge}"
        );
    }
    let linux_signals = emitted_function(&linux, "rich_pl011_raise_signals");
    for handoff in [
        "uart_handle_cts_change",
        "uart_handle_dcd_change",
        "port->icount.dsr++",
        "port->icount.rng++",
        "wake_up_interruptible",
    ] {
        assert!(
            linux_signals.contains(handoff),
            "Linux rich PL011 lost supported modem-status handoff {handoff}:\n{linux_signals}"
        );
    }

    let zephyr = pair.zephyr.tu;
    let zephyr_api = &zephyr[zephyr
        .find("static DEVICE_API(uart, rich_pl011_uart_api)")
        .expect("Zephyr rich PL011 uart API")..];
    let zephyr_api = &zephyr_api[..zephyr_api.find("\n};").expect("Zephyr uart API end")];
    for unsupported in ["purge", "mctrl", "signal"] {
        assert!(
            !zephyr_api.contains(unsupported),
            "Zephyr rich PL011 fabricated unsupported {unsupported} projection:\n{zephyr_api}"
        );
    }
    for uncalled in [
        "rich_pl011_serial_purge(",
        "rich_pl011_serial_set_signals(",
        "rich_pl011_serial_get_signals(",
    ] {
        assert_eq!(
            zephyr.matches(uncalled).count(),
            2,
            "Zephyr rich PL011 unexpectedly calls unprojected operation {uncalled}"
        );
    }
    let zephyr_mask = emitted_function(&zephyr, "rich_pl011_uart_apply_irq_mask");
    assert!(
        zephyr_mask.contains(".signal_change = false"),
        "Zephyr rich PL011 must not enable unsupported modem-status IRQs:\n{zephyr_mask}"
    );
    let zephyr_signals = emitted_function(&zephyr, "rich_pl011_raise_signals");
    assert!(
        zephyr_signals.contains("\n\treturn;")
            && !zephyr_signals.contains("uart_handle_")
            && !zephyr_signals.contains("__rseq_irq_callback"),
        "Zephyr rich PL011 must retain no-op modem-status delivery:\n{zephyr_signals}"
    );
    assert!(zephyr.contains(
        "Signal control, lifecycle, purge, and abort have no Zephyr uart_driver_api slot here; absence is explicit and no result is fabricated."
    ));
}

#[test]
fn generated_e1000_preserves_unadmitted_power_netpoll_and_vlan_operations() {
    let pair = emit_pair("c5-e1000-rich");
    let linux = pair.linux.tu;

    let netdev_ops = &linux[linux
        .find("static const struct net_device_ops rich_e1000_netdev_ops")
        .expect("Linux rich E1000 net_device_ops")..];
    let netdev_ops = &netdev_ops[..netdev_ops.find("\n};").expect("Linux net_device_ops end")];
    for supported in [".ndo_open =", ".ndo_stop =", ".ndo_start_xmit ="] {
        assert!(
            netdev_ops.contains(supported),
            "Linux rich E1000 lost supported callback {supported}:\n{netdev_ops}"
        );
    }
    for unsupported in [
        ".ndo_poll_controller =",
        ".ndo_set_features =",
        ".ndo_vlan_rx_add_vid =",
        ".ndo_vlan_rx_kill_vid =",
    ] {
        assert!(
            !netdev_ops.contains(unsupported),
            "Linux rich E1000 fabricated unsupported callback {unsupported}:\n{netdev_ops}"
        );
    }
    assert!(
        linux.contains("priv->netdev->vlan_features ="),
        "test must distinguish the retained offload mask from absent VLAN control callbacks"
    );

    let pci_driver = &linux[linux
        .find("static struct pci_driver rich_e1000_driver")
        .expect("Linux rich E1000 pci_driver")..];
    let pci_driver = &pci_driver[..pci_driver.find("\n};").expect("Linux pci_driver end")];
    for unsupported in [".suspend =", ".resume =", ".driver.pm ="] {
        assert!(
            !pci_driver.contains(unsupported),
            "Linux rich E1000 fabricated unsupported power callback {unsupported}:\n{pci_driver}"
        );
    }

    let zephyr = pair.zephyr.tu;
    let ethernet_api = &zephyr[zephyr
        .find("static const struct ethernet_api rich_e1000_net_api")
        .expect("Zephyr rich E1000 ethernet_api")..];
    let ethernet_api = &ethernet_api[..ethernet_api.find("\n};").expect("Zephyr ethernet_api end")];
    for unsupported in ["suspend", "resume", "poll", "vlan", "set_config"] {
        assert!(
            !ethernet_api.contains(unsupported),
            "Zephyr rich E1000 fabricated unsupported {unsupported} projection:\n{ethernet_api}"
        );
    }
}

#[test]
fn full_coverage_and_negative_capability_matrices() {
    let coverage: Value = serde_json::from_slice(
        &std::fs::read(
            repo_root().join("experiments/os-api-gap-closure/coverage/C5-composition.json"),
        )
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
    assert_eq!(
        coverage["construct_epochs"]["network-interface-management"],
        1
    );
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
        let local_ids = local
            .iter()
            .map(|id| id.as_str().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(local_ids, frozen_ids, "{device} denominator drift");
    }

    for (stage, count) in [
        ("C1-serial", 21),
        ("C2-DMA", 15),
        ("C3-DCT", 6),
        ("C4-NIM", 20),
    ] {
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
    assert!(dma_error
        .to_string()
        .contains("DmaBuffer owned allocation is unsupported"));

    let network = fixture_from("_network", "network-full");
    let network_model = bridge(&network.input);
    let publication_error = regseq_backends::emit_zephyr_c(&network_model)
        .expect_err("Zephyr must preserve runtime publication absence");
    assert!(publication_error
        .to_string()
        .contains("runtime publication is unsupported"));

    for (_, _, _, text) in generated_products() {
        for fabricated in [
            "command_complete",
            "correlation_id",
            "packet_canceled",
            "packet_completed",
        ] {
            assert!(
                !text.contains(fabricated),
                "generated output fabricated {fabricated}"
            );
        }
    }
    let negatives = coverage["negative_capabilities"].as_array().unwrap();
    assert_eq!(negatives.len(), 4);
    assert!(negatives
        .iter()
        .all(|row| row["expected"] == "rejected" || row["expected"] == "absent"));
    let authored_absences = coverage["authored_absence_matrix"].as_array().unwrap();
    assert_eq!(authored_absences.len(), 4);
    assert_eq!(
        authored_absences
            .iter()
            .map(|row| (
                row["device"].as_str().unwrap(),
                row["target"].as_str().unwrap()
            ))
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
    write_or_verify_products(false);
    for (label, path) in [
        ("pl011", output_path("pl011", "linux")),
        (
            "pl011-initfail",
            product_output_path("pl011", "linux", "negative_source"),
        ),
        ("e1000", output_path("e1000", "linux")),
        (
            "e1000-initfail",
            product_output_path("e1000", "linux", "negative_source"),
        ),
    ] {
        let module_name = path.file_stem().unwrap().to_str().unwrap();
        let tu = std::fs::read_to_string(&path).unwrap();
        let kbuild = format!("obj-m += {module_name}.o\n");
        match kbuild::kbuild_module(&tu, &kbuild, module_name) {
            Ok(KbuildOutcome::Skipped { notice }) => visible(notice),
            Ok(KbuildOutcome::Built {
                kernel_version,
                warnings,
            }) => {
                visible(format!("C5 {label} kbuild PASS against {kernel_version}"));
                if !warnings.trim().is_empty() {
                    visible(warnings);
                }
            }
            Err(error) => panic!("C5 {label} kbuild failed: {error}"),
        }
    }
}

#[test]
fn rich_zephyr_outputs_build_against_real_headers() {
    let pl011 = emit_pair("c5-pl011-rich").zephyr;
    let pl011_initfail = emit_zephyr("c5-pl011-linux-initfail");
    let e1000 = emit_pair("c5-e1000-rich").zephyr;
    let e1000_initfail = emit_composed_pair("e1000-initfail").zephyr;
    let drivers = [
        StagedDriver {
            tu: &pl011.tu,
            binding_yaml: &pl011.binding_yaml,
            driver_name: &pl011.driver_name,
            compatible: "regseq,serialpl011",
            bus: StagedBus::Platform {
                region: (0x0904_0000, 0x1000),
                interrupts: Some("<0 34 2 0>"),
            },
        },
        StagedDriver {
            tu: &pl011_initfail.tu,
            binding_yaml: &pl011_initfail.binding_yaml,
            driver_name: &pl011_initfail.driver_name,
            compatible: "regseq,serialpl011",
            bus: StagedBus::Platform {
                region: (0x0904_0000, 0x1000),
                interrupts: None,
            },
        },
        StagedDriver {
            tu: &e1000.tu,
            binding_yaml: &e1000.binding_yaml,
            driver_name: &e1000.driver_name,
            compatible: "regseq,e1000",
            bus: StagedBus::Pcie {
                vendor_id: 0x8086,
                device_id: 0x100e,
            },
        },
        StagedDriver {
            tu: &e1000_initfail.tu,
            binding_yaml: &e1000_initfail.binding_yaml,
            driver_name: &e1000_initfail.driver_name,
            compatible: "regseq,e1000fail",
            bus: StagedBus::Pcie {
                vendor_id: 0x8086,
                device_id: 0x100e,
            },
        },
    ];
    for driver in &drivers {
        match zephyr_build::west_build(driver) {
            Ok(WestOutcome::Skipped { notice }) => visible(notice),
            Ok(WestOutcome::Built {
                zephyr_version,
                warnings,
            }) => {
                visible(format!("C5 west PASS against Zephyr {zephyr_version}"));
                if !warnings.trim().is_empty() {
                    visible(warnings);
                }
            }
            Err(error) => panic!("C5 west build failed: {error}"),
        }
    }
}
