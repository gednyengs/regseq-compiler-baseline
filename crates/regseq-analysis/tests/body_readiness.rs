//! Tests for MIR body capture exposure and supported-subset validation.
//!
//! Verifies that AnalysisInput exposes per-function MIR body data,
//! computes helper reachability from MIR call edges, and that body
//! readiness validation catches missing, unsupported, and
//! provenance-deficient MIR features.
//!
//! This test file exercises the deprecated `regseq_analysis::legacy_input`
//! shim; `#![allow(deprecated)]` is applied crate-wide so the shim's
//! deprecation notices don't trip `-D warnings` while the shim is still
//! in place (retirement target: `regseq-lowering-10`).
#![allow(deprecated)]

use regseq_analysis::legacy_input::{check_body_readiness, AnalysisInput};
use regseq_model::binding::{
    BindingHeader, BindingModel, BindingProvenance, InterruptBinding, JoinKey, MmioBinding,
    PropertyBinding,
};
use regseq_model::declaration::bodies::BodyDeclSet;
use regseq_model::declaration::callbacks::{
    CallbackContext, CallbackDeclSet, CallbackFamily, CallbackImplDecl,
};
use regseq_model::declaration::mir_body::{
    FnKind, FnParamDecl, IntWidth, MirBasicBlockData, MirBody, MirBodyDeclSet, MirConstValue,
    MirConstant, MirFnBodyDecl, MirLocalDecl, MirMutability, MirOperand, MirPlace,
    MirProjectionElem, MirQueryFlavor, MirRvalue, MirSourceInfo, MirSourceScope, MirStatement,
    MirStatementKind, MirSwitchTargets, MirTerminator, MirTerminatorKind, MirTy, MirUnwindAction,
};
use regseq_model::declaration::operations::{LoggingDeclSet, PmGateDeclSet, TimedDelayDeclSet};
use regseq_model::declaration::properties::{PropertyDeclSet, PropertyFieldDecl};
use regseq_model::declaration::provenance::Provenance;
use regseq_model::declaration::registers::RegisterDeclSet;
use regseq_model::declaration::resources::{
    ClockDeclSet, InterruptBindingHints, InterruptDeclSet, InterruptFieldShape,
    InterruptResourceDecl, MmioFieldDecl, PinCtrlDeclSet, ResetDeclSet, ResourceDeclSet,
};
use regseq_model::declaration::synchronization::SynchronizationDeclSet;
use regseq_model::declaration::{
    BusType, DeclarationBuildContext, DeclarationModel, DeviceClass, DriverDecl,
};
use regseq_model::joined::perform_join;
use regseq_model::platform::PlatformValue;

// ---- MIR fixture helpers ----

fn src_info() -> MirSourceInfo {
    MirSourceInfo {
        scope: 0,
        span: Some("test".into()),
    }
}

/// A local variable declaration.
fn local_decl(index: u32, ty: MirTy, name: Option<&str>) -> MirLocalDecl {
    MirLocalDecl {
        index,
        ty,
        mutability: MirMutability::Mut,
        name: name.map(|s| s.into()),
    }
}

/// A minimal MIR body with just a Return terminator.
fn trivial_mir_body() -> MirBody {
    MirBody {
        local_decls: vec![local_decl(0, MirTy::Tuple { elements: vec![] }, None)],
        arg_count: 0,
        basic_blocks: vec![MirBasicBlockData {
            statements: vec![],
            terminator: Some(MirTerminator {
                source_info: src_info(),
                kind: MirTerminatorKind::Return,
            }),
            is_cleanup: false,
        }],
        source_scopes: vec![MirSourceScope {
            index: 0,
            name: None,
        }],
    }
}

/// A MIR body with a Call to a helper followed by Return.
fn calling_mir_body(callee_path: &str) -> MirBody {
    MirBody {
        local_decls: vec![
            local_decl(0, MirTy::Tuple { elements: vec![] }, None),
            local_decl(1, MirTy::Tuple { elements: vec![] }, Some("_tmp")),
        ],
        arg_count: 0,
        basic_blocks: vec![
            MirBasicBlockData {
                statements: vec![],
                terminator: Some(MirTerminator {
                    source_info: src_info(),
                    kind: MirTerminatorKind::Call {
                        func: MirOperand::Constant {
                            constant: MirConstant {
                                ty: MirTy::Fn {
                                    path: callee_path.into(),
                                    params: vec![],
                                    ret: Box::new(MirTy::Tuple { elements: vec![] }),
                                },
                                value: MirConstValue::ZeroSized,
                            },
                        },
                        args: vec![],
                        destination: MirPlace {
                            local: 1,
                            projection: vec![],
                        },
                        target: Some(1),
                        unwind: MirUnwindAction::Continue,
                    },
                }),
                is_cleanup: false,
            },
            MirBasicBlockData {
                statements: vec![],
                terminator: Some(MirTerminator {
                    source_info: src_info(),
                    kind: MirTerminatorKind::Return,
                }),
                is_cleanup: false,
            },
        ],
        source_scopes: vec![MirSourceScope {
            index: 0,
            name: None,
        }],
    }
}

/// A MIR body exhibiting fallible control flow (`?`-style pattern):
/// Discriminant assignment followed by SwitchInt, with a cleanup block.
fn fallible_mir_body() -> MirBody {
    MirBody {
        local_decls: vec![
            local_decl(
                0,
                MirTy::Enum {
                    path: "core::result::Result".into(),
                    variants: vec![],
                },
                None,
            ),
            local_decl(
                1,
                MirTy::Enum {
                    path: "core::result::Result".into(),
                    variants: vec![],
                },
                Some("_result"),
            ),
            local_decl(
                2,
                MirTy::Int {
                    signed: false,
                    width: IntWidth::U8,
                },
                Some("_discr"),
            ),
        ],
        arg_count: 0,
        basic_blocks: vec![
            // bb0: call that returns Result, then goto bb1
            MirBasicBlockData {
                statements: vec![],
                terminator: Some(MirTerminator {
                    source_info: src_info(),
                    kind: MirTerminatorKind::Call {
                        func: MirOperand::Constant {
                            constant: MirConstant {
                                ty: MirTy::Fn {
                                    path: "some_fallible_op".into(),
                                    params: vec![],
                                    ret: Box::new(MirTy::Tuple { elements: vec![] }),
                                },
                                value: MirConstValue::ZeroSized,
                            },
                        },
                        args: vec![],
                        destination: MirPlace {
                            local: 1,
                            projection: vec![],
                        },
                        target: Some(1),
                        unwind: MirUnwindAction::Cleanup { target: 3 },
                    },
                }),
                is_cleanup: false,
            },
            // bb1: Discriminant + SwitchInt (the ? pattern)
            MirBasicBlockData {
                statements: vec![MirStatement {
                    source_info: src_info(),
                    kind: MirStatementKind::Assign {
                        place: MirPlace {
                            local: 2,
                            projection: vec![],
                        },
                        rvalue: Box::new(MirRvalue::Discriminant {
                            place: MirPlace {
                                local: 1,
                                projection: vec![],
                            },
                        }),
                    },
                }],
                terminator: Some(MirTerminator {
                    source_info: src_info(),
                    kind: MirTerminatorKind::SwitchInt {
                        discr: MirOperand::Copy {
                            place: MirPlace {
                                local: 2,
                                projection: vec![],
                            },
                        },
                        targets: MirSwitchTargets {
                            values: vec![0],
                            targets: vec![2, 4],
                        },
                    },
                }),
                is_cleanup: false,
            },
            // bb2: success path → Return
            MirBasicBlockData {
                statements: vec![],
                terminator: Some(MirTerminator {
                    source_info: src_info(),
                    kind: MirTerminatorKind::Return,
                }),
                is_cleanup: false,
            },
            // bb3: cleanup block
            MirBasicBlockData {
                statements: vec![],
                terminator: Some(MirTerminator {
                    source_info: src_info(),
                    kind: MirTerminatorKind::UnwindResume,
                }),
                is_cleanup: true,
            },
            // bb4: error path → Drop + Return
            MirBasicBlockData {
                statements: vec![],
                terminator: Some(MirTerminator {
                    source_info: src_info(),
                    kind: MirTerminatorKind::Drop {
                        place: MirPlace {
                            local: 1,
                            projection: vec![],
                        },
                        target: 2,
                        unwind: MirUnwindAction::Cleanup { target: 3 },
                        replace: false,
                    },
                }),
                is_cleanup: false,
            },
        ],
        source_scopes: vec![MirSourceScope {
            index: 0,
            name: Some("init".into()),
        }],
    }
}

/// Build a MIR callback body declaration.
fn mir_callback(
    trait_name: &str,
    method_name: &str,
    body: MirBody,
    helpers: Vec<&str>,
) -> MirFnBodyDecl {
    MirFnBodyDecl {
        name: method_name.into(),
        def_path: format!("TestDriver::{}::{}", trait_name, method_name),
        kind: FnKind::Callback {
            trait_name: trait_name.into(),
            method_name: method_name.into(),
        },
        receiver: "ref_mut".into(),
        params: vec![],
        return_type: Some(MirTy::Unsupported {
            description: "Result<(), RegseqError>".into(),
        }),
        body,
        helper_calls: helpers.into_iter().map(|s| s.into()).collect(),
        monomorphization: None,
        provenance: Provenance::new("mir_bodies").with_containing_item(method_name),
    }
}

/// Build a MIR helper body declaration.
fn mir_helper(name: &str, body: MirBody, helpers: Vec<&str>) -> MirFnBodyDecl {
    MirFnBodyDecl {
        name: name.into(),
        def_path: format!("TestDriver::{}", name),
        kind: FnKind::Helper,
        receiver: "ref_mut".into(),
        params: vec![],
        return_type: None,
        body,
        helper_calls: helpers.into_iter().map(|s| s.into()).collect(),
        monomorphization: None,
        provenance: Provenance::new("mir_bodies").with_containing_item(name),
    }
}

/// Build a MIR ISR body declaration.
fn mir_isr(name: &str, body: MirBody, helpers: Vec<&str>) -> MirFnBodyDecl {
    MirFnBodyDecl {
        name: name.into(),
        def_path: format!("TestDriver::{}", name),
        kind: FnKind::Isr,
        receiver: "ref_mut".into(),
        params: vec![],
        return_type: Some(MirTy::Unsupported {
            description: "IrqResult".into(),
        }),
        body,
        helper_calls: helpers.into_iter().map(|s| s.into()).collect(),
        monomorphization: None,
        provenance: Provenance::new("mir_bodies").with_containing_item(name),
    }
}

/// Build a MIR free function body declaration.
fn mir_free_fn(name: &str, params: Vec<(&str, MirTy)>, body: MirBody) -> MirFnBodyDecl {
    MirFnBodyDecl {
        name: name.into(),
        def_path: format!("crate::{}", name),
        kind: FnKind::FreeFunction,
        receiver: "none".into(),
        params: params
            .into_iter()
            .map(|(n, t)| FnParamDecl {
                name: n.into(),
                ty: t,
            })
            .collect(),
        return_type: None,
        body,
        helper_calls: vec![],
        monomorphization: None,
        provenance: Provenance::new("mir_bodies").with_containing_item(name),
    }
}

/// Build a representative set of MIR bodies.
fn representative_mir_bodies() -> MirBodyDeclSet {
    MirBodyDeclSet {
        functions: vec![
            // Lifecycle::init calls setup_device, has fallible pattern
            mir_callback(
                "Lifecycle",
                "init",
                fallible_mir_body(),
                vec!["setup_device"],
            ),
            // Lifecycle::deinit calls shutdown_device
            mir_callback(
                "Lifecycle",
                "deinit",
                calling_mir_body("TestDriver::shutdown_device"),
                vec!["shutdown_device"],
            ),
            // Serial::tx_byte — leaf callback
            mir_callback("Serial", "tx_byte", trivial_mir_body(), vec![]),
            // ISR: handle_irq calls service_rx
            mir_isr(
                "handle_irq",
                calling_mir_body("TestDriver::service_rx"),
                vec!["service_rx"],
            ),
            // Helper: setup_device calls configure_hw (transitive)
            mir_helper(
                "setup_device",
                calling_mir_body("TestDriver::configure_hw"),
                vec!["configure_hw"],
            ),
            // Helper: shutdown_device — leaf
            mir_helper("shutdown_device", trivial_mir_body(), vec![]),
            // Helper: configure_hw — leaf (transitively reachable)
            mir_helper("configure_hw", trivial_mir_body(), vec![]),
            // Helper: service_rx — reachable from ISR
            mir_helper("service_rx", trivial_mir_body(), vec![]),
            // Free function: compute_divisor
            mir_free_fn(
                "compute_divisor",
                vec![
                    (
                        "clock_hz",
                        MirTy::Int {
                            signed: false,
                            width: IntWidth::U32,
                        },
                    ),
                    (
                        "baud",
                        MirTy::Int {
                            signed: false,
                            width: IntWidth::U32,
                        },
                    ),
                ],
                MirBody {
                    local_decls: vec![
                        local_decl(
                            0,
                            MirTy::Int {
                                signed: false,
                                width: IntWidth::U32,
                            },
                            None,
                        ),
                        local_decl(
                            1,
                            MirTy::Int {
                                signed: false,
                                width: IntWidth::U32,
                            },
                            Some("clock_hz"),
                        ),
                        local_decl(
                            2,
                            MirTy::Int {
                                signed: false,
                                width: IntWidth::U32,
                            },
                            Some("baud"),
                        ),
                    ],
                    arg_count: 2,
                    basic_blocks: vec![MirBasicBlockData {
                        statements: vec![MirStatement {
                            source_info: src_info(),
                            kind: MirStatementKind::Assign {
                                place: MirPlace {
                                    local: 0,
                                    projection: vec![],
                                },
                                rvalue: Box::new(MirRvalue::BinaryOp {
                                    op: regseq_model::declaration::mir_body::MirBinOp::Div,
                                    lhs: MirOperand::Copy {
                                        place: MirPlace {
                                            local: 1,
                                            projection: vec![],
                                        },
                                    },
                                    rhs: MirOperand::Copy {
                                        place: MirPlace {
                                            local: 2,
                                            projection: vec![],
                                        },
                                    },
                                }),
                            },
                        }],
                        terminator: Some(MirTerminator {
                            source_info: src_info(),
                            kind: MirTerminatorKind::Return,
                        }),
                        is_cleanup: false,
                    }],
                    source_scopes: vec![MirSourceScope {
                        index: 0,
                        name: Some("compute_divisor".into()),
                    }],
                },
            ),
        ],
        query_flavor: MirQueryFlavor::DropsElaborated,
    }
}

// ---- Declaration model builder ----

fn decl_with_mir_bodies(mir_bodies: MirBodyDeclSet) -> DeclarationModel {
    DeclarationModel {
        driver: DriverDecl {
            driver_name: "TestDriver".into(),
            compatible: vec!["vendor,test-device".into()],
            device_class: DeviceClass::SerialUart,
            bus: Some(BusType::Amba),
            provenance: Provenance::new("identity"),
        },
        resources: ResourceDeclSet {
            mmio: vec![MmioFieldDecl {
                field_name: "regs".into(),
                mmio_id: Some("dev_regs".into()),
                register_block_type: Some("DevRegs".into()),
                provenance: Provenance::new("register_access"),
            }],
            interrupts: InterruptDeclSet {
                resources: vec![InterruptResourceDecl {
                    field_name: "irq".into(),
                    interrupt_id: "dev_irq".into(),
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
            lifecycle_impls: vec![CallbackImplDecl {
                driver_type: "TestDriver".into(),
                context: CallbackContext::Framework,
                family: CallbackFamily::Lifecycle,
                provenance: Provenance::new("callback"),
            }],
            serial_impls: vec![CallbackImplDecl {
                driver_type: "TestDriver".into(),
                context: CallbackContext::SubsystemSerialized,
                family: CallbackFamily::Serial,
                provenance: Provenance::new("callback"),
            }],
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
            fields: vec![PropertyFieldDecl {
                driver_type: "TestDriver".into(),
                field_name: "speed".into(),
                field_type: "u32".into(),
                property_id: "speed".into(),
                default_literal: Some("9600".into()),
                vendor_prefix: None,
                required: true,
                provenance: Provenance::new("firmware_properties"),
            }],
            lints: vec![],
        },
        timed_delay: TimedDelayDeclSet {
            call_sites: vec![],
            duration_ctors: vec![],
        },
        logging: LoggingDeclSet { call_sites: vec![] },
        pm_gate: PmGateDeclSet { gates: vec![] },
        bodies: BodyDeclSet { functions: vec![] },
        mir_bodies,
        build_context: DeclarationBuildContext {
            target_os: "linux".into(),
        },
    }
}

fn minimal_binding() -> BindingModel {
    BindingModel {
        header: BindingHeader {
            version: "1".into(),
            os: "linux".into(),
            arch: "aarch64".into(),
            board: "test-board".into(),
            provenance: BindingProvenance::new("platform").with_source_file("test.platform.toml"),
        },
        properties: vec![PropertyBinding {
            key: "speed".into(),
            value: PlatformValue::Integer(9600),
            provenance: BindingProvenance::new("properties").with_section_key("speed"),
        }],
        mmio: vec![MmioBinding {
            join_key: JoinKey::ExplicitId("dev_regs".into()),
            base: Some(0x0900_0000),
            size: Some(0x1000),
            provenance: BindingProvenance::new("mmio").with_section_key("dev_regs"),
        }],
        interrupts: vec![InterruptBinding {
            join_key: JoinKey::ExplicitId("dev_irq".into()),
            trigger_type: Some("level".into()),
            trigger_polarity: Some("high".into()),
            priority: None,
            shared: Some(false),
            provenance: BindingProvenance::new("interrupt").with_section_key("dev_irq"),
        }],
        clocks: vec![],
        resets: vec![],
        pinctrl: vec![],
        bus: None,
    }
}

fn make_input(mir_bodies: MirBodyDeclSet) -> AnalysisInput {
    let decl = decl_with_mir_bodies(mir_bodies);
    let binding = minimal_binding();
    let joined = perform_join(&decl, &binding).expect("join should succeed");
    AnalysisInput::new(decl, joined, None)
}

// ============================================================
// Test 1: AnalysisInput exposes per-function MIR body capture
// ============================================================

#[test]
fn mir_bodies_exposed_through_analysis_input() {
    let input = make_input(representative_mir_bodies());
    assert_eq!(input.mir_bodies().functions.len(), 9);
}

#[test]
fn mir_callback_bodies_filtered_correctly() {
    let input = make_input(representative_mir_bodies());
    let cbs = input.mir_callback_bodies();
    assert_eq!(cbs.len(), 3);
    assert!(cbs.iter().all(|b| b.is_callback()));

    let names: Vec<&str> = cbs.iter().map(|b| b.name.as_str()).collect();
    assert!(names.contains(&"init"));
    assert!(names.contains(&"deinit"));
    assert!(names.contains(&"tx_byte"));
}

#[test]
fn mir_helper_bodies_filtered_correctly() {
    let input = make_input(representative_mir_bodies());
    let helpers = input.mir_helper_bodies();
    assert_eq!(helpers.len(), 4);
    assert!(helpers.iter().all(|b| b.is_helper()));
}

#[test]
fn mir_isr_bodies_filtered_correctly() {
    let input = make_input(representative_mir_bodies());
    let isrs = input.mir_isr_bodies();
    assert_eq!(isrs.len(), 1);
    assert_eq!(isrs[0].name, "handle_irq");
}

#[test]
fn mir_free_function_bodies_filtered_correctly() {
    let input = make_input(representative_mir_bodies());
    let ffs = input.mir_free_function_bodies();
    assert_eq!(ffs.len(), 1);
    assert_eq!(ffs[0].name, "compute_divisor");
}

#[test]
fn mir_body_by_name_lookup() {
    let input = make_input(representative_mir_bodies());

    let init = input.mir_body_by_name("init").expect("init should exist");
    assert!(init.is_callback());

    let helper = input
        .mir_body_by_name("setup_device")
        .expect("setup_device should exist");
    assert!(helper.is_helper());

    assert!(input.mir_body_by_name("nonexistent").is_none());
}

#[test]
fn mir_query_flavor_accessible() {
    let input = make_input(representative_mir_bodies());
    assert_eq!(*input.mir_query_flavor(), MirQueryFlavor::DropsElaborated);
}

// ============================================================
// Test 2: reachable helpers from MIR call edges
// ============================================================

#[test]
fn mir_reachable_helpers_from_callbacks() {
    let input = make_input(representative_mir_bodies());
    let reachable = input.mir_reachable_helper_names();

    // Directly called from callbacks:
    assert!(reachable.contains("setup_device"));
    assert!(reachable.contains("shutdown_device"));
    // Transitively reachable from setup_device:
    assert!(reachable.contains("configure_hw"));
    // Called from ISR:
    assert!(reachable.contains("service_rx"));
}

#[test]
fn mir_unreachable_free_function_not_in_reachable_set() {
    let input = make_input(representative_mir_bodies());
    let reachable = input.mir_reachable_helper_names();
    // compute_divisor is not called by any callback or ISR.
    assert!(!reachable.contains("compute_divisor"));
}

#[test]
fn mir_transitive_reachability_works() {
    let input = make_input(representative_mir_bodies());
    let reachable = input.mir_reachable_helper_names();
    // configure_hw is only reachable via setup_device (called by init).
    assert!(reachable.contains("configure_hw"));
}

// ============================================================
// Test 3: supported-subset validation catches missing/unsupported
// ============================================================

#[test]
fn clean_mir_bodies_pass_validation() {
    let input = make_input(representative_mir_bodies());
    let report = check_body_readiness(&input);

    assert!(!report.has_errors());
    assert_eq!(report.unsupported_node_count, 0);
    assert_eq!(report.callback_body_count, 3);
    assert_eq!(report.helper_body_count, 4);
    assert_eq!(report.isr_body_count, 1);
}

#[test]
fn missing_callback_mir_bodies_detected() {
    let input = make_input(MirBodyDeclSet {
        functions: vec![],
        query_flavor: MirQueryFlavor::DropsElaborated,
    });
    let report = check_body_readiness(&input);

    assert!(report.has_errors());
    let body_coverage_errors: Vec<_> = report
        .errors()
        .iter()
        .filter(|f| f.category == "body_coverage")
        .cloned()
        .collect();
    // Both Lifecycle and Serial families missing bodies.
    assert_eq!(body_coverage_errors.len(), 2);
}

#[test]
fn unsupported_mir_statement_surfaced() {
    let mut bodies = representative_mir_bodies();
    // Inject an unsupported statement into init's body.
    bodies.functions[0].body.basic_blocks[0]
        .statements
        .push(MirStatement {
            source_info: src_info(),
            kind: MirStatementKind::Unsupported {
                description: "FakeIntrinsic".into(),
            },
        });

    let input = make_input(bodies);
    let report = check_body_readiness(&input);

    assert!(report.has_cautions());
    assert_eq!(report.unsupported_node_count, 1);
    let cautions = report.cautions();
    assert!(cautions
        .iter()
        .any(|f| f.message.contains("FakeIntrinsic") && f.message.contains("init")));
}

#[test]
fn unsupported_mir_terminator_surfaced() {
    let bodies = MirBodyDeclSet {
        functions: vec![
            mir_callback(
                "Lifecycle",
                "init",
                MirBody {
                    local_decls: vec![local_decl(0, MirTy::Tuple { elements: vec![] }, None)],
                    arg_count: 0,
                    basic_blocks: vec![MirBasicBlockData {
                        statements: vec![],
                        terminator: Some(MirTerminator {
                            source_info: src_info(),
                            kind: MirTerminatorKind::Unsupported {
                                description: "CoroutineDrop".into(),
                            },
                        }),
                        is_cleanup: false,
                    }],
                    source_scopes: vec![MirSourceScope {
                        index: 0,
                        name: None,
                    }],
                },
                vec![],
            ),
            mir_callback("Serial", "tx_byte", trivial_mir_body(), vec![]),
        ],
        query_flavor: MirQueryFlavor::DropsElaborated,
    };

    let input = make_input(bodies);
    let report = check_body_readiness(&input);

    assert_eq!(report.unsupported_node_count, 1);
    let cautions = report.cautions();
    assert!(cautions
        .iter()
        .any(|f| f.message.contains("CoroutineDrop") && f.message.contains("init")));
}

#[test]
fn unsupported_rvalue_surfaced() {
    let bodies = MirBodyDeclSet {
        functions: vec![
            mir_callback(
                "Lifecycle",
                "init",
                MirBody {
                    local_decls: vec![
                        local_decl(0, MirTy::Tuple { elements: vec![] }, None),
                        local_decl(
                            1,
                            MirTy::Int {
                                signed: false,
                                width: IntWidth::U32,
                            },
                            None,
                        ),
                    ],
                    arg_count: 0,
                    basic_blocks: vec![MirBasicBlockData {
                        statements: vec![MirStatement {
                            source_info: src_info(),
                            kind: MirStatementKind::Assign {
                                place: MirPlace {
                                    local: 1,
                                    projection: vec![],
                                },
                                rvalue: Box::new(MirRvalue::Unsupported {
                                    description: "ThreadLocalRef".into(),
                                }),
                            },
                        }],
                        terminator: Some(MirTerminator {
                            source_info: src_info(),
                            kind: MirTerminatorKind::Return,
                        }),
                        is_cleanup: false,
                    }],
                    source_scopes: vec![MirSourceScope {
                        index: 0,
                        name: None,
                    }],
                },
                vec![],
            ),
            mir_callback("Serial", "tx_byte", trivial_mir_body(), vec![]),
        ],
        query_flavor: MirQueryFlavor::DropsElaborated,
    };

    let input = make_input(bodies);
    let report = check_body_readiness(&input);

    assert_eq!(report.unsupported_node_count, 1);
    assert!(report
        .cautions()
        .iter()
        .any(|f| f.message.contains("ThreadLocalRef")));
}

#[test]
fn unsupported_projection_surfaced() {
    let bodies = MirBodyDeclSet {
        functions: vec![
            mir_callback(
                "Lifecycle",
                "init",
                MirBody {
                    local_decls: vec![
                        local_decl(0, MirTy::Tuple { elements: vec![] }, None),
                        local_decl(
                            1,
                            MirTy::Int {
                                signed: false,
                                width: IntWidth::U32,
                            },
                            None,
                        ),
                    ],
                    arg_count: 0,
                    basic_blocks: vec![MirBasicBlockData {
                        statements: vec![MirStatement {
                            source_info: src_info(),
                            kind: MirStatementKind::Assign {
                                place: MirPlace {
                                    local: 1,
                                    projection: vec![],
                                },
                                rvalue: Box::new(MirRvalue::Use {
                                    operand: MirOperand::Copy {
                                        place: MirPlace {
                                            local: 0,
                                            projection: vec![MirProjectionElem::Unsupported {
                                                description: "WackyProjection".into(),
                                            }],
                                        },
                                    },
                                }),
                            },
                        }],
                        terminator: Some(MirTerminator {
                            source_info: src_info(),
                            kind: MirTerminatorKind::Return,
                        }),
                        is_cleanup: false,
                    }],
                    source_scopes: vec![MirSourceScope {
                        index: 0,
                        name: None,
                    }],
                },
                vec![],
            ),
            mir_callback("Serial", "tx_byte", trivial_mir_body(), vec![]),
        ],
        query_flavor: MirQueryFlavor::DropsElaborated,
    };

    let input = make_input(bodies);
    let report = check_body_readiness(&input);

    assert_eq!(report.unsupported_node_count, 1);
    assert!(report
        .cautions()
        .iter()
        .any(|f| f.message.contains("WackyProjection")));
}

// ============================================================
// Test 4: cleanup/drop semantics visible in analysis data
// ============================================================

#[test]
fn cleanup_blocks_visible_in_report() {
    let input = make_input(representative_mir_bodies());
    let report = check_body_readiness(&input);

    // fallible_mir_body has 1 cleanup block and 1 Drop terminator.
    assert!(report.mir_form_summary.cleanup_block_count >= 1);
    assert!(report.mir_form_summary.drop_terminator_count >= 1);
}

#[test]
fn cleanup_blocks_visible_directly() {
    let input = make_input(representative_mir_bodies());

    let init = input.mir_body_by_name("init").unwrap();
    let cleanup_count = init
        .body
        .basic_blocks
        .iter()
        .filter(|bb| bb.is_cleanup)
        .count();
    assert!(
        cleanup_count >= 1,
        "init should have cleanup blocks from ?-desugaring"
    );
}

#[test]
fn drop_terminators_visible_directly() {
    let input = make_input(representative_mir_bodies());

    let init = input.mir_body_by_name("init").unwrap();
    let drop_count = init
        .body
        .basic_blocks
        .iter()
        .filter_map(|bb| bb.terminator.as_ref())
        .filter(|t| matches!(t.kind, MirTerminatorKind::Drop { .. }))
        .count();
    assert!(drop_count >= 1, "init should have Drop terminators");
}

#[test]
fn unwind_cleanup_edges_visible() {
    let input = make_input(representative_mir_bodies());

    let init = input.mir_body_by_name("init").unwrap();
    let has_cleanup_edge = init
        .body
        .basic_blocks
        .iter()
        .filter_map(|bb| bb.terminator.as_ref())
        .any(|t| match &t.kind {
            MirTerminatorKind::Call { unwind, .. }
            | MirTerminatorKind::Drop { unwind, .. }
            | MirTerminatorKind::Assert { unwind, .. } => {
                matches!(unwind, MirUnwindAction::Cleanup { .. })
            }
            _ => false,
        });
    assert!(has_cleanup_edge, "init should have Cleanup unwind edges");
}

// ============================================================
// Test 5: body data remains traceable to authored source/provenance
// ============================================================

#[test]
fn mir_provenance_present_on_all_bodies() {
    let input = make_input(representative_mir_bodies());

    for func in &input.mir_bodies().functions {
        assert_eq!(
            func.provenance.raw_family, "mir_bodies",
            "function '{}' should have mir_bodies provenance",
            func.name,
        );
        assert!(
            func.provenance.containing_item.is_some(),
            "function '{}' should have containing_item provenance",
            func.name,
        );
        assert!(
            !func.def_path.is_empty(),
            "function '{}' should have a non-empty def_path",
            func.name,
        );
    }
}

#[test]
fn empty_provenance_produces_caution() {
    let bodies = MirBodyDeclSet {
        functions: vec![
            mir_callback("Lifecycle", "init", trivial_mir_body(), vec![]),
            mir_callback("Serial", "tx_byte", trivial_mir_body(), vec![]),
            MirFnBodyDecl {
                name: "bad_provenance".into(),
                def_path: "".into(), // empty
                kind: FnKind::Helper,
                receiver: "ref_mut".into(),
                params: vec![],
                return_type: None,
                body: trivial_mir_body(),
                helper_calls: vec![],
                monomorphization: None,
                provenance: Provenance::new(""), // empty raw_family
            },
        ],
        query_flavor: MirQueryFlavor::DropsElaborated,
    };

    let input = make_input(bodies);
    let report = check_body_readiness(&input);

    let cautions = report.cautions();
    let prov_findings: Vec<_> = cautions
        .iter()
        .filter(|f| f.category == "body_provenance")
        .collect();
    // Should have findings for both empty raw_family and empty def_path.
    assert!(prov_findings.len() >= 2);
    assert!(prov_findings
        .iter()
        .any(|f| f.message.contains("bad_provenance") && f.message.contains("raw_family")));
    assert!(prov_findings
        .iter()
        .any(|f| f.message.contains("bad_provenance") && f.message.contains("def_path")));
}

// ============================================================
// Test 6: fallible control-flow patterns validated deterministically
// ============================================================

#[test]
fn fallible_patterns_detected() {
    let input = make_input(representative_mir_bodies());
    let report = check_body_readiness(&input);

    // init has a fallible pattern (Discriminant + SwitchInt).
    assert!(report.mir_form_summary.fallible_pattern_count >= 1);
}

#[test]
fn fallible_pattern_finding_present() {
    let input = make_input(representative_mir_bodies());
    let report = check_body_readiness(&input);

    let infos: Vec<_> = report
        .findings
        .iter()
        .filter(|f| f.category == "fallible_pattern")
        .collect();
    assert!(!infos.is_empty());
    assert!(infos.iter().any(|f| f.message.contains("init")));
}

#[test]
fn no_false_fallible_pattern_on_trivial_body() {
    // A trivial body with just Return should not trigger fallible detection.
    let bodies = MirBodyDeclSet {
        functions: vec![
            mir_callback("Lifecycle", "init", trivial_mir_body(), vec![]),
            mir_callback("Serial", "tx_byte", trivial_mir_body(), vec![]),
        ],
        query_flavor: MirQueryFlavor::DropsElaborated,
    };

    let input = make_input(bodies);
    let report = check_body_readiness(&input);

    assert_eq!(report.mir_form_summary.fallible_pattern_count, 0);
}

// ============================================================
// Test 7: no analysis-local manual reconstruction needed
// ============================================================

#[test]
fn mir_body_data_comes_from_model_not_analysis() {
    let bodies = representative_mir_bodies();
    let expected_count = bodies.functions.len();
    let expected_names: Vec<String> = bodies.functions.iter().map(|f| f.name.clone()).collect();

    let input = make_input(bodies);

    assert_eq!(input.mir_bodies().functions.len(), expected_count);
    let actual_names: Vec<String> = input
        .mir_bodies()
        .functions
        .iter()
        .map(|f| f.name.clone())
        .collect();
    assert_eq!(actual_names, expected_names);
}

#[test]
fn mir_body_readiness_report_counts_are_correct() {
    let input = make_input(representative_mir_bodies());
    let report = check_body_readiness(&input);

    assert_eq!(report.callback_body_count, 3);
    assert_eq!(report.helper_body_count, 4);
    assert_eq!(report.isr_body_count, 1);
    // Reachable: setup_device, shutdown_device, configure_hw, service_rx
    assert_eq!(report.reachable_helper_count, 4);
    assert_eq!(report.unsupported_node_count, 0);
}

#[test]
fn empty_mir_bodies_with_no_callbacks_is_not_error() {
    let mut decl = decl_with_mir_bodies(MirBodyDeclSet {
        functions: vec![],
        query_flavor: MirQueryFlavor::DropsElaborated,
    });
    decl.callbacks = CallbackDeclSet {
        lifecycle_impls: vec![],
        serial_impls: vec![],
        serial_console_impls: vec![],
        power_management_impls: vec![],
    };
    let binding = minimal_binding();
    let joined = perform_join(&decl, &binding).expect("join should succeed");
    let input = AnalysisInput::new(decl, joined, None);

    let report = check_body_readiness(&input);
    let body_errors: Vec<_> = report
        .errors()
        .iter()
        .filter(|f| f.category == "body_coverage")
        .cloned()
        .collect();
    assert!(body_errors.is_empty());
}

#[test]
fn report_determinism() {
    let input1 = make_input(representative_mir_bodies());
    let input2 = make_input(representative_mir_bodies());

    let report1 = check_body_readiness(&input1);
    let report2 = check_body_readiness(&input2);

    assert_eq!(report1, report2);
}

// ============================================================
// MIR form summary explicit-subset tests
// ============================================================

#[test]
fn mir_form_summary_enumerates_statement_kinds() {
    let input = make_input(representative_mir_bodies());
    let report = check_body_readiness(&input);
    let summary = &report.mir_form_summary;

    assert!(summary.query_flavor_valid);
    // The representative bodies include Assign statements.
    assert!(summary.statement_kinds.contains_key("Assign"));
}

#[test]
fn mir_form_summary_enumerates_terminator_kinds() {
    let input = make_input(representative_mir_bodies());
    let report = check_body_readiness(&input);
    let summary = &report.mir_form_summary;

    assert!(summary.terminator_kinds.contains_key("Return"));
    assert!(summary.terminator_kinds.contains_key("Call"));
    assert!(summary.terminator_kinds.contains_key("SwitchInt"));
    assert!(summary.terminator_kinds.contains_key("Drop"));
    assert!(summary.terminator_kinds.contains_key("UnwindResume"));
}

#[test]
fn mir_form_summary_enumerates_rvalue_kinds() {
    let input = make_input(representative_mir_bodies());
    let report = check_body_readiness(&input);
    let summary = &report.mir_form_summary;

    assert!(summary.rvalue_kinds.contains_key("Discriminant"));
    assert!(summary.rvalue_kinds.contains_key("BinaryOp"));
}
