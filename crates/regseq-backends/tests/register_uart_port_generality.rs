//! Tests for `regseq-linux-backend-08`: RegisterUartPort
//! serial-resource generality.
//!
//! Exercises the typed plan fields that replace the emitter's
//! positional/string-match serial-port wiring:
//! - [`LinuxSerialCoreRealization::primary_mmio_field`] /
//!   `primary_irq_field` / `uartclk_field` / `uartclk_optional`.
//! - [`LinuxPropertyRealization::serial_role`] +
//!   [`SerialPortRole`] (FifoSize / ClockRateDefault).
//! - Structured [`LinuxPlanError`] variants for missing primary
//!   resources on a serial driver.
//!
//! PL011 byte parity for the single-resource serial case is
//! covered by `linux_emission_scaffold.rs` /
//! `pl011_full_spec_lowering.rs`; these tests cover the
//! multi-resource + custom-field-name / custom-property-id paths
//! that PL011 does not exercise.

use std::path::Path;

use regseq_backends::linux::body_index::FunctionBodyIndex;
use regseq_backends::linux::emitter::emit_linux_artifacts;
use regseq_backends::linux::plan::{LinuxBackendPlan, LinuxFileRole, SerialPortRole};
use regseq_backends::linux::planner::{build_linux_backend_plan, LinuxPlanError};
use regseq_backends::lowering::diagnostics::DriverLoweringDiagnostics;
use regseq_backends::lowering::plan::{
    AcquisitionStep, BusIdentity, BusType, CallbackInventory, ClockObligation, DeviceClassRoles,
    DriverIdentity, DriverLoweringPlan, InterruptObligation, MmioObligation, PropertyObligation,
    RegisterObligations, ResourceKind, ResourceObligations, ResourcePolicy,
    SynchronizationObligations, TeardownStep,
};
use regseq_model::declaration::driver::DeviceClass;
use regseq_model::declaration::provenance::Provenance;
use regseq_model::semantic::model::SemanticModel;
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// Plan-construction helpers
// ---------------------------------------------------------------------------

fn empty_semantic_model() -> SemanticModel {
    SemanticModel {
        functions: vec![],
        constants: vec![],
        interrupt_classes: vec![],
    }
}

fn provenance(raw: &str) -> Provenance {
    Provenance::new(raw)
}

/// Build a serial-UART portable plan parameterized by resource sets.
///
/// `clocks` must be in acquisition order; the first entry becomes
/// `uartclk_field`. Same pattern for `mmios` and `irqs` (first entry
/// becomes `primary_mmio_field` / `primary_irq_field`). Any entry in
/// `clocks` with `required = false` becomes an optional clock.
///
/// Property acquisitions follow resource acquisitions and are driven
/// by `properties` order.
fn serial_plan(
    driver_name: &str,
    mmios: Vec<MmioObligation>,
    irqs: Vec<InterruptObligation>,
    clocks: Vec<ClockObligation>,
    properties: Vec<PropertyObligation>,
) -> DriverLoweringPlan {
    let mut acquisition_order = Vec::new();
    let mut order_counter = 0usize;
    for c in &clocks {
        acquisition_order.push(AcquisitionStep {
            order: order_counter,
            resource_kind: ResourceKind::Clock,
            resource_id: c.clock_id.clone(),
            policy: c.policy.clone(),
        });
        order_counter += 1;
    }
    for m in &mmios {
        acquisition_order.push(AcquisitionStep {
            order: order_counter,
            resource_kind: ResourceKind::Mmio,
            resource_id: m.mmio_id.clone(),
            policy: m.policy.clone(),
        });
        order_counter += 1;
    }
    for i in &irqs {
        acquisition_order.push(AcquisitionStep {
            order: order_counter,
            resource_kind: ResourceKind::Interrupt,
            resource_id: i.interrupt_id.clone(),
            policy: i.policy.clone(),
        });
        order_counter += 1;
    }
    for p in &properties {
        acquisition_order.push(AcquisitionStep {
            order: order_counter,
            resource_kind: ResourceKind::Property,
            resource_id: p.property_id.clone(),
            policy: p.policy.clone(),
        });
        order_counter += 1;
    }

    let teardown_order = acquisition_order
        .iter()
        .rev()
        .enumerate()
        .map(|(i, s)| TeardownStep {
            order: i,
            resource_kind: s.resource_kind,
            resource_id: s.resource_id.clone(),
        })
        .collect();

    DriverLoweringPlan {
        identity: DriverIdentity {
            driver_name: driver_name.into(),
            compatible: vec!["vendor,serialspec".into()],
            device_class: DeviceClass::SerialUart,
            source_modules: vec![],
            provenance: provenance("identity"),
        },
        callbacks: CallbackInventory {
            lifecycle: vec![],
            serial: vec![],
            serial_console: vec![],
            power_management: vec![],
        },
        resources: ResourceObligations {
            mmio: mmios,
            interrupts: irqs,
            properties,
            clocks,
            resets: vec![],
            pinctrl: vec![],
            bus_identity: BusIdentity {
                bus_type: BusType::Platform,
                compatible: vec!["vendor,serialspec".into()],
                amba_id: None,
                amba_revision: None,
                amba_mask: None,
            },
        },
        acquisition_order,
        teardown_order,
        device_class_roles: DeviceClassRoles {
            device_class: DeviceClass::SerialUart,
            serial_ops_realized: vec![],
            has_lifecycle: false,
            has_power_management: false,
        },
        register_obligations: RegisterObligations {
            blocks: vec![],
            access_sites: vec![],
            poll_sites: vec![],
            descriptors: vec![],
        },
        synchronization: SynchronizationObligations {
            fields: vec![],
            lock_scopes: vec![],
            completion_uses: vec![],
        },
        diagnostics: DriverLoweringDiagnostics::default(),
    }
}

fn mmio(field: &str, id: &str, block: Option<&str>) -> MmioObligation {
    MmioObligation {
        field_name: field.into(),
        mmio_id: id.into(),
        register_block_type: block.map(str::to_owned),
        policy: ResourcePolicy::Required,
        provenance: provenance("mmio"),
    }
}

fn irq(field: &str, id: &str) -> InterruptObligation {
    InterruptObligation {
        field_name: field.into(),
        interrupt_id: id.into(),
        trigger_type: None,
        shared: None,
        policy: ResourcePolicy::Required,
        provenance: provenance("interrupt"),
    }
}

fn clock(field: &str, id: &str, required: bool) -> ClockObligation {
    ClockObligation {
        field_name: field.into(),
        clock_id: id.into(),
        required,
        default_frequency: None,
        policy: if required {
            ResourcePolicy::Required
        } else {
            ResourcePolicy::OptionalSkip
        },
        provenance: provenance("clock"),
    }
}

fn prop(
    id: &str,
    field: &str,
    ty: &str,
    required: bool,
    default: Option<&str>,
) -> PropertyObligation {
    PropertyObligation {
        property_id: id.into(),
        field_name: field.into(),
        field_type: ty.into(),
        required,
        default_literal: default.map(str::to_owned),
        policy: if required {
            ResourcePolicy::Required
        } else {
            ResourcePolicy::OptionalSkip
        },
        provenance: provenance("property"),
    }
}

fn emit_module_c(linux: &LinuxBackendPlan, dir: &Path) -> String {
    let bodies = FunctionBodyIndex::default();
    let set = emit_linux_artifacts(linux, &bodies, dir).expect("emit");
    let main = set
        .artifacts
        .iter()
        .find(|a| a.role == LinuxFileRole::DriverMain)
        .expect("driver main");
    std::fs::read_to_string(&main.path).expect("read module.c")
}

// ---------------------------------------------------------------------------
// Missing-resource errors for serial drivers
// ---------------------------------------------------------------------------

#[test]
fn missing_mmio_on_serial_driver_raises_structured_error() {
    let portable = serial_plan(
        "NoMmioSpec",
        vec![], // no MMIO
        vec![irq("irq", "primary_irq")],
        vec![clock("clk", "only_clk", true)],
        vec![],
    );
    let err = build_linux_backend_plan(&portable, empty_semantic_model())
        .expect_err("serial driver with no MMIO must fail plan build");
    match err {
        LinuxPlanError::MissingSerialPortPrimaryMmio { driver_name } => {
            assert_eq!(driver_name, "NoMmioSpec");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn missing_irq_on_serial_driver_raises_structured_error() {
    let portable = serial_plan(
        "NoIrqSpec",
        vec![mmio("regs", "primary_regs", None)],
        vec![], // no IRQ
        vec![clock("clk", "only_clk", true)],
        vec![],
    );
    let err = build_linux_backend_plan(&portable, empty_semantic_model())
        .expect_err("serial driver with no IRQ must fail plan build");
    match err {
        LinuxPlanError::MissingSerialPortPrimaryIrq { driver_name } => {
            assert_eq!(driver_name, "NoIrqSpec");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn missing_clock_on_serial_driver_raises_structured_error() {
    let portable = serial_plan(
        "NoClockSpec",
        vec![mmio("regs", "primary_regs", None)],
        vec![irq("irq", "primary_irq")],
        vec![], // no clock
        vec![],
    );
    let err = build_linux_backend_plan(&portable, empty_semantic_model())
        .expect_err("serial driver with no clock must fail plan build");
    match err {
        LinuxPlanError::MissingSerialPortUartClk { driver_name } => {
            assert_eq!(driver_name, "NoClockSpec");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Primary-resource population + uartclk_optional flag
// ---------------------------------------------------------------------------

#[test]
fn planner_populates_primary_fields_from_first_probe_steps() {
    let portable = serial_plan(
        "MultiSpec",
        vec![
            mmio("memory_a", "main_regs", None),
            mmio("memory_b", "aux_regs", None),
        ],
        vec![
            irq("primary_irq_line", "main_irq"),
            irq("secondary_irq_line", "aux_irq"),
        ],
        vec![
            clock("bus_clk", "bus", true),
            clock("ref_clk", "ref", false),
        ],
        vec![],
    );
    let linux = build_linux_backend_plan(&portable, empty_semantic_model())
        .expect("multi-resource serial plan builds");
    let serial = linux
        .serial_core
        .as_ref()
        .expect("serial_core populated for SerialUart driver");
    assert_eq!(serial.primary_mmio_field, "memory_a");
    assert_eq!(serial.primary_irq_field, "primary_irq_line");
    assert_eq!(serial.uartclk_field, "bus_clk");
    assert!(
        !serial.uartclk_optional,
        "first clock is required; uartclk_optional must be false"
    );
}

#[test]
fn planner_tracks_uartclk_optional_when_first_clock_is_optional() {
    let portable = serial_plan(
        "OptClkSpec",
        vec![mmio("regs", "primary_regs", None)],
        vec![irq("irq", "primary_irq")],
        vec![clock("ref_clk", "only_clk", false)], // optional first clock
        vec![],
    );
    let linux = build_linux_backend_plan(&portable, empty_semantic_model())
        .expect("optional-clock serial plan builds");
    let serial = linux.serial_core.as_ref().expect("serial_core populated");
    assert_eq!(serial.uartclk_field, "ref_clk");
    assert!(serial.uartclk_optional);
}

// ---------------------------------------------------------------------------
// serial_role assignment + gating on serial-core driver
// ---------------------------------------------------------------------------

#[test]
fn serial_role_assigned_for_conventional_ids_on_serial_driver() {
    let portable = serial_plan(
        "RoleSpec",
        vec![mmio("regs", "primary_regs", None)],
        vec![irq("irq", "primary_irq")],
        vec![clock("clk", "only_clk", true)],
        vec![
            prop("fifo-size", "fifo_size", "u32", false, None),
            prop(
                "current-speed",
                "current_speed",
                "u32",
                false,
                Some("115200"),
            ),
            prop("other-prop", "other", "u32", false, None),
        ],
    );
    let linux =
        build_linux_backend_plan(&portable, empty_semantic_model()).expect("serial plan builds");

    let by_id: std::collections::HashMap<&str, Option<SerialPortRole>> = linux
        .property_realization
        .iter()
        .map(|r| (r.property_id.as_str(), r.serial_role))
        .collect();
    assert_eq!(by_id["fifo-size"], Some(SerialPortRole::FifoSize));
    assert_eq!(
        by_id["current-speed"],
        Some(SerialPortRole::ClockRateDefault)
    );
    assert_eq!(
        by_id["other-prop"], None,
        "properties outside the id convention must not be tagged"
    );
}

#[test]
fn serial_role_is_none_for_non_serial_driver_even_with_matching_ids() {
    // Use the property_realization_honoring-style plan (non-serial
    // device class) but with property ids that WOULD be tagged on a
    // serial driver. Build an entirely non-serial plan inline so we
    // exercise the gating directly.
    let properties = vec![
        prop("fifo-size", "fifo_size", "u32", false, None),
        prop("current-speed", "current_speed", "u32", false, Some("9600")),
    ];
    let acquisition_order = properties
        .iter()
        .enumerate()
        .map(|(i, p)| AcquisitionStep {
            order: i,
            resource_kind: ResourceKind::Property,
            resource_id: p.property_id.clone(),
            policy: p.policy.clone(),
        })
        .collect();
    let portable = DriverLoweringPlan {
        identity: DriverIdentity {
            driver_name: "NonSerialSpec".into(),
            compatible: vec!["vendor,nonser".into()],
            device_class: DeviceClass::Other(String::new()),
            source_modules: vec![],
            provenance: provenance("identity"),
        },
        callbacks: CallbackInventory {
            lifecycle: vec![],
            serial: vec![],
            serial_console: vec![],
            power_management: vec![],
        },
        resources: ResourceObligations {
            mmio: vec![],
            interrupts: vec![],
            properties,
            clocks: vec![],
            resets: vec![],
            pinctrl: vec![],
            bus_identity: BusIdentity {
                bus_type: BusType::Platform,
                compatible: vec!["vendor,nonser".into()],
                amba_id: None,
                amba_revision: None,
                amba_mask: None,
            },
        },
        acquisition_order,
        teardown_order: vec![],
        device_class_roles: DeviceClassRoles {
            device_class: DeviceClass::Other(String::new()),
            serial_ops_realized: vec![],
            has_lifecycle: false,
            has_power_management: false,
        },
        register_obligations: RegisterObligations {
            blocks: vec![],
            access_sites: vec![],
            poll_sites: vec![],
            descriptors: vec![],
        },
        synchronization: SynchronizationObligations {
            fields: vec![],
            lock_scopes: vec![],
            completion_uses: vec![],
        },
        diagnostics: DriverLoweringDiagnostics::default(),
    };

    let linux = build_linux_backend_plan(&portable, empty_semantic_model())
        .expect("non-serial plan builds");
    assert!(linux.serial_core.is_none(), "non-serial driver");
    for r in &linux.property_realization {
        assert_eq!(
            r.serial_role, None,
            "non-serial driver must not tag serial_role on property '{}'",
            r.property_id
        );
    }
}

// ---------------------------------------------------------------------------
// Multi-resource emission: primary-resource selection, non-role
// properties not wired, exactly-one uartclk line.
// ---------------------------------------------------------------------------

#[test]
fn multi_resource_serial_emission_wires_primaries_and_honors_roles() {
    let portable = serial_plan(
        "MultiSpec",
        vec![
            mmio("memory_a", "main_regs", None),
            mmio("memory_b", "aux_regs", None),
        ],
        vec![
            irq("primary_irq_line", "main_irq"),
            irq("secondary_irq_line", "aux_irq"),
        ],
        vec![
            clock("bus_clk", "bus", true),  // required first clock
            clock("ref_clk", "ref", false), // optional second clock
        ],
        vec![
            // Custom FIFO field name; MUST wire `d->port.fifosize =
            // d->fifo_depth;` (not `d->fifo_size`).
            prop("fifo-size", "fifo_depth", "u32", false, None),
            // Non-role property; MUST NOT be wired onto d->port.*.
            prop("baud-default", "baud_rate", "u32", false, None),
        ],
    );
    let linux = build_linux_backend_plan(&portable, empty_semantic_model())
        .expect("multi-resource serial plan builds");

    let dir = TempDir::new().expect("tempdir");
    let content = emit_module_c(&linux, dir.path());

    // Primary-resource wiring: first MMIO / first IRQ selected.
    let membase_count = content.matches("d->port.membase = ").count();
    assert_eq!(
        membase_count, 1,
        "exactly one d->port.membase assignment; got:\n{content}"
    );
    assert!(
        content.contains("d->port.membase = d->memory_a;"),
        "membase must wire the first MMIO field 'memory_a'; got:\n{content}"
    );
    assert!(
        !content.contains("d->port.membase = d->memory_b;"),
        "membase must not wire the second MMIO"
    );

    let irq_count = content.matches("d->port.irq = ").count();
    assert_eq!(
        irq_count, 1,
        "exactly one d->port.irq assignment; got:\n{content}"
    );
    assert!(
        content.contains("d->port.irq = d->primary_irq_line;"),
        "irq must wire the first IRQ field 'primary_irq_line'; got:\n{content}"
    );
    assert!(
        !content.contains("d->port.irq = d->secondary_irq_line;"),
        "irq must not wire the second IRQ"
    );

    // uartclk: exactly ONE line, using the first (required) clock.
    let uartclk_count = content.matches("d->port.uartclk = ").count();
    assert_eq!(
        uartclk_count, 1,
        "exactly one d->port.uartclk assignment; got:\n{content}"
    );
    assert!(
        content.contains("d->port.uartclk = clk_get_rate(d->bus_clk);"),
        "required first clock → direct clk_get_rate form; got:\n{content}"
    );
    assert!(
        !content.contains("d->ref_clk ?"),
        "second (optional) clock must NOT produce its own uartclk ternary"
    );

    // FIFO wiring: role-typed, custom field name.
    let fifosize_count = content.matches("d->port.fifosize = ").count();
    assert_eq!(
        fifosize_count, 1,
        "exactly one d->port.fifosize assignment; got:\n{content}"
    );
    assert!(
        content.contains("d->port.fifosize = d->fifo_depth;"),
        "fifo wiring must use custom field 'fifo_depth'; got:\n{content}"
    );
    assert!(
        !content.contains("d->port.fifosize = d->fifo_size;"),
        "emitter must not hardcode 'fifo_size' field name"
    );
    assert!(
        !content.contains("d->port.fifosize = d->baud_rate;"),
        "non-role property must not be wired onto d->port.fifosize"
    );
}

// ---------------------------------------------------------------------------
// Typed ClockRateDefault is honored by the emitter regardless of the
// authored property id.
// ---------------------------------------------------------------------------

#[test]
fn emitter_honors_typed_clock_rate_default_role_for_custom_property_id() {
    // Start from a serial plan where the planner assigns the
    // ClockRateDefault role via the current-speed id convention,
    // then rename the property id to a custom spelling. This
    // exercises the EMITTER's role-typed consumption (the regression
    // guard for leak 3) independent of the planner's id-match
    // limitation.
    let portable = serial_plan(
        "SerialSpec",
        vec![mmio("regs", "primary_regs", None)],
        vec![irq("irq", "primary_irq")],
        vec![clock("ref_clk", "only_clk", false)], // optional → ternary fires
        vec![prop(
            "current-speed",
            "default_speed_val",
            "u32",
            false,
            Some("9600"),
        )],
    );
    let mut linux =
        build_linux_backend_plan(&portable, empty_semantic_model()).expect("serial plan builds");
    // Sanity: planner tagged the role via the id convention.
    assert_eq!(
        linux.property_realization[0].serial_role,
        Some(SerialPortRole::ClockRateDefault)
    );
    // Rename the property id to a custom authored spelling while
    // keeping the typed role. emit_constants generates the define
    // from property_id, so both define + ternary substitution flow
    // through the new id.
    linux.property_realization[0].property_id = "default-speed".into();

    let dir = TempDir::new().expect("tempdir");
    let content = emit_module_c(&linux, dir.path());

    // The ternary fallback must use the generated define, not
    // "115200", because the role-tagged property has a
    // default_literal.
    let expected_define = "SERIAL_SPEC_DEFAULT_DEFAULT_SPEED";
    assert!(
        content.contains(&format!(
            "d->port.uartclk = d->ref_clk ? clk_get_rate(d->ref_clk) : {expected_define};"
        )),
        "emitter must substitute the role-driven define for the \
         custom-id ClockRateDefault property; got:\n{content}"
    );
    assert!(
        !content.contains("115200"),
        "framework fallback must not appear when a typed \
         ClockRateDefault property has a default_literal"
    );
}

// ---------------------------------------------------------------------------
// Planner-convention limitation test.
// ---------------------------------------------------------------------------

#[test]
fn planner_convention_limits_clock_rate_default_to_current_speed_id() {
    // The planner's id-match convention only tags `"current-speed"`
    // as ClockRateDefault; a custom-id property keeps serial_role =
    // None and the emitter's optional-clock ternary therefore falls
    // back to "115200". This test documents the current transitional
    // limitation and will be retired by the authoring-surface
    // follow-up task.
    let portable = serial_plan(
        "CustomIdSpec",
        vec![mmio("regs", "primary_regs", None)],
        vec![irq("irq", "primary_irq")],
        vec![clock("ref_clk", "only_clk", false)], // optional
        vec![prop(
            "default-speed", // NOT "current-speed"
            "default_speed_val",
            "u32",
            false,
            Some("9600"),
        )],
    );
    let linux = build_linux_backend_plan(&portable, empty_semantic_model())
        .expect("custom-id serial plan builds");
    assert_eq!(
        linux.property_realization[0].serial_role, None,
        "planner's id convention does not tag custom-id properties (transitional)"
    );

    let dir = TempDir::new().expect("tempdir");
    let content = emit_module_c(&linux, dir.path());

    assert!(
        content.contains("d->port.uartclk = d->ref_clk ? clk_get_rate(d->ref_clk) : 115200;"),
        "without a typed ClockRateDefault tag, the ternary falls to \
         the framework default '115200'; got:\n{content}"
    );
}
