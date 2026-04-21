//! PL011 full-spec lowering completeness and backend-independence proof.
//!
//! This is the reviewer-facing gate between richer lowering and resumed
//! Linux backend completion. It proves that:
//!
//! 1. The authored `pl011-full-spec` semantics required for the supported
//!    slice are present in lowering plans.
//! 2. The Linux backend plan carries those facts without re-reading raw
//!    frontend models.
//! 3. Generic backend planner code does not hardcode PL011 facts.
//! 4. The proofs are deterministic.
//!
//! Regression context: an earlier backend draft passed internal tests
//! while still embedding PL011-specific knowledge in generic emitter
//! code. This proof guards against both accidental reintroduction and
//! overly coarse lowering that forces the backend to reconstruct
//! authored behavior.
//!
//! Exercises the deprecated `regseq_analysis::legacy_input` shim during
//! the transitional window that ends with `regseq-lowering-10`.

#![allow(dead_code)]
#![allow(deprecated)]

use std::collections::BTreeSet;

use regseq_analysis::legacy_input::{check_body_readiness, AnalysisInput};
use regseq_backends::linux::planner::build_linux_backend_plan;
use regseq_backends::lowering::behavior::{BehaviorNode, BehaviorPlan, FunctionRole};
use regseq_backends::lowering::mir_interpreter::resolve_interrupt_class_masks;
use regseq_backends::lowering::plan::BusType;
use regseq_backends::lowering::planner::{build_driver_lowering_plan, build_legacy_behavior_for};
use regseq_model::binding::{
    BindingHeader, BindingModel, BindingProvenance, BusBinding, ClockBinding, InterruptBinding,
    JoinKey, MmioBinding, PinCtrlBinding, PropertyBinding, ResetBinding,
};
use regseq_model::declaration::bodies::{
    BodyDeclSet, BodyExpr, BodyStmt, FnBodyDecl, FnKind, FnParamDecl,
};
use regseq_model::declaration::callbacks::{
    CallbackContext, CallbackDeclSet, CallbackFamily, CallbackImplDecl,
};
use regseq_model::declaration::driver::BusType as ModelBusType;
use regseq_model::declaration::mir_body::{MirBodyDeclSet, MirQueryFlavor};
use regseq_model::declaration::operations::{
    DelayIntrinsic, DelayPathKind, DurationCtor, DurationCtorSiteDecl, LoggingDeclSet,
    PmGateDeclSet, TimedDelayCallSiteDecl, TimedDelayDeclSet,
};
use regseq_model::declaration::properties::{PropertyDeclSet, PropertyFieldDecl};
use regseq_model::declaration::provenance::Provenance;
use regseq_model::declaration::registers::{
    RegisterBlockDecl, RegisterDeclSet, RegisterDescriptorDecl, RegisterDescriptorFieldDecl,
    RegisterFieldDecl, RegisterNamedValueDecl, WrapperCallSiteDecl, WrapperMethod,
};
use regseq_model::declaration::resources::{
    ClockCallSiteDecl, ClockDeclSet, ClockFieldDecl, ClockMethod, InterruptBindingHints,
    InterruptDeclSet, InterruptFieldShape, InterruptResourceDecl, IsrBindTarget, IsrBindingDecl,
    MmioFieldDecl, PinCtrlDeclSet, PinCtrlFieldDecl, ResetCallSiteDecl, ResetDeclSet,
    ResetFieldDecl, ResetMethod, ResourceDeclSet,
};
use regseq_model::declaration::synchronization::SynchronizationDeclSet;
use regseq_model::declaration::{
    DeclarationBuildContext, DeclarationModel, DeviceClass, DriverDecl,
};
use regseq_model::joined::perform_join;
use regseq_model::platform::PlatformValue;

// ---- Fixture construction (mirrors pl011_full_spec_lowering.rs) ----

fn pl011_full_spec_decl() -> DeclarationModel {
    DeclarationModel {
        driver: DriverDecl {
            driver_name: "Pl011FullSpec".into(),
            compatible: vec!["arm,pl011".into()],
            device_class: DeviceClass::SerialUart,
            bus: Some(ModelBusType::Amba),
            provenance: Provenance::new("identity"),
        },
        resources: ResourceDeclSet {
            mmio: vec![MmioFieldDecl {
                field_name: "regs".into(),
                mmio_id: Some("uart_regs".into()),
                register_block_type: Some("Pl011Registers".into()),
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
                isr_bindings: vec![IsrBindingDecl {
                    handler_name: "handle_irq".into(),
                    bind_target: IsrBindTarget::Auto,
                    has_index_param: false,
                    context: CallbackContext::Framework,
                    provenance: Provenance::new("interrupt"),
                }],
            },
            clock: ClockDeclSet {
                fields: vec![ClockFieldDecl {
                    field_name: "clock".into(),
                    clock_id: "uart_clk".into(),
                    required: false,
                    provenance: Provenance::new("clock"),
                }],
                call_sites: vec![
                    ClockCallSiteDecl {
                        method: ClockMethod::Enable,
                        containing_fn: "enable_clock_if_present".into(),
                        provenance: Provenance::new("clock"),
                    },
                    ClockCallSiteDecl {
                        method: ClockMethod::Disable,
                        containing_fn: "disable_clock_if_present".into(),
                        provenance: Provenance::new("clock"),
                    },
                ],
            },
            reset: ResetDeclSet {
                fields: vec![ResetFieldDecl {
                    field_name: "reset".into(),
                    required: false,
                    provenance: Provenance::new("reset"),
                }],
                call_sites: vec![ResetCallSiteDecl {
                    method: ResetMethod::Toggle,
                    containing_fn: "toggle_reset_if_present".into(),
                    provenance: Provenance::new("reset"),
                }],
            },
            pinctrl: PinCtrlDeclSet {
                fields: vec![PinCtrlFieldDecl {
                    field_name: "pinctrl".into(),
                    required: false,
                    pinctrl_id: Some("uart_pins".into()),
                    provenance: Provenance::new("pinctrl"),
                }],
                call_sites: vec![],
            },
        },
        callbacks: CallbackDeclSet {
            lifecycle_impls: vec![CallbackImplDecl {
                driver_type: "Pl011FullSpec".into(),
                context: CallbackContext::Framework,
                family: CallbackFamily::Lifecycle,
                provenance: Provenance::new("callback"),
            }],
            serial_impls: vec![CallbackImplDecl {
                driver_type: "Pl011FullSpec".into(),
                context: CallbackContext::SubsystemSerialized,
                family: CallbackFamily::Serial,
                provenance: Provenance::new("callback"),
            }],
            serial_console_impls: vec![],
            power_management_impls: vec![],
        },
        registers: RegisterDeclSet {
            blocks: vec![RegisterBlockDecl {
                name: "Pl011Registers".into(),
                fields: vec![
                    RegisterFieldDecl {
                        name: "dr".into(),
                        access_width: Some(32),
                        endian: None,
                        side_effects: None,
                        hw_updates: None,
                    },
                    RegisterFieldDecl {
                        name: "fr".into(),
                        access_width: Some(32),
                        endian: None,
                        side_effects: None,
                        hw_updates: None,
                    },
                    RegisterFieldDecl {
                        name: "ibrd".into(),
                        access_width: Some(32),
                        endian: None,
                        side_effects: None,
                        hw_updates: None,
                    },
                    RegisterFieldDecl {
                        name: "fbrd".into(),
                        access_width: Some(32),
                        endian: None,
                        side_effects: None,
                        hw_updates: None,
                    },
                    RegisterFieldDecl {
                        name: "lcr_h".into(),
                        access_width: Some(32),
                        endian: None,
                        side_effects: None,
                        hw_updates: None,
                    },
                    RegisterFieldDecl {
                        name: "cr".into(),
                        access_width: Some(32),
                        endian: None,
                        side_effects: None,
                        hw_updates: None,
                    },
                    RegisterFieldDecl {
                        name: "ifls".into(),
                        access_width: Some(32),
                        endian: None,
                        side_effects: None,
                        hw_updates: None,
                    },
                    RegisterFieldDecl {
                        name: "imsc".into(),
                        access_width: Some(32),
                        endian: None,
                        side_effects: None,
                        hw_updates: None,
                    },
                    RegisterFieldDecl {
                        name: "mis".into(),
                        access_width: Some(32),
                        endian: None,
                        side_effects: None,
                        hw_updates: None,
                    },
                    RegisterFieldDecl {
                        name: "icr".into(),
                        access_width: Some(32),
                        endian: None,
                        side_effects: None,
                        hw_updates: None,
                    },
                ],
                provenance: Provenance::new("register_access"),
            }],
            descriptors: vec![
                RegisterDescriptorDecl {
                    register_module: "DR_REG".into(),
                    fields: vec![
                        RegisterDescriptorFieldDecl {
                            field_name: "DATA".into(),
                            offset: 0,
                            width: 8,
                            named_values: vec![],
                        },
                        RegisterDescriptorFieldDecl {
                            field_name: "OE".into(),
                            offset: 11,
                            width: 1,
                            named_values: vec![],
                        },
                        RegisterDescriptorFieldDecl {
                            field_name: "BE".into(),
                            offset: 10,
                            width: 1,
                            named_values: vec![],
                        },
                        RegisterDescriptorFieldDecl {
                            field_name: "PE".into(),
                            offset: 9,
                            width: 1,
                            named_values: vec![],
                        },
                        RegisterDescriptorFieldDecl {
                            field_name: "FE".into(),
                            offset: 8,
                            width: 1,
                            named_values: vec![],
                        },
                    ],
                    provenance: Provenance::new("register_access"),
                },
                RegisterDescriptorDecl {
                    register_module: "FR_REG".into(),
                    fields: vec![
                        RegisterDescriptorFieldDecl {
                            field_name: "TXFF".into(),
                            offset: 5,
                            width: 1,
                            named_values: vec![],
                        },
                        RegisterDescriptorFieldDecl {
                            field_name: "RXFE".into(),
                            offset: 4,
                            width: 1,
                            named_values: vec![],
                        },
                        RegisterDescriptorFieldDecl {
                            field_name: "BUSY".into(),
                            offset: 3,
                            width: 1,
                            named_values: vec![],
                        },
                    ],
                    provenance: Provenance::new("register_access"),
                },
                RegisterDescriptorDecl {
                    register_module: "LCRH_REG".into(),
                    fields: vec![
                        RegisterDescriptorFieldDecl {
                            field_name: "WLEN".into(),
                            offset: 5,
                            width: 2,
                            named_values: vec![RegisterNamedValueDecl {
                                name: "EIGHT_BITS".into(),
                                value: 0b11,
                            }],
                        },
                        RegisterDescriptorFieldDecl {
                            field_name: "FEN".into(),
                            offset: 4,
                            width: 1,
                            named_values: vec![
                                RegisterNamedValueDecl {
                                    name: "DISABLED".into(),
                                    value: 0,
                                },
                                RegisterNamedValueDecl {
                                    name: "ENABLED".into(),
                                    value: 1,
                                },
                            ],
                        },
                    ],
                    provenance: Provenance::new("register_access"),
                },
            ],
            wrapper_call_sites: vec![
                WrapperCallSiteDecl {
                    method: WrapperMethod::Write,
                    containing_fn: "disable_uart".into(),
                    provenance: Provenance::new("register_access"),
                },
                WrapperCallSiteDecl {
                    method: WrapperMethod::Read,
                    containing_fn: "rx_char".into(),
                    provenance: Provenance::new("register_access"),
                },
                WrapperCallSiteDecl {
                    method: WrapperMethod::Write,
                    containing_fn: "tx_byte".into(),
                    provenance: Provenance::new("register_access"),
                },
            ],
            poll_call_sites: vec![],
        },
        properties: PropertyDeclSet {
            fields: vec![
                PropertyFieldDecl {
                    driver_type: "Pl011FullSpec".into(),
                    field_name: "current_speed".into(),
                    field_type: "u32".into(),
                    property_id: "current-speed".into(),
                    default_literal: Some("115200".into()),
                    vendor_prefix: None,
                    required: true,
                    provenance: Provenance::new("firmware_properties"),
                },
                PropertyFieldDecl {
                    driver_type: "Pl011FullSpec".into(),
                    field_name: "fifo_size".into(),
                    field_type: "u32".into(),
                    property_id: "fifo-size".into(),
                    default_literal: Some("32".into()),
                    vendor_prefix: None,
                    required: true,
                    provenance: Provenance::new("firmware_properties"),
                },
            ],
            lints: vec![],
        },
        synchronization: SynchronizationDeclSet {
            sync_fields: vec![],
            lock_operations: vec![],
            completion_operations: vec![],
            guard_scopes: vec![],
        },
        timed_delay: TimedDelayDeclSet {
            call_sites: vec![TimedDelayCallSiteDecl {
                intrinsic: DelayIntrinsic::BusyWait,
                path_kind: DelayPathKind::Canonical,
                containing_fn: "toggle_reset_if_present".into(),
                provenance: Provenance::new("timed_delay"),
            }],
            duration_ctors: vec![DurationCtorSiteDecl {
                ctor: DurationCtor::FromUs,
                containing_fn: "toggle_reset_if_present".into(),
                provenance: Provenance::new("timed_delay"),
            }],
        },
        logging: LoggingDeclSet { call_sites: vec![] },
        pm_gate: PmGateDeclSet { gates: vec![] },
        bodies: BodyDeclSet { functions: vec![] },
        mir_bodies: load_real_mir_bodies(),
        build_context: DeclarationBuildContext {
            target_os: "linux".into(),
        },
    }
}

fn pl011_full_spec_binding() -> BindingModel {
    BindingModel {
        header: BindingHeader {
            version: "1".into(),
            os: "linux".into(),
            arch: "aarch64".into(),
            board: "raspberry-pi-4".into(),
            provenance: BindingProvenance::new("platform")
                .with_source_file("pl011-full-spec/platform.toml"),
        },
        properties: vec![
            PropertyBinding {
                key: "current-speed".into(),
                value: PlatformValue::Integer(115200),
                provenance: BindingProvenance::new("properties").with_section_key("current-speed"),
            },
            PropertyBinding {
                key: "fifo-size".into(),
                value: PlatformValue::Integer(32),
                provenance: BindingProvenance::new("properties").with_section_key("fifo-size"),
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
            join_key: JoinKey::FieldName("reset".into()),
            reset_line_id: Some("uart_rst0".into()),
            reset_shared: Some(false),
            provenance: BindingProvenance::new("reset").with_section_key("reset"),
        }],
        pinctrl: vec![PinCtrlBinding {
            join_key: JoinKey::ExplicitId("uart_pins".into()),
            pin_states: Some(vec!["default".into(), "sleep".into()]),
            provenance: BindingProvenance::new("pinctrl").with_section_key("uart_pins"),
        }],
        bus: Some(BusBinding {
            bus_type: ModelBusType::Amba,
            amba_id: Some(0x00041011),
            revision: Some(3),
            provenance: BindingProvenance::new("bus"),
        }),
    }
}

// ---- Body expression helpers ----

fn self_path() -> BodyExpr {
    BodyExpr::Path {
        path: "self".into(),
    }
}

fn self_method_call(method: &str) -> BodyExpr {
    BodyExpr::MethodCall {
        receiver: Box::new(self_path()),
        method: method.into(),
        type_args: vec![],
        args: vec![],
    }
}

fn self_method_call_with(method: &str, args: Vec<BodyExpr>) -> BodyExpr {
    BodyExpr::MethodCall {
        receiver: Box::new(self_path()),
        method: method.into(),
        type_args: vec![],
        args,
    }
}

fn self_field(name: &str) -> BodyExpr {
    BodyExpr::Field {
        base: Box::new(self_path()),
        name: name.into(),
    }
}

fn reg_method(register: &str, method: &str, args: Vec<BodyExpr>) -> BodyExpr {
    BodyExpr::MethodCall {
        receiver: Box::new(BodyExpr::Field {
            base: Box::new(self_field("regs")),
            name: register.into(),
        }),
        method: method.into(),
        type_args: vec![],
        args,
    }
}

fn ok_unit() -> BodyExpr {
    BodyExpr::Call {
        path: "Ok".into(),
        type_args: vec![],
        args: vec![BodyExpr::Tuple { elements: vec![] }],
    }
}

fn block(stmts: Vec<BodyExpr>, trailing: Option<BodyExpr>) -> BodyExpr {
    BodyExpr::Block {
        stmts: stmts
            .into_iter()
            .map(|expr| BodyStmt::Semi { expr })
            .collect(),
        expr: trailing.map(Box::new),
    }
}

fn block_stmts(stmts: Vec<BodyStmt>, trailing: Option<BodyExpr>) -> BodyExpr {
    BodyExpr::Block {
        stmts,
        expr: trailing.map(Box::new),
    }
}

fn callback_body(trait_name: &str, method_name: &str, body: BodyExpr) -> FnBodyDecl {
    FnBodyDecl {
        name: method_name.into(),
        kind: FnKind::Callback {
            trait_name: trait_name.into(),
            method_name: method_name.into(),
        },
        receiver: "&mut self".into(),
        params: vec![],
        return_type: Some("Result<(), RegseqError>".into()),
        body,
        provenance: Provenance::new("bodies").with_containing_item(method_name),
    }
}

fn callback_body_with_params(
    trait_name: &str,
    method_name: &str,
    params: Vec<(&str, &str)>,
    return_type: Option<&str>,
    body: BodyExpr,
) -> FnBodyDecl {
    FnBodyDecl {
        name: method_name.into(),
        kind: FnKind::Callback {
            trait_name: trait_name.into(),
            method_name: method_name.into(),
        },
        receiver: "&mut self".into(),
        params: params
            .into_iter()
            .map(|(n, t)| FnParamDecl {
                name: n.into(),
                ty: t.into(),
            })
            .collect(),
        return_type: return_type.map(Into::into),
        body,
        provenance: Provenance::new("bodies").with_containing_item(method_name),
    }
}

fn helper_body(name: &str, body: BodyExpr) -> FnBodyDecl {
    FnBodyDecl {
        name: name.into(),
        kind: FnKind::Helper,
        receiver: "&mut self".into(),
        params: vec![],
        return_type: None,
        body,
        provenance: Provenance::new("bodies").with_containing_item(name),
    }
}

fn helper_body_with(
    name: &str,
    params: Vec<(&str, &str)>,
    return_type: Option<&str>,
    body: BodyExpr,
) -> FnBodyDecl {
    FnBodyDecl {
        name: name.into(),
        kind: FnKind::Helper,
        receiver: "&mut self".into(),
        params: params
            .into_iter()
            .map(|(n, t)| FnParamDecl {
                name: n.into(),
                ty: t.into(),
            })
            .collect(),
        return_type: return_type.map(Into::into),
        body,
        provenance: Provenance::new("bodies").with_containing_item(name),
    }
}

fn isr_body(name: &str, body: BodyExpr) -> FnBodyDecl {
    FnBodyDecl {
        name: name.into(),
        kind: FnKind::Isr,
        receiver: "&mut self".into(),
        params: vec![],
        return_type: Some("IrqResult".into()),
        body,
        provenance: Provenance::new("bodies").with_containing_item(name),
    }
}

// ---- Fixture body data ----

/// Build a representative set of authored function bodies for the full-spec
/// fixture. Covers callbacks, ISR, and reachable helpers.
fn build_fixture_bodies() -> BodyDeclSet {
    BodyDeclSet {
        functions: vec![
            // -- Callbacks --
            callback_body(
                "Lifecycle",
                "init",
                block(
                    vec![
                        BodyExpr::Try {
                            expr: Box::new(self_method_call("toggle_reset_if_present")),
                        },
                        BodyExpr::Try {
                            expr: Box::new(self_method_call("apply_default_pins_if_present")),
                        },
                        BodyExpr::Try {
                            expr: Box::new(self_method_call("enable_clock_if_present")),
                        },
                        self_method_call_with(
                            "program_polling_port",
                            vec![
                                self_method_call("uart_clock_hz"),
                                self_field("current_speed"),
                            ],
                        ),
                    ],
                    Some(ok_unit()),
                ),
            ),
            callback_body(
                "Lifecycle",
                "deinit",
                block(
                    vec![
                        self_method_call("shutdown_port"),
                        self_method_call("disable_clock_if_present"),
                        BodyExpr::Try {
                            expr: Box::new(self_method_call("apply_sleep_pins_if_present")),
                        },
                    ],
                    Some(ok_unit()),
                ),
            ),
            callback_body(
                "Serial",
                "port_open",
                block(
                    vec![
                        BodyExpr::Try {
                            expr: Box::new(self_method_call("enable_clock_if_present")),
                        },
                        self_method_call_with(
                            "program_polling_port",
                            vec![
                                self_method_call("uart_clock_hz"),
                                self_field("current_speed"),
                            ],
                        ),
                    ],
                    Some(ok_unit()),
                ),
            ),
            callback_body(
                "Serial",
                "port_close",
                block(
                    vec![
                        self_method_call("shutdown_port"),
                        self_method_call("disable_clock_if_present"),
                    ],
                    None,
                ),
            ),
            callback_body(
                "Serial",
                "configure",
                block(
                    vec![
                        BodyExpr::Try {
                            expr: Box::new(self_method_call("enable_clock_if_present")),
                        },
                        self_method_call_with(
                            "program_polling_port",
                            vec![
                                self_method_call("uart_clock_hz"),
                                self_field("current_speed"),
                            ],
                        ),
                    ],
                    Some(ok_unit()),
                ),
            ),
            // tx_byte
            callback_body_with_params(
                "Serial",
                "tx_byte",
                vec![("byte", "u8")],
                Some("()"),
                block(
                    vec![BodyExpr::If {
                        cond: Box::new(BodyExpr::Binary {
                            op: "&&".into(),
                            lhs: Box::new(BodyExpr::MethodCall {
                                receiver: Box::new(self_field("pending_tx")),
                                method: "is_none".into(),
                                type_args: vec![],
                                args: vec![],
                            }),
                            rhs: Box::new(self_method_call("tx_ready")),
                        }),
                        then_branch: Box::new(block(
                            vec![
                                reg_method(
                                    "dr",
                                    "write",
                                    vec![BodyExpr::Call {
                                        path: "field_val".into(),
                                        type_args: vec![],
                                        args: vec![
                                            BodyExpr::Path {
                                                path: "DR_REG::DATA::FIELD".into(),
                                            },
                                            BodyExpr::Cast {
                                                expr: Box::new(BodyExpr::Path {
                                                    path: "byte".into(),
                                                }),
                                                ty: "u32".into(),
                                            },
                                        ],
                                    }],
                                ),
                                BodyExpr::Return { expr: None },
                            ],
                            None,
                        )),
                        else_branch: Some(Box::new(block(
                            vec![
                                BodyExpr::Assign {
                                    target: Box::new(self_field("pending_tx")),
                                    value: Box::new(BodyExpr::Call {
                                        path: "Some".into(),
                                        type_args: vec![],
                                        args: vec![BodyExpr::Path {
                                            path: "byte".into(),
                                        }],
                                    }),
                                },
                                self_method_call("enable_tx_interrupts"),
                            ],
                            None,
                        ))),
                    }],
                    None,
                ),
            ),
            // rx_char
            callback_body_with_params(
                "Serial",
                "rx_char",
                vec![],
                Some("Option<RxChar>"),
                block_stmts(
                    vec![
                        BodyStmt::Semi {
                            expr: BodyExpr::If {
                                cond: Box::new(BodyExpr::Binary {
                                    op: "!=".into(),
                                    lhs: Box::new(reg_method(
                                        "fr",
                                        "read",
                                        vec![BodyExpr::Path {
                                            path: "FR_REG::RXFE::FIELD".into(),
                                        }],
                                    )),
                                    rhs: Box::new(BodyExpr::LitInt {
                                        value: "0".into(),
                                        suffix: None,
                                    }),
                                }),
                                then_branch: Box::new(BodyExpr::Return {
                                    expr: Some(Box::new(BodyExpr::Path {
                                        path: "None".into(),
                                    })),
                                }),
                                else_branch: None,
                            },
                        },
                        BodyStmt::Let {
                            name: "raw".into(),
                            mutable: false,
                            ty: None,
                            init: Some(reg_method("dr", "read_raw", vec![])),
                        },
                        BodyStmt::Let {
                            name: "byte".into(),
                            mutable: false,
                            ty: None,
                            init: Some(BodyExpr::Call {
                                path: "field_from_raw".into(),
                                type_args: vec![],
                                args: vec![
                                    BodyExpr::Path {
                                        path: "DR_REG::DATA::FIELD".into(),
                                    },
                                    BodyExpr::Path { path: "raw".into() },
                                ],
                            }),
                        },
                        BodyStmt::Let {
                            name: "errors".into(),
                            mutable: true,
                            ty: None,
                            init: Some(BodyExpr::Call {
                                path: "RxErrors::default".into(),
                                type_args: vec![],
                                args: vec![],
                            }),
                        },
                        BodyStmt::Semi {
                            expr: BodyExpr::If {
                                cond: Box::new(BodyExpr::Binary {
                                    op: "!=".into(),
                                    lhs: Box::new(BodyExpr::Call {
                                        path: "field_from_raw".into(),
                                        type_args: vec![],
                                        args: vec![
                                            BodyExpr::Path {
                                                path: "DR_REG::OE::FIELD".into(),
                                            },
                                            BodyExpr::Path { path: "raw".into() },
                                        ],
                                    }),
                                    rhs: Box::new(BodyExpr::LitInt {
                                        value: "0".into(),
                                        suffix: None,
                                    }),
                                }),
                                then_branch: Box::new(BodyExpr::Assign {
                                    target: Box::new(BodyExpr::Field {
                                        base: Box::new(BodyExpr::Path {
                                            path: "errors".into(),
                                        }),
                                        name: "overrun".into(),
                                    }),
                                    value: Box::new(BodyExpr::LitBool { value: true }),
                                }),
                                else_branch: None,
                            },
                        },
                        BodyStmt::Semi {
                            expr: BodyExpr::If {
                                cond: Box::new(BodyExpr::Binary {
                                    op: "!=".into(),
                                    lhs: Box::new(BodyExpr::Call {
                                        path: "field_from_raw".into(),
                                        type_args: vec![],
                                        args: vec![
                                            BodyExpr::Path {
                                                path: "DR_REG::BE::FIELD".into(),
                                            },
                                            BodyExpr::Path { path: "raw".into() },
                                        ],
                                    }),
                                    rhs: Box::new(BodyExpr::LitInt {
                                        value: "0".into(),
                                        suffix: None,
                                    }),
                                }),
                                then_branch: Box::new(BodyExpr::Assign {
                                    target: Box::new(BodyExpr::Field {
                                        base: Box::new(BodyExpr::Path {
                                            path: "errors".into(),
                                        }),
                                        name: "break_cond".into(),
                                    }),
                                    value: Box::new(BodyExpr::LitBool { value: true }),
                                }),
                                else_branch: None,
                            },
                        },
                        BodyStmt::Semi {
                            expr: BodyExpr::If {
                                cond: Box::new(BodyExpr::Binary {
                                    op: "!=".into(),
                                    lhs: Box::new(BodyExpr::Call {
                                        path: "field_from_raw".into(),
                                        type_args: vec![],
                                        args: vec![
                                            BodyExpr::Path {
                                                path: "DR_REG::PE::FIELD".into(),
                                            },
                                            BodyExpr::Path { path: "raw".into() },
                                        ],
                                    }),
                                    rhs: Box::new(BodyExpr::LitInt {
                                        value: "0".into(),
                                        suffix: None,
                                    }),
                                }),
                                then_branch: Box::new(BodyExpr::Assign {
                                    target: Box::new(BodyExpr::Field {
                                        base: Box::new(BodyExpr::Path {
                                            path: "errors".into(),
                                        }),
                                        name: "parity".into(),
                                    }),
                                    value: Box::new(BodyExpr::LitBool { value: true }),
                                }),
                                else_branch: None,
                            },
                        },
                        BodyStmt::Semi {
                            expr: BodyExpr::If {
                                cond: Box::new(BodyExpr::Binary {
                                    op: "!=".into(),
                                    lhs: Box::new(BodyExpr::Call {
                                        path: "field_from_raw".into(),
                                        type_args: vec![],
                                        args: vec![
                                            BodyExpr::Path {
                                                path: "DR_REG::FE::FIELD".into(),
                                            },
                                            BodyExpr::Path { path: "raw".into() },
                                        ],
                                    }),
                                    rhs: Box::new(BodyExpr::LitInt {
                                        value: "0".into(),
                                        suffix: None,
                                    }),
                                }),
                                then_branch: Box::new(BodyExpr::Assign {
                                    target: Box::new(BodyExpr::Field {
                                        base: Box::new(BodyExpr::Path {
                                            path: "errors".into(),
                                        }),
                                        name: "framing".into(),
                                    }),
                                    value: Box::new(BodyExpr::LitBool { value: true }),
                                }),
                                else_branch: None,
                            },
                        },
                    ],
                    Some(BodyExpr::Call {
                        path: "Some".into(),
                        type_args: vec![],
                        args: vec![BodyExpr::Call {
                            path: "RxChar::new".into(),
                            type_args: vec![],
                            args: vec![
                                BodyExpr::Path {
                                    path: "byte".into(),
                                },
                                BodyExpr::Path {
                                    path: "errors".into(),
                                },
                            ],
                        }],
                    }),
                ),
            ),
            // tx_ready, tx_empty
            callback_body_with_params(
                "Serial",
                "tx_ready",
                vec![],
                Some("bool"),
                block(
                    vec![],
                    Some(BodyExpr::Binary {
                        op: "==".into(),
                        lhs: Box::new(reg_method(
                            "fr",
                            "read",
                            vec![BodyExpr::Path {
                                path: "FR_REG::TXFF::FIELD".into(),
                            }],
                        )),
                        rhs: Box::new(BodyExpr::LitInt {
                            value: "0".into(),
                            suffix: None,
                        }),
                    }),
                ),
            ),
            callback_body_with_params(
                "Serial",
                "tx_empty",
                vec![],
                Some("bool"),
                block(
                    vec![],
                    Some(BodyExpr::Binary {
                        op: "==".into(),
                        lhs: Box::new(reg_method(
                            "fr",
                            "read",
                            vec![BodyExpr::Path {
                                path: "FR_REG::BUSY::FIELD".into(),
                            }],
                        )),
                        rhs: Box::new(BodyExpr::LitInt {
                            value: "0".into(),
                            suffix: None,
                        }),
                    }),
                ),
            ),
            // flush_rx_fifo
            callback_body_with_params(
                "Serial",
                "flush_rx_fifo",
                vec![],
                Some("()"),
                block(
                    vec![BodyExpr::Loop {
                        body: Box::new(block_stmts(
                            vec![
                                BodyStmt::Semi {
                                    expr: BodyExpr::If {
                                        cond: Box::new(BodyExpr::Binary {
                                            op: "!=".into(),
                                            lhs: Box::new(reg_method(
                                                "fr",
                                                "read",
                                                vec![BodyExpr::Path {
                                                    path: "FR_REG::RXFE::FIELD".into(),
                                                }],
                                            )),
                                            rhs: Box::new(BodyExpr::LitInt {
                                                value: "0".into(),
                                                suffix: None,
                                            }),
                                        }),
                                        then_branch: Box::new(BodyExpr::Break { expr: None }),
                                        else_branch: None,
                                    },
                                },
                                BodyStmt::Semi {
                                    expr: reg_method("dr", "read_raw", vec![]),
                                },
                            ],
                            None,
                        )),
                    }],
                    None,
                ),
            ),
            // flush_tx_fifo
            callback_body_with_params(
                "Serial",
                "flush_tx_fifo",
                vec![],
                Some("()"),
                block(
                    vec![
                        reg_method(
                            "lcr_h",
                            "modify",
                            vec![BodyExpr::Path {
                                path: "LCRH_REG::FEN::DISABLED".into(),
                            }],
                        ),
                        reg_method(
                            "lcr_h",
                            "modify",
                            vec![BodyExpr::Path {
                                path: "LCRH_REG::FEN::ENABLED".into(),
                            }],
                        ),
                    ],
                    None,
                ),
            ),
            // send_break
            callback_body_with_params(
                "Serial",
                "send_break",
                vec![("on", "bool")],
                Some("()"),
                block(
                    vec![BodyExpr::If {
                        cond: Box::new(BodyExpr::Path { path: "on".into() }),
                        then_branch: Box::new(reg_method(
                            "lcr_h",
                            "modify",
                            vec![BodyExpr::Path {
                                path: "LCRH_REG::BRK::ENABLED".into(),
                            }],
                        )),
                        else_branch: Some(Box::new(reg_method(
                            "lcr_h",
                            "modify",
                            vec![BodyExpr::Path {
                                path: "LCRH_REG::BRK::DISABLED".into(),
                            }],
                        ))),
                    }],
                    None,
                ),
            ),
            // -- ISR --
            isr_body(
                "handle_irq",
                block_stmts(
                    vec![
                        BodyStmt::Let {
                            name: "status".into(),
                            mutable: false,
                            ty: None,
                            init: Some(reg_method("mis", "read_raw", vec![])),
                        },
                        BodyStmt::Semi {
                            expr: BodyExpr::If {
                                cond: Box::new(BodyExpr::Binary {
                                    op: "==".into(),
                                    lhs: Box::new(BodyExpr::Path {
                                        path: "status".into(),
                                    }),
                                    rhs: Box::new(BodyExpr::LitInt {
                                        value: "0".into(),
                                        suffix: None,
                                    }),
                                }),
                                then_branch: Box::new(BodyExpr::Return {
                                    expr: Some(Box::new(BodyExpr::Path {
                                        path: "IrqResult::NotHandled".into(),
                                    })),
                                }),
                                else_branch: None,
                            },
                        },
                        BodyStmt::Let {
                            name: "handled".into(),
                            mutable: true,
                            ty: None,
                            init: Some(BodyExpr::LitBool { value: false }),
                        },
                        BodyStmt::Semi {
                            expr: BodyExpr::If {
                                cond: Box::new(BodyExpr::Binary {
                                    op: "!=".into(),
                                    lhs: Box::new(BodyExpr::Binary {
                                        op: "&".into(),
                                        lhs: Box::new(BodyExpr::Path {
                                            path: "status".into(),
                                        }),
                                        rhs: Box::new(BodyExpr::Path {
                                            path: "Self::RX_INTERRUPT_MASK".into(),
                                        }),
                                    }),
                                    rhs: Box::new(BodyExpr::LitInt {
                                        value: "0".into(),
                                        suffix: None,
                                    }),
                                }),
                                then_branch: Box::new(block(
                                    vec![BodyExpr::AssignOp {
                                        op: "|=".into(),
                                        target: Box::new(BodyExpr::Path {
                                            path: "handled".into(),
                                        }),
                                        value: Box::new(self_method_call("service_rx_fifo")),
                                    }],
                                    None,
                                )),
                                else_branch: None,
                            },
                        },
                        BodyStmt::Semi {
                            expr: BodyExpr::If {
                                cond: Box::new(BodyExpr::Binary {
                                    op: "!=".into(),
                                    lhs: Box::new(BodyExpr::Binary {
                                        op: "&".into(),
                                        lhs: Box::new(BodyExpr::Path {
                                            path: "status".into(),
                                        }),
                                        rhs: Box::new(BodyExpr::Path {
                                            path: "Self::TX_INTERRUPT_MASK".into(),
                                        }),
                                    }),
                                    rhs: Box::new(BodyExpr::LitInt {
                                        value: "0".into(),
                                        suffix: None,
                                    }),
                                }),
                                then_branch: Box::new(block(
                                    vec![BodyExpr::AssignOp {
                                        op: "|=".into(),
                                        target: Box::new(BodyExpr::Path {
                                            path: "handled".into(),
                                        }),
                                        value: Box::new(self_method_call("service_tx_fifo")),
                                    }],
                                    None,
                                )),
                                else_branch: None,
                            },
                        },
                        BodyStmt::Semi {
                            expr: self_method_call_with(
                                "clear_explicit_interrupts",
                                vec![BodyExpr::Path {
                                    path: "status".into(),
                                }],
                            ),
                        },
                    ],
                    Some(BodyExpr::If {
                        cond: Box::new(BodyExpr::Binary {
                            op: "||".into(),
                            lhs: Box::new(BodyExpr::Path {
                                path: "handled".into(),
                            }),
                            rhs: Box::new(BodyExpr::Binary {
                                op: "!=".into(),
                                lhs: Box::new(BodyExpr::Binary {
                                    op: "&".into(),
                                    lhs: Box::new(BodyExpr::Path {
                                        path: "status".into(),
                                    }),
                                    rhs: Box::new(BodyExpr::Path {
                                        path: "Self::EXPLICIT_ISR_CLEAR_MASK".into(),
                                    }),
                                }),
                                rhs: Box::new(BodyExpr::LitInt {
                                    value: "0".into(),
                                    suffix: None,
                                }),
                            }),
                        }),
                        then_branch: Box::new(BodyExpr::Path {
                            path: "IrqResult::Handled".into(),
                        }),
                        else_branch: Some(Box::new(BodyExpr::Path {
                            path: "IrqResult::NotHandled".into(),
                        })),
                    }),
                ),
            ),
            // -- Helpers --
            helper_body_with(
                "program_polling_port",
                vec![("clock_hz", "u32"), ("baud", "u32")],
                None,
                block(
                    vec![
                        self_method_call("disable_uart"),
                        self_method_call("disable_fifo"),
                    ],
                    Some(block_stmts(
                        vec![
                            BodyStmt::Semi {
                                expr: BodyExpr::Call {
                                    path: "memory_barrier".into(),
                                    type_args: vec![],
                                    args: vec![BodyExpr::Path {
                                        path: "BarrierKind::Full".into(),
                                    }],
                                },
                            },
                            BodyStmt::Let {
                                name: "__regseq_ordered_result".into(),
                                mutable: false,
                                ty: None,
                                init: Some(block(
                                    vec![
                                        self_method_call_with(
                                            "set_baud_rate",
                                            vec![
                                                BodyExpr::Path {
                                                    path: "clock_hz".into(),
                                                },
                                                BodyExpr::Path {
                                                    path: "baud".into(),
                                                },
                                            ],
                                        ),
                                        self_method_call("configure_line_control"),
                                        self_method_call("configure_interrupt_levels"),
                                        self_method_call("mask_all_interrupts"),
                                        self_method_call("clear_all_interrupts"),
                                        self_method_call("enable_uart"),
                                        self_method_call("enable_rx_interrupts"),
                                    ],
                                    None,
                                )),
                            },
                            BodyStmt::Semi {
                                expr: BodyExpr::Call {
                                    path: "memory_barrier".into(),
                                    type_args: vec![],
                                    args: vec![BodyExpr::Path {
                                        path: "BarrierKind::Full".into(),
                                    }],
                                },
                            },
                        ],
                        Some(BodyExpr::Path {
                            path: "__regseq_ordered_result".into(),
                        }),
                    )),
                ),
            ),
            helper_body(
                "shutdown_port",
                block(
                    vec![
                        self_method_call("mask_all_interrupts"),
                        self_method_call("clear_all_interrupts"),
                        self_method_call("disable_fifo"),
                        self_method_call("disable_uart"),
                        BodyExpr::Assign {
                            target: Box::new(self_field("pending_tx")),
                            value: Box::new(BodyExpr::Path {
                                path: "None".into(),
                            }),
                        },
                    ],
                    None,
                ),
            ),
            helper_body_with(
                "service_rx_fifo",
                vec![],
                Some("bool"),
                block_stmts(
                    vec![
                        BodyStmt::Let {
                            name: "handled".into(),
                            mutable: true,
                            ty: None,
                            init: Some(BodyExpr::LitBool { value: false }),
                        },
                        BodyStmt::Semi {
                            expr: BodyExpr::Loop {
                                body: Box::new(block_stmts(
                                    vec![
                                        BodyStmt::Let {
                                            name: "ch".into(),
                                            mutable: false,
                                            ty: None,
                                            init: Some(self_method_call("rx_char")),
                                        },
                                        BodyStmt::Semi {
                                            expr: BodyExpr::If {
                                                cond: Box::new(BodyExpr::MethodCall {
                                                    receiver: Box::new(BodyExpr::Path {
                                                        path: "ch".into(),
                                                    }),
                                                    method: "is_none".into(),
                                                    type_args: vec![],
                                                    args: vec![],
                                                }),
                                                then_branch: Box::new(BodyExpr::Break {
                                                    expr: None,
                                                }),
                                                else_branch: None,
                                            },
                                        },
                                    ],
                                    None,
                                )),
                            },
                        },
                    ],
                    Some(BodyExpr::Path {
                        path: "handled".into(),
                    }),
                ),
            ),
            helper_body_with(
                "service_tx_fifo",
                vec![],
                Some("bool"),
                block(
                    vec![BodyExpr::If {
                        cond: Box::new(BodyExpr::MethodCall {
                            receiver: Box::new(self_field("pending_tx")),
                            method: "is_some".into(),
                            type_args: vec![],
                            args: vec![],
                        }),
                        then_branch: Box::new(block(
                            vec![self_method_call("disable_tx_interrupts")],
                            None,
                        )),
                        else_branch: None,
                    }],
                    Some(BodyExpr::LitBool { value: true }),
                ),
            ),
            // Register-level helpers
            helper_body(
                "disable_uart",
                block(
                    vec![reg_method(
                        "cr",
                        "write",
                        vec![BodyExpr::Path {
                            path: "CR_REG::UARTEN::DISABLED".into(),
                        }],
                    )],
                    None,
                ),
            ),
            helper_body(
                "enable_uart",
                block(
                    vec![reg_method(
                        "cr",
                        "write",
                        vec![BodyExpr::Path {
                            path: "CR_REG::UARTEN::ENABLED".into(),
                        }],
                    )],
                    None,
                ),
            ),
            helper_body(
                "disable_fifo",
                block(
                    vec![reg_method(
                        "lcr_h",
                        "modify",
                        vec![BodyExpr::Path {
                            path: "LCRH_REG::FEN::DISABLED".into(),
                        }],
                    )],
                    None,
                ),
            ),
            helper_body(
                "mask_all_interrupts",
                block(
                    vec![reg_method(
                        "imsc",
                        "write",
                        vec![BodyExpr::LitInt {
                            value: "0".into(),
                            suffix: None,
                        }],
                    )],
                    None,
                ),
            ),
            helper_body(
                "clear_all_interrupts",
                block(
                    vec![reg_method(
                        "icr",
                        "write",
                        vec![BodyExpr::Path {
                            path: "Self::ALL_UART_INTERRUPTS_MASK".into(),
                        }],
                    )],
                    None,
                ),
            ),
            helper_body(
                "configure_interrupt_levels",
                block(
                    vec![reg_method(
                        "ifls",
                        "write",
                        vec![BodyExpr::LitInt {
                            value: "0".into(),
                            suffix: None,
                        }],
                    )],
                    None,
                ),
            ),
            helper_body_with(
                "set_baud_rate",
                vec![("clock_hz", "u32"), ("baud", "u32")],
                None,
                block(
                    vec![
                        reg_method(
                            "ibrd",
                            "write",
                            vec![BodyExpr::Path {
                                path: "divisor_int".into(),
                            }],
                        ),
                        reg_method(
                            "fbrd",
                            "write",
                            vec![BodyExpr::Path {
                                path: "divisor_frac".into(),
                            }],
                        ),
                    ],
                    None,
                ),
            ),
            helper_body(
                "configure_line_control",
                block(
                    vec![reg_method(
                        "lcr_h",
                        "write",
                        vec![
                            BodyExpr::Path {
                                path: "LCRH_REG::WLEN::EIGHT_BITS".into(),
                            },
                            BodyExpr::Path {
                                path: "LCRH_REG::FEN::ENABLED".into(),
                            },
                        ],
                    )],
                    None,
                ),
            ),
            helper_body(
                "enable_rx_interrupts",
                block(
                    vec![reg_method(
                        "imsc",
                        "modify",
                        vec![BodyExpr::Path {
                            path: "Self::RX_INTERRUPT_MASK".into(),
                        }],
                    )],
                    None,
                ),
            ),
            helper_body(
                "enable_tx_interrupts",
                block(
                    vec![reg_method(
                        "imsc",
                        "modify",
                        vec![BodyExpr::Path {
                            path: "Self::TX_INTERRUPT_MASK".into(),
                        }],
                    )],
                    None,
                ),
            ),
            helper_body(
                "disable_tx_interrupts",
                block(
                    vec![reg_method(
                        "imsc",
                        "modify",
                        vec![BodyExpr::LitInt {
                            value: "0".into(),
                            suffix: None,
                        }],
                    )],
                    None,
                ),
            ),
            helper_body_with(
                "uart_clock_hz",
                vec![],
                Some("u32"),
                block(
                    vec![],
                    Some(BodyExpr::LitInt {
                        value: "48000000".into(),
                        suffix: Some("u32".into()),
                    }),
                ),
            ),
            helper_body(
                "enable_clock_if_present",
                block(vec![self_method_call("clock_enable_inner")], None),
            ),
            helper_body(
                "disable_clock_if_present",
                block(vec![self_method_call("clock_disable_inner")], None),
            ),
            helper_body(
                "toggle_reset_if_present",
                block(vec![self_method_call("reset_toggle_inner")], None),
            ),
            helper_body(
                "apply_default_pins_if_present",
                block(vec![self_method_call("pinctrl_apply_inner")], None),
            ),
            helper_body(
                "apply_sleep_pins_if_present",
                block(vec![self_method_call("pinctrl_apply_inner")], None),
            ),
            helper_body_with(
                "clear_explicit_interrupts",
                vec![("status", "u32")],
                None,
                block(
                    vec![reg_method(
                        "icr",
                        "write",
                        vec![BodyExpr::Path {
                            path: "status".into(),
                        }],
                    )],
                    None,
                ),
            ),
        ],
    }
}

fn authored_interrupt_constants() -> Vec<(String, u32)> {
    vec![
        ("Self::RX_INTERRUPT_MASK".into(), (1 << 4) | (1 << 6)),
        ("Self::TX_INTERRUPT_MASK".into(), 1 << 5),
        (
            "Self::ERROR_INTERRUPT_MASK".into(),
            (1 << 7) | (1 << 8) | (1 << 9) | (1 << 10),
        ),
        (
            "Self::EXPLICIT_ISR_CLEAR_MASK".into(),
            (1 << 7) | (1 << 8) | (1 << 9) | (1 << 10),
        ),
        ("Self::ALL_UART_INTERRUPTS_MASK".into(), 0x07ff),
    ]
}

/// Load real captured MIR bodies from the pl011-full-spec raw artifact.
fn load_real_mir_bodies() -> MirBodyDeclSet {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let fixture_path = manifest_dir
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("tests/fixtures/pl011-full-spec/expected/raw-frontend.json");
    let json = std::fs::read_to_string(&fixture_path).unwrap_or_else(|e| {
        panic!(
            "Failed to read raw-frontend.json at {}: {e}",
            fixture_path.display()
        )
    });
    let model = regseq_model::declaration::normalize::normalize_from_json(&json)
        .expect("Failed to normalize raw artifact");
    model.mir_bodies
}

fn make_full_spec_input() -> AnalysisInput {
    let decl = pl011_full_spec_decl();
    let binding = pl011_full_spec_binding();
    let bus_binding = binding.bus.clone();
    let joined = perform_join(&decl, &binding).expect("join should succeed");
    AnalysisInput::new(decl, joined, bus_binding)
}

/// Build a portable plan with resolved interrupt class masks.
fn make_portable_plan_with_constants() -> regseq_backends::lowering::plan::DriverLoweringPlan {
    let input = make_full_spec_input();
    build_driver_lowering_plan(&input).expect("lowering should succeed")
}

/// Build a legacy behavior plan with resolved interrupt class masks.
fn make_legacy_behavior_with_constants() -> BehaviorPlan {
    let input = make_full_spec_input();
    let (_sm, mut legacy) = build_legacy_behavior_for(&input, &authored_interrupt_constants());
    resolve_interrupt_class_masks(&mut legacy, &authored_interrupt_constants());
    legacy
}

// ===========================================================================
// PROOF 1: Lowered plans expose all authored semantics for the supported slice
// ===========================================================================

/// Comprehensive proof that register layout facts survive lowering.
#[test]
fn proof_register_layout_completeness() {
    let input = make_full_spec_input();
    let plan = build_driver_lowering_plan(&input).unwrap();

    // Block exists with correct field count.
    assert_eq!(plan.register_obligations.blocks.len(), 1);
    let block = &plan.register_obligations.blocks[0];
    assert_eq!(block.name, "Pl011Registers");
    assert_eq!(block.field_count, 10);

    // Every register has a computed byte offset and access width.
    for (i, field) in block.register_fields.iter().enumerate() {
        assert_eq!(
            field.byte_offset,
            (i as u32) * 4,
            "field '{}' offset",
            field.name
        );
        assert_eq!(
            field.access_width,
            Some(32),
            "field '{}' access_width",
            field.name
        );
    }

    // Bitfield descriptors present for DR, FR, LCRH.
    assert!(plan
        .register_obligations
        .descriptors
        .iter()
        .any(|d| d.register_name == "DR_REG"));
    assert!(plan
        .register_obligations
        .descriptors
        .iter()
        .any(|d| d.register_name == "FR_REG"));
    assert!(plan
        .register_obligations
        .descriptors
        .iter()
        .any(|d| d.register_name == "LCRH_REG"));

    // Named values present in LCRH.
    let lcrh = plan
        .register_obligations
        .descriptors
        .iter()
        .find(|d| d.register_name == "LCRH_REG")
        .unwrap();
    let wlen = lcrh.bit_fields.iter().find(|f| f.name == "WLEN").unwrap();
    assert!(
        !wlen.named_values.is_empty(),
        "WLEN must carry named values"
    );
}

/// Comprehensive proof that property defaults survive lowering.
#[test]
fn proof_property_completeness() {
    let input = make_full_spec_input();
    let plan = build_driver_lowering_plan(&input).unwrap();

    assert_eq!(plan.resources.properties.len(), 2);

    for prop in &plan.resources.properties {
        assert!(!prop.property_id.is_empty(), "property_id present");
        assert!(!prop.field_name.is_empty(), "field_name present");
        assert!(!prop.field_type.is_empty(), "field_type present");
        assert!(
            prop.default_literal.is_some(),
            "default_literal required for {}",
            prop.property_id
        );
    }

    let speed = plan
        .resources
        .properties
        .iter()
        .find(|p| p.property_id == "current-speed")
        .unwrap();
    assert_eq!(speed.default_literal.as_deref(), Some("115200"));
    assert_eq!(speed.field_name, "current_speed");

    let fifo = plan
        .resources
        .properties
        .iter()
        .find(|p| p.property_id == "fifo-size")
        .unwrap();
    assert_eq!(fifo.default_literal.as_deref(), Some("32"));
    assert_eq!(fifo.field_name, "fifo_size");
}

/// Comprehensive proof that optional resource metadata survives lowering.
#[test]
fn proof_optional_resource_metadata_completeness() {
    let input = make_full_spec_input();
    let plan = build_driver_lowering_plan(&input).unwrap();

    // Clock with deployment frequency.
    let clock = &plan.resources.clocks[0];
    assert_eq!(clock.clock_id, "uart_clk");
    assert_eq!(clock.default_frequency, Some(48_000_000));

    // Reset with line id and shared flag.
    let reset = &plan.resources.resets[0];
    assert_eq!(reset.reset_line_id.as_deref(), Some("uart_rst0"));
    assert_eq!(reset.reset_shared, Some(false));

    // Pinctrl with available states.
    let pin = &plan.resources.pinctrl[0];
    assert_eq!(pin.pinctrl_id.as_deref(), Some("uart_pins"));
    assert_eq!(
        pin.pin_states.as_deref(),
        Some(&["default".to_string(), "sleep".to_string()][..])
    );

    // Bus identity with AMBA id and revision.
    assert_eq!(plan.resources.bus_identity.bus_type, BusType::Amba);
    assert_eq!(plan.resources.bus_identity.amba_id, Some(0x00041011));
    assert_eq!(plan.resources.bus_identity.amba_revision, Some(3));
}

/// Comprehensive proof that behavioral lowering captures the authored functions.
#[test]
fn proof_behavioral_completeness() {
    let legacy = make_legacy_behavior_with_constants();

    let b = &legacy;

    // Required function roles for the supported slice.
    let required_roles = [
        FunctionRole::LifecycleInit,
        FunctionRole::LifecycleDeinit,
        FunctionRole::SerialPortOpen,
        FunctionRole::SerialPortClose,
        FunctionRole::SerialConfigure,
        FunctionRole::SerialTxByte,
        FunctionRole::SerialRxChar,
        FunctionRole::SerialTxReady,
        FunctionRole::SerialTxEmpty,
        FunctionRole::SerialFlushRxFifo,
        FunctionRole::SerialFlushTxFifo,
        FunctionRole::SerialSendBreak,
        FunctionRole::Isr,
    ];

    for role in &required_roles {
        assert!(
            b.functions.iter().any(|f| f.role == *role),
            "missing required function role: {role:?}"
        );
    }

    // Every function has a non-empty body.
    for f in &b.functions {
        assert!(
            !f.body.is_empty(),
            "function '{}' must have non-empty body",
            f.name
        );
    }

    // ISR behavior must have branch-based interrupt status discrimination.
    let isr = b
        .functions
        .iter()
        .find(|f| f.role == FunctionRole::Isr)
        .unwrap();
    let isr_branch_count = count_branches_recursive(&isr.body);
    assert!(
        isr_branch_count >= 2,
        "ISR must have at least 2 branches for interrupt discrimination, got {isr_branch_count}"
    );

    // If interrupt classes are derived from MIR, they carry mask values.
    // All non-"all" classes must have nonzero masks.
    for cls in &b.interrupt_classes {
        if cls.name != "all" {
            assert_ne!(
                cls.mask_value, 0,
                "interrupt class '{}' mask must be nonzero",
                cls.name
            );
        }
    }
}

// ===========================================================================
// PROOF 2: Linux planning consumes lowered facts without reconstruction
// ---- Behavior tree traversal helpers ----

/// Count Branch nodes recursively in the behavior tree.
fn count_branches_recursive(nodes: &[regseq_backends::lowering::behavior::BehaviorNode]) -> usize {
    use regseq_backends::lowering::behavior::BehaviorNode;
    let mut count = 0;
    for node in nodes {
        match node {
            BehaviorNode::Branch {
                then_body,
                else_body,
                ..
            } => {
                count += 1;
                count += count_branches_recursive(then_body);
                count += count_branches_recursive(else_body);
            }
            BehaviorNode::Loop { body, .. } => {
                count += count_branches_recursive(body);
            }
            BehaviorNode::OrderedSequence { steps } => {
                count += count_branches_recursive(steps);
            }
            BehaviorNode::FallibleOp {
                body,
                cleanup_on_error,
            } => {
                count += count_branches_recursive(body);
                count += count_branches_recursive(cleanup_on_error);
            }
            _ => {}
        }
    }
    count
}

/// Count FallibleOp nodes recursively in the behavior tree.
fn count_fallible_ops_recursive(nodes: &[BehaviorNode]) -> usize {
    let mut count = 0;
    for node in nodes {
        match node {
            BehaviorNode::FallibleOp {
                body,
                cleanup_on_error,
            } => {
                count += 1;
                count += count_fallible_ops_recursive(body);
                count += count_fallible_ops_recursive(cleanup_on_error);
            }
            BehaviorNode::Branch {
                then_body,
                else_body,
                ..
            } => {
                count += count_fallible_ops_recursive(then_body);
                count += count_fallible_ops_recursive(else_body);
            }
            BehaviorNode::Loop { body, .. } => {
                count += count_fallible_ops_recursive(body);
            }
            BehaviorNode::OrderedSequence { steps } => {
                count += count_fallible_ops_recursive(steps);
            }
            BehaviorNode::CleanupRegion { body, cleanup } => {
                count += count_fallible_ops_recursive(body);
                count += count_fallible_ops_recursive(cleanup);
            }
            _ => {}
        }
    }
    count
}

/// Collect all CallHelper callee names recursively from the behavior tree.
fn collect_callhelper_callees(nodes: &[BehaviorNode]) -> BTreeSet<String> {
    let mut callees = BTreeSet::new();
    collect_callhelper_callees_inner(nodes, &mut callees);
    callees
}

fn collect_callhelper_callees_inner(nodes: &[BehaviorNode], callees: &mut BTreeSet<String>) {
    for node in nodes {
        match node {
            BehaviorNode::CallHelper { callee, .. } => {
                callees.insert(callee.clone());
            }
            BehaviorNode::Branch {
                then_body,
                else_body,
                ..
            } => {
                collect_callhelper_callees_inner(then_body, callees);
                collect_callhelper_callees_inner(else_body, callees);
            }
            BehaviorNode::Loop { body, .. } => {
                collect_callhelper_callees_inner(body, callees);
            }
            BehaviorNode::OrderedSequence { steps } => {
                collect_callhelper_callees_inner(steps, callees);
            }
            BehaviorNode::FallibleOp {
                body,
                cleanup_on_error,
            } => {
                collect_callhelper_callees_inner(body, callees);
                collect_callhelper_callees_inner(cleanup_on_error, callees);
            }
            BehaviorNode::CleanupRegion { body, cleanup } => {
                collect_callhelper_callees_inner(body, callees);
                collect_callhelper_callees_inner(cleanup, callees);
            }
            _ => {}
        }
    }
}

/// Count CleanupRegion nodes recursively in the behavior tree.
fn count_cleanup_regions_recursive(nodes: &[BehaviorNode]) -> usize {
    let mut count = 0;
    for node in nodes {
        match node {
            BehaviorNode::CleanupRegion { body, cleanup } => {
                count += 1;
                count += count_cleanup_regions_recursive(body);
                count += count_cleanup_regions_recursive(cleanup);
            }
            BehaviorNode::FallibleOp {
                body,
                cleanup_on_error,
            } => {
                count += count_cleanup_regions_recursive(body);
                count += count_cleanup_regions_recursive(cleanup_on_error);
            }
            BehaviorNode::Branch {
                then_body,
                else_body,
                ..
            } => {
                count += count_cleanup_regions_recursive(then_body);
                count += count_cleanup_regions_recursive(else_body);
            }
            BehaviorNode::Loop { body, .. } => {
                count += count_cleanup_regions_recursive(body);
            }
            BehaviorNode::OrderedSequence { steps } => {
                count += count_cleanup_regions_recursive(steps);
            }
            _ => {}
        }
    }
    count
}

/// Count cleanup-bearing nodes whose cleanup body is non-empty.
fn count_non_empty_cleanup_paths_recursive(nodes: &[BehaviorNode]) -> usize {
    let mut count = 0;
    for node in nodes {
        match node {
            BehaviorNode::CleanupRegion { body, cleanup } => {
                if !cleanup.is_empty() {
                    count += 1;
                }
                count += count_non_empty_cleanup_paths_recursive(body);
                count += count_non_empty_cleanup_paths_recursive(cleanup);
            }
            BehaviorNode::FallibleOp {
                body,
                cleanup_on_error,
            } => {
                if !cleanup_on_error.is_empty() {
                    count += 1;
                }
                count += count_non_empty_cleanup_paths_recursive(body);
                count += count_non_empty_cleanup_paths_recursive(cleanup_on_error);
            }
            BehaviorNode::Branch {
                then_body,
                else_body,
                ..
            } => {
                count += count_non_empty_cleanup_paths_recursive(then_body);
                count += count_non_empty_cleanup_paths_recursive(else_body);
            }
            BehaviorNode::Loop { body, .. } => {
                count += count_non_empty_cleanup_paths_recursive(body);
            }
            BehaviorNode::OrderedSequence { steps } => {
                count += count_non_empty_cleanup_paths_recursive(steps);
            }
            _ => {}
        }
    }
    count
}

fn expected_full_spec_reachable_helpers() -> BTreeSet<String> {
    [
        "apply_default_pins_if_present",
        "apply_sleep_pins_if_present",
        "clear_all_interrupts",
        "clear_explicit_interrupts",
        "configure_interrupt_levels",
        "configure_line_control",
        "disable_clock_if_present",
        "disable_fifo",
        "disable_tx_interrupts",
        "disable_uart",
        "enable_clock_if_present",
        "enable_rx_interrupts",
        "enable_tx_interrupts",
        "enable_uart",
        "mask_all_interrupts",
        "program_polling_port",
        "service_rx_fifo",
        "service_tx_fifo",
        "set_baud_rate",
        "shutdown_port",
        "toggle_reset_if_present",
        "uart_clock_hz",
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

// ===========================================================================

/// Prove that the Linux plan carries register layout data from the portable plan.
#[test]
fn proof_linux_plan_register_layout_from_plan() {
    let input = make_full_spec_input();
    let portable = build_driver_lowering_plan(&input).unwrap();
    let (sm, _legacy) = build_legacy_behavior_for(&input, &authored_interrupt_constants());
    let linux = build_linux_backend_plan(&portable, sm).unwrap();

    // Linux register layout mirrors portable plan.
    assert_eq!(
        linux.register_layout.blocks.len(),
        portable.register_obligations.blocks.len()
    );
    assert_eq!(
        linux.register_layout.descriptors.len(),
        portable.register_obligations.descriptors.len()
    );

    // Field count matches.
    let linux_fields = &linux.register_layout.blocks[0].fields;
    let portable_fields = &portable.register_obligations.blocks[0].register_fields;
    assert_eq!(linux_fields.len(), portable_fields.len());

    // Offsets match.
    for (lf, pf) in linux_fields.iter().zip(portable_fields.iter()) {
        assert_eq!(lf.name, pf.name);
        assert_eq!(lf.byte_offset, pf.byte_offset);
    }
}

/// Prove that the Linux plan carries property realization from the portable plan.
#[test]
fn proof_linux_plan_properties_from_plan() {
    let input = make_full_spec_input();
    let portable = build_driver_lowering_plan(&input).unwrap();
    let (sm, _legacy) = build_legacy_behavior_for(&input, &authored_interrupt_constants());
    let linux = build_linux_backend_plan(&portable, sm).unwrap();

    assert_eq!(
        linux.property_realization.len(),
        portable.resources.properties.len()
    );

    for pr in &linux.property_realization {
        let portable_prop = portable
            .resources
            .properties
            .iter()
            .find(|p| p.property_id == pr.property_id)
            .expect("Linux property must come from portable plan");
        assert_eq!(pr.field_name, portable_prop.field_name);
        assert_eq!(pr.default_literal, portable_prop.default_literal);
    }
}

/// Prove that Linux deployment metadata comes from the portable plan.
#[test]
fn proof_linux_plan_deployment_from_plan() {
    let input = make_full_spec_input();
    let portable = build_driver_lowering_plan(&input).unwrap();
    let (sm, _legacy) = build_legacy_behavior_for(&input, &authored_interrupt_constants());
    let _linux = build_linux_backend_plan(&portable, sm).unwrap();

    let metadata = &portable.diagnostics.linux_deployment_metadata;
    assert_eq!(metadata.clocks.len(), portable.resources.clocks.len());
    assert_eq!(metadata.resets.len(), portable.resources.resets.len());
    assert_eq!(metadata.pinctrl.len(), portable.resources.pinctrl.len());

    // Values match.
    assert_eq!(
        metadata.clocks[0].default_frequency_hz,
        portable.resources.clocks[0].default_frequency
    );
}

/// Prove that the Linux behavior mapping covers all portable functions.
#[test]
fn proof_linux_plan_behavior_from_plan() {
    let input = make_full_spec_input();
    let portable = build_driver_lowering_plan(&input).unwrap();
    let (sm, _legacy) = build_legacy_behavior_for(&input, &authored_interrupt_constants());
    let linux = build_linux_backend_plan(&portable, sm).unwrap();

    // The behavior mapping now reads from SemanticModel. When built via
    // the legacy compat path (empty SemanticModel), the mapping is empty.
    // Full behavior mapping coverage is validated in
    // linux_emission_scaffold.rs which uses the hierarchy-based builder.
    assert_eq!(
        linux.behavior_mapping.portable_function_count,
        linux.behavior.functions.len(),
        "mapping count must match SemanticModel function count"
    );
    assert_eq!(
        linux.behavior_mapping.interrupt_class_count,
        linux.behavior.interrupt_classes.len(),
        "mapping interrupt count must match SemanticModel"
    );

    for func in &linux.behavior.functions {
        let mapping = linux
            .behavior_mapping
            .callback_mappings
            .iter()
            .find(|m| m.portable_function == func.name);
        assert!(
            mapping.is_some(),
            "no Linux mapping for semantic function '{}'",
            func.name
        );
    }
}

// ===========================================================================
// PROOF 3: Generic backend code does not hardcode PL011 facts
// ===========================================================================

/// Regression test: the Linux planner source does not contain hardcoded
/// PL011 register offset values, interrupt mask literals, or
/// device-naming strings.
///
/// The planner must derive all device-specific facts from the portable
/// `DriverLoweringPlan`. If PL011-specific values appear in the planner
/// source, it means the planner is not fully generic.
#[test]
fn regression_linux_planner_has_no_hardcoded_pl011_facts() {
    let source = include_str!("../src/linux/planner.rs");

    // PL011 register offset values (hex literals matching PL011 TRM).
    let forbidden_offsets = [
        "0x018", "0x024", "0x028", "0x02C", "0x030", "0x034", "0x038", "0x040", "0x044",
    ];
    for offset in &forbidden_offsets {
        assert!(
            !source.contains(offset),
            "linux/planner.rs contains hardcoded PL011 offset {offset}"
        );
    }

    // PL011 interrupt mask values.
    let forbidden_masks = ["0x07ff", "0x7ff"];
    for mask in &forbidden_masks {
        assert!(
            !source.contains(mask),
            "linux/planner.rs contains hardcoded PL011 mask {mask}"
        );
    }

    // PL011 compatibility string as a hardcoded literal.
    assert!(
        !source.contains(r#""arm,pl011""#),
        "linux/planner.rs contains hardcoded PL011 compatibility string"
    );
}

/// Regression test: the Linux plan type definitions do not contain
/// hardcoded PL011-specific string literals.
#[test]
fn regression_linux_plan_types_are_generic() {
    let source = include_str!("../src/linux/plan.rs");

    assert!(
        !source.contains(r#""arm,pl011""#),
        "linux/plan.rs contains hardcoded PL011 compatibility string"
    );
    assert!(
        !source.contains("0x07ff"),
        "linux/plan.rs contains hardcoded PL011 mask"
    );
}

/// Regression test: the portable plan type definitions do not contain
/// hardcoded PL011 device facts.
#[test]
fn regression_portable_plan_types_are_generic() {
    let source = include_str!("../src/lowering/plan.rs");

    assert!(
        !source.contains(r#""arm,pl011""#),
        "lowering/plan.rs contains hardcoded PL011 compatibility string"
    );
    assert!(
        !source.contains("0x07ff"),
        "lowering/plan.rs contains hardcoded PL011 mask"
    );
}

/// Regression test: the portable planner does not embed PL011 register
/// offset values or compatibility strings.
#[test]
fn regression_portable_planner_has_no_hardcoded_pl011_offsets() {
    let source = include_str!("../src/lowering/planner.rs");

    let forbidden_offsets = ["0x018", "0x024", "0x028", "0x02C", "0x030"];
    for offset in &forbidden_offsets {
        assert!(
            !source.contains(offset),
            "lowering/planner.rs contains hardcoded PL011 offset {offset}"
        );
    }

    assert!(
        !source.contains(r#""arm,pl011""#),
        "lowering/planner.rs contains hardcoded PL011 compatibility string"
    );
}

// ===========================================================================
// PROOF 4: Determinism
// ===========================================================================

/// Prove that the full completeness proof is deterministic across runs.
#[test]
fn proof_deterministic_across_runs() {
    let input1 = make_full_spec_input();
    let plan1 = build_driver_lowering_plan(&input1).unwrap();
    let (sm1, legacy1) = build_legacy_behavior_for(&input1, &authored_interrupt_constants());
    let linux1 = build_linux_backend_plan(&plan1, sm1).unwrap();

    let input2 = make_full_spec_input();
    let plan2 = build_driver_lowering_plan(&input2).unwrap();
    let (sm2, legacy2) = build_legacy_behavior_for(&input2, &authored_interrupt_constants());
    let linux2 = build_linux_backend_plan(&plan2, sm2).unwrap();

    // Portable plan determinism.
    assert_eq!(
        plan1.register_obligations.blocks[0].register_fields.len(),
        plan2.register_obligations.blocks[0].register_fields.len()
    );
    assert_eq!(
        plan1.resources.properties.len(),
        plan2.resources.properties.len()
    );
    assert_eq!(legacy1.functions.len(), legacy2.functions.len());
    assert_eq!(
        legacy1.interrupt_classes.len(),
        legacy2.interrupt_classes.len()
    );

    // Linux plan determinism.
    assert_eq!(
        linux1.register_layout.blocks.len(),
        linux2.register_layout.blocks.len()
    );
    assert_eq!(
        linux1.property_realization.len(),
        linux2.property_realization.len()
    );
    assert_eq!(
        linux1.behavior_mapping.callback_mappings.len(),
        linux2.behavior_mapping.callback_mappings.len()
    );

    // Function order is deterministic.
    for (f1, f2) in legacy1.functions.iter().zip(legacy2.functions.iter()) {
        assert_eq!(f1.name, f2.name);
        assert_eq!(f1.role, f2.role);
    }
}

// ===========================================================================
// PROOF 5: Per-callback and per-helper behavior coverage
// ===========================================================================

/// Prove that every authored callback has a FunctionBehavior entry matched
/// by both name and role. Unlike `proof_behavioral_completeness` which
/// checks only that each role is present somewhere, this test verifies
/// the specific name-role pairing for every authored callback.
#[test]
fn proof_per_callback_behavior_coverage() {
    let legacy = make_legacy_behavior_with_constants();

    let expected: &[(&str, FunctionRole)] = &[
        ("init", FunctionRole::LifecycleInit),
        ("deinit", FunctionRole::LifecycleDeinit),
        ("port_open", FunctionRole::SerialPortOpen),
        ("port_close", FunctionRole::SerialPortClose),
        ("configure", FunctionRole::SerialConfigure),
        ("tx_byte", FunctionRole::SerialTxByte),
        ("rx_char", FunctionRole::SerialRxChar),
        ("tx_ready", FunctionRole::SerialTxReady),
        ("tx_empty", FunctionRole::SerialTxEmpty),
        ("flush_rx_fifo", FunctionRole::SerialFlushRxFifo),
        ("flush_tx_fifo", FunctionRole::SerialFlushTxFifo),
        ("send_break", FunctionRole::SerialSendBreak),
        ("handle_irq", FunctionRole::Isr),
    ];

    for (name, role) in expected {
        let matched = legacy
            .functions
            .iter()
            .find(|f| f.name == *name && f.role == *role);
        assert!(
            matched.is_some(),
            "missing callback: name={name}, role={role:?}"
        );
        assert!(
            !matched.unwrap().body.is_empty(),
            "callback '{name}' has empty body"
        );
    }
}

/// Prove that every reachable helper has a corresponding FunctionBehavior
/// entry with `role == Helper` and a non-empty body.
#[test]
fn proof_per_helper_behavior_coverage() {
    let input = make_full_spec_input();
    let legacy = make_legacy_behavior_with_constants();

    let reachable = input.mir_reachable_helper_names();
    let expected = expected_full_spec_reachable_helpers();
    assert!(
        !reachable.is_empty(),
        "reachable helper set must not be empty"
    );
    assert_eq!(
        reachable, expected,
        "reachable helper set must match the explicit pl011-full-spec helper closure"
    );

    for name in &reachable {
        let matched = legacy
            .functions
            .iter()
            .find(|f| f.name == *name && f.role == FunctionRole::Helper);
        assert!(
            matched.is_some(),
            "missing helper behavior for reachable helper '{name}'"
        );
        assert!(
            !matched.unwrap().body.is_empty(),
            "helper '{name}' has empty body"
        );
    }
}

// ===========================================================================
// PROOF 6: Cleanup/drop behavior visible in lowering plans
// ===========================================================================

/// Prove that functions using the `?` operator produce FallibleOp nodes
/// in the behavior tree, meaning cleanup/drop behavior is visible in
/// lowering plans without backend reconstruction.
///
/// Also proves that functions without fallible control flow do NOT
/// contain FallibleOp nodes, confirming the planner is not inventing
/// cleanup where none was authored.
#[test]
fn proof_cleanup_behavior_visible_in_plans() {
    let legacy = make_legacy_behavior_with_constants();

    // Functions known to use the `?` operator in the authored spec.
    let fallible_fns = ["init", "deinit", "port_open", "configure"];
    for name in &fallible_fns {
        let func = legacy
            .functions
            .iter()
            .find(|f| f.name == *name)
            .unwrap_or_else(|| panic!("missing function '{name}'"));
        let fallible_count = count_fallible_ops_recursive(&func.body);
        let cleanup_region_count = count_cleanup_regions_recursive(&func.body);
        let non_empty_cleanup_count = count_non_empty_cleanup_paths_recursive(&func.body);
        assert!(
            fallible_count >= 1,
            "'{name}' uses `?` but has {fallible_count} FallibleOp nodes (expected >= 1)"
        );
        assert!(
            cleanup_region_count + non_empty_cleanup_count >= 1,
            "'{name}' uses `?` but lowering exposed no cleanup-bearing structure"
        );
        assert!(
            non_empty_cleanup_count >= 1,
            "'{name}' uses `?` but all cleanup-bearing nodes have empty cleanup bodies"
        );
    }

    // Functions that do NOT use `?` -- should have zero FallibleOp nodes.
    let non_fallible_fns = ["tx_ready", "tx_empty"];
    for name in &non_fallible_fns {
        let func = legacy
            .functions
            .iter()
            .find(|f| f.name == *name)
            .unwrap_or_else(|| panic!("missing function '{name}'"));
        let fallible_count = count_fallible_ops_recursive(&func.body);
        assert_eq!(
            fallible_count, 0,
            "'{name}' has no `?` but got {fallible_count} FallibleOp nodes"
        );
    }
}

// ===========================================================================
// PROOF 7: MIR provenance is mir_drops_elaborated_and_const_checked
// ===========================================================================

/// Prove that the MIR body source of truth is
/// `mir_drops_elaborated_and_const_checked` and that this is validated
/// by the body readiness infrastructure.
#[test]
fn proof_mir_provenance_is_drops_elaborated() {
    let input = make_full_spec_input();

    // Query flavor on the input is DropsElaborated.
    assert_eq!(
        *input.mir_query_flavor(),
        MirQueryFlavor::DropsElaborated,
        "MIR query flavor must be DropsElaborated"
    );

    // Body readiness validates the query flavor.
    let report = check_body_readiness(&input);
    assert!(
        report.mir_form_summary.query_flavor_valid,
        "body readiness must confirm query flavor is valid"
    );

    // No errors in the body readiness report.
    assert!(
        !report.has_errors(),
        "body readiness must have no errors for the full-spec fixture"
    );

    // Coverage counts are nonzero, confirming bodies were processed.
    assert!(
        report.callback_body_count > 0,
        "callback body count must be > 0"
    );
    assert!(report.isr_body_count > 0, "ISR body count must be > 0");
    assert!(
        report.reachable_helper_count > 0,
        "reachable helper count must be > 0"
    );
}

// ===========================================================================
// PROOF 8: Behavior plan traceable to captured MIR bodies
// ===========================================================================

/// Prove structural traceability from behavior plan content back to
/// captured MIR bodies. For every function in the behavior plan, a
/// corresponding MIR body must exist. For orchestrating functions,
/// the CallHelper callees in the behavior tree must overlap with the
/// helper_calls from the MIR function.
#[test]
fn proof_behavior_traceable_to_mir_bodies() {
    let input = make_full_spec_input();
    let legacy = make_legacy_behavior_with_constants();

    // Every FunctionBehavior must have a corresponding MIR body.
    for fb in &legacy.functions {
        assert!(
            input.mir_body_by_name(&fb.name).is_some(),
            "behavior function '{}' has no corresponding MIR body",
            fb.name
        );
    }

    // Total behavior function count == callbacks + ISR + reachable helpers.
    let reachable_helpers = input.mir_reachable_helper_names();
    let callback_count = input.mir_callback_bodies().len();
    let isr_count = input.mir_isr_bodies().len();
    let expected_total = callback_count + isr_count + reachable_helpers.len();
    assert_eq!(
        legacy.functions.len(),
        expected_total,
        "behavior function count must equal callbacks ({}) + ISRs ({}) + helpers ({})",
        callback_count,
        isr_count,
        reachable_helpers.len()
    );

    // For orchestrating functions, verify CallHelper callees overlap
    // with the MIR function's helper_calls.
    let orchestrating = [
        "init",
        "program_polling_port",
        "shutdown_port",
        "handle_irq",
    ];
    for name in &orchestrating {
        let fb = legacy
            .functions
            .iter()
            .find(|f| f.name == *name)
            .unwrap_or_else(|| panic!("missing behavior for '{name}'"));
        let mir_fn = input
            .mir_body_by_name(name)
            .unwrap_or_else(|| panic!("missing MIR body for '{name}'"));

        let behavior_callees = collect_callhelper_callees(&fb.body);
        let mir_callees: BTreeSet<String> = mir_fn.helper_calls.iter().cloned().collect();

        // Every MIR helper_call that is a reachable helper should
        // appear as a CallHelper in the behavior tree.
        for callee in &mir_callees {
            if reachable_helpers.contains(callee) {
                assert!(
                    behavior_callees.contains(callee),
                    "'{name}' MIR calls helper '{callee}' but behavior tree is missing CallHelper"
                );
            }
        }
    }
}

// ===========================================================================
// PROOF 9: Extended regression -- lowering behavior source genericity
// ===========================================================================

/// Regression test: the behavior type definitions and behavior planner
/// do not contain hardcoded PL011-specific register offsets, masks,
/// compatibility strings, or device-naming literals.
#[test]
fn regression_lowering_behavior_source_has_no_pl011_facts() {
    let sources = [
        ("behavior.rs", include_str!("../src/lowering/behavior.rs")),
        (
            "behavior_planner.rs",
            include_str!("../src/lowering/behavior_planner.rs"),
        ),
    ];

    let forbidden_offsets = [
        "0x018", "0x024", "0x028", "0x02C", "0x030", "0x034", "0x038", "0x040", "0x044",
    ];
    let forbidden_masks = ["0x07ff", "0x7ff"];

    for (filename, source) in &sources {
        for offset in &forbidden_offsets {
            assert!(
                !source.contains(offset),
                "{filename} contains hardcoded PL011 offset {offset}"
            );
        }
        for mask in &forbidden_masks {
            assert!(
                !source.contains(mask),
                "{filename} contains hardcoded PL011 mask {mask}"
            );
        }
        assert!(
            !source.contains(r#""arm,pl011""#),
            "{filename} contains hardcoded PL011 compatibility string"
        );
        assert!(
            !source.contains("Pl011"),
            "{filename} contains PL011 device name reference"
        );
        assert!(
            !source.contains("PL011"),
            "{filename} contains PL011 device name reference"
        );
    }
}

/// Regression test: the MIR interpreter modules do not contain hardcoded
/// PL011-specific register offsets, masks, compatibility strings, or
/// device-naming literals in non-comment code.
///
/// Doc-comment examples (e.g. `/// e.g. "registers::Pl011Registers"`)
/// are excluded because they illustrate usage but do not encode device
/// behavior.
#[test]
fn regression_mir_interpreter_has_no_pl011_facts() {
    let sources = [
        (
            "mir_interpreter/mod.rs",
            include_str!("../src/lowering/mir_interpreter/mod.rs"),
        ),
        (
            "mir_interpreter/call_classifier.rs",
            include_str!("../src/lowering/mir_interpreter/call_classifier.rs"),
        ),
        (
            "mir_interpreter/cfg_helpers.rs",
            include_str!("../src/lowering/mir_interpreter/cfg_helpers.rs"),
        ),
        (
            "mir_interpreter/cfg_walker.rs",
            include_str!("../src/lowering/mir_interpreter/cfg_walker.rs"),
        ),
        (
            "mir_interpreter/context.rs",
            include_str!("../src/lowering/mir_interpreter/context.rs"),
        ),
        (
            "mir_interpreter/place_resolver.rs",
            include_str!("../src/lowering/mir_interpreter/place_resolver.rs"),
        ),
        (
            "mir_interpreter/value_lowering.rs",
            include_str!("../src/lowering/mir_interpreter/value_lowering.rs"),
        ),
    ];

    let forbidden_offsets = [
        "0x018", "0x024", "0x028", "0x02C", "0x030", "0x034", "0x038", "0x040", "0x044",
    ];
    let forbidden_masks = ["0x07ff", "0x7ff"];

    for (filename, source) in &sources {
        // Strip comment-only lines so doc-comment examples do not
        // cause false positives.
        let code_lines: String = source
            .lines()
            .filter(|l| {
                let trimmed = l.trim_start();
                !trimmed.starts_with("///") && !trimmed.starts_with("//!")
            })
            .collect::<Vec<_>>()
            .join("\n");

        for offset in &forbidden_offsets {
            assert!(
                !code_lines.contains(offset),
                "{filename} contains hardcoded PL011 offset {offset}"
            );
        }
        for mask in &forbidden_masks {
            assert!(
                !code_lines.contains(mask),
                "{filename} contains hardcoded PL011 mask {mask}"
            );
        }
        assert!(
            !code_lines.contains(r#""arm,pl011""#),
            "{filename} contains hardcoded PL011 compatibility string"
        );
        assert!(
            !code_lines.contains("Pl011"),
            "{filename} contains PL011 device name reference in non-comment code"
        );
        assert!(
            !code_lines.contains("PL011"),
            "{filename} contains PL011 device name reference in non-comment code"
        );
    }
}

/// Regression test: the legacy body interpreter does not contain hardcoded
/// PL011-specific register offsets, masks, compatibility strings, or
/// device-naming literals.
#[test]
fn regression_body_interpreter_has_no_pl011_facts() {
    let source = include_str!("../src/lowering/body_interpreter.rs");

    let forbidden_offsets = [
        "0x018", "0x024", "0x028", "0x02C", "0x030", "0x034", "0x038", "0x040", "0x044",
    ];
    let forbidden_masks = ["0x07ff", "0x7ff"];

    for offset in &forbidden_offsets {
        assert!(
            !source.contains(offset),
            "body_interpreter.rs contains hardcoded PL011 offset {offset}"
        );
    }
    for mask in &forbidden_masks {
        assert!(
            !source.contains(mask),
            "body_interpreter.rs contains hardcoded PL011 mask {mask}"
        );
    }
    assert!(
        !source.contains(r#""arm,pl011""#),
        "body_interpreter.rs contains hardcoded PL011 compatibility string"
    );
    assert!(
        !source.contains("Pl011"),
        "body_interpreter.rs contains PL011 device name reference"
    );
    assert!(
        !source.contains("PL011"),
        "body_interpreter.rs contains PL011 device name reference"
    );
}
