//! Multi-resource Linux planner/emitter tests for
//! `regseq-linux-backend-06`.
//!
//! Exercises the per-resource probe/emission schema by building
//! portable [`DriverLoweringPlan`] values with non-default field
//! names, multiple MMIO regions, and multiple interrupts/clocks.
//! PL011 byte parity is covered by the single-resource tests in
//! `linux_emission_scaffold.rs`; these tests cover the new
//! multi-resource code paths that PL011 does not exercise.

use std::path::Path;

use regseq_backends::linux::body_index::FunctionBodyIndex;
use regseq_backends::linux::emitter::emit_linux_artifacts;
use regseq_backends::linux::plan::{LinuxFileRole, LinuxProbeStep};
use regseq_backends::linux::planner::{build_linux_backend_plan, LinuxPlanError};
use regseq_backends::lowering::diagnostics::DriverLoweringDiagnostics;
use regseq_backends::lowering::plan::{
    AcquisitionStep, BusIdentity, BusType, CallbackInventory, ClockObligation, DeviceClassRoles,
    DriverIdentity, DriverLoweringPlan, InterruptObligation, MmioObligation,
    RegisterBlockObligation, RegisterFieldInfo, RegisterObligations, ResourceKind,
    ResourceObligations, ResourcePolicy, SynchronizationObligations, TeardownStep,
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

/// Build a portable plan with two MMIO regions, two interrupts, two
/// clocks, and two register blocks. Each MMIO claims ownership of one
/// register block via `register_block_type`.
fn two_of_everything_plan() -> DriverLoweringPlan {
    let mmio = vec![
        MmioObligation {
            field_name: "memory".into(),
            mmio_id: "core_regs".into(),
            register_block_type: Some("CoreRegs".into()),
            policy: ResourcePolicy::Required,
            provenance: provenance("mmio"),
        },
        MmioObligation {
            field_name: "aux_memory".into(),
            mmio_id: "aux_regs".into(),
            register_block_type: Some("AuxRegs".into()),
            policy: ResourcePolicy::Required,
            provenance: provenance("mmio"),
        },
    ];
    let interrupts = vec![
        InterruptObligation {
            field_name: "hw_irq".into(),
            interrupt_id: "primary_irq".into(),
            trigger_type: None,
            shared: None,
            policy: ResourcePolicy::Required,
            provenance: provenance("interrupt"),
        },
        InterruptObligation {
            field_name: "aux_irq_line".into(),
            interrupt_id: "aux_irq".into(),
            trigger_type: None,
            shared: None,
            policy: ResourcePolicy::Required,
            provenance: provenance("interrupt"),
        },
    ];
    let clocks = vec![
        ClockObligation {
            field_name: "baud_clk".into(),
            clock_id: "baud".into(),
            required: true,
            default_frequency: None,
            policy: ResourcePolicy::Required,
            provenance: provenance("clock"),
        },
        ClockObligation {
            field_name: "aux_clock".into(),
            clock_id: "aux".into(),
            required: false,
            default_frequency: None,
            policy: ResourcePolicy::OptionalSkip,
            provenance: provenance("clock"),
        },
    ];

    let resources = ResourceObligations {
        mmio,
        interrupts,
        properties: vec![],
        clocks,
        resets: vec![],
        pinctrl: vec![],
        bus_identity: BusIdentity {
            bus_type: BusType::Platform,
            compatible: vec!["vendor,multi".into()],
            amba_id: None,
            amba_revision: None,
            amba_mask: None,
        },
    };

    let acquisition_order = vec![
        AcquisitionStep {
            order: 0,
            resource_kind: ResourceKind::Clock,
            resource_id: "baud".into(),
            policy: ResourcePolicy::Required,
        },
        AcquisitionStep {
            order: 1,
            resource_kind: ResourceKind::Clock,
            resource_id: "aux".into(),
            policy: ResourcePolicy::OptionalSkip,
        },
        AcquisitionStep {
            order: 2,
            resource_kind: ResourceKind::Mmio,
            resource_id: "core_regs".into(),
            policy: ResourcePolicy::Required,
        },
        AcquisitionStep {
            order: 3,
            resource_kind: ResourceKind::Mmio,
            resource_id: "aux_regs".into(),
            policy: ResourcePolicy::Required,
        },
        AcquisitionStep {
            order: 4,
            resource_kind: ResourceKind::Interrupt,
            resource_id: "primary_irq".into(),
            policy: ResourcePolicy::Required,
        },
        AcquisitionStep {
            order: 5,
            resource_kind: ResourceKind::Interrupt,
            resource_id: "aux_irq".into(),
            policy: ResourcePolicy::Required,
        },
    ];
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

    let register_obligations = RegisterObligations {
        blocks: vec![
            RegisterBlockObligation {
                name: "CoreRegs".into(),
                field_count: 1,
                register_fields: vec![RegisterFieldInfo {
                    name: "core_status".into(),
                    access_width: Some(32),
                    byte_offset: 0,
                }],
                provenance: provenance("register_block"),
            },
            RegisterBlockObligation {
                name: "AuxRegs".into(),
                field_count: 1,
                register_fields: vec![RegisterFieldInfo {
                    name: "aux_status".into(),
                    access_width: Some(32),
                    byte_offset: 0,
                }],
                provenance: provenance("register_block"),
            },
        ],
        access_sites: vec![],
        poll_sites: vec![],
        descriptors: vec![],
    };

    DriverLoweringPlan {
        identity: DriverIdentity {
            driver_name: "MultiSpec".into(),
            compatible: vec!["vendor,multi".into()],
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
        resources,
        acquisition_order,
        teardown_order,
        device_class_roles: DeviceClassRoles {
            device_class: DeviceClass::Other(String::new()),
            serial_ops_realized: vec![],
            has_lifecycle: false,
            has_power_management: false,
        },
        register_obligations,
        synchronization: SynchronizationObligations {
            fields: vec![],
            lock_scopes: vec![],
            completion_uses: vec![],
        },
        diagnostics: DriverLoweringDiagnostics::default(),
    }
}

fn emit_plan(plan: &regseq_backends::linux::plan::LinuxBackendPlan, dir: &Path) -> String {
    let bodies = FunctionBodyIndex::default();
    let set = emit_linux_artifacts(plan, &bodies, dir).expect("emit");
    let main = set
        .artifacts
        .iter()
        .find(|a| a.role == LinuxFileRole::DriverMain)
        .expect("driver main");
    std::fs::read_to_string(&main.path).expect("read module.c")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn build_probe_emits_field_names_on_every_resource_step() {
    let portable = two_of_everything_plan();
    let linux =
        build_linux_backend_plan(&portable, empty_semantic_model()).expect("linux plan builds");

    let clock_fields: Vec<&str> = linux
        .probe
        .steps
        .iter()
        .filter_map(|s| match s {
            LinuxProbeStep::GetClock { field_name, .. } => Some(field_name.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(clock_fields, vec!["baud_clk", "aux_clock"]);

    let mmio_fields_indexes: Vec<(&str, u32)> = linux
        .probe
        .steps
        .iter()
        .filter_map(|s| match s {
            LinuxProbeStep::MapMmio {
                field_name, index, ..
            } => Some((field_name.as_str(), *index)),
            _ => None,
        })
        .collect();
    assert_eq!(mmio_fields_indexes, vec![("memory", 0), ("aux_memory", 1)]);

    let irq_fields_indexes: Vec<(&str, u32)> = linux
        .probe
        .steps
        .iter()
        .filter_map(|s| match s {
            LinuxProbeStep::RequestIrq {
                field_name, index, ..
            } => Some((field_name.as_str(), *index)),
            _ => None,
        })
        .collect();
    assert_eq!(irq_fields_indexes, vec![("hw_irq", 0), ("aux_irq_line", 1)]);
}

#[test]
fn build_probe_carries_per_clock_optional_flag() {
    let portable = two_of_everything_plan();
    let linux =
        build_linux_backend_plan(&portable, empty_semantic_model()).expect("linux plan builds");

    let clock_flags: Vec<(&str, bool)> = linux
        .probe
        .steps
        .iter()
        .filter_map(|s| match s {
            LinuxProbeStep::GetClock {
                field_name,
                optional,
                ..
            } => Some((field_name.as_str(), *optional)),
            _ => None,
        })
        .collect();
    assert_eq!(clock_flags, vec![("baud_clk", false), ("aux_clock", true)]);
}

#[test]
fn build_register_layout_resolves_owning_mmio_field() {
    let portable = two_of_everything_plan();
    let linux =
        build_linux_backend_plan(&portable, empty_semantic_model()).expect("linux plan builds");

    let mapping: Vec<(&str, &str)> = linux
        .register_layout
        .blocks
        .iter()
        .map(|b| (b.block_name.as_str(), b.owning_mmio_field.as_str()))
        .collect();
    assert_eq!(
        mapping,
        vec![("CoreRegs", "memory"), ("AuxRegs", "aux_memory")]
    );
}

#[test]
fn ambiguous_register_block_ownership_on_duplicate_claim() {
    let mut portable = two_of_everything_plan();
    portable.resources.mmio[1].register_block_type = Some("CoreRegs".into());

    let err = build_linux_backend_plan(&portable, empty_semantic_model()).expect_err(
        "both MMIOs claim CoreRegs -- planner must raise AmbiguousRegisterBlockOwnership",
    );
    match err {
        LinuxPlanError::AmbiguousRegisterBlockOwnership {
            block_name,
            candidates,
        } => {
            assert_eq!(block_name, "CoreRegs");
            assert_eq!(candidates, vec!["memory", "aux_memory"]);
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn unresolved_register_block_owner_also_raises_ambiguity() {
    let mut portable = two_of_everything_plan();
    portable.resources.mmio[0].register_block_type = Some("SomeOtherBlock".into());
    portable.resources.mmio[1].register_block_type = Some("SomeOtherBlock".into());
    // CoreRegs now has zero candidates.

    let err = build_linux_backend_plan(&portable, empty_semantic_model()).expect_err(
        "no MMIO claims CoreRegs -- planner must raise AmbiguousRegisterBlockOwnership",
    );
    match err {
        LinuxPlanError::AmbiguousRegisterBlockOwnership {
            block_name,
            candidates,
        } => {
            assert_eq!(block_name, "CoreRegs");
            assert!(candidates.is_empty());
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn multi_resource_emission_renders_all_field_names() {
    let portable = two_of_everything_plan();
    let linux =
        build_linux_backend_plan(&portable, empty_semantic_model()).expect("linux plan builds");

    let dir = TempDir::new().expect("tempdir");
    let content = emit_plan(&linux, dir.path());

    // Every authored MMIO field must appear in the generated C.
    assert!(
        content.contains("d->memory = devm_ioremap_resource"),
        "MMIO 'memory' initializer missing; got:\n{content}"
    );
    assert!(
        content.contains("d->aux_memory = devm_ioremap_resource"),
        "MMIO 'aux_memory' initializer missing"
    );

    // Per-family ordinals flow into platform_get_resource indices.
    assert!(
        content.contains("platform_get_resource(pdev, IORESOURCE_MEM, 0)"),
        "MMIO index 0 missing"
    );
    assert!(
        content.contains("platform_get_resource(pdev, IORESOURCE_MEM, 1)"),
        "MMIO index 1 missing"
    );

    // Every authored IRQ field must appear.
    assert!(content.contains("d->hw_irq = platform_get_irq(pdev, 0)"));
    assert!(content.contains("d->aux_irq_line = platform_get_irq(pdev, 1)"));

    // Every authored clock field must appear; required clock takes the
    // devm_clk_get API, optional clock takes devm_clk_get_optional.
    assert!(content.contains("d->baud_clk = devm_clk_get("));
    assert!(content.contains("d->aux_clock = devm_clk_get_optional("));

    // Grep gate: no PL011-shaped hardcoded field names for a driver
    // that did not author them.
    assert!(
        !content.contains("d->regs"),
        "multi-resource plan must not emit hardcoded 'd->regs'"
    );
    assert!(
        !content.contains("d->irq"),
        "multi-resource plan must not emit hardcoded 'd->irq'"
    );
    assert!(
        !content.contains("d->clock"),
        "multi-resource plan must not emit hardcoded 'd->clock'"
    );
}

#[test]
fn register_accessor_helpers_dereference_first_block_owning_mmio_field() {
    let portable = two_of_everything_plan();
    let linux =
        build_linux_backend_plan(&portable, empty_semantic_model()).expect("linux plan builds");

    // The `LinuxRegisterBlock.owning_mmio_field` resolution is what
    // this task plumbs end-to-end; asserting at the plan layer avoids
    // coupling to the emitter's single-helper-set shape (which
    // `regseq-linux-backend-05` amends with `{block_prefix}` naming).
    let mapping: Vec<(&str, &str)> = linux
        .register_layout
        .blocks
        .iter()
        .map(|b| (b.block_name.as_str(), b.owning_mmio_field.as_str()))
        .collect();
    assert_eq!(
        mapping,
        vec![("CoreRegs", "memory"), ("AuxRegs", "aux_memory")]
    );

    // The single helper set emitted today dereferences the first
    // block's owning MMIO field. PL011 has one block so this is
    // byte-identical to the pre-task-06 output; multi-block drivers
    // will get per-block helper names from task 05.
    let dir = TempDir::new().expect("tempdir");
    let content = emit_plan(&linux, dir.path());
    assert!(
        content.contains("readl(d->memory + off)"),
        "first-block accessor must dereference owning MMIO field 'memory'"
    );
    assert!(
        content.contains("writel(val, d->memory + off)"),
        "first-block write accessor must dereference 'memory'"
    );
}
