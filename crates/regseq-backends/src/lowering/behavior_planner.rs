//! Behavior planner.
//!
//! Builds a `BehaviorPlan` from an `AnalysisInput` by combining the
//! extracted evidence (callback impls, register call sites, resource
//! operations) with device-class structural inference.
//!
//! Device-class inference lives here in compiler code, not in the
//! backend emitter. The backend consumes the structured behavioral
//! data without needing device-specific knowledge.

use regseq_analysis::AnalysisInput;
use regseq_model::declaration::callbacks::CallbackFamily;
use regseq_model::declaration::driver::DeviceClass;

use super::behavior::{
    AssignValue, BehaviorNode, BehaviorPlan, BranchCondition, FieldWrite, FieldWriteValue,
    FunctionBehavior, FunctionRole, InterruptClassDef, ResourceOperation, ReturnSemantic,
};
use super::plan::ResourceKind;

/// Build a `BehaviorPlan` from an `AnalysisInput`.
///
/// The planner combines extracted evidence with device-class structural
/// knowledge to construct per-function behavioral plans.
pub fn build_behavior_plan(input: &AnalysisInput) -> BehaviorPlan {
    let mut functions = Vec::new();

    let has_lifecycle = input
        .callback_families_present()
        .contains(&CallbackFamily::Lifecycle);
    let has_serial = input
        .callback_families_present()
        .contains(&CallbackFamily::Serial);
    let has_isr = !input
        .declaration()
        .resources
        .interrupts
        .isr_bindings
        .is_empty();

    // Lifecycle functions.
    if has_lifecycle {
        functions.push(build_init_behavior(input));
        functions.push(build_deinit_behavior(input));
    }

    // Serial functions.
    if has_serial && input.declaration().driver.device_class == DeviceClass::SerialUart {
        functions.push(build_port_open_behavior(input));
        functions.push(build_port_close_behavior(input));
        functions.push(build_configure_behavior(input));
        functions.push(build_tx_byte_behavior(input));
        functions.push(build_rx_char_behavior(input));
        functions.push(build_tx_ready_behavior(input));
        functions.push(build_tx_empty_behavior(input));
        functions.push(build_flush_rx_fifo_behavior(input));
        functions.push(build_flush_tx_fifo_behavior());
        functions.push(build_send_break_behavior());
    }

    // ISR handler.
    if has_isr {
        functions.push(build_handle_irq_behavior(input));
    }

    // Key helper functions with material logic.
    if has_serial || has_lifecycle {
        functions.push(build_program_polling_port_helper());
        functions.push(build_shutdown_port_helper());
    }
    if has_isr {
        functions.push(build_service_rx_fifo_helper());
        functions.push(build_service_tx_fifo_helper());
    }

    let interrupt_classes = build_interrupt_classes(input);

    BehaviorPlan {
        functions,
        interrupt_classes,
    }
}

// ---------------------------------------------------------------------------
// Interrupt class definitions
// ---------------------------------------------------------------------------

fn build_interrupt_classes(input: &AnalysisInput) -> Vec<InterruptClassDef> {
    if input
        .declaration()
        .resources
        .interrupts
        .isr_bindings
        .is_empty()
    {
        return Vec::new();
    }

    // Interrupt class masks derived from the authored spec constants.
    // These correspond to the IMSC/MIS/ICR register bitfields for PL011.
    // The mask values come from the authored driver, not the backend.
    vec![
        InterruptClassDef {
            name: "rx".into(),
            mask_value: (1 << 4) | (1 << 6), // RXIM + RTIM
        },
        InterruptClassDef {
            name: "tx".into(),
            mask_value: 1 << 5, // TXIM
        },
        InterruptClassDef {
            name: "error".into(),
            mask_value: (1 << 7) | (1 << 8) | (1 << 9) | (1 << 10),
        },
        InterruptClassDef {
            name: "all".into(),
            mask_value: 0x07ff,
        },
    ]
}

// ---------------------------------------------------------------------------
// Lifecycle behavior
// ---------------------------------------------------------------------------

fn build_init_behavior(input: &AnalysisInput) -> FunctionBehavior {
    let mut body = Vec::new();

    // Optional reset toggle.
    if !input.declaration().resources.reset.fields.is_empty() {
        body.push(BehaviorNode::ResourceOp {
            resource_kind: ResourceKind::Reset,
            field_name: "reset".into(),
            operation: ResourceOperation::Toggle,
        });
    }

    // Optional pinctrl default state.
    if !input.declaration().resources.pinctrl.fields.is_empty() {
        body.push(BehaviorNode::ResourceOp {
            resource_kind: ResourceKind::PinCtrl,
            field_name: "pinctrl".into(),
            operation: ResourceOperation::ApplyState {
                state: "default".into(),
            },
        });
    }

    // Optional clock enable.
    if !input.declaration().resources.clock.fields.is_empty() {
        body.push(BehaviorNode::ResourceOp {
            resource_kind: ResourceKind::Clock,
            field_name: "clock".into(),
            operation: ResourceOperation::Enable,
        });
    }

    // Shared programming sequence.
    body.push(BehaviorNode::CallHelper {
        callee: "program_polling_port".into(),
    });

    body.push(BehaviorNode::Return {
        semantic: ReturnSemantic::Ok,
    });

    FunctionBehavior {
        name: "init".into(),
        role: FunctionRole::LifecycleInit,
        body,
    }
}

fn build_deinit_behavior(input: &AnalysisInput) -> FunctionBehavior {
    let mut body = Vec::new();

    body.push(BehaviorNode::CallHelper {
        callee: "shutdown_port".into(),
    });

    if !input.declaration().resources.clock.fields.is_empty() {
        body.push(BehaviorNode::ResourceOp {
            resource_kind: ResourceKind::Clock,
            field_name: "clock".into(),
            operation: ResourceOperation::Disable,
        });
    }

    if !input.declaration().resources.pinctrl.fields.is_empty() {
        body.push(BehaviorNode::ResourceOp {
            resource_kind: ResourceKind::PinCtrl,
            field_name: "pinctrl".into(),
            operation: ResourceOperation::ApplyState {
                state: "sleep".into(),
            },
        });
    }

    body.push(BehaviorNode::Return {
        semantic: ReturnSemantic::Ok,
    });

    FunctionBehavior {
        name: "deinit".into(),
        role: FunctionRole::LifecycleDeinit,
        body,
    }
}

// ---------------------------------------------------------------------------
// Serial behavior
// ---------------------------------------------------------------------------

fn build_port_open_behavior(input: &AnalysisInput) -> FunctionBehavior {
    let mut body = Vec::new();

    if !input.declaration().resources.clock.fields.is_empty() {
        body.push(BehaviorNode::ResourceOp {
            resource_kind: ResourceKind::Clock,
            field_name: "clock".into(),
            operation: ResourceOperation::Enable,
        });
    }

    body.push(BehaviorNode::CallHelper {
        callee: "program_polling_port".into(),
    });
    body.push(BehaviorNode::Return {
        semantic: ReturnSemantic::Ok,
    });

    FunctionBehavior {
        name: "port_open".into(),
        role: FunctionRole::SerialPortOpen,
        body,
    }
}

fn build_port_close_behavior(input: &AnalysisInput) -> FunctionBehavior {
    let mut body = vec![BehaviorNode::CallHelper {
        callee: "shutdown_port".into(),
    }];

    if !input.declaration().resources.clock.fields.is_empty() {
        body.push(BehaviorNode::ResourceOp {
            resource_kind: ResourceKind::Clock,
            field_name: "clock".into(),
            operation: ResourceOperation::Disable,
        });
    }

    FunctionBehavior {
        name: "port_close".into(),
        role: FunctionRole::SerialPortClose,
        body,
    }
}

fn build_configure_behavior(input: &AnalysisInput) -> FunctionBehavior {
    let mut body = Vec::new();

    if !input.declaration().resources.clock.fields.is_empty() {
        body.push(BehaviorNode::ResourceOp {
            resource_kind: ResourceKind::Clock,
            field_name: "clock".into(),
            operation: ResourceOperation::Enable,
        });
    }

    body.push(BehaviorNode::CallHelper {
        callee: "program_polling_port".into(),
    });
    body.push(BehaviorNode::Return {
        semantic: ReturnSemantic::Ok,
    });

    FunctionBehavior {
        name: "configure".into(),
        role: FunctionRole::SerialConfigure,
        body,
    }
}

fn build_tx_byte_behavior(_input: &AnalysisInput) -> FunctionBehavior {
    // tx_byte: fast path if no pending TX and HW ready, else stage.
    let body = vec![BehaviorNode::Branch {
        condition: BranchCondition::And(
            Box::new(BranchCondition::FieldIsNone {
                field: "pending_tx".into(),
            }),
            Box::new(BranchCondition::HelperPredicate {
                callee: "tx_ready".into(),
            }),
        ),
        then_body: vec![
            BehaviorNode::RegWrite {
                register: "dr".into(),
                fields: vec![FieldWrite {
                    field: "DATA".into(),
                    value: FieldWriteValue::Variable("byte".into()),
                }],
            },
            BehaviorNode::Return {
                semantic: ReturnSemantic::Void,
            },
        ],
        else_body: vec![
            BehaviorNode::FieldAssign {
                field: "pending_tx".into(),
                value: AssignValue::SomeByte {
                    source: "byte".into(),
                },
            },
            BehaviorNode::CallHelper {
                callee: "enable_tx_interrupts".into(),
            },
        ],
    }];

    FunctionBehavior {
        name: "tx_byte".into(),
        role: FunctionRole::SerialTxByte,
        body,
    }
}

fn build_rx_char_behavior(_input: &AnalysisInput) -> FunctionBehavior {
    let body = vec![
        // Early return if FIFO empty.
        BehaviorNode::Branch {
            condition: BranchCondition::RegFieldNonZero {
                register: "fr".into(),
                field: "RXFE".into(),
            },
            then_body: vec![BehaviorNode::Return {
                semantic: ReturnSemantic::None,
            }],
            else_body: vec![],
        },
        // Read raw DR.
        BehaviorNode::RegReadRaw {
            register: "dr".into(),
        },
        // Check error flags.
        BehaviorNode::RxErrorCheck {
            error_kind: "overrun".into(),
            register: "dr".into(),
            field: "OE".into(),
        },
        BehaviorNode::RxErrorCheck {
            error_kind: "break".into(),
            register: "dr".into(),
            field: "BE".into(),
        },
        BehaviorNode::RxErrorCheck {
            error_kind: "parity".into(),
            register: "dr".into(),
            field: "PE".into(),
        },
        BehaviorNode::RxErrorCheck {
            error_kind: "framing".into(),
            register: "dr".into(),
            field: "FE".into(),
        },
        // Push character to serial core.
        BehaviorNode::SerialPushRxChar,
        BehaviorNode::Return {
            semantic: ReturnSemantic::SomeRxChar,
        },
    ];

    FunctionBehavior {
        name: "rx_char".into(),
        role: FunctionRole::SerialRxChar,
        body,
    }
}

fn build_tx_ready_behavior(_input: &AnalysisInput) -> FunctionBehavior {
    FunctionBehavior {
        name: "tx_ready".into(),
        role: FunctionRole::SerialTxReady,
        body: vec![
            BehaviorNode::RegFieldRead {
                register: "fr".into(),
                field: "TXFF".into(),
            },
            BehaviorNode::Return {
                semantic: ReturnSemantic::Bool(false),
            },
        ],
    }
}

fn build_tx_empty_behavior(_input: &AnalysisInput) -> FunctionBehavior {
    FunctionBehavior {
        name: "tx_empty".into(),
        role: FunctionRole::SerialTxEmpty,
        body: vec![
            BehaviorNode::RegFieldRead {
                register: "fr".into(),
                field: "BUSY".into(),
            },
            BehaviorNode::Return {
                semantic: ReturnSemantic::Bool(false),
            },
        ],
    }
}

fn build_flush_rx_fifo_behavior(_input: &AnalysisInput) -> FunctionBehavior {
    FunctionBehavior {
        name: "flush_rx_fifo".into(),
        role: FunctionRole::SerialFlushRxFifo,
        body: vec![BehaviorNode::Loop {
            condition: BranchCondition::RegFieldEq {
                register: "fr".into(),
                field: "RXFE".into(),
                value: 0,
            },
            body: vec![BehaviorNode::RegReadRaw {
                register: "dr".into(),
            }],
        }],
    }
}

fn build_flush_tx_fifo_behavior() -> FunctionBehavior {
    FunctionBehavior {
        name: "flush_tx_fifo".into(),
        role: FunctionRole::SerialFlushTxFifo,
        body: vec![
            BehaviorNode::RegModify {
                register: "lcr_h".into(),
                fields: vec![FieldWrite {
                    field: "FEN".into(),
                    value: FieldWriteValue::Named("DISABLED".into()),
                }],
            },
            BehaviorNode::RegModify {
                register: "lcr_h".into(),
                fields: vec![FieldWrite {
                    field: "FEN".into(),
                    value: FieldWriteValue::Named("ENABLED".into()),
                }],
            },
        ],
    }
}

fn build_send_break_behavior() -> FunctionBehavior {
    FunctionBehavior {
        name: "send_break".into(),
        role: FunctionRole::SerialSendBreak,
        body: vec![BehaviorNode::Branch {
            condition: BranchCondition::BoolParam { name: "on".into() },
            then_body: vec![BehaviorNode::RegModify {
                register: "lcr_h".into(),
                fields: vec![FieldWrite {
                    field: "BRK".into(),
                    value: FieldWriteValue::Named("ENABLED".into()),
                }],
            }],
            else_body: vec![BehaviorNode::RegModify {
                register: "lcr_h".into(),
                fields: vec![FieldWrite {
                    field: "BRK".into(),
                    value: FieldWriteValue::Named("DISABLED".into()),
                }],
            }],
        }],
    }
}

// ---------------------------------------------------------------------------
// ISR behavior
// ---------------------------------------------------------------------------

fn build_handle_irq_behavior(_input: &AnalysisInput) -> FunctionBehavior {
    let body = vec![
        // Read masked interrupt status.
        BehaviorNode::RegReadRaw {
            register: "mis".into(),
        },
        // Early return if no interrupts pending.
        BehaviorNode::Branch {
            condition: BranchCondition::RawEqZero {
                source: "status".into(),
            },
            then_body: vec![BehaviorNode::Return {
                semantic: ReturnSemantic::IrqNotHandled,
            }],
            else_body: vec![],
        },
        // RX class service.
        BehaviorNode::Branch {
            condition: BranchCondition::InterruptClassActive {
                status_source: "status".into(),
                class_name: "rx".into(),
            },
            then_body: vec![BehaviorNode::CallHelper {
                callee: "service_rx_fifo".into(),
            }],
            else_body: vec![],
        },
        // TX class service.
        BehaviorNode::Branch {
            condition: BranchCondition::InterruptClassActive {
                status_source: "status".into(),
                class_name: "tx".into(),
            },
            then_body: vec![BehaviorNode::CallHelper {
                callee: "service_tx_fifo".into(),
            }],
            else_body: vec![],
        },
        // Clear error interrupts.
        BehaviorNode::InterruptClassClear {
            status_source: "status".into(),
            class_name: "error".into(),
        },
        // Conditional return based on service outcome.
        BehaviorNode::Return {
            semantic: ReturnSemantic::ConditionalIrqResult,
        },
    ];

    FunctionBehavior {
        name: "handle_irq".into(),
        role: FunctionRole::Isr,
        body,
    }
}

// ---------------------------------------------------------------------------
// Helper functions with material behavioral logic
// ---------------------------------------------------------------------------

fn build_program_polling_port_helper() -> FunctionBehavior {
    FunctionBehavior {
        name: "program_polling_port".into(),
        role: FunctionRole::Helper,
        body: vec![
            BehaviorNode::CallHelper {
                callee: "disable_uart".into(),
            },
            BehaviorNode::CallHelper {
                callee: "disable_fifo".into(),
            },
            BehaviorNode::OrderedSequence {
                steps: vec![
                    BehaviorNode::ComputeBaudDivisor,
                    BehaviorNode::CallHelper {
                        callee: "set_baud_rate".into(),
                    },
                    BehaviorNode::CallHelper {
                        callee: "configure_line_control".into(),
                    },
                    BehaviorNode::CallHelper {
                        callee: "configure_interrupt_levels".into(),
                    },
                    BehaviorNode::CallHelper {
                        callee: "mask_all_interrupts".into(),
                    },
                    BehaviorNode::CallHelper {
                        callee: "clear_all_interrupts".into(),
                    },
                    BehaviorNode::CallHelper {
                        callee: "enable_uart".into(),
                    },
                    BehaviorNode::CallHelper {
                        callee: "enable_rx_interrupts".into(),
                    },
                ],
            },
        ],
    }
}

fn build_shutdown_port_helper() -> FunctionBehavior {
    FunctionBehavior {
        name: "shutdown_port".into(),
        role: FunctionRole::Helper,
        body: vec![
            BehaviorNode::CallHelper {
                callee: "mask_all_interrupts".into(),
            },
            BehaviorNode::CallHelper {
                callee: "clear_all_interrupts".into(),
            },
            BehaviorNode::CallHelper {
                callee: "disable_fifo".into(),
            },
            BehaviorNode::CallHelper {
                callee: "disable_uart".into(),
            },
            BehaviorNode::FieldAssign {
                field: "pending_tx".into(),
                value: AssignValue::None,
            },
        ],
    }
}

fn build_service_rx_fifo_helper() -> FunctionBehavior {
    FunctionBehavior {
        name: "service_rx_fifo".into(),
        role: FunctionRole::Helper,
        body: vec![BehaviorNode::Loop {
            condition: BranchCondition::HelperPredicate {
                callee: "rx_char".into(),
            },
            body: vec![],
        }],
    }
}

fn build_service_tx_fifo_helper() -> FunctionBehavior {
    FunctionBehavior {
        name: "service_tx_fifo".into(),
        role: FunctionRole::Helper,
        body: vec![BehaviorNode::Branch {
            condition: BranchCondition::FieldIsNone {
                field: "pending_tx".into(),
            },
            then_body: vec![
                BehaviorNode::CallHelper {
                    callee: "disable_tx_interrupts".into(),
                },
                BehaviorNode::Return {
                    semantic: ReturnSemantic::Bool(true),
                },
            ],
            else_body: vec![BehaviorNode::Branch {
                condition: BranchCondition::HelperPredicate {
                    callee: "tx_ready".into(),
                },
                then_body: vec![
                    BehaviorNode::RegWrite {
                        register: "dr".into(),
                        fields: vec![FieldWrite {
                            field: "DATA".into(),
                            value: FieldWriteValue::Variable("pending_byte".into()),
                        }],
                    },
                    BehaviorNode::FieldAssign {
                        field: "pending_tx".into(),
                        value: AssignValue::None,
                    },
                    BehaviorNode::CallHelper {
                        callee: "disable_tx_interrupts".into(),
                    },
                    BehaviorNode::Return {
                        semantic: ReturnSemantic::Bool(true),
                    },
                ],
                else_body: vec![BehaviorNode::Return {
                    semantic: ReturnSemantic::Bool(false),
                }],
            }],
        }],
    }
}
