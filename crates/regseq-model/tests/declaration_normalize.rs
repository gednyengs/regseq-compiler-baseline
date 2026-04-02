//! Normalization tests for raw frontend artifact to declaration model.
//!
//! Tests cover: representative artifact normalization, all raw families
//! mapping correctly, stringly tags becoming typed enums, containing_fn
//! provenance preservation, interrupt Layer 2 sidecar separation, absent
//! family handling, property-lint survival, synchronization sub-family
//! distinction, and logging/timed-delay detail preservation.

use regseq_model::declaration::callbacks::{CallbackContext, CallbackFamily};
use regseq_model::declaration::normalize::{normalize, normalize_from_json, NormalizeError};
use regseq_model::declaration::operations::{
    DelayIntrinsic, DelayPathKind, DurationCtor, LogArgKind, LogLevel,
};
use regseq_model::declaration::raw_input::*;
use regseq_model::declaration::registers::WrapperMethod;
use regseq_model::declaration::resources::{
    ClockMethod, InterruptFieldShape, IsrBindTarget, ResetMethod,
};
use regseq_model::declaration::synchronization::{CompletionMethod, SyncFieldKind};
use regseq_model::declaration::DeviceClass;

// ---- 1. Representative raw artifact normalizes successfully ----

#[test]
fn pl011_spec_slice_fixture_normalizes() {
    let json = include_str!(
        "../../../tests/fixtures/pl011-spec-slice/regseq-out/drivers/pl011-spec-slice.raw.json"
    );
    let model = normalize_from_json(json).expect("normalization should succeed");
    assert_eq!(model.driver.driver_name, "pl011-spec-slice");
    assert_eq!(model.driver.compatible, vec!["arm,pl011"]);
    assert_eq!(model.build_context.target_os, "linux");
}

// ---- 2. All currently present raw families map into the declaration model ----

#[test]
fn all_raw_families_map_into_model() {
    let input = make_rich_input();
    let model = normalize(&input).expect("normalization should succeed");

    // Identity
    assert_eq!(model.driver.driver_name, "test-driver");

    // Register access
    assert_eq!(model.registers.blocks.len(), 1);
    assert_eq!(model.registers.descriptors.len(), 1);
    assert_eq!(model.registers.wrapper_call_sites.len(), 1);
    assert_eq!(model.registers.poll_call_sites.len(), 1);
    assert_eq!(model.resources.mmio.len(), 1);

    // Callbacks
    assert_eq!(model.callbacks.lifecycle_impls.len(), 1);
    assert_eq!(model.callbacks.serial_impls.len(), 1);

    // Properties
    assert_eq!(model.properties.fields.len(), 1);
    assert_eq!(model.properties.lints.len(), 1);

    // Clock
    assert_eq!(model.resources.clock.fields.len(), 1);
    assert_eq!(model.resources.clock.call_sites.len(), 1);

    // Reset
    assert_eq!(model.resources.reset.fields.len(), 1);
    assert_eq!(model.resources.reset.call_sites.len(), 1);

    // PinCtrl
    assert_eq!(model.resources.pinctrl.fields.len(), 1);
    assert_eq!(model.resources.pinctrl.call_sites.len(), 1);

    // Interrupts
    assert_eq!(model.resources.interrupts.resources.len(), 1);
    assert_eq!(model.resources.interrupts.isr_bindings.len(), 1);

    // Synchronization
    assert_eq!(model.synchronization.sync_fields.len(), 1);
    assert_eq!(model.synchronization.lock_operations.len(), 1);
    assert_eq!(model.synchronization.completion_operations.len(), 1);
    assert_eq!(model.synchronization.guard_scopes.len(), 1);

    // Timed delay
    assert_eq!(model.timed_delay.call_sites.len(), 1);
    assert_eq!(model.timed_delay.duration_ctors.len(), 1);

    // Logging
    assert_eq!(model.logging.call_sites.len(), 1);

    // PM gate
    assert_eq!(model.pm_gate.gates.len(), 1);
}

// ---- 3. Stringly raw kinds become typed enums ----

#[test]
fn wrapper_method_normalized_to_typed_enum() {
    let input = make_rich_input();
    let model = normalize(&input).unwrap();
    assert_eq!(
        model.registers.wrapper_call_sites[0].method,
        WrapperMethod::Write
    );
}

#[test]
fn clock_method_normalized_to_typed_enum() {
    let input = make_rich_input();
    let model = normalize(&input).unwrap();
    assert_eq!(
        model.resources.clock.call_sites[0].method,
        ClockMethod::Enable
    );
}

#[test]
fn reset_method_normalized_to_typed_enum() {
    let input = make_rich_input();
    let model = normalize(&input).unwrap();
    assert_eq!(
        model.resources.reset.call_sites[0].method,
        ResetMethod::Deassert
    );
}

#[test]
fn sync_field_kind_normalized_to_typed_enum() {
    let input = make_rich_input();
    let model = normalize(&input).unwrap();
    assert_eq!(
        model.synchronization.sync_fields[0].kind,
        SyncFieldKind::SpinLock
    );
}

#[test]
fn completion_method_normalized_to_typed_enum() {
    let input = make_rich_input();
    let model = normalize(&input).unwrap();
    assert_eq!(
        model.synchronization.completion_operations[0].method,
        CompletionMethod::Wait
    );
}

#[test]
fn delay_intrinsic_normalized_to_typed_enum() {
    let input = make_rich_input();
    let model = normalize(&input).unwrap();
    assert_eq!(
        model.timed_delay.call_sites[0].intrinsic,
        DelayIntrinsic::BusyWait
    );
    assert_eq!(
        model.timed_delay.call_sites[0].path_kind,
        DelayPathKind::Canonical
    );
}

#[test]
fn duration_ctor_normalized_to_typed_enum() {
    let input = make_rich_input();
    let model = normalize(&input).unwrap();
    assert_eq!(
        model.timed_delay.duration_ctors[0].ctor,
        DurationCtor::FromUs
    );
}

#[test]
fn log_level_normalized_to_typed_enum() {
    let input = make_rich_input();
    let model = normalize(&input).unwrap();
    assert_eq!(model.logging.call_sites[0].level, LogLevel::Info);
}

#[test]
fn log_arg_kinds_normalized_to_typed_enums() {
    let input = make_rich_input();
    let model = normalize(&input).unwrap();
    assert_eq!(model.logging.call_sites[0].arg_kinds, vec![LogArgKind::U32]);
}

#[test]
fn callback_context_normalized_to_typed_enum() {
    let input = make_rich_input();
    let model = normalize(&input).unwrap();
    assert_eq!(
        model.callbacks.lifecycle_impls[0].context,
        CallbackContext::Framework
    );
    assert_eq!(
        model.callbacks.lifecycle_impls[0].family,
        CallbackFamily::Lifecycle
    );
    assert_eq!(
        model.callbacks.serial_impls[0].context,
        CallbackContext::SubsystemSerialized
    );
}

#[test]
fn device_class_normalized_to_typed_enum() {
    let input = make_rich_input();
    let model = normalize(&input).unwrap();
    assert_eq!(model.driver.device_class, DeviceClass::SerialUart);
}

#[test]
fn interrupt_field_shape_normalized() {
    let input = make_rich_input();
    let model = normalize(&input).unwrap();
    assert_eq!(
        model.resources.interrupts.resources[0].shape,
        InterruptFieldShape::Single
    );
}

#[test]
fn isr_bind_target_normalized() {
    let input = make_rich_input();
    let model = normalize(&input).unwrap();
    assert_eq!(
        model.resources.interrupts.isr_bindings[0].bind_target,
        IsrBindTarget::Auto
    );
}

// ---- 4. Construct-local call-site evidence retains containing_fn provenance ----

#[test]
fn wrapper_call_site_retains_containing_fn() {
    let input = make_rich_input();
    let model = normalize(&input).unwrap();
    let site = &model.registers.wrapper_call_sites[0];
    assert_eq!(site.containing_fn, "init");
    assert_eq!(site.provenance.containing_item.as_deref(), Some("init"));
    assert_eq!(site.provenance.raw_family, "register_access");
}

#[test]
fn clock_call_site_retains_containing_fn() {
    let input = make_rich_input();
    let model = normalize(&input).unwrap();
    let site = &model.resources.clock.call_sites[0];
    assert_eq!(site.containing_fn, "init");
    assert_eq!(site.provenance.containing_item.as_deref(), Some("init"));
}

#[test]
fn lock_operation_retains_containing_fn() {
    let input = make_rich_input();
    let model = normalize(&input).unwrap();
    let op = &model.synchronization.lock_operations[0];
    assert_eq!(op.containing_fn, "tx_chars");
    assert_eq!(op.provenance.containing_item.as_deref(), Some("tx_chars"));
}

#[test]
fn log_call_site_retains_containing_fn() {
    let input = make_rich_input();
    let model = normalize(&input).unwrap();
    let site = &model.logging.call_sites[0];
    assert_eq!(site.containing_fn, "init");
    assert_eq!(site.provenance.containing_item.as_deref(), Some("init"));
}

// ---- 5. Interrupt Layer 2 leakage is kept separate from declaration facts ----

#[test]
fn interrupt_binding_hints_preserved_as_sidecar() {
    let input = make_rich_input();
    let model = normalize(&input).unwrap();
    let irq = &model.resources.interrupts.resources[0];
    assert_eq!(irq.binding_hints.trigger_type.as_deref(), Some("level"));
    assert_eq!(irq.binding_hints.shared, Some(false));
    // The binding hint fields are separate from the declaration identity.
    assert_eq!(irq.interrupt_id, "uart_irq");
}

// ---- 6. Absent families remain absent rather than being inferred ----

#[test]
fn absent_families_normalize_to_empty() {
    let input = RawArtifactInput {
        driver_name: "minimal".into(),
        target_os: "linux".into(),
        identity: Some(RawIdentityInput {
            driver_name: "minimal".into(),
            compatible: vec![],
            device_class: "serial_uart".into(),
        }),
        register_access: None,
        trait_callbacks: None,
        firmware_properties: None,
        clock: None,
        reset: None,
        pinctrl: None,
        interrupts: None,
        synchronization: None,
        timed_delay: None,
        logging: None,
        pm_gate: None,
    };
    let model = normalize(&input).unwrap();
    assert!(model.registers.blocks.is_empty());
    assert!(model.registers.wrapper_call_sites.is_empty());
    assert!(model.resources.mmio.is_empty());
    assert!(model.resources.clock.fields.is_empty());
    assert!(model.resources.reset.fields.is_empty());
    assert!(model.resources.pinctrl.fields.is_empty());
    assert!(model.resources.interrupts.resources.is_empty());
    assert!(model.synchronization.sync_fields.is_empty());
    assert!(model.callbacks.lifecycle_impls.is_empty());
    assert!(model.properties.fields.is_empty());
    assert!(model.timed_delay.call_sites.is_empty());
    assert!(model.logging.call_sites.is_empty());
    assert!(model.pm_gate.gates.is_empty());
}

#[test]
fn missing_identity_returns_error() {
    let input = RawArtifactInput {
        driver_name: "no-id".into(),
        target_os: "linux".into(),
        identity: None,
        register_access: None,
        trait_callbacks: None,
        firmware_properties: None,
        clock: None,
        reset: None,
        pinctrl: None,
        interrupts: None,
        synchronization: None,
        timed_delay: None,
        logging: None,
        pm_gate: None,
    };
    let err = normalize(&input).unwrap_err();
    assert_eq!(err, NormalizeError::MissingIdentity);
}

// ---- 7. Property-side lints survive normalization ----

#[test]
fn property_lint_survives_normalization() {
    let input = make_rich_input();
    let model = normalize(&input).unwrap();
    assert_eq!(model.properties.lints.len(), 1);
    match &model.properties.lints[0] {
        regseq_model::declaration::PropertyLintDecl::RequiredWithDefault {
            property_id, ..
        } => {
            assert_eq!(property_id, "reg-io-width");
        }
    }
}

// ---- 8. Synchronization sub-families stay structurally distinct ----

#[test]
fn sync_sub_families_distinct_after_normalization() {
    let input = make_rich_input();
    let model = normalize(&input).unwrap();
    assert_eq!(
        model.synchronization.sync_fields[0].kind,
        SyncFieldKind::SpinLock
    );
    assert_eq!(
        model.synchronization.lock_operations[0].lock_kind,
        SyncFieldKind::SpinLock
    );
    assert_eq!(
        model.synchronization.completion_operations[0].method,
        CompletionMethod::Wait
    );
    assert_eq!(
        model.synchronization.guard_scopes[0].lock_kind,
        SyncFieldKind::SpinLock
    );
}

// ---- 9. Logging/timed-delay detail survives normalization ----

#[test]
fn logging_detail_survives_normalization() {
    let input = make_rich_input();
    let model = normalize(&input).unwrap();
    let site = &model.logging.call_sites[0];
    assert_eq!(site.level, LogLevel::Info);
    assert!(site.canonical);
    assert_eq!(site.template, "initialized: baud={}");
    assert_eq!(site.arg_count, 1);
    assert_eq!(site.arg_kinds, vec![LogArgKind::U32]);
}

#[test]
fn timed_delay_detail_survives_normalization() {
    let input = make_rich_input();
    let model = normalize(&input).unwrap();
    let site = &model.timed_delay.call_sites[0];
    assert_eq!(site.intrinsic, DelayIntrinsic::BusyWait);
    assert_eq!(site.path_kind, DelayPathKind::Canonical);
    assert_eq!(site.containing_fn, "init");
    let ctor = &model.timed_delay.duration_ctors[0];
    assert_eq!(ctor.ctor, DurationCtor::FromUs);
    assert_eq!(ctor.containing_fn, "init");
}

// ---- 10. Build context is separate from identity ----

#[test]
fn build_context_populated_from_target_os() {
    let input = make_rich_input();
    let model = normalize(&input).unwrap();
    assert_eq!(model.build_context.target_os, "linux");
}

// ---- 11. PM gate detail survives ----

#[test]
fn pm_gate_detail_survives_normalization() {
    let input = make_rich_input();
    let model = normalize(&input).unwrap();
    let gate = &model.pm_gate.gates[0];
    assert_eq!(gate.driver_type, "TestDriver");
    assert!(gate.properties.idle);
    assert!(!gate.properties.wakeup_capable);
    assert!(gate.has_pm_impl);
}

// ---- 12. Unrecognized tag error ----

#[test]
fn unrecognized_wrapper_method_returns_error() {
    let mut input = make_rich_input();
    input.register_access.as_mut().unwrap().wrapper_call_sites[0].method = "unknown_op".into();
    let err = normalize(&input).unwrap_err();
    assert_eq!(
        err,
        NormalizeError::UnrecognizedTag {
            family: "register_access",
            field: "method",
            value: "unknown_op".into(),
        }
    );
}

// ---- Helpers ----

fn make_rich_input() -> RawArtifactInput {
    RawArtifactInput {
        driver_name: "test-driver".into(),
        target_os: "linux".into(),
        identity: Some(RawIdentityInput {
            driver_name: "test-driver".into(),
            compatible: vec!["arm,pl011".into()],
            device_class: "serial_uart".into(),
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
            register_descriptors: vec![RawRegisterDescriptorInput {
                register_module: "DR_REG".into(),
                fields: vec![RawRegisterDescriptorFieldInput {
                    field_name: "DATA".into(),
                    offset: 0,
                    width: 8,
                    named_values: vec![RawRegisterNamedValueInput {
                        name: "ZERO".into(),
                        value: 0,
                    }],
                }],
            }],
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
    }
}
