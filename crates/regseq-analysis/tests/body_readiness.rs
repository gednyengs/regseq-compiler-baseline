//! Tests for authored body capture exposure and supported-subset validation.
//!
//! Verifies that AnalysisInput exposes per-function body data, computes
//! helper reachability, and that body readiness validation catches
//! missing, unsupported, and provenance-deficient body features.

use regseq_analysis::{check_body_readiness, AnalysisInput};
use regseq_model::binding::{
    BindingHeader, BindingModel, BindingProvenance, InterruptBinding, JoinKey, MmioBinding,
    PropertyBinding,
};
use regseq_model::declaration::bodies::{BodyDeclSet, BodyExpr, BodyStmt, FnBodyDecl, FnKind};
use regseq_model::declaration::callbacks::{
    CallbackContext, CallbackDeclSet, CallbackFamily, CallbackImplDecl,
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
    BusType, DeclarationBuildContext, DeclarationModel, DeviceClass, DriverDecl, FnParamDecl,
};
use regseq_model::joined::perform_join;
use regseq_model::platform::PlatformValue;

// ---- Fixture builders ----

/// Build a minimal declaration model with body data for testing.
///
/// Uses generic device-neutral names to avoid PL011-specific coupling.
fn decl_with_bodies(bodies: BodyDeclSet) -> DeclarationModel {
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
        bodies,
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

fn make_input(bodies: BodyDeclSet) -> AnalysisInput {
    let decl = decl_with_bodies(bodies);
    let binding = minimal_binding();
    let joined = perform_join(&decl, &binding).expect("join should succeed");
    AnalysisInput::new(decl, joined, None)
}

// ---- Body expression helpers ----

/// A simple method call: `self.method_name()`.
fn self_method_call(method: &str) -> BodyExpr {
    BodyExpr::MethodCall {
        receiver: Box::new(BodyExpr::Path {
            path: "self".into(),
        }),
        method: method.into(),
        type_args: vec![],
        args: vec![],
    }
}

/// `Ok(())` expression.
fn ok_unit() -> BodyExpr {
    BodyExpr::Call {
        path: "Ok".into(),
        type_args: vec![],
        args: vec![BodyExpr::Tuple { elements: vec![] }],
    }
}

/// A block with semicolon statements and an optional trailing expr.
fn block(stmts: Vec<BodyExpr>, trailing: Option<BodyExpr>) -> BodyExpr {
    BodyExpr::Block {
        stmts: stmts
            .into_iter()
            .map(|expr| BodyStmt::Semi { expr })
            .collect(),
        expr: trailing.map(Box::new),
    }
}

/// Build a callback body declaration.
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

/// Build a helper body declaration.
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

/// Build an ISR body declaration.
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

/// Build a free function body declaration.
fn free_fn_body(name: &str, params: Vec<(&str, &str)>, body: BodyExpr) -> FnBodyDecl {
    FnBodyDecl {
        name: name.into(),
        kind: FnKind::FreeFunction,
        receiver: "none".into(),
        params: params
            .into_iter()
            .map(|(n, t)| FnParamDecl {
                name: n.into(),
                ty: t.into(),
            })
            .collect(),
        return_type: None,
        body,
        provenance: Provenance::new("bodies").with_containing_item(name),
    }
}

/// Build a representative set of bodies with callbacks, helpers, ISR.
fn representative_bodies() -> BodyDeclSet {
    BodyDeclSet {
        functions: vec![
            // Lifecycle::init calls setup_device
            callback_body(
                "Lifecycle",
                "init",
                block(
                    vec![BodyExpr::Try {
                        expr: Box::new(self_method_call("setup_device")),
                    }],
                    Some(ok_unit()),
                ),
            ),
            // Lifecycle::deinit calls shutdown_device
            callback_body(
                "Lifecycle",
                "deinit",
                block(vec![self_method_call("shutdown_device")], Some(ok_unit())),
            ),
            // Serial::tx_byte -- no helper calls
            callback_body(
                "Serial",
                "tx_byte",
                block(
                    vec![BodyExpr::MethodCall {
                        receiver: Box::new(BodyExpr::Field {
                            base: Box::new(BodyExpr::Path {
                                path: "self".into(),
                            }),
                            name: "regs".into(),
                        }),
                        method: "write".into(),
                        type_args: vec![],
                        args: vec![BodyExpr::Path {
                            path: "byte".into(),
                        }],
                    }],
                    None,
                ),
            ),
            // ISR: handle_irq calls service_rx
            isr_body(
                "handle_irq",
                block(
                    vec![self_method_call("service_rx")],
                    Some(BodyExpr::Path {
                        path: "IrqResult::Handled".into(),
                    }),
                ),
            ),
            // Helper: setup_device calls configure_hw (transitive)
            helper_body(
                "setup_device",
                block(vec![self_method_call("configure_hw")], None),
            ),
            // Helper: shutdown_device -- leaf
            helper_body(
                "shutdown_device",
                block(
                    vec![BodyExpr::MethodCall {
                        receiver: Box::new(BodyExpr::Field {
                            base: Box::new(BodyExpr::Path {
                                path: "self".into(),
                            }),
                            name: "regs".into(),
                        }),
                        method: "write".into(),
                        type_args: vec![],
                        args: vec![BodyExpr::LitInt {
                            value: "0".into(),
                            suffix: None,
                        }],
                    }],
                    None,
                ),
            ),
            // Helper: configure_hw -- leaf helper (transitively reachable)
            helper_body(
                "configure_hw",
                block(
                    vec![BodyExpr::MethodCall {
                        receiver: Box::new(BodyExpr::Field {
                            base: Box::new(BodyExpr::Path {
                                path: "self".into(),
                            }),
                            name: "regs".into(),
                        }),
                        method: "write".into(),
                        type_args: vec![],
                        args: vec![BodyExpr::LitInt {
                            value: "1".into(),
                            suffix: None,
                        }],
                    }],
                    None,
                ),
            ),
            // Helper: service_rx -- reachable from ISR
            helper_body(
                "service_rx",
                block(
                    vec![BodyExpr::MethodCall {
                        receiver: Box::new(BodyExpr::Field {
                            base: Box::new(BodyExpr::Path {
                                path: "self".into(),
                            }),
                            name: "regs".into(),
                        }),
                        method: "read".into(),
                        type_args: vec![],
                        args: vec![],
                    }],
                    None,
                ),
            ),
            // Free function: compute_divisor (reachable from setup_device if we add
            // a call, but here standalone for coverage of the free function kind)
            free_fn_body(
                "compute_divisor",
                vec![("clock_hz", "u32"), ("baud", "u32")],
                BodyExpr::Binary {
                    op: "/".into(),
                    lhs: Box::new(BodyExpr::Path {
                        path: "clock_hz".into(),
                    }),
                    rhs: Box::new(BodyExpr::Path {
                        path: "baud".into(),
                    }),
                },
            ),
        ],
    }
}

// ============================================================
// Test 1: AnalysisInput exposes per-function authored body capture
// ============================================================

#[test]
fn bodies_exposed_through_analysis_input() {
    let input = make_input(representative_bodies());

    assert!(!input.bodies().functions.is_empty());
    assert_eq!(input.bodies().functions.len(), 9);
}

#[test]
fn callback_bodies_filtered_correctly() {
    let input = make_input(representative_bodies());
    let cbs = input.callback_bodies();

    assert_eq!(cbs.len(), 3);
    assert!(cbs.iter().all(|b| b.is_callback()));

    let names: Vec<&str> = cbs.iter().map(|b| b.name.as_str()).collect();
    assert!(names.contains(&"init"));
    assert!(names.contains(&"deinit"));
    assert!(names.contains(&"tx_byte"));
}

#[test]
fn helper_bodies_filtered_correctly() {
    let input = make_input(representative_bodies());
    let helpers = input.helper_bodies();

    assert_eq!(helpers.len(), 4);
    assert!(helpers.iter().all(|b| b.is_helper()));
}

#[test]
fn isr_bodies_filtered_correctly() {
    let input = make_input(representative_bodies());
    let isrs = input.isr_bodies();

    assert_eq!(isrs.len(), 1);
    assert_eq!(isrs[0].name, "handle_irq");
    assert!(isrs[0].is_isr());
}

#[test]
fn free_function_bodies_filtered_correctly() {
    let input = make_input(representative_bodies());
    let ffs = input.free_function_bodies();

    assert_eq!(ffs.len(), 1);
    assert_eq!(ffs[0].name, "compute_divisor");
    assert!(ffs[0].is_free_function());
}

#[test]
fn body_by_name_lookup() {
    let input = make_input(representative_bodies());

    let init = input.body_by_name("init").expect("init should exist");
    assert!(init.is_callback());

    let helper = input
        .body_by_name("setup_device")
        .expect("setup_device should exist");
    assert!(helper.is_helper());

    assert!(input.body_by_name("nonexistent").is_none());
}

// ============================================================
// Test 2: reachable helpers from the supported slice are visible
// ============================================================

#[test]
fn reachable_helpers_from_callbacks() {
    let input = make_input(representative_bodies());
    let reachable = input.reachable_helper_names();

    // Directly called from callbacks:
    assert!(reachable.contains("setup_device"));
    assert!(reachable.contains("shutdown_device"));
    // Transitively reachable from setup_device:
    assert!(reachable.contains("configure_hw"));
    // Called from ISR:
    assert!(reachable.contains("service_rx"));
}

#[test]
fn unreachable_free_function_not_in_reachable_set() {
    let input = make_input(representative_bodies());
    let reachable = input.reachable_helper_names();

    // compute_divisor is not called by any callback or ISR in this fixture.
    assert!(!reachable.contains("compute_divisor"));
}

#[test]
fn transitive_reachability_works() {
    // configure_hw is only reachable via setup_device (called by init).
    let input = make_input(representative_bodies());
    let reachable = input.reachable_helper_names();
    assert!(reachable.contains("configure_hw"));
}

// ============================================================
// Test 3: supported-subset validation catches missing/unsupported
// ============================================================

#[test]
fn clean_bodies_pass_validation() {
    let input = make_input(representative_bodies());
    let report = check_body_readiness(&input);

    assert!(!report.has_errors());
    assert_eq!(report.unsupported_node_count, 0);
    assert_eq!(report.callback_body_count, 3);
    assert_eq!(report.helper_body_count, 4);
    assert_eq!(report.isr_body_count, 1);
}

#[test]
fn missing_callback_bodies_detected() {
    // Declare Lifecycle and Serial families but provide no bodies.
    let input = make_input(BodyDeclSet { functions: vec![] });
    let report = check_body_readiness(&input);

    assert!(report.has_errors());
    let errors = report.errors();
    // Both Lifecycle and Serial families missing bodies.
    let body_coverage_errors: Vec<_> = errors
        .iter()
        .filter(|f| f.category == "body_coverage")
        .collect();
    assert_eq!(body_coverage_errors.len(), 2);
}

#[test]
fn reachable_helper_count_excludes_absent_callees() {
    // Callback calls helper_a which calls helper_b, but helper_b has no
    // body in the set. Reachable helpers only includes functions that
    // exist in the body set.
    let bodies = BodyDeclSet {
        functions: vec![
            callback_body(
                "Lifecycle",
                "init",
                block(vec![self_method_call("helper_a")], Some(ok_unit())),
            ),
            callback_body("Serial", "tx_byte", block(vec![], None)),
            helper_body("helper_a", block(vec![self_method_call("helper_b")], None)),
            // helper_b is NOT in the body set.
        ],
    };

    let input = make_input(bodies);
    let report = check_body_readiness(&input);

    // helper_a is reachable and has a body; helper_b is absent.
    assert!(!report.has_errors());
    assert_eq!(report.reachable_helper_count, 1);
}

#[test]
fn unsupported_nodes_surfaced() {
    let bodies = BodyDeclSet {
        functions: vec![
            callback_body(
                "Lifecycle",
                "init",
                block(
                    vec![BodyExpr::Unsupported {
                        hir_kind: "InlineAsm".into(),
                        span_text: Some("asm!(...)".into()),
                    }],
                    Some(ok_unit()),
                ),
            ),
            callback_body("Serial", "tx_byte", block(vec![], None)),
        ],
    };

    let input = make_input(bodies);
    let report = check_body_readiness(&input);

    assert!(report.has_cautions());
    assert_eq!(report.unsupported_node_count, 1);

    let cautions = report.cautions();
    let unsupported_findings: Vec<_> = cautions
        .iter()
        .filter(|f| f.category == "unsupported_construct")
        .collect();
    assert_eq!(unsupported_findings.len(), 1);
    assert!(unsupported_findings[0].message.contains("InlineAsm"));
    assert!(unsupported_findings[0].message.contains("init"));
}

#[test]
fn nested_unsupported_nodes_found() {
    // Unsupported node nested inside an if-then branch.
    let bodies = BodyDeclSet {
        functions: vec![
            callback_body(
                "Lifecycle",
                "init",
                BodyExpr::Block {
                    stmts: vec![BodyStmt::Semi {
                        expr: BodyExpr::If {
                            cond: Box::new(BodyExpr::LitBool { value: true }),
                            then_branch: Box::new(BodyExpr::Unsupported {
                                hir_kind: "Yield".into(),
                                span_text: None,
                            }),
                            else_branch: None,
                        },
                    }],
                    expr: Some(Box::new(ok_unit())),
                },
            ),
            callback_body("Serial", "tx_byte", block(vec![], None)),
        ],
    };

    let input = make_input(bodies);
    let report = check_body_readiness(&input);

    assert_eq!(report.unsupported_node_count, 1);
    let cautions = report.cautions();
    assert!(cautions
        .iter()
        .any(|f| f.message.contains("Yield") && f.message.contains("init")));
}

// ============================================================
// Test 4: body data remains traceable to authored source/provenance
// ============================================================

#[test]
fn provenance_present_on_all_bodies() {
    let input = make_input(representative_bodies());

    for func in &input.bodies().functions {
        assert_eq!(func.provenance.raw_family, "bodies");
        assert!(
            func.provenance.containing_item.is_some(),
            "function '{}' should have containing_item provenance",
            func.name,
        );
        assert_eq!(
            func.provenance.containing_item.as_deref(),
            Some(func.name.as_str()),
        );
    }
}

#[test]
fn empty_provenance_produces_caution() {
    let bodies = BodyDeclSet {
        functions: vec![
            callback_body("Lifecycle", "init", block(vec![], Some(ok_unit()))),
            callback_body("Serial", "tx_byte", block(vec![], None)),
            FnBodyDecl {
                name: "bad_provenance".into(),
                kind: FnKind::Helper,
                receiver: "&mut self".into(),
                params: vec![],
                return_type: None,
                body: block(vec![], None),
                provenance: Provenance::new(""), // empty raw_family
            },
        ],
    };

    let input = make_input(bodies);
    let report = check_body_readiness(&input);

    let cautions = report.cautions();
    let prov_findings: Vec<_> = cautions
        .iter()
        .filter(|f| f.category == "body_provenance")
        .collect();
    assert_eq!(prov_findings.len(), 1);
    assert!(prov_findings[0].message.contains("bad_provenance"));
}

// ============================================================
// Test 5: no analysis-local manual reconstruction needed
// ============================================================

#[test]
fn body_data_comes_from_model_not_analysis() {
    // The body data accessible through AnalysisInput is the exact same
    // data stored in DeclarationModel.bodies. Analysis does not
    // reconstruct or synthesize body trees.
    let bodies = representative_bodies();
    let expected_count = bodies.functions.len();
    let expected_names: Vec<String> = bodies.functions.iter().map(|f| f.name.clone()).collect();

    let input = make_input(bodies);

    // The AnalysisInput body accessors delegate to the model.
    assert_eq!(input.bodies().functions.len(), expected_count);
    let actual_names: Vec<String> = input
        .bodies()
        .functions
        .iter()
        .map(|f| f.name.clone())
        .collect();
    assert_eq!(actual_names, expected_names);
}

#[test]
fn body_readiness_report_counts_are_correct() {
    let input = make_input(representative_bodies());
    let report = check_body_readiness(&input);

    assert_eq!(report.callback_body_count, 3);
    assert_eq!(report.helper_body_count, 4);
    assert_eq!(report.isr_body_count, 1);
    // Reachable: setup_device, shutdown_device, configure_hw, service_rx
    assert_eq!(report.reachable_helper_count, 4);
    assert_eq!(report.unsupported_node_count, 0);
}

#[test]
fn empty_bodies_with_no_callbacks_is_not_error() {
    // If no callback families are declared, empty bodies is fine.
    let mut decl = decl_with_bodies(BodyDeclSet { functions: vec![] });
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
    // No callback families present, so no body coverage errors.
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
    let input1 = make_input(representative_bodies());
    let input2 = make_input(representative_bodies());

    let report1 = check_body_readiness(&input1);
    let report2 = check_body_readiness(&input2);

    assert_eq!(report1, report2);
}
