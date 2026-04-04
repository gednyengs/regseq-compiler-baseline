//! Reviewability and traversal tests for the declaration model.
//!
//! Verifies deterministic output, stable serialized form, traversal helpers
//! respecting construct ownership, and round-trip serialization. Tests cover
//! the spec-slice fixture plus a richer synthetic input for families that
//! are empty in the spec-slice artifact.

use regseq_model::declaration::callbacks::CallbackFamily;
use regseq_model::declaration::normalize::normalize_from_json;
use regseq_model::declaration::operations::{DelayIntrinsic, LogLevel};
use regseq_model::declaration::raw_input::*;
use regseq_model::declaration::registers::WrapperMethod;
use regseq_model::declaration::synchronization::SyncFieldKind;
use regseq_model::declaration::{normalize, BusType, DeclarationModel};

// ---- 1. Declaration-model output is deterministic ----

#[test]
fn spec_slice_normalization_is_deterministic() {
    let json = include_str!("../../../tests/fixtures/pl011-spec-slice/expected/raw-frontend.json");
    let model_a = normalize_from_json(json).unwrap();
    let model_b = normalize_from_json(json).unwrap();
    assert_eq!(model_a, model_b);
}

#[test]
fn serialized_output_is_deterministic() {
    let json = include_str!("../../../tests/fixtures/pl011-spec-slice/expected/raw-frontend.json");
    let model = normalize_from_json(json).unwrap();
    let json_a = serde_json::to_string_pretty(&model).unwrap();
    let json_b = serde_json::to_string_pretty(&model).unwrap();
    assert_eq!(json_a, json_b);
}

// ---- 2. Representative PL011 declaration artifact is reviewable ----

#[test]
fn spec_slice_model_has_expected_driver_identity() {
    let model = load_spec_slice();
    assert_eq!(model.driver.driver_name, "pl011-spec-slice");
    assert_eq!(model.driver.compatible, vec!["arm,pl011"]);
    assert_eq!(model.driver.bus, Some(BusType::Amba));
    assert_eq!(model.driver.provenance.raw_family, "identity");
}

#[test]
fn spec_slice_model_has_expected_register_blocks() {
    let model = load_spec_slice();
    assert_eq!(model.registers.blocks.len(), 1);
    assert_eq!(model.registers.blocks[0].name, "Pl011Registers");
    assert_eq!(model.registers.blocks[0].fields.len(), 8);
}

#[test]
fn spec_slice_model_has_expected_wrapper_call_sites() {
    let model = load_spec_slice();
    assert!(!model.registers.wrapper_call_sites.is_empty());
    let write_sites: Vec<_> = model
        .registers
        .wrapper_call_sites
        .iter()
        .filter(|s| s.method == WrapperMethod::Write)
        .collect();
    assert!(!write_sites.is_empty());
}

#[test]
fn spec_slice_model_has_expected_callbacks() {
    let model = load_spec_slice();
    assert_eq!(model.callbacks.lifecycle_impls.len(), 1);
    assert_eq!(model.callbacks.serial_impls.len(), 1);
    assert!(model.callbacks.serial_console_impls.is_empty());
    assert!(model.callbacks.power_management_impls.is_empty());
}

#[test]
fn spec_slice_model_has_expected_properties() {
    let model = load_spec_slice();
    assert_eq!(model.properties.fields.len(), 2);
    assert_eq!(model.properties.lints.len(), 1);
}

#[test]
fn spec_slice_model_has_expected_resources() {
    let model = load_spec_slice();
    assert_eq!(model.resources.mmio.len(), 1);
    assert_eq!(model.resources.clock.fields.len(), 1);
    assert_eq!(model.resources.reset.fields.len(), 1);
    assert_eq!(model.resources.pinctrl.fields.len(), 1);
}

#[test]
fn spec_slice_model_has_empty_families_for_absent_data() {
    let model = load_spec_slice();
    // These families are empty in the spec-slice because the fixture
    // has no interrupt, sync, delay, logging, or PM data.
    assert!(model.resources.interrupts.resources.is_empty());
    assert!(model.synchronization.sync_fields.is_empty());
    assert!(model.timed_delay.call_sites.is_empty());
    assert!(model.logging.call_sites.is_empty());
    assert!(model.pm_gate.gates.is_empty());
}

#[test]
fn spec_slice_build_context_preserved() {
    let model = load_spec_slice();
    assert_eq!(model.build_context.target_os, "linux");
}

// ---- 3. Traversal helpers respect construct ownership ----

#[test]
fn all_resource_field_names_preserves_family() {
    let model = load_spec_slice();
    let fields = model.all_resource_field_names();
    // Should contain mmio, clock, reset, pinctrl fields with family labels.
    let mmio_fields: Vec<_> = fields.iter().filter(|(f, _)| *f == "mmio").collect();
    assert_eq!(mmio_fields.len(), 1);
    assert_eq!(mmio_fields[0].1, "regs");

    let clock_fields: Vec<_> = fields.iter().filter(|(f, _)| *f == "clock").collect();
    assert_eq!(clock_fields.len(), 1);
}

#[test]
fn all_call_sites_preserves_family_label() {
    let model = load_spec_slice();
    let sites = model.all_call_sites();
    // Spec-slice only has register_access wrapper call sites.
    assert!(!sites.is_empty());
    for site in &sites {
        assert_eq!(site.family, "register_access");
    }
}

#[test]
fn all_callback_impls_returns_all_families() {
    let model = load_spec_slice();
    let impls = model.all_callback_impls();
    assert_eq!(impls.len(), 2);
    assert!(impls.iter().any(|i| i.family == CallbackFamily::Lifecycle));
    assert!(impls.iter().any(|i| i.family == CallbackFamily::Serial));
}

#[test]
fn all_containing_fns_deduplicates() {
    let model = load_spec_slice();
    let fns = model.all_containing_fns();
    // Each fn should appear only once.
    let mut sorted = fns.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(fns.len(), sorted.len());
}

// ---- Traversal on richer model ----

#[test]
fn rich_model_all_call_sites_spans_families() {
    let model = load_rich_model();
    let sites = model.all_call_sites();
    let families: Vec<_> = sites.iter().map(|s| s.family).collect();
    assert!(families.contains(&"register_access"));
    assert!(families.contains(&"clock"));
    assert!(families.contains(&"reset"));
    assert!(families.contains(&"pinctrl"));
    assert!(families.contains(&"synchronization"));
    assert!(families.contains(&"timed_delay"));
    assert!(families.contains(&"logging"));
}

#[test]
fn rich_model_all_resource_fields_spans_families() {
    let model = load_rich_model();
    let fields = model.all_resource_field_names();
    let families: Vec<_> = fields.iter().map(|(f, _)| *f).collect();
    assert!(families.contains(&"mmio"));
    assert!(families.contains(&"interrupts"));
    assert!(families.contains(&"clock"));
    assert!(families.contains(&"reset"));
    assert!(families.contains(&"pinctrl"));
    assert!(families.contains(&"synchronization"));
}

#[test]
fn rich_model_traversal_shortcuts_work() {
    let model = load_rich_model();
    assert_eq!(model.wrapper_call_sites().len(), 1);
    assert_eq!(model.clock_fields().len(), 1);
    assert_eq!(model.reset_fields().len(), 1);
    assert_eq!(model.pinctrl_fields().len(), 1);
    assert_eq!(model.interrupt_resources().len(), 1);
    assert_eq!(model.isr_bindings().len(), 1);
    assert_eq!(model.sync_fields().len(), 1);
    assert_eq!(model.lock_operations().len(), 1);
    assert_eq!(model.completion_operations().len(), 1);
    assert_eq!(model.guard_scopes().len(), 1);
    assert_eq!(model.timed_delay_call_sites().len(), 1);
    assert_eq!(model.duration_ctor_sites().len(), 1);
    assert_eq!(model.log_call_sites().len(), 1);
    assert_eq!(model.pm_gates().len(), 1);
}

// ---- 4. Serialization round-trips ----

#[test]
fn spec_slice_model_round_trips_through_json() {
    let model = load_spec_slice();
    let json = serde_json::to_string(&model).unwrap();
    let restored: DeclarationModel = serde_json::from_str(&json).unwrap();
    assert_eq!(model, restored);
}

#[test]
fn rich_model_round_trips_through_json() {
    let model = load_rich_model();
    let json = serde_json::to_string(&model).unwrap();
    let restored: DeclarationModel = serde_json::from_str(&json).unwrap();
    assert_eq!(model, restored);
}

#[test]
fn spec_slice_model_round_trips_through_pretty_json() {
    let model = load_spec_slice();
    let json = serde_json::to_string_pretty(&model).unwrap();
    let restored: DeclarationModel = serde_json::from_str(&json).unwrap();
    assert_eq!(model, restored);
}

// ---- 5. Richer families are also reviewable in deterministic form ----

#[test]
fn rich_model_sync_sub_families_reviewable() {
    let model = load_rich_model();
    assert_eq!(model.sync_fields()[0].kind, SyncFieldKind::SpinLock);
    assert_eq!(model.sync_fields()[0].field_name, "lock");
}

#[test]
fn rich_model_timed_delay_reviewable() {
    let model = load_rich_model();
    assert_eq!(
        model.timed_delay_call_sites()[0].intrinsic,
        DelayIntrinsic::BusyWait
    );
}

#[test]
fn rich_model_logging_reviewable() {
    let model = load_rich_model();
    assert_eq!(model.log_call_sites()[0].level, LogLevel::Info);
    assert_eq!(model.log_call_sites()[0].template, "initialized: baud={}");
}

#[test]
fn rich_model_pm_gate_reviewable() {
    let model = load_rich_model();
    assert!(model.pm_gates()[0].properties.idle);
    assert!(model.pm_gates()[0].has_pm_impl);
}

#[test]
fn rich_model_interrupts_reviewable() {
    let model = load_rich_model();
    let irq = &model.interrupt_resources()[0];
    assert_eq!(irq.interrupt_id, "uart_irq");
    assert_eq!(irq.binding_hints.trigger_type.as_deref(), Some("level"));
}

#[test]
fn rich_model_serialized_output_is_deterministic() {
    let model = load_rich_model();
    let json_a = serde_json::to_string_pretty(&model).unwrap();
    let json_b = serde_json::to_string_pretty(&model).unwrap();
    assert_eq!(json_a, json_b);
}

// ---- Helpers ----

fn load_spec_slice() -> DeclarationModel {
    let json = include_str!("../../../tests/fixtures/pl011-spec-slice/expected/raw-frontend.json");
    normalize_from_json(json).expect("spec-slice normalization failed")
}

fn load_rich_model() -> DeclarationModel {
    let input = make_rich_input();
    normalize(&input).expect("rich model normalization failed")
}

fn make_rich_input() -> RawArtifactInput {
    RawArtifactInput {
        driver_name: "test-driver".into(),
        target_os: "linux".into(),
        identity: Some(RawIdentityInput {
            driver_name: "test-driver".into(),
            compatible: vec!["arm,pl011".into()],
            device_class: "serial_uart".into(),
            bus: Some("amba".into()),
        }),
        register_access: Some(RawRegisterAccessInput {
            register_blocks: vec![RawRegisterBlockInput {
                name: "TestRegs".into(),
                fields: vec![RawRegisterFieldInput {
                    name: "dr".into(),
                    access_width: None,
                    endian: None,
                    side_effects: None,
                    hw_updates: None,
                }],
            }],
            mmio_fields: vec![RawMmioFieldInput {
                field_name: "regs".into(),
                mmio_id: Some("uart_regs".into()),
                register_block_type: Some("TestRegs".into()),
            }],
            wrapper_call_sites: vec![RawWrapperCallSiteInput {
                method: "write".into(),
                containing_fn: "init".into(),
            }],
            poll_call_sites: vec![RawPollCallSiteInput {
                containing_fn: "wait_tx".into(),
            }],
            register_descriptors: vec![],
        }),
        trait_callbacks: Some(RawTraitSurfaceInput {
            lifecycle_impls: vec![RawTraitImplInput {
                driver_type: "TestDriver".into(),
                context: "F".into(),
            }],
            serial_impls: vec![RawTraitImplInput {
                driver_type: "TestDriver".into(),
                context: "S".into(),
            }],
            serial_console_impls: vec![],
            power_management_impls: vec![],
        }),
        firmware_properties: Some(RawPropertySurfaceInput {
            fields: vec![RawPropertyFieldInput {
                driver_type: "TestDriver".into(),
                field_name: "reg_io_width".into(),
                field_type: "u32".into(),
                property_id: "reg-io-width".into(),
                default_literal: Some("4".into()),
                vendor_prefix: None,
                required: true,
            }],
            lints: vec![RawPropertyLintInput::RequiredWithDefault {
                driver_type: "TestDriver".into(),
                field_name: "reg_io_width".into(),
                property_id: "reg-io-width".into(),
            }],
        }),
        clock: Some(RawClockSurfaceInput {
            fields: vec![RawClockFieldInput {
                field_name: "clock".into(),
                clock_id: "uart_clk".into(),
                required: false,
            }],
            call_sites: vec![RawClockCallSiteInput {
                method: "enable".into(),
                containing_fn: "init".into(),
            }],
        }),
        reset: Some(RawResetSurfaceInput {
            fields: vec![RawResetFieldInput {
                field_name: "reset".into(),
                required: false,
            }],
            call_sites: vec![RawResetCallSiteInput {
                method: "deassert".into(),
                containing_fn: "init".into(),
            }],
        }),
        pinctrl: Some(RawPinCtrlSurfaceInput {
            fields: vec![RawPinCtrlFieldInput {
                field_name: "pinctrl".into(),
                required: false,
                pinctrl_id: Some("uart_pins".into()),
            }],
            call_sites: vec![RawPinCtrlCallSiteInput {
                state_type: "default".into(),
                containing_fn: "init".into(),
            }],
        }),
        interrupts: Some(RawInterruptSurfaceInput {
            resources: vec![RawInterruptResourceInput {
                field_name: "irq".into(),
                interrupt_id: "uart_irq".into(),
                shape: "single".into(),
                layer2: RawInterruptLayer2Input {
                    trigger_type: Some("level".into()),
                    trigger_polarity: Some("high".into()),
                    priority: None,
                    shared: Some(false),
                },
            }],
            isr_bindings: vec![RawIsrBindingInput {
                handler_name: "handle_irq".into(),
                bind_target: "auto".into(),
                has_index_param: false,
                context: "H".into(),
            }],
        }),
        synchronization: Some(RawSynchronizationSurfaceInput {
            sync_fields: vec![RawSyncFieldInput {
                field_name: "lock".into(),
                kind: "spinlock".into(),
                payload_type: None,
            }],
            lock_operations: vec![RawLockOperationInput {
                lock_kind: "spinlock".into(),
                containing_fn: "tx_chars".into(),
            }],
            completion_operations: vec![RawCompletionOperationInput {
                method: "wait".into(),
                containing_fn: "wait_tx".into(),
            }],
            guard_scopes: vec![RawGuardScopeInput {
                lock_kind: "spinlock".into(),
                containing_fn: "tx_chars".into(),
            }],
        }),
        timed_delay: Some(RawTimedDelaySurfaceInput {
            call_sites: vec![RawTimedDelayCallSiteInput {
                intrinsic: "busy_wait".into(),
                path_kind: "canonical".into(),
                containing_fn: "init".into(),
            }],
            duration_ctors: vec![RawDurationCtorSiteInput {
                ctor: "from_us".into(),
                containing_fn: "init".into(),
            }],
        }),
        logging: Some(RawLoggingSurfaceInput {
            call_sites: vec![RawLogCallSiteInput {
                level: "info".into(),
                containing_fn: "init".into(),
                canonical: true,
                template: "initialized: baud={}".into(),
                arg_count: 1,
                arg_kinds: vec!["u32".into()],
            }],
        }),
        pm_gate: Some(RawPmGateSurfaceInput {
            gates: vec![RawPmGateInput {
                driver_type: "TestDriver".into(),
                properties: RawPmGatePropertiesInput {
                    idle: true,
                    wakeup_capable: false,
                },
                has_pm_impl: true,
            }],
        }),
        bodies: None,
    }
}
