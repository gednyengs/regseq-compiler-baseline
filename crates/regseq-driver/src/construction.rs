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

pub mod driver;
pub mod id_allocator;
pub mod impls;
pub mod pin_state_registry;
pub mod registers;
pub mod walk;

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
    NoDriverStruct,
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
}

impl std::fmt::Display for ConstructionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoDriverStruct => write!(f, "no #[rseq::driver] struct found in crate"),
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
        }
    }
}

impl std::error::Error for ConstructionError {}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

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
    let mut symbols = SymbolInterner::new();
    let mut source_locations: BTreeMap<DeclId, SourceOrigin> = BTreeMap::new();
    let mut allocator = DeclIdAllocator::new();

    let crate_name = tcx.crate_name(rustc_hir::def_id::LOCAL_CRATE);
    let root_name: Symbol = symbols.intern(crate_name.as_str());
    let root_id: ModuleId = ModuleId::from(allocator.alloc());
    // Root crate module has no DefId-backed span; record as compiler
    // generated fallback only if span resolution fails.
    let root_span = tcx
        .hir_crate_items(())
        .free_items()
        .next()
        .map(|id| tcx.def_span(id.owner_id.def_id.to_def_id()))
        .unwrap_or_default();
    record_span(
        tcx,
        &mut symbols,
        &mut source_locations,
        root_id.decl_id(),
        root_span,
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
        0 => return Err(ConstructionError::NoDriverStruct),
        1 => {}
        _ => {
            return Err(ConstructionError::MultipleDriverStructs {
                drivers: cx.driver_modules,
            });
        }
    }

    Ok(RegseqModel {
        symbols,
        source_locations,
        root_module,
        format_version: 1,
    })
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
