//! Schema-focused tests for the declaration model.
//!
//! Verifies that declaration-model types construct and import cleanly,
//! construct-owned sub-families remain structurally distinct, per-entity
//! provenance fields are present, build-context is separate from identity,
//! and interrupt binding hints are distinct from declaration facts.

use regseq_model::declaration::*;

// ---- 1. Types construct and import cleanly ----

#[test]
fn declaration_model_constructs() {
    let model = make_minimal_model();
    assert_eq!(model.driver.driver_name, "pl011");
    assert_eq!(model.build_context.target_os, "linux");
}

#[test]
fn driver_decl_constructs_with_device_class() {
    let driver = DriverDecl {
        driver_name: "pl011".into(),
        compatible: vec!["arm,pl011".into()],
        device_class: DeviceClass::SerialUart,
        provenance: Provenance::new("identity"),
    };
    assert_eq!(driver.device_class, DeviceClass::SerialUart);
    assert_eq!(driver.device_class.as_str(), "serial_uart");
}

#[test]
fn device_class_from_raw_round_trips() {
    assert_eq!(
        DeviceClass::from_raw("serial_uart"),
        DeviceClass::SerialUart
    );
    assert_eq!(DeviceClass::from_raw("network"), DeviceClass::Network);
    assert_eq!(
        DeviceClass::from_raw("custom_thing"),
        DeviceClass::Other("custom_thing".into())
    );
    assert_eq!(DeviceClass::from_raw("serial_uart").as_str(), "serial_uart");
}

// ---- 2. Construct-owned sub-families remain structurally distinct ----

#[test]
fn resource_sub_families_are_distinct_types() {
    let model = make_minimal_model();
    // Each resource sub-family is a separate typed field, not a generic bag.
    let _mmio: &[MmioFieldDecl] = &model.resources.mmio;
    let _clock: &ClockDeclSet = &model.resources.clock;
    let _reset: &ResetDeclSet = &model.resources.reset;
    let _pinctrl: &PinCtrlDeclSet = &model.resources.pinctrl;
    let _interrupts: &InterruptDeclSet = &model.resources.interrupts;
}

#[test]
fn callback_families_are_structurally_separate() {
    let model = make_minimal_model();
    // Each callback family has its own vec, not one polymorphic list.
    assert!(model.callbacks.lifecycle_impls.is_empty());
    assert!(model.callbacks.serial_impls.is_empty());
    assert!(model.callbacks.serial_console_impls.is_empty());
    assert!(model.callbacks.power_management_impls.is_empty());
}

#[test]
fn register_evidence_is_construct_owned() {
    let model = make_model_with_register_evidence();
    // Wrapper and poll call sites live under registers, not a global bag.
    assert_eq!(model.registers.wrapper_call_sites.len(), 1);
    assert_eq!(model.registers.poll_call_sites.len(), 1);
    assert_eq!(
        model.registers.wrapper_call_sites[0].method,
        WrapperMethod::Read
    );
}

// ---- 3. Per-entity provenance fields are present where expected ----

#[test]
fn driver_decl_has_provenance() {
    let driver = DriverDecl {
        driver_name: "pl011".into(),
        compatible: vec![],
        device_class: DeviceClass::SerialUart,
        provenance: Provenance::new("identity")
            .with_source_file("src/lib.rs")
            .with_containing_item("Pl011Driver"),
    };
    assert_eq!(driver.provenance.raw_family, "identity");
    assert_eq!(driver.provenance.source_file.as_deref(), Some("src/lib.rs"));
    assert_eq!(
        driver.provenance.containing_item.as_deref(),
        Some("Pl011Driver")
    );
}

#[test]
fn clock_call_site_retains_containing_fn_provenance() {
    let site = ClockCallSiteDecl {
        method: ClockMethod::Enable,
        containing_fn: "init".into(),
        provenance: Provenance::new("clock").with_containing_item("init"),
    };
    assert_eq!(site.containing_fn, "init");
    assert_eq!(site.provenance.containing_item.as_deref(), Some("init"));
}

#[test]
fn property_field_has_provenance() {
    let field = PropertyFieldDecl {
        driver_type: "Pl011Driver".into(),
        field_name: "fifo_depth".into(),
        field_type: "u32".into(),
        property_id: "serial_uart.fifo_depth".into(),
        default_literal: Some("16".into()),
        vendor_prefix: None,
        required: false,
        provenance: Provenance::new("firmware_properties"),
    };
    assert_eq!(field.provenance.raw_family, "firmware_properties");
}

// ---- 4. Build-context metadata is separate from declaration identity ----

#[test]
fn build_context_separate_from_driver_identity() {
    let model = make_minimal_model();
    // target_os lives in build_context, not in driver identity.
    assert_eq!(model.build_context.target_os, "linux");
    // DriverDecl has no target_os field -- it only has driver_name,
    // compatible, device_class, and provenance.
    assert_eq!(model.driver.driver_name, "pl011");
}

// ---- 5. Interrupt binding-hint / sidecar placement ----

#[test]
fn interrupt_binding_hints_are_separate_from_declaration_facts() {
    let irq = InterruptResourceDecl {
        field_name: "irq".into(),
        interrupt_id: "uart_irq".into(),
        shape: InterruptFieldShape::Single,
        binding_hints: InterruptBindingHints {
            trigger_type: Some("level".into()),
            trigger_polarity: Some("high".into()),
            priority: None,
            shared: Some(false),
        },
        provenance: Provenance::new("interrupts"),
    };
    // binding_hints is a separate struct, not mixed into the declaration fields.
    assert_eq!(irq.binding_hints.trigger_type.as_deref(), Some("level"));
    assert_eq!(irq.interrupt_id, "uart_irq");
}

#[test]
fn interrupt_field_shape_from_raw() {
    assert_eq!(
        InterruptFieldShape::from_raw("single"),
        InterruptFieldShape::Single
    );
    assert_eq!(
        InterruptFieldShape::from_raw("optional"),
        InterruptFieldShape::Optional
    );
    assert_eq!(
        InterruptFieldShape::from_raw("array(4)"),
        InterruptFieldShape::Array(4)
    );
}

#[test]
fn isr_bind_target_from_raw() {
    assert_eq!(IsrBindTarget::from_raw("auto"), IsrBindTarget::Auto);
    assert_eq!(
        IsrBindTarget::from_raw("irq_field"),
        IsrBindTarget::ExplicitField("irq_field".into())
    );
}

// ---- 6. Property-side lints have an explicit home ----

#[test]
fn property_lint_has_explicit_home() {
    let lint = PropertyLintDecl::RequiredWithDefault {
        driver_type: "Pl011Driver".into(),
        field_name: "fifo_depth".into(),
        property_id: "serial_uart.fifo_depth".into(),
        provenance: Provenance::new("firmware_properties"),
    };
    match &lint {
        PropertyLintDecl::RequiredWithDefault {
            property_id,
            provenance,
            ..
        } => {
            assert_eq!(property_id, "serial_uart.fifo_depth");
            assert_eq!(provenance.raw_family, "firmware_properties");
        }
    }
}

// ---- 7. Distinct typed sub-families for sync, delay, logging, PM gate ----

#[test]
fn synchronization_sub_families_are_structurally_distinct() {
    let sync = SynchronizationDeclSet {
        sync_fields: vec![SyncFieldDecl {
            field_name: "lock".into(),
            kind: SyncFieldKind::SpinLock,
            payload_type: None,
            provenance: Provenance::new("synchronization"),
        }],
        lock_operations: vec![LockOperationDecl {
            lock_kind: SyncFieldKind::SpinLock,
            containing_fn: "tx_chars".into(),
            provenance: Provenance::new("synchronization"),
        }],
        completion_operations: vec![CompletionOperationDecl {
            method: CompletionMethod::Wait,
            containing_fn: "wait_for_tx".into(),
            provenance: Provenance::new("synchronization"),
        }],
        guard_scopes: vec![GuardScopeDecl {
            lock_kind: SyncFieldKind::SpinLock,
            containing_fn: "tx_chars".into(),
            provenance: Provenance::new("synchronization"),
        }],
    };
    // All four sub-families are separate vecs with distinct types.
    assert_eq!(sync.sync_fields[0].kind, SyncFieldKind::SpinLock);
    assert_eq!(sync.lock_operations[0].lock_kind, SyncFieldKind::SpinLock);
    assert_eq!(sync.completion_operations[0].method, CompletionMethod::Wait);
    assert_eq!(sync.guard_scopes[0].lock_kind, SyncFieldKind::SpinLock);
}

#[test]
fn sync_field_kind_from_raw() {
    assert_eq!(SyncFieldKind::from_raw("mutex"), Some(SyncFieldKind::Mutex));
    assert_eq!(
        SyncFieldKind::from_raw("spinlock"),
        Some(SyncFieldKind::SpinLock)
    );
    assert_eq!(
        SyncFieldKind::from_raw("irq_spinlock"),
        Some(SyncFieldKind::IrqSpinLock)
    );
    assert_eq!(
        SyncFieldKind::from_raw("completion"),
        Some(SyncFieldKind::Completion)
    );
    assert_eq!(SyncFieldKind::from_raw("unknown"), None);
}

#[test]
fn timed_delay_retains_distinct_typed_fields() {
    let delay = TimedDelayDeclSet {
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
    };
    assert_eq!(delay.call_sites[0].intrinsic, DelayIntrinsic::BusyWait);
    assert_eq!(delay.call_sites[0].path_kind, DelayPathKind::Canonical);
    assert_eq!(delay.duration_ctors[0].ctor, DurationCtor::FromUs);
}

#[test]
fn logging_retains_level_template_arg_shape() {
    let log = LoggingDeclSet {
        call_sites: vec![LogCallSiteDecl {
            level: LogLevel::Info,
            containing_fn: "init".into(),
            canonical: true,
            template: "pl011: initialized at {}".into(),
            arg_count: 1,
            arg_kinds: vec![LogArgKind::U32],
            provenance: Provenance::new("logging"),
        }],
    };
    assert_eq!(log.call_sites[0].level, LogLevel::Info);
    assert_eq!(log.call_sites[0].template, "pl011: initialized at {}");
    assert_eq!(log.call_sites[0].arg_kinds, vec![LogArgKind::U32]);
}

#[test]
fn pm_gate_retains_distinct_properties() {
    let pm = PmGateDeclSet {
        gates: vec![PmGateDecl {
            driver_type: "Pl011Driver".into(),
            properties: PmGateProperties {
                idle: true,
                wakeup_capable: false,
            },
            has_pm_impl: true,
            provenance: Provenance::new("pm_gate"),
        }],
    };
    assert!(pm.gates[0].properties.idle);
    assert!(!pm.gates[0].properties.wakeup_capable);
    assert!(pm.gates[0].has_pm_impl);
}

// ---- Typed enum round-trip tests ----

#[test]
fn callback_context_round_trips() {
    assert_eq!(
        CallbackContext::from_raw("F"),
        Some(CallbackContext::Framework)
    );
    assert_eq!(
        CallbackContext::from_raw("S"),
        Some(CallbackContext::SubsystemSerialized)
    );
    assert_eq!(
        CallbackContext::from_raw("H"),
        Some(CallbackContext::HandlerManaged)
    );
    assert_eq!(CallbackContext::from_raw("X"), None);
    assert_eq!(CallbackContext::Framework.as_str(), "F");
}

#[test]
fn wrapper_method_round_trips() {
    for (raw, expected) in [
        ("read_raw", WrapperMethod::ReadRaw),
        ("read", WrapperMethod::Read),
        ("write", WrapperMethod::Write),
        ("modify", WrapperMethod::Modify),
        ("set", WrapperMethod::Set),
    ] {
        assert_eq!(WrapperMethod::from_raw(raw), Some(expected));
        assert_eq!(expected.as_str(), raw);
    }
    assert_eq!(WrapperMethod::from_raw("unknown"), None);
}

#[test]
fn clock_method_round_trips() {
    assert_eq!(ClockMethod::from_raw("enable"), Some(ClockMethod::Enable));
    assert_eq!(ClockMethod::from_raw("disable"), Some(ClockMethod::Disable));
    assert_eq!(
        ClockMethod::from_raw("get_rate"),
        Some(ClockMethod::GetRate)
    );
    assert_eq!(ClockMethod::Enable.as_str(), "enable");
}

#[test]
fn reset_method_round_trips() {
    assert_eq!(ResetMethod::from_raw("assert"), Some(ResetMethod::Assert));
    assert_eq!(
        ResetMethod::from_raw("deassert"),
        Some(ResetMethod::Deassert)
    );
    assert_eq!(ResetMethod::from_raw("toggle"), Some(ResetMethod::Toggle));
    assert_eq!(ResetMethod::Assert.as_str(), "assert");
}

#[test]
fn completion_method_round_trips() {
    assert_eq!(
        CompletionMethod::from_raw("wait"),
        Some(CompletionMethod::Wait)
    );
    assert_eq!(
        CompletionMethod::from_raw("signal"),
        Some(CompletionMethod::Signal)
    );
    assert_eq!(
        CompletionMethod::from_raw("reset"),
        Some(CompletionMethod::Reset)
    );
    assert_eq!(CompletionMethod::Wait.as_str(), "wait");
}

#[test]
fn delay_intrinsic_round_trips() {
    assert_eq!(
        DelayIntrinsic::from_raw("busy_wait"),
        Some(DelayIntrinsic::BusyWait)
    );
    assert_eq!(
        DelayIntrinsic::from_raw("sleep"),
        Some(DelayIntrinsic::Sleep)
    );
    assert_eq!(DelayIntrinsic::BusyWait.as_str(), "busy_wait");
}

#[test]
fn duration_ctor_round_trips() {
    assert_eq!(
        DurationCtor::from_raw("from_us"),
        Some(DurationCtor::FromUs)
    );
    assert_eq!(
        DurationCtor::from_raw("from_ms"),
        Some(DurationCtor::FromMs)
    );
    assert_eq!(DurationCtor::from_raw("from_s"), Some(DurationCtor::FromS));
    assert_eq!(DurationCtor::FromUs.as_str(), "from_us");
}

#[test]
fn log_level_round_trips() {
    assert_eq!(LogLevel::from_raw("error"), Some(LogLevel::Error));
    assert_eq!(LogLevel::from_raw("warn"), Some(LogLevel::Warn));
    assert_eq!(LogLevel::from_raw("info"), Some(LogLevel::Info));
    assert_eq!(LogLevel::from_raw("debug"), Some(LogLevel::Debug));
    assert_eq!(LogLevel::Error.as_str(), "error");
}

#[test]
fn log_arg_kind_round_trips() {
    for (raw, expected) in [
        ("u8", LogArgKind::U8),
        ("u32", LogArgKind::U32),
        ("bool", LogArgKind::Bool),
        ("str", LogArgKind::Str),
        ("ptr", LogArgKind::Ptr),
    ] {
        assert_eq!(LogArgKind::from_raw(raw), Some(expected));
        assert_eq!(expected.as_str(), raw);
    }
}

// ---- Serialization round-trip ----

#[test]
fn declaration_model_serializes_to_json() {
    let model = make_minimal_model();
    let json = serde_json::to_string(&model).expect("serialize");
    let round_tripped: DeclarationModel = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(model, round_tripped);
}

// ---- Helpers ----

fn make_minimal_model() -> DeclarationModel {
    DeclarationModel {
        driver: DriverDecl {
            driver_name: "pl011".into(),
            compatible: vec!["arm,pl011".into()],
            device_class: DeviceClass::SerialUart,
            provenance: Provenance::new("identity"),
        },
        resources: ResourceDeclSet {
            mmio: vec![],
            interrupts: InterruptDeclSet {
                resources: vec![],
                isr_bindings: vec![],
            },
            clock: ClockDeclSet {
                fields: vec![],
                call_sites: vec![],
            },
            reset: ResetDeclSet {
                fields: vec![],
                call_sites: vec![],
            },
            pinctrl: PinCtrlDeclSet {
                fields: vec![],
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
            fields: vec![],
            lints: vec![],
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

fn make_model_with_register_evidence() -> DeclarationModel {
    let mut model = make_minimal_model();
    model
        .registers
        .wrapper_call_sites
        .push(WrapperCallSiteDecl {
            method: WrapperMethod::Read,
            containing_fn: "rx_chars".into(),
            provenance: Provenance::new("register_access").with_containing_item("rx_chars"),
        });
    model.registers.poll_call_sites.push(PollCallSiteDecl {
        containing_fn: "wait_for_tx_empty".into(),
        provenance: Provenance::new("register_access").with_containing_item("wait_for_tx_empty"),
    });
    model
}
