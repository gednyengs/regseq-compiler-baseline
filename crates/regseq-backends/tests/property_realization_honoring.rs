//! Property realization tests for `regseq-linux-backend-07`.
//!
//! Exercises the typed [`LinuxPropertyScalar`] enum, the rejection
//! of unsupported scalar spellings, and the emitter's consumption of
//! `LinuxPropertyRealization.field_name` + `scalar.read_api()` /
//! `scalar.call_shape()`. PL011 byte parity for u32 properties is
//! covered by `pl011_full_spec_lowering.rs`; these tests cover the
//! bool / u64 / custom-field-name / rejected-scalar code paths that
//! PL011 does not exercise.

use std::path::Path;

use regseq_backends::linux::body_index::FunctionBodyIndex;
use regseq_backends::linux::emitter::emit_linux_artifacts;
use regseq_backends::linux::plan::{LinuxFileRole, LinuxPropertyScalar};
use regseq_backends::linux::planner::{build_linux_backend_plan, LinuxPlanError};
use regseq_backends::lowering::diagnostics::DriverLoweringDiagnostics;
use regseq_backends::lowering::plan::{
    AcquisitionStep, BusIdentity, BusType, CallbackInventory, DeviceClassRoles, DriverIdentity,
    DriverLoweringPlan, PropertyObligation, RegisterObligations, ResourceKind, ResourceObligations,
    ResourcePolicy, SynchronizationObligations,
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

/// Build a minimal portable plan with no MMIO/IRQ/clock resources,
/// just the supplied property list. The property is always the first
/// (and only) step in the acquisition order.
fn plan_with_properties(properties: Vec<PropertyObligation>) -> DriverLoweringPlan {
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

    DriverLoweringPlan {
        identity: DriverIdentity {
            driver_name: "PropSpec".into(),
            compatible: vec!["vendor,propspec".into()],
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
                compatible: vec!["vendor,propspec".into()],
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

fn emit_module_c(linux: &regseq_backends::linux::plan::LinuxBackendPlan, dir: &Path) -> String {
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
// LinuxPropertyScalar::parse
// ---------------------------------------------------------------------------

#[test]
fn property_scalar_parse_round_trips_all_five_spellings() {
    assert_eq!(
        LinuxPropertyScalar::parse("u8"),
        Some(LinuxPropertyScalar::U8)
    );
    assert_eq!(
        LinuxPropertyScalar::parse("u16"),
        Some(LinuxPropertyScalar::U16)
    );
    assert_eq!(
        LinuxPropertyScalar::parse("u32"),
        Some(LinuxPropertyScalar::U32)
    );
    assert_eq!(
        LinuxPropertyScalar::parse("u64"),
        Some(LinuxPropertyScalar::U64)
    );
    assert_eq!(
        LinuxPropertyScalar::parse("bool"),
        Some(LinuxPropertyScalar::Bool)
    );
    assert_eq!(LinuxPropertyScalar::parse("i32"), None);
    assert_eq!(LinuxPropertyScalar::parse(""), None);
    assert_eq!(LinuxPropertyScalar::parse("U32"), None);
}

#[test]
fn property_scalar_c_type_and_read_api_cover_every_variant() {
    for (spelling, c_ty, api) in [
        ("u8", "u8", "device_property_read_u8"),
        ("u16", "u16", "device_property_read_u16"),
        ("u32", "u32", "device_property_read_u32"),
        ("u64", "u64", "device_property_read_u64"),
        ("bool", "bool", "device_property_read_bool"),
    ] {
        let s = LinuxPropertyScalar::parse(spelling).expect("known spelling");
        assert_eq!(s.c_type(), c_ty);
        assert_eq!(s.read_api(), api);
    }
}

// ---------------------------------------------------------------------------
// Plan construction
// ---------------------------------------------------------------------------

#[test]
fn unsupported_scalar_fails_plan_build_with_structured_error() {
    let portable = plan_with_properties(vec![prop("weird-prop", "weird_prop", "i32", true, None)]);

    let err = build_linux_backend_plan(&portable, empty_semantic_model())
        .expect_err("i32 is outside the Linux scalar allowlist");
    match err {
        LinuxPlanError::UnsupportedPropertyScalar {
            property_id,
            spelling,
        } => {
            assert_eq!(property_id, "weird-prop");
            assert_eq!(spelling, "i32");
            let msg = format!(
                "{}",
                LinuxPlanError::UnsupportedPropertyScalar {
                    property_id,
                    spelling,
                }
            );
            assert!(
                msg.contains("weird-prop") && msg.contains("i32"),
                "error display must include property id and spelling: {msg}"
            );
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn bool_property_plan_carries_bool_scalar() {
    let portable = plan_with_properties(vec![prop("wakeup-source", "wakeup", "bool", false, None)]);

    let linux =
        build_linux_backend_plan(&portable, empty_semantic_model()).expect("bool property builds");
    assert_eq!(linux.property_realization.len(), 1);
    let real = &linux.property_realization[0];
    assert_eq!(real.property_id, "wakeup-source");
    assert_eq!(real.field_name, "wakeup");
    assert_eq!(real.scalar, LinuxPropertyScalar::Bool);
    assert_eq!(real.scalar.read_api(), "device_property_read_bool");
}

#[test]
fn u64_property_plan_carries_u64_scalar() {
    let portable =
        plan_with_properties(vec![prop("big-counter", "big_counter", "u64", true, None)]);

    let linux =
        build_linux_backend_plan(&portable, empty_semantic_model()).expect("u64 property builds");
    let real = &linux.property_realization[0];
    assert_eq!(real.scalar, LinuxPropertyScalar::U64);
    assert_eq!(real.scalar.c_type(), "u64");
    assert_eq!(real.scalar.read_api(), "device_property_read_u64");
}

#[test]
fn shared_struct_field_uses_scalar_c_type_for_u64() {
    let portable =
        plan_with_properties(vec![prop("big-counter", "big_counter", "u64", true, None)]);

    let linux =
        build_linux_backend_plan(&portable, empty_semantic_model()).expect("u64 property builds");
    let field = linux
        .shared_struct_fields
        .iter()
        .find(|f| f.field_name == "big_counter")
        .expect("field present");
    assert_eq!(
        field.c_type, "u64",
        "shared-struct field must carry the scalar's c_type, not the u32 fallback"
    );
}

// ---------------------------------------------------------------------------
// Emission shapes
// ---------------------------------------------------------------------------

#[test]
fn bool_property_emits_assign_return_shape() {
    let portable = plan_with_properties(vec![prop("wakeup-source", "wakeup", "bool", false, None)]);
    let linux =
        build_linux_backend_plan(&portable, empty_semantic_model()).expect("bool property builds");

    let dir = TempDir::new().expect("tempdir");
    let content = emit_module_c(&linux, dir.path());

    assert!(
        content.contains("d->wakeup = device_property_read_bool(&pdev->dev, \"wakeup-source\");"),
        "bool property must use the presence-test assign-return shape; got:\n{content}"
    );
    assert!(
        !content.contains("ret = device_property_read_bool"),
        "bool property must NOT use the integer-style out-pointer shape"
    );
    assert!(
        !content.contains("device_property_read_u32(&pdev->dev, \"wakeup-source\""),
        "bool property must NOT fall back to the u32 API"
    );
}

#[test]
fn u64_property_emits_out_pointer_shape_with_u64_api() {
    let portable =
        plan_with_properties(vec![prop("big-counter", "big_counter", "u64", true, None)]);
    let linux =
        build_linux_backend_plan(&portable, empty_semantic_model()).expect("u64 property builds");

    let dir = TempDir::new().expect("tempdir");
    let content = emit_module_c(&linux, dir.path());

    assert!(
        content.contains(
            "ret = device_property_read_u64(&pdev->dev, \"big-counter\", &d->big_counter);"
        ),
        "u64 property must use the u64 API with out-pointer shape; got:\n{content}"
    );
    assert!(
        !content.contains("device_property_read_u32(&pdev->dev, \"big-counter\""),
        "u64 property must NOT silently fall back to the u32 API"
    );
}

#[test]
fn bool_property_with_default_ignores_default_in_emission() {
    // Spec: bool properties never take a `_DEFAULT_` fallback. The
    // emitter silently ignores `default_literal: Some(_)` on bool
    // properties -- no `_DEFAULT_` define, no fallback branch.
    let portable = plan_with_properties(vec![prop(
        "wakeup-source",
        "wakeup",
        "bool",
        false,
        Some("false"),
    )]);
    let linux =
        build_linux_backend_plan(&portable, empty_semantic_model()).expect("bool property builds");

    let dir = TempDir::new().expect("tempdir");
    let content = emit_module_c(&linux, dir.path());

    assert!(
        !content.contains("_DEFAULT_WAKEUP_SOURCE"),
        "bool property must not emit a _DEFAULT_ define; got:\n{content}"
    );
    assert!(
        !content.contains("if (ret)"),
        "bool-only probe must not render a `ret` error branch for the property"
    );
    assert!(
        content.contains("d->wakeup = device_property_read_bool(&pdev->dev, \"wakeup-source\");"),
        "bool property must still emit the AssignReturn shape"
    );
}

#[test]
fn custom_field_name_is_honored_in_emission() {
    let portable = plan_with_properties(vec![prop(
        "tx-threshold-level",
        "tx_high_water",
        "u32",
        true,
        None,
    )]);
    let linux = build_linux_backend_plan(&portable, empty_semantic_model()).expect("plan builds");

    let dir = TempDir::new().expect("tempdir");
    let content = emit_module_c(&linux, dir.path());

    assert!(
        content.contains("&d->tx_high_water"),
        "emitter must write to the authored field name 'tx_high_water'; got:\n{content}"
    );
    assert!(
        !content.contains("&d->tx_threshold_level"),
        "emitter must NOT derive the field name from property_id via replace('-', \"_\")"
    );
}
