//! PL011 full-spec lowering proof.
//!
//! Verifies that the accepted PL011 full-spec slice lowers successfully into
//! a portable `DriverLoweringPlan` and a Linux-specific `LinuxBackendPlan`
//! that carry real compiler-owned semantics.

use regseq_analysis::AnalysisInput;
use regseq_backends::linux::plan::{LinuxBackendPlan, LinuxFileRole, LinuxProbeStep};
use regseq_backends::linux::planner::build_linux_backend_plan;
use regseq_backends::lowering::behavior::{FunctionRole, ReturnSemantic};
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
    RegisterFieldDecl, RegisterNamedValueDecl, WrapperCallSiteDecl, WrapperMethod,
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
                RegisterDescriptorDecl {
                    register_module: "LCRH_REG".into(),
                    fields: vec![
                        RegisterDescriptorFieldDecl {
                            field_name: "WLEN".into(),
                            offset: 5,
                            width: 2,
                            named_values: vec![
                                RegisterNamedValueDecl {
                                    name: "FIVE_BITS".into(),
                                    value: 0b00,
                                },
                                RegisterNamedValueDecl {
                                    name: "SIX_BITS".into(),
                                    value: 0b01,
                                },
                                RegisterNamedValueDecl {
                                    name: "SEVEN_BITS".into(),
                                    value: 0b10,
                                },
                                RegisterNamedValueDecl {
                                    name: "EIGHT_BITS".into(),
                                    value: 0b11,
                                },
                            ],
                        },
                        RegisterDescriptorFieldDecl {
                            field_name: "FEN".into(),
                            offset: 4,
                            width: 1,
                            named_values: vec![
                                RegisterNamedValueDecl {
                                    name: "DISABLED".into(),
                                    value: 0,
                                },
                                RegisterNamedValueDecl {
                                    name: "ENABLED".into(),
                                    value: 1,
                                },
                            ],
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

// ---- Test 6: register layout carries offsets and bitfield info ----

#[test]
fn portable_plan_register_fields_carry_byte_offsets() {
    let plan = make_portable_plan();
    let block = &plan.register_obligations.blocks[0];
    assert_eq!(block.name, "Pl011Registers");
    assert_eq!(block.register_fields.len(), 10);

    // First field (dr) at offset 0.
    assert_eq!(block.register_fields[0].name, "dr");
    assert_eq!(block.register_fields[0].byte_offset, 0);
    assert_eq!(block.register_fields[0].access_width, Some(32));

    // Second field (fr) at offset 4 (sequential 32-bit stride).
    assert_eq!(block.register_fields[1].name, "fr");
    assert_eq!(block.register_fields[1].byte_offset, 4);

    // Last field (icr) at offset 36.
    assert_eq!(block.register_fields[9].name, "icr");
    assert_eq!(block.register_fields[9].byte_offset, 36);
}

#[test]
fn portable_plan_bitfield_descriptors_preserve_offsets_and_widths() {
    let plan = make_portable_plan();
    let dr = plan
        .register_obligations
        .descriptors
        .iter()
        .find(|d| d.register_name == "DR_REG")
        .expect("should have DR_REG descriptor");

    let data_field = dr
        .bit_fields
        .iter()
        .find(|f| f.name == "DATA")
        .expect("DATA field");
    assert_eq!(data_field.offset, 0);
    assert_eq!(data_field.width, 8);

    let oe_field = dr
        .bit_fields
        .iter()
        .find(|f| f.name == "OE")
        .expect("OE field");
    assert_eq!(oe_field.offset, 11);
    assert_eq!(oe_field.width, 1);
}

#[test]
fn portable_plan_bitfield_named_values_preserved() {
    let plan = make_portable_plan();
    let lcrh = plan
        .register_obligations
        .descriptors
        .iter()
        .find(|d| d.register_name == "LCRH_REG")
        .expect("should have LCRH_REG descriptor");

    let wlen = lcrh
        .bit_fields
        .iter()
        .find(|f| f.name == "WLEN")
        .expect("WLEN field");
    assert_eq!(wlen.offset, 5);
    assert_eq!(wlen.width, 2);
    assert_eq!(wlen.named_values.len(), 4);

    let eight_bits = wlen
        .named_values
        .iter()
        .find(|nv| nv.name == "EIGHT_BITS")
        .expect("EIGHT_BITS value");
    assert_eq!(eight_bits.value, 0b11);

    let fen = lcrh
        .bit_fields
        .iter()
        .find(|f| f.name == "FEN")
        .expect("FEN field");
    assert_eq!(fen.named_values.len(), 2);
}

// ---- Test 7: property plans preserve field names, types, and defaults ----

#[test]
fn portable_plan_property_field_names_and_types() {
    let plan = make_portable_plan();

    let speed = plan
        .resources
        .properties
        .iter()
        .find(|p| p.property_id == "current-speed")
        .expect("current-speed");
    assert_eq!(speed.field_name, "current_speed");
    assert_eq!(speed.field_type, "u32");
    assert!(speed.required);
    assert_eq!(speed.default_literal.as_deref(), Some("115200"));

    let fifo = plan
        .resources
        .properties
        .iter()
        .find(|p| p.property_id == "fifo-size")
        .expect("fifo-size");
    assert_eq!(fifo.field_name, "fifo_size");
    assert_eq!(fifo.field_type, "u32");
    assert!(fifo.required);
    assert_eq!(fifo.default_literal.as_deref(), Some("32"));
}

// ---- Test 8: clock/reset/pinctrl deployment metadata preserved ----

#[test]
fn portable_plan_clock_carries_default_frequency() {
    let plan = make_portable_plan();
    let clock = &plan.resources.clocks[0];
    assert_eq!(clock.clock_id, "uart_clk");
    assert!(!clock.required);
    assert_eq!(clock.default_frequency, Some(48_000_000));
}

#[test]
fn portable_plan_reset_carries_deployment_metadata() {
    let plan = make_portable_plan();
    let reset = &plan.resources.resets[0];
    assert_eq!(reset.field_name, "reset");
    assert!(!reset.required);
    assert_eq!(reset.reset_line_id.as_deref(), Some("uart_rst0"));
    assert_eq!(reset.reset_shared, Some(false));
}

#[test]
fn portable_plan_pinctrl_carries_states() {
    let plan = make_portable_plan();
    let pin = &plan.resources.pinctrl[0];
    assert_eq!(pin.field_name, "pinctrl");
    assert!(!pin.required);
    assert_eq!(pin.pinctrl_id.as_deref(), Some("uart_pins"));
    assert_eq!(
        pin.pin_states.as_deref(),
        Some(vec!["default".to_string(), "sleep".to_string()].as_slice())
    );
}

#[test]
fn portable_plan_bus_identity_carries_amba_revision() {
    let plan = make_portable_plan();
    assert_eq!(plan.resources.bus_identity.amba_id, Some(0x00041011));
    assert_eq!(plan.resources.bus_identity.amba_revision, Some(3));
}

// ---- Test 9: Linux plan carries realization data without raw model access ----

#[test]
fn linux_plan_register_layout_carries_offsets() {
    let linux = make_linux_plan();
    assert_eq!(linux.register_layout.blocks.len(), 1);

    let block = &linux.register_layout.blocks[0];
    assert_eq!(block.block_name, "Pl011Registers");
    assert_eq!(block.fields.len(), 10);
    assert_eq!(block.fields[0].name, "dr");
    assert_eq!(block.fields[0].byte_offset, 0);
    assert_eq!(block.fields[0].access_width_bytes, 4);
}

#[test]
fn linux_plan_register_descriptors_carry_named_values() {
    let linux = make_linux_plan();
    let lcrh = linux
        .register_layout
        .descriptors
        .iter()
        .find(|d| d.register_name == "LCRH_REG")
        .expect("LCRH_REG");

    let wlen = lcrh
        .bit_fields
        .iter()
        .find(|f| f.name == "WLEN")
        .expect("WLEN");
    assert_eq!(wlen.bit_offset, 5);
    assert_eq!(wlen.bit_width, 2);
    assert_eq!(wlen.named_values.len(), 4);
}

#[test]
fn linux_plan_property_realization_carries_api_and_defaults() {
    let linux = make_linux_plan();
    assert_eq!(linux.property_realization.len(), 2);

    let speed = linux
        .property_realization
        .iter()
        .find(|p| p.property_id == "current-speed")
        .expect("current-speed");
    assert_eq!(speed.field_name, "current_speed");
    assert_eq!(speed.c_type, "u32");
    assert_eq!(speed.linux_read_api, "device_property_read_u32");
    assert_eq!(speed.default_literal.as_deref(), Some("115200"));
}

#[test]
fn linux_plan_deployment_metadata_carries_clock_frequency() {
    let linux = make_linux_plan();
    assert_eq!(linux.deployment_metadata.clocks.len(), 1);

    let clock = &linux.deployment_metadata.clocks[0];
    assert_eq!(clock.clock_id, "uart_clk");
    assert!(!clock.required);
    assert_eq!(clock.default_frequency_hz, Some(48_000_000));
}

#[test]
fn linux_plan_deployment_metadata_carries_reset_info() {
    let linux = make_linux_plan();
    assert_eq!(linux.deployment_metadata.resets.len(), 1);

    let reset = &linux.deployment_metadata.resets[0];
    assert_eq!(reset.field_name, "reset");
    assert!(!reset.required);
    assert_eq!(reset.reset_line_id.as_deref(), Some("uart_rst0"));
    assert_eq!(reset.shared, Some(false));
}

#[test]
fn linux_plan_deployment_metadata_carries_pinctrl_states() {
    let linux = make_linux_plan();
    assert_eq!(linux.deployment_metadata.pinctrl.len(), 1);

    let pin = &linux.deployment_metadata.pinctrl[0];
    assert_eq!(pin.field_name, "pinctrl");
    assert!(!pin.required);
    assert_eq!(
        pin.states.as_deref(),
        Some(vec!["default".to_string(), "sleep".to_string()].as_slice())
    );
}

// ---- Test 10: realization data determinism ----

#[test]
fn realization_data_is_deterministic() {
    let plan1 = make_portable_plan();
    let plan2 = make_portable_plan();

    // Register field offsets are deterministic.
    assert_eq!(
        plan1.register_obligations.blocks[0].register_fields.len(),
        plan2.register_obligations.blocks[0].register_fields.len()
    );
    for (f1, f2) in plan1.register_obligations.blocks[0]
        .register_fields
        .iter()
        .zip(plan2.register_obligations.blocks[0].register_fields.iter())
    {
        assert_eq!(f1.name, f2.name);
        assert_eq!(f1.byte_offset, f2.byte_offset);
    }

    // Linux property realization is deterministic.
    let linux1 = make_linux_plan();
    let linux2 = make_linux_plan();
    assert_eq!(
        linux1.property_realization.len(),
        linux2.property_realization.len()
    );
    assert_eq!(
        linux1.register_layout.blocks.len(),
        linux2.register_layout.blocks.len()
    );
}

// ---- Test 11: behavior plan scoped to individual functions ----

#[test]
fn behavior_plan_has_per_function_entries() {
    use regseq_backends::lowering::behavior::FunctionRole;

    let plan = make_portable_plan();
    let b = &plan.behavior;

    // Should have lifecycle, serial, ISR, and helper functions.
    assert!(
        b.functions.len() >= 14,
        "expected at least 14 function behaviors, got {}",
        b.functions.len()
    );

    // Check specific function roles exist.
    assert!(b
        .functions
        .iter()
        .any(|f| f.role == FunctionRole::LifecycleInit));
    assert!(b
        .functions
        .iter()
        .any(|f| f.role == FunctionRole::LifecycleDeinit));
    assert!(b
        .functions
        .iter()
        .any(|f| f.role == FunctionRole::SerialTxByte));
    assert!(b
        .functions
        .iter()
        .any(|f| f.role == FunctionRole::SerialRxChar));
    assert!(b
        .functions
        .iter()
        .any(|f| f.role == FunctionRole::SerialTxReady));
    assert!(b
        .functions
        .iter()
        .any(|f| f.role == FunctionRole::SerialTxEmpty));
    assert!(b
        .functions
        .iter()
        .any(|f| f.role == FunctionRole::SerialFlushRxFifo));
    assert!(b
        .functions
        .iter()
        .any(|f| f.role == FunctionRole::SerialFlushTxFifo));
    assert!(b
        .functions
        .iter()
        .any(|f| f.role == FunctionRole::SerialSendBreak));
    assert!(b.functions.iter().any(|f| f.role == FunctionRole::Isr));
}

#[test]
fn behavior_functions_are_named() {
    let plan = make_portable_plan();
    let b = &plan.behavior;

    let init = b.functions.iter().find(|f| f.name == "init").expect("init");
    assert_eq!(init.role, FunctionRole::LifecycleInit);

    let tx = b
        .functions
        .iter()
        .find(|f| f.name == "tx_byte")
        .expect("tx_byte");
    assert_eq!(tx.role, FunctionRole::SerialTxByte);

    let isr = b
        .functions
        .iter()
        .find(|f| f.name == "handle_irq")
        .expect("handle_irq");
    assert_eq!(isr.role, FunctionRole::Isr);
}

// ---- Test 12: behavior preserves ordering and control flow ----

#[test]
fn init_behavior_has_ordered_resource_then_programming() {
    use regseq_backends::lowering::behavior::BehaviorNode;

    let plan = make_portable_plan();
    let init = plan
        .behavior
        .functions
        .iter()
        .find(|f| f.name == "init")
        .unwrap();

    // init body: resource ops then CallHelper("program_polling_port") then Return.
    assert!(
        init.body.len() >= 3,
        "init should have resource ops + helper call + return"
    );

    // Last action should be a Return(Ok).
    assert!(matches!(
        init.body.last().unwrap(),
        BehaviorNode::Return {
            semantic: ReturnSemantic::Ok
        }
    ));

    // Should call program_polling_port helper.
    assert!(init.body.iter().any(|n| matches!(
        n,
        BehaviorNode::CallHelper { callee } if callee == "program_polling_port"
    )));
}

#[test]
fn program_polling_port_has_ordered_sequence() {
    use regseq_backends::lowering::behavior::BehaviorNode;

    let plan = make_portable_plan();
    let helper = plan
        .behavior
        .functions
        .iter()
        .find(|f| f.name == "program_polling_port")
        .unwrap();

    // Should contain an OrderedSequence.
    let has_ordered = helper
        .body
        .iter()
        .any(|n| matches!(n, BehaviorNode::OrderedSequence { .. }));
    assert!(
        has_ordered,
        "program_polling_port must have an OrderedSequence block"
    );

    // The ordered sequence should contain baud rate and line control steps.
    if let Some(BehaviorNode::OrderedSequence { steps }) = helper
        .body
        .iter()
        .find(|n| matches!(n, BehaviorNode::OrderedSequence { .. }))
    {
        assert!(
            steps.len() >= 5,
            "ordered sequence should have at least 5 steps, got {}",
            steps.len()
        );
    }
}

#[test]
fn tx_byte_has_conditional_fast_path() {
    use regseq_backends::lowering::behavior::BehaviorNode;

    let plan = make_portable_plan();
    let tx = plan
        .behavior
        .functions
        .iter()
        .find(|f| f.name == "tx_byte")
        .unwrap();

    // tx_byte body should contain a Branch.
    assert!(tx
        .body
        .iter()
        .any(|n| matches!(n, BehaviorNode::Branch { .. })));
}

#[test]
fn rx_char_has_empty_check_and_error_accumulation() {
    use regseq_backends::lowering::behavior::BehaviorNode;

    let plan = make_portable_plan();
    let rx = plan
        .behavior
        .functions
        .iter()
        .find(|f| f.name == "rx_char")
        .unwrap();

    // rx_char should have a Branch (RXFE check) and RxErrorCheck nodes.
    assert!(rx
        .body
        .iter()
        .any(|n| matches!(n, BehaviorNode::Branch { .. })));
    let error_checks: Vec<_> = rx
        .body
        .iter()
        .filter(|n| matches!(n, BehaviorNode::RxErrorCheck { .. }))
        .collect();
    assert_eq!(
        error_checks.len(),
        4,
        "should have 4 RX error checks (OE, BE, PE, FE)"
    );
}

#[test]
fn flush_rx_fifo_has_while_loop() {
    use regseq_backends::lowering::behavior::BehaviorNode;

    let plan = make_portable_plan();
    let flush = plan
        .behavior
        .functions
        .iter()
        .find(|f| f.name == "flush_rx_fifo")
        .unwrap();

    assert!(flush
        .body
        .iter()
        .any(|n| matches!(n, BehaviorNode::Loop { .. })));
}

#[test]
fn send_break_has_conditional_on_bool_param() {
    use regseq_backends::lowering::behavior::{BehaviorNode, BranchCondition};

    let plan = make_portable_plan();
    let brk = plan
        .behavior
        .functions
        .iter()
        .find(|f| f.name == "send_break")
        .unwrap();

    if let Some(BehaviorNode::Branch { condition, .. }) = brk
        .body
        .iter()
        .find(|n| matches!(n, BehaviorNode::Branch { .. }))
    {
        assert!(
            matches!(condition, BranchCondition::BoolParam { name } if name == "on"),
            "send_break should branch on a bool param"
        );
    } else {
        panic!("send_break must have a Branch node");
    }
}

// ---- Test 13: ISR behavior uses interrupt classes, not hardcoded masks ----

#[test]
fn isr_behavior_uses_interrupt_class_discrimination() {
    use regseq_backends::lowering::behavior::{BehaviorNode, BranchCondition};

    let plan = make_portable_plan();
    let isr = plan
        .behavior
        .functions
        .iter()
        .find(|f| f.name == "handle_irq")
        .unwrap();

    // ISR should read raw MIS register.
    assert!(isr.body.iter().any(|n| matches!(
        n,
        BehaviorNode::RegReadRaw { register } if register == "mis"
    )));

    // ISR should branch on interrupt classes, not hardcoded masks.
    let class_tests: Vec<_> = isr
        .body
        .iter()
        .filter(|n| {
            matches!(
                n,
                BehaviorNode::Branch {
                    condition: BranchCondition::InterruptClassActive { .. },
                    ..
                }
            )
        })
        .collect();
    assert!(
        class_tests.len() >= 2,
        "ISR should have at least RX and TX class branches"
    );

    // ISR should clear error interrupts using interrupt class.
    assert!(isr.body.iter().any(|n| matches!(
        n,
        BehaviorNode::InterruptClassClear { class_name, .. } if class_name == "error"
    )));
}

#[test]
fn interrupt_classes_carry_authored_mask_values() {
    let plan = make_portable_plan();
    let classes = &plan.behavior.interrupt_classes;

    assert!(
        classes.len() >= 3,
        "should have rx, tx, error classes at minimum"
    );

    let rx = classes.iter().find(|c| c.name == "rx").expect("rx class");
    assert_eq!(rx.mask_value, (1 << 4) | (1 << 6));

    let tx = classes.iter().find(|c| c.name == "tx").expect("tx class");
    assert_eq!(tx.mask_value, 1 << 5);

    let err = classes
        .iter()
        .find(|c| c.name == "error")
        .expect("error class");
    assert_eq!(err.mask_value, (1 << 7) | (1 << 8) | (1 << 9) | (1 << 10));
}

// ---- Test 14: Linux plan consumes behavior without inventing PL011 facts ----

#[test]
fn linux_behavior_mapping_covers_all_portable_functions() {
    use regseq_backends::linux::plan::LinuxCallbackGroup;

    let linux = make_linux_plan();
    let mapping = &linux.behavior_mapping;

    assert_eq!(
        mapping.portable_function_count,
        mapping.callback_mappings.len(),
        "every portable function should have a Linux mapping"
    );

    // Check specific mappings.
    let init_map = mapping
        .callback_mappings
        .iter()
        .find(|m| m.portable_function == "init")
        .expect("init mapping");
    assert_eq!(init_map.group, LinuxCallbackGroup::ProbeRemove);

    let tx_map = mapping
        .callback_mappings
        .iter()
        .find(|m| m.portable_function == "tx_byte")
        .expect("tx_byte mapping");
    assert_eq!(tx_map.group, LinuxCallbackGroup::UartOps);

    let isr_map = mapping
        .callback_mappings
        .iter()
        .find(|m| m.portable_function == "handle_irq")
        .expect("handle_irq mapping");
    assert_eq!(isr_map.group, LinuxCallbackGroup::IsrHandler);
}

#[test]
fn linux_behavior_mapping_carries_interrupt_class_count() {
    let linux = make_linux_plan();
    assert!(
        linux.behavior_mapping.interrupt_class_count >= 3,
        "Linux plan should know about interrupt classes from the portable plan"
    );
}

// ---- Test 15: behavior plan determinism ----

#[test]
fn behavior_plan_is_deterministic() {
    let plan1 = make_portable_plan();
    let plan2 = make_portable_plan();

    assert_eq!(
        plan1.behavior.functions.len(),
        plan2.behavior.functions.len()
    );
    for (f1, f2) in plan1
        .behavior
        .functions
        .iter()
        .zip(plan2.behavior.functions.iter())
    {
        assert_eq!(f1.name, f2.name);
        assert_eq!(f1.role, f2.role);
        assert_eq!(f1.body.len(), f2.body.len());
    }
    assert_eq!(
        plan1.behavior.interrupt_classes.len(),
        plan2.behavior.interrupt_classes.len()
    );
}
