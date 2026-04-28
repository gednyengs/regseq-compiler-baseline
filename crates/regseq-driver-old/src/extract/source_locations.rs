//! Declaration-level source location extraction.
//!
//! Collects source spans for every local-crate item that carries a [`DefId`]:
//! structs, enums, free functions and associated methods, constants and
//! statics (including associated constants), impl blocks, and modules. Each
//! span is resolved via the session source map into a file path and
//! 1-based (line, column) range.
//!
//! The resulting [`RawSourceLocationSurface`] is consumed later by the
//! normalization pass that populates `RegseqModel.source_locations`.
//! Body-level spans are out of scope for this pass and are handled during
//! MIR body construction in Phase 5.

use std::collections::BTreeMap;

use rustc_hir::def::DefKind;
use rustc_hir::def_id::DefId;
use rustc_hir::ItemKind;
use rustc_middle::ty::TyCtxt;
use rustc_span::source_map::SourceMap;
use rustc_span::{FileName, Span};
use serde::{Deserialize, Serialize};

/// Source location information for a single item.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawSourceLocation {
    /// File path resolved from the session source map. Relative to the
    /// compilation working directory when available.
    pub file: String,
    /// 1-based start line.
    pub start_line: u32,
    /// 1-based start column.
    pub start_col: u32,
    /// 1-based end line.
    pub end_line: u32,
    /// 1-based end column.
    pub end_col: u32,
}

/// Collected source locations for all extracted items.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawSourceLocationSurface {
    /// Map from item path (as rendered by `tcx.def_path_str`) to the
    /// item's source location. `BTreeMap` gives deterministic JSON
    /// serialization ordering.
    pub locations: BTreeMap<String, RawSourceLocation>,
}

impl RawSourceLocationSurface {
    /// Number of items with recorded source locations.
    pub fn len(&self) -> usize {
        self.locations.len()
    }

    /// Whether any source locations were recorded.
    pub fn is_empty(&self) -> bool {
        self.locations.is_empty()
    }

    /// Look up the recorded source location for an item path.
    pub fn get(&self, path: &str) -> Option<&RawSourceLocation> {
        self.locations.get(path)
    }
}

/// Extract source locations for every local-crate item that carries a
/// [`DefId`] and is relevant to the declaration skeleton.
///
/// Structs, enums, free functions, constants, statics, modules, impl blocks,
/// and their associated items (methods, associated constants) are included.
/// Items whose `def_span` is a dummy span or is not backed by a real file
/// (e.g. synthetic compiler-generated items) are silently skipped.
pub fn extract_source_locations<'tcx>(tcx: TyCtxt<'tcx>) -> RawSourceLocationSurface {
    let mut locations: BTreeMap<String, RawSourceLocation> = BTreeMap::new();
    let source_map = tcx.sess.source_map();

    for item_id in tcx.hir_free_items() {
        let item = tcx.hir_item(item_id);
        let def_id = item.owner_id.def_id.to_def_id();

        match &item.kind {
            ItemKind::Struct(..)
            | ItemKind::Enum(..)
            | ItemKind::Fn { .. }
            | ItemKind::Const(..)
            | ItemKind::Static(..)
            | ItemKind::Mod(..) => {
                record_span(tcx, source_map, def_id, &mut locations);
            }
            ItemKind::Impl(impl_block) => {
                record_span(tcx, source_map, def_id, &mut locations);
                for impl_item_ref in impl_block.items {
                    let impl_item_def_id = impl_item_ref.owner_id.def_id.to_def_id();
                    record_span(tcx, source_map, impl_item_def_id, &mut locations);
                }
            }
            _ => {}
        }
    }

    RawSourceLocationSurface { locations }
}

/// Record a source location for `def_id` if its span is backed by a real
/// source file. Silently drops dummy spans and synthetic file names.
fn record_span(
    tcx: TyCtxt<'_>,
    source_map: &SourceMap,
    def_id: DefId,
    out: &mut BTreeMap<String, RawSourceLocation>,
) {
    let span = tcx.def_span(def_id);
    let Some(loc) = span_to_location(source_map, span) else {
        return;
    };
    let key = item_path_key(tcx, def_id);
    out.insert(key, loc);
}

/// Build a stable, collision-free key for an item's source-location entry.
///
/// [`TyCtxt::def_path_str`] renders an inherent `impl Type` as just `Type`,
/// which collides with the self-type's own entry in the surface. For impl
/// blocks we therefore append the `{impl#N}` disambiguator from the
/// definition path.
fn item_path_key(tcx: TyCtxt<'_>, def_id: DefId) -> String {
    let base = tcx.def_path_str(def_id);
    if matches!(tcx.def_kind(def_id), DefKind::Impl { .. }) {
        let disambiguator = tcx
            .def_path(def_id)
            .data
            .last()
            .map(|c| c.disambiguator)
            .unwrap_or(0);
        format!("{base}::{{impl#{disambiguator}}}")
    } else {
        base
    }
}

/// Resolve a [`Span`] into a [`RawSourceLocation`]. Returns `None` for
/// dummy spans, spans that do not resolve to a real file on disk, or
/// spans whose start and end fall in different source files.
fn span_to_location(source_map: &SourceMap, span: Span) -> Option<RawSourceLocation> {
    if span.is_dummy() {
        return None;
    }

    let lo = source_map.lookup_char_pos(span.lo());
    let hi = source_map.lookup_char_pos(span.hi());

    let (lo_real, hi_real) = match (&lo.file.name, &hi.file.name) {
        (FileName::Real(lo_real), FileName::Real(hi_real)) => (lo_real, hi_real),
        _ => return None,
    };

    if lo_real != hi_real {
        return None;
    }

    let file = lo_real.local_path()?.to_string_lossy().into_owned();

    Some(RawSourceLocation {
        file,
        start_line: lo.line as u32,
        start_col: (lo.col.0 as u32) + 1,
        end_line: hi.line as u32,
        end_col: (hi.col.0 as u32) + 1,
    })
}
