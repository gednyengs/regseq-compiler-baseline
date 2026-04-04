//! Lowering boundary tests.
//!
//! Verifies that the lowering module scaffold establishes a real typed
//! boundary between the portable `DriverLoweringPlan`, the Linux-specific
//! `LinuxBackendPlan`, and the `AnalysisInput` entry point.

use regseq_analysis::AnalysisInput;
use regseq_backends::linux::plan::{LinuxBackendPlan, LinuxFileRole};
use regseq_backends::linux::planner::build_linux_backend_plan;
use regseq_backends::lowering::plan::{BusType, DriverLoweringPlan, ResourceKind, ResourcePolicy};
use regseq_backends::lowering::planner::{build_driver_lowering_plan, LoweringPlanError};
use regseq_model::binding::{
    BindingHeader, BindingModel, BindingProvenance, ClockBinding, InterruptBinding, JoinKey,
    MmioBinding, PinCtrlBinding, PropertyBinding, ResetBinding,
};
use regseq_model::declaration::callbacks::{
    CallbackContext, CallbackDeclSet, CallbackFamily, CallbackImplDecl,
};
use regseq_model::declaration::operations::{
    DelayIntrinsic, DelayPathKind, DurationCtor, DurationCtorSiteDecl, LogCallSiteDecl, LogLevel,
    LoggingDeclSet, PmGateDecl, PmGateDeclSet, PmGateProperties, TimedDelayCallSiteDecl,
    TimedDelayDeclSet,
};
use regseq_model::declaration::properties::{PropertyDeclSet, PropertyFieldDecl, PropertyLintDecl};
use regseq_model::declaration::provenance::Provenance;
use regseq_model::declaration::registers::{
    PollCallSiteDecl, RegisterBlockDecl, RegisterDeclSet, RegisterDescriptorDecl,
    RegisterDescriptorFieldDecl, RegisterFieldDecl, WrapperCallSiteDecl, WrapperMethod,
};
use regseq_model::declaration::resources::{
    ClockCallSiteDecl, ClockDeclSet, ClockFieldDecl, ClockMethod, InterruptBindingHints,
    InterruptDeclSet, InterruptFieldShape, InterruptResourceDecl, MmioFieldDecl, PinCtrlDeclSet,
    PinCtrlFieldDecl, ResetCallSiteDecl, ResetDeclSet, ResetFieldDecl, ResetMethod,
    ResourceDeclSet,
};
use regseq_model::declaration::synchronization::{
    CompletionMethod, CompletionOperationDecl, GuardScopeDecl, LockOperationDecl, SyncFieldDecl,
    SyncFieldKind, SynchronizationDeclSet,
};
use regseq_model::declaration::{
    BusType as ModelBusType, DeclarationBuildContext, DeclarationModel, DeviceClass, DriverDecl,
};
use regseq_model::joined::perform_join;
use regseq_model::platform::PlatformValue;

// ---- Fixture builders ----

fn pl011_decl() -> DeclarationModel {
    DeclarationModel {
        driver: DriverDecl {
            driver_name: "Pl011Driver".into(),
            compatible: vec!["arm,pl011".into()],
            device_class: DeviceClass::SerialUart,
            bus: Some(ModelBusType::Amba),
            provenance: Provenance::new("identity"),
        },
        resources: ResourceDeclSet {
            mmio: vec![MmioFieldDecl {
                field_name: "regs".into(),
                mmio_id: Some("uart_regs".into()),
                register_block_type: Some("Pl011Regs".into()),
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
                isr_bindings: vec![],
            },
            clock: ClockDeclSet {
                fields: vec![ClockFieldDecl {
                    field_name: "clk".into(),
                    clock_id: "uart_clk".into(),
                    required: true,
                    provenance: Provenance::new("clock"),
                }],
                call_sites: vec![ClockCallSiteDecl {
                    method: ClockMethod::Enable,
                    containing_fn: "init".into(),
                    provenance: Provenance::new("clock"),
                }],
            },
            reset: ResetDeclSet {
                fields: vec![ResetFieldDecl {
                    field_name: "uart_reset".into(),
                    required: false,
                    provenance: Provenance::new("reset"),
                }],
                call_sites: vec![ResetCallSiteDecl {
                    method: ResetMethod::Deassert,
                    containing_fn: "init".into(),
                    provenance: Provenance::new("reset"),
                }],
            },
            pinctrl: PinCtrlDeclSet {
                fields: vec![PinCtrlFieldDecl {
                    field_name: "pins".into(),
                    required: false,
                    pinctrl_id: Some("uart_pins".into()),
                    provenance: Provenance::new("pinctrl"),
                }],
                call_sites: vec![],
            },
        },
        synchronization: SynchronizationDeclSet {
            sync_fields: vec![SyncFieldDecl {
                field_name: "tx_lock".into(),
                kind: SyncFieldKind::SpinLock,
                payload_type: None,
                provenance: Provenance::new("synchronization"),
            }],
            lock_operations: vec![LockOperationDecl {
                lock_kind: SyncFieldKind::SpinLock,
                containing_fn: "write_char".into(),
                provenance: Provenance::new("synchronization"),
            }],
            completion_operations: vec![CompletionOperationDecl {
                method: CompletionMethod::Wait,
                containing_fn: "write_char".into(),
                provenance: Provenance::new("synchronization"),
            }],
            guard_scopes: vec![GuardScopeDecl {
                lock_kind: SyncFieldKind::SpinLock,
                containing_fn: "write_char".into(),
                provenance: Provenance::new("synchronization"),
            }],
        },
        callbacks: CallbackDeclSet {
            lifecycle_impls: vec![CallbackImplDecl {
                driver_type: "Pl011Driver".into(),
                context: CallbackContext::Framework,
                family: CallbackFamily::Lifecycle,
                provenance: Provenance::new("callback"),
            }],
            serial_impls: vec![CallbackImplDecl {
                driver_type: "Pl011Driver".into(),
                context: CallbackContext::SubsystemSerialized,
                family: CallbackFamily::Serial,
                provenance: Provenance::new("callback"),
            }],
            serial_console_impls: vec![],
            power_management_impls: vec![],
        },
        registers: RegisterDeclSet {
            blocks: vec![RegisterBlockDecl {
                name: "Pl011Regs".into(),
                fields: vec![RegisterFieldDecl {
                    name: "DR".into(),
                    access_width: Some(32),
                    endian: None,
                    side_effects: None,
                    hw_updates: None,
                }],
                provenance: Provenance::new("register_access"),
            }],
            descriptors: vec![RegisterDescriptorDecl {
                register_module: "DR".into(),
                fields: vec![RegisterDescriptorFieldDecl {
                    field_name: "DATA".into(),
                    offset: 0,
                    width: 8,
                    named_values: vec![],
                }],
                provenance: Provenance::new("register_access"),
            }],
            wrapper_call_sites: vec![WrapperCallSiteDecl {
                method: WrapperMethod::Read,
                containing_fn: "read_char".into(),
                provenance: Provenance::new("register_access"),
            }],
            poll_call_sites: vec![PollCallSiteDecl {
                containing_fn: "wait_rx_ready".into(),
                provenance: Provenance::new("register_access"),
            }],
        },
        properties: PropertyDeclSet {
            fields: vec![
                PropertyFieldDecl {
                    driver_type: "Pl011Driver".into(),
                    field_name: "baud_rate".into(),
                    field_type: "u32".into(),
                    property_id: "baud_rate".into(),
                    default_literal: Some("115200".into()),
                    vendor_prefix: None,
                    required: true,
                    provenance: Provenance::new("firmware_properties"),
                },
                PropertyFieldDecl {
                    driver_type: "Pl011Driver".into(),
                    field_name: "hw_flow_control".into(),
                    field_type: "bool".into(),
                    property_id: "hw_flow_control".into(),
                    default_literal: None,
                    vendor_prefix: None,
                    required: false,
                    provenance: Provenance::new("firmware_properties"),
                },
            ],
            lints: vec![PropertyLintDecl::RequiredWithDefault {
                driver_type: "Pl011Driver".into(),
                field_name: "baud_rate".into(),
                property_id: "baud_rate".into(),
                provenance: Provenance::new("firmware_properties"),
            }],
        },
        timed_delay: TimedDelayDeclSet {
            call_sites: vec![TimedDelayCallSiteDecl {
                intrinsic: DelayIntrinsic::BusyWait,
                path_kind: DelayPathKind::Canonical,
                containing_fn: "init".into(),
                provenance: Provenance::new("timed_delay"),
            }],
            duration_ctors: vec![DurationCtorSiteDecl {
                ctor: DurationCtor::FromUs,
                containing_fn: "init".into(),
                provenance: Provenance::new("timed_delay"),
            }],
        },
        logging: LoggingDeclSet {
            call_sites: vec![LogCallSiteDecl {
                level: LogLevel::Info,
                containing_fn: "init".into(),
                canonical: true,
                template: "pl011 init done".into(),
                arg_count: 0,
                arg_kinds: vec![],
                provenance: Provenance::new("logging"),
            }],
        },
        pm_gate: PmGateDeclSet {
            gates: vec![PmGateDecl {
                driver_type: "Pl011Driver".into(),
                properties: PmGateProperties {
                    idle: true,
                    wakeup_capable: false,
                },
                has_pm_impl: false,
                provenance: Provenance::new("pm_gate"),
            }],
        },
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
            provenance: BindingProvenance::new("platform").with_source_file("pl011.platform.toml"),
        },
        properties: vec![PropertyBinding {
            key: "baud_rate".into(),
            value: PlatformValue::Integer(115200),
            provenance: BindingProvenance::new("properties").with_section_key("baud_rate"),
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
            join_key: JoinKey::FieldName("uart_reset".into()),
            reset_line_id: Some("rst0".into()),
            reset_shared: Some(false),
            provenance: BindingProvenance::new("reset").with_section_key("uart_reset"),
        }],
        pinctrl: vec![PinCtrlBinding {
            join_key: JoinKey::ExplicitId("uart_pins".into()),
            pin_states: Some(vec!["default".into(), "sleep".into()]),
            provenance: BindingProvenance::new("pinctrl").with_section_key("uart_pins"),
        }],
        bus: None,
    }
}

fn make_analysis_input() -> AnalysisInput {
    let decl = pl011_decl();
    let binding = pl011_binding();
    let bus_binding = binding.bus.clone();
    let joined = perform_join(&decl, &binding).expect("join should succeed");
    AnalysisInput::new(decl, joined, bus_binding)
}

fn make_portable_plan() -> DriverLoweringPlan {
    let input = make_analysis_input();
    build_driver_lowering_plan(&input).expect("portable plan should succeed")
}

fn make_linux_plan() -> LinuxBackendPlan {
    let plan = make_portable_plan();
    build_linux_backend_plan(&plan).expect("linux plan should succeed")
}

// ---- Test 1: module compiles and is importable ----

#[test]
fn lowering_module_compiles_and_is_importable() {
    // Verify types from both modules are accessible.
    let _: fn() -> Option<DriverLoweringPlan> = || None;
    let _: fn() -> Option<LinuxBackendPlan> = || None;
}

// ---- Test 2: AnalysisInput reaches a typed portable lowering-plan entry point ----

#[test]
fn analysis_input_reaches_portable_plan() {
    let input = make_analysis_input();
    let plan = build_driver_lowering_plan(&input);
    assert!(plan.is_ok());

    let plan = plan.unwrap();
    assert_eq!(plan.identity.driver_name, "Pl011Driver");
    assert_eq!(plan.identity.compatible, vec!["arm,pl011"]);
}

#[test]
fn analysis_input_without_explicit_bus_is_rejected() {
    let mut decl = pl011_decl();
    decl.driver.bus = None;

    let binding = pl011_binding();
    let bus_binding = binding.bus.clone();
    let joined = perform_join(&decl, &binding).expect("join should succeed");
    let input = AnalysisInput::new(decl, joined, bus_binding);

    let err = build_driver_lowering_plan(&input).expect_err("missing bus should fail");
    assert!(matches!(
        err,
        LoweringPlanError::MissingBusIdentity { driver_name } if driver_name == "Pl011Driver"
    ));
}

// ---- Test 3: LinuxBackendPlan is typed separately from DriverLoweringPlan ----

#[test]
fn linux_plan_is_typed_separately() {
    let portable = make_portable_plan();
    let linux = make_linux_plan();

    // The types are distinct: portable plan has resource obligations,
    // Linux plan has probe/remove orchestration.
    assert!(!portable.resources.mmio.is_empty());
    assert!(!linux.probe.steps.is_empty());
    assert!(!linux.remove.steps.is_empty());

    // Linux plan has registration shape not present in portable plan.
    assert!(!linux.registration.driver_struct_name.is_empty());
    assert!(!linux.registration.match_table_name.is_empty());
}

// ---- Test 4: plan structures preserve provenance and ordering slots ----

#[test]
fn portable_plan_preserves_provenance() {
    let plan = make_portable_plan();

    // Driver provenance is preserved.
    assert_eq!(plan.identity.provenance.raw_family, "identity");

    // MMIO obligation preserves provenance.
    assert_eq!(
        plan.resources.mmio[0].provenance.raw_family,
        "register_access"
    );

    // Interrupt obligation preserves provenance.
    assert_eq!(
        plan.resources.interrupts[0].provenance.raw_family,
        "interrupt"
    );

    // Register block obligation preserves provenance.
    assert_eq!(
        plan.register_obligations.blocks[0].provenance.raw_family,
        "register_access"
    );

    // Synchronization field preserves provenance.
    assert_eq!(
        plan.synchronization.fields[0].provenance.raw_family,
        "synchronization"
    );
}

#[test]
fn portable_plan_preserves_acquisition_ordering() {
    let plan = make_portable_plan();

    // Acquisition order: clocks -> resets -> pinctrl -> mmio -> properties -> interrupts.
    assert!(!plan.acquisition_order.is_empty());
    assert!(!plan.teardown_order.is_empty());

    let kinds: Vec<&ResourceKind> = plan
        .acquisition_order
        .iter()
        .map(|s| &s.resource_kind)
        .collect();

    // First should be clock.
    assert_eq!(kinds[0], &ResourceKind::Clock);

    // Last should be interrupt.
    assert_eq!(kinds[kinds.len() - 1], &ResourceKind::Interrupt);

    // Teardown should be reverse of acquisition.
    assert_eq!(plan.teardown_order.len(), plan.acquisition_order.len());
    assert_eq!(
        plan.teardown_order[0].resource_kind,
        ResourceKind::Interrupt
    );
}

#[test]
fn portable_plan_preserves_optional_resource_policy() {
    let plan = make_portable_plan();

    // Clock is required.
    assert_eq!(plan.resources.clocks[0].policy, ResourcePolicy::Required);

    // Reset is optional-skip (not required, no fallback).
    assert_eq!(
        plan.resources.resets[0].policy,
        ResourcePolicy::OptionalSkip
    );

    // baud_rate is required-with-default -> OptionalWithFallback.
    let baud = plan
        .resources
        .properties
        .iter()
        .find(|p| p.property_id == "baud_rate")
        .unwrap();
    assert_eq!(baud.policy, ResourcePolicy::OptionalWithFallback);

    // hw_flow_control is not required -> OptionalSkip.
    let hwfc = plan
        .resources
        .properties
        .iter()
        .find(|p| p.property_id == "hw_flow_control")
        .unwrap();
    assert_eq!(hwfc.policy, ResourcePolicy::OptionalSkip);
}

// ---- Test 5: no Linux-specific realization types leak into the portable plan API ----

#[test]
fn portable_plan_contains_no_linux_types() {
    let plan = make_portable_plan();

    // The portable plan uses BusType, ResourceKind, ResourcePolicy --
    // none of which are Linux-specific.
    assert_eq!(plan.resources.bus_identity.bus_type, BusType::Amba);

    // Device class roles use DeviceClass from the model, not a Linux type.
    assert_eq!(
        plan.device_class_roles.device_class,
        DeviceClass::SerialUart
    );

    // Callback actions use CallbackFamily, not Linux-specific types.
    assert!(!plan.callback_actions.is_empty());
    assert!(plan
        .callback_actions
        .iter()
        .any(|a| a.family == CallbackFamily::Lifecycle));
}

// ---- Additional boundary tests ----

#[test]
fn portable_plan_callback_inventory() {
    let plan = make_portable_plan();

    assert_eq!(plan.callbacks.lifecycle.len(), 1);
    assert_eq!(plan.callbacks.serial.len(), 1);
    assert!(plan.callbacks.serial_console.is_empty());
    assert!(plan.callbacks.power_management.is_empty());

    assert_eq!(
        plan.callbacks.lifecycle[0].family,
        CallbackFamily::Lifecycle
    );
    assert_eq!(plan.callbacks.serial[0].family, CallbackFamily::Serial);
}

#[test]
fn portable_plan_register_obligations() {
    let plan = make_portable_plan();

    assert_eq!(plan.register_obligations.blocks.len(), 1);
    assert_eq!(plan.register_obligations.blocks[0].name, "Pl011Regs");
    assert_eq!(plan.register_obligations.blocks[0].field_count, 1);

    assert_eq!(plan.register_obligations.access_sites.len(), 1);
    assert_eq!(
        plan.register_obligations.access_sites[0].method,
        WrapperMethod::Read
    );

    assert_eq!(plan.register_obligations.poll_sites.len(), 1);
    assert_eq!(
        plan.register_obligations.poll_sites[0].containing_fn,
        "wait_rx_ready"
    );
}

#[test]
fn portable_plan_synchronization_obligations() {
    let plan = make_portable_plan();

    assert_eq!(plan.synchronization.fields.len(), 1);
    assert_eq!(plan.synchronization.fields[0].field_name, "tx_lock");
    assert_eq!(plan.synchronization.fields[0].kind, SyncFieldKind::SpinLock);

    assert_eq!(plan.synchronization.lock_scopes.len(), 1);
    assert_eq!(plan.synchronization.completion_uses.len(), 1);
}

#[test]
fn portable_plan_callback_body_actions() {
    let plan = make_portable_plan();

    let lifecycle = plan
        .callback_actions
        .iter()
        .find(|a| a.family == CallbackFamily::Lifecycle)
        .expect("should have lifecycle callback actions");

    // Lifecycle should have property reads, delays, logs, and interrupt registration.
    assert!(!lifecycle.actions.is_empty());

    let serial = plan
        .callback_actions
        .iter()
        .find(|a| a.family == CallbackFamily::Serial)
        .expect("should have serial callback actions");

    // Serial should have register accesses and lock scopes.
    assert!(!serial.actions.is_empty());
}

#[test]
fn portable_plan_device_class_roles() {
    let plan = make_portable_plan();

    assert_eq!(
        plan.device_class_roles.device_class,
        DeviceClass::SerialUart
    );
    assert!(plan.device_class_roles.has_lifecycle);
    assert!(!plan.device_class_roles.has_power_management);
}

#[test]
fn linux_plan_registration_shape() {
    let linux = make_linux_plan();

    assert!(linux.registration.driver_struct_name.contains("pl011"));
    assert!(linux.registration.match_table_name.contains("pl011"));
    assert_eq!(linux.registration.compatible_entries, vec!["arm,pl011"]);
    assert_eq!(linux.registration.module_license, "GPL");
}

#[test]
fn linux_plan_serial_core_realization() {
    let linux = make_linux_plan();

    let serial = linux.serial_core.as_ref().expect("should have serial core");
    assert!(serial.uart_driver_name.contains("pl011"));
    assert!(serial.uart_ops_name.contains("pl011"));
}

#[test]
fn linux_plan_probe_remove_orchestration() {
    let linux = make_linux_plan();

    // Probe should have steps for all resources plus RegisterUartPort.
    assert!(linux.probe.steps.len() >= 2);

    // Remove should start with UnregisterUartPort for a serial device.
    assert!(matches!(
        linux.remove.steps[0],
        regseq_backends::linux::plan::LinuxRemoveStep::UnregisterUartPort
    ));
}

#[test]
fn linux_plan_artifact_layout() {
    let linux = make_linux_plan();

    assert!(!linux.artifact_layout.emitted_files.is_empty());

    let roles: Vec<&LinuxFileRole> = linux
        .artifact_layout
        .emitted_files
        .iter()
        .map(|f| &f.role)
        .collect();

    assert!(roles.contains(&&LinuxFileRole::DriverMain));
    assert!(roles.contains(&&LinuxFileRole::Makefile));
    assert!(roles.contains(&&LinuxFileRole::RegisterHelpers));
}

#[test]
fn linux_plan_resource_sequencing() {
    let linux = make_linux_plan();

    assert!(!linux.resource_acquisition.acquisition_order.is_empty());
    assert!(!linux.resource_acquisition.release_order.is_empty());

    // Verify API families are Linux-specific strings.
    let apis: Vec<&str> = linux
        .resource_acquisition
        .acquisition_order
        .iter()
        .map(|e| e.api_family.as_str())
        .collect();

    assert!(apis.contains(&"devm_clk_get"));
    assert!(apis.contains(&"devm_ioremap_resource"));
    assert!(apis.contains(&"devm_request_irq"));
}
