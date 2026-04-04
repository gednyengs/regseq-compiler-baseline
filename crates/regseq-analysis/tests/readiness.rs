//! Tests for lowering-input readiness and consistency checks.
//!
//! Verifies that the readiness report correctly identifies callback-family
//! integrity, property lint carry-through, join completeness, interrupt
//! authority, and resource optionality for PL011-shaped inputs.

use regseq_analysis::{check_readiness, AnalysisInput};
use regseq_model::binding::{
    BindingHeader, BindingModel, BindingProvenance, ClockBinding, InterruptBinding, JoinKey,
    MmioBinding, PinCtrlBinding, PropertyBinding, ResetBinding,
};
use regseq_model::declaration::callbacks::{
    CallbackContext, CallbackDeclSet, CallbackFamily, CallbackImplDecl,
};
use regseq_model::declaration::operations::{LoggingDeclSet, PmGateDeclSet, TimedDelayDeclSet};
use regseq_model::declaration::properties::{PropertyDeclSet, PropertyFieldDecl, PropertyLintDecl};
use regseq_model::declaration::provenance::Provenance;
use regseq_model::declaration::registers::RegisterDeclSet;
use regseq_model::declaration::resources::{
    ClockDeclSet, ClockFieldDecl, InterruptBindingHints, InterruptDeclSet, InterruptFieldShape,
    InterruptResourceDecl, MmioFieldDecl, PinCtrlDeclSet, PinCtrlFieldDecl, ResetDeclSet,
    ResetFieldDecl, ResourceDeclSet,
};
use regseq_model::declaration::synchronization::SynchronizationDeclSet;
use regseq_model::declaration::{
    BusType, DeclarationBuildContext, DeclarationModel, DeviceClass, DriverDecl,
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
                call_sites: vec![],
            },
            reset: ResetDeclSet {
                fields: vec![ResetFieldDecl {
                    field_name: "uart_reset".into(),
                    required: false,
                    provenance: Provenance::new("reset"),
                }],
                call_sites: vec![],
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
            sync_fields: vec![],
            lock_operations: vec![],
            completion_operations: vec![],
            guard_scopes: vec![],
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
            blocks: vec![],
            descriptors: vec![],
            wrapper_call_sites: vec![],
            poll_call_sites: vec![],
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
            call_sites: vec![],
            duration_ctors: vec![],
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
            provenance: BindingProvenance::new("platform").with_source_file("pl011.platform.toml"),
        },
        properties: vec![
            PropertyBinding {
                key: "baud_rate".into(),
                value: PlatformValue::Integer(115200),
                provenance: BindingProvenance::new("properties").with_section_key("baud_rate"),
            },
            PropertyBinding {
                key: "fifo_depth".into(),
                value: PlatformValue::Integer(16),
                provenance: BindingProvenance::new("properties").with_section_key("fifo_depth"),
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

fn make_input(decl: DeclarationModel, binding: BindingModel) -> AnalysisInput {
    let joined = perform_join(&decl, &binding).expect("join should succeed");
    AnalysisInput::new(decl, joined)
}

// ---- Test 1: baseline PL011 readiness succeeds ----

#[test]
fn pl011_baseline_readiness_no_errors() {
    let input = make_input(pl011_decl(), pl011_binding());
    let report = check_readiness(&input);

    assert!(
        !report.has_errors(),
        "PL011 baseline should have no errors, got: {:?}",
        report.errors()
    );
}

// ---- Test 2: callback-family integrity is reported deterministically ----

#[test]
fn callback_family_integrity_deterministic() {
    let input = make_input(pl011_decl(), pl011_binding());

    let report1 = check_readiness(&input);
    let report2 = check_readiness(&input);

    assert_eq!(report1.findings, report2.findings);
}

#[test]
fn missing_lifecycle_callback_reported_as_error() {
    let mut decl = pl011_decl();
    decl.callbacks.lifecycle_impls.clear();

    let input = make_input(decl, pl011_binding());
    let report = check_readiness(&input);

    assert!(report.has_errors());
    let errors = report.errors();
    assert!(errors.iter().any(|f| f.category == "callback_integrity"
        && f.message.contains("lifecycle")
        && f.message.contains("missing")));
}

#[test]
fn missing_serial_callback_for_serial_uart_reported_as_error() {
    let mut decl = pl011_decl();
    decl.callbacks.serial_impls.clear();

    let input = make_input(decl, pl011_binding());
    let report = check_readiness(&input);

    assert!(report.has_errors());
    let errors = report.errors();
    assert!(errors.iter().any(|f| f.category == "callback_integrity"
        && f.message.contains("serial")
        && f.message.contains("missing")));
}

#[test]
fn optional_serial_console_reported_as_info() {
    let mut decl = pl011_decl();
    decl.callbacks.serial_console_impls.push(CallbackImplDecl {
        driver_type: "Pl011Driver".into(),
        context: CallbackContext::Framework,
        family: CallbackFamily::SerialConsole,
        provenance: Provenance::new("callback"),
    });

    let input = make_input(decl, pl011_binding());
    let report = check_readiness(&input);

    let infos = report.infos();
    assert!(infos.iter().any(|f| f.category == "callback_integrity"
        && f.message.contains("serial_console")
        && f.message.contains("optional")));
}

// ---- Test 3: property lint state visible ----

#[test]
fn required_with_default_lint_visible() {
    let input = make_input(pl011_decl(), pl011_binding());
    let report = check_readiness(&input);

    let cautions = report.cautions();
    assert!(cautions
        .iter()
        .any(|f| f.category == "property_lint" && f.message.contains("baud_rate")));
}

#[test]
fn no_property_lint_when_none_present() {
    let mut decl = pl011_decl();
    decl.properties.lints.clear();

    let input = make_input(decl, pl011_binding());
    let report = check_readiness(&input);

    assert!(!report
        .findings
        .iter()
        .any(|f| f.category == "property_lint"));
}

// ---- Test 4: partial and unmatched join outcomes explicit ----

#[test]
fn decl_only_properties_reported_as_caution() {
    let input = make_input(pl011_decl(), pl011_binding());
    let report = check_readiness(&input);

    // hw_flow_control is declaration-only.
    let cautions = report.cautions();
    assert!(cautions.iter().any(|f| f.category == "join_completeness"
        && f.message.contains("properties")
        && f.message.contains("declaration-only")));
}

#[test]
fn binding_only_entries_reported_as_caution() {
    let input = make_input(pl011_decl(), pl011_binding());
    let report = check_readiness(&input);

    // fifo_depth is binding-only.
    let cautions = report.cautions();
    assert!(cautions.iter().any(|f| f.category == "join_completeness"
        && f.message.contains("properties")
        && f.message.contains("binding-only")));
}

#[test]
fn incomplete_join_reported_as_caution() {
    let decl = pl011_decl();
    let mut binding = pl011_binding();
    // Make interrupt binding incomplete.
    binding.interrupts[0].shared = None;

    let input = make_input(decl, binding);
    let report = check_readiness(&input);

    let cautions = report.cautions();
    assert!(cautions.iter().any(|f| f.category == "join_completeness"
        && f.message.contains("interrupts")
        && f.message.contains("incomplete")));
}

#[test]
fn join_summaries_carried_through() {
    let input = make_input(pl011_decl(), pl011_binding());
    let report = check_readiness(&input);

    assert_eq!(report.join_summaries.len(), 6);
    let families: Vec<&str> = report.join_summaries.iter().map(|s| s.family).collect();
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
}

// ---- Test 5: interrupt binding-side authority preserved ----

#[test]
fn interrupt_decl_hints_reported_as_info() {
    let input = make_input(pl011_decl(), pl011_binding());
    let report = check_readiness(&input);

    let infos = report.infos();
    assert!(infos.iter().any(|f| f.category == "interrupt_authority"
        && f.message.contains("uart_irq")
        && f.message.contains("diagnostic-only")));
}

// ---- Test 6: negative cases ----

#[test]
fn required_clock_without_binding_is_error() {
    let decl = pl011_decl();
    let mut binding = pl011_binding();
    binding.clocks.clear();

    let input = make_input(decl, binding);
    let report = check_readiness(&input);

    assert!(report.has_errors());
    let errors = report.errors();
    assert!(errors.iter().any(|f| f.category == "resource_optionality"
        && f.message.contains("uart_clk")
        && f.message.contains("no matching binding")));
}

#[test]
fn required_property_without_binding_is_error() {
    let decl = pl011_decl();
    let mut binding = pl011_binding();
    // Remove baud_rate binding so required property has no match.
    binding.properties.retain(|p| p.key != "baud_rate");

    let input = make_input(decl, binding);
    let report = check_readiness(&input);

    assert!(report.has_errors());
    let errors = report.errors();
    assert!(errors
        .iter()
        .any(|f| f.category == "resource_optionality" && f.message.contains("baud_rate")));
}

#[test]
fn optional_reset_without_binding_is_not_error() {
    let decl = pl011_decl();
    let mut binding = pl011_binding();
    binding.resets.clear();

    let input = make_input(decl, binding);
    let report = check_readiness(&input);

    // Optional reset missing binding is a caution (join_completeness),
    // but not an error (resource_optionality).
    assert!(!report
        .errors()
        .iter()
        .any(|f| f.category == "resource_optionality" && f.message.contains("uart_reset")));
}

#[test]
fn all_findings_are_sorted() {
    let input = make_input(pl011_decl(), pl011_binding());
    let report = check_readiness(&input);

    // Verify findings are sorted.
    let mut sorted = report.findings.clone();
    sorted.sort();
    assert_eq!(report.findings, sorted);
}

#[test]
fn network_device_class_does_not_require_serial() {
    let mut decl = pl011_decl();
    decl.driver.device_class = DeviceClass::Network;
    // Remove serial callbacks to match network device.
    decl.callbacks.serial_impls.clear();

    let input = make_input(decl, pl011_binding());
    let report = check_readiness(&input);

    // Should not report missing serial for network device.
    assert!(!report
        .errors()
        .iter()
        .any(|f| f.category == "callback_integrity" && f.message.contains("serial")));
}
