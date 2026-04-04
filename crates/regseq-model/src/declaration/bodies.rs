//! Normalized authored function body declarations.
//!
//! These types preserve the authored body structure from frontend extraction
//! without collapsing into device-specific, OS-specific, or backend-oriented
//! lowering decisions. They are the declaration-model counterpart to the raw
//! body capture in [`super::raw_input`].
//!
//! The body model preserves per-function identity, callback-vs-helper role,
//! helper-call graph edges, local declarations, assignments, control flow,
//! returns, and expressions referencing registers, properties, and resources
//! as authored constructs.

use serde::{Deserialize, Serialize};

use super::provenance::Provenance;

/// Set of normalized authored function body declarations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BodyDeclSet {
    /// All extracted function bodies in declaration order.
    pub functions: Vec<FnBodyDecl>,
}

/// A normalized authored function body declaration.
///
/// Preserves the authored function structure including its role (callback,
/// ISR, helper, free function), parameters, and full body expression tree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FnBodyDecl {
    /// Function name as declared in the spec.
    pub name: String,
    /// Function kind: callback, ISR, helper, or free function.
    pub kind: FnKind,
    /// Receiver pattern (e.g. `"&mut self"`, `"&self"`, `"none"`).
    pub receiver: String,
    /// Function parameters (excluding receiver).
    pub params: Vec<FnParamDecl>,
    /// Return type, if explicitly declared.
    pub return_type: Option<String>,
    /// The function body expression tree.
    pub body: BodyExpr,
    /// Provenance back to the raw artifact.
    pub provenance: Provenance,
}

/// Function kind distinguishing callbacks from helpers and ISR handlers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum FnKind {
    /// A trait callback method (e.g. `Lifecycle::init`, `Serial::tx_byte`).
    Callback {
        /// Trait name (e.g. `"Lifecycle"`, `"Serial"`).
        trait_name: String,
        /// Method name within the trait (e.g. `"init"`, `"tx_byte"`).
        method_name: String,
    },
    /// An interrupt service routine annotated with `#[rseq::isr]`.
    Isr,
    /// A reachable helper function called by callbacks or the ISR.
    Helper,
    /// A free (non-method) function reachable from the supported subset.
    FreeFunction,
}

/// A function parameter declaration (excluding receiver).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FnParamDecl {
    /// Parameter name.
    pub name: String,
    /// Parameter type as a string (e.g. `"u8"`, `"&SerialConfig"`).
    pub ty: String,
}

/// Normalized body statement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum BodyStmt {
    /// An expression used as a statement (trailing value position).
    Expr {
        /// The expression.
        expr: BodyExpr,
    },
    /// A semicolon-terminated expression statement.
    Semi {
        /// The expression.
        expr: BodyExpr,
    },
    /// A `let` binding.
    Let {
        /// Binding name.
        name: String,
        /// Whether the binding is mutable.
        mutable: bool,
        /// Optional type annotation.
        #[serde(default)]
        ty: Option<String>,
        /// Optional initializer expression.
        #[serde(default)]
        init: Option<BodyExpr>,
    },
}

/// Normalized body expression tree.
///
/// Preserves the authored expression structure without introducing any
/// device-specific, OS-specific, or lowering-oriented semantic nodes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum BodyExpr {
    /// A block expression `{ stmts; expr }`.
    Block {
        /// Statements in the block.
        stmts: Vec<BodyStmt>,
        /// Optional trailing expression (block value).
        #[serde(default)]
        expr: Option<Box<BodyExpr>>,
    },
    /// A method call `receiver.method::<type_args>(args)`.
    MethodCall {
        /// Receiver expression.
        receiver: Box<BodyExpr>,
        /// Method name.
        method: String,
        /// Type arguments, if any.
        #[serde(default)]
        type_args: Vec<String>,
        /// Call arguments.
        args: Vec<BodyExpr>,
    },
    /// A function or path call `path::<type_args>(args)`.
    Call {
        /// Called path.
        path: String,
        /// Type arguments, if any.
        #[serde(default)]
        type_args: Vec<String>,
        /// Call arguments.
        args: Vec<BodyExpr>,
    },
    /// A field access `base.name`.
    Field {
        /// Base expression.
        base: Box<BodyExpr>,
        /// Field name.
        name: String,
    },
    /// A path expression (variable, constant, enum variant).
    Path {
        /// The path string.
        path: String,
    },
    /// An integer literal.
    LitInt {
        /// The literal value as a string.
        value: String,
        /// Optional type suffix (e.g. `"u32"`, `"u64"`).
        #[serde(default)]
        suffix: Option<String>,
    },
    /// A boolean literal.
    LitBool {
        /// The boolean value.
        value: bool,
    },
    /// A string literal.
    LitStr {
        /// The string value.
        value: String,
    },
    /// A binary operation `lhs op rhs`.
    Binary {
        /// Operator (e.g. `"+"`, `"|"`, `"!="`, `"&&"`).
        op: String,
        /// Left-hand side.
        lhs: Box<BodyExpr>,
        /// Right-hand side.
        rhs: Box<BodyExpr>,
    },
    /// A unary operation `op operand`.
    Unary {
        /// Operator (e.g. `"!"`, `"-"`).
        op: String,
        /// Operand.
        operand: Box<BodyExpr>,
    },
    /// A type cast `expr as ty`.
    Cast {
        /// Expression being cast.
        expr: Box<BodyExpr>,
        /// Target type.
        ty: String,
    },
    /// An `if` expression with optional `else` branch.
    If {
        /// Condition.
        cond: Box<BodyExpr>,
        /// Then branch.
        then_branch: Box<BodyExpr>,
        /// Optional else branch.
        #[serde(default)]
        else_branch: Option<Box<BodyExpr>>,
    },
    /// A `loop` expression.
    Loop {
        /// Loop body.
        body: Box<BodyExpr>,
    },
    /// A `return` expression.
    Return {
        /// Optional return value.
        #[serde(default)]
        expr: Option<Box<BodyExpr>>,
    },
    /// A `?` try operator.
    Try {
        /// Inner expression.
        expr: Box<BodyExpr>,
    },
    /// A reference `&expr` or `&mut expr`.
    Ref {
        /// Whether this is a mutable reference.
        mutable: bool,
        /// Referenced expression.
        expr: Box<BodyExpr>,
    },
    /// An assignment `target = value`.
    Assign {
        /// Assignment target.
        target: Box<BodyExpr>,
        /// Assigned value.
        value: Box<BodyExpr>,
    },
    /// A compound assignment `target op= value`.
    AssignOp {
        /// Operator (e.g. `"|="`).
        op: String,
        /// Assignment target.
        target: Box<BodyExpr>,
        /// Assigned value.
        value: Box<BodyExpr>,
    },
    /// A tuple expression `(a, b, ...)`.
    Tuple {
        /// Tuple elements.
        elements: Vec<BodyExpr>,
    },
    /// An array expression `[a, b, ...]`.
    Array {
        /// Array elements.
        elements: Vec<BodyExpr>,
    },
    /// An index expression `base[index]`.
    Index {
        /// Base expression.
        base: Box<BodyExpr>,
        /// Index expression.
        index: Box<BodyExpr>,
    },
    /// A struct literal expression.
    Struct {
        /// Struct path.
        path: String,
        /// Named fields.
        fields: Vec<StructFieldExpr>,
        /// Optional functional-update base (`.. base`).
        #[serde(default)]
        base: Option<Box<BodyExpr>>,
    },
    /// A closure expression `|params| body`.
    Closure {
        /// Closure parameter names.
        params: Vec<String>,
        /// Closure body.
        body: Box<BodyExpr>,
    },
    /// A `match` expression.
    Match {
        /// Scrutinee expression.
        scrutinee: Box<BodyExpr>,
        /// Match arms.
        arms: Vec<MatchArm>,
    },
    /// A `break` expression.
    Break {
        /// Optional break value.
        #[serde(default)]
        expr: Option<Box<BodyExpr>>,
    },
    /// A `continue` expression.
    Continue,
    /// An unsupported HIR node preserved for diagnostics.
    Unsupported {
        /// The HIR node kind string.
        hir_kind: String,
        /// Optional source span text.
        #[serde(default)]
        span_text: Option<String>,
    },
}

/// A named field in a struct literal expression.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StructFieldExpr {
    /// Field name.
    pub name: String,
    /// Field value expression.
    pub value: BodyExpr,
}

/// A match arm with pattern, optional guard, and body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MatchArm {
    /// Pattern string.
    pub pattern: String,
    /// Optional guard expression.
    #[serde(default)]
    pub guard: Option<BodyExpr>,
    /// Arm body expression.
    pub body: BodyExpr,
}

impl FnBodyDecl {
    /// Return whether this function is a callback (trait method).
    pub fn is_callback(&self) -> bool {
        matches!(self.kind, FnKind::Callback { .. })
    }

    /// Return whether this function is an ISR handler.
    pub fn is_isr(&self) -> bool {
        matches!(self.kind, FnKind::Isr)
    }

    /// Return whether this function is a helper.
    pub fn is_helper(&self) -> bool {
        matches!(self.kind, FnKind::Helper)
    }

    /// Return whether this function is a free function.
    pub fn is_free_function(&self) -> bool {
        matches!(self.kind, FnKind::FreeFunction)
    }
}

impl BodyDeclSet {
    /// Return all callback function bodies.
    pub fn callbacks(&self) -> Vec<&FnBodyDecl> {
        self.functions.iter().filter(|f| f.is_callback()).collect()
    }

    /// Return all helper function bodies.
    pub fn helpers(&self) -> Vec<&FnBodyDecl> {
        self.functions.iter().filter(|f| f.is_helper()).collect()
    }

    /// Return all ISR function bodies.
    pub fn isrs(&self) -> Vec<&FnBodyDecl> {
        self.functions.iter().filter(|f| f.is_isr()).collect()
    }

    /// Return all free function bodies.
    pub fn free_functions(&self) -> Vec<&FnBodyDecl> {
        self.functions
            .iter()
            .filter(|f| f.is_free_function())
            .collect()
    }

    /// Look up a function body by name.
    pub fn by_name(&self, name: &str) -> Option<&FnBodyDecl> {
        self.functions.iter().find(|f| f.name == name)
    }
}

/// Collect all function names that appear as call targets within a body expression.
///
/// This traverses the expression tree and collects method-call and function-call
/// names, which can be used to build helper-call graph edges.
pub fn collect_call_targets(expr: &BodyExpr) -> Vec<String> {
    let mut targets = Vec::new();
    collect_call_targets_inner(expr, &mut targets);
    targets.sort();
    targets.dedup();
    targets
}

fn collect_call_targets_inner(expr: &BodyExpr, targets: &mut Vec<String>) {
    match expr {
        BodyExpr::Block { stmts, expr } => {
            for stmt in stmts {
                match stmt {
                    BodyStmt::Expr { expr } | BodyStmt::Semi { expr } => {
                        collect_call_targets_inner(expr, targets);
                    }
                    BodyStmt::Let { init, .. } => {
                        if let Some(init) = init {
                            collect_call_targets_inner(init, targets);
                        }
                    }
                }
            }
            if let Some(e) = expr {
                collect_call_targets_inner(e, targets);
            }
        }
        BodyExpr::MethodCall {
            receiver,
            method,
            args,
            ..
        } => {
            targets.push(method.clone());
            collect_call_targets_inner(receiver, targets);
            for arg in args {
                collect_call_targets_inner(arg, targets);
            }
        }
        BodyExpr::Call { path, args, .. } => {
            targets.push(path.clone());
            for arg in args {
                collect_call_targets_inner(arg, targets);
            }
        }
        BodyExpr::Field { base, .. } => {
            collect_call_targets_inner(base, targets);
        }
        BodyExpr::Binary { lhs, rhs, .. } => {
            collect_call_targets_inner(lhs, targets);
            collect_call_targets_inner(rhs, targets);
        }
        BodyExpr::Unary { operand, .. } => {
            collect_call_targets_inner(operand, targets);
        }
        BodyExpr::Cast { expr, .. } => {
            collect_call_targets_inner(expr, targets);
        }
        BodyExpr::If {
            cond,
            then_branch,
            else_branch,
        } => {
            collect_call_targets_inner(cond, targets);
            collect_call_targets_inner(then_branch, targets);
            if let Some(e) = else_branch {
                collect_call_targets_inner(e, targets);
            }
        }
        BodyExpr::Loop { body } => {
            collect_call_targets_inner(body, targets);
        }
        BodyExpr::Return { expr } => {
            if let Some(e) = expr {
                collect_call_targets_inner(e, targets);
            }
        }
        BodyExpr::Try { expr } => {
            collect_call_targets_inner(expr, targets);
        }
        BodyExpr::Ref { expr, .. } => {
            collect_call_targets_inner(expr, targets);
        }
        BodyExpr::Assign { target, value } => {
            collect_call_targets_inner(target, targets);
            collect_call_targets_inner(value, targets);
        }
        BodyExpr::AssignOp { target, value, .. } => {
            collect_call_targets_inner(target, targets);
            collect_call_targets_inner(value, targets);
        }
        BodyExpr::Tuple { elements } | BodyExpr::Array { elements } => {
            for e in elements {
                collect_call_targets_inner(e, targets);
            }
        }
        BodyExpr::Index { base, index } => {
            collect_call_targets_inner(base, targets);
            collect_call_targets_inner(index, targets);
        }
        BodyExpr::Struct { fields, base, .. } => {
            for f in fields {
                collect_call_targets_inner(&f.value, targets);
            }
            if let Some(b) = base {
                collect_call_targets_inner(b, targets);
            }
        }
        BodyExpr::Closure { body, .. } => {
            collect_call_targets_inner(body, targets);
        }
        BodyExpr::Match { scrutinee, arms } => {
            collect_call_targets_inner(scrutinee, targets);
            for arm in arms {
                if let Some(g) = &arm.guard {
                    collect_call_targets_inner(g, targets);
                }
                collect_call_targets_inner(&arm.body, targets);
            }
        }
        BodyExpr::Break { expr } => {
            if let Some(e) = expr {
                collect_call_targets_inner(e, targets);
            }
        }
        BodyExpr::Path { .. }
        | BodyExpr::LitInt { .. }
        | BodyExpr::LitBool { .. }
        | BodyExpr::LitStr { .. }
        | BodyExpr::Continue
        | BodyExpr::Unsupported { .. } => {}
    }
}
