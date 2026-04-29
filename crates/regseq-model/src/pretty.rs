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
//! | `BodyId`      | `Body/N` (or owning item path)          |
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
use crate::ids::{ConstId, InstanceId, ItemId, TyId};
use crate::model::RegseqModel;
use crate::source::Span;

/// Context-bearing renderer over a `RegseqModel`.
///
/// Stage 1 ships the shape; stages 3-13 fill in the
/// `unimplemented!()` per-type render methods as their IR types
/// land.
pub struct PrettyPrinter<'a> {
    model: &'a RegseqModel,
}

impl<'a> PrettyPrinter<'a> {
    /// Constructs a printer over `model`.
    pub fn new(model: &'a RegseqModel) -> Self {
        Self { model }
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

    /// Type rendering. Filled in stage 3 (Type system).
    pub fn render_ty(&self, _ty: TyId) -> String {
        unimplemented!("stage 3 fills render_ty")
    }

    /// Const rendering. Filled in stage 4 (Constants).
    pub fn render_const(&self, _c: ConstId) -> String {
        unimplemented!("stage 4 fills render_const")
    }

    /// Item-keyed `DefPath` rendering. Filled in stage 12 (Items).
    /// Stage 12's body resolves
    /// `self.model.strings.resolve(self.model.items[id].def_path.rendered).to_owned()`
    /// -- no re-rendering, just resolving the cached canonical
    /// form (the eager `rendered` field is the cached result of
    /// `render_def_path` at construction time).
    pub fn render_def_path(&self, _item: ItemId) -> String {
        unimplemented!(
            "stage 12 fills render_def_path by reading Item.def_path.rendered (an interned StringId); use render_def_path_value(&DefPath) at stage 1"
        )
    }

    /// Instance rendering. Filled in stage 13.
    pub fn render_instance(&self, _id: InstanceId) -> String {
        unimplemented!("stage 13 fills render_instance")
    }
}
