//! Rule 6 logging intrinsic metadata extraction for Pass 2.
//!
//! Implemented by task regseq-model-50.
//!
//! At Pass 2 setup, [`LogArgUnwrap::resolve`] caches the [`DefId`] of
//! the `regseq::intrinsics::diagnostic_logging::LogArg` enum by
//! walking the facade crate's module tree (the same pattern used by
//! [`super::ordered_region::OrderedMarkers`]). The cache is consulted
//! during [`extract_log_metadata`] to validate that array elements
//! supplied to a logging intrinsic actually originate from a
//! `LogArg::from(...)` carrier call -- specimens that pass a non-`LogArg`
//! operand into a logging call are rejected as malformed rather than
//! silently miscompiled.
//!
//! During terminator translation in [`super::pass2`], when an
//! [`IntrinsicCall`](regseq_model_old::ir::TerminatorKind::IntrinsicCall)
//! resolves to one of [`RegseqIntrinsic::LogError`],
//! [`RegseqIntrinsic::LogWarn`], [`RegseqIntrinsic::LogInfo`], or
//! [`RegseqIntrinsic::LogDebug`], [`extract_log_metadata`] runs to:
//!
//! 1. extract the template-string literal from `args[0]`,
//! 2. parse the restricted Regseq format subset into
//!    [`Vec<FormatPlaceholder>`](regseq_model_old::ir::FormatPlaceholder),
//! 3. trace `args[1]` (the `[LogArg<'_>; N]` aggregate) back through
//!    each constituent `LogArg::from(value)` Call to recover the
//!    inner `value` operand,
//! 4. assert placeholder/argument arity, and
//! 5. return the rebuilt `(IntrinsicMetadata::Log, args)` payload --
//!    `args[0]` (template) is removed and `args[1..]` are the
//!    unwrapped inner operands.
//!
//! Any structural deviation surfaces as
//! [`ConstructionError::UnsupportedLogFormat`],
//! [`ConstructionError::LogPlaceholderArityMismatch`], or
//! [`ConstructionError::UnsupportedConstShape`] -- the recognition
//! never silently skips an unrecognized log call.

use std::collections::BTreeSet;

use rustc_hir::def::DefKind;
use rustc_hir::def_id::{CrateNum, DefId, CRATE_DEF_INDEX};
use rustc_middle::mir;
use rustc_middle::ty::{self, TyCtxt};
use rustc_span::Span;

use regseq_model_old::identity::SymbolInterner;
use regseq_model_old::ir::{
    FormatKind, FormatPlaceholder, IntrinsicMetadata, Operand, RegseqIntrinsic,
};

use super::mir_mapping::{map_operand, ConstantIndex, LocalIdMap, MapOperandResult, TypeDefIndex};
use super::{ConstructionError, ErrorProvenance};

// ---------------------------------------------------------------------------
// LogArg ADT lookup
// ---------------------------------------------------------------------------

/// Cached [`DefId`] of `regseq::intrinsics::diagnostic_logging::LogArg`.
///
/// Resolved at Pass 2 setup via a live `TyCtxt` walk through the
/// `regseq` facade crate's module tree -- no path-string heuristics,
/// no hard-coded tables. Specimens that do not depend on the facade
/// (for example minimal inline test specimens that exercise only
/// non-logging intrinsics) record `None`; those specimens never reach
/// a logging intrinsic and so never consult the cache.
#[derive(Debug, Default, Copy, Clone, Eq, PartialEq)]
pub struct LogArgUnwrap {
    log_arg_adt: Option<DefId>,
}

impl LogArgUnwrap {
    /// Resolve [`Self::log_arg_adt`] by walking the `regseq` facade
    /// crate.
    ///
    /// Returns a cache with `log_arg_adt = None` when the facade is
    /// not in the dependency graph -- consistent with
    /// [`super::ordered_region::OrderedMarkers::resolve`] for
    /// specimens that do not pull in `regseq`.
    pub fn resolve(tcx: TyCtxt<'_>) -> Self {
        let Some(facade_crate) = find_crate_by_name(tcx, "regseq") else {
            return Self::default();
        };
        let root = DefId {
            krate: facade_crate,
            index: CRATE_DEF_INDEX,
        };
        let Some(intrinsics_mod) = resolve_child_module(tcx, root, "intrinsics") else {
            return Self::default();
        };
        let Some(diagnostic_logging) =
            resolve_child_module(tcx, intrinsics_mod, "diagnostic_logging")
        else {
            return Self::default();
        };
        let log_arg_adt = resolve_child_adt(tcx, diagnostic_logging, "LogArg");
        Self { log_arg_adt }
    }

    /// Build a [`LogArgUnwrap`] from an explicit `LogArg` ADT
    /// [`DefId`]. Test-only entry point: integration tests that
    /// declare a synthetic `LogArg`-like enum without depending on
    /// the `regseq` facade crate use this constructor to seed the
    /// recognition path.
    #[allow(dead_code)]
    pub fn from_def_id(log_arg_adt: DefId) -> Self {
        Self {
            log_arg_adt: Some(log_arg_adt),
        }
    }

    /// Cached `LogArg` ADT [`DefId`], if the facade crate was
    /// resolvable at setup.
    pub fn log_arg_adt(&self) -> Option<DefId> {
        self.log_arg_adt
    }
}

// ---------------------------------------------------------------------------
// LogArg-typed local pre-fold analysis
// ---------------------------------------------------------------------------

/// Body-wide pre-fold analysis that flags every MIR local whose type
/// is `LogArg<'_>` (or an array/reference thereof). Pass 2's
/// [`super::pass2::build_local_decls`] skips these locals so the
/// cross-crate `LogArg` ADT never reaches [`super::mir_mapping::map_ty`]
/// -- which would otherwise reject it as an unregistered cross-crate
/// type. The same predicate drives statement and terminator
/// rewrites: any [`mir::StatementKind::Assign`] whose destination is
/// dropped is skipped, and any [`mir::TerminatorKind::Call`] whose
/// destination is dropped is rewritten as a [`Goto`](mir::TerminatorKind::Goto)
/// to its return target. The end result is that the entire `LogArg`
/// carrier chain that the `log_*!` macros emit (per-arg
/// `LogArg::from(value)` Calls + `[LogArg<'_>; N]` Aggregate
/// statement) is elided from the Regseq IR -- the inner `value`
/// operands flow through to the IntrinsicCall via
/// [`extract_log_metadata`].
#[derive(Debug, Default, Clone)]
pub(crate) struct LogFoldAnalysis {
    dropped_locals: BTreeSet<mir::Local>,
}

impl LogFoldAnalysis {
    /// Walk every MIR local once and record those whose type contains
    /// the cached `LogArg` ADT. Returns an empty analysis when
    /// [`LogArgUnwrap::log_arg_adt`] is `None` (the facade crate is
    /// not in the dependency graph -- in that case there are no log
    /// intrinsic calls to fold).
    pub(crate) fn analyze<'tcx>(mir_body: &mir::Body<'tcx>, log_arg: &LogArgUnwrap) -> Self {
        let Some(log_arg_adt) = log_arg.log_arg_adt() else {
            return Self::default();
        };
        let mut dropped_locals: BTreeSet<mir::Local> = BTreeSet::new();
        for (local, decl) in mir_body.local_decls.iter_enumerated() {
            if ty_contains_log_arg(decl.ty, log_arg_adt) {
                dropped_locals.insert(local);
            }
        }
        Self { dropped_locals }
    }

    /// Whether `local` is dropped by the fold. Driven by
    /// [`build_local_decls`](super::pass2) and statement/terminator
    /// translation alike.
    pub(crate) fn is_dropped_local(&self, local: mir::Local) -> bool {
        self.dropped_locals.contains(&local)
    }

    /// Whether the analysis flagged any locals at all. Cheap shortcut
    /// for callers that want to skip rewrite plumbing when no log
    /// chain exists in the body.
    #[allow(dead_code)]
    pub(crate) fn is_empty(&self) -> bool {
        self.dropped_locals.is_empty()
    }
}

/// Recursive type-shape check: does `ty` directly equal the
/// `LogArg<'_>` ADT, or is it an array / reference / tuple whose
/// element transitively equals `LogArg`? Conservative for forms the
/// log macros do not emit (Adt fields, opaque types, function
/// pointers, etc.) -- those return `false` and the local is kept.
fn ty_contains_log_arg<'tcx>(ty: ty::Ty<'tcx>, log_arg_adt: DefId) -> bool {
    match ty.kind() {
        ty::TyKind::Adt(adt_def, _) => adt_def.did() == log_arg_adt,
        ty::TyKind::Array(inner, _) | ty::TyKind::Slice(inner) => {
            ty_contains_log_arg(*inner, log_arg_adt)
        }
        ty::TyKind::Ref(_, inner, _) => ty_contains_log_arg(*inner, log_arg_adt),
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// Public extraction entry point
// ---------------------------------------------------------------------------

/// Whether `intrinsic` is one of the four logging intrinsics.
pub(crate) fn is_log_intrinsic(intrinsic: RegseqIntrinsic) -> bool {
    matches!(
        intrinsic,
        RegseqIntrinsic::LogError
            | RegseqIntrinsic::LogWarn
            | RegseqIntrinsic::LogInfo
            | RegseqIntrinsic::LogDebug
    )
}

/// Outcome of [`extract_log_metadata`]: the structured metadata
/// payload to attach to the [`IntrinsicCall`] terminator and the
/// rebuilt argument list (template removed, `LogArg` carriers
/// stripped).
pub(crate) struct LogExtraction {
    pub metadata: IntrinsicMetadata,
    pub args: Vec<Operand>,
}

/// Run Rule 6 metadata extraction for a logging intrinsic call.
///
/// `mir_args` is the original MIR call's spanned-operand list (length
/// 2 for the facade contract: template + `[LogArg<'_>; N]` array).
/// `owner_def_id` and `call_span` are used purely for diagnostic
/// provenance.
#[allow(clippy::too_many_arguments)]
pub(crate) fn extract_log_metadata<'tcx>(
    tcx: TyCtxt<'tcx>,
    mir_body: &mir::Body<'tcx>,
    owner_def_id: DefId,
    mir_args: &[rustc_span::source_map::Spanned<mir::Operand<'tcx>>],
    locals: &LocalIdMap<'_, 'tcx>,
    type_def_index: &TypeDefIndex<'_>,
    constants: &ConstantIndex<'_>,
    log_arg: &LogArgUnwrap,
    call_span: Span,
    interner: &mut SymbolInterner,
) -> Result<LogExtraction, ConstructionError> {
    if mir_args.len() != 2 {
        return Err(ConstructionError::UnsupportedConstShape {
            provenance: ErrorProvenance {
                item_path: tcx.def_path_str(owner_def_id),
                span: call_span,
            },
            rendered: format!(
                "logging intrinsic expects exactly 2 args (template, [LogArg; N]), got {}",
                mir_args.len(),
            ),
        });
    }

    let provenance = ErrorProvenance {
        item_path: tcx.def_path_str(owner_def_id),
        span: call_span,
    };

    let template = extract_template_string(tcx, &mir_args[0].node, &provenance)?;
    let placeholders = parse_template(&template, &provenance)?;

    let inner_mir_operands = unwrap_log_args(
        tcx,
        mir_body,
        owner_def_id,
        &mir_args[1].node,
        log_arg,
        call_span,
    )?;

    if placeholders.len() != inner_mir_operands.len() {
        return Err(ConstructionError::LogPlaceholderArityMismatch {
            provenance,
            template,
            placeholders: placeholders.len(),
            args: inner_mir_operands.len(),
        });
    }

    let mut mapped_args: Vec<Operand> = Vec::with_capacity(inner_mir_operands.len());
    for op in inner_mir_operands {
        match map_operand(tcx, &op, locals, constants, type_def_index, interner)? {
            MapOperandResult::Mapped(o) => mapped_args.push(o),
            MapOperandResult::Drop => {
                return Err(ConstructionError::UnsupportedConstShape {
                    provenance: ErrorProvenance::for_span(call_span),
                    rendered: "dropped operand in logging argument position".to_string(),
                });
            }
        }
    }

    Ok(LogExtraction {
        metadata: IntrinsicMetadata::Log {
            template,
            placeholders,
        },
        args: mapped_args,
    })
}

// ---------------------------------------------------------------------------
// Template extraction
// ---------------------------------------------------------------------------

/// Extract the UTF-8 contents of a `&'static str` constant operand.
///
/// Accepts every [`mir::Const`] form a `&str` literal can take at the
/// `mir_drops_elaborated_and_const_checked` stage:
///
/// - [`mir::Const::Val`] -- the value is already a
///   [`mir::ConstValue::Slice`] / [`mir::ConstValue::Indirect`], read
///   directly via [`mir::ConstValue::try_get_slice_bytes_for_diagnostics`].
/// - [`mir::Const::Unevaluated`] -- evaluated through
///   [`mir::Const::eval`] in the fully-monomorphized typing
///   environment so the resulting [`mir::ConstValue`] yields the
///   slice bytes the same way.
/// - [`mir::Const::Ty`] -- evaluated through [`mir::Const::eval`]
///   first; the resulting `ConstValue` is then read.
///
/// The check rejects any constant whose type is not `&str`, and any
/// fully-evaluated constant that fails the slice-bytes read.
fn extract_template_string<'tcx>(
    tcx: TyCtxt<'tcx>,
    operand: &mir::Operand<'tcx>,
    provenance: &ErrorProvenance,
) -> Result<String, ConstructionError> {
    let mir::Operand::Constant(const_op) = operand else {
        return Err(ConstructionError::UnsupportedConstShape {
            provenance: provenance.clone(),
            rendered: "logging template arg is not a constant".to_string(),
        });
    };
    let ty = const_op.ty();
    if !is_static_str_ref(ty) {
        return Err(ConstructionError::UnsupportedConstShape {
            provenance: provenance.clone(),
            rendered: format!("logging template arg is not `&str`: {ty}"),
        });
    }
    let typing_env = ty::TypingEnv::fully_monomorphized();
    let value = const_op
        .const_
        .eval(tcx, typing_env, const_op.span)
        .map_err(|err| ConstructionError::UnsupportedConstShape {
            provenance: provenance.clone(),
            rendered: format!("logging template `&str` const-eval failed: {err:?}"),
        })?;
    let bytes = value
        .try_get_slice_bytes_for_diagnostics(tcx)
        .ok_or_else(|| ConstructionError::UnsupportedConstShape {
            provenance: provenance.clone(),
            rendered: "logging template `&str` constant has no readable bytes".to_string(),
        })?;
    let s = std::str::from_utf8(bytes).map_err(|err| ConstructionError::UnsupportedConstShape {
        provenance: provenance.clone(),
        rendered: format!("logging template is not valid UTF-8: {err}"),
    })?;
    Ok(s.to_string())
}

fn is_static_str_ref(ty: ty::Ty<'_>) -> bool {
    let ty::TyKind::Ref(_, inner, _) = ty.kind() else {
        return false;
    };
    matches!(inner.kind(), ty::TyKind::Str)
}

// ---------------------------------------------------------------------------
// Template parser
// ---------------------------------------------------------------------------

/// Parse a Regseq-restricted format template into a positional
/// [`Vec<FormatPlaceholder>`].
///
/// The grammar accepted here is intentionally narrower than
/// `core::fmt`. Each placeholder must match one of:
///
/// - `{}`            -> [`FormatKind::Display`]
/// - `{:x}`          -> [`FormatKind::LowerHex`]
/// - `{:X}`          -> [`FormatKind::UpperHex`]
/// - `{:#x}`         -> [`FormatKind::LowerHexPrefixed`]
/// - `{:#X}`         -> [`FormatKind::UpperHexPrefixed`]
/// - `{:[#]0<W>x}`   -> [`FormatKind::PaddedHex`] (lowercase; `#` optional)
/// - `{:[#]0<W>X}`   -> [`FormatKind::PaddedHex`] (uppercase; `#` optional)
/// - `{:p}`          -> [`FormatKind::Pointer`]
///
/// `{{` and `}}` literal escapes are skipped over (no placeholder
/// emitted). Any other shape -- named arguments, positional indices,
/// `{:?}` debug, fill/align, sign flags, precision, dynamic width --
/// raises [`ConstructionError::UnsupportedLogFormat`].
fn parse_template(
    template: &str,
    provenance: &ErrorProvenance,
) -> Result<Vec<FormatPlaceholder>, ConstructionError> {
    let bytes = template.as_bytes();
    let mut out: Vec<FormatPlaceholder> = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'{' {
            // `{{` literal escape; skip the second brace.
            if bytes.get(i + 1) == Some(&b'{') {
                i += 2;
                continue;
            }
            let close =
                template[i..]
                    .find('}')
                    .ok_or_else(|| ConstructionError::UnsupportedLogFormat {
                        provenance: provenance.clone(),
                        template: template.to_string(),
                        placeholder: template[i..].to_string(),
                    })?;
            let end = i + close;
            let raw = &template[i..=end];
            let kind = parse_placeholder_body(&template[i + 1..end]).ok_or_else(|| {
                ConstructionError::UnsupportedLogFormat {
                    provenance: provenance.clone(),
                    template: template.to_string(),
                    placeholder: raw.to_string(),
                }
            })?;
            out.push(FormatPlaceholder { format: kind });
            i = end + 1;
            continue;
        }
        if b == b'}' {
            // `}}` literal escape; otherwise an unmatched closing
            // brace is a malformed template.
            if bytes.get(i + 1) == Some(&b'}') {
                i += 2;
                continue;
            }
            return Err(ConstructionError::UnsupportedLogFormat {
                provenance: provenance.clone(),
                template: template.to_string(),
                placeholder: "}".to_string(),
            });
        }
        i += 1;
    }
    Ok(out)
}

/// Parse the body of a single placeholder (the text between `{` and
/// `}`, exclusive). Returns `None` for any specifier outside the
/// restricted Regseq subset -- including the `{:}` shape, which has
/// the colon's "format spec follows" sigil with no actual spec and
/// is not a member of the supported set documented in
/// `ir-reference.md` Section 5.
fn parse_placeholder_body(body: &str) -> Option<FormatKind> {
    if body.is_empty() {
        return Some(FormatKind::Display);
    }
    let spec = body.strip_prefix(':')?;
    match spec {
        "x" => return Some(FormatKind::LowerHex),
        "X" => return Some(FormatKind::UpperHex),
        "#x" => return Some(FormatKind::LowerHexPrefixed),
        "#X" => return Some(FormatKind::UpperHexPrefixed),
        "p" => return Some(FormatKind::Pointer),
        _ => {}
    }
    parse_padded_hex(spec)
}

/// Parse the `[#]0<W>{x|X}` padded-hex shape into
/// [`FormatKind::PaddedHex`].
fn parse_padded_hex(spec: &str) -> Option<FormatKind> {
    let mut rest = spec;
    let prefix = if let Some(stripped) = rest.strip_prefix('#') {
        rest = stripped;
        true
    } else {
        false
    };
    let rest = rest.strip_prefix('0')?;
    let case_byte = *rest.as_bytes().last()?;
    let uppercase = match case_byte {
        b'x' => false,
        b'X' => true,
        _ => return None,
    };
    let width_str = &rest[..rest.len() - 1];
    if width_str.is_empty() {
        return None;
    }
    let width: u8 = width_str.parse().ok()?;
    Some(FormatKind::PaddedHex {
        prefix,
        uppercase,
        width,
    })
}

// ---------------------------------------------------------------------------
// LogArg unwrapping
// ---------------------------------------------------------------------------

/// Trace `array_op` (the `[LogArg<'_>; N]` operand) back to its
/// constituent `LogArg::from(value)` Call sites and return the inner
/// `value` operands in array order.
fn unwrap_log_args<'tcx>(
    tcx: TyCtxt<'tcx>,
    body: &mir::Body<'tcx>,
    owner_def_id: DefId,
    array_op: &mir::Operand<'tcx>,
    log_arg: &LogArgUnwrap,
    call_span: Span,
) -> Result<Vec<mir::Operand<'tcx>>, ConstructionError> {
    let array_local =
        operand_as_local(array_op).ok_or_else(|| ConstructionError::UnsupportedConstShape {
            provenance: ErrorProvenance {
                item_path: tcx.def_path_str(owner_def_id),
                span: call_span,
            },
            rendered: format!(
                "logging args operand is not a Place-backed local: {:?}",
                array_op,
            ),
        })?;
    let elements = find_array_elements(body, array_local).ok_or_else(|| {
        ConstructionError::UnsupportedConstShape {
            provenance: ErrorProvenance {
                item_path: tcx.def_path_str(owner_def_id),
                span: call_span,
            },
            rendered: format!(
                "logging args local _{} is not assigned via an Aggregate(Array, ..)",
                array_local.as_u32(),
            ),
        }
    })?;

    let mut out: Vec<mir::Operand<'tcx>> = Vec::with_capacity(elements.len());
    for element in elements {
        let inner = unwrap_log_arg_element(tcx, body, owner_def_id, &element, log_arg, call_span)?;
        out.push(inner);
    }
    Ok(out)
}

/// Walk a single array element back through trivial `_t = move _u`
/// renames until the producing `LogArg::from(value)` Call is reached
/// and return that Call's `value` argument.
fn unwrap_log_arg_element<'tcx>(
    tcx: TyCtxt<'tcx>,
    body: &mir::Body<'tcx>,
    owner_def_id: DefId,
    element: &mir::Operand<'tcx>,
    log_arg: &LogArgUnwrap,
    call_span: Span,
) -> Result<mir::Operand<'tcx>, ConstructionError> {
    let mut current_local = match operand_as_local(element) {
        Some(l) => l,
        // Constant operands are not produced by `LogArg::from(...)`
        // and therefore cannot be unwrapped. The macro contract
        // always materializes each argument through the carrier.
        None => {
            return Err(ConstructionError::UnsupportedConstShape {
                provenance: ErrorProvenance {
                    item_path: tcx.def_path_str(owner_def_id),
                    span: call_span,
                },
                rendered: format!(
                    "logging arg element is not a Place-backed local: {:?}",
                    element,
                ),
            });
        }
    };

    // Bound the rename walk so a malformed specimen with a self-referential
    // assignment cannot loop forever. In practice the chain is at most a
    // handful of moves (rustc's Rule-2-friendly mir_opt_level=0 layout).
    let max_renames = body.local_decls.len() + 16;
    for _ in 0..max_renames {
        match find_unique_definition(tcx, body, current_local) {
            Some(LocalDefinition::Statement(rvalue)) => {
                if let Some(next) = rvalue_as_local_rename(rvalue) {
                    current_local = next;
                    continue;
                }
                return Err(ConstructionError::UnsupportedConstShape {
                    provenance: ErrorProvenance {
                        item_path: tcx.def_path_str(owner_def_id),
                        span: call_span,
                    },
                    rendered: format!(
                        "logging arg producer for _{} is a non-rename rvalue: {:?}",
                        current_local.as_u32(),
                        rvalue,
                    ),
                });
            }
            Some(LocalDefinition::Call { args, return_ty }) => {
                if !is_log_arg_return_ty(tcx, return_ty, log_arg) {
                    return Err(ConstructionError::UnsupportedConstShape {
                        provenance: ErrorProvenance {
                            item_path: tcx.def_path_str(owner_def_id),
                            span: call_span,
                        },
                        rendered: format!(
                            "logging arg producer Call returns `{return_ty}`, not `LogArg`",
                        ),
                    });
                }
                if args.len() != 1 {
                    return Err(ConstructionError::UnsupportedConstShape {
                        provenance: ErrorProvenance {
                            item_path: tcx.def_path_str(owner_def_id),
                            span: call_span,
                        },
                        rendered: format!(
                            "LogArg constructor Call has {} args, expected 1",
                            args.len(),
                        ),
                    });
                }
                return Ok(args[0].node.clone());
            }
            None => {
                return Err(ConstructionError::UnsupportedConstShape {
                    provenance: ErrorProvenance {
                        item_path: tcx.def_path_str(owner_def_id),
                        span: call_span,
                    },
                    rendered: format!(
                        "logging arg local _{} has no unique definition",
                        current_local.as_u32(),
                    ),
                });
            }
        }
    }
    Err(ConstructionError::UnsupportedConstShape {
        provenance: ErrorProvenance {
            item_path: tcx.def_path_str(owner_def_id),
            span: call_span,
        },
        rendered: format!(
            "logging arg rename chain exceeded depth bound starting at _{}",
            current_local.as_u32(),
        ),
    })
}

/// A single MIR-level definition site for a local: an assigning
/// statement (carrying its rvalue) or a call terminator (carrying
/// its argument list and return type).
enum LocalDefinition<'a, 'tcx> {
    Statement(&'a mir::Rvalue<'tcx>),
    Call {
        args: &'a [rustc_span::source_map::Spanned<mir::Operand<'tcx>>],
        return_ty: ty::Ty<'tcx>,
    },
}

/// Find the unique definition (statement or call terminator) of
/// `local`. Returns [`None`] when there is zero or more than one
/// definition in the body -- both situations mean the rename chain
/// cannot be safely traced.
fn find_unique_definition<'a, 'tcx>(
    tcx: TyCtxt<'tcx>,
    body: &'a mir::Body<'tcx>,
    local: mir::Local,
) -> Option<LocalDefinition<'a, 'tcx>> {
    let mut found: Option<LocalDefinition<'a, 'tcx>> = None;
    for (_bb, data) in body.basic_blocks.iter_enumerated() {
        for stmt in &data.statements {
            if let mir::StatementKind::Assign(boxed) = &stmt.kind {
                let (place, rvalue) = boxed.as_ref();
                if place.local == local && place.projection.is_empty() {
                    if found.is_some() {
                        return None;
                    }
                    found = Some(LocalDefinition::Statement(rvalue));
                }
            }
        }
        if let Some(term) = &data.terminator {
            if let mir::TerminatorKind::Call {
                destination,
                func,
                args,
                ..
            } = &term.kind
            {
                if destination.local == local && destination.projection.is_empty() {
                    if found.is_some() {
                        return None;
                    }
                    let return_ty = call_return_ty(tcx, func)?;
                    found = Some(LocalDefinition::Call {
                        args: &args[..],
                        return_ty,
                    });
                }
            }
        }
    }
    found
}

/// Resolve a Call's return type from its `func` operand. Returns
/// [`None`] when `func` is not a direct `FnDef` constant -- indirect
/// pointer calls have no statically-known return type and are not
/// expected in the `LogArg::from` chain.
fn call_return_ty<'tcx>(tcx: TyCtxt<'tcx>, func: &mir::Operand<'tcx>) -> Option<ty::Ty<'tcx>> {
    let mir::Operand::Constant(c) = func else {
        return None;
    };
    let ty = match c.const_ {
        mir::Const::Val(_, ty) => ty,
        mir::Const::Ty(ty, _) => ty,
        mir::Const::Unevaluated(_, ty) => ty,
    };
    let ty::TyKind::FnDef(def_id, args) = ty.kind() else {
        return None;
    };
    // The MIR body is monomorphized, so the FnDef carries concrete
    // generic args; instantiate the polymorphic signature against
    // them to obtain the concrete return type.
    let sig = tcx
        .fn_sig(*def_id)
        .instantiate(tcx, args)
        .output()
        .skip_binder();
    Some(sig)
}

/// Whether the given rvalue is a trivial `(Use|CopyForDeref)` of a
/// bare local Place, i.e. an SSA-style rename. Such rvalues can be
/// transparently traversed by the `LogArg` unwrapper.
fn rvalue_as_local_rename<'tcx>(rvalue: &mir::Rvalue<'tcx>) -> Option<mir::Local> {
    match rvalue {
        mir::Rvalue::Use(op) => operand_as_local(op),
        mir::Rvalue::CopyForDeref(place) if place.projection.is_empty() => Some(place.local),
        _ => None,
    }
}

/// Return the bare local backing `op`, when `op` is an
/// [`mir::Operand::Copy`] or [`mir::Operand::Move`] of a place with
/// no projections. Returns [`None`] for constants and projected
/// places.
fn operand_as_local<'tcx>(op: &mir::Operand<'tcx>) -> Option<mir::Local> {
    let place = match op {
        mir::Operand::Copy(p) | mir::Operand::Move(p) => p,
        mir::Operand::Constant(_) | mir::Operand::RuntimeChecks(_) => return None,
    };
    if place.projection.is_empty() {
        Some(place.local)
    } else {
        None
    }
}

/// Walk the MIR body for an assignment of the form
/// `_array_local = [op0, op1, ...]` and return the array's element
/// operands. Returns [`None`] when there is no Aggregate(Array)
/// assignment or when there is more than one (a structurally
/// ambiguous specimen).
fn find_array_elements<'tcx>(
    body: &mir::Body<'tcx>,
    array_local: mir::Local,
) -> Option<Vec<mir::Operand<'tcx>>> {
    let mut found: Option<Vec<mir::Operand<'tcx>>> = None;
    for (_bb, data) in body.basic_blocks.iter_enumerated() {
        for stmt in &data.statements {
            let mir::StatementKind::Assign(boxed) = &stmt.kind else {
                continue;
            };
            let (place, rvalue) = boxed.as_ref();
            if place.local != array_local || !place.projection.is_empty() {
                continue;
            }
            let mir::Rvalue::Aggregate(kind, ops) = rvalue else {
                continue;
            };
            if !matches!(kind.as_ref(), mir::AggregateKind::Array(_)) {
                continue;
            }
            if found.is_some() {
                return None;
            }
            found = Some(ops.iter().cloned().collect());
        }
    }
    found
}

/// Whether `ty` is a `LogArg<'_>` (i.e. the cached facade ADT).
/// Returns `false` when the cache is empty -- callers translate that
/// as "this Call does not produce a LogArg" and emit the appropriate
/// error.
fn is_log_arg_return_ty<'tcx>(
    _tcx: TyCtxt<'tcx>,
    ty: ty::Ty<'tcx>,
    log_arg: &LogArgUnwrap,
) -> bool {
    let Some(adt_did) = log_arg.log_arg_adt() else {
        return false;
    };
    let ty::TyKind::Adt(adt_def, _) = ty.kind() else {
        return false;
    };
    adt_def.did() == adt_did
}

// ---------------------------------------------------------------------------
// Facade crate walking helpers
// ---------------------------------------------------------------------------

fn find_crate_by_name(tcx: TyCtxt<'_>, name: &str) -> Option<CrateNum> {
    tcx.crates(())
        .iter()
        .copied()
        .find(|&krate| tcx.crate_name(krate).as_str() == name)
}

fn resolve_child_module(tcx: TyCtxt<'_>, parent: DefId, name: &str) -> Option<DefId> {
    for child in tcx.module_children(parent) {
        let Some(def_id) = child.res.opt_def_id() else {
            continue;
        };
        if !matches!(tcx.def_kind(def_id), DefKind::Mod) {
            continue;
        }
        if child.ident.name.as_str() == name {
            return Some(def_id);
        }
    }
    None
}

fn resolve_child_adt(tcx: TyCtxt<'_>, parent: DefId, name: &str) -> Option<DefId> {
    for child in tcx.module_children(parent) {
        let Some(def_id) = child.res.opt_def_id() else {
            continue;
        };
        if !matches!(
            tcx.def_kind(def_id),
            DefKind::Enum | DefKind::Struct | DefKind::Union
        ) {
            continue;
        }
        if child.ident.name.as_str() == name {
            return Some(def_id);
        }
    }
    None
}

// All tests in this module exercise post-HIR/MIR output (constructed model,
// JSON serialization, snapshots). Voided pending MonoRust + RegseqModel
// redesign. See docs/discussions/compiler-refactoring/test-audit.md.
#[cfg(all(test, feature = "post-monorust-tests"))]
mod tests {
    use super::*;

    fn dummy_provenance() -> ErrorProvenance {
        ErrorProvenance::for_span(rustc_span::DUMMY_SP)
    }

    #[test]
    fn parse_template_empty() {
        let out = parse_template("no placeholders here", &dummy_provenance()).unwrap();
        assert!(out.is_empty());
    }

    #[test]
    fn parse_template_display() {
        let out = parse_template("baud rate: {}", &dummy_provenance()).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].format, FormatKind::Display);
    }

    #[test]
    fn parse_template_all_format_kinds() {
        let template = "{} {:x} {:X} {:#x} {:#X} {:08x} {:#010X} {:p}";
        let out = parse_template(template, &dummy_provenance()).unwrap();
        assert_eq!(
            out.iter().map(|p| p.format).collect::<Vec<_>>(),
            vec![
                FormatKind::Display,
                FormatKind::LowerHex,
                FormatKind::UpperHex,
                FormatKind::LowerHexPrefixed,
                FormatKind::UpperHexPrefixed,
                FormatKind::PaddedHex {
                    prefix: false,
                    uppercase: false,
                    width: 8,
                },
                FormatKind::PaddedHex {
                    prefix: true,
                    uppercase: true,
                    width: 10,
                },
                FormatKind::Pointer,
            ],
        );
    }

    #[test]
    fn parse_template_rejects_debug() {
        let err = parse_template("hello {:?}", &dummy_provenance()).unwrap_err();
        match err {
            ConstructionError::UnsupportedLogFormat { placeholder, .. } => {
                assert_eq!(placeholder, "{:?}");
            }
            other => panic!("expected UnsupportedLogFormat, got {other:?}"),
        }
    }

    #[test]
    fn parse_template_rejects_named() {
        let err = parse_template("hello {name}", &dummy_provenance()).unwrap_err();
        assert!(matches!(
            err,
            ConstructionError::UnsupportedLogFormat { .. }
        ));
    }

    #[test]
    fn parse_template_handles_brace_escapes() {
        let out = parse_template("{{not a placeholder}} but {} is", &dummy_provenance()).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].format, FormatKind::Display);
    }

    #[test]
    fn parse_template_rejects_unmatched_close_brace() {
        let err = parse_template("oops }", &dummy_provenance()).unwrap_err();
        assert!(matches!(
            err,
            ConstructionError::UnsupportedLogFormat { .. }
        ));
    }

    #[test]
    fn parse_template_rejects_empty_format_spec() {
        let err = parse_template("oops {:}", &dummy_provenance()).unwrap_err();
        match err {
            ConstructionError::UnsupportedLogFormat { placeholder, .. } => {
                assert_eq!(placeholder, "{:}");
            }
            other => panic!("expected UnsupportedLogFormat, got {other:?}"),
        }
    }
}
