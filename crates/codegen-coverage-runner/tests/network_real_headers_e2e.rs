#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::Path;

use codegen_coverage_runner::zephyr_build::{
    west_build, StagedBus, StagedDriver, WestOutcome, BOARD, WORKSPACE_VAR,
};
use codegen_coverage_runner::kbuild;
use test_ir_fixtures::{try_bridge_fixture_path, BridgeOutcome};

fn model() -> regseq_model::RegseqModel {
    let input = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/_network/network-full/input.rs");
    match try_bridge_fixture_path(&input) {
        BridgeOutcome::Ok(model) => model,
        other => panic!("network-full bridge failed: {other:?}"),
    }
}

fn linux_model() -> regseq_model::RegseqModel {
    let mut model = model();
    model.device_lifecycle.drivers.values_mut().next().unwrap().ops =
        regseq_model::LifecycleOps::default();
    model.interrupt.drivers.values_mut().next().unwrap().resources[0]
        .binding
        .shared = false;
    model
}

fn zephyr_model() -> regseq_model::RegseqModel {
    let mut model = model();
    let compatible = model.strings.intern("regseq,netfull");
    let driver = model.device_lifecycle.drivers.values_mut().next().unwrap();
    driver.descriptor.bus_type = None;
    driver.descriptor.match_set = vec![regseq_model::MatchEntry::Devicetree {
        compatible,
    }];
    driver.ops = regseq_model::LifecycleOps::default();
    model.interrupt.drivers.clear();
    model.dma.drivers.clear();
    model
        .network_interface
        .drivers
        .values_mut()
        .next()
        .unwrap()
        .ops
        .set_published = None;
    model
}

fn visible(line: String) {
    use std::io::Write;
    let _ = writeln!(std::io::stderr().lock(), "{line}");
}

#[test]
fn network_linux_full_surface_builds_against_real_headers() {
    let emit = regseq_backends::emit_linux_c(&linux_model()).expect("emit Linux NIM");
    match kbuild::kbuild_module(&emit.tu, &emit.kbuild, &emit.module_name) {
        Ok(kbuild::KbuildOutcome::Skipped { notice }) => visible(notice),
        Ok(kbuild::KbuildOutcome::Built { kernel_version, warnings }) => {
            visible(format!("NIM kbuild PASS against kernel {kernel_version}"));
            if !warnings.trim().is_empty() {
                visible(warnings);
            }
        }
        Err(error) => panic!("NIM kbuild failed: {error}"),
    }
}

#[test]
fn network_zephyr_full_surface_builds_against_real_headers() {
    let emit = regseq_backends::emit_zephyr_c(&zephyr_model()).expect("emit Zephyr NIM");
    let driver = StagedDriver {
        tu: &emit.tu,
        binding_yaml: &emit.binding_yaml,
        driver_name: &emit.driver_name,
        compatible: "regseq,netfull",
        bus: StagedBus::Platform {
            region: (0x0904_0000, 0x1000),
            interrupts: None,
        },
    };
    match west_build(&driver) {
        Ok(WestOutcome::Skipped { notice }) => visible(notice),
        Ok(WestOutcome::Built { zephyr_version, warnings }) => {
            visible(format!(
                "NIM west PASS for {BOARD} / Zephyr {zephyr_version} (${WORKSPACE_VAR})"
            ));
            if !warnings.trim().is_empty() {
                visible(warnings);
            }
        }
        Err(error) => panic!("NIM west gate failed: {error}"),
    }
}
