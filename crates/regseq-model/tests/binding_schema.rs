//! Schema-focused tests for the binding-model type family.
//!
//! Verifies that binding-model types construct and import cleanly,
//! section-owned sub-families remain structurally distinct, provenance
//! is present per entity, and join keys have an explicit home.

use regseq_model::binding::{
    BindingHeader, BindingModel, BindingProvenance, ClockBinding, InterruptBinding, JoinKey,
    MmioBinding, PinCtrlBinding, PropertyBinding, ResetBinding,
};
use regseq_model::platform::PlatformValue;

/// Helper: build a minimal binding header for tests.
fn test_header() -> BindingHeader {
    BindingHeader {
        version: "1".into(),
        os: "linux".into(),
        arch: "aarch64".into(),
        board: "raspberry-pi-4".into(),
        provenance: BindingProvenance::new("platform"),
    }
}

// ---- Test 1: types construct and import cleanly ----

#[test]
fn binding_model_constructs_with_all_families() {
    let model = BindingModel {
        header: test_header(),
        properties: vec![],
        mmio: vec![],
        interrupts: vec![],
        clocks: vec![],
        resets: vec![],
        pinctrl: vec![],
        bus: None,
    };

    assert_eq!(model.header.version, "1");
    assert!(model.properties.is_empty());
    assert!(model.mmio.is_empty());
    assert!(model.interrupts.is_empty());
    assert!(model.clocks.is_empty());
    assert!(model.resets.is_empty());
    assert!(model.pinctrl.is_empty());
}

// ---- Test 2: section-owned sub-families remain structurally distinct ----

#[test]
fn section_families_are_structurally_distinct() {
    let model = BindingModel {
        header: test_header(),
        properties: vec![PropertyBinding {
            key: "baud_rate".into(),
            value: PlatformValue::Integer(115200),
            provenance: BindingProvenance::new("properties"),
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
    };

    // Each family is independently accessible and non-empty.
    assert_eq!(model.properties.len(), 1);
    assert_eq!(model.mmio.len(), 1);
    assert_eq!(model.interrupts.len(), 1);
    assert_eq!(model.clocks.len(), 1);
    assert_eq!(model.resets.len(), 1);
    assert_eq!(model.pinctrl.len(), 1);
}

// ---- Test 3: per-entity provenance fields are present ----

#[test]
fn per_entity_provenance_is_present() {
    let mmio = MmioBinding {
        join_key: JoinKey::ExplicitId("uart_regs".into()),
        base: Some(0x0900_0000),
        size: Some(0x1000),
        provenance: BindingProvenance::new("mmio")
            .with_section_key("uart_regs")
            .with_source_file("pl011.platform.toml"),
    };

    assert_eq!(mmio.provenance.section_kind, "mmio");
    assert_eq!(mmio.provenance.section_key.as_deref(), Some("uart_regs"));
    assert_eq!(
        mmio.provenance.source_file.as_deref(),
        Some("pl011.platform.toml")
    );

    let intr = InterruptBinding {
        join_key: JoinKey::ExplicitId("uart_irq".into()),
        trigger_type: None,
        trigger_polarity: None,
        priority: None,
        shared: None,
        provenance: BindingProvenance::new("interrupt").with_section_key("uart_irq"),
    };

    assert_eq!(intr.provenance.section_kind, "interrupt");
    assert_eq!(intr.provenance.section_key.as_deref(), Some("uart_irq"));
}

// ---- Test 4: header/build metadata is separate from resource/property entries ----

#[test]
fn header_is_separate_from_resource_entries() {
    let model = BindingModel {
        header: BindingHeader {
            version: "1".into(),
            os: "zephyr".into(),
            arch: "x86_64".into(),
            board: "qemu-x86".into(),
            provenance: BindingProvenance::new("platform"),
        },
        properties: vec![PropertyBinding {
            key: "fifo_depth".into(),
            value: PlatformValue::Integer(16),
            provenance: BindingProvenance::new("properties"),
        }],
        mmio: vec![],
        interrupts: vec![],
        clocks: vec![],
        resets: vec![],
        pinctrl: vec![],
        bus: None,
    };

    // Header metadata lives on its own struct, not mixed into properties.
    assert_eq!(model.header.os, "zephyr");
    assert_eq!(model.header.board, "qemu-x86");
    // Properties are separate.
    assert_eq!(model.properties[0].key, "fifo_depth");
}

// ---- Test 5: explicit-id and field-name join identifiers have an explicit home ----

#[test]
fn join_key_explicit_id_and_field_name_are_distinct() {
    let explicit = JoinKey::ExplicitId("uart_clk".into());
    let field = JoinKey::FieldName("uart_reset".into());

    // Variants are distinct.
    assert_ne!(explicit, field);

    // Values are accessible.
    assert_eq!(explicit.value(), "uart_clk");
    assert_eq!(field.value(), "uart_reset");

    // Pattern matching works.
    assert!(matches!(explicit, JoinKey::ExplicitId(_)));
    assert!(matches!(field, JoinKey::FieldName(_)));
}

#[test]
fn resource_bindings_carry_appropriate_join_key_variants() {
    // MMIO uses explicit-id join.
    let mmio = MmioBinding {
        join_key: JoinKey::ExplicitId("uart_regs".into()),
        base: None,
        size: None,
        provenance: BindingProvenance::new("mmio"),
    };
    assert!(matches!(mmio.join_key, JoinKey::ExplicitId(_)));

    // Reset uses field-name join.
    let reset = ResetBinding {
        join_key: JoinKey::FieldName("uart_reset".into()),
        reset_line_id: None,
        reset_shared: None,
        provenance: BindingProvenance::new("reset"),
    };
    assert!(matches!(reset.join_key, JoinKey::FieldName(_)));
}

// ---- Test 6: accepted construct-section fields have typed homes ----

#[test]
fn mmio_fields_are_typed() {
    let mmio = MmioBinding {
        join_key: JoinKey::ExplicitId("uart_regs".into()),
        base: Some(0x0900_0000),
        size: Some(0x1000),
        provenance: BindingProvenance::new("mmio"),
    };

    assert_eq!(mmio.base, Some(0x0900_0000));
    assert_eq!(mmio.size, Some(0x1000));
}

#[test]
fn interrupt_fields_are_typed() {
    let intr = InterruptBinding {
        join_key: JoinKey::ExplicitId("uart_irq".into()),
        trigger_type: Some("edge".into()),
        trigger_polarity: Some("rising".into()),
        priority: Some("128".into()),
        shared: Some(true),
        provenance: BindingProvenance::new("interrupt"),
    };

    assert_eq!(intr.trigger_type.as_deref(), Some("edge"));
    assert_eq!(intr.trigger_polarity.as_deref(), Some("rising"));
    assert_eq!(intr.priority.as_deref(), Some("128"));
    assert_eq!(intr.shared, Some(true));
}

#[test]
fn clock_fields_are_typed() {
    let clock = ClockBinding {
        join_key: JoinKey::ExplicitId("uart_clk".into()),
        default_frequency: Some(48_000_000),
        provenance: BindingProvenance::new("clock"),
    };

    assert_eq!(clock.default_frequency, Some(48_000_000));
}

#[test]
fn reset_fields_are_typed() {
    let reset = ResetBinding {
        join_key: JoinKey::FieldName("uart_reset".into()),
        reset_line_id: Some("rst0".into()),
        reset_shared: Some(false),
        provenance: BindingProvenance::new("reset"),
    };

    assert_eq!(reset.reset_line_id.as_deref(), Some("rst0"));
    assert_eq!(reset.reset_shared, Some(false));
}

#[test]
fn pinctrl_fields_are_typed() {
    let pinctrl = PinCtrlBinding {
        join_key: JoinKey::ExplicitId("uart_pins".into()),
        pin_states: Some(vec!["default".into(), "sleep".into()]),
        provenance: BindingProvenance::new("pinctrl"),
    };

    let states = pinctrl.pin_states.as_ref().unwrap();
    assert_eq!(states.len(), 2);
    assert_eq!(states[0], "default");
    assert_eq!(states[1], "sleep");
}

#[test]
fn binding_entry_fields_are_individually_optional() {
    // All construct-section fields can be None for incomplete bindings.
    let mmio = MmioBinding {
        join_key: JoinKey::ExplicitId("uart_regs".into()),
        base: None,
        size: None,
        provenance: BindingProvenance::new("mmio"),
    };
    assert!(mmio.base.is_none());
    assert!(mmio.size.is_none());

    let intr = InterruptBinding {
        join_key: JoinKey::ExplicitId("uart_irq".into()),
        trigger_type: None,
        trigger_polarity: None,
        priority: None,
        shared: None,
        provenance: BindingProvenance::new("interrupt"),
    };
    assert!(intr.trigger_type.is_none());
    assert!(intr.shared.is_none());

    let clock = ClockBinding {
        join_key: JoinKey::ExplicitId("uart_clk".into()),
        default_frequency: None,
        provenance: BindingProvenance::new("clock"),
    };
    assert!(clock.default_frequency.is_none());

    let reset = ResetBinding {
        join_key: JoinKey::FieldName("uart_reset".into()),
        reset_line_id: None,
        reset_shared: None,
        provenance: BindingProvenance::new("reset"),
    };
    assert!(reset.reset_line_id.is_none());

    let pinctrl = PinCtrlBinding {
        join_key: JoinKey::ExplicitId("uart_pins".into()),
        pin_states: None,
        provenance: BindingProvenance::new("pinctrl"),
    };
    assert!(pinctrl.pin_states.is_none());
}

// ---- Serialization round-trip ----

#[test]
fn binding_model_serializes_deterministically() {
    let model = BindingModel {
        header: test_header(),
        properties: vec![PropertyBinding {
            key: "baud_rate".into(),
            value: PlatformValue::Integer(115200),
            provenance: BindingProvenance::new("properties"),
        }],
        mmio: vec![MmioBinding {
            join_key: JoinKey::ExplicitId("uart_regs".into()),
            base: Some(0x0900_0000),
            size: Some(0x1000),
            provenance: BindingProvenance::new("mmio").with_section_key("uart_regs"),
        }],
        interrupts: vec![],
        clocks: vec![],
        resets: vec![],
        pinctrl: vec![],
        bus: None,
    };

    let json = serde_json::to_string_pretty(&model).expect("serialize");
    let roundtrip: BindingModel = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(model, roundtrip);
}
