use rustc_ast::LitKind;
use rustc_hir::intravisit::{self, Visitor};
use rustc_hir::{ExprKind, ImplItemKind, ItemKind};
use rustc_middle::ty::TyCtxt;

// ---------------------------------------------------------------------------
// Record types
// ---------------------------------------------------------------------------

/// Top-level extraction result for the diagnostic logging surface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoggingSurface {
    pub call_sites: Vec<LogCallSite>,
}

/// Accepted log levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
}

/// Recognized LogArg variant kind at a call site.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogArgKind {
    U8,
    U16,
    U32,
    U64,
    Usize,
    I8,
    I16,
    I32,
    I64,
    Isize,
    Bool,
    Str,
    Ptr,
}

/// A recognized logging anchor call site.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogCallSite {
    pub level: LogLevel,
    pub containing_fn: String,
    pub canonical: bool,
    pub template: String,
    pub arg_count: usize,
    pub arg_kinds: Vec<LogArgKind>,
}

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoggingError {
    UnsupportedPlaceholder(String),
    UnsupportedArgType(String),
    PlaceholderCountMismatch {
        template: String,
        placeholders: usize,
        arguments: usize,
    },
}

// ---------------------------------------------------------------------------
// Main extraction entry point
// ---------------------------------------------------------------------------

/// Extract the diagnostic logging surface from HIR after analysis.
pub fn extract_logging_surface<'tcx>(tcx: TyCtxt<'tcx>) -> Result<LoggingSurface, LoggingError> {
    let mut call_sites = Vec::new();

    for item_id in tcx.hir_free_items() {
        let item = tcx.hir_item(item_id);

        if let ItemKind::Impl(impl_block) = &item.kind {
            for impl_item_ref in impl_block.items {
                let impl_item = tcx.hir_impl_item(rustc_hir::ImplItemId {
                    owner_id: impl_item_ref.owner_id,
                });
                collect_logging_sites(tcx, impl_item, &mut call_sites)?;
            }
        }
    }

    Ok(LoggingSurface { call_sites })
}

// ---------------------------------------------------------------------------
// Call-site collection
// ---------------------------------------------------------------------------

fn collect_logging_sites<'tcx>(
    tcx: TyCtxt<'tcx>,
    impl_item: &'tcx rustc_hir::ImplItem<'tcx>,
    sites: &mut Vec<LogCallSite>,
) -> Result<(), LoggingError> {
    if let ImplItemKind::Fn(_sig, body_id) = impl_item.kind {
        let fn_name = impl_item.ident.name.as_str().to_string();
        let body = tcx.hir_body(body_id);
        let typeck_results = tcx.typeck(impl_item.owner_id.def_id);
        let mut visitor = LoggingVisitor {
            tcx,
            typeck_results,
            current_fn: fn_name,
            sites: Vec::new(),
            error: None,
        };
        visitor.visit_expr(body.value);
        if let Some(err) = visitor.error {
            return Err(err);
        }
        sites.extend(visitor.sites);
    }
    Ok(())
}

struct LoggingVisitor<'tcx> {
    tcx: TyCtxt<'tcx>,
    typeck_results: &'tcx rustc_middle::ty::TypeckResults<'tcx>,
    current_fn: String,
    sites: Vec<LogCallSite>,
    error: Option<LoggingError>,
}

impl<'tcx> Visitor<'tcx> for LoggingVisitor<'tcx> {
    fn visit_expr(&mut self, expr: &'tcx rustc_hir::Expr<'tcx>) {
        if self.error.is_some() {
            return;
        }

        if let ExprKind::Call(callee, args) = expr.kind {
            if let Some((level, canonical)) = classify_logging_path(callee) {
                match self.extract_log_call(level, canonical, args) {
                    Ok(site) => self.sites.push(site),
                    Err(err) => {
                        self.error = Some(err);
                        return;
                    }
                }
            }
        }

        intravisit::walk_expr(self, expr);
    }
}

impl<'tcx> LoggingVisitor<'tcx> {
    fn extract_log_call(
        &self,
        level: LogLevel,
        canonical: bool,
        args: &'tcx [rustc_hir::Expr<'tcx>],
    ) -> Result<LogCallSite, LoggingError> {
        // Expected shape: (template: &'static str, args: [LogArg; N])
        let template = if let Some(first) = args.first() {
            extract_string_literal(first).unwrap_or_default()
        } else {
            String::new()
        };

        // Validate template placeholders
        let placeholders = parse_placeholders(&template)?;

        // Extract argument array
        let (arg_count, arg_kinds) = if let Some(arr_expr) = args.get(1) {
            self.extract_log_args(arr_expr)?
        } else {
            (0, Vec::new())
        };

        // Validate placeholder count matches argument count
        if placeholders.len() != arg_count {
            return Err(LoggingError::PlaceholderCountMismatch {
                template: template.clone(),
                placeholders: placeholders.len(),
                arguments: arg_count,
            });
        }

        Ok(LogCallSite {
            level,
            containing_fn: self.current_fn.clone(),
            canonical,
            template,
            arg_count,
            arg_kinds,
        })
    }

    fn extract_log_args(
        &self,
        expr: &'tcx rustc_hir::Expr<'tcx>,
    ) -> Result<(usize, Vec<LogArgKind>), LoggingError> {
        let ExprKind::Array(elements) = expr.kind else {
            return Ok((0, Vec::new()));
        };

        let mut kinds = Vec::new();
        for elem in elements {
            let kind = self.classify_log_arg(elem)?;
            kinds.push(kind);
        }

        Ok((kinds.len(), kinds))
    }

    fn classify_log_arg(
        &self,
        expr: &'tcx rustc_hir::Expr<'tcx>,
    ) -> Result<LogArgKind, LoggingError> {
        // The argument shape is LogArg::from(value) -- a Call expression.
        // We classify by the type of the inner value expression.
        let inner = match expr.kind {
            ExprKind::Call(_, args) if !args.is_empty() => &args[0],
            _ => expr,
        };

        let ty = self.typeck_results.expr_ty(inner);
        let ty = ty.peel_refs();

        if ty.is_bool() {
            return Ok(LogArgKind::Bool);
        }
        if ty.is_str() {
            return Ok(LogArgKind::Str);
        }
        if ty.is_raw_ptr() {
            return Ok(LogArgKind::Ptr);
        }

        let type_name = if let Some(adt) = ty.ty_adt_def() {
            self.tcx.item_name(adt.did()).as_str().to_string()
        } else {
            format!("{ty}")
        };

        match type_name.as_str() {
            "u8" => Ok(LogArgKind::U8),
            "u16" => Ok(LogArgKind::U16),
            "u32" => Ok(LogArgKind::U32),
            "u64" => Ok(LogArgKind::U64),
            "usize" => Ok(LogArgKind::Usize),
            "i8" => Ok(LogArgKind::I8),
            "i16" => Ok(LogArgKind::I16),
            "i32" => Ok(LogArgKind::I32),
            "i64" => Ok(LogArgKind::I64),
            "isize" => Ok(LogArgKind::Isize),
            _ => Err(LoggingError::UnsupportedArgType(type_name)),
        }
    }
}

// ---------------------------------------------------------------------------
// Path classification
// ---------------------------------------------------------------------------

fn classify_logging_path(callee: &rustc_hir::Expr<'_>) -> Option<(LogLevel, bool)> {
    let ExprKind::Path(rustc_hir::QPath::Resolved(_, path)) = &callee.kind else {
        return None;
    };

    let segs: Vec<&str> = path
        .segments
        .iter()
        .map(|s| s.ident.name.as_str())
        .collect();

    // Canonical: ...::intrinsics::diagnostic_logging::{level}
    if segs.len() >= 3
        && segs[segs.len() - 3] == "intrinsics"
        && segs[segs.len() - 2] == "diagnostic_logging"
    {
        return match segs[segs.len() - 1] {
            "error" => Some((LogLevel::Error, true)),
            "warn" => Some((LogLevel::Warn, true)),
            "info" => Some((LogLevel::Info, true)),
            "debug" => Some((LogLevel::Debug, true)),
            _ => None,
        };
    }

    // Compatibility: ...::diagnostic_logging::{level} (shorter path)
    if segs.len() >= 2 && segs[segs.len() - 2] == "diagnostic_logging" {
        return match segs[segs.len() - 1] {
            "error" => Some((LogLevel::Error, false)),
            "warn" => Some((LogLevel::Warn, false)),
            "info" => Some((LogLevel::Info, false)),
            "debug" => Some((LogLevel::Debug, false)),
            _ => None,
        };
    }

    None
}

// ---------------------------------------------------------------------------
// Template parsing and validation
// ---------------------------------------------------------------------------

/// Parse and validate format placeholders in a template string.
/// Returns the list of accepted placeholder specs.
fn parse_placeholders(template: &str) -> Result<Vec<String>, LoggingError> {
    let mut placeholders = Vec::new();
    let mut chars = template.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '{' {
            if chars.peek() == Some(&'{') {
                // Escaped `{{` -- skip
                chars.next();
                continue;
            }

            let mut spec = String::new();
            let mut closed = false;
            for c in chars.by_ref() {
                if c == '}' {
                    closed = true;
                    break;
                }
                spec.push(c);
            }

            if !closed {
                return Err(LoggingError::UnsupportedPlaceholder(
                    "unclosed placeholder".to_string(),
                ));
            }

            validate_placeholder(&spec)?;
            placeholders.push(spec);
        } else if ch == '}' && chars.peek() == Some(&'}') {
            // Escaped `}}`
            chars.next();
            continue;
        }
    }

    Ok(placeholders)
}

/// Validate a single placeholder spec (the content between `{` and `}`).
fn validate_placeholder(spec: &str) -> Result<(), LoggingError> {
    // `{}` -- default display
    if spec.is_empty() {
        return Ok(());
    }

    // Must start with `:` for format specifiers
    if !spec.starts_with(':') {
        return Err(LoggingError::UnsupportedPlaceholder(format!("{{{spec}}}")));
    }

    let fmt = &spec[1..];

    // `:p` -- pointer
    if fmt == "p" {
        return Ok(());
    }

    // `:x`, `:X` -- hex
    if fmt == "x" || fmt == "X" {
        return Ok(());
    }

    // `:#x`, `:#X` -- alternate hex
    if fmt == "#x" || fmt == "#X" {
        return Ok(());
    }

    // `:0<width>x`, `:0<width>X` -- zero-padded hex
    if let Some(rest) = fmt.strip_prefix('0') {
        if let Some((width_str, suffix)) = split_width_suffix(rest) {
            if (suffix == "x" || suffix == "X") && width_str.chars().all(|c| c.is_ascii_digit()) {
                return Ok(());
            }
        }
    }

    // `:#0<width>x`, `:#0<width>X` -- alternate zero-padded hex
    if let Some(rest) = fmt.strip_prefix("#0") {
        if let Some((width_str, suffix)) = split_width_suffix(rest) {
            if (suffix == "x" || suffix == "X") && width_str.chars().all(|c| c.is_ascii_digit()) {
                return Ok(());
            }
        }
    }

    Err(LoggingError::UnsupportedPlaceholder(format!("{{{spec}}}")))
}

/// Split a format suffix into a decimal width and trailing format letter(s).
fn split_width_suffix(s: &str) -> Option<(&str, &str)> {
    // Find the last position where digits end
    let digit_end = s
        .char_indices()
        .find(|(_, c)| !c.is_ascii_digit())
        .map(|(i, _)| i)
        .unwrap_or(s.len());

    if digit_end == 0 || digit_end == s.len() {
        return None;
    }

    Some((&s[..digit_end], &s[digit_end..]))
}

// ---------------------------------------------------------------------------
// HIR helpers
// ---------------------------------------------------------------------------

fn extract_string_literal(expr: &rustc_hir::Expr<'_>) -> Option<String> {
    if let ExprKind::Lit(lit) = &expr.kind {
        if let LitKind::Str(sym, _) = lit.node {
            return Some(sym.as_str().to_string());
        }
    }
    None
}
