//! Tests for MIR-derived body normalization.
//!
//! Verifies that MIR body structures from the raw artifact deserialize and
//! normalize into the canonical `MirBodyDeclSet` with CFG/basic-block
//! structure preserved, per-function identity and helper reachability intact,
//! and cleanup/drop semantics available for later lowering.

use regseq_model::declaration::mir_body::*;
use regseq_model::declaration::normalize::normalize;
use regseq_model::declaration::raw_input::*;

// ---------------------------------------------------------------------------
// 1. Empty / absent bodies
// ---------------------------------------------------------------------------

#[test]
fn empty_bodies_normalize_to_empty_mir_set() {
    let input = make_minimal_input(None);
    let model = normalize(&input).unwrap();
    assert!(model.mir_bodies.functions.is_empty());
    assert_eq!(
        model.mir_bodies.query_flavor,
        MirQueryFlavor::DropsElaborated
    );
}

// ---------------------------------------------------------------------------
// 2. Per-function metadata extraction via mir_bodies
// ---------------------------------------------------------------------------

#[test]
fn single_callback_normalizes_mir_metadata() {
    let input = make_input_with_callback();
    let model = normalize(&input).unwrap();
    assert_eq!(model.mir_bodies.functions.len(), 1);

    let f = &model.mir_bodies.functions[0];
    assert_eq!(f.name, "init");
    assert!(f.is_callback());
    assert_eq!(
        f.kind,
        FnKind::Callback {
            trait_name: "Lifecycle".into(),
            method_name: "init".into(),
        }
    );
    assert_eq!(f.receiver, "ref_mut");
    assert!(f.params.is_empty());
    assert_eq!(f.return_type.as_deref(), Some("Result<(), RegseqError>"));
    assert_eq!(f.def_path, "TestDriver::init");
}

#[test]
fn helper_and_isr_normalize_mir_metadata() {
    let input = make_input_with_mixed();
    let model = normalize(&input).unwrap();

    let helpers = model.mir_bodies.helpers();
    assert_eq!(helpers.len(), 1);
    assert_eq!(helpers[0].name, "disable_uart");

    let isrs = model.mir_bodies.isrs();
    assert_eq!(isrs.len(), 1);
    assert_eq!(isrs[0].name, "handle_irq");
}

#[test]
fn callback_params_preserved_in_mir() {
    let input = make_input_with_params();
    let model = normalize(&input).unwrap();

    let f = &model.mir_bodies.functions[0];
    assert_eq!(f.params.len(), 1);
    assert_eq!(f.params[0].name, "byte");
    assert_eq!(f.params[0].ty, "u8");
}

// ---------------------------------------------------------------------------
// 3. MIR body CFG structure is preserved
// ---------------------------------------------------------------------------

#[test]
fn mir_body_preserves_basic_blocks() {
    let input = make_input_with_callback();
    let model = normalize(&input).unwrap();

    let f = &model.mir_bodies.functions[0];
    // Our test MIR body has 1 basic block
    assert_eq!(f.body.basic_blocks.len(), 1);
    let bb0 = &f.body.basic_blocks[0];
    assert!(!bb0.is_cleanup);
    assert!(bb0.terminator.is_some());
    assert!(matches!(
        bb0.terminator.as_ref().unwrap().kind,
        MirTerminatorKind::Return
    ));
}

#[test]
fn mir_body_preserves_local_decls() {
    let input = make_input_with_callback();
    let model = normalize(&input).unwrap();

    let f = &model.mir_bodies.functions[0];
    assert_eq!(f.body.local_decls.len(), 1);
    assert_eq!(f.body.local_decls[0].index, 0);
    assert!(matches!(
        f.body.local_decls[0].ty,
        MirTy::Tuple { ref elements } if elements.is_empty()
    ));
}

#[test]
fn mir_body_preserves_cleanup_blocks() {
    let input = make_input_with_cleanup();
    let model = normalize(&input).unwrap();

    let f = &model.mir_bodies.functions[0];
    assert_eq!(f.body.basic_blocks.len(), 2);
    // bb0 is normal
    assert!(!f.body.basic_blocks[0].is_cleanup);
    // bb1 is cleanup
    assert!(f.body.basic_blocks[1].is_cleanup);
    assert!(matches!(
        f.body.basic_blocks[1].terminator.as_ref().unwrap().kind,
        MirTerminatorKind::UnwindResume
    ));
}

#[test]
fn mir_body_preserves_drop_semantics() {
    let input = make_input_with_drop();
    let model = normalize(&input).unwrap();

    let f = &model.mir_bodies.functions[0];
    let bb0 = &f.body.basic_blocks[0];
    let term = bb0.terminator.as_ref().unwrap();
    match &term.kind {
        MirTerminatorKind::Drop {
            place,
            target,
            unwind,
            replace,
        } => {
            assert_eq!(place.local, 1);
            assert_eq!(*target, 1);
            assert!(matches!(unwind, MirUnwindAction::Cleanup { target: 2 }));
            assert!(!replace);
        }
        other => panic!("expected Drop, got {other:?}"),
    }
}

#[test]
fn mir_body_preserves_statements() {
    let input = make_input_with_statements();
    let model = normalize(&input).unwrap();

    let f = &model.mir_bodies.functions[0];
    let stmts = &f.body.basic_blocks[0].statements;
    assert_eq!(stmts.len(), 3);
    assert!(matches!(
        stmts[0].kind,
        MirStatementKind::StorageLive { local: 1 }
    ));
    assert!(matches!(stmts[1].kind, MirStatementKind::Assign { .. }));
    assert!(matches!(
        stmts[2].kind,
        MirStatementKind::StorageDead { local: 1 }
    ));
}

// ---------------------------------------------------------------------------
// 4. Helper call graph edges preserved
// ---------------------------------------------------------------------------

#[test]
fn helper_calls_preserved_in_mir() {
    let input = make_input_with_callback();
    let model = normalize(&input).unwrap();

    let f = model.mir_bodies.by_name("init").unwrap();
    assert_eq!(f.helper_calls, vec!["disable_uart".to_string()]);
}

// ---------------------------------------------------------------------------
// 5. Lookup and classification
// ---------------------------------------------------------------------------

#[test]
fn mir_by_name_lookup_works() {
    let input = make_input_with_mixed();
    let model = normalize(&input).unwrap();

    assert!(model.mir_bodies.by_name("init").is_some());
    assert!(model.mir_bodies.by_name("disable_uart").is_some());
    assert!(model.mir_bodies.by_name("handle_irq").is_some());
    assert!(model.mir_bodies.by_name("nonexistent").is_none());
}

#[test]
fn mir_classification_queries_work() {
    let input = make_input_with_mixed();
    let model = normalize(&input).unwrap();

    assert_eq!(model.mir_bodies.callbacks().len(), 1);
    assert_eq!(model.mir_bodies.helpers().len(), 1);
    assert_eq!(model.mir_bodies.isrs().len(), 1);
    assert_eq!(model.mir_bodies.free_functions().len(), 0);
}

// ---------------------------------------------------------------------------
// 6. Deterministic normalization
// ---------------------------------------------------------------------------

#[test]
fn mir_normalization_is_deterministic() {
    let input = make_input_with_mixed();
    let m1 = normalize(&input).unwrap();
    let m2 = normalize(&input).unwrap();
    assert_eq!(m1.mir_bodies, m2.mir_bodies);
}

// ---------------------------------------------------------------------------
// 7. Provenance
// ---------------------------------------------------------------------------

#[test]
fn mir_body_provenance_set() {
    let input = make_input_with_callback();
    let model = normalize(&input).unwrap();
    let f = &model.mir_bodies.functions[0];
    assert_eq!(f.provenance.raw_family, "bodies");
}

// ---------------------------------------------------------------------------
// 8. CFG not expression-tree
// ---------------------------------------------------------------------------

#[test]
fn mir_body_is_cfg_not_expression_tree() {
    let input = make_input_with_callback();
    let model = normalize(&input).unwrap();

    let f = &model.mir_bodies.functions[0];
    // Body has basic_blocks (CFG), not a recursive expression tree.
    assert!(!f.body.basic_blocks.is_empty());
    // Each block has a terminator (control flow edge).
    for bb in &f.body.basic_blocks {
        assert!(bb.terminator.is_some());
    }
}

// ---------------------------------------------------------------------------
// 9. Unsupported construct surfacing
// ---------------------------------------------------------------------------

#[test]
fn unsupported_statements_are_surfaced() {
    let input = make_input_with_unsupported();
    let model = normalize(&input).unwrap();

    let f = &model.mir_bodies.functions[0];
    let count = count_unsupported(&f.body);
    assert!(count > 0);
}

// ---------------------------------------------------------------------------
// 10. Query flavor tag
// ---------------------------------------------------------------------------

#[test]
fn query_flavor_is_drops_elaborated() {
    let input = make_input_with_callback();
    let model = normalize(&input).unwrap();
    assert_eq!(
        model.mir_bodies.query_flavor,
        MirQueryFlavor::DropsElaborated
    );
}

// ---------------------------------------------------------------------------
// Test data helpers
// ---------------------------------------------------------------------------

fn minimal_mir_body() -> MirBody {
    MirBody {
        local_decls: vec![MirLocalDecl {
            index: 0,
            ty: MirTy::Tuple { elements: vec![] },
            mutability: MirMutability::Mut,
            name: Some("_0".to_string()),
        }],
        arg_count: 0,
        basic_blocks: vec![MirBasicBlockData {
            statements: vec![],
            terminator: Some(MirTerminator {
                source_info: MirSourceInfo {
                    scope: 0,
                    span: None,
                },
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

fn make_minimal_input(bodies: Option<RawBodySurfaceInput>) -> RawArtifactInput {
    RawArtifactInput {
        driver_name: "test-driver".into(),
        target_os: "linux".into(),
        identity: Some(RawIdentityInput {
            driver_name: "test-driver".into(),
            compatible: vec!["test,driver".into()],
            device_class: "serial".into(),
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

fn make_fn(
    name: &str,
    kind: RawFnKindInput,
    receiver: &str,
    params: Vec<RawFnParamInput>,
    return_type: Option<&str>,
    helper_calls: Vec<String>,
    body: MirBody,
) -> RawMirExtractedFnInput {
    RawMirExtractedFnInput {
        name: name.into(),
        def_path: format!("TestDriver::{name}"),
        kind,
        receiver: receiver.into(),
        params,
        return_type: return_type.map(Into::into),
        body,
        helper_calls,
    }
}

fn make_input_with_callback() -> RawArtifactInput {
    make_minimal_input(Some(RawBodySurfaceInput {
        functions: vec![make_fn(
            "init",
            RawFnKindInput::Callback {
                trait_name: "Lifecycle".into(),
                method_name: "init".into(),
            },
            "ref_mut",
            vec![],
            Some("Result<(), RegseqError>"),
            vec!["disable_uart".into()],
            minimal_mir_body(),
        )],
    }))
}

fn make_input_with_mixed() -> RawArtifactInput {
    make_minimal_input(Some(RawBodySurfaceInput {
        functions: vec![
            make_fn(
                "init",
                RawFnKindInput::Callback {
                    trait_name: "Lifecycle".into(),
                    method_name: "init".into(),
                },
                "ref_mut",
                vec![],
                Some("Result<(), RegseqError>"),
                vec!["disable_uart".into()],
                minimal_mir_body(),
            ),
            make_fn(
                "disable_uart",
                RawFnKindInput::Helper,
                "ref_mut",
                vec![],
                None,
                vec![],
                minimal_mir_body(),
            ),
            make_fn(
                "handle_irq",
                RawFnKindInput::Isr,
                "ref_mut",
                vec![],
                Some("IrqResult"),
                vec![],
                minimal_mir_body(),
            ),
        ],
    }))
}

fn make_input_with_params() -> RawArtifactInput {
    make_minimal_input(Some(RawBodySurfaceInput {
        functions: vec![make_fn(
            "tx_byte",
            RawFnKindInput::Callback {
                trait_name: "Serial".into(),
                method_name: "tx_byte".into(),
            },
            "ref_mut",
            vec![RawFnParamInput {
                name: "byte".into(),
                ty: "u8".into(),
            }],
            None,
            vec![],
            minimal_mir_body(),
        )],
    }))
}

fn make_input_with_cleanup() -> RawArtifactInput {
    let body = MirBody {
        local_decls: vec![MirLocalDecl {
            index: 0,
            ty: MirTy::Tuple { elements: vec![] },
            mutability: MirMutability::Mut,
            name: Some("_0".to_string()),
        }],
        arg_count: 0,
        basic_blocks: vec![
            MirBasicBlockData {
                statements: vec![],
                terminator: Some(MirTerminator {
                    source_info: MirSourceInfo {
                        scope: 0,
                        span: None,
                    },
                    kind: MirTerminatorKind::Return,
                }),
                is_cleanup: false,
            },
            MirBasicBlockData {
                statements: vec![],
                terminator: Some(MirTerminator {
                    source_info: MirSourceInfo {
                        scope: 0,
                        span: None,
                    },
                    kind: MirTerminatorKind::UnwindResume,
                }),
                is_cleanup: true,
            },
        ],
        source_scopes: vec![MirSourceScope {
            index: 0,
            name: None,
        }],
    };
    make_minimal_input(Some(RawBodySurfaceInput {
        functions: vec![make_fn(
            "init",
            RawFnKindInput::Callback {
                trait_name: "Lifecycle".into(),
                method_name: "init".into(),
            },
            "ref_mut",
            vec![],
            None,
            vec![],
            body,
        )],
    }))
}

fn make_input_with_drop() -> RawArtifactInput {
    let body = MirBody {
        local_decls: vec![
            MirLocalDecl {
                index: 0,
                ty: MirTy::Tuple { elements: vec![] },
                mutability: MirMutability::Mut,
                name: Some("_0".to_string()),
            },
            MirLocalDecl {
                index: 1,
                ty: MirTy::Adt {
                    path: "String".into(),
                    args: vec![],
                },
                mutability: MirMutability::Mut,
                name: Some("_1".to_string()),
            },
        ],
        arg_count: 0,
        basic_blocks: vec![
            MirBasicBlockData {
                statements: vec![],
                terminator: Some(MirTerminator {
                    source_info: MirSourceInfo {
                        scope: 0,
                        span: None,
                    },
                    kind: MirTerminatorKind::Drop {
                        place: MirPlace {
                            local: 1,
                            projection: vec![],
                        },
                        target: 1,
                        unwind: MirUnwindAction::Cleanup { target: 2 },
                        replace: false,
                    },
                }),
                is_cleanup: false,
            },
            MirBasicBlockData {
                statements: vec![],
                terminator: Some(MirTerminator {
                    source_info: MirSourceInfo {
                        scope: 0,
                        span: None,
                    },
                    kind: MirTerminatorKind::Return,
                }),
                is_cleanup: false,
            },
            MirBasicBlockData {
                statements: vec![],
                terminator: Some(MirTerminator {
                    source_info: MirSourceInfo {
                        scope: 0,
                        span: None,
                    },
                    kind: MirTerminatorKind::UnwindResume,
                }),
                is_cleanup: true,
            },
        ],
        source_scopes: vec![MirSourceScope {
            index: 0,
            name: None,
        }],
    };
    make_minimal_input(Some(RawBodySurfaceInput {
        functions: vec![make_fn(
            "cleanup_fn",
            RawFnKindInput::Helper,
            "ref_mut",
            vec![],
            None,
            vec![],
            body,
        )],
    }))
}

fn make_input_with_statements() -> RawArtifactInput {
    let si = MirSourceInfo {
        scope: 0,
        span: None,
    };
    let body = MirBody {
        local_decls: vec![
            MirLocalDecl {
                index: 0,
                ty: MirTy::Tuple { elements: vec![] },
                mutability: MirMutability::Mut,
                name: Some("_0".to_string()),
            },
            MirLocalDecl {
                index: 1,
                ty: MirTy::Uint {
                    uint_ty: MirUintTy::U32,
                },
                mutability: MirMutability::Mut,
                name: Some("_1".to_string()),
            },
        ],
        arg_count: 0,
        basic_blocks: vec![MirBasicBlockData {
            statements: vec![
                MirStatement {
                    source_info: si.clone(),
                    kind: MirStatementKind::StorageLive { local: 1 },
                },
                MirStatement {
                    source_info: si.clone(),
                    kind: MirStatementKind::Assign {
                        place: MirPlace {
                            local: 1,
                            projection: vec![],
                        },
                        rvalue: Box::new(MirRvalue::Use {
                            operand: MirOperand::Constant {
                                constant: MirConstant {
                                    ty: MirTy::Uint {
                                        uint_ty: MirUintTy::U32,
                                    },
                                    value: MirConstValue::Scalar { bits: 42, size: 4 },
                                },
                            },
                        }),
                    },
                },
                MirStatement {
                    source_info: si,
                    kind: MirStatementKind::StorageDead { local: 1 },
                },
            ],
            terminator: Some(MirTerminator {
                source_info: MirSourceInfo {
                    scope: 0,
                    span: None,
                },
                kind: MirTerminatorKind::Return,
            }),
            is_cleanup: false,
        }],
        source_scopes: vec![MirSourceScope {
            index: 0,
            name: None,
        }],
    };
    make_minimal_input(Some(RawBodySurfaceInput {
        functions: vec![make_fn(
            "stmt_fn",
            RawFnKindInput::Helper,
            "ref_mut",
            vec![],
            None,
            vec![],
            body,
        )],
    }))
}

fn make_input_with_unsupported() -> RawArtifactInput {
    let body = MirBody {
        local_decls: vec![MirLocalDecl {
            index: 0,
            ty: MirTy::Tuple { elements: vec![] },
            mutability: MirMutability::Mut,
            name: Some("_0".to_string()),
        }],
        arg_count: 0,
        basic_blocks: vec![MirBasicBlockData {
            statements: vec![MirStatement {
                source_info: MirSourceInfo {
                    scope: 0,
                    span: None,
                },
                kind: MirStatementKind::Unsupported {
                    description: "FakeIntrinsic".into(),
                },
            }],
            terminator: Some(MirTerminator {
                source_info: MirSourceInfo {
                    scope: 0,
                    span: None,
                },
                kind: MirTerminatorKind::Return,
            }),
            is_cleanup: false,
        }],
        source_scopes: vec![MirSourceScope {
            index: 0,
            name: None,
        }],
    };
    make_minimal_input(Some(RawBodySurfaceInput {
        functions: vec![make_fn(
            "unsupported_fn",
            RawFnKindInput::Helper,
            "ref_mut",
            vec![],
            None,
            vec![],
            body,
        )],
    }))
}
