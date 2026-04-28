//! C statement AST (ISO C11 6.8).
//!
//! Phase-7C subset: compound statements, declarations as block items,
//! expression statements, selection (`if` / `if-else`), iteration
//! (`while`), return, `goto`, labeled statements, empty statement, and a
//! `Raw` escape hatch. Deferred per migration.md Q8: `for` and
//! `do while` iteration, `break` and `continue`, `switch` / `case`,
//! `typedef`, `enum`, `union`, and preprocessor conditionals.

use super::declarators::CDeclaration;
use super::expressions::CExpr;

/// A C statement (6.8 statement).
#[derive(Debug, Clone)]
pub enum CStmt {
    /// Compound statement: `{ <items>... }` (6.8.2).
    Compound {
        /// Block items, rendered in insertion order.
        items: Vec<CBlockItem>,
    },
    /// Declaration statement (6.7 promoted via 6.8.2 block-item).
    Decl(CDeclaration),
    /// Expression statement (6.8.3): `<expr>;`.
    Expr(CExpr),
    /// `if` / `if-else` selection statement (6.8.4.1).
    If {
        /// Condition expression.
        cond: CExpr,
        /// Consequent branch.
        then_branch: Box<CStmt>,
        /// Optional alternative branch.
        else_branch: Option<Box<CStmt>>,
    },
    /// `while` iteration statement (6.8.5.1).
    While {
        /// Loop condition.
        cond: CExpr,
        /// Loop body.
        body: Box<CStmt>,
    },
    /// `return [expr];` jump statement (6.8.6.4).
    Return(Option<CExpr>),
    /// `goto <label>;` jump statement (6.8.6.1).
    Goto(String),
    /// Labeled statement: `<label>: <stmt>` (6.8.1 identifier label).
    Labeled {
        /// Label identifier.
        label: String,
        /// Statement carrying the label.
        stmt: Box<CStmt>,
    },
    /// Empty statement (6.8.3 null-statement): `;`.
    Empty,
    /// Raw C statement text (transitional escape hatch). Appended at the
    /// current indent level with a trailing newline if one is missing.
    Raw(String),
}

/// A block item inside a compound statement (6.8.2 block-item).
#[derive(Debug, Clone)]
pub enum CBlockItem {
    /// Declaration item.
    Decl(CDeclaration),
    /// Statement item.
    Stmt(CStmt),
}

impl CBlockItem {
    /// Render the block item at the given indent level.
    pub fn render_into(&self, out: &mut String, indent: usize) {
        match self {
            Self::Decl(decl) => decl.render_into(out, indent),
            Self::Stmt(stmt) => stmt.render_into(out, indent),
        }
    }
}

impl CStmt {
    /// Render the statement at the given indent level. Each emitted
    /// line begins with `indent` tab characters. Nested compound
    /// bodies render at `indent + 1`. Rendering always terminates
    /// with a newline; callers do not need to add one.
    pub fn render_into(&self, out: &mut String, indent: usize) {
        match self {
            Self::Compound { items } => {
                push_indent(out, indent);
                out.push_str("{\n");
                for item in items {
                    item.render_into(out, indent + 1);
                }
                push_indent(out, indent);
                out.push_str("}\n");
            }
            Self::Decl(decl) => decl.render_into(out, indent),
            Self::Expr(expr) => {
                push_indent(out, indent);
                expr.render_into(out);
                out.push(';');
                out.push('\n');
            }
            Self::If {
                cond,
                then_branch,
                else_branch,
            } => {
                push_indent(out, indent);
                out.push_str("if (");
                cond.render_into(out);
                out.push(')');
                render_if_branch(out, indent, then_branch, else_branch.is_some());
                if let Some(else_b) = else_branch {
                    match then_branch.as_ref() {
                        CStmt::Compound { .. } => out.push_str(" else"),
                        _ => {
                            push_indent(out, indent);
                            out.push_str("else");
                        }
                    }
                    render_if_branch(out, indent, else_b, false);
                }
            }
            Self::While { cond, body } => {
                push_indent(out, indent);
                out.push_str("while (");
                cond.render_into(out);
                out.push(')');
                render_if_branch(out, indent, body, false);
            }
            Self::Return(value) => {
                push_indent(out, indent);
                match value {
                    Some(expr) => {
                        out.push_str("return ");
                        expr.render_into(out);
                        out.push_str(";\n");
                    }
                    None => out.push_str("return;\n"),
                }
            }
            Self::Goto(label) => {
                push_indent(out, indent);
                out.push_str("goto ");
                out.push_str(label);
                out.push_str(";\n");
            }
            Self::Labeled { label, stmt } => {
                // Labels always start at column 0 in idiomatic kernel
                // style; emit without indentation to match.
                out.push_str(label);
                out.push_str(":\n");
                stmt.render_into(out, indent);
            }
            Self::Empty => {
                push_indent(out, indent);
                out.push_str(";\n");
            }
            Self::Raw(text) => {
                push_indent(out, indent);
                out.push_str(text);
                if !text.ends_with('\n') {
                    out.push('\n');
                }
            }
        }
    }
}

fn push_indent(out: &mut String, indent: usize) {
    for _ in 0..indent {
        out.push('\t');
    }
}

/// Render the body of an `if` / `while` / `else` clause, following K&R
/// brace style: the opening brace of a compound body is emitted on the
/// same line as the header keyword, separated by a single space.
///
/// `followed_by_else` signals that an `else` clause follows a `then`
/// body; when true, the closing `}` is emitted without a trailing
/// newline so the caller can append ` else` on the same line.
fn render_if_branch(out: &mut String, indent: usize, stmt: &CStmt, followed_by_else: bool) {
    match stmt {
        CStmt::Compound { items } => {
            out.push_str(" {\n");
            for item in items {
                item.render_into(out, indent + 1);
            }
            push_indent(out, indent);
            if followed_by_else {
                out.push('}');
            } else {
                out.push_str("}\n");
            }
        }
        other => {
            out.push('\n');
            other.render_into(out, indent + 1);
        }
    }
}
