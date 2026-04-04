//! Tests for the analysis-facing bootstrap input layer.
//!
//! Verifies that the AnalysisInput contract wraps the declaration model
//! and joined artifact correctly, preserves family ownership, and exposes
//! all required query surfaces without introducing backend policy.

use regseq_analysis::{AnalysisInput, ResourceJoinSummary};
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
    BodyDeclSet, BusType, DeclarationBuildContext, DeclarationModel, DeviceClass, DriverDecl,
};
use regseq_model::joined::perform_join;
use regseq_model::platform::PlatformValue;

// ---- Fixture builders ----

/// Build a PL011-shaped declaration model with richer operation evidence
/// to exercise all declaration-only families that the bootstrap must expose.
fn rich_pl011_decl() -> DeclarationModel {
    DeclarationModel {
        driver: DriverDecl {
            driver_name: "Pl011Driver".into(),
            compatible: vec!["arm,pl011".into()],
            device_class: DeviceClass::SerialUart,
            bus: Some(BusType::Amba),
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
        bodies: BodyDeclSet { functions: vec![] },
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
    let decl = rich_pl011_decl();
    let binding = pl011_binding();
    let joined = perform_join(&decl, &binding).expect("join should succeed");
    AnalysisInput::new(decl, joined, None)
}

// ---- Test 1: bootstrap wrapping ----

#[test]
fn bootstrap_wraps_declaration_and_joined() {
    let input = make_analysis_input();

    assert_eq!(input.declaration().driver.driver_name, "Pl011Driver");
    assert_eq!(input.joined().header.os, "linux");
    assert_eq!(input.joined().header.board, "raspberry-pi-4");
}

// ---- Test 2: family ownership ----

#[test]
fn family_ownership_explicit_in_join_summaries() {
    let input = make_analysis_input();
    let summaries = input.all_join_summaries();

    assert_eq!(summaries.len(), 6);
    let families: Vec<&str> = summaries.iter().map(|s| s.family).collect();
    assert_eq!(
        families,
        &[
            "properties",
            "mmio",
            "interrupts",
            "clocks",
            "resets",
            "pinctrl"
        ]
    );

    // Each summary preserves its own family counts.
    let prop = &summaries[0];
    assert_eq!(prop.family, "properties");
    assert_eq!(prop.joined_complete, 1); // baud_rate matched
    assert_eq!(prop.decl_only, 1); // hw_flow_control unmatched

    let mmio = &summaries[1];
    assert_eq!(mmio.family, "mmio");
    assert_eq!(mmio.joined_complete, 1);
    assert_eq!(mmio.decl_only, 0);
}

#[test]
fn resource_family_accessors_preserve_ownership() {
    let input = make_analysis_input();

    // Resources accessible through family-specific accessors.
    assert_eq!(input.declaration().mmio_fields().len(), 1);
    assert_eq!(input.declaration().interrupt_resources().len(), 1);
    assert_eq!(input.declaration().clock_fields().len(), 1);
    assert_eq!(input.declaration().reset_fields().len(), 1);
    assert_eq!(input.declaration().pinctrl_fields().len(), 1);
}

// ---- Test 3: callback-family presence ----

#[test]
fn callback_families_present_queryable() {
    let input = make_analysis_input();
    let families = input.callback_families_present();

    assert!(families.contains(&CallbackFamily::Lifecycle));
    assert!(families.contains(&CallbackFamily::Serial));
    assert!(!families.contains(&CallbackFamily::SerialConsole));
    assert!(!families.contains(&CallbackFamily::PowerManagement));
}

#[test]
fn callback_families_no_backend_assumptions() {
    let input = make_analysis_input();
    // The presence set is purely declaration-driven, no backend check.
    let families = input.callback_families_present();
    assert_eq!(families.len(), 2);
}

#[test]
fn all_callback_impls_accessible() {
    let input = make_analysis_input();
    let impls = input.all_callback_impls();
    assert_eq!(impls.len(), 2);
    assert_eq!(impls[0].family, CallbackFamily::Lifecycle);
    assert_eq!(impls[1].family, CallbackFamily::Serial);
}

// ---- Test 4: partial join outcomes and unmatched facts ----

#[test]
fn partial_join_outcomes_queryable() {
    let decl = rich_pl011_decl();
    // hw_flow_control has no binding: decl-only
    // baud_rate has a binding: joined
    let binding = pl011_binding();
    let joined = perform_join(&decl, &binding).unwrap();
    let input = AnalysisInput::new(decl, joined, None);

    let summary = input.property_join_summary();
    assert_eq!(summary.joined_complete, 1);
    assert_eq!(summary.decl_only, 1);
    assert!(summary.has_unmatched());
    assert!(!summary.is_fully_joined());
}

#[test]
fn incomplete_binding_outcomes_queryable() {
    let decl = rich_pl011_decl();
    let mut binding = pl011_binding();
    // Make interrupt binding incomplete.
    binding.interrupts[0].shared = None;

    let joined = perform_join(&decl, &binding).unwrap();
    let input = AnalysisInput::new(decl, joined, None);

    let summary = input.interrupt_join_summary();
    assert!(summary.has_incomplete());
    assert_eq!(summary.joined_incomplete, 1);
    assert_eq!(summary.joined_complete, 0);
}

#[test]
fn unmatched_facts_remain_queryable() {
    let decl = rich_pl011_decl();
    let binding = pl011_binding();
    let joined = perform_join(&decl, &binding).unwrap();
    let input = AnalysisInput::new(decl, joined, None);

    // Binding-only: no "fifo_depth" in this fixture, but hw_flow_control is decl-only.
    assert_eq!(input.joined().properties_decl_only.len(), 1);
    assert_eq!(
        input.joined().properties_decl_only[0].property_id,
        "hw_flow_control"
    );
}

// ---- Test 5: provenance visible through bootstrap ----

#[test]
fn provenance_visible_through_bootstrap() {
    let input = make_analysis_input();

    // Declaration-side provenance via the bootstrap layer.
    let mmio_entries = input.mmio_joined();
    assert_eq!(mmio_entries.len(), 1);
    assert_eq!(
        mmio_entries[0].decl_provenance.raw_family,
        "register_access"
    );
    assert_eq!(mmio_entries[0].binding_provenance.section_kind, "mmio");

    // Interrupt provenance: both-side accessible.
    let int_entries = input.interrupts_joined();
    assert_eq!(int_entries[0].decl_provenance.raw_family, "interrupt");
    assert_eq!(int_entries[0].binding_provenance.section_kind, "interrupt");

    // Declaration-side binding hints are preserved.
    assert_eq!(
        int_entries[0].decl_binding_hints.trigger_type.as_deref(),
        Some("level")
    );
}

// ---- Test 6: declaration-only operation-evidence families ----

#[test]
fn register_blocks_accessible() {
    let input = make_analysis_input();
    assert_eq!(input.register_blocks().len(), 1);
    assert_eq!(input.register_blocks()[0].name, "Pl011Regs");
}

#[test]
fn register_descriptors_accessible() {
    let input = make_analysis_input();
    assert_eq!(input.register_descriptors().len(), 1);
    assert_eq!(input.register_descriptors()[0].register_module, "DR");
}

#[test]
fn wrapper_call_sites_accessible() {
    let input = make_analysis_input();
    assert_eq!(input.wrapper_call_sites().len(), 1);
    assert_eq!(input.wrapper_call_sites()[0].method, WrapperMethod::Read);
    assert_eq!(input.wrapper_call_sites()[0].containing_fn, "read_char");
}

#[test]
fn poll_call_sites_accessible() {
    let input = make_analysis_input();
    assert_eq!(input.poll_call_sites().len(), 1);
    assert_eq!(input.poll_call_sites()[0].containing_fn, "wait_rx_ready");
}

#[test]
fn sync_fields_accessible() {
    let input = make_analysis_input();
    assert_eq!(input.sync_fields().len(), 1);
    assert_eq!(input.sync_fields()[0].field_name, "tx_lock");
    assert_eq!(input.sync_fields()[0].kind, SyncFieldKind::SpinLock);
}

#[test]
fn lock_operations_accessible() {
    let input = make_analysis_input();
    assert_eq!(input.lock_operations().len(), 1);
    assert_eq!(input.lock_operations()[0].containing_fn, "write_char");
}

#[test]
fn completion_operations_accessible() {
    let input = make_analysis_input();
    assert_eq!(input.completion_operations().len(), 1);
    assert_eq!(
        input.completion_operations()[0].method,
        CompletionMethod::Wait
    );
}

#[test]
fn guard_scopes_accessible() {
    let input = make_analysis_input();
    assert_eq!(input.guard_scopes().len(), 1);
    assert_eq!(input.guard_scopes()[0].containing_fn, "write_char");
}

#[test]
fn timed_delay_call_sites_accessible() {
    let input = make_analysis_input();
    assert_eq!(input.timed_delay_call_sites().len(), 1);
    assert_eq!(
        input.timed_delay_call_sites()[0].intrinsic,
        DelayIntrinsic::BusyWait
    );
}

#[test]
fn duration_ctor_sites_accessible() {
    let input = make_analysis_input();
    assert_eq!(input.duration_ctor_sites().len(), 1);
    assert_eq!(input.duration_ctor_sites()[0].ctor, DurationCtor::FromUs);
}

#[test]
fn log_call_sites_accessible() {
    let input = make_analysis_input();
    assert_eq!(input.log_call_sites().len(), 1);
    assert_eq!(input.log_call_sites()[0].level, LogLevel::Info);
    assert_eq!(input.log_call_sites()[0].template, "pl011 init done");
}

#[test]
fn pm_gates_accessible() {
    let input = make_analysis_input();
    assert_eq!(input.pm_gates().len(), 1);
    assert!(input.pm_gates()[0].properties.idle);
    assert!(!input.pm_gates()[0].properties.wakeup_capable);
}

// ---- Test 7: determinism ----

#[test]
fn bootstrap_output_is_deterministic() {
    let input1 = make_analysis_input();
    let input2 = make_analysis_input();

    // Join summaries are identical.
    assert_eq!(input1.all_join_summaries(), input2.all_join_summaries());

    // Callback families are identical.
    assert_eq!(
        input1.callback_families_present(),
        input2.callback_families_present()
    );
}

// ---- ResourceJoinSummary helpers ----

#[test]
fn join_summary_total_counts() {
    let summary = ResourceJoinSummary {
        family: "test",
        joined_complete: 3,
        joined_incomplete: 1,
        decl_only: 2,
        binding_only: 1,
    };
    assert_eq!(summary.total(), 7);
    assert!(!summary.is_fully_joined());
    assert!(summary.has_incomplete());
    assert!(summary.has_unmatched());
}

#[test]
fn fully_joined_summary() {
    let summary = ResourceJoinSummary {
        family: "test",
        joined_complete: 5,
        joined_incomplete: 0,
        decl_only: 0,
        binding_only: 0,
    };
    assert!(summary.is_fully_joined());
    assert!(!summary.has_incomplete());
    assert!(!summary.has_unmatched());
}

// ---- Property lint visibility ----

#[test]
fn property_lints_visible() {
    let input = make_analysis_input();
    let lints = input.property_lints();
    assert_eq!(lints.len(), 1);
    match &lints[0] {
        PropertyLintDecl::RequiredWithDefault {
            property_id,
            field_name,
            ..
        } => {
            assert_eq!(property_id, "baud_rate");
            assert_eq!(field_name, "baud_rate");
        }
    }
}

// ---- Full declaration set accessors ----

#[test]
fn full_set_accessors_delegate_correctly() {
    let input = make_analysis_input();

    // These should return the same data as the underlying model.
    assert_eq!(input.registers().blocks.len(), 1);
    assert_eq!(input.synchronization().sync_fields.len(), 1);
    assert_eq!(input.timed_delay().call_sites.len(), 1);
    assert_eq!(input.logging().call_sites.len(), 1);
    assert_eq!(input.pm_gate().gates.len(), 1);
    assert_eq!(input.properties().fields.len(), 2);
    assert_eq!(input.callbacks().lifecycle_impls.len(), 1);
    assert_eq!(input.resources().mmio.len(), 1);
}

// ---- Bus identity and metadata exposure ----

#[test]
fn bus_identity_exposed_from_declaration() {
    let input = make_analysis_input();
    // The rich PL011 decl has bus = Some(BusType::Amba).
    let bus = input
        .bus_identity()
        .expect("bus identity should be present");
    assert_eq!(*bus, regseq_model::declaration::driver::BusType::Amba);
}

#[test]
fn bus_binding_exposed_when_provided() {
    use regseq_model::binding::BindingProvenance;
    use regseq_model::binding::BusBinding;
    use regseq_model::declaration::driver::BusType;

    let decl = rich_pl011_decl();
    let binding = pl011_binding();
    let bus = BusBinding {
        bus_type: BusType::Amba,
        amba_id: Some(0x00041011),
        revision: Some(3),
        provenance: BindingProvenance::new("bus"),
    };
    let joined = perform_join(&decl, &binding).unwrap();
    let input = AnalysisInput::new(decl, joined, Some(bus));

    let bb = input.bus_binding().expect("bus binding should be present");
    assert_eq!(bb.bus_type, BusType::Amba);
    assert_eq!(bb.amba_id, Some(0x00041011));
    assert_eq!(bb.revision, Some(3));
}

#[test]
fn bus_binding_is_none_when_not_provided() {
    let input = make_analysis_input();
    assert!(input.bus_binding().is_none());
}
