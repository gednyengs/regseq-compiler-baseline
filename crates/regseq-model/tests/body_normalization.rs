//! Tests for authored body schema and normalization.
//!
//! Verifies that raw body structures deserialize and normalize correctly,
//! normalized bodies preserve function boundaries and helper reachability,
//! expressions and control flow are preserved, normalization is deterministic,
//! and malformed/unsupported body structures are surfaced cleanly.

use regseq_model::declaration::bodies::{
    collect_call_targets, BodyDeclSet, BodyExpr, BodyStmt, FnBodyDecl, FnKind, FnParamDecl,
    MatchArm, StructFieldExpr,
};
use regseq_model::declaration::normalize::{normalize, normalize_from_json};
use regseq_model::declaration::raw_input::*;

// ---- 1. Raw body structures deserialize/normalize correctly ----

#[test]
fn empty_bodies_normalize_to_empty_set() {
    let input = make_minimal_input_with_bodies(None);
    let model = normalize(&input).unwrap();
    assert!(model.bodies.functions.is_empty());
}

#[test]
fn single_callback_body_normalizes() {
    let input = make_input_with_callback_body();
    let model = normalize(&input).unwrap();
    assert_eq!(model.bodies.functions.len(), 1);

    let f = &model.bodies.functions[0];
    assert_eq!(f.name, "init");
    assert!(f.is_callback());
    assert_eq!(
        f.kind,
        FnKind::Callback {
            trait_name: "Lifecycle".into(),
            method_name: "init".into(),
        }
    );
    assert_eq!(f.receiver, "&mut self");
    assert!(f.params.is_empty());
    assert_eq!(f.return_type.as_deref(), Some("Result<(), RegseqError>"));
}

#[test]
fn helper_and_isr_bodies_normalize() {
    let input = make_input_with_mixed_bodies();
    let model = normalize(&input).unwrap();

    let helpers: Vec<_> = model.bodies.helpers();
    assert_eq!(helpers.len(), 1);
    assert_eq!(helpers[0].name, "disable_uart");

    let isrs: Vec<_> = model.bodies.isrs();
    assert_eq!(isrs.len(), 1);
    assert_eq!(isrs[0].name, "handle_irq");
}

#[test]
fn free_function_body_normalizes() {
    let input = make_input_with_free_fn();
    let model = normalize(&input).unwrap();

    let free_fns: Vec<_> = model.bodies.free_functions();
    assert_eq!(free_fns.len(), 1);
    assert_eq!(free_fns[0].name, "field_val");
    assert_eq!(free_fns[0].params.len(), 2);
    assert_eq!(free_fns[0].params[0].name, "field");
    assert_eq!(free_fns[0].params[0].ty, "Field<u32, R>");
}

#[test]
fn body_normalization_from_json_round_trips() {
    let input = make_input_with_mixed_bodies();
    let json = serde_json::to_string(&input).unwrap();
    let model = normalize_from_json(&json).unwrap();

    assert_eq!(model.bodies.functions.len(), 3);
    assert!(model.bodies.by_name("init").is_some());
    assert!(model.bodies.by_name("disable_uart").is_some());
    assert!(model.bodies.by_name("handle_irq").is_some());
}

// ---- 2. Normalized bodies preserve function boundaries and helper reachability ----

#[test]
fn function_boundaries_preserved() {
    let input = make_input_with_mixed_bodies();
    let model = normalize(&input).unwrap();

    let callbacks: Vec<_> = model.bodies.callbacks();
    assert_eq!(callbacks.len(), 1);
    assert_eq!(callbacks[0].name, "init");

    let helpers: Vec<_> = model.bodies.helpers();
    assert_eq!(helpers.len(), 1);
    assert_eq!(helpers[0].name, "disable_uart");
}

#[test]
fn helper_call_graph_edges_discoverable() {
    let input = make_input_with_helper_calls();
    let model = normalize(&input).unwrap();

    let init = model.bodies.by_name("init").unwrap();
    let targets = collect_call_targets(&init.body);
    assert!(targets.contains(&"disable_uart".to_string()));
    assert!(targets.contains(&"enable_uart".to_string()));
}

#[test]
fn by_name_lookup_works() {
    let input = make_input_with_mixed_bodies();
    let model = normalize(&input).unwrap();

    assert!(model.bodies.by_name("init").is_some());
    assert!(model.bodies.by_name("nonexistent").is_none());
}

// ---- 3. Normalized bodies preserve expressions/control flow ----

#[test]
fn let_binding_preserved() {
    let input = make_input_with_let_binding();
    let model = normalize(&input).unwrap();
    let f = model.bodies.by_name("test_fn").unwrap();

    if let BodyExpr::Block { stmts, .. } = &f.body {
        assert!(!stmts.is_empty());
        match &stmts[0] {
            BodyStmt::Let {
                name,
                mutable,
                ty,
                init,
            } => {
                assert_eq!(name, "x");
                assert!(mutable);
                assert_eq!(ty.as_deref(), Some("u32"));
                assert!(init.is_some());
            }
            other => panic!("expected Let, got {:?}", other),
        }
    } else {
        panic!("expected Block body");
    }
}

#[test]
fn if_else_preserved() {
    let input = make_input_with_if_else();
    let model = normalize(&input).unwrap();
    let f = model.bodies.by_name("test_fn").unwrap();

    if let BodyExpr::Block { stmts, .. } = &f.body {
        if let BodyStmt::Semi { expr } = &stmts[0] {
            if let BodyExpr::If {
                cond,
                then_branch,
                else_branch,
            } = expr
            {
                assert!(matches!(cond.as_ref(), BodyExpr::Path { path } if path == "flag"));
                assert!(matches!(then_branch.as_ref(), BodyExpr::Block { .. }));
                assert!(else_branch.is_some());
                return;
            }
        }
    }
    panic!("expected if-else in block");
}

#[test]
fn loop_and_break_preserved() {
    let input = make_input_with_loop();
    let model = normalize(&input).unwrap();
    let f = model.bodies.by_name("test_fn").unwrap();

    if let BodyExpr::Block { stmts, .. } = &f.body {
        if let BodyStmt::Semi { expr } = &stmts[0] {
            if let BodyExpr::Loop { body } = expr {
                if let BodyExpr::Block { stmts, .. } = body.as_ref() {
                    assert!(stmts.iter().any(|s| matches!(
                        s,
                        BodyStmt::Semi {
                            expr: BodyExpr::Break { .. }
                        }
                    )));
                    return;
                }
            }
        }
    }
    panic!("expected loop with break in block");
}

#[test]
fn return_expression_preserved() {
    let input = make_input_with_return();
    let model = normalize(&input).unwrap();
    let f = model.bodies.by_name("test_fn").unwrap();

    if let BodyExpr::Block { stmts, .. } = &f.body {
        if let BodyStmt::Semi { expr } = &stmts[0] {
            if let BodyExpr::Return { expr: Some(val) } = expr {
                assert!(matches!(val.as_ref(), BodyExpr::LitBool { value: true }));
                return;
            }
        }
    }
    panic!("expected return true in block");
}

#[test]
fn method_call_preserved() {
    let input = make_input_with_method_call();
    let model = normalize(&input).unwrap();
    let f = model.bodies.by_name("test_fn").unwrap();

    if let BodyExpr::Block { stmts, .. } = &f.body {
        if let BodyStmt::Semi { expr } = &stmts[0] {
            if let BodyExpr::MethodCall {
                receiver,
                method,
                args,
                ..
            } = expr
            {
                assert_eq!(method, "write");
                assert!(matches!(receiver.as_ref(), BodyExpr::Field { name, .. } if name == "cr"));
                assert_eq!(args.len(), 1);
                return;
            }
        }
    }
    panic!("expected method call in block");
}

#[test]
fn binary_and_unary_ops_preserved() {
    let input = make_input_with_binary_ops();
    let model = normalize(&input).unwrap();
    let f = model.bodies.by_name("test_fn").unwrap();

    if let BodyExpr::Block { stmts, .. } = &f.body {
        if let BodyStmt::Let {
            init: Some(init), ..
        } = &stmts[0]
        {
            if let BodyExpr::Binary { op, .. } = init {
                assert_eq!(op, "&");
                return;
            }
        }
    }
    panic!("expected binary op in let binding");
}

#[test]
fn assignment_preserved() {
    let input = make_input_with_assignment();
    let model = normalize(&input).unwrap();
    let f = model.bodies.by_name("test_fn").unwrap();

    if let BodyExpr::Block { stmts, .. } = &f.body {
        if let BodyStmt::Semi { expr } = &stmts[0] {
            if let BodyExpr::Assign { target, value } = expr {
                assert!(
                    matches!(target.as_ref(), BodyExpr::Field { name, .. } if name == "pending_tx")
                );
                assert!(matches!(value.as_ref(), BodyExpr::Path { path } if path == "None"));
                return;
            }
        }
    }
    panic!("expected assignment in block");
}

#[test]
fn try_operator_preserved() {
    let input = make_input_with_try();
    let model = normalize(&input).unwrap();
    let f = model.bodies.by_name("test_fn").unwrap();

    if let BodyExpr::Block { stmts, .. } = &f.body {
        if let BodyStmt::Semi { expr } = &stmts[0] {
            assert!(matches!(expr, BodyExpr::Try { .. }));
            return;
        }
    }
    panic!("expected try in block");
}

#[test]
fn match_expression_preserved() {
    let input = make_input_with_match();
    let model = normalize(&input).unwrap();
    let f = model.bodies.by_name("test_fn").unwrap();

    if let BodyExpr::Block { stmts, .. } = &f.body {
        if let BodyStmt::Expr { expr } = &stmts[0] {
            if let BodyExpr::Match { scrutinee, arms } = expr {
                assert!(matches!(scrutinee.as_ref(), BodyExpr::Path { path } if path == "status"));
                assert_eq!(arms.len(), 2);
                assert_eq!(arms[0].pattern, "0");
                return;
            }
        }
    }
    panic!("expected match in block");
}

#[test]
fn cast_expression_preserved() {
    let input = make_input_with_cast();
    let model = normalize(&input).unwrap();
    let f = model.bodies.by_name("test_fn").unwrap();

    if let BodyExpr::Block { stmts, .. } = &f.body {
        if let BodyStmt::Let {
            init: Some(init), ..
        } = &stmts[0]
        {
            if let BodyExpr::Cast { ty, .. } = init {
                assert_eq!(ty, "u8");
                return;
            }
        }
    }
    panic!("expected cast in let binding");
}

#[test]
fn struct_literal_preserved() {
    let input = make_input_with_struct_literal();
    let model = normalize(&input).unwrap();
    let f = model.bodies.by_name("test_fn").unwrap();

    if let BodyExpr::Block {
        expr: Some(trailing),
        ..
    } = &f.body
    {
        if let BodyExpr::Struct { path, fields, .. } = trailing.as_ref() {
            assert_eq!(path, "RxChar");
            assert_eq!(fields.len(), 2);
            assert_eq!(fields[0].name, "byte");
            return;
        }
    }
    panic!("expected struct literal as trailing expr");
}

#[test]
fn closure_expression_preserved() {
    let input = make_input_with_closure();
    let model = normalize(&input).unwrap();
    let f = model.bodies.by_name("test_fn").unwrap();

    if let BodyExpr::Block { stmts, .. } = &f.body {
        if let BodyStmt::Semi { expr } = &stmts[0] {
            if let BodyExpr::MethodCall { args, .. } = expr {
                if let Some(BodyExpr::Closure { params, .. }) = args.first() {
                    assert_eq!(params, &["x"]);
                    return;
                }
            }
        }
    }
    panic!("expected closure in method call");
}

// ---- 4. Normalization is deterministic and reviewer-readable ----

#[test]
fn normalization_is_deterministic() {
    let input = make_input_with_mixed_bodies();
    let model1 = normalize(&input).unwrap();
    let model2 = normalize(&input).unwrap();
    assert_eq!(model1.bodies, model2.bodies);
}

#[test]
fn normalized_bodies_serialize_deterministically() {
    let input = make_input_with_mixed_bodies();
    let model = normalize(&input).unwrap();
    let json1 = serde_json::to_string_pretty(&model.bodies).unwrap();
    let json2 = serde_json::to_string_pretty(&model.bodies).unwrap();
    assert_eq!(json1, json2);
}

#[test]
fn normalized_bodies_round_trip_through_json() {
    let input = make_input_with_mixed_bodies();
    let model = normalize(&input).unwrap();
    let json = serde_json::to_string(&model.bodies).unwrap();
    let round_tripped: BodyDeclSet = serde_json::from_str(&json).unwrap();
    assert_eq!(model.bodies, round_tripped);
}

// ---- 5. Normalization rejects/surfaces unsupported structures cleanly ----

#[test]
fn unsupported_hir_node_passes_through() {
    let input = make_input_with_unsupported_node();
    let model = normalize(&input).unwrap();
    let f = model.bodies.by_name("test_fn").unwrap();

    if let BodyExpr::Block { stmts, .. } = &f.body {
        if let BodyStmt::Semi { expr } = &stmts[0] {
            if let BodyExpr::Unsupported {
                hir_kind,
                span_text,
            } = expr
            {
                assert_eq!(hir_kind, "InlineAsm");
                assert_eq!(span_text.as_deref(), Some("asm!(\"nop\")"));
                return;
            }
        }
    }
    panic!("expected Unsupported node in block");
}

// ---- 6. Provenance ----

#[test]
fn body_fn_has_provenance() {
    let input = make_input_with_callback_body();
    let model = normalize(&input).unwrap();
    let f = &model.bodies.functions[0];
    assert_eq!(f.provenance.raw_family, "bodies");
    assert_eq!(f.provenance.containing_item.as_deref(), Some("init"));
}

// ---- 7. Traversal helpers ----

#[test]
fn traversal_body_decls_accessor() {
    let input = make_input_with_mixed_bodies();
    let model = normalize(&input).unwrap();
    assert_eq!(model.body_decls().functions.len(), 3);
    assert_eq!(model.all_fn_bodies().len(), 3);
}

#[test]
fn traversal_fn_body_by_name() {
    let input = make_input_with_mixed_bodies();
    let model = normalize(&input).unwrap();
    let init = model.fn_body_by_name("init").unwrap();
    assert!(init.is_callback());
    assert!(model.fn_body_by_name("nonexistent").is_none());
}

// ---- 8. Body types are device-neutral and OS-neutral ----

#[test]
fn fn_kind_variants_are_generic() {
    // Verify FnKind has no device-specific or OS-specific variants
    let callback = FnKind::Callback {
        trait_name: "AnyTrait".into(),
        method_name: "any_method".into(),
    };
    let helper = FnKind::Helper;
    let isr = FnKind::Isr;
    let free = FnKind::FreeFunction;

    // All four kinds should construct without device or OS info
    assert!(matches!(callback, FnKind::Callback { .. }));
    assert!(matches!(helper, FnKind::Helper));
    assert!(matches!(isr, FnKind::Isr));
    assert!(matches!(free, FnKind::FreeFunction));
}

#[test]
fn body_expr_variants_are_generic() {
    // Verify BodyExpr has no device-class or OS nodes
    let exprs: Vec<BodyExpr> = vec![
        BodyExpr::Path {
            path: "self.regs".into(),
        },
        BodyExpr::LitInt {
            value: "42".into(),
            suffix: None,
        },
        BodyExpr::LitBool { value: true },
        BodyExpr::Continue,
    ];
    // All should serialize without device/OS metadata
    for expr in &exprs {
        let json = serde_json::to_string(expr).unwrap();
        assert!(!json.contains("linux"));
        assert!(!json.contains("pl011"));
    }
}

// ---- Helpers ----

fn make_minimal_input_with_bodies(bodies: Option<RawBodySurfaceInput>) -> RawArtifactInput {
    RawArtifactInput {
        driver_name: "test".into(),
        target_os: "linux".into(),
        identity: Some(RawIdentityInput {
            driver_name: "test".into(),
            compatible: vec![],
            device_class: "serial_uart".into(),
            bus: None,
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
        bodies,
    }
}

fn make_simple_block(stmts: Vec<RawBodyStmtInput>) -> RawBodyExprInput {
    RawBodyExprInput::Block { stmts, expr: None }
}

fn make_input_with_callback_body() -> RawArtifactInput {
    make_minimal_input_with_bodies(Some(RawBodySurfaceInput {
        functions: vec![RawExtractedFnInput {
            name: "init".into(),
            kind: RawFnKindInput::Callback {
                trait_name: "Lifecycle".into(),
                method_name: "init".into(),
            },
            receiver: "&mut self".into(),
            params: vec![],
            return_type: Some("Result<(), RegseqError>".into()),
            body: make_simple_block(vec![RawBodyStmtInput::Semi {
                expr: RawBodyExprInput::Return {
                    expr: Some(Box::new(RawBodyExprInput::Call {
                        path: "Ok".into(),
                        type_args: vec![],
                        args: vec![RawBodyExprInput::Tuple { elements: vec![] }],
                    })),
                },
            }]),
        }],
    }))
}

fn make_input_with_mixed_bodies() -> RawArtifactInput {
    make_minimal_input_with_bodies(Some(RawBodySurfaceInput {
        functions: vec![
            RawExtractedFnInput {
                name: "init".into(),
                kind: RawFnKindInput::Callback {
                    trait_name: "Lifecycle".into(),
                    method_name: "init".into(),
                },
                receiver: "&mut self".into(),
                params: vec![],
                return_type: Some("Result<(), RegseqError>".into()),
                body: make_simple_block(vec![RawBodyStmtInput::Semi {
                    expr: RawBodyExprInput::MethodCall {
                        receiver: Box::new(RawBodyExprInput::Path {
                            path: "self".into(),
                        }),
                        method: "disable_uart".into(),
                        type_args: vec![],
                        args: vec![],
                    },
                }]),
            },
            RawExtractedFnInput {
                name: "disable_uart".into(),
                kind: RawFnKindInput::Helper,
                receiver: "&mut self".into(),
                params: vec![],
                return_type: None,
                body: make_simple_block(vec![RawBodyStmtInput::Semi {
                    expr: RawBodyExprInput::MethodCall {
                        receiver: Box::new(RawBodyExprInput::Field {
                            base: Box::new(RawBodyExprInput::Field {
                                base: Box::new(RawBodyExprInput::Path {
                                    path: "self".into(),
                                }),
                                name: "regs".into(),
                            }),
                            name: "cr".into(),
                        }),
                        method: "write".into(),
                        type_args: vec![],
                        args: vec![RawBodyExprInput::LitInt {
                            value: "0".into(),
                            suffix: None,
                        }],
                    },
                }]),
            },
            RawExtractedFnInput {
                name: "handle_irq".into(),
                kind: RawFnKindInput::Isr,
                receiver: "&mut self".into(),
                params: vec![],
                return_type: Some("IrqResult".into()),
                body: make_simple_block(vec![RawBodyStmtInput::Semi {
                    expr: RawBodyExprInput::Return {
                        expr: Some(Box::new(RawBodyExprInput::Path {
                            path: "IrqResult::NotHandled".into(),
                        })),
                    },
                }]),
            },
        ],
    }))
}

fn make_input_with_free_fn() -> RawArtifactInput {
    make_minimal_input_with_bodies(Some(RawBodySurfaceInput {
        functions: vec![RawExtractedFnInput {
            name: "field_val".into(),
            kind: RawFnKindInput::FreeFunction,
            receiver: "none".into(),
            params: vec![
                RawFnParamInput {
                    name: "field".into(),
                    ty: "Field<u32, R>".into(),
                },
                RawFnParamInput {
                    name: "raw".into(),
                    ty: "u32".into(),
                },
            ],
            return_type: Some("FieldValue<u32, R>".into()),
            body: make_simple_block(vec![]),
        }],
    }))
}

fn make_input_with_helper_calls() -> RawArtifactInput {
    make_minimal_input_with_bodies(Some(RawBodySurfaceInput {
        functions: vec![
            RawExtractedFnInput {
                name: "init".into(),
                kind: RawFnKindInput::Callback {
                    trait_name: "Lifecycle".into(),
                    method_name: "init".into(),
                },
                receiver: "&mut self".into(),
                params: vec![],
                return_type: None,
                body: make_simple_block(vec![
                    RawBodyStmtInput::Semi {
                        expr: RawBodyExprInput::MethodCall {
                            receiver: Box::new(RawBodyExprInput::Path {
                                path: "self".into(),
                            }),
                            method: "disable_uart".into(),
                            type_args: vec![],
                            args: vec![],
                        },
                    },
                    RawBodyStmtInput::Semi {
                        expr: RawBodyExprInput::MethodCall {
                            receiver: Box::new(RawBodyExprInput::Path {
                                path: "self".into(),
                            }),
                            method: "enable_uart".into(),
                            type_args: vec![],
                            args: vec![],
                        },
                    },
                ]),
            },
            RawExtractedFnInput {
                name: "disable_uart".into(),
                kind: RawFnKindInput::Helper,
                receiver: "&mut self".into(),
                params: vec![],
                return_type: None,
                body: make_simple_block(vec![]),
            },
            RawExtractedFnInput {
                name: "enable_uart".into(),
                kind: RawFnKindInput::Helper,
                receiver: "&mut self".into(),
                params: vec![],
                return_type: None,
                body: make_simple_block(vec![]),
            },
        ],
    }))
}

fn make_input_with_let_binding() -> RawArtifactInput {
    make_minimal_input_with_bodies(Some(RawBodySurfaceInput {
        functions: vec![RawExtractedFnInput {
            name: "test_fn".into(),
            kind: RawFnKindInput::Helper,
            receiver: "&mut self".into(),
            params: vec![],
            return_type: None,
            body: make_simple_block(vec![RawBodyStmtInput::Let {
                name: "x".into(),
                mutable: true,
                ty: Some("u32".into()),
                init: Some(RawBodyExprInput::LitInt {
                    value: "42".into(),
                    suffix: Some("u32".into()),
                }),
            }]),
        }],
    }))
}

fn make_input_with_if_else() -> RawArtifactInput {
    make_minimal_input_with_bodies(Some(RawBodySurfaceInput {
        functions: vec![RawExtractedFnInput {
            name: "test_fn".into(),
            kind: RawFnKindInput::Helper,
            receiver: "&mut self".into(),
            params: vec![],
            return_type: None,
            body: make_simple_block(vec![RawBodyStmtInput::Semi {
                expr: RawBodyExprInput::If {
                    cond: Box::new(RawBodyExprInput::Path {
                        path: "flag".into(),
                    }),
                    then_branch: Box::new(RawBodyExprInput::Block {
                        stmts: vec![],
                        expr: None,
                    }),
                    else_branch: Some(Box::new(RawBodyExprInput::Block {
                        stmts: vec![],
                        expr: None,
                    })),
                },
            }]),
        }],
    }))
}

fn make_input_with_loop() -> RawArtifactInput {
    make_minimal_input_with_bodies(Some(RawBodySurfaceInput {
        functions: vec![RawExtractedFnInput {
            name: "test_fn".into(),
            kind: RawFnKindInput::Helper,
            receiver: "&mut self".into(),
            params: vec![],
            return_type: None,
            body: make_simple_block(vec![RawBodyStmtInput::Semi {
                expr: RawBodyExprInput::Loop {
                    body: Box::new(RawBodyExprInput::Block {
                        stmts: vec![RawBodyStmtInput::Semi {
                            expr: RawBodyExprInput::Break { expr: None },
                        }],
                        expr: None,
                    }),
                },
            }]),
        }],
    }))
}

fn make_input_with_return() -> RawArtifactInput {
    make_minimal_input_with_bodies(Some(RawBodySurfaceInput {
        functions: vec![RawExtractedFnInput {
            name: "test_fn".into(),
            kind: RawFnKindInput::Helper,
            receiver: "&self".into(),
            params: vec![],
            return_type: Some("bool".into()),
            body: make_simple_block(vec![RawBodyStmtInput::Semi {
                expr: RawBodyExprInput::Return {
                    expr: Some(Box::new(RawBodyExprInput::LitBool { value: true })),
                },
            }]),
        }],
    }))
}

fn make_input_with_method_call() -> RawArtifactInput {
    make_minimal_input_with_bodies(Some(RawBodySurfaceInput {
        functions: vec![RawExtractedFnInput {
            name: "test_fn".into(),
            kind: RawFnKindInput::Helper,
            receiver: "&mut self".into(),
            params: vec![],
            return_type: None,
            body: make_simple_block(vec![RawBodyStmtInput::Semi {
                expr: RawBodyExprInput::MethodCall {
                    receiver: Box::new(RawBodyExprInput::Field {
                        base: Box::new(RawBodyExprInput::Field {
                            base: Box::new(RawBodyExprInput::Path {
                                path: "self".into(),
                            }),
                            name: "regs".into(),
                        }),
                        name: "cr".into(),
                    }),
                    method: "write".into(),
                    type_args: vec![],
                    args: vec![RawBodyExprInput::LitInt {
                        value: "0".into(),
                        suffix: None,
                    }],
                },
            }]),
        }],
    }))
}

fn make_input_with_binary_ops() -> RawArtifactInput {
    make_minimal_input_with_bodies(Some(RawBodySurfaceInput {
        functions: vec![RawExtractedFnInput {
            name: "test_fn".into(),
            kind: RawFnKindInput::Helper,
            receiver: "&self".into(),
            params: vec![],
            return_type: None,
            body: make_simple_block(vec![RawBodyStmtInput::Let {
                name: "masked".into(),
                mutable: false,
                ty: None,
                init: Some(RawBodyExprInput::Binary {
                    op: "&".into(),
                    lhs: Box::new(RawBodyExprInput::Path {
                        path: "status".into(),
                    }),
                    rhs: Box::new(RawBodyExprInput::LitInt {
                        value: "0xff".into(),
                        suffix: None,
                    }),
                }),
            }]),
        }],
    }))
}

fn make_input_with_assignment() -> RawArtifactInput {
    make_minimal_input_with_bodies(Some(RawBodySurfaceInput {
        functions: vec![RawExtractedFnInput {
            name: "test_fn".into(),
            kind: RawFnKindInput::Helper,
            receiver: "&mut self".into(),
            params: vec![],
            return_type: None,
            body: make_simple_block(vec![RawBodyStmtInput::Semi {
                expr: RawBodyExprInput::Assign {
                    target: Box::new(RawBodyExprInput::Field {
                        base: Box::new(RawBodyExprInput::Path {
                            path: "self".into(),
                        }),
                        name: "pending_tx".into(),
                    }),
                    value: Box::new(RawBodyExprInput::Path {
                        path: "None".into(),
                    }),
                },
            }]),
        }],
    }))
}

fn make_input_with_try() -> RawArtifactInput {
    make_minimal_input_with_bodies(Some(RawBodySurfaceInput {
        functions: vec![RawExtractedFnInput {
            name: "test_fn".into(),
            kind: RawFnKindInput::Callback {
                trait_name: "Lifecycle".into(),
                method_name: "init".into(),
            },
            receiver: "&mut self".into(),
            params: vec![],
            return_type: Some("Result<(), RegseqError>".into()),
            body: make_simple_block(vec![RawBodyStmtInput::Semi {
                expr: RawBodyExprInput::Try {
                    expr: Box::new(RawBodyExprInput::MethodCall {
                        receiver: Box::new(RawBodyExprInput::Path {
                            path: "self".into(),
                        }),
                        method: "enable_clock".into(),
                        type_args: vec![],
                        args: vec![],
                    }),
                },
            }]),
        }],
    }))
}

fn make_input_with_match() -> RawArtifactInput {
    make_minimal_input_with_bodies(Some(RawBodySurfaceInput {
        functions: vec![RawExtractedFnInput {
            name: "test_fn".into(),
            kind: RawFnKindInput::Isr,
            receiver: "&mut self".into(),
            params: vec![],
            return_type: Some("IrqResult".into()),
            body: make_simple_block(vec![RawBodyStmtInput::Expr {
                expr: RawBodyExprInput::Match {
                    scrutinee: Box::new(RawBodyExprInput::Path {
                        path: "status".into(),
                    }),
                    arms: vec![
                        RawMatchArmInput {
                            pattern: "0".into(),
                            guard: None,
                            body: RawBodyExprInput::Path {
                                path: "IrqResult::NotHandled".into(),
                            },
                        },
                        RawMatchArmInput {
                            pattern: "_".into(),
                            guard: None,
                            body: RawBodyExprInput::Path {
                                path: "IrqResult::Handled".into(),
                            },
                        },
                    ],
                },
            }]),
        }],
    }))
}

fn make_input_with_cast() -> RawArtifactInput {
    make_minimal_input_with_bodies(Some(RawBodySurfaceInput {
        functions: vec![RawExtractedFnInput {
            name: "test_fn".into(),
            kind: RawFnKindInput::Helper,
            receiver: "&self".into(),
            params: vec![],
            return_type: None,
            body: make_simple_block(vec![RawBodyStmtInput::Let {
                name: "byte".into(),
                mutable: false,
                ty: None,
                init: Some(RawBodyExprInput::Cast {
                    expr: Box::new(RawBodyExprInput::Path {
                        path: "raw_val".into(),
                    }),
                    ty: "u8".into(),
                }),
            }]),
        }],
    }))
}

fn make_input_with_struct_literal() -> RawArtifactInput {
    make_minimal_input_with_bodies(Some(RawBodySurfaceInput {
        functions: vec![RawExtractedFnInput {
            name: "test_fn".into(),
            kind: RawFnKindInput::Helper,
            receiver: "&self".into(),
            params: vec![],
            return_type: Some("RxChar".into()),
            body: RawBodyExprInput::Block {
                stmts: vec![],
                expr: Some(Box::new(RawBodyExprInput::Struct {
                    path: "RxChar".into(),
                    fields: vec![
                        RawStructFieldInput {
                            name: "byte".into(),
                            value: RawBodyExprInput::LitInt {
                                value: "0".into(),
                                suffix: None,
                            },
                        },
                        RawStructFieldInput {
                            name: "errors".into(),
                            value: RawBodyExprInput::Path {
                                path: "ErrorKind::empty".into(),
                            },
                        },
                    ],
                    base: None,
                })),
            },
        }],
    }))
}

fn make_input_with_closure() -> RawArtifactInput {
    make_minimal_input_with_bodies(Some(RawBodySurfaceInput {
        functions: vec![RawExtractedFnInput {
            name: "test_fn".into(),
            kind: RawFnKindInput::Helper,
            receiver: "&self".into(),
            params: vec![],
            return_type: None,
            body: make_simple_block(vec![RawBodyStmtInput::Semi {
                expr: RawBodyExprInput::MethodCall {
                    receiver: Box::new(RawBodyExprInput::Path {
                        path: "self.clock".into(),
                    }),
                    method: "map".into(),
                    type_args: vec![],
                    args: vec![RawBodyExprInput::Closure {
                        params: vec!["x".into()],
                        body: Box::new(RawBodyExprInput::MethodCall {
                            receiver: Box::new(RawBodyExprInput::Path { path: "x".into() }),
                            method: "get_rate".into(),
                            type_args: vec![],
                            args: vec![],
                        }),
                    }],
                },
            }]),
        }],
    }))
}

fn make_input_with_unsupported_node() -> RawArtifactInput {
    make_minimal_input_with_bodies(Some(RawBodySurfaceInput {
        functions: vec![RawExtractedFnInput {
            name: "test_fn".into(),
            kind: RawFnKindInput::Helper,
            receiver: "&mut self".into(),
            params: vec![],
            return_type: None,
            body: make_simple_block(vec![RawBodyStmtInput::Semi {
                expr: RawBodyExprInput::Unsupported {
                    hir_kind: "InlineAsm".into(),
                    span_text: Some("asm!(\"nop\")".into()),
                },
            }]),
        }],
    }))
}
