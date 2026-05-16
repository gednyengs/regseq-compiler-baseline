//! Textual pretty-printer for `RegseqModel`.
//!
//! Per Q5 the pretty-printer is the primary serialization (the
//! textual MIR-inspired form is the human-readable contract;
//! serde JSON is the structured-format secondary). Per-type
//! `Display` impls are insufficient: rendering requires context
//! (the model's tables for ID resolution), so the pretty-printer
//! holds a reference to a `RegseqModel` and resolves IDs through
//! it.
//!
//! Stage 1's contribution is the *shape* and the conventions
//! table. Methods that depend on stage-1 surface only --
//! `render_span`, `render_def_path_value` -- are implemented
//! here. Methods that depend on later-stage IR content
//! (`render_ty`, `render_const`, `render_def_path(ItemId)`,
//! `render_instance`, top-level `render`) are stubbed
//! `unimplemented!(...)` with messages pointing at the stage
//! that fills them.
//!
//! Sigil-conventions table:
//!
//! | ID            | Sigil                                   |
//! |---------------|-----------------------------------------|
//! | `ItemId`      | `Item/N`                                |
//! | `TyId`        | rendered as the type itself             |
//! | `ConstId`     | rendered as the const value             |
//! | `LocalId`     | `_N` (zero-based; `_0` is RETURN_PLACE) |
//! | `BlockId`     | `bbN`                                   |
//! | `SourceFileId`| rendered as the file path               |
//! | `StringId`    | rendered as the resolved string         |
//! | `Span`        | `path:line:col: line:col`               |
//! | `InstanceId`  | `Item::<args>`                          |
//!
//! `LocalId` (`_N`) and `BlockId` (`bbN`) sigils land in stages 10
//! and 11 alongside the IDs themselves.

use crate::def_path::{render_def_path, DefPath};
use crate::ids::{ConstId, InstanceId, ItemId, SourceFileId, TyId};
use crate::item::{InlineHint, ItemKind, Linkage};
use crate::model::RegseqModel;
use crate::source::Span;
use crate::ty::Mutability;

/// Column at which trailing `// <span>` comments are aligned per
/// `pretty-printer.md` §4.2. Matches rustc's `mir/pretty.rs:21`
/// `ALIGN = 40`.
const ALIGN: usize = 40;

/// Renderer options per `pretty-printer.md` §4.1.
///
/// Two independent flags gate optional output:
/// - `include_spans` enables trailing `// <span>` comments per §4.2.
/// - `include_debug_names` enables `debug NAME => _N;` lines per
///   §4.4 (consumed by body-level renderers).
///
/// Defaults are off; consumers opt in via [`PrettyPrinter::with_options`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct PrettyPrintOptions {
    /// Emit trailing `// <render_span(...)>` comments on Item,
    /// Statement, Terminator, and LocalDecl lines.
    pub include_spans: bool,
    /// Emit `debug NAME => _N;` debug-info lines after locals
    /// declarations.
    pub include_debug_names: bool,
}

/// Context-bearing renderer over a `RegseqModel`.
///
/// Stage 1 ships the shape; stages 4-14 fill in the
/// `unimplemented!()` per-type render methods as their IR types
/// land.
pub struct PrettyPrinter<'a> {
    model: &'a RegseqModel,
    options: PrettyPrintOptions,
}

impl<'a> PrettyPrinter<'a> {
    /// Constructs a printer over `model` with default-off options.
    pub fn new(model: &'a RegseqModel) -> Self {
        Self {
            model,
            options: PrettyPrintOptions::default(),
        }
    }

    /// Constructs a printer over `model` with caller-supplied options.
    pub fn with_options(model: &'a RegseqModel, options: PrettyPrintOptions) -> Self {
        Self { model, options }
    }

    /// Renders a `Span` per the spec sigil
    /// `"{path}:{lo_line}:{lo_col}: {hi_line}:{hi_col}"`.
    ///
    /// The synthesized-source sentinel renders uniformly --
    /// `with_primary` interns the literal `"<synthesized>"` for
    /// `source_files[0].path`, so a span with
    /// `file == SourceFileId::SYNTHESIZED` and `lo == hi == 0`
    /// resolves through the standard path to
    /// `"<synthesized>:1:1: 1:1"`. No special branch.
    pub fn render_span(&self, span: Span) -> String {
        let file = &self.model.source_files[span.file];
        let path = self.model.strings.resolve(file.path);
        let lo = span.start(&self.model.source_files);
        let hi = span.end(&self.model.source_files);
        format!(
            "{path}:{lo_line}:{lo_col}: {hi_line}:{hi_col}",
            path = path,
            lo_line = lo.line,
            lo_col = lo.col,
            hi_line = hi.line,
            hi_col = hi.col
        )
    }

    /// Renders a `DefPath` to its canonical verbose form by
    /// delegating to the free `render_def_path` helper. Useful at
    /// stage 1 when no `Item` is populated yet (so the spec's
    /// `render_def_path(ItemId)` form has nothing to look up); at
    /// stage 12 the `ItemId` overload becomes functional by
    /// reading the cached `Item.def_path.rendered` interned
    /// string instead of re-rendering.
    pub fn render_def_path_value(&self, path: &DefPath) -> String {
        render_def_path(path, &self.model.crates, &self.model.strings)
    }

    /// Top-level dump of the entire model. Filled in stage 12+
    /// once the body content (stage 11), Item content
    /// (stage 12), and instances exist in non-trivial form.
    pub fn render(&self) -> String {
        unimplemented!("stage 12+ fills the top-level render")
    }

    /// Type rendering. Filled in stage 13 (type-system
    /// pretty-printing). Stage 3 specified the closed `Ty` enum
    /// but deferred the renderer-style decision (originally
    /// reserved as D14) to stage 13, where the renderer's input
    /// is fully populated by stage-11 (`FnSig`) and stage-12
    /// (`Item` / DefPath) content.
    pub fn render_ty(&self, _ty: TyId) -> String {
        unimplemented!("stage 13 fills render_ty")
    }

    /// Const rendering. Deferred to stage 14 because complete
    /// rendering depends on const bodies, type rendering, def-path
    /// rendering, and instance rendering.
    pub fn render_const(&self, _c: ConstId) -> String {
        unimplemented!("stage 14 fills render_const (deferred per D-4-K)")
    }

    /// Item-keyed `DefPath` rendering. Resolves the eager
    /// `Item.def_path.rendered` cache (per pretty-printer.md §8
    /// ownership table; stage-12 D-12-6 cached-lookup
    /// convention) -- no re-rendering, just resolving the cached
    /// canonical form interned at construction time.
    pub fn render_def_path(&self, item: ItemId) -> String {
        self.model
            .strings
            .resolve(self.model.items[item].def_path.rendered)
            .to_owned()
    }

    /// Item-level wrapper per `pretty-printer.md` §5: attribute
    /// block (one `#[...]` per line, §5.3 row order, §5.2
    /// default-suppression) plus kind-keyword (with `unsafe`
    /// prefix on `fn` when `Item.is_unsafe` per §5.4) plus the
    /// rendered DefPath. Owns the §4 / §4.2 / §4.3 span gating
    /// for the Item header source position (`Item.span`).
    pub fn render_item(&self, id: ItemId) -> String {
        let item = &self.model.items[id];
        let attr_block = self.render_item_attribute_block(id);
        let kind_keyword = match &item.kind {
            ItemKind::Function => {
                if item.is_unsafe {
                    "unsafe fn"
                } else {
                    "fn"
                }
            }
            ItemKind::Static {
                mutability: Mutability::Not,
            } => "static",
            ItemKind::Static {
                mutability: Mutability::Mut,
            } => "static mut",
            ItemKind::Const => "const",
            ItemKind::Struct => "struct",
            ItemKind::Enum => "enum",
            ItemKind::Union => "union",
        };
        let path = self.render_def_path(id);
        let header_line = format!("{kind_keyword} {path}");

        // §4 Item-header span gating: emit `// <render_span(span)>`
        // aligned to column ALIGN, only when `include_spans == true`
        // AND `span.file != SourceFileId::SYNTHESIZED` (§4.3).
        let header = if self.options.include_spans && item.span.file != SourceFileId::SYNTHESIZED {
            let span_text = self.render_span(item.span);
            // Match rustc mir/pretty.rs ALIGN convention: pad the
            // header to width ALIGN, then ` // <comment>`. When the
            // header is longer than ALIGN, the format spec leaves
            // the header at natural width so the trailing comment
            // follows after the single literal space.
            format!("{header_line:ALIGN$} // {span_text}")
        } else {
            header_line
        };

        format!("{attr_block}{header}")
    }

    /// Shared §5.6 helper: walks the §5.3 per-flag attribute table
    /// in row order, emits surviving `#[...]` rows per §5.2
    /// default-suppression. Returns a multi-line block ending in a
    /// trailing `\n` when non-empty, or an empty string when zero
    /// attributes survive. Consumed by both `render_item` (§5.4)
    /// and the future `render_instance` composer (§10.2.7 Stream 1)
    /// for byte-identical Item-flag attribute output.
    fn render_item_attribute_block(&self, id: ItemId) -> String {
        let item = &self.model.items[id];
        let mut out = String::new();

        // Row 1: linkage (Default suppresses).
        match item.linkage {
            Linkage::NoMangle => out.push_str("#[no_mangle]\n"),
            Linkage::Foreign => out.push_str("#[foreign]\n"),
            Linkage::Weak => out.push_str("#[linkage = \"weak\"]\n"),
            Linkage::Default => {}
        }

        // Row 2: inline_hint (None suppresses).
        match item.inline_hint {
            InlineHint::Hint => out.push_str("#[inline]\n"),
            InlineHint::Always => out.push_str("#[inline(always)]\n"),
            InlineHint::Never => out.push_str("#[inline(never)]\n"),
            InlineHint::None => {}
        }

        // Row 3: lang-item flags in field declaration order
        // (`is_phantom_data`, `is_unsafe_cell`, `is_manually_drop`
        // per spec.md §8.0). `is_unsafe` is intentionally NOT
        // surfaced here per pretty-printer.md:576-583.
        if item.is_phantom_data {
            out.push_str("#[lang = \"phantom_data\"]\n");
        }
        if item.is_unsafe_cell {
            out.push_str("#[lang = \"unsafe_cell\"]\n");
        }
        if item.is_manually_drop {
            out.push_str("#[lang = \"manually_drop\"]\n");
        }

        // Row 4: link_section (None suppresses).
        if let Some(section_id) = item.link_section {
            let resolved = self.model.strings.resolve(section_id);
            out.push_str(&format!("#[link_section = \"{resolved}\"]\n"));
        }

        // Row 5: opaque_handle_eligible (false suppresses).
        if item.opaque_handle_eligible {
            out.push_str("#[opaque_handle]\n");
        }

        out
    }

    /// Instance rendering. Filled in stage 14
    /// (domain-specific extensions).
    pub fn render_instance(&self, _id: InstanceId) -> String {
        unimplemented!("stage 14 fills render_instance")
    }
}

#[cfg(test)]
#[path = "pretty_tests.rs"]
mod tests;
