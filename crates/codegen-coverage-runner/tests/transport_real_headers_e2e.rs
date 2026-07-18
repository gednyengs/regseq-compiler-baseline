#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::Path;

use codegen_coverage_runner::kbuild;
use codegen_coverage_runner::zephyr_build::{west_build, StagedBus, StagedDriver, WestOutcome};
use test_ir_fixtures::{try_bridge_fixture_path, BridgeOutcome};

fn model() -> regseq_model::RegseqModel {
    let input = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/_transport/transport-admission/input.rs");
    match try_bridge_fixture_path(&input) {
        BridgeOutcome::Ok(model) => model,
        other => panic!("transport-admission bridge failed: {other:?}"),
    }
}

fn visible(line: impl AsRef<str>) {
    use std::io::Write;
    let _ = writeln!(std::io::stderr().lock(), "{}", line.as_ref());
}

#[test]
fn transport_linux_admission_builds_against_real_headers() {
    let mut model = model();
    let driver = model.device_lifecycle.drivers.values_mut().next().unwrap();
    driver.descriptor.bus_type = Some(regseq_model::BusType::Amba);
    driver.descriptor.match_set = vec![regseq_model::MatchEntry::Amba {
        id: 0x0004_10d3,
        mask: 0x000f_ffff,
    }];
    let emit = regseq_backends::emit_linux_c(&model).expect("emit Linux transport");
    match kbuild::kbuild_module(&emit.tu, &emit.kbuild, &emit.module_name) {
        Ok(kbuild::KbuildOutcome::Skipped { notice }) => visible(notice),
        Ok(kbuild::KbuildOutcome::Built {
            kernel_version,
            warnings,
        }) => {
            visible(format!("DCT kbuild PASS against kernel {kernel_version}"));
            if !warnings.trim().is_empty() {
                visible(warnings);
            }
        }
        Err(err) => panic!("DCT kbuild gate failed: {err}"),
    }
}

#[test]
fn transport_zephyr_admission_builds_against_real_headers() {
    let emit = regseq_backends::emit_zephyr_c(&model()).expect("emit Zephyr transport");
    let driver = StagedDriver {
        tu: &emit.tu,
        binding_yaml: &emit.binding_yaml,
        driver_name: &emit.driver_name,
        compatible: "regseq,dct-admit",
        bus: StagedBus::Platform {
            region: (0x0905_0000, 0x1000),
            interrupts: None,
        },
    };
    match west_build(&driver) {
        Ok(WestOutcome::Skipped { notice }) => visible(notice),
        Ok(WestOutcome::Built {
            zephyr_version,
            warnings,
        }) => {
            visible(format!("DCT west PASS against Zephyr {zephyr_version}"));
            if !warnings.trim().is_empty() {
                visible(warnings);
            }
        }
        Err(err) => panic!("DCT west gate failed: {err}"),
    }
}
