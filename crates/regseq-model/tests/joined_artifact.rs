//! Join-focused tests for the declaration + binding joined artifact.
//!
//! Verifies that the join works for all accepted families, preserves
//! provenance, handles unmatched entries, and produces deterministic output.

use regseq_model::binding::{
    BindingHeader, BindingModel, BindingProvenance, ClockBinding, InterruptBinding, JoinKey,
    MmioBinding, PinCtrlBinding, PropertyBinding, ResetBinding,
};
use regseq_model::declaration::mir_body::{MirBodyDeclSet, MirQueryFlavor};
use regseq_model::declaration::provenance::Provenance;
use regseq_model::declaration::resources::{
    ClockDeclSet, ClockFieldDecl, InterruptBindingHints, InterruptDeclSet, InterruptFieldShape,
    InterruptResourceDecl, MmioFieldDecl, PinCtrlDeclSet, PinCtrlFieldDecl, ResetDeclSet,
    ResetFieldDecl, ResourceDeclSet,
};
use regseq_model::declaration::{
    BodyDeclSet, CallbackDeclSet, DeclarationBuildContext, DeclarationModel, DriverDecl,
};
use regseq_model::declaration::{
    BusType, DeviceClass, LoggingDeclSet, PmGateDeclSet, PropertyDeclSet, PropertyFieldDecl,
    RegisterDeclSet, SynchronizationDeclSet, TimedDelayDeclSet,
};
use regseq_model::joined::{perform_join, JoinOutcome, JoinedArtifact};
use regseq_model::platform::PlatformValue;

/// Build a PL011-shaped declaration model for testing.
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
            lifecycle_impls: vec![],
            serial_impls: vec![],
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
                    required: false,
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
            lints: vec![],
        },
        timed_delay: TimedDelayDeclSet {
            call_sites: vec![],
            duration_ctors: vec![],
        },
        logging: LoggingDeclSet { call_sites: vec![] },
        pm_gate: PmGateDeclSet { gates: vec![] },
        bodies: BodyDeclSet { functions: vec![] },
        mir_bodies: MirBodyDeclSet {
            functions: vec![],
            query_flavor: MirQueryFlavor::DropsElaborated,
        },
        build_context: DeclarationBuildContext {
            target_os: "linux".into(),
        },
    }
}

/// Build a PL011-shaped binding model for testing.
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

// ---- Test 1: representative join succeeds ----

#[test]
fn pl011_join_succeeds() {
    let decl = pl011_decl();
    let binding = pl011_binding();
    let artifact = perform_join(&decl, &binding).expect("join should succeed");

    assert_eq!(artifact.header.os, "linux");
    assert_eq!(artifact.header.board, "raspberry-pi-4");
    assert!(!artifact.properties_joined.is_empty());
    assert!(!artifact.mmio_joined.is_empty());
    assert!(!artifact.interrupts_joined.is_empty());
    assert!(!artifact.clocks_joined.is_empty());
    assert!(!artifact.resets_joined.is_empty());
    assert!(!artifact.pinctrl_joined.is_empty());
}

// ---- Test 2: explicit-id joins work ----

#[test]
fn explicit_id_join_for_mmio() {
    let decl = pl011_decl();
    let binding = pl011_binding();
    let artifact = perform_join(&decl, &binding).unwrap();

    assert_eq!(artifact.mmio_joined.len(), 1);
    let entry = &artifact.mmio_joined[0];
    assert_eq!(entry.mmio_id, "uart_regs");
    assert_eq!(entry.field_name, "regs");
    assert_eq!(entry.base, Some(0x0900_0000));
    assert_eq!(entry.size, Some(0x1000));
    assert_eq!(entry.outcome, JoinOutcome::Matched);
}

#[test]
fn explicit_id_join_for_interrupts() {
    let decl = pl011_decl();
    let binding = pl011_binding();
    let artifact = perform_join(&decl, &binding).unwrap();

    assert_eq!(artifact.interrupts_joined.len(), 1);
    let entry = &artifact.interrupts_joined[0];
    assert_eq!(entry.interrupt_id, "uart_irq");
    assert_eq!(entry.trigger_type.as_deref(), Some("level"));
    assert_eq!(entry.trigger_polarity.as_deref(), Some("high"));
    assert_eq!(entry.shared, Some(false));
}

#[test]
fn explicit_id_join_for_clocks() {
    let decl = pl011_decl();
    let binding = pl011_binding();
    let artifact = perform_join(&decl, &binding).unwrap();

    assert_eq!(artifact.clocks_joined.len(), 1);
    let entry = &artifact.clocks_joined[0];
    assert_eq!(entry.clock_id, "uart_clk");
    assert_eq!(entry.default_frequency, Some(48_000_000));
    assert_eq!(entry.outcome, JoinOutcome::Matched);
}

#[test]
fn explicit_id_join_for_pinctrl() {
    let decl = pl011_decl();
    let binding = pl011_binding();
    let artifact = perform_join(&decl, &binding).unwrap();

    assert_eq!(artifact.pinctrl_joined.len(), 1);
    let entry = &artifact.pinctrl_joined[0];
    assert_eq!(entry.field_name, "pins");
    assert_eq!(entry.join_key, JoinKey::ExplicitId("uart_pins".into()));
    let states = entry.pin_states.as_ref().unwrap();
    assert_eq!(states, &["default", "sleep"]);
}

// ---- Test 3: field-name join works ----

#[test]
fn field_name_join_for_resets() {
    let decl = pl011_decl();
    let binding = pl011_binding();
    let artifact = perform_join(&decl, &binding).unwrap();

    assert_eq!(artifact.resets_joined.len(), 1);
    let entry = &artifact.resets_joined[0];
    assert_eq!(entry.field_name, "uart_reset");
    assert_eq!(entry.reset_line_id.as_deref(), Some("rst0"));
    assert_eq!(entry.reset_shared, Some(false));
    assert_eq!(entry.outcome, JoinOutcome::Matched);
}

#[test]
fn pinctrl_falls_back_to_field_name_join() {
    let mut decl = pl011_decl();
    // Remove explicit pinctrl_id to force field-name join.
    decl.resources.pinctrl.fields[0].pinctrl_id = None;
    decl.resources.pinctrl.fields[0].field_name = "uart_pins".into();

    let binding = pl011_binding();
    let artifact = perform_join(&decl, &binding).unwrap();

    assert_eq!(artifact.pinctrl_joined.len(), 1);
    assert_eq!(artifact.pinctrl_joined[0].field_name, "uart_pins");
}

// ---- Test 4: unmatched entries remain reviewable ----

#[test]
fn unmatched_declaration_only_properties() {
    let decl = pl011_decl();
    let binding = pl011_binding();
    let artifact = perform_join(&decl, &binding).unwrap();

    // "baud_rate" matches; "hw_flow_control" has no binding entry.
    assert_eq!(artifact.properties_joined.len(), 1);
    assert_eq!(artifact.properties_joined[0].property_id, "baud_rate");
    assert_eq!(artifact.properties_decl_only.len(), 1);
    assert_eq!(
        artifact.properties_decl_only[0].property_id,
        "hw_flow_control"
    );
}

#[test]
fn unmatched_binding_only_properties() {
    let decl = pl011_decl();
    let binding = pl011_binding();
    let artifact = perform_join(&decl, &binding).unwrap();

    // "fifo_depth" is in binding but not in declarations.
    assert_eq!(artifact.properties_binding_only.len(), 1);
    assert_eq!(artifact.properties_binding_only[0].key, "fifo_depth");
}

#[test]
fn unmatched_binding_only_mmio() {
    let mut decl = pl011_decl();
    // Remove mmio declaration to create an unmatched binding entry.
    decl.resources.mmio.clear();

    let binding = pl011_binding();
    let artifact = perform_join(&decl, &binding).unwrap();

    assert!(artifact.mmio_joined.is_empty());
    assert_eq!(artifact.mmio_binding_only.len(), 1);
    assert_eq!(artifact.mmio_binding_only[0].join_key.value(), "uart_regs");
}

#[test]
fn unmatched_declaration_only_interrupts() {
    let decl = pl011_decl();
    let mut binding = pl011_binding();
    binding.interrupts.clear();

    let artifact = perform_join(&decl, &binding).unwrap();

    assert!(artifact.interrupts_joined.is_empty());
    assert_eq!(artifact.interrupts_decl_only.len(), 1);
    assert_eq!(artifact.interrupts_decl_only[0].interrupt_id, "uart_irq");
}

// ---- Test 5: deterministic and snapshot-friendly ----

#[test]
fn joined_output_is_deterministic() {
    let decl = pl011_decl();
    let binding = pl011_binding();

    let a1 = perform_join(&decl, &binding).unwrap();
    let a2 = perform_join(&decl, &binding).unwrap();

    assert_eq!(a1, a2);

    let json1 = serde_json::to_string_pretty(&a1).unwrap();
    let json2 = serde_json::to_string_pretty(&a2).unwrap();
    assert_eq!(json1, json2);
}

#[test]
fn joined_artifact_round_trips_through_json() {
    let decl = pl011_decl();
    let binding = pl011_binding();
    let artifact = perform_join(&decl, &binding).unwrap();

    let json = serde_json::to_string_pretty(&artifact).unwrap();
    let roundtrip: JoinedArtifact = serde_json::from_str(&json).unwrap();
    assert_eq!(artifact, roundtrip);
}

// ---- Test 6: provenance points back to both sides ----

#[test]
fn joined_provenance_from_both_sides() {
    let decl = pl011_decl();
    let binding = pl011_binding();
    let artifact = perform_join(&decl, &binding).unwrap();

    // Property join: both provenances present.
    let prop = &artifact.properties_joined[0];
    assert_eq!(prop.decl_provenance.raw_family, "firmware_properties");
    let bp = prop.binding_provenance.as_ref().unwrap();
    assert_eq!(bp.section_kind, "properties");

    // MMIO join: both provenances present.
    let mmio = &artifact.mmio_joined[0];
    assert_eq!(mmio.decl_provenance.raw_family, "register_access");
    assert_eq!(mmio.binding_provenance.section_kind, "mmio");

    // Interrupt join: both provenances present.
    let intr = &artifact.interrupts_joined[0];
    assert_eq!(intr.decl_provenance.raw_family, "interrupt");
    assert_eq!(intr.binding_provenance.section_kind, "interrupt");

    // Clock join.
    let clk = &artifact.clocks_joined[0];
    assert_eq!(clk.decl_provenance.raw_family, "clock");
    assert_eq!(clk.binding_provenance.section_kind, "clock");

    // Reset join.
    let rst = &artifact.resets_joined[0];
    assert_eq!(rst.decl_provenance.raw_family, "reset");
    assert_eq!(rst.binding_provenance.section_kind, "reset");

    // PinCtrl join.
    let pin = &artifact.pinctrl_joined[0];
    assert_eq!(pin.decl_provenance.raw_family, "pinctrl");
    assert_eq!(pin.binding_provenance.section_kind, "pinctrl");
}

// ---- Test 7: matched-but-incomplete bindings ----

#[test]
fn matched_incomplete_mmio_binding() {
    let decl = pl011_decl();
    let mut binding = pl011_binding();
    // Remove size to create an incomplete binding.
    binding.mmio[0].size = None;

    let artifact = perform_join(&decl, &binding).unwrap();

    assert_eq!(artifact.mmio_joined.len(), 1);
    let entry = &artifact.mmio_joined[0];
    assert_eq!(entry.outcome, JoinOutcome::MatchedIncomplete);
    assert_eq!(entry.base, Some(0x0900_0000));
    assert!(entry.size.is_none());
}

#[test]
fn matched_incomplete_interrupt_binding() {
    let decl = pl011_decl();
    let mut binding = pl011_binding();
    // Remove trigger_polarity and shared to make it incomplete.
    binding.interrupts[0].trigger_polarity = None;
    binding.interrupts[0].shared = None;

    let artifact = perform_join(&decl, &binding).unwrap();

    assert_eq!(artifact.interrupts_joined.len(), 1);
    let entry = &artifact.interrupts_joined[0];
    assert_eq!(entry.outcome, JoinOutcome::MatchedIncomplete);
    assert_eq!(entry.trigger_type.as_deref(), Some("level"));
    assert!(entry.trigger_polarity.is_none());
    assert!(entry.shared.is_none());
}

#[test]
fn matched_incomplete_clock_binding() {
    let decl = pl011_decl();
    let mut binding = pl011_binding();
    binding.clocks[0].default_frequency = None;

    let artifact = perform_join(&decl, &binding).unwrap();

    assert_eq!(artifact.clocks_joined.len(), 1);
    assert_eq!(
        artifact.clocks_joined[0].outcome,
        JoinOutcome::MatchedIncomplete
    );
}

#[test]
fn matched_incomplete_reset_binding() {
    let decl = pl011_decl();
    let mut binding = pl011_binding();
    binding.resets[0].reset_shared = None;

    let artifact = perform_join(&decl, &binding).unwrap();

    assert_eq!(artifact.resets_joined.len(), 1);
    assert_eq!(
        artifact.resets_joined[0].outcome,
        JoinOutcome::MatchedIncomplete
    );
    assert_eq!(
        artifact.resets_joined[0].reset_line_id.as_deref(),
        Some("rst0")
    );
    assert!(artifact.resets_joined[0].reset_shared.is_none());
}

#[test]
fn matched_incomplete_pinctrl_binding() {
    let decl = pl011_decl();
    let mut binding = pl011_binding();
    binding.pinctrl[0].pin_states = None;

    let artifact = perform_join(&decl, &binding).unwrap();

    assert_eq!(artifact.pinctrl_joined.len(), 1);
    assert_eq!(
        artifact.pinctrl_joined[0].outcome,
        JoinOutcome::MatchedIncomplete
    );
}

// ---- Test 8: interrupt sidecar hints do not override binding values ----

#[test]
fn interrupt_binding_hints_do_not_override_authoritative_values() {
    let mut decl = pl011_decl();
    // Set declaration hints that conflict with binding values.
    decl.resources.interrupts.resources[0].binding_hints = InterruptBindingHints {
        trigger_type: Some("edge".into()),
        trigger_polarity: Some("rising".into()),
        priority: Some("255".into()),
        shared: Some(true),
    };

    let binding = pl011_binding();
    let artifact = perform_join(&decl, &binding).unwrap();

    let entry = &artifact.interrupts_joined[0];

    // Authoritative values come from the binding model.
    assert_eq!(entry.trigger_type.as_deref(), Some("level"));
    assert_eq!(entry.trigger_polarity.as_deref(), Some("high"));
    assert_eq!(entry.priority, None); // binding has None
    assert_eq!(entry.shared, Some(false));

    // Declaration hints are preserved as diagnostic annotations.
    assert_eq!(
        entry.decl_binding_hints.trigger_type.as_deref(),
        Some("edge")
    );
    assert_eq!(
        entry.decl_binding_hints.trigger_polarity.as_deref(),
        Some("rising")
    );
    assert_eq!(entry.decl_binding_hints.priority.as_deref(), Some("255"));
    assert_eq!(entry.decl_binding_hints.shared, Some(true));
}

// ---- Empty inputs ----

#[test]
fn empty_binding_produces_all_decl_only() {
    let decl = pl011_decl();
    let binding = BindingModel {
        header: BindingHeader {
            version: "1".into(),
            os: "linux".into(),
            arch: "aarch64".into(),
            board: "raspberry-pi-4".into(),
            provenance: BindingProvenance::new("platform"),
        },
        properties: vec![],
        mmio: vec![],
        interrupts: vec![],
        clocks: vec![],
        resets: vec![],
        pinctrl: vec![],
        bus: None,
    };

    let artifact = perform_join(&decl, &binding).unwrap();

    assert!(artifact.properties_joined.is_empty());
    assert_eq!(artifact.properties_decl_only.len(), 2);
    assert!(artifact.mmio_joined.is_empty());
    assert_eq!(artifact.mmio_decl_only.len(), 1);
    assert!(artifact.interrupts_joined.is_empty());
    assert_eq!(artifact.interrupts_decl_only.len(), 1);
    assert!(artifact.clocks_joined.is_empty());
    assert_eq!(artifact.clocks_decl_only.len(), 1);
    assert!(artifact.resets_joined.is_empty());
    assert_eq!(artifact.resets_decl_only.len(), 1);
    assert!(artifact.pinctrl_joined.is_empty());
    assert_eq!(artifact.pinctrl_decl_only.len(), 1);
}
