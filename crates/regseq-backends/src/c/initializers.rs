//! C initializer AST (ISO C11 6.7.9).
//!
//! Supports scalar initializers (`<expr>`) and braced list initializers
//! with optional field / index designators (`{ .f = ..., [0] = ... }`).
//! This is the Phase-7C subset needed to replace the current
//! `Vec<String>` initializer bodies in `emit_module.rs`.

use super::expressions::CExpr;

/// A C initializer (ISO C11 6.7.9 initializer).
#[derive(Debug, Clone)]
pub enum CInitializer {
    /// Scalar initializer: `<expr>`.
    Expr(CExpr),
    /// Braced list initializer: `{ <item>, <item>, ... }`.
    List(Vec<CInitItem>),
}

/// A single item in a list initializer (6.7.9 designation + initializer).
#[derive(Debug, Clone)]
pub struct CInitItem {
    /// Optional designator (`.f =` or `[i] =`).
    pub designator: Option<CDesignator>,
    /// Item initializer (may itself be a nested list).
    pub init: CInitializer,
}

/// A designator (ISO C11 6.7.9 designator).
#[derive(Debug, Clone)]
pub enum CDesignator {
    /// `.<field>`.
    Field(String),
    /// `[<expr>]`.
    Index(CExpr),
}

impl CDesignator {
    /// Render the designator fragment `.f` or `[i]` (without the
    /// trailing `=`).
    pub fn render_into(&self, out: &mut String) {
        match self {
            Self::Field(name) => {
                out.push('.');
                out.push_str(name);
            }
            Self::Index(expr) => {
                out.push('[');
                expr.render_into(out);
                out.push(']');
            }
        }
    }
}

impl CInitializer {
    /// Render the initializer at the given indent level. Scalar
    /// initializers emit on the current line. List initializers emit
    /// a multi-line block:
    ///
    /// ```text
    /// {
    ///     .f = value,
    ///     [0] = other,
    /// }
    /// ```
    ///
    /// The caller is responsible for any leading whitespace before the
    /// initializer (e.g. the `= ` in a declaration). Indent levels are
    /// applied to list items (one extra tab beyond `indent`).
    pub fn render_into(&self, out: &mut String, indent: usize) {
        match self {
            Self::Expr(expr) => expr.render_into(out),
            Self::List(items) => {
                if items.is_empty() {
                    out.push_str("{}");
                    return;
                }
                out.push_str("{\n");
                for item in items {
                    for _ in 0..indent + 1 {
                        out.push('\t');
                    }
                    if let Some(des) = &item.designator {
                        des.render_into(out);
                        out.push_str(" = ");
                    }
                    item.init.render_into(out, indent + 1);
                    out.push(',');
                    out.push('\n');
                }
                for _ in 0..indent {
                    out.push('\t');
                }
                out.push('}');
            }
        }
    }
}
