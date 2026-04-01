//! C code emission in three modes.
//!
//! 1. **Naive** -- direct basic-block-to-C with goto labels (baseline).
//! 2. **Structured** -- from the structured AST (if/else/while/switch).
//! 3. **Decorated** -- structured + HIR-derived variable names, OS-specific
//!    idioms, and comments linking back to DSL constructs.

use std::collections::HashMap;
use std::fmt::Write;

use super::types::{CExpr, CStmt, HirFuncInfo, HirSummary, MirFunc};

// ===== Mode 1: Naive (goto-based) ==========================================

/// Emit a function as flat basic blocks with goto labels.
pub fn emit_naive(func: &MirFunc) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "/* NAIVE (goto-based) -- {} */", func.name);
    let _ = writeln!(out, "void {}(void) {{", mangle_name(&func.name));

    // Declare locals.
    for local in &func.locals {
        let name = func
            .debug_names
            .get(&local.index)
            .cloned()
            .unwrap_or_else(|| format!("_{}", local.index));
        let _ = writeln!(out, "    {} {};  /* _{} */", c_type(&local.ty), name, local.index);
    }
    let _ = writeln!(out);

    for blk in &func.blocks {
        if blk.is_cleanup {
            let _ = writeln!(out, "  bb{}: /* cleanup */", blk.index);
        } else {
            let _ = writeln!(out, "  bb{}:", blk.index);
        }
        for stmt in &blk.statements {
            let _ = writeln!(out, "    {};", stmt);
        }
        let _ = writeln!(out, "    {};", blk.terminator);
        let _ = writeln!(out);
    }

    let _ = writeln!(out, "}}");
    out
}

// ===== Mode 2: Structured ==================================================

/// Emit a function using the structured AST (no goto).
pub fn emit_structured(func: &MirFunc, body: &[CStmt]) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "/* STRUCTURED -- {} */", func.name);
    let _ = writeln!(out, "void {}(void) {{", mangle_name(&func.name));
    emit_stmts(&mut out, body, 1, &HashMap::new());
    let _ = writeln!(out, "}}");
    out
}

// ===== Mode 3: Decorated (structured + HIR info) ============================

/// Emit a function using the structured AST, decorated with HIR information:
/// source variable names, DSL-role comments, and OS-flavoured idioms.
pub fn emit_decorated(
    func: &MirFunc,
    body: &[CStmt],
    hir: &HirSummary,
) -> String {
    let hir_info = hir
        .functions
        .iter()
        .find(|f| f.name == func.name || func.name.ends_with(&f.short_name));

    let mut out = String::new();

    // Header comment from DSL attributes.
    if let Some(info) = hir_info {
        for attr in &info.rseq_attrs {
            let _ = writeln!(out, "/* @rseq::{}{} */",
                attr.kind,
                attr.args.as_ref().map(|a| format!("({a})")).unwrap_or_default(),
            );
        }
    }

    // Function signature using HIR info where available.
    let sig = if let Some(info) = hir_info {
        format_decorated_signature(info, func)
    } else {
        format!("void {}(void)", mangle_name(&func.name))
    };

    let _ = writeln!(out, "{sig} {{");

    // Build name map: local index -> decorated name.
    let name_map = build_decorated_names(func, hir_info);

    emit_stmts(&mut out, body, 1, &name_map);
    let _ = writeln!(out, "}}");
    out
}

// ===== Shared emission helpers =============================================

fn emit_stmts(
    out: &mut String,
    stmts: &[CStmt],
    indent: usize,
    names: &HashMap<usize, String>,
) {
    let pad = "    ".repeat(indent);
    for stmt in stmts {
        match stmt {
            CStmt::Comment(text) => {
                let _ = writeln!(out, "{pad}/* {text} */");
            }
            CStmt::VarDecl { name, ty, init } => {
                if let Some(init) = init {
                    let _ = writeln!(out, "{pad}{ty} {name} = {};", render_expr(init, names));
                } else {
                    let _ = writeln!(out, "{pad}{ty} {name};");
                }
            }
            CStmt::Assign { lhs, rhs } => {
                let _ = writeln!(
                    out,
                    "{pad}{} = {};",
                    render_expr(lhs, names),
                    render_expr(rhs, names)
                );
            }
            CStmt::If {
                cond,
                then_body,
                else_body,
            } => {
                let _ = writeln!(out, "{pad}if ({}) {{", render_expr(cond, names));
                emit_stmts(out, then_body, indent + 1, names);
                if !else_body.is_empty() {
                    let _ = writeln!(out, "{pad}}} else {{");
                    emit_stmts(out, else_body, indent + 1, names);
                }
                let _ = writeln!(out, "{pad}}}");
            }
            CStmt::While { cond, body } => {
                let _ = writeln!(out, "{pad}while ({}) {{", render_expr(cond, names));
                emit_stmts(out, body, indent + 1, names);
                let _ = writeln!(out, "{pad}}}");
            }
            CStmt::Switch {
                scrutinee,
                cases,
                default,
            } => {
                let _ = writeln!(out, "{pad}switch ({}) {{", render_expr(scrutinee, names));
                for (label, body) in cases {
                    let _ = writeln!(out, "{pad}case {label}:");
                    emit_stmts(out, body, indent + 1, names);
                    let _ = writeln!(out, "{pad}    break;");
                }
                if !default.is_empty() {
                    let _ = writeln!(out, "{pad}default:");
                    emit_stmts(out, default, indent + 1, names);
                    let _ = writeln!(out, "{pad}    break;");
                }
                let _ = writeln!(out, "{pad}}}");
            }
            CStmt::Return(expr) => {
                if let Some(e) = expr {
                    let _ = writeln!(out, "{pad}return {};", render_expr(e, names));
                } else {
                    let _ = writeln!(out, "{pad}return;");
                }
            }
            CStmt::Expr(expr) => {
                let _ = writeln!(out, "{pad}{};", render_expr(expr, names));
            }
            CStmt::Drop { place } => {
                let _ = writeln!(out, "{pad}drop({});", render_expr(place, names));
            }
            CStmt::Break => {
                let _ = writeln!(out, "{pad}break;");
            }
            CStmt::Goto(label) => {
                let _ = writeln!(out, "{pad}goto {label};");
            }
            CStmt::Label(label) => {
                let _ = writeln!(out, "{label}:");
            }
        }
    }
}

// ===== CExpr rendering ====================================================

/// Render a CExpr to a string, substituting local names where available.
fn render_expr(expr: &CExpr, names: &HashMap<usize, String>) -> String {
    match expr {
        CExpr::Local(n) => names
            .get(n)
            .cloned()
            .unwrap_or_else(|| format!("_{n}")),
        CExpr::Literal(s) => s.clone(),
        CExpr::BinOp { op, lhs, rhs } => {
            format!("{} {op} {}", render_expr(lhs, names), render_expr(rhs, names))
        }
        CExpr::UnaryOp { op, operand } => {
            format!("{op}({})", render_expr(operand, names))
        }
        CExpr::Call { func, args } => {
            let args_str: Vec<String> = args.iter().map(|a| render_expr(a, names)).collect();
            format!("{func}({})", args_str.join(", "))
        }
        CExpr::Field { base, field } => {
            format!("{}.{field}", render_expr(base, names))
        }
        CExpr::AddrOf(inner) => {
            format!("&{}", render_expr(inner, names))
        }
        CExpr::Deref(inner) => {
            format!("(*{})", render_expr(inner, names))
        }
        CExpr::Cast { operand, ty } => {
            format!("({ty}){}", render_expr(operand, names))
        }
        CExpr::Aggregate { name, fields } => {
            let fields_str: Vec<String> =
                fields.iter().map(|f| render_expr(f, names)).collect();
            format!("{name} {{ {} }}", fields_str.join(", "))
        }
        CExpr::Unit => "()".to_string(),
    }
}

// ===== Decoration helpers ==================================================

fn format_decorated_signature(info: &HirFuncInfo, func: &MirFunc) -> String {
    // Use MIR local _0 type for return type (resolved, not HIR debug).
    let ret = func
        .locals
        .first()
        .map(|l| c_type(&l.ty))
        .unwrap_or_else(|| "void".to_string());

    let params: Vec<String> = func
        .locals
        .iter()
        .filter(|l| l.is_arg)
        .map(|l| {
            let name = func
                .debug_names
                .get(&l.index)
                .cloned()
                .unwrap_or_else(|| format!("_{}", l.index));
            format!("{} {}", c_type(&l.ty), name)
        })
        .collect();
    let params_str = if params.is_empty() {
        "void".to_string()
    } else {
        params.join(", ")
    };
    format!("{ret} {name}({params_str})", name = info.short_name)
}

fn build_decorated_names(
    func: &MirFunc,
    _hir_info: Option<&HirFuncInfo>,
) -> HashMap<usize, String> {
    // Use debug names as the primary source.
    func.debug_names.clone()
}

/// Rough Rust-type-to-C-type mapping for display purposes.
fn c_type(rust_ty: &str) -> String {
    match rust_ty {
        "usize" => "size_t".to_string(),
        "isize" => "ssize_t".to_string(),
        "u8" => "uint8_t".to_string(),
        "u16" => "uint16_t".to_string(),
        "u32" => "uint32_t".to_string(),
        "u64" => "uint64_t".to_string(),
        "i8" => "int8_t".to_string(),
        "i16" => "int16_t".to_string(),
        "i32" => "int32_t".to_string(),
        "i64" => "int64_t".to_string(),
        "bool" => "bool".to_string(),
        "()" => "void".to_string(),
        other => other.replace("::", "_").to_string(),
    }
}

fn mangle_name(name: &str) -> String {
    name.replace("::", "_")
}
