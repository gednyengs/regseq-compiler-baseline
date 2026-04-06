//! Tests for the semantic procedural IR type boundary.
//!
//! These tests verify that the semantic IR types compile, match the approved
//! plan shape, use structured identity, and contain no OS-specific variants.

use regseq_model::item::identity::{CanonicalPath, ItemId};
use regseq_model::item::interrupt::InterruptClassDef;
use regseq_model::semantic::{
    AssignTarget, BinOp, Block, ConstantRef, DriverFieldRef, Expr, FieldWrite, FieldWriteValue,
    FunctionRef, InterruptClassId, InterruptClassRef, LocalId, LocalOrigin, PinCtrlState,
    RegisterFieldRef, RegisterRef, ResourceKind, ResourceOperation, ResourceProperty, ResourceRef,
    SemanticBody, SemanticLocal, SemanticModel, SemanticType, Stmt, SubsystemActionKind,
    SubsystemActionRef, SubsystemKind, UnaryOp,
};

// ---------------------------------------------------------------------------
// Test 1: Semantic IR module compiles and is importable
// ---------------------------------------------------------------------------

#[test]
fn semantic_ir_module_is_importable() {
    // Prove all core types are importable by constructing basic instances.
    let _body = SemanticBody::empty();
    let _block = Block { stmts: vec![] };
    let _local_id = LocalId(0);
    let _class_id = InterruptClassId(0);
    let _ty = SemanticType::Void;
    let _kind = ResourceKind::Clock;
    let _op = BinOp::Add;
    let _unary = UnaryOp::Not;
}

// ---------------------------------------------------------------------------
// Test 2: Public type surface matches the approved plan shape
// ---------------------------------------------------------------------------

/// Construct the `set_baud_rate` body from plan Section 4.13.
#[test]
fn type_surface_set_baud_rate_arithmetic() {
    let clk = LocalId(0);
    let baud = LocalId(1);
    let div64 = LocalId(2);
    let ibrd_local = LocalId(3);
    let fbrd_local = LocalId(4);

    let reg_block = ItemId(10);
    let ibrd_reg = RegisterRef {
        block: reg_block,
        canonical_path: CanonicalPath::new("pkg::regs::Regs::ibrd"),
        name: "ibrd".into(),
    };
    let fbrd_reg = RegisterRef {
        block: reg_block,
        canonical_path: CanonicalPath::new("pkg::regs::Regs::fbrd"),
        name: "fbrd".into(),
    };
    let baud_divint_field = RegisterFieldRef {
        register: ibrd_reg.clone(),
        canonical_path: CanonicalPath::new("pkg::regs::Regs::ibrd::baud_divint"),
        name: "baud_divint".into(),
    };
    let baud_divfrac_field = RegisterFieldRef {
        register: fbrd_reg.clone(),
        canonical_path: CanonicalPath::new("pkg::regs::Regs::fbrd::baud_divfrac"),
        name: "baud_divfrac".into(),
    };

    let body = SemanticBody {
        locals: vec![
            SemanticLocal {
                id: clk,
                name: "clk".into(),
                ty: SemanticType::U32,
                origin: LocalOrigin::Param { position: 1 },
            },
            SemanticLocal {
                id: baud,
                name: "baud".into(),
                ty: SemanticType::U32,
                origin: LocalOrigin::Param { position: 2 },
            },
            SemanticLocal {
                id: div64,
                name: "div64".into(),
                ty: SemanticType::U64,
                origin: LocalOrigin::Derived,
            },
            SemanticLocal {
                id: ibrd_local,
                name: "ibrd".into(),
                ty: SemanticType::U32,
                origin: LocalOrigin::Derived,
            },
            SemanticLocal {
                id: fbrd_local,
                name: "fbrd".into(),
                ty: SemanticType::U32,
                origin: LocalOrigin::Derived,
            },
        ],
        params: vec![LocalId(0), LocalId(1), LocalId(2)],
        return_type: SemanticType::Void,
        body: Block {
            stmts: vec![
                // div64 = (clk as u64 * 4) / (baud as u64)
                Stmt::Let {
                    local: div64,
                    value: Expr::BinaryOp {
                        op: BinOp::Div,
                        lhs: Box::new(Expr::BinaryOp {
                            op: BinOp::Mul,
                            lhs: Box::new(Expr::Cast {
                                expr: Box::new(Expr::LocalRef { local: clk }),
                                ty: SemanticType::U64,
                            }),
                            rhs: Box::new(Expr::IntLit {
                                value: 4,
                                ty: SemanticType::U64,
                            }),
                        }),
                        rhs: Box::new(Expr::Cast {
                            expr: Box::new(Expr::LocalRef { local: baud }),
                            ty: SemanticType::U64,
                        }),
                    },
                },
                // ibrd = (div64 >> 6) as u32
                Stmt::Let {
                    local: ibrd_local,
                    value: Expr::Cast {
                        expr: Box::new(Expr::BinaryOp {
                            op: BinOp::Shr,
                            lhs: Box::new(Expr::LocalRef { local: div64 }),
                            rhs: Box::new(Expr::IntLit {
                                value: 6,
                                ty: SemanticType::U64,
                            }),
                        }),
                        ty: SemanticType::U32,
                    },
                },
                // fbrd = (div64 & 0x3f) as u32
                Stmt::Let {
                    local: fbrd_local,
                    value: Expr::Cast {
                        expr: Box::new(Expr::BinaryOp {
                            op: BinOp::BitAnd,
                            lhs: Box::new(Expr::LocalRef { local: div64 }),
                            rhs: Box::new(Expr::IntLit {
                                value: 0x3f,
                                ty: SemanticType::U64,
                            }),
                        }),
                        ty: SemanticType::U32,
                    },
                },
                // RegWrite ibrd
                Stmt::RegWrite {
                    register: ibrd_reg,
                    fields: vec![FieldWrite {
                        field: baud_divint_field,
                        value: FieldWriteValue::Local { local: ibrd_local },
                    }],
                },
                // RegWrite fbrd
                Stmt::RegWrite {
                    register: fbrd_reg,
                    fields: vec![FieldWrite {
                        field: baud_divfrac_field,
                        value: FieldWriteValue::Local { local: fbrd_local },
                    }],
                },
            ],
        },
    };

    assert_eq!(body.locals.len(), 5);
    assert_eq!(body.body.stmts.len(), 5);
}

/// Construct the `uart_clock_hz` body from plan Section 4.13.
#[test]
fn type_surface_uart_clock_hz_resource_read() {
    let rate = LocalId(0);
    let driver_id = ItemId(0);

    let clock_field = DriverFieldRef {
        owner: driver_id,
        canonical_path: CanonicalPath::new("pkg::driver::MyDriver::clk"),
        name: "clk".into(),
    };
    let clock_ref = ResourceRef {
        kind: ResourceKind::Clock,
        field: clock_field,
    };
    let default_const = ConstantRef {
        id: Some(ItemId(5)),
        canonical_path: CanonicalPath::new("pkg::driver::MyDriver::DEFAULT_UART_CLOCK_HZ"),
        name: "DEFAULT_UART_CLOCK_HZ".into(),
        stable_name: "MY_DRIVER_DEFAULT_UART_CLOCK_HZ".into(),
        known_value: Some(48_000_000),
    };

    let body = SemanticBody {
        locals: vec![SemanticLocal {
            id: rate,
            name: "rate".into(),
            ty: SemanticType::U32,
            origin: LocalOrigin::Derived,
        }],
        params: vec![],
        return_type: SemanticType::U32,
        body: Block {
            stmts: vec![
                Stmt::Let {
                    local: rate,
                    value: Expr::WithDefault {
                        value: Box::new(Expr::ResourcePropertyRead {
                            resource: clock_ref,
                            property: ResourceProperty::ClockRate,
                        }),
                        default: Box::new(Expr::Constant {
                            constant: default_const,
                        }),
                    },
                },
                Stmt::Return {
                    value: Some(Expr::LocalRef { local: rate }),
                },
            ],
        },
    };

    assert_eq!(body.return_type, SemanticType::U32);
    assert_eq!(body.body.stmts.len(), 2);
}

// ---------------------------------------------------------------------------
// Test 3: Body-level refs use structured identity
// ---------------------------------------------------------------------------

#[test]
fn body_refs_use_structured_identity() {
    // FunctionRef carries ItemId + CanonicalPath
    let func_ref = FunctionRef {
        item: ItemId(1),
        canonical_path: CanonicalPath::new("pkg::driver::MyDriver::set_baud_rate"),
        name: "set_baud_rate".into(),
    };
    assert_eq!(func_ref.item, ItemId(1));
    assert_eq!(
        func_ref.canonical_path,
        CanonicalPath::new("pkg::driver::MyDriver::set_baud_rate")
    );

    // DriverFieldRef carries owner ItemId + CanonicalPath
    let field_ref = DriverFieldRef {
        owner: ItemId(0),
        canonical_path: CanonicalPath::new("pkg::driver::MyDriver::regs"),
        name: "regs".into(),
    };
    assert_eq!(field_ref.owner, ItemId(0));

    // RegisterRef carries block ItemId + CanonicalPath
    let reg_ref = RegisterRef {
        block: ItemId(2),
        canonical_path: CanonicalPath::new("pkg::regs::Regs::cr"),
        name: "cr".into(),
    };
    assert_eq!(reg_ref.block, ItemId(2));

    // RegisterFieldRef carries full register + field identity
    let reg_field_ref = RegisterFieldRef {
        register: reg_ref.clone(),
        canonical_path: CanonicalPath::new("pkg::regs::Regs::cr::uarten"),
        name: "uarten".into(),
    };
    assert_eq!(reg_field_ref.register.block, ItemId(2));

    // ConstantRef carries optional ItemId + CanonicalPath + stable_name
    let const_ref = ConstantRef {
        id: Some(ItemId(10)),
        canonical_path: CanonicalPath::new("pkg::driver::MyDriver::RX_INTERRUPT_MASK"),
        name: "RX_INTERRUPT_MASK".into(),
        stable_name: "MY_DRIVER_RX_INTERRUPT_MASK".into(),
        known_value: Some(0x10),
    };
    assert_eq!(const_ref.id, Some(ItemId(10)));
    assert_eq!(const_ref.stable_name, "MY_DRIVER_RX_INTERRUPT_MASK");
}

// ---------------------------------------------------------------------------
// Test 4: FallibleOp supports both unit-success and value-success
// ---------------------------------------------------------------------------

#[test]
fn fallible_op_unit_success() {
    let error_local = LocalId(0);
    let stmt = Stmt::FallibleOp {
        body: Block {
            stmts: vec![Stmt::Eval {
                expr: Expr::BoolLit { value: true },
            }],
        },
        success_local: None,
        error_local,
        cleanup_on_error: Block {
            stmts: vec![Stmt::Return {
                value: Some(Expr::LocalRef { local: error_local }),
            }],
        },
    };
    if let Stmt::FallibleOp { success_local, .. } = &stmt {
        assert!(success_local.is_none());
    } else {
        panic!("expected FallibleOp");
    }
}

#[test]
fn fallible_op_value_success() {
    let success = LocalId(0);
    let error_local = LocalId(1);
    let stmt = Stmt::FallibleOp {
        body: Block {
            stmts: vec![Stmt::Eval {
                expr: Expr::IntLit {
                    value: 42,
                    ty: SemanticType::I32,
                },
            }],
        },
        success_local: Some(success),
        error_local,
        cleanup_on_error: Block { stmts: vec![] },
    };
    if let Stmt::FallibleOp { success_local, .. } = &stmt {
        assert_eq!(*success_local, Some(LocalId(0)));
    } else {
        panic!("expected FallibleOp");
    }
}

// ---------------------------------------------------------------------------
// Test 5: Option-carrier forms exist as explicit IR constructs
// ---------------------------------------------------------------------------

#[test]
fn option_carrier_forms_are_explicit() {
    let inner = Box::new(Expr::LocalRef { local: LocalId(0) });

    // IsSome
    let is_some = Expr::IsSome {
        inner: inner.clone(),
    };
    assert!(matches!(is_some, Expr::IsSome { .. }));

    // OptionNone
    let none = Expr::OptionNone;
    assert!(matches!(none, Expr::OptionNone));

    // OptionSome
    let some = Expr::OptionSome {
        inner: inner.clone(),
    };
    assert!(matches!(some, Expr::OptionSome { .. }));

    // OptionUnwrap
    let unwrap = Expr::OptionUnwrap { inner };
    assert!(matches!(unwrap, Expr::OptionUnwrap { .. }));

    // Verify they are distinct variants
    assert_ne!(is_some, none);
}

/// Verify the option carrier pattern from plan Section 4.6:
/// service_tx_fifo with IsSome + OptionUnwrap + OptionNone.
#[test]
fn option_carrier_service_tx_fifo_pattern() {
    let pending_tx_ref = DriverFieldRef {
        owner: ItemId(0),
        canonical_path: CanonicalPath::new("pkg::driver::MyDriver::pending_tx"),
        name: "pending_tx".into(),
    };

    // Branch on IsSome(pending_tx)
    let condition = Expr::IsSome {
        inner: Box::new(Expr::DriverFieldRead {
            field: pending_tx_ref.clone(),
        }),
    };

    // In the then-branch: unwrap the pending_tx
    let byte = LocalId(5);
    let unwrap_stmt = Stmt::Let {
        local: byte,
        value: Expr::OptionUnwrap {
            inner: Box::new(Expr::DriverFieldRead {
                field: pending_tx_ref.clone(),
            }),
        },
    };

    // Clear pending_tx with OptionNone
    let clear_stmt = Stmt::FieldAssign {
        field: pending_tx_ref,
        value: Expr::OptionNone,
    };

    // All three carrier forms participate in the same branch
    let _branch = Stmt::Branch {
        condition,
        then_body: Block {
            stmts: vec![unwrap_stmt, clear_stmt],
        },
        else_body: Block { stmts: vec![] },
    };
}

// ---------------------------------------------------------------------------
// Test 6: No Linux-specific realization types in the semantic IR
// ---------------------------------------------------------------------------

#[test]
fn no_os_specific_semantic_type_variants() {
    // Exhaustive match proves no Linux/Zephyr/UART/PL011 variants exist.
    fn classify_type(ty: &SemanticType) -> &'static str {
        match ty {
            SemanticType::Bool => "scalar",
            SemanticType::U8 => "scalar",
            SemanticType::U16 => "scalar",
            SemanticType::U32 => "scalar",
            SemanticType::U64 => "scalar",
            SemanticType::I8 => "scalar",
            SemanticType::I16 => "scalar",
            SemanticType::I32 => "scalar",
            SemanticType::I64 => "scalar",
            SemanticType::Void => "void",
            SemanticType::Struct { .. } => "compound",
            SemanticType::Enum { .. } => "compound",
            SemanticType::Array { .. } => "compound",
            SemanticType::Ref { .. } => "reference",
            SemanticType::RefMut { .. } => "reference",
            SemanticType::Option { .. } => "carrier",
            SemanticType::Result { .. } => "carrier",
            SemanticType::DriverData => "domain",
            SemanticType::RegisterBlock { .. } => "domain",
            SemanticType::RegisterValue { .. } => "domain",
            SemanticType::ResourceHandle { .. } => "domain",
            SemanticType::Opaque { .. } => "opaque",
        }
    }
    assert_eq!(classify_type(&SemanticType::Bool), "scalar");
    assert_eq!(classify_type(&SemanticType::DriverData), "domain");
    assert_eq!(
        classify_type(&SemanticType::Opaque {
            description: "test".into()
        }),
        "opaque"
    );
}

#[test]
fn no_os_specific_stmt_variants() {
    // Exhaustive match proves no Linux/Zephyr variants exist.
    fn classify_stmt(stmt: &Stmt) -> &'static str {
        match stmt {
            Stmt::Let { .. } => "binding",
            Stmt::Assign { .. } => "binding",
            Stmt::CompoundAssign { .. } => "binding",
            Stmt::Eval { .. } => "binding",
            Stmt::Branch { .. } => "control",
            Stmt::Loop { .. } => "control",
            Stmt::OrderedRegion { .. } => "control",
            Stmt::FallibleOp { .. } => "control",
            Stmt::Return { .. } => "control",
            Stmt::RegWrite { .. } => "device",
            Stmt::RegModify { .. } => "device",
            Stmt::ResourceOp { .. } => "device",
            Stmt::FieldAssign { .. } => "device",
            Stmt::SubsystemAction { .. } => "device",
        }
    }
    let s = Stmt::Return { value: None };
    assert_eq!(classify_stmt(&s), "control");
}

#[test]
fn no_os_specific_expr_variants() {
    // Exhaustive match proves no Linux/Zephyr variants exist.
    fn classify_expr(expr: &Expr) -> &'static str {
        match expr {
            Expr::IntLit { .. } => "literal",
            Expr::BoolLit { .. } => "literal",
            Expr::LocalRef { .. } => "ref",
            Expr::DriverFieldRead { .. } => "ref",
            Expr::Constant { .. } => "ref",
            Expr::BinaryOp { .. } => "arith",
            Expr::UnaryOp { .. } => "arith",
            Expr::Cast { .. } => "arith",
            Expr::RegFieldRead { .. } => "register",
            Expr::RegReadRaw { .. } => "register",
            Expr::FieldExtract { .. } => "register",
            Expr::HelperCall { .. } => "call",
            Expr::ResourcePropertyRead { .. } => "resource",
            Expr::WithDefault { .. } => "resource",
            Expr::InterruptClassTest { .. } => "interrupt",
            Expr::ResourcePresent { .. } => "resource",
            Expr::IsSome { .. } => "carrier",
            Expr::OptionNone => "carrier",
            Expr::OptionSome { .. } => "carrier",
            Expr::OptionUnwrap { .. } => "carrier",
        }
    }
    let e = Expr::OptionNone;
    assert_eq!(classify_expr(&e), "carrier");
}

// ---------------------------------------------------------------------------
// Additional: SemanticModel can be constructed
// ---------------------------------------------------------------------------

#[test]
fn semantic_model_construction() {
    use regseq_model::declaration::Provenance;
    use regseq_model::item::constant::{ConstValue, ConstantDef};
    use regseq_model::item::function::{FunctionDef, FunctionKind, FunctionRole};
    use regseq_model::item::type_ref::{PrimitiveType, TypeRef};

    let model = SemanticModel {
        functions: vec![FunctionDef {
            id: ItemId(0),
            canonical_path: CanonicalPath::new("pkg::MyDriver::init"),
            name: "init".into(),
            kind: FunctionKind::FreeFunction,
            role: FunctionRole::Helper,
            body: SemanticBody::empty(),
            provenance: Provenance::new("identity"),
        }],
        constants: vec![ConstantDef {
            id: ItemId(1),
            canonical_path: CanonicalPath::new("pkg::MyDriver::BAUD_DEFAULT"),
            name: "BAUD_DEFAULT".into(),
            owner: Some(ItemId(0)),
            ty: TypeRef::Primitive {
                ty: PrimitiveType::U32,
            },
            value: Some(ConstValue::Integer(115200)),
            stable_name: "MY_DRIVER_BAUD_DEFAULT".into(),
            provenance: Provenance::new("identity"),
        }],
        interrupt_classes: vec![InterruptClassDef {
            name: "rx".into(),
            mask_value: 0x10,
        }],
    };

    assert_eq!(model.functions.len(), 1);
    assert_eq!(model.constants.len(), 1);
    assert_eq!(model.interrupt_classes.len(), 1);
}
