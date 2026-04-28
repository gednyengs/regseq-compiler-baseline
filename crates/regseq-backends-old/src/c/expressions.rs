//! C expression AST (ISO C11 6.5 `expression` and related productions).
//!
//! Covers the Phase-7C subset required to express the operators emitted
//! today by `linux::emit_semantic::emit_expr`: integer and string literals,
//! identifiers, field projection (dot / arrow), array indexing, function
//! calls, unary and binary operators, casts, assignments (plain and
//! compound), and ternary conditionals. Deferred constructs (comma operator,
//! compound-literal expressions, etc.) land with later driver demands.

/// A C expression node.
///
/// Production mapping (C11 6.5):
/// - `IntLiteral` / `StringLiteral` -- 6.4.4.1, 6.4.5 constant / string-literal.
/// - `Ident` -- 6.5.1 primary-expression.
/// - `Field` -- 6.5.2.3 postfix-expression `.` / `->`.
/// - `Index` -- 6.5.2.1 postfix-expression `[ ]`.
/// - `Call` -- 6.5.2.2 postfix-expression function call.
/// - `Unary` -- 6.5.3 unary-expression.
/// - `Binary` -- 6.5.5-6.5.14 multiplicative through logical-OR.
/// - `Cast` -- 6.5.4 cast-expression.
/// - `Assign` -- 6.5.16 assignment-expression (plain and compound).
/// - `Ternary` -- 6.5.15 conditional-expression.
/// - `Paren` -- 6.5.1 primary-expression `( expression )`.
/// - `Raw` -- transitional escape hatch; not a C production.
#[derive(Debug, Clone)]
pub enum CExpr {
    /// Integer literal (6.4.4.1 integer-constant).
    IntLiteral {
        /// Numeric value of the literal. `i128` captures every `u64` and
        /// `i64` value representable in generated drivers.
        value: i128,
        /// Optional integer-suffix (`U`, `L`, `UL`, ...).
        suffix: CIntSuffix,
    },
    /// String literal (6.4.5). The inner value is the unescaped logical
    /// string; the renderer emits the quoted form with C escapes.
    StringLiteral(String),
    /// Identifier reference (6.5.1 primary-expression identifier).
    Ident(String),
    /// Field projection: `<base>.<name>` or `<base>-><name>` (6.5.2.3).
    Field {
        /// Base expression.
        base: Box<CExpr>,
        /// Member name.
        name: String,
        /// `true` emits `->`, `false` emits `.`.
        via_ptr: bool,
    },
    /// Array index: `<base>[<idx>]` (6.5.2.1).
    Index {
        /// Indexed object.
        base: Box<CExpr>,
        /// Index expression.
        idx: Box<CExpr>,
    },
    /// Function call: `<callee>(<args>, ...)` (6.5.2.2).
    Call {
        /// Callee expression (typically an identifier).
        callee: Box<CExpr>,
        /// Argument expressions in positional order.
        args: Vec<CExpr>,
    },
    /// Unary operator (6.5.3).
    Unary {
        /// Operator.
        op: CUnaryOp,
        /// Operand.
        operand: Box<CExpr>,
    },
    /// Binary operator (6.5.5-6.5.14).
    Binary {
        /// Operator.
        op: CBinaryOp,
        /// Left-hand operand.
        lhs: Box<CExpr>,
        /// Right-hand operand.
        rhs: Box<CExpr>,
    },
    /// Cast: `(<type>)<expr>` (6.5.4).
    Cast {
        /// Target type.
        ty: super::declarators::CType,
        /// Cast operand.
        expr: Box<CExpr>,
    },
    /// Assignment (6.5.16). `op == None` emits plain `=`; `op == Some(b)`
    /// emits compound assignment such as `|=` or `<<=`.
    Assign {
        /// Assignment target.
        lhs: Box<CExpr>,
        /// `None` = plain `=`; `Some(op)` = compound-assign with `op`.
        op: Option<CBinaryOp>,
        /// Right-hand-side value.
        rhs: Box<CExpr>,
    },
    /// Ternary conditional: `<cond> ? <then> : <else>` (6.5.15).
    Ternary {
        /// Condition.
        cond: Box<CExpr>,
        /// Consequent branch.
        then_branch: Box<CExpr>,
        /// Alternative branch.
        else_branch: Box<CExpr>,
    },
    /// Explicit parenthesization (6.5.1 `( expression )`). The renderer
    /// already wraps compound sub-expressions in parens; this node exists
    /// only for authoring sites that want forced grouping.
    Paren(Box<CExpr>),
    /// Raw C expression text (transitional escape hatch). Always rendered
    /// verbatim.
    Raw(String),
}

/// Unary operators (ISO C11 6.5.3 unary-operator / unary-expression).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CUnaryOp {
    /// Logical `!`.
    LogicalNot,
    /// Bitwise `~`.
    BitNot,
    /// Arithmetic negation `-`.
    Neg,
    /// Unary plus `+`.
    Plus,
    /// Address-of `&`.
    AddrOf,
    /// Pointer dereference `*`.
    Deref,
}

impl CUnaryOp {
    /// Return the C source form of this unary operator.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::LogicalNot => "!",
            Self::BitNot => "~",
            Self::Neg => "-",
            Self::Plus => "+",
            Self::AddrOf => "&",
            Self::Deref => "*",
        }
    }
}

/// Binary operators (ISO C11 6.5.5-6.5.14 plus 6.5.16.2 compound-assign).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CBinaryOp {
    /// `+`.
    Add,
    /// `-`.
    Sub,
    /// `*`.
    Mul,
    /// `/`.
    Div,
    /// `%`.
    Mod,
    /// `<<`.
    Shl,
    /// `>>`.
    Shr,
    /// `&`.
    BitAnd,
    /// `|`.
    BitOr,
    /// `^`.
    BitXor,
    /// `&&`.
    LogicalAnd,
    /// `||`.
    LogicalOr,
    /// `==`.
    Eq,
    /// `!=`.
    Ne,
    /// `<`.
    Lt,
    /// `<=`.
    Le,
    /// `>`.
    Gt,
    /// `>=`.
    Ge,
}

impl CBinaryOp {
    /// Return the C source form of this binary operator.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Add => "+",
            Self::Sub => "-",
            Self::Mul => "*",
            Self::Div => "/",
            Self::Mod => "%",
            Self::Shl => "<<",
            Self::Shr => ">>",
            Self::BitAnd => "&",
            Self::BitOr => "|",
            Self::BitXor => "^",
            Self::LogicalAnd => "&&",
            Self::LogicalOr => "||",
            Self::Eq => "==",
            Self::Ne => "!=",
            Self::Lt => "<",
            Self::Le => "<=",
            Self::Gt => ">",
            Self::Ge => ">=",
        }
    }
}

/// Integer-constant suffix (ISO C11 6.4.4.1 integer-suffix).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CIntSuffix {
    /// No suffix.
    None,
    /// `U` -- unsigned.
    U,
    /// `L` -- long.
    L,
    /// `UL`.
    UL,
    /// `LL` -- long long.
    LL,
    /// `ULL`.
    ULL,
}

impl CIntSuffix {
    /// Return the literal suffix as a C source fragment.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "",
            Self::U => "U",
            Self::L => "L",
            Self::UL => "UL",
            Self::LL => "LL",
            Self::ULL => "ULL",
        }
    }
}

impl CExpr {
    /// Render this expression into `out`. Expressions do not carry
    /// indentation; the caller is responsible for any leading whitespace.
    ///
    /// Compound sub-expressions (`Unary`, `Binary`, `Cast`, `Assign`,
    /// `Ternary`) are conservatively wrapped in parentheses. This matches
    /// the always-paren convention used by the current string-formatted
    /// emitter and keeps precedence unambiguous without a precedence table.
    pub fn render_into(&self, out: &mut String) {
        match self {
            Self::IntLiteral { value, suffix } => {
                if *value < 0 {
                    out.push('-');
                    out.push_str(&format!("{}", -*value));
                } else {
                    out.push_str(&format!("{}", value));
                }
                out.push_str(suffix.as_str());
            }
            Self::StringLiteral(s) => {
                out.push('"');
                for ch in s.chars() {
                    match ch {
                        '\\' => out.push_str("\\\\"),
                        '"' => out.push_str("\\\""),
                        '\n' => out.push_str("\\n"),
                        '\r' => out.push_str("\\r"),
                        '\t' => out.push_str("\\t"),
                        c if (c as u32) < 0x20 => {
                            out.push_str(&format!("\\x{:02x}", c as u32));
                        }
                        c => out.push(c),
                    }
                }
                out.push('"');
            }
            Self::Ident(name) => out.push_str(name),
            Self::Field {
                base,
                name,
                via_ptr,
            } => {
                base.render_into(out);
                out.push_str(if *via_ptr { "->" } else { "." });
                out.push_str(name);
            }
            Self::Index { base, idx } => {
                base.render_into(out);
                out.push('[');
                idx.render_into(out);
                out.push(']');
            }
            Self::Call { callee, args } => {
                callee.render_into(out);
                out.push('(');
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        out.push_str(", ");
                    }
                    arg.render_into(out);
                }
                out.push(')');
            }
            Self::Unary { op, operand } => {
                out.push('(');
                out.push_str(op.as_str());
                operand.render_into(out);
                out.push(')');
            }
            Self::Binary { op, lhs, rhs } => {
                out.push('(');
                lhs.render_into(out);
                out.push(' ');
                out.push_str(op.as_str());
                out.push(' ');
                rhs.render_into(out);
                out.push(')');
            }
            Self::Cast { ty, expr } => {
                out.push('(');
                out.push('(');
                ty.render_into(out);
                out.push(')');
                expr.render_into(out);
                out.push(')');
            }
            Self::Assign { lhs, op, rhs } => {
                out.push('(');
                lhs.render_into(out);
                match op {
                    None => out.push_str(" = "),
                    Some(b) => {
                        out.push(' ');
                        out.push_str(b.as_str());
                        out.push_str("= ");
                    }
                }
                rhs.render_into(out);
                out.push(')');
            }
            Self::Ternary {
                cond,
                then_branch,
                else_branch,
            } => {
                out.push('(');
                cond.render_into(out);
                out.push_str(" ? ");
                then_branch.render_into(out);
                out.push_str(" : ");
                else_branch.render_into(out);
                out.push(')');
            }
            Self::Paren(inner) => {
                out.push('(');
                inner.render_into(out);
                out.push(')');
            }
            Self::Raw(text) => out.push_str(text),
        }
    }

    /// Convenience: render into a fresh `String`.
    pub fn render(&self) -> String {
        let mut out = String::new();
        self.render_into(&mut out);
        out
    }
}
