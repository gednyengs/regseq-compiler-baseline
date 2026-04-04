//! PL011 full-spec lowering proof.
//!
//! Verifies that the accepted PL011 full-spec slice lowers successfully into
//! a portable `DriverLoweringPlan` and a Linux-specific `LinuxBackendPlan`
//! that carry real compiler-owned semantics.

use regseq_analysis::AnalysisInput;
use regseq_backends::linux::plan::{LinuxBackendPlan, LinuxFileRole, LinuxProbeStep};
use regseq_backends::linux::planner::build_linux_backend_plan;
use regseq_backends::lowering::plan::{BusType, DriverLoweringPlan, ResourceKind, ResourcePolicy};
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
    RegisterBlockDecl, RegisterDeclSet, RegisterDescriptorDecl, RegisterDescriptorFieldDecl,
    RegisterFieldDecl, WrapperCallSiteDecl, WrapperMethod,
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

// ---- PL011 full-spec declaration model ----

fn pl011_full_spec_decl() -> DeclarationModel {
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
                        name: "ibrd".into(),
                        access_width: Some(32),
                        endian: None,
                        side_effects: None,
                        hw_updates: None,
                    },
                    RegisterFieldDecl {
                        name: "fbrd".into(),
                        access_width: Some(32),
                        endian: None,
                        side_effects: None,
                        hw_updates: None,
                    },
                    RegisterFieldDecl {
                        name: "lcr_h".into(),
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
                    RegisterFieldDecl {
                        name: "ifls".into(),
                        access_width: Some(32),
                        endian: None,
                        side_effects: None,
                        hw_updates: None,
                    },
                    RegisterFieldDecl {
                        name: "imsc".into(),
                        access_width: Some(32),
                        endian: None,
                        side_effects: None,
                        hw_updates: None,
                    },
                    RegisterFieldDecl {
                        name: "mis".into(),
                        access_width: Some(32),
                        endian: None,
                        side_effects: None,
                        hw_updates: None,
                    },
                    RegisterFieldDecl {
                        name: "icr".into(),
                        access_width: Some(32),
                        endian: None,
                        side_effects: None,
                        hw_updates: None,
                    },
                ],
                provenance: Provenance::new("register_access"),
            }],
            descriptors: vec![
                RegisterDescriptorDecl {
                    register_module: "DR_REG".into(),
                    fields: vec![
                        RegisterDescriptorFieldDecl {
                            field_name: "DATA".into(),
                            offset: 0,
                            width: 8,
                            named_values: vec![],
                        },
                        RegisterDescriptorFieldDecl {
                            field_name: "OE".into(),
                            offset: 11,
                            width: 1,
                            named_values: vec![],
                        },
                        RegisterDescriptorFieldDecl {
                            field_name: "BE".into(),
                            offset: 10,
                            width: 1,
                            named_values: vec![],
                        },
                        RegisterDescriptorFieldDecl {
                            field_name: "PE".into(),
                            offset: 9,
                            width: 1,
                            named_values: vec![],
                        },
                        RegisterDescriptorFieldDecl {
                            field_name: "FE".into(),
                            offset: 8,
                            width: 1,
                            named_values: vec![],
                        },
                    ],
                    provenance: Provenance::new("register_access"),
                },
                RegisterDescriptorDecl {
                    register_module: "FR_REG".into(),
                    fields: vec![
                        RegisterDescriptorFieldDecl {
                            field_name: "TXFF".into(),
                            offset: 5,
                            width: 1,
                            named_values: vec![],
                        },
                        RegisterDescriptorFieldDecl {
                            field_name: "RXFE".into(),
                            offset: 4,
                            width: 1,
                            named_values: vec![],
                        },
                        RegisterDescriptorFieldDecl {
                            field_name: "BUSY".into(),
                            offset: 3,
                            width: 1,
                            named_values: vec![],
                        },
                    ],
                    provenance: Provenance::new("register_access"),
                },
            ],
            wrapper_call_sites: vec![
                WrapperCallSiteDecl {
                    method: WrapperMethod::Write,
                    containing_fn: "disable_uart".into(),
                    provenance: Provenance::new("register_access"),
                },
                WrapperCallSiteDecl {
                    method: WrapperMethod::Modify,
                    containing_fn: "disable_fifo".into(),
                    provenance: Provenance::new("register_access"),
                },
                WrapperCallSiteDecl {
                    method: WrapperMethod::Write,
                    containing_fn: "mask_all_interrupts".into(),
                    provenance: Provenance::new("register_access"),
                },
                WrapperCallSiteDecl {
                    method: WrapperMethod::Write,
                    containing_fn: "clear_all_interrupts".into(),
                    provenance: Provenance::new("register_access"),
                },
                WrapperCallSiteDecl {
                    method: WrapperMethod::Write,
                    containing_fn: "set_baud_rate".into(),
                    provenance: Provenance::new("register_access"),
                },
                WrapperCallSiteDecl {
                    method: WrapperMethod::Write,
                    containing_fn: "configure_line_control".into(),
                    provenance: Provenance::new("register_access"),
                },
                WrapperCallSiteDecl {
                    method: WrapperMethod::Write,
                    containing_fn: "enable_uart".into(),
                    provenance: Provenance::new("register_access"),
                },
                WrapperCallSiteDecl {
                    method: WrapperMethod::Modify,
                    containing_fn: "enable_rx_interrupts".into(),
                    provenance: Provenance::new("register_access"),
                },
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
                WrapperCallSiteDecl {
                    method: WrapperMethod::Read,
                    containing_fn: "tx_ready".into(),
                    provenance: Provenance::new("register_access"),
                },
                WrapperCallSiteDecl {
                    method: WrapperMethod::Read,
                    containing_fn: "tx_empty".into(),
                    provenance: Provenance::new("register_access"),
                },
                WrapperCallSiteDecl {
                    method: WrapperMethod::Read,
                    containing_fn: "flush_rx_fifo".into(),
                    provenance: Provenance::new("register_access"),
                },
                WrapperCallSiteDecl {
                    method: WrapperMethod::Modify,
                    containing_fn: "flush_tx_fifo".into(),
                    provenance: Provenance::new("register_access"),
                },
                WrapperCallSiteDecl {
                    method: WrapperMethod::Modify,
                    containing_fn: "send_break".into(),
                    provenance: Provenance::new("register_access"),
                },
            ],
            poll_call_sites: vec![],
        },
        properties: PropertyDeclSet {
            fields: vec![
                PropertyFieldDecl {
                    driver_type: "Pl011FullSpec".into(),
                    field_name: "current_speed".into(),
                    field_type: "u32".into(),
                    property_id: "current-speed".into(),
                    default_literal: Some("115200".into()),
                    vendor_prefix: None,
                    required: true,
                    provenance: Provenance::new("firmware_properties"),
                },
                PropertyFieldDecl {
                    driver_type: "Pl011FullSpec".into(),
                    field_name: "fifo_size".into(),
                    field_type: "u32".into(),
                    property_id: "fifo-size".into(),
                    default_literal: Some("32".into()),
                    vendor_prefix: None,
                    required: true,
                    provenance: Provenance::new("firmware_properties"),
                },
            ],
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

fn pl011_full_spec_binding() -> BindingModel {
    BindingModel {
        header: BindingHeader {
            version: "1".into(),
            os: "linux".into(),
            arch: "aarch64".into(),
            board: "raspberry-pi-4".into(),
            provenance: BindingProvenance::new("platform")
                .with_source_file("pl011-full-spec/platform.toml"),
        },
        properties: vec![
            PropertyBinding {
                key: "current-speed".into(),
                value: PlatformValue::Integer(115200),
                provenance: BindingProvenance::new("properties").with_section_key("current-speed"),
            },
            PropertyBinding {
                key: "fifo-size".into(),
                value: PlatformValue::Integer(32),
                provenance: BindingProvenance::new("properties").with_section_key("fifo-size"),
            },
        ],
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

fn make_full_spec_input() -> AnalysisInput {
    let decl = pl011_full_spec_decl();
    let binding = pl011_full_spec_binding();
    let bus_binding = binding.bus.clone();
    let joined = perform_join(&decl, &binding).expect("join should succeed");
    AnalysisInput::new(decl, joined, bus_binding)
}

fn make_portable_plan() -> DriverLoweringPlan {
    let input = make_full_spec_input();
    build_driver_lowering_plan(&input).expect("portable plan should succeed")
}

fn make_linux_plan() -> LinuxBackendPlan {
    let plan = make_portable_plan();
    build_linux_backend_plan(&plan).expect("linux plan should succeed")
}

// ---- Test 1: PL011 full-spec lowers into a portable plan ----

#[test]
fn full_spec_lowers_to_portable_plan() {
    let plan = make_portable_plan();
    assert_eq!(plan.identity.driver_name, "Pl011FullSpec");
    assert_eq!(plan.identity.compatible, vec!["arm,pl011"]);
    assert_eq!(plan.identity.device_class, DeviceClass::SerialUart);
}

// ---- Test 2: portable plan carries required obligations ----

#[test]
fn portable_plan_bus_identity_is_amba() {
    let plan = make_portable_plan();
    assert_eq!(
        plan.resources.bus_identity.bus_type,
        BusType::Amba,
        "PL011 bus must be AMBA, not Platform or Pci"
    );
}

#[test]
fn portable_plan_mmio_obligation() {
    let plan = make_portable_plan();
    assert_eq!(plan.resources.mmio.len(), 1);
    assert_eq!(plan.resources.mmio[0].mmio_id, "uart_regs");
    assert_eq!(
        plan.resources.mmio[0].register_block_type.as_deref(),
        Some("Pl011Registers")
    );
    assert_eq!(plan.resources.mmio[0].policy, ResourcePolicy::Required);
}

#[test]
fn portable_plan_interrupt_obligation() {
    let plan = make_portable_plan();
    assert_eq!(plan.resources.interrupts.len(), 1);
    assert_eq!(plan.resources.interrupts[0].interrupt_id, "uart_irq");
    assert_eq!(
        plan.resources.interrupts[0].trigger_type.as_deref(),
        Some("level")
    );
    assert_eq!(plan.resources.interrupts[0].shared, Some(false));
}

#[test]
fn portable_plan_property_obligations_with_defaults() {
    let plan = make_portable_plan();
    assert_eq!(plan.resources.properties.len(), 2);

    let speed = plan
        .resources
        .properties
        .iter()
        .find(|p| p.property_id == "current-speed")
        .expect("current-speed");
    assert_eq!(speed.default_literal.as_deref(), Some("115200"));
    assert_eq!(speed.policy, ResourcePolicy::OptionalWithFallback);

    let fifo = plan
        .resources
        .properties
        .iter()
        .find(|p| p.property_id == "fifo-size")
        .expect("fifo-size");
    assert_eq!(fifo.default_literal.as_deref(), Some("32"));
    assert_eq!(fifo.policy, ResourcePolicy::OptionalWithFallback);
}

#[test]
fn portable_plan_optional_resources() {
    let plan = make_portable_plan();

    // Clock is optional.
    assert_eq!(plan.resources.clocks.len(), 1);
    assert_eq!(plan.resources.clocks[0].clock_id, "uart_clk");
    assert!(!plan.resources.clocks[0].required);
    assert_eq!(
        plan.resources.clocks[0].policy,
        ResourcePolicy::OptionalSkip
    );

    // Reset is optional.
    assert_eq!(plan.resources.resets.len(), 1);
    assert!(!plan.resources.resets[0].required);
    assert_eq!(
        plan.resources.resets[0].policy,
        ResourcePolicy::OptionalSkip
    );

    // Pinctrl is optional.
    assert_eq!(plan.resources.pinctrl.len(), 1);
    assert!(!plan.resources.pinctrl[0].required);
    assert_eq!(
        plan.resources.pinctrl[0].policy,
        ResourcePolicy::OptionalSkip
    );
}

#[test]
fn portable_plan_callback_inventory() {
    let plan = make_portable_plan();
    assert_eq!(plan.callbacks.lifecycle.len(), 1);
    assert_eq!(plan.callbacks.serial.len(), 1);
    assert!(plan.callbacks.serial_console.is_empty());
    assert!(plan.callbacks.power_management.is_empty());
}

#[test]
fn portable_plan_register_block_obligation() {
    let plan = make_portable_plan();
    assert_eq!(plan.register_obligations.blocks.len(), 1);
    assert_eq!(plan.register_obligations.blocks[0].name, "Pl011Registers");
    assert_eq!(plan.register_obligations.blocks[0].field_count, 10);
}

#[test]
fn portable_plan_register_access_sites() {
    let plan = make_portable_plan();
    // The full-spec has 15 wrapper call sites across serial operations.
    assert_eq!(plan.register_obligations.access_sites.len(), 15);
}

#[test]
fn portable_plan_acquisition_ordering() {
    let plan = make_portable_plan();
    let kinds: Vec<&ResourceKind> = plan
        .acquisition_order
        .iter()
        .map(|s| &s.resource_kind)
        .collect();

    // Order: clocks -> resets -> pinctrl -> mmio -> properties -> interrupts
    assert_eq!(kinds[0], &ResourceKind::Clock);
    assert_eq!(kinds[kinds.len() - 1], &ResourceKind::Interrupt);

    // Teardown is reverse of acquisition.
    assert_eq!(plan.teardown_order.len(), plan.acquisition_order.len());
}

#[test]
fn portable_plan_serial_device_class_roles() {
    let plan = make_portable_plan();
    assert_eq!(
        plan.device_class_roles.device_class,
        DeviceClass::SerialUart
    );
    assert!(plan.device_class_roles.has_lifecycle);
}

// ---- Test 3: Linux plan preserves AMBA-specific identity ----

#[test]
fn linux_plan_uses_amba_registration() {
    let linux = make_linux_plan();
    assert!(
        linux.registration.is_amba,
        "PL011 Linux plan must use AMBA registration, not platform_driver"
    );
    assert!(linux.registration.driver_struct_name.contains("pl011"));
    assert_eq!(linux.registration.compatible_entries, vec!["arm,pl011"]);
    assert_eq!(linux.registration.module_license, "GPL");
}

#[test]
fn linux_plan_serial_core_realization() {
    let linux = make_linux_plan();
    let serial = linux
        .serial_core
        .as_ref()
        .expect("PL011 must have serial-core realization");
    assert!(serial.uart_driver_name.contains("pl011"));
    assert!(serial.uart_ops_name.contains("pl011"));
}

#[test]
fn linux_plan_probe_sequence() {
    let linux = make_linux_plan();
    assert!(!linux.probe.steps.is_empty());

    // Should end with RegisterUartPort for a serial device.
    assert!(matches!(
        linux.probe.steps.last().unwrap(),
        LinuxProbeStep::RegisterUartPort
    ));

    // Should include clock, mmio, interrupt steps.
    assert!(linux
        .probe
        .steps
        .iter()
        .any(|s| matches!(s, LinuxProbeStep::GetClock { .. })));
    assert!(linux
        .probe
        .steps
        .iter()
        .any(|s| matches!(s, LinuxProbeStep::MapMmio { .. })));
    assert!(linux
        .probe
        .steps
        .iter()
        .any(|s| matches!(s, LinuxProbeStep::RequestIrq { .. })));
}

#[test]
fn linux_plan_remove_sequence() {
    let linux = make_linux_plan();
    assert!(matches!(
        linux.remove.steps[0],
        regseq_backends::linux::plan::LinuxRemoveStep::UnregisterUartPort
    ));
}

#[test]
fn linux_plan_artifact_layout() {
    let linux = make_linux_plan();
    let roles: Vec<&LinuxFileRole> = linux
        .artifact_layout
        .emitted_files
        .iter()
        .map(|f| &f.role)
        .collect();
    assert!(roles.contains(&&LinuxFileRole::DriverMain));
    assert!(roles.contains(&&LinuxFileRole::SharedHeader));
    assert!(roles.contains(&&LinuxFileRole::Makefile));
    assert!(roles.contains(&&LinuxFileRole::KbuildFragment));
    assert!(roles.contains(&&LinuxFileRole::RegisterHelpers));
}

// ---- Test 4: optional resources and defaults visible in Linux plan ----

#[test]
fn linux_plan_optional_clock_probe_step() {
    let linux = make_linux_plan();
    let clock_step = linux
        .probe
        .steps
        .iter()
        .find(|s| matches!(s, LinuxProbeStep::GetClock { .. }))
        .expect("should have GetClock step");
    match clock_step {
        LinuxProbeStep::GetClock { optional, .. } => {
            assert!(optional, "clock should be optional");
        }
        _ => unreachable!(),
    }
}

#[test]
fn linux_plan_property_steps_carry_defaults() {
    let linux = make_linux_plan();
    let prop_steps: Vec<_> = linux
        .probe
        .steps
        .iter()
        .filter(|s| matches!(s, LinuxProbeStep::ReadProperty { .. }))
        .collect();
    assert_eq!(prop_steps.len(), 2);
    for step in prop_steps {
        match step {
            LinuxProbeStep::ReadProperty { has_default, .. } => {
                assert!(has_default, "both properties have defaults");
            }
            _ => unreachable!(),
        }
    }
}

// ---- Test 5: determinism ----

#[test]
fn portable_plan_is_deterministic() {
    let plan1 = make_portable_plan();
    let plan2 = make_portable_plan();
    assert_eq!(plan1.identity.driver_name, plan2.identity.driver_name);
    assert_eq!(plan1.acquisition_order.len(), plan2.acquisition_order.len());
    assert_eq!(plan1.resources.mmio.len(), plan2.resources.mmio.len());
}

#[test]
fn linux_plan_is_deterministic() {
    let linux1 = make_linux_plan();
    let linux2 = make_linux_plan();
    assert_eq!(
        linux1.registration.driver_struct_name,
        linux2.registration.driver_struct_name
    );
    assert_eq!(linux1.probe.steps.len(), linux2.probe.steps.len());
    assert_eq!(linux1.remove.steps.len(), linux2.remove.steps.len());
}
