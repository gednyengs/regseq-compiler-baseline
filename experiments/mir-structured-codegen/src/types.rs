//! Rustc-independent intermediate types for the codegen experiment.

use std::collections::HashMap;
use std::fmt;

// ---------------------------------------------------------------------------
// HIR-level information
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct RseqAttr {
    pub kind: String,
    pub args: Option<String>,
}

#[derive(Debug, Clone)]
pub struct HirFuncInfo {
    pub name: String,
    pub short_name: String,
    pub rseq_attrs: Vec<RseqAttr>,
    pub params: Vec<(String, String)>,
    pub return_ty: String,
}

#[derive(Debug, Clone)]
pub struct HirStructInfo {
    pub name: String,
    pub rseq_attrs: Vec<RseqAttr>,
    pub fields: Vec<(String, String)>,
}

#[derive(Debug, Clone, Default)]
pub struct HirSummary {
    pub structs: Vec<HirStructInfo>,
    pub functions: Vec<HirFuncInfo>,
}

// ---------------------------------------------------------------------------
// Expression tree (shared between MIR and C layers)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum CExpr {
    /// Reference to a local: _0, _1, ...
    Local(usize),
    /// A literal value: 42_u32, true, etc.
    Literal(String),
    /// Binary operation: lhs op rhs.
    BinOp {
        op: String,
        lhs: Box<CExpr>,
        rhs: Box<CExpr>,
    },
    /// Unary operation: op(operand).
    UnaryOp { op: String, operand: Box<CExpr> },
    /// Function / method call.
    Call { func: String, args: Vec<CExpr> },
    /// Field access: base.field.
    Field { base: Box<CExpr>, field: String },
    /// Address-of / reference: &expr.
    AddrOf(Box<CExpr>),
    /// Pointer dereference: *expr.
    Deref(Box<CExpr>),
    /// Type cast: (ty)expr.
    Cast { operand: Box<CExpr>, ty: String },
    /// Aggregate (struct / enum variant / tuple) construction.
    Aggregate { name: String, fields: Vec<CExpr> },
    /// Unit value `()`.
    Unit,
}

impl CExpr {
    /// Returns the local index if this is `CExpr::Local(n)`.
    pub fn as_local(&self) -> Option<usize> {
        match self {
            Self::Local(n) => Some(*n),
            _ => None,
        }
    }

    /// Collect all local indices that appear in this expression (reads).
    pub fn locals_read(&self, out: &mut Vec<usize>) {
        match self {
            Self::Local(n) => out.push(*n),
            Self::Literal(_) | Self::Unit => {}
            Self::BinOp { lhs, rhs, .. } => {
                lhs.locals_read(out);
                rhs.locals_read(out);
            }
            Self::UnaryOp { operand, .. } => operand.locals_read(out),
            Self::Call { args, .. } => {
                for a in args {
                    a.locals_read(out);
                }
            }
            Self::Field { base, .. } => base.locals_read(out),
            Self::AddrOf(inner) | Self::Deref(inner) => inner.locals_read(out),
            Self::Cast { operand, .. } => operand.locals_read(out),
            Self::Aggregate { fields, .. } => {
                for f in fields {
                    f.locals_read(out);
                }
            }
        }
    }

    /// Replace every occurrence of `Local(target)` with `replacement`.
    pub fn substitute(&mut self, target: usize, replacement: &CExpr) {
        match self {
            Self::Local(n) if *n == target => {
                *self = replacement.clone();
            }
            Self::Local(_) | Self::Literal(_) | Self::Unit => {}
            Self::BinOp { lhs, rhs, .. } => {
                lhs.substitute(target, replacement);
                rhs.substitute(target, replacement);
            }
            Self::UnaryOp { operand, .. } => operand.substitute(target, replacement),
            Self::Call { args, .. } => {
                for a in args {
                    a.substitute(target, replacement);
                }
            }
            Self::Field { base, .. } => base.substitute(target, replacement),
            Self::AddrOf(inner) | Self::Deref(inner) => {
                inner.substitute(target, replacement);
            }
            Self::Cast { operand, .. } => operand.substitute(target, replacement),
            Self::Aggregate { fields, .. } => {
                for f in fields {
                    f.substitute(target, replacement);
                }
            }
        }
    }
}

impl fmt::Display for CExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Local(n) => write!(f, "_{n}"),
            Self::Literal(s) => write!(f, "{s}"),
            Self::BinOp { op, lhs, rhs } => write!(f, "{lhs} {op} {rhs}"),
            Self::UnaryOp { op, operand } => write!(f, "{op}({operand})"),
            Self::Call { func, args } => {
                write!(f, "{func}(")?;
                for (i, a) in args.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{a}")?;
                }
                write!(f, ")")
            }
            Self::Field { base, field } => write!(f, "{base}.{field}"),
            Self::AddrOf(inner) => write!(f, "&{inner}"),
            Self::Deref(inner) => write!(f, "(*{inner})"),
            Self::Cast { operand, ty } => write!(f, "({ty}){operand}"),
            Self::Aggregate { name, fields } => {
                write!(f, "{name} {{ ")?;
                for (i, v) in fields.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{v}")?;
                }
                write!(f, " }}")
            }
            Self::Unit => write!(f, "()"),
        }
    }
}

// ---------------------------------------------------------------------------
// MIR-level information (simplified)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct LocalInfo {
    pub index: usize,
    pub ty: String,
    pub is_arg: bool,
}

#[derive(Debug, Clone)]
pub enum MirStmt {
    Assign { lhs: CExpr, rhs: CExpr },
    StorageLive(usize),
    StorageDead(usize),
    Other(String),
}

#[derive(Debug, Clone)]
pub enum Unwind {
    Cleanup(usize),
    Continue,
    Unreachable,
    Terminate,
}

#[derive(Debug, Clone)]
pub enum MirTerminator {
    Goto(usize),
    Return,
    Unreachable,
    SwitchInt {
        discr: CExpr,
        targets: Vec<(u128, usize)>,
        otherwise: usize,
    },
    Drop {
        place: CExpr,
        target: usize,
        unwind: Unwind,
    },
    Call {
        func: String,
        args: Vec<CExpr>,
        destination: CExpr,
        target: Option<usize>,
        unwind: Unwind,
    },
    Assert {
        cond: CExpr,
        expected: bool,
        target: usize,
        unwind: Unwind,
    },
}

#[derive(Debug, Clone)]
pub struct MirBlock {
    pub index: usize,
    pub statements: Vec<MirStmt>,
    pub terminator: MirTerminator,
    pub is_cleanup: bool,
}

#[derive(Debug, Clone)]
pub struct MirFunc {
    pub name: String,
    pub locals: Vec<LocalInfo>,
    pub arg_count: usize,
    pub blocks: Vec<MirBlock>,
    pub debug_names: HashMap<usize, String>,
}

// ---------------------------------------------------------------------------
// Structured C-like AST
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum CStmt {
    Comment(String),
    VarDecl {
        name: String,
        ty: String,
        init: Option<CExpr>,
    },
    Assign {
        lhs: CExpr,
        rhs: CExpr,
    },
    If {
        cond: CExpr,
        then_body: Vec<CStmt>,
        else_body: Vec<CStmt>,
    },
    While {
        cond: CExpr,
        body: Vec<CStmt>,
    },
    Switch {
        scrutinee: CExpr,
        cases: Vec<(String, Vec<CStmt>)>,
        default: Vec<CStmt>,
    },
    Return(Option<CExpr>),
    /// A bare expression used as a statement (e.g. function call).
    Expr(CExpr),
    /// Explicit resource cleanup.
    Drop {
        place: CExpr,
    },
    /// `break;`
    Break,
    /// Fallback: `goto label;`
    Goto(String),
    /// Fallback: `label:`
    Label(String),
}

// ---------------------------------------------------------------------------
// Display helpers (for the raw MIR dump section of the report)
// ---------------------------------------------------------------------------

impl fmt::Display for MirStmt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Assign { lhs, rhs } => write!(f, "{lhs} = {rhs}"),
            Self::StorageLive(l) => write!(f, "StorageLive(_{l})"),
            Self::StorageDead(l) => write!(f, "StorageDead(_{l})"),
            Self::Other(s) => write!(f, "{s}"),
        }
    }
}

impl fmt::Display for MirTerminator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Goto(t) => write!(f, "goto -> bb{t}"),
            Self::Return => write!(f, "return"),
            Self::Unreachable => write!(f, "unreachable"),
            Self::SwitchInt {
                discr,
                targets,
                otherwise,
            } => {
                write!(f, "switchInt({discr}) -> [")?;
                for (i, (val, bb)) in targets.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{val}: bb{bb}")?;
                }
                write!(f, ", otherwise: bb{otherwise}]")
            }
            Self::Drop {
                place, target, ..
            } => write!(f, "drop({place}) -> bb{target}"),
            Self::Call {
                func,
                args,
                destination,
                target,
                ..
            } => {
                write!(f, "{destination} = {func}(")?;
                for (i, a) in args.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{a}")?;
                }
                write!(f, ")")?;
                if let Some(t) = target {
                    write!(f, " -> bb{t}")?;
                }
                Ok(())
            }
            Self::Assert {
                cond,
                expected,
                target,
                ..
            } => write!(f, "assert({cond} == {expected}) -> bb{target}"),
        }
    }
}
