//! Voided pending MonoRust + RegseqModel redesign.
//! See docs/discussions/compiler-refactoring/test-audit.md for the classification rationale
//! and the retirement criterion.
#![cfg(feature = "post-monorust-tests")]

//! Unit + integration coverage for the backend-neutral C AST.
//!
//! Exercises construction and rendering of every Phase-7C node type,
//! checks a small multi-statement body renders as valid C (best-effort
//! via `cc -fsyntax-only` when `cc` is on `PATH`), and asserts that
//! rendering is deterministic across identical inputs.

use regseq_backends_old::c::declarators::{CDeclaration, CStorageClass, CType};
use regseq_backends_old::c::expressions::{CBinaryOp, CExpr, CIntSuffix, CUnaryOp};
use regseq_backends_old::c::initializers::{CDesignator, CInitItem, CInitializer};
use regseq_backends_old::c::statements::{CBlockItem, CStmt};
use regseq_backends_old::c::{CStructField, CTopLevelItem, CTranslationUnit};

// ---------------------------------------------------------------------------
// Expression node coverage
// ---------------------------------------------------------------------------

fn ident(name: &str) -> CExpr {
    CExpr::Ident(name.into())
}

#[test]
fn int_literal_renders_value_and_suffix() {
    let e = CExpr::IntLiteral {
        value: 42,
        suffix: CIntSuffix::U,
    };
    assert_eq!(e.render(), "42U");
}

#[test]
fn int_literal_negative_renders_sign() {
    let e = CExpr::IntLiteral {
        value: -7,
        suffix: CIntSuffix::None,
    };
    assert_eq!(e.render(), "-7");
}

#[test]
fn string_literal_escapes_special_chars() {
    let e = CExpr::StringLiteral("a\"b\\c\n".into());
    assert_eq!(e.render(), "\"a\\\"b\\\\c\\n\"");
}

#[test]
fn identifier_renders_name() {
    assert_eq!(ident("port").render(), "port");
}

#[test]
fn field_projection_dot_and_arrow() {
    let dot = CExpr::Field {
        base: Box::new(ident("s")),
        name: "x".into(),
        via_ptr: false,
    };
    let arrow = CExpr::Field {
        base: Box::new(ident("p")),
        name: "x".into(),
        via_ptr: true,
    };
    assert_eq!(dot.render(), "s.x");
    assert_eq!(arrow.render(), "p->x");
}

#[test]
fn array_index_renders() {
    let e = CExpr::Index {
        base: Box::new(ident("arr")),
        idx: Box::new(CExpr::IntLiteral {
            value: 3,
            suffix: CIntSuffix::None,
        }),
    };
    assert_eq!(e.render(), "arr[3]");
}

#[test]
fn call_with_args() {
    let e = CExpr::Call {
        callee: Box::new(ident("f")),
        args: vec![ident("a"), ident("b")],
    };
    assert_eq!(e.render(), "f(a, b)");
}

#[test]
fn call_no_args() {
    let e = CExpr::Call {
        callee: Box::new(ident("f")),
        args: vec![],
    };
    assert_eq!(e.render(), "f()");
}

#[test]
fn unary_ops_render_parenthesized() {
    let not = CExpr::Unary {
        op: CUnaryOp::LogicalNot,
        operand: Box::new(ident("x")),
    };
    let bitnot = CExpr::Unary {
        op: CUnaryOp::BitNot,
        operand: Box::new(ident("x")),
    };
    let neg = CExpr::Unary {
        op: CUnaryOp::Neg,
        operand: Box::new(ident("x")),
    };
    let plus = CExpr::Unary {
        op: CUnaryOp::Plus,
        operand: Box::new(ident("x")),
    };
    let addr = CExpr::Unary {
        op: CUnaryOp::AddrOf,
        operand: Box::new(ident("x")),
    };
    let deref = CExpr::Unary {
        op: CUnaryOp::Deref,
        operand: Box::new(ident("x")),
    };
    assert_eq!(not.render(), "(!x)");
    assert_eq!(bitnot.render(), "(~x)");
    assert_eq!(neg.render(), "(-x)");
    assert_eq!(plus.render(), "(+x)");
    assert_eq!(addr.render(), "(&x)");
    assert_eq!(deref.render(), "(*x)");
}

#[test]
fn binary_ops_cover_bitwise_and_comparison() {
    let or = CExpr::Binary {
        op: CBinaryOp::BitOr,
        lhs: Box::new(ident("a")),
        rhs: Box::new(ident("b")),
    };
    let shl = CExpr::Binary {
        op: CBinaryOp::Shl,
        lhs: Box::new(ident("a")),
        rhs: Box::new(CExpr::IntLiteral {
            value: 4,
            suffix: CIntSuffix::None,
        }),
    };
    let eq = CExpr::Binary {
        op: CBinaryOp::Eq,
        lhs: Box::new(ident("x")),
        rhs: Box::new(CExpr::IntLiteral {
            value: 0,
            suffix: CIntSuffix::None,
        }),
    };
    assert_eq!(or.render(), "(a | b)");
    assert_eq!(shl.render(), "(a << 4)");
    assert_eq!(eq.render(), "(x == 0)");
}

#[test]
fn cast_wraps_type_and_expr() {
    let e = CExpr::Cast {
        ty: CType::Named("u32".into()),
        expr: Box::new(ident("x")),
    };
    assert_eq!(e.render(), "((u32)x)");
}

#[test]
fn assignment_plain_and_compound() {
    let plain = CExpr::Assign {
        lhs: Box::new(ident("x")),
        op: None,
        rhs: Box::new(ident("y")),
    };
    let compound = CExpr::Assign {
        lhs: Box::new(ident("x")),
        op: Some(CBinaryOp::BitOr),
        rhs: Box::new(ident("y")),
    };
    assert_eq!(plain.render(), "(x = y)");
    assert_eq!(compound.render(), "(x |= y)");
}

#[test]
fn ternary_renders() {
    let e = CExpr::Ternary {
        cond: Box::new(ident("c")),
        then_branch: Box::new(ident("a")),
        else_branch: Box::new(ident("b")),
    };
    assert_eq!(e.render(), "(c ? a : b)");
}

#[test]
fn paren_forces_grouping() {
    let e = CExpr::Paren(Box::new(ident("x")));
    assert_eq!(e.render(), "(x)");
}

#[test]
fn raw_expr_is_verbatim() {
    let e = CExpr::Raw("SOME_MACRO(1, 2)".into());
    assert_eq!(e.render(), "SOME_MACRO(1, 2)");
}

// ---------------------------------------------------------------------------
// Declarator / type coverage
// ---------------------------------------------------------------------------

#[test]
fn pointer_declaration_no_extra_space() {
    let d = CDeclaration::new(
        CStorageClass::None,
        CType::Pointer {
            inner: Box::new(CType::Named("int".into())),
            const_qualified: false,
        },
        "p",
    );
    let mut out = String::new();
    d.render_into(&mut out, 0);
    assert_eq!(out, "int *p;\n");
}

#[test]
fn const_pointer_declaration() {
    let d = CDeclaration::new(
        CStorageClass::Static,
        CType::Pointer {
            inner: Box::new(CType::Named("char".into())),
            const_qualified: true,
        },
        "s",
    );
    let mut out = String::new();
    d.render_into(&mut out, 0);
    assert_eq!(out, "static const char *s;\n");
}

#[test]
fn array_declaration_places_size_after_name() {
    let d = CDeclaration::new(
        CStorageClass::None,
        CType::Array {
            inner: Box::new(CType::Named("u32".into())),
            size: Some(4),
        },
        "arr",
    );
    let mut out = String::new();
    d.render_into(&mut out, 0);
    assert_eq!(out, "u32 arr[4];\n");
}

#[test]
fn struct_declaration() {
    let d = CDeclaration::new(CStorageClass::None, CType::Struct("foo".into()), "f");
    let mut out = String::new();
    d.render_into(&mut out, 0);
    assert_eq!(out, "struct foo f;\n");
}

#[test]
fn typedef_declaration() {
    let d = CDeclaration::new(CStorageClass::None, CType::Typedef("size_t".into()), "n");
    let mut out = String::new();
    d.render_into(&mut out, 0);
    assert_eq!(out, "size_t n;\n");
}

// ---------------------------------------------------------------------------
// Initializer / designator coverage
// ---------------------------------------------------------------------------

#[test]
fn scalar_initializer_is_just_the_expr() {
    let d = CDeclaration::new(CStorageClass::None, CType::Named("int".into()), "x")
        .with_initializer(CInitializer::Expr(CExpr::IntLiteral {
            value: 7,
            suffix: CIntSuffix::None,
        }));
    let mut out = String::new();
    d.render_into(&mut out, 0);
    assert_eq!(out, "int x = 7;\n");
}

#[test]
fn designated_list_initializer() {
    let init = CInitializer::List(vec![
        CInitItem {
            designator: Some(CDesignator::Field("a".into())),
            init: CInitializer::Expr(ident("x")),
        },
        CInitItem {
            designator: Some(CDesignator::Index(CExpr::IntLiteral {
                value: 0,
                suffix: CIntSuffix::None,
            })),
            init: CInitializer::Expr(CExpr::IntLiteral {
                value: 1,
                suffix: CIntSuffix::None,
            }),
        },
    ]);
    let d = CDeclaration::new(CStorageClass::Static, CType::Struct("s".into()), "v")
        .with_initializer(init);
    let mut out = String::new();
    d.render_into(&mut out, 0);
    assert_eq!(out, "static struct s v = {\n\t.a = x,\n\t[0] = 1,\n};\n");
}

#[test]
fn empty_list_initializer() {
    let d = CDeclaration::new(CStorageClass::None, CType::Named("int".into()), "x")
        .with_initializer(CInitializer::List(vec![]));
    let mut out = String::new();
    d.render_into(&mut out, 0);
    assert_eq!(out, "int x = {};\n");
}

// ---------------------------------------------------------------------------
// Statement coverage
// ---------------------------------------------------------------------------

#[test]
fn compound_with_block_items_renders() {
    let stmt = CStmt::Compound {
        items: vec![
            CBlockItem::Decl(CDeclaration::new(
                CStorageClass::None,
                CType::Named("int".into()),
                "x",
            )),
            CBlockItem::Stmt(CStmt::Expr(CExpr::Assign {
                lhs: Box::new(ident("x")),
                op: None,
                rhs: Box::new(CExpr::IntLiteral {
                    value: 0,
                    suffix: CIntSuffix::None,
                }),
            })),
        ],
    };
    let mut out = String::new();
    stmt.render_into(&mut out, 0);
    assert_eq!(out, "{\n\tint x;\n\t(x = 0);\n}\n");
}

#[test]
fn if_else_renders_knr_braces() {
    let stmt = CStmt::If {
        cond: ident("c"),
        then_branch: Box::new(CStmt::Compound {
            items: vec![CBlockItem::Stmt(CStmt::Return(Some(CExpr::IntLiteral {
                value: 0,
                suffix: CIntSuffix::None,
            })))],
        }),
        else_branch: Some(Box::new(CStmt::Compound {
            items: vec![CBlockItem::Stmt(CStmt::Return(Some(CExpr::IntLiteral {
                value: 1,
                suffix: CIntSuffix::None,
            })))],
        })),
    };
    let mut out = String::new();
    stmt.render_into(&mut out, 0);
    assert_eq!(out, "if (c) {\n\treturn 0;\n} else {\n\treturn 1;\n}\n");
}

#[test]
fn while_loop_renders() {
    let stmt = CStmt::While {
        cond: CExpr::Call {
            callee: Box::new(ident("ready")),
            args: vec![],
        },
        body: Box::new(CStmt::Compound {
            items: vec![CBlockItem::Stmt(CStmt::Expr(CExpr::Call {
                callee: Box::new(ident("step")),
                args: vec![],
            }))],
        }),
    };
    let mut out = String::new();
    stmt.render_into(&mut out, 0);
    assert_eq!(out, "while (ready()) {\n\tstep();\n}\n");
}

#[test]
fn return_void_and_value() {
    let mut a = String::new();
    CStmt::Return(None).render_into(&mut a, 0);
    let mut b = String::new();
    CStmt::Return(Some(ident("x"))).render_into(&mut b, 0);
    assert_eq!(a, "return;\n");
    assert_eq!(b, "return x;\n");
}

#[test]
fn goto_and_labeled_cooperate() {
    let stmt = CStmt::Compound {
        items: vec![
            CBlockItem::Stmt(CStmt::Goto("err".into())),
            CBlockItem::Stmt(CStmt::Labeled {
                label: "err".into(),
                stmt: Box::new(CStmt::Return(Some(CExpr::IntLiteral {
                    value: -1,
                    suffix: CIntSuffix::None,
                }))),
            }),
        ],
    };
    let mut out = String::new();
    stmt.render_into(&mut out, 0);
    // Labels render at column 0 (idiomatic kernel style); the
    // labeled statement itself inherits the caller's indent.
    assert_eq!(out, "{\n\tgoto err;\nerr:\n\treturn -1;\n}\n");
}

#[test]
fn empty_and_raw_statements() {
    let mut a = String::new();
    CStmt::Empty.render_into(&mut a, 1);
    let mut b = String::new();
    CStmt::Raw("asm volatile(\"nop\");".into()).render_into(&mut b, 1);
    assert_eq!(a, "\t;\n");
    assert_eq!(b, "\tasm volatile(\"nop\");\n");
}

// ---------------------------------------------------------------------------
// Top-level item coverage
// ---------------------------------------------------------------------------

#[test]
fn func_def_structured_body_renders() {
    let body = CStmt::Compound {
        items: vec![CBlockItem::Stmt(CStmt::Return(Some(CExpr::IntLiteral {
            value: 0,
            suffix: CIntSuffix::None,
        })))],
    };
    let item = CTopLevelItem::FuncDefStructured {
        signature: "static int f(void)".into(),
        body,
        comment: Some("provenance".into()),
    };
    let mut out = String::new();
    item.render_into(&mut out);
    assert_eq!(
        out,
        "/* provenance */\nstatic int f(void)\n{\n\treturn 0;\n}\n"
    );
}

#[test]
fn translation_unit_composes() {
    let mut tu = CTranslationUnit::new();
    tu.push(CTopLevelItem::Include {
        path: "linux/types.h".into(),
        system: true,
    });
    tu.push(CTopLevelItem::BlankLine);
    tu.push(CTopLevelItem::StructDef {
        name: "foo".into(),
        fields: vec![CStructField {
            type_name: "int".into(),
            field_name: "x".into(),
            doc: None,
        }],
    });
    let s = tu.render();
    assert!(s.contains("#include <linux/types.h>"));
    assert!(s.contains("struct foo {\n\tint x;\n};\n"));
}

// ---------------------------------------------------------------------------
// Integration + determinism
// ---------------------------------------------------------------------------

fn build_reference_body() -> CStmt {
    CStmt::Compound {
        items: vec![
            CBlockItem::Decl(CDeclaration::new(
                CStorageClass::None,
                CType::Named("int".into()),
                "ret",
            )),
            CBlockItem::Stmt(CStmt::Labeled {
                label: "start".into(),
                stmt: Box::new(CStmt::Expr(CExpr::Assign {
                    lhs: Box::new(ident("ret")),
                    op: None,
                    rhs: Box::new(CExpr::IntLiteral {
                        value: 0,
                        suffix: CIntSuffix::None,
                    }),
                })),
            }),
            CBlockItem::Stmt(CStmt::If {
                cond: CExpr::Binary {
                    op: CBinaryOp::Eq,
                    lhs: Box::new(ident("ret")),
                    rhs: Box::new(CExpr::IntLiteral {
                        value: 0,
                        suffix: CIntSuffix::None,
                    }),
                },
                then_branch: Box::new(CStmt::Compound {
                    items: vec![CBlockItem::Stmt(CStmt::Return(Some(ident("ret"))))],
                }),
                else_branch: Some(Box::new(CStmt::Compound {
                    items: vec![CBlockItem::Stmt(CStmt::Goto("start".into()))],
                })),
            }),
            CBlockItem::Stmt(CStmt::While {
                cond: CExpr::Call {
                    callee: Box::new(ident("ready")),
                    args: vec![],
                },
                body: Box::new(CStmt::Compound {
                    items: vec![CBlockItem::Stmt(CStmt::Expr(CExpr::Call {
                        callee: Box::new(ident("step")),
                        args: vec![],
                    }))],
                }),
            }),
            CBlockItem::Stmt(CStmt::Return(Some(CExpr::IntLiteral {
                value: 0,
                suffix: CIntSuffix::None,
            }))),
        ],
    }
}

#[test]
fn integration_body_renders_valid_c_best_effort() {
    let body = build_reference_body();
    let mut body_text = String::new();
    body.render_into(&mut body_text, 0);

    let src = format!(
        "extern int ready(void);\nextern void step(void);\nint f(void)\n{}",
        body_text
    );

    // Attempt to validate via `cc -fsyntax-only`; skip silently when
    // `cc` is not available on PATH (e.g. locked-down CI images).
    use std::io::Write;
    use std::process::{Command, Stdio};
    let cc = Command::new("cc")
        .arg("-fsyntax-only")
        .arg("-w")
        .arg("-xc")
        .arg("-")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn();
    let mut child = match cc {
        Ok(c) => c,
        Err(_) => {
            eprintln!("cc not available; skipping syntax check");
            return;
        }
    };
    child
        .stdin
        .as_mut()
        .expect("stdin")
        .write_all(src.as_bytes())
        .expect("write");
    let out = child.wait_with_output().expect("wait");
    assert!(
        out.status.success(),
        "cc rejected AST-rendered body:\n---source---\n{src}\n---stderr---\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn rendering_is_byte_deterministic() {
    let a = build_reference_body();
    let b = build_reference_body();
    let mut out_a = String::new();
    let mut out_b = String::new();
    a.render_into(&mut out_a, 0);
    b.render_into(&mut out_b, 0);
    assert_eq!(out_a, out_b);
    assert!(!out_a.lines().any(|l| l.ends_with(' ')));
}
