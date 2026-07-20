#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::Path;

use codegen_coverage_runner::zephyr_build::{
    west_build, StagedBus, StagedDriver, WestOutcome, BOARD, WORKSPACE_VAR,
};
use codegen_coverage_runner::{kbuild, manifest};
use test_ir_fixtures::{try_bridge_fixture_path, BridgeOutcome};

fn fixture(name: &str) -> manifest::Fixture {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/_dma");
    manifest::discover(&dir)
        .expect("discover DMA corpus")
        .into_iter()
        .find(|fixture| fixture.manifest.name == name)
        .unwrap_or_else(|| panic!("missing DMA fixture {name}"))
}

fn model(name: &str) -> regseq_model::RegseqModel {
    let fixture = fixture(name);
    match try_bridge_fixture_path(&fixture.input) {
        BridgeOutcome::Ok(model) => model,
        other => panic!("{name} bridge failed: {other:?}"),
    }
}

fn linux_buffer_model() -> regseq_model::RegseqModel {
    let mut model = model("dma-buffer-e1000");
    let driver = model.device_lifecycle.drivers.values_mut().next().unwrap();
    driver.descriptor.bus_type = Some(regseq_model::BusType::Pci);
    // Header-check the native DMA binding without assigning the Generic-C
    // reference scenario's bodyless core helpers to the Linux backend.
    driver.ops = regseq_model::LifecycleOps::default();
    model
}

fn visible(line: String) {
    use std::io::Write;
    let _ = writeln!(std::io::stderr().lock(), "{line}");
}

fn kbuild_gate(label: &str, emit: &regseq_backends::LinuxEmit) {
    match kbuild::kbuild_module(&emit.tu, &emit.kbuild, &emit.module_name) {
        Ok(kbuild::KbuildOutcome::Skipped { notice }) => visible(format!("[{label}] {notice}")),
        Ok(kbuild::KbuildOutcome::Built {
            kernel_version,
            warnings,
        }) => {
            visible(format!(
                "[{label}] DMA kbuild PASS against kernel {kernel_version}"
            ));
            if !warnings.trim().is_empty() {
                visible(warnings);
            }
        }
        Err(err) => panic!("[{label}] {err}"),
    }
}

#[test]
fn dma_linux_locked_output_builds_against_real_headers() {
    let buffer =
        regseq_backends::emit_linux_c(&linux_buffer_model()).expect("emit Linux DMA buffer");
    kbuild_gate("dma-buffer-e1000", &buffer);

    let mut channel_model = model("dma-channel-pl011");
    let driver = channel_model
        .device_lifecycle
        .drivers
        .values_mut()
        .next()
        .unwrap();
    driver.descriptor.bus_type = Some(regseq_model::BusType::Amba);
    driver.descriptor.match_set = vec![regseq_model::MatchEntry::Amba {
        id: 0x0004_1011,
        mask: 0x000f_ffff,
    }];
    driver.ops = regseq_model::LifecycleOps::default();
    let channel = regseq_backends::emit_linux_c(&channel_model).expect("emit Linux DMA channel");
    kbuild_gate("dma-channel-pl011", &channel);
}

#[test]
fn dma_zephyr_locked_channel_builds_against_real_headers() {
    let mut model = model("dma-channel-pl011");
    model
        .device_lifecycle
        .drivers
        .values_mut()
        .next()
        .unwrap()
        .ops = regseq_model::LifecycleOps::default();
    let emit = regseq_backends::emit_zephyr_c(&model).expect("emit Zephyr DMA channel");
    let driver = StagedDriver {
        tu: &emit.tu,
        binding_yaml: &emit.binding_yaml,
        driver_name: &emit.driver_name,
        compatible: "regseq,dma-channel-pl011",
        bus: StagedBus::Platform {
            region: (0x0904_0000, 0x1000),
            interrupts: None,
        },
    };
    match west_build(&driver) {
        Ok(WestOutcome::Skipped { notice }) => visible(notice),
        Ok(WestOutcome::Built {
            zephyr_version,
            warnings,
        }) => {
            visible(format!(
                "DMA west PASS for {BOARD} / Zephyr {zephyr_version} (${WORKSPACE_VAR})"
            ));
            if !warnings.trim().is_empty() {
                visible(warnings);
            }
        }
        Err(err) => panic!("DMA west gate failed: {err}"),
    }
}
