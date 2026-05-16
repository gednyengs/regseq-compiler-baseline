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

use crate::args_key::{GenericArg, GenericArgs};
use crate::def_path::{render_def_path, DefPath};
use crate::ids::{ConstId, InstanceId, ItemId, SourceFileId, TyId};
use crate::instance::InstanceKind;
use crate::item::{InlineHint, ItemKind, Linkage};
use crate::model::RegseqModel;
use crate::source::Span;
use crate::ty::{
    AutoTrait, FloatKind, FnAbi, IntKind, Mutability, Provenance, Safety, Ty, UintKind,
};

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

    /// Type rendering. Dispatches across the closed 19-variant
    /// `Ty` enum per `pretty-printer.md` §6.1 - §6.6. All nested
    /// `TyId` payloads resolve recursively through this method.
    pub fn render_ty(&self, id: TyId) -> String {
        match self.model.types.resolve(id) {
            // §6.1 Primitives.
            Ty::Bool => "bool".to_owned(),
            Ty::Char => "char".to_owned(),
            Ty::Int(kind) => match kind {
                IntKind::I8 => "i8",
                IntKind::I16 => "i16",
                IntKind::I32 => "i32",
                IntKind::I64 => "i64",
                IntKind::I128 => "i128",
                IntKind::Isize => "isize",
            }
            .to_owned(),
            Ty::Uint(kind) => match kind {
                UintKind::U8 => "u8",
                UintKind::U16 => "u16",
                UintKind::U32 => "u32",
                UintKind::U64 => "u64",
                UintKind::U128 => "u128",
                UintKind::Usize => "usize",
            }
            .to_owned(),
            Ty::Float(kind) => match kind {
                FloatKind::F16 => "f16",
                FloatKind::F32 => "f32",
                FloatKind::F64 => "f64",
                FloatKind::F128 => "f128",
            }
            .to_owned(),
            Ty::Str => "str".to_owned(),
            Ty::Never => "!".to_owned(),

            // §6.2 Compound non-recursive.
            Ty::Array(elem, n) => format!("[{}; {}]", self.render_ty(*elem), n),
            Ty::Slice(elem) => format!("[{}]", self.render_ty(*elem)),
            Ty::Ref(pointee, Provenance::SharedBorrow) => {
                format!("&{}", self.render_ty(*pointee))
            }
            Ty::Ref(pointee, Provenance::UniqueBorrow) => {
                format!("&mut {}", self.render_ty(*pointee))
            }
            Ty::Ptr(pointee, Mutability::Not) => {
                format!("*const {}", self.render_ty(*pointee))
            }
            Ty::Ptr(pointee, Mutability::Mut) => {
                format!("*mut {}", self.render_ty(*pointee))
            }

            // §6.3 Tuple.
            Ty::Tuple { fields, .. } => match fields.as_slice() {
                [] => "()".to_owned(),
                [only] => format!("({},)", self.render_ty(*only)),
                many => {
                    let parts: Vec<String> = many.iter().map(|t| self.render_ty(*t)).collect();
                    format!("({})", parts.join(", "))
                }
            },

            // §6.4 ADT and Closure.
            Ty::Struct { item, args, .. }
            | Ty::Enum { item, args, .. }
            | Ty::Union { item, args, .. } => {
                let path = self.render_def_path(*item);
                let args_text = self.render_generic_args(args, *item);
                format!("{path}{args_text}")
            }
            Ty::Closure(item, args) => {
                let path = self.render_def_path(*item);
                let args_text = self.render_generic_args(args, *item);
                format!("{path}{args_text}")
            }

            // §6.4 FnDef: rustc-faithful curly-brace form. Look up
            // the matching `Instance` by `(item, generic_args)`,
            // resolve its `InstanceKind::Function { fn_sig, .. }`,
            // and render the post-monomorphization signature's
            // arg / return types via `render_ty`.
            Ty::FnDef(item, args) => {
                let path = self.render_def_path(*item);
                let args_text = self.render_generic_args(args, *item);
                let inner = format!("{path}{args_text}");
                let fn_sig_id = self
                    .model
                    .instances
                    .iter()
                    .find(|inst| inst.item == *item && &inst.generic_args == args)
                    .and_then(|inst| match &inst.kind {
                        InstanceKind::Function(data) => Some(data.fn_sig),
                        _ => None,
                    })
                    .unwrap_or_else(|| {
                        panic!(
                            "render_ty: Ty::FnDef has no matching function Instance \
                             for ItemId {} with the given GenericArgs",
                            item.as_u32()
                        )
                    });
                let sig = self.model.sigs.resolve(fn_sig_id);
                let inputs: Vec<String> = sig.inputs.iter().map(|t| self.render_ty(*t)).collect();
                let output = self.render_ty(sig.output);
                format!("fn({}) -> {} {{{}}}", inputs.join(", "), output, inner)
            }

            // §6.5 Function pointer.
            Ty::FnPtr(sig_id) => {
                let sig = self.model.sigs.resolve(*sig_id);
                let mut head = String::new();
                if matches!(sig.safety, Safety::Unsafe) {
                    head.push_str("unsafe ");
                }
                if !matches!(sig.abi, FnAbi::Rust) {
                    let abi = match sig.abi {
                        FnAbi::Rust => "Rust",
                        FnAbi::C => "C",
                    };
                    head.push_str(&format!("extern \"{abi}\" "));
                }
                let inputs: Vec<String> = sig.inputs.iter().map(|t| self.render_ty(*t)).collect();
                let output = self.render_ty(sig.output);
                format!("{head}fn({}) -> {}", inputs.join(", "), output)
            }

            // §6.6 Dynamic.
            Ty::Dynamic {
                auto_traits,
                vtable_ty,
            } => {
                let vtable_item = match self.model.types.resolve(*vtable_ty) {
                    Ty::Struct { item, .. } => *item,
                    other => panic!(
                        "render_ty: Ty::Dynamic.vtable_ty must resolve to Ty::Struct, got {:?}",
                        other
                    ),
                };
                let mut out = format!("dyn {}", self.render_def_path(vtable_item));
                // Per pretty-printer.md §6.6 + spec.md §10.1: auto-traits
                // render in sorted order (Send < Sync < Unpin). Although
                // model validation enforces sorted storage, the renderer
                // sorts defensively so a malformed-but-loaded input still
                // produces the canonical textual form (matches the
                // same defensive sort in args_key.rs canonical encoding).
                let mut sorted: Vec<AutoTrait> = auto_traits.clone();
                sorted.sort();
                for auto in sorted {
                    let label = match auto {
                        AutoTrait::Send => "Send",
                        AutoTrait::Sync => "Sync",
                        AutoTrait::Unpin => "Unpin",
                    };
                    out.push_str(" + ");
                    out.push_str(label);
                }
                out
            }
        }
    }

    /// Generic-argument rendering per `pretty-printer.md` §6.7.
    ///
    /// Returns the empty `String` when `args.is_empty()`; otherwise
    /// returns `<T1, T2, K1>` with `, ` separators wrapped in
    /// angle brackets. The caller does not need to add a wrapper.
    ///
    /// Per stage-14 D-14-4.9, the enclosing `ItemId` is required so
    /// each `GenericArg::Const(c)` arg can be resolved to the
    /// declared type of the matching positional const-generic
    /// parameter from `model.items[item]`'s generic-parameter list.
    /// `render_const` (P1a-PP-04a/b) is not yet available; the
    /// Const-arm body therefore stays `todo!()` per this task's
    /// spec until P1a-PP-04a lands.
    pub fn render_generic_args(&self, args: &GenericArgs, item: ItemId) -> String {
        if args.is_empty() {
            return String::new();
        }
        let parts: Vec<String> = args
            .iter()
            .map(|arg| match arg {
                GenericArg::Type(ty) => self.render_ty(*ty),
                GenericArg::Const(_c) => {
                    // Per spec: Const-arm wiring depends on
                    // P1a-PP-04a's `render_const`. The helper
                    // signature is fixed; the body stays `todo!()`
                    // until that task lands.
                    let _ = item;
                    todo!(
                        "render_generic_args: GenericArg::Const wiring lands with P1a-PP-04a (render_const)"
                    )
                }
            })
            .collect();
        format!("<{}>", parts.join(", "))
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
