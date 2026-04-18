//! Two-pass rustc-MIR-to-Regseq-IR construction.
//!
//! This module implements Pass 1 (declaration skeleton) of the
//! RegseqModel construction algorithm described in
//! `docs/discussions/models/regseq-model/ir-reference.md` Section 8.
//!
//! Construction is invoked in-process with extraction, inside the same
//! rustc `after_analysis` hook. It consumes a live [`TyCtxt`] plus the
//! in-memory attribute registries produced by Phase 4 extraction and
//! returns a [`RegseqModel`] with every declaration-level identifier
//! assigned. Pass 2 (function body translation) is not in scope here.

use std::collections::BTreeMap;

use rustc_hir::def_id::DefId;
use rustc_middle::ty::TyCtxt;
use rustc_span::Span;

use regseq_model::identity::{
    DeclId, ModuleId, SourceLocation, SourceOrigin, Symbol, SymbolInterner,
};
use regseq_model::model::{Module, ModuleKind, RegseqModel};

use crate::extract::abstraction_types::AbstractionTypeRegistry;
use crate::extract::framework_types::FrameworkTypeRegistry;
use crate::extract::intrinsic_registry::IntrinsicRegistry;

pub mod attrs;
pub mod driver;
pub mod id_allocator;
pub mod impls;
pub mod pin_state_registry;
pub mod registers;
pub mod walk;

pub(crate) mod cross_crate;
pub(crate) mod deref_collapse;
pub(crate) mod intrinsics;
pub(crate) mod log_metadata;
pub(crate) mod mir_mapping;
pub(crate) mod module_deps;
pub(crate) mod ordered_region;
pub(crate) mod pass2;
pub(crate) mod pinctrl_metadata;
pub(crate) mod register_metadata;
pub(crate) mod switch_and_loops;
pub(crate) mod try_folding;

// Re-export the Pass 2 mapping helpers so integration tests and Pass
// 2 task code can reach them without the sub-module itself being part
// of the public `construction::*` surface.
pub use mir_mapping::{
    map_const, map_operand, map_place, map_rvalue, map_ty, ConstantIndex, FunctionIndex,
    LocalIdMap, MapOperandResult, TypeDefIndex,
};
pub use pass2::build_body;

use id_allocator::DeclIdAllocator;

pub use pin_state_registry::PinStateRegistry;

// ---------------------------------------------------------------------------
// Extraction registry bundle
// ---------------------------------------------------------------------------

/// Borrow-only bundle of the Phase 4 attribute registries consumed by
/// Pass 1 construction.
pub struct ExtractionRegistries<'a> {
    /// `#[rseq::intrinsic("canonical_name")]` map.
    pub intrinsic: &'a IntrinsicRegistry,
    /// `#[rseq::framework_type("canonical_name")]` map.
    pub framework_types: &'a FrameworkTypeRegistry,
    /// `#[rseq::abstraction("canonical_name")]` map.
    pub abstraction_types: &'a AbstractionTypeRegistry,
    /// `#[rseq::pin_state(id = "...")]` table.
    pub pin_states: &'a PinStateRegistry,
}

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// A span paired with a human-readable item path, used in error payloads
/// so diagnostics can point back to a specific source location even after
/// the `TyCtxt` is gone.
#[derive(Clone, Debug)]
pub struct ErrorProvenance {
    /// Best-effort item path rendered via `tcx.def_path_str` when
    /// available. Empty string for errors without a specific item.
    pub item_path: String,
    /// The rustc span, preserved so diagnostics can resolve it through
    /// the session source map if the caller still has access to one.
    pub span: Span,
}

impl ErrorProvenance {
    /// Build provenance from a [`DefId`] and its containing rustc
    /// context.
    pub fn for_def_id(tcx: TyCtxt<'_>, def_id: DefId) -> Self {
        Self {
            item_path: tcx.def_path_str(def_id),
            span: tcx.def_span(def_id),
        }
    }

    /// Provenance carrying only a span (no item path).
    pub fn for_span(span: Span) -> Self {
        Self {
            item_path: String::new(),
            span,
        }
    }
}

/// Construction-time errors. Each variant carries [`ErrorProvenance`] so
/// downstream diagnostic code can render a useful message.
#[derive(Clone, Debug)]
pub enum ConstructionError {
    /// No `#[rseq::driver]`-annotated struct was found in the crate.
    NoDriverStruct {
        /// Provenance anchored at the crate root so diagnostics can
        /// point to where the driver declaration was expected.
        crate_root: ErrorProvenance,
    },
    /// More than one `#[rseq::driver]` struct was found.
    MultipleDriverStructs {
        /// Provenance for each discovered driver struct.
        drivers: Vec<ErrorProvenance>,
    },
    /// Two items in the same module share a name.
    DuplicateItemName {
        /// The module that contains the duplicates.
        module: ErrorProvenance,
        /// The clashing name.
        name: String,
    },
    /// A user-data field must have type `Option<T>`.
    InvalidUserDataShape {
        /// Provenance of the offending struct field.
        field: ErrorProvenance,
        /// Rendered type that failed the `Option<T>` check.
        rendered_type: String,
    },
    /// A user-data field's `T` in `Option<T>` is not `Copy`.
    UserDataNotCopy {
        /// Provenance of the offending struct field.
        field: ErrorProvenance,
        /// Rendered inner type `T`.
        inner_type: String,
    },
    /// A resource field lacks its required `id = "..."` attribute.
    MissingResourceId {
        /// Provenance of the offending struct field.
        field: ErrorProvenance,
        /// Resource attribute kind (`"clock"`, `"reset"`, etc.).
        kind: &'static str,
    },
    /// An `#[rseq::<...>]` attribute payload could not be parsed.
    ///
    /// Reserved for a stricter validation pass: Pass 1 is intentionally
    /// permissive about unknown `#[rseq::...]` attribute names so the DSL
    /// can add new attributes without breaking existing drivers. A later
    /// verifier may elect to raise this variant once the attribute set is
    /// frozen.
    UnknownRseqAttribute {
        /// Provenance for the offending attribute.
        provenance: ErrorProvenance,
        /// Rendered attribute name.
        attr: String,
    },
    /// A registry entry maps a DefId to a string that does not
    /// correspond to a known canonical name.
    RegistryLookupMismatch {
        /// Provenance of the mis-mapped item.
        provenance: ErrorProvenance,
        /// The canonical name string that failed the lookup.
        canonical: String,
    },
    /// An ADT encountered during MIR-to-IR type mapping resolves to a
    /// `std::*` path. Driver specs must not depend on `std`.
    StdTypeNotPermitted {
        /// Provenance of the offending type reference.
        provenance: ErrorProvenance,
        /// Rendered `std::*` path for diagnostics.
        path: String,
    },
    /// A rustc `ty::TyKind` encountered during MIR-to-IR type mapping
    /// has no `RegseqType` representation (closures, coroutines, raw
    /// pointers, function pointers, trait objects, etc.).
    UnsupportedRustType {
        /// Provenance of the offending type reference.
        provenance: ErrorProvenance,
        /// Rendered rustc type for diagnostics.
        rendered: String,
    },
    /// A MIR constant operand cannot be evaluated to a scalar or named
    /// constant reference (e.g., aggregate constants, slice constants).
    UnsupportedConstShape {
        /// Provenance of the offending constant operand.
        provenance: ErrorProvenance,
        /// Short description of the unsupported constant shape.
        rendered: String,
    },
    /// A MIR place projection has no Regseq IR representation
    /// (`OpaqueCast`, `UnwrapUnsafeBinder`).
    UnsupportedProjection {
        /// Provenance of the offending projection.
        provenance: ErrorProvenance,
        /// Rendered projection kind for diagnostics.
        rendered: String,
    },
    /// A MIR `Rvalue` has no Regseq IR representation
    /// (`ThreadLocalRef`, `RawPtr`, `ShallowInitBox`, `WrapUnsafeBinder`,
    /// and similar pointer-centric constructs).
    UnsupportedRvalue {
        /// Provenance of the offending rvalue.
        provenance: ErrorProvenance,
        /// Rendered rvalue kind for diagnostics.
        rendered: String,
    },
    /// A MIR terminator has no Regseq IR representation in Pass 2's
    /// basic-terminator subset (task 43). Covers `TailCall`, `Yield`,
    /// `CoroutineDrop`, `InlineAsm`, `FalseEdge`, `FalseUnwind`,
    /// `Assert`, `UnwindResume`, `UnwindTerminate`, and -- for this
    /// task -- `SwitchInt` (handled by task 44).
    UnsupportedTerminator {
        /// Provenance of the offending terminator.
        provenance: ErrorProvenance,
        /// Rendered terminator kind for diagnostics.
        rendered: String,
    },
    /// A MIR `Call` terminator's `func` operand does not resolve to a
    /// known [`FunctionId`]: the operand is not an `FnDef` constant, or
    /// its [`DefId`] is missing from the Pass 1 `FunctionIndex`.
    UnresolvedCallTarget {
        /// Provenance of the offending call site.
        provenance: ErrorProvenance,
        /// Rendered callee for diagnostics.
        rendered: String,
    },
}

impl std::fmt::Display for ConstructionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoDriverStruct { .. } => write!(f, "no #[rseq::driver] struct found in crate"),
            Self::MultipleDriverStructs { drivers } => {
                write!(
                    f,
                    "multiple #[rseq::driver] structs found ({} total)",
                    drivers.len(),
                )
            }
            Self::DuplicateItemName { module, name } => write!(
                f,
                "duplicate item name `{}` in module `{}`",
                name, module.item_path
            ),
            Self::InvalidUserDataShape {
                field,
                rendered_type,
            } => write!(
                f,
                "user-data field `{}` must be Option<T>, got `{}`",
                field.item_path, rendered_type,
            ),
            Self::UserDataNotCopy { field, inner_type } => write!(
                f,
                "user-data field `{}`: inner type `{}` is not Copy",
                field.item_path, inner_type,
            ),
            Self::MissingResourceId { field, kind } => write!(
                f,
                "resource field `{}` is missing required id on #[rseq::{kind}]",
                field.item_path,
            ),
            Self::UnknownRseqAttribute { provenance, attr } => write!(
                f,
                "malformed or unknown attribute `#[rseq::{}]` on `{}`",
                attr, provenance.item_path,
            ),
            Self::RegistryLookupMismatch {
                provenance,
                canonical,
            } => write!(
                f,
                "registry-mapped canonical name `{}` is not recognized (item `{}`)",
                canonical, provenance.item_path,
            ),
            Self::StdTypeNotPermitted { provenance, path } => write!(
                f,
                "driver specs must not use `std`: `{}` at `{}`",
                path, provenance.item_path,
            ),
            Self::UnsupportedRustType {
                provenance,
                rendered,
            } => write!(
                f,
                "rustc type `{}` has no Regseq IR representation (at `{}`)",
                rendered, provenance.item_path,
            ),
            Self::UnsupportedConstShape {
                provenance,
                rendered,
            } => write!(
                f,
                "unsupported constant shape `{}` (at `{}`)",
                rendered, provenance.item_path,
            ),
            Self::UnsupportedProjection {
                provenance,
                rendered,
            } => write!(
                f,
                "unsupported MIR projection `{}` (at `{}`)",
                rendered, provenance.item_path,
            ),
            Self::UnsupportedRvalue {
                provenance,
                rendered,
            } => write!(
                f,
                "unsupported MIR rvalue `{}` (at `{}`)",
                rendered, provenance.item_path,
            ),
            Self::UnsupportedTerminator {
                provenance,
                rendered,
            } => write!(
                f,
                "unsupported MIR terminator `{}` (at `{}`)",
                rendered, provenance.item_path,
            ),
            Self::UnresolvedCallTarget {
                provenance,
                rendered,
            } => write!(
                f,
                "MIR call target `{}` cannot be resolved to a FunctionId (at `{}`)",
                rendered, provenance.item_path,
            ),
        }
    }
}

impl std::error::Error for ConstructionError {}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Cross-reference maps produced by Pass 1. Downstream passes (task 43+
/// MIR lowering) consume these to resolve rustc [`DefId`]s to the
/// model's typed IDs without re-walking the HIR.
#[derive(Clone, Default)]
pub struct CrossRefMaps {
    /// Local ADTs keyed by their rustc `DefId`.
    pub type_defs: std::collections::HashMap<DefId, regseq_model::identity::TypeDefId>,
    /// `#[rseq::register_block]` structs.
    pub register_blocks: std::collections::HashMap<DefId, regseq_model::identity::RegisterBlockId>,
    /// Free functions and methods, used by Pass 2 to resolve call targets
    /// against the model's function surface.
    pub functions: std::collections::HashMap<DefId, regseq_model::identity::FunctionId>,
    /// `register_bitfields!`-style descriptor modules.
    pub register_descriptors:
        std::collections::HashMap<DefId, regseq_model::identity::RegisterDescriptorId>,
    /// Named constants and statics (free or associated), used by Pass 2
    /// to resolve `Operand::Constant` references from unevaluated MIR
    /// constants.
    pub constants: std::collections::HashMap<DefId, regseq_model::identity::ConstantId>,
}

/// Full Pass 1 output: the declaration-level [`RegseqModel`] plus the
/// cross-reference maps needed by Pass 2 to resolve rustc [`DefId`]s to
/// model IDs.
pub struct Pass1Output {
    pub model: RegseqModel,
    pub cross_refs: CrossRefMaps,
}

/// Run Pass 1 of Regseq-Model construction and return a
/// [`RegseqModel`] with every declaration-level identifier assigned.
///
/// Function bodies are attached as empty `Body` placeholders -- Pass 2
/// (out of scope for this function) replaces them with translated MIR.
/// Every binding-side slot on resources is set to `None`; platform join
/// (Phase 6) populates them later.
pub fn build_declaration_skeleton<'tcx>(
    tcx: TyCtxt<'tcx>,
    registries: &ExtractionRegistries<'_>,
) -> Result<RegseqModel, ConstructionError> {
    build_declaration_skeleton_inner(tcx, registries).map(|out| out.model)
}

fn build_declaration_skeleton_inner<'tcx>(
    tcx: TyCtxt<'tcx>,
    registries: &ExtractionRegistries<'_>,
) -> Result<Pass1Output, ConstructionError> {
    let mut symbols = SymbolInterner::new();
    let mut source_locations: BTreeMap<DeclId, SourceOrigin> = BTreeMap::new();
    let mut allocator = DeclIdAllocator::new();

    let crate_name = tcx.crate_name(rustc_hir::def_id::LOCAL_CRATE);
    let root_name: Symbol = symbols.intern(crate_name.as_str());
    let root_id: ModuleId = ModuleId::from(allocator.alloc());
    // The crate root has its own DefId (`CRATE_DEF_ID`) with a real span
    // covering the whole crate file; use it rather than borrowing a span
    // from the first free item (which can be synthetic or missing).
    let root_def_id = rustc_hir::def_id::CRATE_DEF_ID.to_def_id();
    record_span(
        tcx,
        &mut symbols,
        &mut source_locations,
        root_id.decl_id(),
        tcx.def_span(root_def_id),
    );

    let mut cx = walk::BuildContext {
        tcx,
        registries,
        symbols: &mut symbols,
        source_locations: &mut source_locations,
        allocator: &mut allocator,
        driver_modules: Vec::new(),
        type_def_map: std::collections::HashMap::new(),
        register_block_map: std::collections::HashMap::new(),
        function_map: std::collections::HashMap::new(),
        descriptor_map: std::collections::HashMap::new(),
        constant_map: std::collections::HashMap::new(),
    };

    let mut root_module = Module {
        id: root_id,
        name: root_name,
        kind: ModuleKind::Normal,
        submodules: Vec::new(),
        constants: Vec::new(),
        free_functions: Vec::new(),
        type_defs: Vec::new(),
        register_descriptors: Vec::new(),
        register_blocks: Vec::new(),
        impl_blocks: Vec::new(),
        driver: None,
        trait_impls: Vec::new(),
        driver_impl_blocks: Vec::new(),
        abstractions_used: std::collections::BTreeSet::new(),
        intrinsics_used: std::collections::BTreeSet::new(),
    };

    walk::populate_module_items_and_submodules(
        &mut cx,
        &mut root_module,
        rustc_hir::def_id::CRATE_DEF_ID.to_def_id(),
    )?;

    match cx.driver_modules.len() {
        0 => {
            return Err(ConstructionError::NoDriverStruct {
                crate_root: ErrorProvenance::for_def_id(tcx, root_def_id),
            });
        }
        1 => {}
        _ => {
            return Err(ConstructionError::MultipleDriverStructs {
                drivers: cx.driver_modules,
            });
        }
    }

    let cross_refs = CrossRefMaps {
        type_defs: cx.type_def_map,
        register_blocks: cx.register_block_map,
        functions: cx.function_map,
        register_descriptors: cx.descriptor_map,
        constants: cx.constant_map,
    };

    Ok(Pass1Output {
        model: RegseqModel {
            symbols,
            source_locations,
            root_module,
            format_version: 1,
        },
        cross_refs,
    })
}

/// Run Pass 1 and return both the [`RegseqModel`] and the cross-reference
/// maps downstream passes need to resolve rustc [`DefId`]s to model IDs.
/// The plain [`build_declaration_skeleton`] discards these maps; when
/// wiring Pass 2 (task 43+), use this entry point instead so call targets
/// and type references can be resolved without re-walking the HIR.
pub fn build_pass1_output<'tcx>(
    tcx: TyCtxt<'tcx>,
    registries: &ExtractionRegistries<'_>,
) -> Result<Pass1Output, ConstructionError> {
    build_declaration_skeleton_inner(tcx, registries)
}

// ---------------------------------------------------------------------------
// Span helpers (shared with walk submodule)
// ---------------------------------------------------------------------------

/// Resolve a [`Span`] into a [`SourceOrigin`] and record it under
/// `decl_id`. Returns early for dummy spans, writing
/// [`SourceOrigin::CompilerGenerated`] instead (should not occur in Pass
/// 1).
pub(crate) fn record_span(
    tcx: TyCtxt<'_>,
    symbols: &mut SymbolInterner,
    out: &mut BTreeMap<DeclId, SourceOrigin>,
    decl_id: DeclId,
    span: Span,
) {
    let origin = resolve_span(tcx, symbols, span).unwrap_or(SourceOrigin::CompilerGenerated);
    out.insert(decl_id, origin);
}

/// Resolve a span into a [`SourceOrigin`], returning
/// [`SourceOrigin::CompilerGenerated`] when the span cannot be mapped
/// to a real file (dummy span, cross-file macro, etc.).
pub(crate) fn resolve_origin(
    tcx: TyCtxt<'_>,
    symbols: &mut SymbolInterner,
    span: Span,
) -> SourceOrigin {
    resolve_span(tcx, symbols, span).unwrap_or(SourceOrigin::CompilerGenerated)
}

fn resolve_span(tcx: TyCtxt<'_>, symbols: &mut SymbolInterner, span: Span) -> Option<SourceOrigin> {
    if span.is_dummy() {
        return None;
    }
    // Drill through macro expansions to the originating user call
    // site. Pass 2's spec says `Source(...)` whenever a rustc `Span`
    // is available and non-dummy, and reserves `CompilerGenerated`
    // for genuinely synthesized nodes (see ir-reference.md Section 7
    // design rationale: "use the originating user span"). A macro
    // body's span points at a synthetic expansion file; walking to
    // `source_callsite()` recovers the author-visible location.
    let span = span.source_callsite();
    let source_map = tcx.sess.source_map();
    let lo = source_map.lookup_char_pos(span.lo());
    let hi = source_map.lookup_char_pos(span.hi());

    let (lo_real, hi_real) = match (&lo.file.name, &hi.file.name) {
        (rustc_span::FileName::Real(l), rustc_span::FileName::Real(h)) => (l, h),
        _ => return None,
    };
    if lo_real != hi_real {
        return None;
    }
    let file_path = lo_real.local_path()?.to_string_lossy().into_owned();
    let file = symbols.intern_file(&file_path);

    Some(SourceOrigin::Source(SourceLocation {
        file,
        start_line: lo.line as u32,
        start_column: (lo.col.0 as u32) + 1,
        end_line: hi.line as u32,
        end_column: (hi.col.0 as u32) + 1,
    }))
}
