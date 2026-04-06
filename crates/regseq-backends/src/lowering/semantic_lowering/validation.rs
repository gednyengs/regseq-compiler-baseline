//! Semantic validation passes.
//!
//! Post-lowering validation enforcing the task spec's required invariants:
//!
//! 1. No `Opaque` backend-facing types in supported bodies
//! 2. `OptionUnwrap` domination validation
//! 3. No stringly lowering-critical refs in produced semantic bodies
//! 4. All `SemanticLocal.name` values are valid C identifiers
//! 5. No raw MIR local spellings (`_3`, `_14`, etc.)
//! 6. Unsupported MIR constructs fail explicitly (enforced during lowering;
//!    post-hoc check for residue)

use regseq_model::semantic::body::{Block, SemanticBody};
use regseq_model::semantic::expr::Expr;
use regseq_model::semantic::stmt::Stmt;
use regseq_model::semantic::types::SemanticType;

// ---------------------------------------------------------------------------
// Validation error
// ---------------------------------------------------------------------------

/// A semantic validation violation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationError {
    /// Name of the function containing the violation.
    pub function: String,
    /// Which validation pass caught this.
    pub pass: ValidationPass,
    /// Description of the violation.
    pub description: String,
}

/// Identifies which validation pass detected a violation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationPass {
    /// Pass 1: no Opaque types.
    NoOpaqueTypes,
    /// Pass 2: OptionUnwrap domination.
    OptionUnwrapDomination,
    /// Pass 3: no stringly refs.
    NoStringlyRefs,
    /// Pass 4: valid C identifiers.
    ValidCIdentifiers,
    /// Pass 5: no raw MIR locals.
    NoRawMirLocals,
    /// Pass 6: no unsupported residue.
    NoUnsupportedResidue,
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "[{:?}] in '{}': {}",
            self.pass, self.function, self.description
        )
    }
}

// ---------------------------------------------------------------------------
// Validation runner
// ---------------------------------------------------------------------------

/// Run all six validation passes on a single function body.
pub fn validate_semantic_body(body: &SemanticBody, function_name: &str) -> Vec<ValidationError> {
    let mut errors = Vec::new();
    errors.extend(validate_no_opaque_types(body, function_name));
    errors.extend(validate_option_unwrap_domination(body, function_name));
    errors.extend(validate_no_stringly_refs(body, function_name));
    errors.extend(validate_valid_c_identifiers(body, function_name));
    errors.extend(validate_no_raw_mir_locals(body, function_name));
    errors.extend(validate_no_unsupported_residue(body, function_name));
    errors
}

// ---------------------------------------------------------------------------
// Pass 1: No Opaque types in supported bodies
// ---------------------------------------------------------------------------

fn validate_no_opaque_types(body: &SemanticBody, fn_name: &str) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    // Check locals. No Opaque types allowed — no carve-outs.
    for local in &body.locals {
        if let SemanticType::Opaque { description } = &local.ty {
            errors.push(ValidationError {
                function: fn_name.into(),
                pass: ValidationPass::NoOpaqueTypes,
                description: format!("local '{}' has Opaque type: {description}", local.name),
            });
        }
    }

    // Check return type.
    if let SemanticType::Opaque { description } = &body.return_type {
        errors.push(ValidationError {
            function: fn_name.into(),
            pass: ValidationPass::NoOpaqueTypes,
            description: format!("return type is Opaque: {description}"),
        });
    }

    // Walk expressions for inline Opaque types.
    walk_block_for_opaque(&body.body, fn_name, &mut errors);

    errors
}

fn walk_block_for_opaque(block: &Block, fn_name: &str, errors: &mut Vec<ValidationError>) {
    for stmt in &block.stmts {
        walk_stmt_for_opaque(stmt, fn_name, errors);
    }
}

fn walk_stmt_for_opaque(stmt: &Stmt, fn_name: &str, errors: &mut Vec<ValidationError>) {
    match stmt {
        Stmt::Let { value, .. } | Stmt::Assign { value, .. } => {
            walk_expr_for_opaque(value, fn_name, errors);
        }
        Stmt::CompoundAssign { value, .. } => {
            walk_expr_for_opaque(value, fn_name, errors);
        }
        Stmt::Eval { expr } => walk_expr_for_opaque(expr, fn_name, errors),
        Stmt::Branch {
            condition,
            then_body,
            else_body,
        } => {
            walk_expr_for_opaque(condition, fn_name, errors);
            walk_block_for_opaque(then_body, fn_name, errors);
            walk_block_for_opaque(else_body, fn_name, errors);
        }
        Stmt::Loop { condition, body } => {
            walk_expr_for_opaque(condition, fn_name, errors);
            walk_block_for_opaque(body, fn_name, errors);
        }
        Stmt::OrderedRegion { body } => walk_block_for_opaque(body, fn_name, errors),
        Stmt::FallibleOp {
            body,
            cleanup_on_error,
            ..
        } => {
            walk_block_for_opaque(body, fn_name, errors);
            walk_block_for_opaque(cleanup_on_error, fn_name, errors);
        }
        Stmt::Return { value } => {
            if let Some(v) = value {
                walk_expr_for_opaque(v, fn_name, errors);
            }
        }
        Stmt::FieldAssign { value, .. } => walk_expr_for_opaque(value, fn_name, errors),
        Stmt::SubsystemAction { args, .. } => {
            for a in args {
                walk_expr_for_opaque(a, fn_name, errors);
            }
        }
        Stmt::RegWrite { .. } | Stmt::RegModify { .. } | Stmt::ResourceOp { .. } => {}
    }
}

fn walk_expr_for_opaque(expr: &Expr, fn_name: &str, errors: &mut Vec<ValidationError>) {
    match expr {
        Expr::IntLit { ty, .. } | Expr::Cast { ty, .. } => {
            if let SemanticType::Opaque { description } = ty {
                errors.push(ValidationError {
                    function: fn_name.into(),
                    pass: ValidationPass::NoOpaqueTypes,
                    description: format!("expression contains Opaque type: {description}"),
                });
            }
        }
        Expr::BinaryOp { lhs, rhs, .. } => {
            walk_expr_for_opaque(lhs, fn_name, errors);
            walk_expr_for_opaque(rhs, fn_name, errors);
        }
        Expr::UnaryOp { operand, .. } => walk_expr_for_opaque(operand, fn_name, errors),
        Expr::HelperCall { args, .. } => {
            for a in args {
                walk_expr_for_opaque(a, fn_name, errors);
            }
        }
        Expr::WithDefault { value, default } => {
            walk_expr_for_opaque(value, fn_name, errors);
            walk_expr_for_opaque(default, fn_name, errors);
        }
        Expr::InterruptClassTest { status, .. } => {
            walk_expr_for_opaque(status, fn_name, errors);
        }
        Expr::IsSome { inner } | Expr::OptionSome { inner } | Expr::OptionUnwrap { inner } => {
            walk_expr_for_opaque(inner, fn_name, errors);
        }
        Expr::FieldExtract { raw_value, .. } => {
            walk_expr_for_opaque(raw_value, fn_name, errors);
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// Pass 2: OptionUnwrap domination
// ---------------------------------------------------------------------------

fn validate_option_unwrap_domination(body: &SemanticBody, fn_name: &str) -> Vec<ValidationError> {
    let mut errors = Vec::new();
    let proven: Vec<Expr> = Vec::new();
    walk_block_for_unwrap(&body.body, &proven, fn_name, &mut errors);
    errors
}

fn walk_block_for_unwrap(
    block: &Block,
    proven: &[Expr],
    fn_name: &str,
    errors: &mut Vec<ValidationError>,
) {
    for stmt in &block.stmts {
        walk_stmt_for_unwrap(stmt, proven, fn_name, errors);
    }
}

fn walk_stmt_for_unwrap(
    stmt: &Stmt,
    proven: &[Expr],
    fn_name: &str,
    errors: &mut Vec<ValidationError>,
) {
    match stmt {
        Stmt::Branch {
            condition,
            then_body,
            else_body,
        } => {
            check_expr_for_unwrap(condition, proven, fn_name, errors);
            // If the condition is IsSome(e), then e is proven present in then_body.
            let mut then_proven = proven.to_vec();
            if let Expr::IsSome { inner } = condition {
                then_proven.push(*inner.clone());
            }
            if let Expr::ResourcePresent { resource } = condition {
                then_proven.push(Expr::DriverFieldRead {
                    field: resource.field.clone(),
                });
            }
            walk_block_for_unwrap(then_body, &then_proven, fn_name, errors);
            walk_block_for_unwrap(else_body, proven, fn_name, errors);
        }
        Stmt::Loop { condition, body } => {
            check_expr_for_unwrap(condition, proven, fn_name, errors);
            let mut loop_proven = proven.to_vec();
            if let Expr::IsSome { inner } = condition {
                loop_proven.push(*inner.clone());
            }
            walk_block_for_unwrap(body, &loop_proven, fn_name, errors);
        }
        Stmt::Let { value, .. } => check_expr_for_unwrap(value, proven, fn_name, errors),
        Stmt::Assign { value, .. } => check_expr_for_unwrap(value, proven, fn_name, errors),
        Stmt::CompoundAssign { value, .. } => {
            check_expr_for_unwrap(value, proven, fn_name, errors);
        }
        Stmt::Eval { expr } => check_expr_for_unwrap(expr, proven, fn_name, errors),
        Stmt::FieldAssign { value, .. } => check_expr_for_unwrap(value, proven, fn_name, errors),
        Stmt::Return { value: Some(v) } => check_expr_for_unwrap(v, proven, fn_name, errors),
        Stmt::FallibleOp {
            body,
            cleanup_on_error,
            ..
        } => {
            walk_block_for_unwrap(body, proven, fn_name, errors);
            walk_block_for_unwrap(cleanup_on_error, proven, fn_name, errors);
        }
        Stmt::OrderedRegion { body } => {
            walk_block_for_unwrap(body, proven, fn_name, errors);
        }
        _ => {}
    }
}

fn check_expr_for_unwrap(
    expr: &Expr,
    proven: &[Expr],
    fn_name: &str,
    errors: &mut Vec<ValidationError>,
) {
    match expr {
        Expr::OptionUnwrap { inner } => {
            if !proven.iter().any(|p| structurally_matches(p, inner)) {
                errors.push(ValidationError {
                    function: fn_name.into(),
                    pass: ValidationPass::OptionUnwrapDomination,
                    description: "OptionUnwrap without dominating IsSome".to_string(),
                });
            }
            // Also check the inner expression recursively.
            check_expr_for_unwrap(inner, proven, fn_name, errors);
        }
        Expr::BinaryOp { lhs, rhs, .. } => {
            check_expr_for_unwrap(lhs, proven, fn_name, errors);
            check_expr_for_unwrap(rhs, proven, fn_name, errors);
        }
        Expr::UnaryOp { operand, .. } => {
            check_expr_for_unwrap(operand, proven, fn_name, errors);
        }
        Expr::Cast { expr: inner, .. } => {
            check_expr_for_unwrap(inner, proven, fn_name, errors);
        }
        Expr::HelperCall { args, .. } => {
            for a in args {
                check_expr_for_unwrap(a, proven, fn_name, errors);
            }
        }
        Expr::IsSome { inner } | Expr::OptionSome { inner } => {
            check_expr_for_unwrap(inner, proven, fn_name, errors);
        }
        Expr::WithDefault { value, default } => {
            check_expr_for_unwrap(value, proven, fn_name, errors);
            check_expr_for_unwrap(default, proven, fn_name, errors);
        }
        Expr::InterruptClassTest { status, .. } => {
            check_expr_for_unwrap(status, proven, fn_name, errors);
        }
        _ => {}
    }
}

/// Check structural equality of two expressions for domination purposes.
fn structurally_matches(proven: &Expr, unwrap_inner: &Expr) -> bool {
    match (proven, unwrap_inner) {
        (Expr::DriverFieldRead { field: a }, Expr::DriverFieldRead { field: b }) => {
            a.canonical_path == b.canonical_path
        }
        (Expr::LocalRef { local: a }, Expr::LocalRef { local: b }) => a == b,
        _ => proven == unwrap_inner,
    }
}

// ---------------------------------------------------------------------------
// Pass 3: No stringly lowering-critical refs
// ---------------------------------------------------------------------------

fn validate_no_stringly_refs(body: &SemanticBody, fn_name: &str) -> Vec<ValidationError> {
    let mut errors = Vec::new();
    walk_block_for_stringly(&body.body, fn_name, &mut errors);
    errors
}

fn walk_block_for_stringly(block: &Block, fn_name: &str, errors: &mut Vec<ValidationError>) {
    for stmt in &block.stmts {
        walk_stmt_for_stringly(stmt, fn_name, errors);
    }
}

fn walk_stmt_for_stringly(stmt: &Stmt, fn_name: &str, errors: &mut Vec<ValidationError>) {
    match stmt {
        Stmt::Eval { expr } | Stmt::Let { value: expr, .. } | Stmt::Assign { value: expr, .. } => {
            walk_expr_for_stringly(expr, fn_name, errors);
        }
        Stmt::Branch {
            condition,
            then_body,
            else_body,
        } => {
            walk_expr_for_stringly(condition, fn_name, errors);
            walk_block_for_stringly(then_body, fn_name, errors);
            walk_block_for_stringly(else_body, fn_name, errors);
        }
        Stmt::Loop { condition, body } => {
            walk_expr_for_stringly(condition, fn_name, errors);
            walk_block_for_stringly(body, fn_name, errors);
        }
        Stmt::OrderedRegion { body } => walk_block_for_stringly(body, fn_name, errors),
        Stmt::FallibleOp {
            body,
            cleanup_on_error,
            ..
        } => {
            walk_block_for_stringly(body, fn_name, errors);
            walk_block_for_stringly(cleanup_on_error, fn_name, errors);
        }
        Stmt::FieldAssign { value, .. } => walk_expr_for_stringly(value, fn_name, errors),
        Stmt::Return { value: Some(v) } => walk_expr_for_stringly(v, fn_name, errors),
        _ => {}
    }
}

fn walk_expr_for_stringly(expr: &Expr, fn_name: &str, errors: &mut Vec<ValidationError>) {
    match expr {
        Expr::HelperCall { callee, args, .. } => {
            if callee.canonical_path.as_str().is_empty() {
                errors.push(ValidationError {
                    function: fn_name.into(),
                    pass: ValidationPass::NoStringlyRefs,
                    description: format!(
                        "HelperCall callee '{}' has empty canonical_path",
                        callee.name
                    ),
                });
            }
            for a in args {
                walk_expr_for_stringly(a, fn_name, errors);
            }
        }
        Expr::DriverFieldRead { field } => {
            if field.canonical_path.as_str().is_empty() {
                errors.push(ValidationError {
                    function: fn_name.into(),
                    pass: ValidationPass::NoStringlyRefs,
                    description: format!(
                        "DriverFieldRead '{}' has empty canonical_path",
                        field.name
                    ),
                });
            }
        }
        Expr::Constant { constant } => {
            if constant.canonical_path.as_str().is_empty() {
                errors.push(ValidationError {
                    function: fn_name.into(),
                    pass: ValidationPass::NoStringlyRefs,
                    description: format!(
                        "ConstantRef '{}' has empty canonical_path",
                        constant.name
                    ),
                });
            }
        }
        Expr::BinaryOp { lhs, rhs, .. } => {
            walk_expr_for_stringly(lhs, fn_name, errors);
            walk_expr_for_stringly(rhs, fn_name, errors);
        }
        Expr::UnaryOp { operand, .. } | Expr::Cast { expr: operand, .. } => {
            walk_expr_for_stringly(operand, fn_name, errors);
        }
        Expr::IsSome { inner } | Expr::OptionSome { inner } | Expr::OptionUnwrap { inner } => {
            walk_expr_for_stringly(inner, fn_name, errors);
        }
        Expr::WithDefault { value, default } => {
            walk_expr_for_stringly(value, fn_name, errors);
            walk_expr_for_stringly(default, fn_name, errors);
        }
        Expr::InterruptClassTest { status, .. } => {
            walk_expr_for_stringly(status, fn_name, errors);
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// Pass 4: Valid C identifier local names
// ---------------------------------------------------------------------------

fn validate_valid_c_identifiers(body: &SemanticBody, fn_name: &str) -> Vec<ValidationError> {
    let mut errors = Vec::new();
    for local in &body.locals {
        if !is_valid_c_identifier(&local.name) {
            errors.push(ValidationError {
                function: fn_name.into(),
                pass: ValidationPass::ValidCIdentifiers,
                description: format!("local name '{}' is not a valid C identifier", local.name),
            });
        }
    }
    errors
}

fn is_valid_c_identifier(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    let mut chars = name.chars();
    let first = chars.next().unwrap();
    if !first.is_ascii_alphabetic() && first != '_' {
        return false;
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

// ---------------------------------------------------------------------------
// Pass 5: No raw MIR local spellings
// ---------------------------------------------------------------------------

fn validate_no_raw_mir_locals(body: &SemanticBody, fn_name: &str) -> Vec<ValidationError> {
    let mut errors = Vec::new();
    for local in &body.locals {
        if is_raw_mir_local_name(&local.name) {
            errors.push(ValidationError {
                function: fn_name.into(),
                pass: ValidationPass::NoRawMirLocals,
                description: format!("local name '{}' is a raw MIR local spelling", local.name),
            });
        }
    }
    errors
}

fn is_raw_mir_local_name(name: &str) -> bool {
    if let Some(rest) = name.strip_prefix('_') {
        !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit())
    } else {
        false
    }
}

// ---------------------------------------------------------------------------
// Pass 6: No unsupported residue
// ---------------------------------------------------------------------------

fn validate_no_unsupported_residue(body: &SemanticBody, fn_name: &str) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    // Check for Opaque types that contain "unsupported" markers.
    for local in &body.locals {
        if let SemanticType::Opaque { description } = &local.ty {
            if description.contains("unsupported") {
                errors.push(ValidationError {
                    function: fn_name.into(),
                    pass: ValidationPass::NoUnsupportedResidue,
                    description: format!(
                        "local '{}' contains unsupported residue: {description}",
                        local.name
                    ),
                });
            }
        }
    }

    errors
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use regseq_model::item::identity::CanonicalPath;
    use regseq_model::item::ItemId;
    use regseq_model::semantic::local::{LocalId, LocalOrigin, SemanticLocal};
    use regseq_model::semantic::refs::DriverFieldRef;

    fn void_body_with_locals(locals: Vec<SemanticLocal>) -> SemanticBody {
        SemanticBody {
            locals,
            params: vec![],
            return_type: SemanticType::Void,
            body: Block { stmts: vec![] },
        }
    }

    #[test]
    fn pass4_rejects_empty_name() {
        let body = void_body_with_locals(vec![SemanticLocal {
            id: LocalId(0),
            name: "".into(),
            ty: SemanticType::U32,
            origin: LocalOrigin::Derived,
        }]);
        let errors = validate_valid_c_identifiers(&body, "test");
        assert!(!errors.is_empty());
        assert_eq!(errors[0].pass, ValidationPass::ValidCIdentifiers);
    }

    #[test]
    fn pass5_rejects_raw_mir_name() {
        let body = void_body_with_locals(vec![SemanticLocal {
            id: LocalId(0),
            name: "_7".into(),
            ty: SemanticType::U32,
            origin: LocalOrigin::Derived,
        }]);
        let errors = validate_no_raw_mir_locals(&body, "test");
        assert!(!errors.is_empty());
        assert_eq!(errors[0].pass, ValidationPass::NoRawMirLocals);
    }

    #[test]
    fn pass5_accepts_authored_underscore_name() {
        let body = void_body_with_locals(vec![SemanticLocal {
            id: LocalId(0),
            name: "_config".into(),
            ty: SemanticType::U32,
            origin: LocalOrigin::Authored,
        }]);
        let errors = validate_no_raw_mir_locals(&body, "test");
        assert!(errors.is_empty());
    }

    #[test]
    fn pass1_rejects_opaque_local_type() {
        let body = void_body_with_locals(vec![SemanticLocal {
            id: LocalId(0),
            name: "mystery".into(),
            ty: SemanticType::Opaque {
                description: "unknown type".into(),
            },
            origin: LocalOrigin::Derived,
        }]);
        let errors = validate_no_opaque_types(&body, "test");
        assert!(!errors.is_empty());
        assert_eq!(errors[0].pass, ValidationPass::NoOpaqueTypes);
    }

    #[test]
    fn pass2_rejects_unguarded_option_unwrap() {
        let field = DriverFieldRef {
            owner: ItemId(0),
            canonical_path: CanonicalPath::new("test::pending_tx"),
            name: "pending_tx".into(),
        };
        let body = SemanticBody {
            locals: vec![SemanticLocal {
                id: LocalId(0),
                name: "byte".into(),
                ty: SemanticType::U8,
                origin: LocalOrigin::Authored,
            }],
            params: vec![],
            return_type: SemanticType::Void,
            body: Block {
                stmts: vec![Stmt::Let {
                    local: LocalId(0),
                    value: Expr::OptionUnwrap {
                        inner: Box::new(Expr::DriverFieldRead {
                            field: field.clone(),
                        }),
                    },
                }],
            },
        };
        let errors = validate_option_unwrap_domination(&body, "test");
        assert!(!errors.is_empty());
        assert_eq!(errors[0].pass, ValidationPass::OptionUnwrapDomination);
    }

    #[test]
    fn pass2_accepts_guarded_option_unwrap() {
        let field = DriverFieldRef {
            owner: ItemId(0),
            canonical_path: CanonicalPath::new("test::pending_tx"),
            name: "pending_tx".into(),
        };
        let body = SemanticBody {
            locals: vec![SemanticLocal {
                id: LocalId(0),
                name: "byte".into(),
                ty: SemanticType::U8,
                origin: LocalOrigin::Authored,
            }],
            params: vec![],
            return_type: SemanticType::Void,
            body: Block {
                stmts: vec![Stmt::Branch {
                    condition: Expr::IsSome {
                        inner: Box::new(Expr::DriverFieldRead {
                            field: field.clone(),
                        }),
                    },
                    then_body: Block {
                        stmts: vec![Stmt::Let {
                            local: LocalId(0),
                            value: Expr::OptionUnwrap {
                                inner: Box::new(Expr::DriverFieldRead {
                                    field: field.clone(),
                                }),
                            },
                        }],
                    },
                    else_body: Block { stmts: vec![] },
                }],
            },
        };
        let errors = validate_option_unwrap_domination(&body, "test");
        assert!(errors.is_empty(), "guarded unwrap should pass: {errors:?}");
    }
}
