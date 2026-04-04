//! Linux backend emission scaffold tests.
//!
//! Verifies that `LinuxBackendPlan` reaches the backend-emission entry
//! point, produces deterministic artifacts, and emits readable scaffold
//! C output.

use regseq_analysis::AnalysisInput;
use regseq_backends::linux::emitter::{emit_linux_artifacts, EmittedArtifactSet};
use regseq_backends::linux::plan::{LinuxBackendPlan, LinuxFileRole};
use regseq_backends::linux::planner::build_linux_backend_plan;
use regseq_backends::lowering::plan::DriverLoweringPlan;
use regseq_backends::lowering::planner::build_driver_lowering_plan;
use regseq_model::binding::{
    BindingHeader, BindingModel, BindingProvenance, BusBinding, ClockBinding, InterruptBinding,
    JoinKey, MmioBinding, PinCtrlBinding, PropertyBinding, ResetBinding,
};
use regseq_model::declaration::callbacks::{
    CallbackContext, CallbackDeclSet, CallbackFamily, CallbackImplDecl,
};
use regseq_model::declaration::driver::BusType as ModelBusType;
use regseq_model::declaration::operations::{
    DelayIntrinsic, DelayPathKind, DurationCtor, DurationCtorSiteDecl, LoggingDeclSet,
    PmGateDeclSet, TimedDelayCallSiteDecl, TimedDelayDeclSet,
};
use regseq_model::declaration::properties::{PropertyDeclSet, PropertyFieldDecl};
use regseq_model::declaration::provenance::Provenance;
use regseq_model::declaration::registers::{
    RegisterBlockDecl, RegisterDeclSet, RegisterFieldDecl, WrapperCallSiteDecl, WrapperMethod,
};
use regseq_model::declaration::resources::{
    ClockCallSiteDecl, ClockDeclSet, ClockFieldDecl, ClockMethod, InterruptBindingHints,
    InterruptDeclSet, InterruptFieldShape, InterruptResourceDecl, IsrBindTarget, IsrBindingDecl,
    MmioFieldDecl, PinCtrlDeclSet, PinCtrlFieldDecl, ResetCallSiteDecl, ResetDeclSet,
    ResetFieldDecl, ResetMethod, ResourceDeclSet,
};
use regseq_model::declaration::synchronization::SynchronizationDeclSet;
use regseq_model::declaration::{
    DeclarationBuildContext, DeclarationModel, DeviceClass, DriverDecl,
};
use regseq_model::joined::perform_join;
use regseq_model::platform::PlatformValue;
use tempfile::TempDir;

// ---- Shared fixture builder (mirrors pl011_full_spec_lowering.rs) ----

fn pl011_decl() -> DeclarationModel {
    DeclarationModel {
        driver: DriverDecl {
            driver_name: "Pl011FullSpec".into(),
            compatible: vec!["arm,pl011".into()],
            device_class: DeviceClass::SerialUart,
            bus: Some(ModelBusType::Amba),
            provenance: Provenance::new("identity"),
        },
        resources: ResourceDeclSet {
            mmio: vec![MmioFieldDecl {
                field_name: "regs".into(),
                mmio_id: Some("uart_regs".into()),
                register_block_type: Some("Pl011Registers".into()),
                provenance: Provenance::new("register_access"),
            }],
            interrupts: InterruptDeclSet {
                resources: vec![InterruptResourceDecl {
                    field_name: "irq".into(),
                    interrupt_id: "uart_irq".into(),
                    shape: InterruptFieldShape::Single,
                    binding_hints: InterruptBindingHints {
                        trigger_type: Some("level".into()),
                        trigger_polarity: Some("high".into()),
                        priority: None,
                        shared: Some(false),
                    },
                    provenance: Provenance::new("interrupt"),
                }],
                isr_bindings: vec![IsrBindingDecl {
                    handler_name: "handle_irq".into(),
                    bind_target: IsrBindTarget::Auto,
                    has_index_param: false,
                    context: CallbackContext::Framework,
                    provenance: Provenance::new("interrupt"),
                }],
            },
            clock: ClockDeclSet {
                fields: vec![ClockFieldDecl {
                    field_name: "clock".into(),
                    clock_id: "uart_clk".into(),
                    required: false,
                    provenance: Provenance::new("clock"),
                }],
                call_sites: vec![
                    ClockCallSiteDecl {
                        method: ClockMethod::Enable,
                        containing_fn: "enable_clock_if_present".into(),
                        provenance: Provenance::new("clock"),
                    },
                    ClockCallSiteDecl {
                        method: ClockMethod::Disable,
                        containing_fn: "disable_clock_if_present".into(),
                        provenance: Provenance::new("clock"),
                    },
                ],
            },
            reset: ResetDeclSet {
                fields: vec![ResetFieldDecl {
                    field_name: "reset".into(),
                    required: false,
                    provenance: Provenance::new("reset"),
                }],
                call_sites: vec![ResetCallSiteDecl {
                    method: ResetMethod::Toggle,
                    containing_fn: "toggle_reset_if_present".into(),
                    provenance: Provenance::new("reset"),
                }],
            },
            pinctrl: PinCtrlDeclSet {
                fields: vec![PinCtrlFieldDecl {
                    field_name: "pinctrl".into(),
                    required: false,
                    pinctrl_id: Some("uart_pins".into()),
                    provenance: Provenance::new("pinctrl"),
                }],
                call_sites: vec![],
            },
        },
        callbacks: CallbackDeclSet {
            lifecycle_impls: vec![CallbackImplDecl {
                driver_type: "Pl011FullSpec".into(),
                context: CallbackContext::Framework,
                family: CallbackFamily::Lifecycle,
                provenance: Provenance::new("callback"),
            }],
            serial_impls: vec![CallbackImplDecl {
                driver_type: "Pl011FullSpec".into(),
                context: CallbackContext::SubsystemSerialized,
                family: CallbackFamily::Serial,
                provenance: Provenance::new("callback"),
            }],
            serial_console_impls: vec![],
            power_management_impls: vec![],
        },
        registers: RegisterDeclSet {
            blocks: vec![RegisterBlockDecl {
                name: "Pl011Registers".into(),
                fields: vec![
                    RegisterFieldDecl {
                        name: "dr".into(),
                        access_width: Some(32),
                        endian: None,
                        side_effects: None,
                        hw_updates: None,
                    },
                    RegisterFieldDecl {
                        name: "fr".into(),
                        access_width: Some(32),
                        endian: None,
                        side_effects: None,
                        hw_updates: None,
                    },
                    RegisterFieldDecl {
                        name: "cr".into(),
                        access_width: Some(32),
                        endian: None,
                        side_effects: None,
                        hw_updates: None,
                    },
                ],
                provenance: Provenance::new("register_access"),
            }],
            descriptors: vec![],
            wrapper_call_sites: vec![
                WrapperCallSiteDecl {
                    method: WrapperMethod::Write,
                    containing_fn: "tx_byte".into(),
                    provenance: Provenance::new("register_access"),
                },
                WrapperCallSiteDecl {
                    method: WrapperMethod::Read,
                    containing_fn: "rx_char".into(),
                    provenance: Provenance::new("register_access"),
                },
            ],
            poll_call_sites: vec![],
        },
        properties: PropertyDeclSet {
            fields: vec![PropertyFieldDecl {
                driver_type: "Pl011FullSpec".into(),
                field_name: "current_speed".into(),
                field_type: "u32".into(),
                property_id: "current-speed".into(),
                default_literal: Some("115200".into()),
                vendor_prefix: None,
                required: true,
                provenance: Provenance::new("firmware_properties"),
            }],
            lints: vec![],
        },
        synchronization: SynchronizationDeclSet {
            sync_fields: vec![],
            lock_operations: vec![],
            completion_operations: vec![],
            guard_scopes: vec![],
        },
        timed_delay: TimedDelayDeclSet {
            call_sites: vec![TimedDelayCallSiteDecl {
                intrinsic: DelayIntrinsic::BusyWait,
                path_kind: DelayPathKind::Canonical,
                containing_fn: "toggle_reset_if_present".into(),
                provenance: Provenance::new("timed_delay"),
            }],
            duration_ctors: vec![DurationCtorSiteDecl {
                ctor: DurationCtor::FromUs,
                containing_fn: "toggle_reset_if_present".into(),
                provenance: Provenance::new("timed_delay"),
            }],
        },
        logging: LoggingDeclSet { call_sites: vec![] },
        pm_gate: PmGateDeclSet { gates: vec![] },
        build_context: DeclarationBuildContext {
            target_os: "linux".into(),
        },
    }
}

fn pl011_binding() -> BindingModel {
    BindingModel {
        header: BindingHeader {
            version: "1".into(),
            os: "linux".into(),
            arch: "aarch64".into(),
            board: "raspberry-pi-4".into(),
            provenance: BindingProvenance::new("platform")
                .with_source_file("pl011-full-spec/platform.toml"),
        },
        properties: vec![PropertyBinding {
            key: "current-speed".into(),
            value: PlatformValue::Integer(115200),
            provenance: BindingProvenance::new("properties").with_section_key("current-speed"),
        }],
        mmio: vec![MmioBinding {
            join_key: JoinKey::ExplicitId("uart_regs".into()),
            base: Some(0x0900_0000),
            size: Some(0x1000),
            provenance: BindingProvenance::new("mmio").with_section_key("uart_regs"),
        }],
        interrupts: vec![InterruptBinding {
            join_key: JoinKey::ExplicitId("uart_irq".into()),
            trigger_type: Some("level".into()),
            trigger_polarity: Some("high".into()),
            priority: None,
            shared: Some(false),
            provenance: BindingProvenance::new("interrupt").with_section_key("uart_irq"),
        }],
        clocks: vec![ClockBinding {
            join_key: JoinKey::ExplicitId("uart_clk".into()),
            default_frequency: Some(48_000_000),
            provenance: BindingProvenance::new("clock").with_section_key("uart_clk"),
        }],
        resets: vec![ResetBinding {
            join_key: JoinKey::FieldName("reset".into()),
            reset_line_id: Some("uart_rst0".into()),
            reset_shared: Some(false),
            provenance: BindingProvenance::new("reset").with_section_key("reset"),
        }],
        pinctrl: vec![PinCtrlBinding {
            join_key: JoinKey::ExplicitId("uart_pins".into()),
            pin_states: Some(vec!["default".into(), "sleep".into()]),
            provenance: BindingProvenance::new("pinctrl").with_section_key("uart_pins"),
        }],
        bus: Some(BusBinding {
            bus_type: ModelBusType::Amba,
            amba_id: Some(0x00041011),
            revision: Some(3),
            provenance: BindingProvenance::new("bus"),
        }),
    }
}

fn make_input() -> AnalysisInput {
    let decl = pl011_decl();
    let binding = pl011_binding();
    let bus_binding = binding.bus.clone();
    let joined = perform_join(&decl, &binding).expect("join should succeed");
    AnalysisInput::new(decl, joined, bus_binding)
}

fn make_plans() -> (DriverLoweringPlan, LinuxBackendPlan) {
    let input = make_input();
    let portable = build_driver_lowering_plan(&input).expect("portable plan");
    let linux = build_linux_backend_plan(&portable).expect("linux plan");
    (portable, linux)
}

fn emit_to_tempdir() -> (TempDir, EmittedArtifactSet) {
    let (portable, linux) = make_plans();
    let dir = TempDir::new().expect("tempdir");
    let set = emit_linux_artifacts(&linux, &portable, dir.path()).expect("emit");
    (dir, set)
}

// ---- Test 1: LinuxBackendPlan reaches emission entry point ----

#[test]
fn plan_reaches_emission_entry_point() {
    let (portable, linux) = make_plans();
    let dir = TempDir::new().expect("tempdir");
    let result = emit_linux_artifacts(&linux, &portable, dir.path());
    assert!(result.is_ok(), "emission should succeed");
}

// ---- Test 2: emitted artifact set and paths are deterministic ----

#[test]
fn emitted_artifacts_are_deterministic() {
    let (_dir1, set1) = emit_to_tempdir();
    let (_dir2, set2) = emit_to_tempdir();

    assert_eq!(set1.artifacts.len(), set2.artifacts.len());
    for (a, b) in set1.artifacts.iter().zip(set2.artifacts.iter()) {
        assert_eq!(
            a.path.file_name(),
            b.path.file_name(),
            "filenames must match"
        );
        assert_eq!(a.role, b.role, "roles must match");
        assert_eq!(a.size_bytes, b.size_bytes, "sizes must match");
    }
}

#[test]
fn emitted_content_is_deterministic() {
    let (dir1, set1) = emit_to_tempdir();
    let (dir2, set2) = emit_to_tempdir();

    for (a, b) in set1.artifacts.iter().zip(set2.artifacts.iter()) {
        let content_a = std::fs::read_to_string(&a.path).expect("read a");
        let content_b =
            std::fs::read_to_string(dir2.path().join(b.path.file_name().unwrap())).expect("read b");
        assert_eq!(
            content_a,
            content_b,
            "content of {} must be deterministic",
            a.path.display()
        );
    }
    drop(dir1);
}

// ---- Test 3: output root is repo-local output/linux/pl011-full-spec/ ----

#[test]
fn output_root_matches_artifact_set() {
    let (dir, set) = emit_to_tempdir();
    assert_eq!(set.output_root, dir.path());
    assert_eq!(set.driver_id, "Pl011FullSpec");
}

#[test]
fn repo_local_output_path_works() {
    let (portable, linux) = make_plans();
    let dir = TempDir::new().expect("tempdir");
    let output_root = dir.path().join("output/linux/pl011-full-spec");
    let set = emit_linux_artifacts(&linux, &portable, &output_root).expect("emit");
    assert!(output_root.exists());
    assert_eq!(set.output_root, output_root);
    for artifact in &set.artifacts {
        assert!(
            artifact.path.exists(),
            "{} should exist",
            artifact.path.display()
        );
    }
}

// ---- Test 4: emitted file partitioning matches backend contract ----

#[test]
fn emitted_files_match_declared_layout() {
    let (_dir, set) = emit_to_tempdir();
    let roles: Vec<&LinuxFileRole> = set.artifacts.iter().map(|a| &a.role).collect();

    assert!(roles.contains(&&LinuxFileRole::DriverMain));
    assert!(roles.contains(&&LinuxFileRole::SharedHeader));
    assert!(roles.contains(&&LinuxFileRole::Makefile));
    assert!(roles.contains(&&LinuxFileRole::KbuildFragment));
    assert!(roles.contains(&&LinuxFileRole::RegisterHelpers));
}

#[test]
fn emitted_filenames_follow_naming_convention() {
    let (_dir, set) = emit_to_tempdir();
    let filenames: Vec<String> = set
        .artifacts
        .iter()
        .map(|a| a.path.file_name().unwrap().to_string_lossy().into_owned())
        .collect();

    assert!(
        filenames.contains(&"regseq_pl011_full_spec_module.c".to_string()),
        "module integration file"
    );
    assert!(
        filenames.contains(&"regseq_pl011_full_spec_shared.h".to_string()),
        "shared header"
    );
    assert!(filenames.contains(&"Makefile".to_string()), "Makefile");
    assert!(filenames.contains(&"Kbuild".to_string()), "Kbuild");
    assert!(
        filenames.contains(&"pl011_full_spec_regs.h".to_string()),
        "register helpers"
    );
}

#[test]
fn all_emitted_files_are_nonempty() {
    let (_dir, set) = emit_to_tempdir();
    for artifact in &set.artifacts {
        assert!(
            artifact.size_bytes > 0,
            "{} should be non-empty",
            artifact.path.display()
        );
    }
}

// ---- Test 5: smoke-level content checks for readability ----

#[test]
fn module_integration_contains_amba_registration() {
    let (_dir, set) = emit_to_tempdir();
    let main = set
        .artifacts
        .iter()
        .find(|a| a.role == LinuxFileRole::DriverMain)
        .expect("driver main");
    let content = std::fs::read_to_string(&main.path).expect("read");

    assert!(
        content.contains("struct amba_driver"),
        "should use amba_driver for PL011"
    );
    assert!(
        content.contains("amba_driver_register"),
        "should use amba_driver_register for serial + AMBA"
    );
    assert!(
        content.contains("MODULE_LICENSE"),
        "should have module license"
    );
    assert!(
        content.contains("0x00041011"),
        "should contain AMBA peripheral ID"
    );
    assert!(
        content.contains("pl011_full_spec_probe"),
        "should have probe function"
    );
    assert!(
        content.contains("pl011_full_spec_remove"),
        "should have remove function"
    );
}

#[test]
fn module_integration_includes_shared_header() {
    let (_dir, set) = emit_to_tempdir();
    let main = set
        .artifacts
        .iter()
        .find(|a| a.role == LinuxFileRole::DriverMain)
        .expect("driver main");
    let content = std::fs::read_to_string(&main.path).expect("read");

    assert!(content.contains("#include \"regseq_pl011_full_spec_shared.h\""));
}

#[test]
fn module_integration_has_spdx_and_banner() {
    let (_dir, set) = emit_to_tempdir();
    let main = set
        .artifacts
        .iter()
        .find(|a| a.role == LinuxFileRole::DriverMain)
        .expect("driver main");
    let content = std::fs::read_to_string(&main.path).expect("read");

    assert!(content.contains("SPDX-License-Identifier: GPL-2.0"));
    assert!(content.contains("Generated by Regseq compiler"));
}

#[test]
fn shared_header_has_driver_state_struct() {
    let (_dir, set) = emit_to_tempdir();
    let header = set
        .artifacts
        .iter()
        .find(|a| a.role == LinuxFileRole::SharedHeader)
        .expect("shared header");
    let content = std::fs::read_to_string(&header.path).expect("read");

    assert!(content.contains("struct pl011_full_spec_data"));
    assert!(content.contains("void __iomem *"), "MMIO field");
    assert!(content.contains("struct uart_port"), "serial port field");
    assert!(content.contains("#ifndef"), "include guard");
    assert!(content.contains("#endif"), "include guard close");
}

#[test]
fn register_header_has_block_reference() {
    let (_dir, set) = emit_to_tempdir();
    let regs = set
        .artifacts
        .iter()
        .find(|a| a.role == LinuxFileRole::RegisterHelpers)
        .expect("register helpers");
    let content = std::fs::read_to_string(&regs.path).expect("read");

    assert!(content.contains("PL011_DR"), "register offset defines");
    assert!(content.contains("#ifndef"), "include guard");
}

#[test]
fn makefile_has_out_of_tree_build_rules() {
    let (_dir, set) = emit_to_tempdir();
    let mf = set
        .artifacts
        .iter()
        .find(|a| a.role == LinuxFileRole::Makefile)
        .expect("makefile");
    let content = std::fs::read_to_string(&mf.path).expect("read");

    assert!(content.contains("obj-m"));
    assert!(content.contains("$(MAKE)"));
    assert!(content.contains("modules"));
}

#[test]
fn kbuild_references_module_object() {
    let (_dir, set) = emit_to_tempdir();
    let kb = set
        .artifacts
        .iter()
        .find(|a| a.role == LinuxFileRole::KbuildFragment)
        .expect("kbuild");
    let content = std::fs::read_to_string(&kb.path).expect("read");

    assert!(content.contains("obj-m"));
    assert!(content.contains("regseq_pl011_full_spec_module.o"));
}

#[test]
fn probe_body_references_all_resource_steps() {
    let (_dir, set) = emit_to_tempdir();
    let main = set
        .artifacts
        .iter()
        .find(|a| a.role == LinuxFileRole::DriverMain)
        .expect("driver main");
    let content = std::fs::read_to_string(&main.path).expect("read");

    assert!(content.contains("devm_clk_get"), "clock acquisition");
    assert!(content.contains("devm_ioremap_resource"), "MMIO mapping");
    assert!(content.contains("devm_request_irq"), "IRQ request");
    assert!(content.contains("device_property_read"), "property read");
    assert!(content.contains("uart_add_one_port"), "UART registration");
}

#[test]
fn remove_body_references_teardown_steps() {
    let (_dir, set) = emit_to_tempdir();
    let main = set
        .artifacts
        .iter()
        .find(|a| a.role == LinuxFileRole::DriverMain)
        .expect("driver main");
    let content = std::fs::read_to_string(&main.path).expect("read");

    assert!(
        content.contains("uart_remove_one_port"),
        "UART unregistration"
    );
}

#[test]
fn module_integration_uses_correct_includes_for_amba() {
    let (_dir, set) = emit_to_tempdir();
    let main = set
        .artifacts
        .iter()
        .find(|a| a.role == LinuxFileRole::DriverMain)
        .expect("driver main");
    let content = std::fs::read_to_string(&main.path).expect("read");

    assert!(content.contains("#include <linux/amba/bus.h>"));
    assert!(content.contains("#include <linux/serial_core.h>"));
    assert!(content.contains("#include <linux/module.h>"));
    assert!(!content.contains("platform_device.h"));
}

// ---- New tests for real emission content ----

#[test]
fn module_has_uart_ops_definition() {
    let (_dir, set) = emit_to_tempdir();
    let main = set
        .artifacts
        .iter()
        .find(|a| a.role == LinuxFileRole::DriverMain)
        .expect("driver main");
    let content = std::fs::read_to_string(&main.path).expect("read");

    assert!(content.contains("struct uart_ops"), "uart_ops struct");
    assert!(content.contains("tx_empty"), "tx_empty op");
    assert!(content.contains("start_tx"), "start_tx op");
    assert!(content.contains("stop_tx"), "stop_tx op");
    assert!(content.contains("startup"), "startup op");
    assert!(content.contains("shutdown"), "shutdown op");
    assert!(content.contains("set_termios"), "set_termios op");
}

#[test]
fn module_has_uart_driver_definition() {
    let (_dir, set) = emit_to_tempdir();
    let main = set
        .artifacts
        .iter()
        .find(|a| a.role == LinuxFileRole::DriverMain)
        .expect("driver main");
    let content = std::fs::read_to_string(&main.path).expect("read");

    assert!(content.contains("struct uart_driver"), "uart_driver struct");
    assert!(
        content.contains("uart_register_driver"),
        "uart registration"
    );
    assert!(content.contains("uart_unregister_driver"), "uart cleanup");
}

#[test]
fn module_has_isr_handler() {
    let (_dir, set) = emit_to_tempdir();
    let main = set
        .artifacts
        .iter()
        .find(|a| a.role == LinuxFileRole::DriverMain)
        .expect("driver main");
    let content = std::fs::read_to_string(&main.path).expect("read");

    assert!(content.contains("irqreturn_t"), "ISR return type");
    assert!(content.contains("irq_handler"), "ISR function");
    assert!(content.contains("IRQ_HANDLED"), "IRQ_HANDLED");
    assert!(content.contains("IRQ_NONE"), "IRQ_NONE");
    assert!(content.contains("PL011_MIS"), "reads MIS register");
}

#[test]
fn module_has_register_accessors() {
    let (_dir, set) = emit_to_tempdir();
    let main = set
        .artifacts
        .iter()
        .find(|a| a.role == LinuxFileRole::DriverMain)
        .expect("driver main");
    let content = std::fs::read_to_string(&main.path).expect("read");

    assert!(content.contains("readl("), "readl call");
    assert!(content.contains("writel("), "writel call");
    assert!(content.contains("reg_read"), "read accessor");
    assert!(content.contains("reg_write"), "write accessor");
    assert!(content.contains("reg_modify"), "modify accessor");
}

#[test]
fn register_header_has_pl011_offsets() {
    let (_dir, set) = emit_to_tempdir();
    let regs = set
        .artifacts
        .iter()
        .find(|a| a.role == LinuxFileRole::RegisterHelpers)
        .expect("register helpers");
    let content = std::fs::read_to_string(&regs.path).expect("read");

    assert!(content.contains("PL011_DR"), "DR offset");
    assert!(content.contains("PL011_FR"), "FR offset");
    assert!(content.contains("PL011_CR"), "CR offset");
    assert!(content.contains("PL011_IMSC"), "IMSC offset");
    assert!(content.contains("PL011_MIS"), "MIS offset");
    assert!(content.contains("PL011_ICR"), "ICR offset");
    assert!(content.contains("0x000"), "DR at offset 0");
    assert!(content.contains("0x018"), "FR at offset 0x018");
}

#[test]
fn register_header_has_convenience_defines() {
    let (_dir, set) = emit_to_tempdir();
    let regs = set
        .artifacts
        .iter()
        .find(|a| a.role == LinuxFileRole::RegisterHelpers)
        .expect("register helpers");
    let content = std::fs::read_to_string(&regs.path).expect("read");

    assert!(content.contains("PL011_LCRH_WLEN_8"), "word length 8");
    assert!(content.contains("PL011_LCRH_FEN"), "FIFO enable");
    assert!(content.contains("PL011_CR_UARTEN"), "UART enable");
    assert!(content.contains("PL011_CR_TXE"), "TX enable");
    assert!(content.contains("PL011_CR_RXE"), "RX enable");
}

#[test]
fn shared_header_has_property_fields() {
    let (_dir, set) = emit_to_tempdir();
    let header = set
        .artifacts
        .iter()
        .find(|a| a.role == LinuxFileRole::SharedHeader)
        .expect("shared header");
    let content = std::fs::read_to_string(&header.path).expect("read");

    assert!(content.contains("current_speed"), "property field");
}

#[test]
fn module_has_module_init_exit() {
    let (_dir, set) = emit_to_tempdir();
    let main = set
        .artifacts
        .iter()
        .find(|a| a.role == LinuxFileRole::DriverMain)
        .expect("driver main");
    let content = std::fs::read_to_string(&main.path).expect("read");

    assert!(content.contains("module_init("), "module_init");
    assert!(content.contains("module_exit("), "module_exit");
}

#[test]
fn probe_has_real_api_calls() {
    let (_dir, set) = emit_to_tempdir();
    let main = set
        .artifacts
        .iter()
        .find(|a| a.role == LinuxFileRole::DriverMain)
        .expect("driver main");
    let content = std::fs::read_to_string(&main.path).expect("read");

    assert!(content.contains("devm_kzalloc("), "allocates driver data");
    assert!(content.contains("IS_ERR("), "error checking");
    assert!(content.contains("PTR_ERR("), "error propagation");
    assert!(content.contains("amba_set_drvdata("), "sets driver data");
}
