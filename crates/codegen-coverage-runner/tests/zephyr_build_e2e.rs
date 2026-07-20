//! The L1 west build gate over the COMPOSED zephyr goldens (backend-zephyr-c
//! T11; GRILL C03/D08, C01.2).
//!
//! Stages the committed `golden.zephyr.c` + `golden.zephyr.binding.yaml` of
//! the converged `pl011-composed` (spec-convergence-pl011 C05) as a hermetic
//! app and builds it via west for
//! `qemu_cortex_a53` under the `RSEQ_ZEPHYR_WORKSPACE` contract. Gating
//! acceptance is over the composed TU ONLY — the init-fail variant's image
//! build belongs to T16 (C4), no early probe (plan review round-1 fix).
//!
//! C03 semantics, observable here:
//! - unset var ⇒ ONE visible skip notice (test passes — workspace-less
//!   machines and CI stay green, I02);
//! - set-but-invalid ⇒ the test FAILS (misconfiguration never masquerades
//!   as a skip);
//! - build error ⇒ FAILS with the build stderr verbatim;
//! - success ⇒ the actual Zephyr version is printed in the notice.
//!
//! No bridging happens here (the gate consumes the COMMITTED goldens), but
//! the rustc_private anchor is still required to link the runner library.

#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::{Path, PathBuf};

use codegen_coverage_runner::zephyr_build::{
    west_build, StagedBus, StagedDriver, WestOutcome, BOARD, PCIE_BOARD, WORKSPACE_VAR,
};
use codegen_coverage_runner::{manifest, pipeline};
use test_ir_fixtures::{try_bridge_fixture_path, BridgeOutcome};

fn composed_corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/_composed")
}

/// Print to the REAL stderr (bypassing libtest's capture): the C03 skip
/// notice must be visible on a green run, not swallowed (the kbuild gate's
/// precedent; C3 review finding #1).
fn visible(line: String) {
    use std::io::Write;
    let mut err = std::io::stderr().lock();
    let _ = writeln!(err, "{line}");
}

#[test]
fn zephyr_build_gate_over_the_composed_goldens() {
    let fixtures = manifest::discover(&composed_corpus()).expect("discover composed corpus");
    let fixture = fixtures
        .iter()
        .find(|f| f.manifest.name == "pl011-composed")
        .expect("pl011-composed fixture discovered");

    let tu = std::fs::read_to_string(pipeline::zephyr_golden_path(fixture))
        .expect("read the committed golden.zephyr.c");
    let binding_yaml = std::fs::read_to_string(pipeline::zephyr_binding_path(fixture))
        .expect("read the committed golden.zephyr.binding.yaml");

    let driver = StagedDriver {
        tu: &tu,
        binding_yaml: &binding_yaml,
        driver_name: "pl011",
        compatible: "regseq,pl011",
        // The eval DUT (D04/D11) — must agree with the fixture platform.toml
        // (the emitted A-1 BUILD_ASSERT cross-checks the overlay's reg
        // against these same literals at C compile time).
        bus: StagedBus::Platform {
            region: (0x0904_0000, 0x1000),
            interrupts: None,
        },
    };

    match west_build(&driver) {
        Ok(WestOutcome::Skipped { notice }) => {
            // The ONE visible notice (C03) — never a silent pass.
            visible(notice);
        }
        Ok(WestOutcome::Built {
            zephyr_version,
            warnings,
        }) => {
            visible(format!(
                "west gate PASSED: the composed TU built for {BOARD} against Zephyr \
                 {zephyr_version} (workspace: ${WORKSPACE_VAR})"
            ));
            if !warnings.trim().is_empty() {
                // Warnings are recorded, not gating (C01.2).
                visible(format!("--- build warnings (recorded) ---\n{warnings}"));
            }
        }
        Err(err) => panic!("west gate FAILED:\n{err}"),
    }
}

#[test]
fn zephyr_build_gate_over_the_irq_goldens() {
    // The device-interrupt TU (construct #6, T11): IRQ_CONNECT +
    // DT_INST_IRQ(0, priority) + irq_enable/irq_disable + the void ISR
    // wrapper must compile against the same workspace. The overlay node
    // carries well-formed `interrupts` cells (raw `<GIC_SPI 8
    // IRQ_TYPE_LEVEL 0>`): the west gate is a COMPILE check — the runtime
    // line number is the eval campaign's to pin (T14).
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/_irq");
    let fixtures = manifest::discover(&dir).expect("discover irq corpus");
    let fixture = fixtures
        .iter()
        .find(|f| f.manifest.name == "irq-pl011")
        .expect("irq-pl011 fixture discovered");

    let tu = std::fs::read_to_string(pipeline::zephyr_golden_path(fixture))
        .expect("read the committed golden.zephyr.c");
    let binding_yaml = std::fs::read_to_string(pipeline::zephyr_binding_path(fixture))
        .expect("read the committed golden.zephyr.binding.yaml");

    let driver = StagedDriver {
        tu: &tu,
        binding_yaml: &binding_yaml,
        driver_name: "pl011irq",
        compatible: "regseq,pl011irq",
        bus: StagedBus::Platform {
            region: (0x0904_0000, 0x1000),
            interrupts: Some("<0 8 2 0>"),
        },
    };

    match west_build(&driver) {
        Ok(WestOutcome::Skipped { notice }) => {
            visible(notice);
        }
        Ok(WestOutcome::Built {
            zephyr_version,
            warnings,
        }) => {
            visible(format!(
                "west gate PASSED: the irq TU built for {BOARD} against Zephyr \
                 {zephyr_version} (workspace: ${WORKSPACE_VAR})"
            ));
            if !warnings.trim().is_empty() {
                visible(format!("--- build warnings (recorded) ---\n{warnings}"));
            }
        }
        Err(err) => panic!("west gate FAILED:\n{err}"),
    }
}

/// The PCIe variant (backend-zephyr-c-e1000 T07/R09): BOTH generated e1000
/// TUs — the composed PCI keystone and the induced-failure variant — build
/// for qemu_x86 against the same v4.4.0 workspace, each staged with its
/// pcie0 DUT overlay and the O4 CONFIG_PCIE pin set. The C03 env semantics
/// are identical to the pl011 gate above (one visible skip notice; a
/// misconfigured workspace FAILS; build stderr verbatim).
#[test]
fn zephyr_build_gate_over_the_e1000_goldens() {
    let fixtures = manifest::discover(&composed_corpus()).expect("discover composed corpus");
    for (name, driver_name, compatible, device_id) in [
        ("e1000-composed", "e1000", "regseq,e1000", 0x100e_u16),
        (
            "e1000-initfail",
            "e1000fail",
            "regseq,e1000fail",
            0x100e_u16,
        ),
    ] {
        let fixture = fixtures
            .iter()
            .find(|f| f.manifest.name == name)
            .unwrap_or_else(|| panic!("{name} fixture discovered"))
            .with_binding(manifest::OsBinding::Zephyr)
            .expect("the converged fixture resolves the zephyr binding (C05)");
        let tu = std::fs::read_to_string(pipeline::zephyr_golden_path(&fixture))
            .expect("read the committed golden.zephyr.c");
        let binding_yaml = std::fs::read_to_string(pipeline::zephyr_binding_path(&fixture))
            .expect("read the committed golden.zephyr.binding.yaml");
        let driver = StagedDriver {
            tu: &tu,
            binding_yaml: &binding_yaml,
            driver_name,
            compatible,
            // The hermetic DUT identity (D08(4)): the QEMU `-device e1000`
            // default id pair; the overlay node is matched ONLY by the
            // generated compatible.
            bus: StagedBus::Pcie {
                vendor_id: 0x8086,
                device_id,
            },
        };
        match west_build(&driver) {
            Ok(WestOutcome::Skipped { notice }) => {
                visible(format!("[{name}] {notice}"));
            }
            Ok(WestOutcome::Built {
                zephyr_version,
                warnings,
            }) => {
                visible(format!(
                    "west gate PASSED: the {name} TU built for {PCIE_BOARD} against \
                     Zephyr {zephyr_version} (workspace: ${WORKSPACE_VAR})"
                ));
                if !warnings.trim().is_empty() {
                    // Warnings are recorded, not gating.
                    visible(format!("--- build warnings (recorded) ---\n{warnings}"));
                }
            }
            Err(err) => panic!("west gate FAILED for {name}:\n{err}"),
        }
    }
}

#[test]
fn zephyr_build_gate_over_deferred_work() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/_deferred_work");
    let fixture = manifest::discover(&dir)
        .expect("discover deferred-work corpus")
        .into_iter()
        .find(|f| f.manifest.name == "deferred-work-reset")
        .expect("deferred-work-reset fixture discovered");
    let model = match try_bridge_fixture_path(&fixture.input) {
        BridgeOutcome::Ok(model) => model,
        other => panic!("deferred-work-reset bridge failed: {other:?}"),
    };
    let emit = regseq_backends::emit_zephyr_c(&model).expect("emit deferred work for Zephyr");
    let driver = StagedDriver {
        tu: &emit.tu,
        binding_yaml: &emit.binding_yaml,
        driver_name: "dwreset",
        compatible: "regseq,dwreset",
        bus: StagedBus::Platform {
            region: (0x0904_0000, 0x1000),
            interrupts: None,
        },
    };

    match west_build(&driver) {
        Ok(WestOutcome::Skipped { notice }) => visible(format!("[deferred-work-reset] {notice}")),
        Ok(WestOutcome::Built {
            zephyr_version,
            warnings,
        }) => {
            visible(format!(
                "west gate PASSED: the deferred-work TU built for {BOARD} against Zephyr \
                 {zephyr_version} (workspace: ${WORKSPACE_VAR})"
            ));
            if !warnings.trim().is_empty() {
                visible(format!("--- build warnings (recorded) ---\n{warnings}"));
            }
        }
        Err(err) => panic!("west gate FAILED for deferred-work-reset:\n{err}"),
    }
}

#[test]
fn zephyr_build_gate_over_locked_serial_port() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/_serial");
    let fixture = manifest::discover(&dir)
        .expect("discover serial corpus")
        .into_iter()
        .find(|f| f.manifest.name == "serial-port-all")
        .expect("serial-port-all fixture discovered");
    let model = match try_bridge_fixture_path(&fixture.input) {
        BridgeOutcome::Ok(model) => model,
        other => panic!("serial-port-all bridge failed: {other:?}"),
    };
    let emit = regseq_backends::emit_zephyr_c(&model).expect("emit locked serial port for Zephyr");
    let driver = StagedDriver {
        tu: &emit.tu,
        binding_yaml: &emit.binding_yaml,
        driver_name: "serial_port_all",
        compatible: "regseq,serial-port-all",
        bus: StagedBus::Platform {
            region: (0x0904_0000, 0x1000),
            interrupts: None,
        },
    };

    match west_build(&driver) {
        Ok(WestOutcome::Skipped { notice }) => visible(format!("[serial-port-all] {notice}")),
        Ok(WestOutcome::Built {
            zephyr_version,
            warnings,
        }) => {
            visible(format!(
                "west gate PASSED: locked serial-port TU built for {BOARD} against Zephyr \
                 {zephyr_version} (workspace: ${WORKSPACE_VAR})"
            ));
            if !warnings.trim().is_empty() {
                visible(format!("--- build warnings (recorded) ---\n{warnings}"));
            }
        }
        Err(err) => panic!("west gate FAILED for serial-port-all:\n{err}"),
    }
}
