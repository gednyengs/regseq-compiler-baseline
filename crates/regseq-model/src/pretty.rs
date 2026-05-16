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
use crate::basic_block::BasicBlock;
use crate::body::Body;
use crate::body_ty::{operand_ty, place_ty, TypeRecovery};
use crate::const_value::Const;
use crate::def_path::{render_def_path, DefPath};
use crate::ids::{ConstId, InstanceId, ItemId, SourceFileId, TyId};
use crate::instance::InstanceKind;
use crate::item::{InlineHint, ItemKind, Linkage};
use crate::layout::{EnumShape, FieldLayout, LayoutKind};
use crate::model::RegseqModel;
use crate::operand::Operand;
use crate::place::{Place, PlaceElem};
use crate::rvalue::{AggregateKind, BinOp, CastKind, PointerCoercion, Rvalue, UnOp};
use crate::source::Span;
use crate::statement::{CopyNonOverlapping, NonDivergingIntrinsic, Statement, StatementKind};
use crate::terminator::{AssertKind, CallCallee, SwitchTargets, Terminator, TerminatorKind};
use crate::ty::{
    AutoTrait, FloatKind, FnAbi, IntKind, Mutability, Provenance, Safety, Ty, UintKind,
};
use crate::ty_interner::NominalKind;

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

    /// Per-variant Const value renderer.
    ///
    /// Pair-form per `pretty-printer.md` §8 ownership table and
    /// D-14-4.1 LOCKED: takes the consumer-context `TyId` at the use
    /// site so type-context-dependent arms (`ZST` / aggregate / bytes
    /// / null-pointer / reloc) can dispatch on `Ty.kind()`.
    ///
    /// Dispatch per `pretty-printer.md` §10.1:
    /// - `Bool / Char / Int / Uint / Float` key on the variant tag
    ///   and ignore the consumer Ty (§10.1.1).
    /// - `ZST` keys on the consumer `Ty.kind()` and recurses on the
    ///   same `ConstId` for composite cases (§10.1.2).
    /// - `Aggregate / EnumValue / Bytes / NullPtr / Reloc` are owned
    ///   by P1a-PP-04b (regseq-model-90) and stay `todo!()` until
    ///   that task lands.
    pub fn render_const(&self, id: ConstId, expected_ty: TyId) -> String {
        match &self.model.consts[id] {
            Const::Bool(value) => render_const_bool(*value),
            Const::Char(code_point) => render_const_char(*code_point),
            Const::Int(kind, bits) => self.render_const_int(*kind, *bits),
            Const::Uint(kind, bits) => self.render_const_uint(*kind, *bits),
            Const::Float(kind, bits) => render_const_float(*kind, *bits),
            Const::ZST => self.render_const_zst(id, expected_ty),
            Const::Aggregate(_) => {
                todo!("render_const Aggregate arm lands with P1a-PP-04b (regseq-model-90)")
            }
            Const::EnumValue { .. } => {
                todo!("render_const EnumValue arm lands with P1a-PP-04b (regseq-model-90)")
            }
            Const::Bytes(_) => {
                todo!("render_const Bytes arm lands with P1a-PP-04b (regseq-model-90)")
            }
            Const::NullPtr(_) => {
                todo!("render_const NullPtr arm lands with P1a-PP-04b (regseq-model-90)")
            }
            Const::Reloc(_) => {
                todo!("render_const Reloc arm lands with P1a-PP-04b (regseq-model-90)")
            }
        }
    }

    /// `Int` per-arm rendering per §10.1.1.
    ///
    /// Applies D-14-4.10 sentinel collapsing first; falls back to the
    /// underscore-decimal form. `Isize` uses the target-pointer-width
    /// column of `target.scalar_layouts.int[Isize]` to size the
    /// sentinel comparison, but always renders symbolically (no
    /// per-target decimal substitution).
    fn render_const_int(&self, kind: IntKind, bits: u128) -> String {
        let width = self.int_width_bits(kind);
        let decoded = decode_int(width, bits);
        if let Some(sentinel) = int_sentinel_label(kind, width, decoded) {
            return sentinel.to_owned();
        }
        format!("{decoded}_{}", int_kind_name(kind))
    }

    /// `Uint` per-arm rendering per §10.1.1.
    ///
    /// Per D-14-4.10 sentinel-collapsing: only `MAX` collapses (no
    /// unsigned MIN collapsing). `Usize` follows the same
    /// target-pointer-width sourcing rule as `Isize`.
    fn render_const_uint(&self, kind: UintKind, bits: u128) -> String {
        let width = self.uint_width_bits(kind);
        let decoded = decode_uint(width, bits);
        if decoded == uint_max(width) {
            return format!("{}::MAX", uint_kind_name(kind));
        }
        format!("{decoded}_{}", uint_kind_name(kind))
    }

    /// Pointer-width-sized `Int` / `Uint` width selector.
    ///
    /// Reads `target.scalar_layouts.int[Isize].size` (and the parallel
    /// `uint[Usize].size`) per `spec.md` §11 `ScalarLayouts`; falls
    /// back to the target's `pointer_width` if the ABI table has the
    /// zero default. The rendered form remains symbolic (`isize::MAX`
    /// / `usize::MAX`) regardless of the resolved width.
    fn int_width_bits(&self, kind: IntKind) -> u32 {
        match kind {
            IntKind::I8 => 8,
            IntKind::I16 => 16,
            IntKind::I32 => 32,
            IntKind::I64 => 64,
            IntKind::I128 => 128,
            IntKind::Isize => self.isize_width_bits(),
        }
    }

    fn uint_width_bits(&self, kind: UintKind) -> u32 {
        match kind {
            UintKind::U8 => 8,
            UintKind::U16 => 16,
            UintKind::U32 => 32,
            UintKind::U64 => 64,
            UintKind::U128 => 128,
            UintKind::Usize => self.usize_width_bits(),
        }
    }

    fn isize_width_bits(&self) -> u32 {
        let size = self.model.target.scalar_layouts.int[IntKind::Isize as usize].size;
        self.scalar_size_to_width_bits(size)
    }

    fn usize_width_bits(&self) -> u32 {
        let size = self.model.target.scalar_layouts.uint[UintKind::Usize as usize].size;
        self.scalar_size_to_width_bits(size)
    }

    fn scalar_size_to_width_bits(&self, size: u64) -> u32 {
        if size == 0 {
            u32::from(self.model.target.pointer_width)
        } else {
            (size as u32) * 8
        }
    }

    /// `ZST` per-Ty.kind() dispatch per §10.1.2.
    ///
    /// Recurses on the same `ConstId` for composite-ZST cases so the
    /// canonical-form rule (§9.7) is preserved at the storage level
    /// even when the rendered form expands per sub-field Ty.
    fn render_const_zst(&self, id: ConstId, expected_ty: TyId) -> String {
        match self.model.types.resolve(expected_ty) {
            Ty::Tuple { fields, .. } => self.render_zst_tuple(id, fields),
            Ty::Array(elem, n) => self.render_zst_array(id, *elem, *n),
            Ty::Struct { item, args, layout } => self.render_zst_struct(id, *item, args, layout),
            Ty::Enum { item, args, layout } => self.render_zst_enum(id, *item, args, layout),
            Ty::Union { item, args, .. } => self.render_value_namespace_path(*item, args),
            Ty::FnDef(item, args) => self.render_value_namespace_path(*item, args),
            Ty::Closure(item, args) => self.render_value_namespace_path(*item, args),
            other => panic!(
                "render_const: Const::ZST consumed by non-ZST Ty.kind() {:?}; the bridge \
                 produces a non-ZST Const variant for non-ZST consumer types per \
                 spec.md §9.7",
                other
            ),
        }
    }

    fn render_zst_tuple(&self, id: ConstId, fields: &[TyId]) -> String {
        match fields {
            [] => "()".to_owned(),
            [only] => format!("({},)", self.render_const(id, *only)),
            many => {
                let parts: Vec<String> = many.iter().map(|t| self.render_const(id, *t)).collect();
                format!("({})", parts.join(", "))
            }
        }
    }

    fn render_zst_array(&self, id: ConstId, elem: TyId, n: u64) -> String {
        if n == 0 {
            "[]".to_owned()
        } else {
            format!("[{}; {}]", self.render_const(id, elem), n)
        }
    }

    fn render_zst_struct(
        &self,
        id: ConstId,
        item: ItemId,
        args: &GenericArgs,
        layout: &crate::layout::Layout,
    ) -> String {
        let fields = match &layout.kind {
            LayoutKind::Struct { fields } => fields,
            _ => panic!(
                "render_const: Ty::Struct ZST consumer carries non-Struct layout kind \
                 (item {}); validation rejects this shape per spec.md §11",
                item.as_u32()
            ),
        };
        let head = self.render_value_namespace_path(item, args);
        if fields.is_empty() {
            return head;
        }
        let body = self.render_zst_aggregate_fields(id, fields);
        format!("{head}{body}")
    }

    fn render_zst_enum(
        &self,
        id: ConstId,
        item: ItemId,
        args: &GenericArgs,
        layout: &crate::layout::Layout,
    ) -> String {
        let shape = match &layout.kind {
            LayoutKind::Enum { shape } => shape,
            _ => panic!(
                "render_const: Ty::Enum ZST consumer carries non-Enum layout kind \
                 (item {}); validation rejects this shape per spec.md §11",
                item.as_u32()
            ),
        };
        let variant = match shape {
            EnumShape::Single { variant, .. } => variant,
            EnumShape::Multiple { .. } => panic!(
                "render_const: Const::ZST cannot be consumed by a multi-variant Ty::Enum \
                 (item {}); a multi-variant enum carries a discriminant tag of nonzero \
                 size and cannot be ZST",
                item.as_u32()
            ),
            EnumShape::Empty => panic!(
                "render_const: Const::ZST cannot be consumed by a zero-variant Ty::Enum \
                 (item {}); an empty enum has no inhabitants",
                item.as_u32()
            ),
        };
        let enum_path = self.render_value_namespace_path(item, args);
        let variant_name = self.model.strings.resolve(variant.name).to_owned();
        if variant.fields.is_empty() {
            return format!("{enum_path}::{variant_name}");
        }
        let body = self.render_zst_aggregate_fields(id, &variant.fields);
        format!("{enum_path}::{variant_name}{body}")
    }

    /// Emits the parenthesized tuple-style or brace-wrapped named-style
    /// body for a struct or enum variant whose fields are all ZST,
    /// recursing on the same `ConstId` per §10.1.2.
    fn render_zst_aggregate_fields(&self, id: ConstId, fields: &[FieldLayout]) -> String {
        if self.fields_are_tuple_synthetic(fields) {
            let parts: Vec<String> = fields.iter().map(|f| self.render_const(id, f.ty)).collect();
            format!("({})", parts.join(", "))
        } else {
            let parts: Vec<String> = fields
                .iter()
                .map(|f| {
                    let name = self.model.strings.resolve(f.name);
                    format!("{name}: {}", self.render_const(id, f.ty))
                })
                .collect();
            format!(" {{ {} }}", parts.join(", "))
        }
    }

    /// §3.4 tuple-vs-named dispatch test for ZST aggregate-field
    /// rendering: returns `true` when every field name matches the
    /// synthetic pattern `_<index>` (`_0`, `_1`, ...). Empty fields
    /// signal unit-style and are handled by the caller; this helper
    /// is only invoked when `!fields.is_empty()`.
    fn fields_are_tuple_synthetic(&self, fields: &[FieldLayout]) -> bool {
        if fields.is_empty() {
            return false;
        }
        for (i, field) in fields.iter().enumerate() {
            let name = self.model.strings.resolve(field.name);
            if name != format!("_{i}") {
                return false;
            }
        }
        true
    }

    /// Renders a value-namespace path `<def_path>::<args>` per the
    /// rustc-mirrored convention for value-level FnDef / generic-
    /// bearing ADT / Closure rendering (Specimens 89, 93). When
    /// `args.is_empty()`, emits just the def-path (Specimen 88).
    fn render_value_namespace_path(&self, item: ItemId, args: &GenericArgs) -> String {
        let path = self.render_def_path(item);
        if args.is_empty() {
            path
        } else {
            let args_text = self.render_generic_args(args, item);
            format!("{path}::{args_text}")
        }
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

    // ------------------------------------------------------------------
    // §7.1 Body-level renderers.
    // ------------------------------------------------------------------

    /// Renders a `Place` per `pretty-printer.md` §7.1.
    ///
    /// Composes the head local sigil `_<N>` plus each
    /// `PlaceElem` projection's wrap form, applied left-to-right.
    /// Field / variant names resolve via the §3.4 `place_ty` walk
    /// over the body's local-decl tables and the projection chain.
    pub fn render_place(&self, place: &Place, body: &Body) -> String {
        let mut text = format!("_{}", place.local.index());
        let mut current_ty = body.local_decls[place.local].ty;
        let mut active_variant: Option<u32> = None;

        for elem in place.projection.iter() {
            match elem {
                PlaceElem::Deref => {
                    text = format!("(*{text})");
                    if let Ty::Ref(inner, _) | Ty::Ptr(inner, _) =
                        self.model.types.resolve(current_ty)
                    {
                        current_ty = *inner;
                    }
                    active_variant = None;
                }
                PlaceElem::Field(idx, field_ty) => {
                    let field_name = self.field_name(current_ty, active_variant, idx.as_u32());
                    let field_ty_text = self.render_ty(*field_ty);
                    text = format!("({text}.{field_name}: {field_ty_text})");
                    current_ty = *field_ty;
                    active_variant = None;
                }
                PlaceElem::Index(local) => {
                    text = format!("{text}[_{}]", local.index());
                    current_ty = self.narrow_indexable_elem(current_ty).unwrap_or(current_ty);
                }
                PlaceElem::ConstantIndex {
                    offset,
                    min_length,
                    from_end: false,
                } => {
                    text = format!("{text}[{offset} of {min_length}]");
                    current_ty = self.narrow_indexable_elem(current_ty).unwrap_or(current_ty);
                }
                PlaceElem::ConstantIndex {
                    offset,
                    min_length,
                    from_end: true,
                } => {
                    text = format!("{text}[-{offset} of {min_length}]");
                    current_ty = self.narrow_indexable_elem(current_ty).unwrap_or(current_ty);
                }
                PlaceElem::Subslice {
                    from,
                    to,
                    from_end: false,
                } => {
                    text = format!("{text}[{from}..{to}]");
                    // Array predecessor narrows to a smaller array of the
                    // same element type (length = to - from); see §19.4
                    // narrowing table. If the interner has the resulting
                    // array Ty, use it; otherwise narrow to the element
                    // type for any subsequent Field projection step.
                    if let Ty::Array(elem, _) = self.model.types.resolve(current_ty) {
                        let elem = *elem;
                        let len = to.saturating_sub(*from);
                        current_ty = self.model.types.find(&Ty::Array(elem, len)).unwrap_or(elem);
                    }
                }
                PlaceElem::Subslice {
                    from,
                    to,
                    from_end: true,
                } => {
                    text = format!("{text}[{from}..-{to}]");
                    // Slice predecessor stays a slice of the same element
                    // type per §19.4; current_ty is preserved.
                }
                PlaceElem::Downcast(variant_idx) => {
                    let variant_name = self.variant_name(current_ty, variant_idx.as_u32());
                    text = format!("({text} as {variant_name})");
                    active_variant = Some(variant_idx.as_u32());
                }
            }
        }
        text
    }

    /// Renders an `Operand` per `pretty-printer.md` §7.2.
    ///
    /// Pair-form per stage-14 D-14-4.9: `Copy` / `Move` ignore
    /// `expected_ty`; `Constant` threads it to
    /// `render_const(id, expected_ty)`.
    pub fn render_operand(&self, op: &Operand, body: &Body, expected_ty: TyId) -> String {
        match op {
            Operand::Copy(place) => format!("copy {}", self.render_place(place, body)),
            Operand::Move(place) => format!("move {}", self.render_place(place, body)),
            Operand::Constant(id) => self.render_const(*id, expected_ty),
        }
    }

    /// Renders an `Rvalue` per `pretty-printer.md` §7.3.
    ///
    /// Pair-form per stage-14 D-14-4.9. Each arm derives per-operand
    /// expected `TyId`s from the `expected_ty` parameter plus the
    /// variant's structural payload, then dispatches into
    /// `render_operand`.
    pub fn render_rvalue(&self, rv: &Rvalue, body: &Body, expected_ty: TyId) -> String {
        match rv {
            Rvalue::Use(operand) => self.render_operand(operand, body, expected_ty),
            Rvalue::Repeat(operand, count) => {
                let elem_ty = match self.model.types.resolve(expected_ty) {
                    Ty::Array(elem, _) => *elem,
                    _ => expected_ty,
                };
                format!(
                    "[{}; {}]",
                    self.render_operand(operand, body, elem_ty),
                    count
                )
            }
            Rvalue::Ref(Provenance::SharedBorrow, place) => {
                format!("&{}", self.render_place(place, body))
            }
            Rvalue::Ref(Provenance::UniqueBorrow, place) => {
                format!("&mut {}", self.render_place(place, body))
            }
            Rvalue::RawPtr(Mutability::Not, place) => {
                format!("&raw const {}", self.render_place(place, body))
            }
            Rvalue::RawPtr(Mutability::Mut, place) => {
                format!("&raw mut {}", self.render_place(place, body))
            }
            Rvalue::Cast(kind, operand, ty) => {
                let source_ty = self.operand_source_ty(operand, body, expected_ty);
                let operand_text = self.render_operand(operand, body, source_ty);
                let dest_text = self.render_ty(*ty);
                let annotation = cast_kind_annotation(kind);
                match annotation {
                    Some(label) => format!("{operand_text} as {dest_text} ({label})"),
                    None => format!("{operand_text} as {dest_text}"),
                }
            }
            Rvalue::BinaryOp(op, lhs, rhs) => {
                let op_input_ty = match op {
                    BinOp::Eq | BinOp::Lt | BinOp::Le | BinOp::Ne | BinOp::Ge | BinOp::Gt => {
                        // Comparison: derive operand type from operand inspection.
                        self.operand_source_ty(lhs, body, expected_ty)
                    }
                    _ => expected_ty,
                };
                let lhs_text = self.render_operand(lhs, body, op_input_ty);
                let rhs_text = self.render_operand(rhs, body, op_input_ty);
                format!("{lhs_text} {} {rhs_text}", bin_op_glyph(op))
            }
            Rvalue::UnaryOp(op, operand) => {
                let operand_text = self.render_operand(operand, body, expected_ty);
                format!("{}{operand_text}", un_op_glyph(op))
            }
            Rvalue::Discriminant(place) => {
                format!("discriminant({})", self.render_place(place, body))
            }
            Rvalue::Aggregate(kind, operands) => self.render_aggregate(kind, operands, body),
        }
    }

    /// Renders a `Statement` per `pretty-printer.md` §7.4.
    ///
    /// Output always ends with a trailing `;`. When
    /// `PrettyPrintOptions::include_spans == true` and the statement
    /// span is not synthesized, a trailing
    /// `// <render_span(stmt.span)>` comment is emitted aligned to
    /// column `ALIGN` per §4.2 / §4.3.
    pub fn render_statement(&self, stmt: &Statement, body: &Body) -> String {
        let head = match &stmt.kind {
            StatementKind::Assign(place, rvalue) => {
                let place_text = self.render_place(place, body);
                let place_ty = self.place_ty_or(place, body, body.local_decls[place.local].ty);
                let rvalue_text = self.render_rvalue(rvalue, body, place_ty);
                format!("{place_text} = {rvalue_text};")
            }
            StatementKind::SetDiscriminant(place, variant_idx) => {
                let place_text = self.render_place(place, body);
                let place_ty = self.place_ty_or(place, body, body.local_decls[place.local].ty);
                let variant_name = self.variant_name(place_ty, variant_idx.as_u32());
                format!("discriminant({place_text}) = {variant_name};")
            }
            StatementKind::Intrinsic(NonDivergingIntrinsic::Assume(operand)) => {
                let bool_ty = self.find_or_zero_ty(&Ty::Bool);
                format!("assume({});", self.render_operand(operand, body, bool_ty))
            }
            StatementKind::Intrinsic(NonDivergingIntrinsic::CopyNonOverlapping(
                CopyNonOverlapping { src, dst, count },
            )) => {
                let unit_ty = self.find_or_zero_ty(&Ty::Tuple {
                    fields: Vec::new(),
                    layout: crate::layout::UNIT_LAYOUT,
                });
                let src_ty = self.find_or_zero_ty(&Ty::Ptr(unit_ty, Mutability::Not));
                let dst_ty = self.find_or_zero_ty(&Ty::Ptr(unit_ty, Mutability::Mut));
                let count_ty = self.find_or_zero_ty(&Ty::Uint(UintKind::Usize));
                format!(
                    "copy_nonoverlapping({}, {}, {});",
                    self.render_operand(src, body, src_ty),
                    self.render_operand(dst, body, dst_ty),
                    self.render_operand(count, body, count_ty),
                )
            }
        };
        self.maybe_with_span_comment(head, stmt.span)
    }

    /// Renders a `Terminator` per `pretty-printer.md` §7.5.
    ///
    /// Output: `<render_terminator_kind(...)> -> [<successor_labels>]`.
    /// No trailing `;` (the composer in §7.10 appends it). When
    /// `include_spans == true` and the terminator span is not
    /// synthesized, a trailing `// <render_span(terminator.span)>`
    /// comment is emitted after the closing `]` per §4.2 / §4.3.
    pub fn render_terminator(&self, term: &Terminator, body: &Body) -> String {
        let head = self.render_terminator_kind(&term.kind, body);
        let successors = match &term.kind {
            TerminatorKind::Goto(block) => format!("bb{}", block.index()),
            TerminatorKind::SwitchInt { targets, .. } => self.render_switch_targets(targets, body),
            TerminatorKind::Return | TerminatorKind::Unreachable => String::new(),
            TerminatorKind::Call { target, .. } => match target {
                Some(block) => format!("return: bb{}", block.index()),
                None => String::new(),
            },
            TerminatorKind::Assert { target, .. } => format!("success: bb{}", target.index()),
        };
        let line = format!("{head} -> [{successors}]");
        self.maybe_with_span_comment(line, term.span)
    }

    /// Renders the head text of a `TerminatorKind` per
    /// `pretty-printer.md` §7.6 (internal helper). The successor
    /// labels are appended by `render_terminator`.
    fn render_terminator_kind(&self, kind: &TerminatorKind, body: &Body) -> String {
        match kind {
            TerminatorKind::Goto(_) => "goto".to_owned(),
            TerminatorKind::SwitchInt { discr, .. } => {
                let discr_ty = self.operand_source_ty_or_unit(discr, body);
                format!("switchInt({})", self.render_operand(discr, body, discr_ty))
            }
            TerminatorKind::Return => "return".to_owned(),
            TerminatorKind::Unreachable => "unreachable".to_owned(),
            TerminatorKind::Call {
                callee,
                args,
                destination,
                ..
            } => {
                let dest_text = self.render_place(destination, body);
                let callee_text = self.render_call_callee(callee, body);
                let arg_tys = self.call_arg_tys(callee, body);
                let arg_texts: Vec<String> = args
                    .iter()
                    .enumerate()
                    .map(|(i, arg)| {
                        let arg_ty = arg_tys
                            .get(i)
                            .copied()
                            .unwrap_or_else(|| self.operand_source_ty_or_unit(arg, body));
                        self.render_operand(arg, body, arg_ty)
                    })
                    .collect();
                format!("{dest_text} = {callee_text}({})", arg_texts.join(", "))
            }
            TerminatorKind::Assert {
                cond,
                expected,
                msg,
                ..
            } => {
                let bool_ty = self.find_or_zero_ty(&Ty::Bool);
                let cond_text = self.render_operand(cond, body, bool_ty);
                let msg_text = self.render_assert_kind(msg, body);
                format!(
                    "assert({}, {cond_text}, {msg_text})",
                    if *expected { "true" } else { "false" }
                )
            }
        }
    }

    /// Renders a `CallCallee` per `pretty-printer.md` §7.7.
    ///
    /// `Direct(id)` resolves the matching `Instance` and emits its
    /// `def_path` + generic args. `Indirect(operand)` defers to
    /// `render_operand` with the function-pointer type `Ty::FnPtr(sig_id)`
    /// as the expected context, per §10.1.8.
    pub fn render_call_callee(&self, callee: &CallCallee, body: &Body) -> String {
        match callee {
            CallCallee::Direct(id) => {
                let instance = &self.model.instances[*id];
                let path = self.render_def_path(instance.item);
                let args = self.render_generic_args(&instance.generic_args, instance.item);
                format!("{path}{args}")
            }
            CallCallee::Indirect(operand) => {
                // §7.7 expected_ty = `Ty::FnPtr(sig_id)`. Recovery via
                // operand inspection: for Operand::Copy/Move(place),
                // place_ty already resolves to a `Ty::FnPtr(_)` at a
                // well-formed call site. Fall back to the recovered
                // type unchanged (preserving FnPtr) or unit when
                // recovery is `NotApplicable`/`Malformed`.
                let fn_ptr_ty = self.operand_source_ty_or_unit(operand, body);
                self.render_operand(operand, body, fn_ptr_ty)
            }
        }
    }

    /// Renders the successor-label content of a `SwitchInt` per
    /// `pretty-printer.md` §7.8.
    ///
    /// Output form: `<v0>: bb<N0>, <v1>: bb<N1>, ..., otherwise: bb<N>`.
    /// The enclosing `[ ... ]` brackets are added by
    /// `render_terminator`, not by this helper.
    pub fn render_switch_targets(&self, targets: &SwitchTargets, _body: &Body) -> String {
        let mut parts: Vec<String> = targets
            .cases
            .iter()
            .map(|(value, block)| format!("{value}: bb{}", block.index()))
            .collect();
        parts.push(format!("otherwise: bb{}", targets.otherwise.index()));
        parts.join(", ")
    }

    /// Renders an `AssertKind` per `pretty-printer.md` §7.9.
    ///
    /// Each variant emits a fixed template-message string. Operand
    /// placeholders render via `render_operand` with the
    /// per-placeholder `expected_ty` derivation from §7.9.
    pub fn render_assert_kind(&self, msg: &AssertKind, body: &Body) -> String {
        let usize_ty = self.find_or_zero_ty(&Ty::Uint(UintKind::Usize));
        match msg {
            AssertKind::BoundsCheck { len, index } => {
                format!(
                    "index out of bounds: the length is {} but the index is {}",
                    self.render_operand(len, body, usize_ty),
                    self.render_operand(index, body, usize_ty),
                )
            }
            AssertKind::Overflow(op, l, r) => {
                let input_ty = self.operand_source_ty_or_unit(l, body);
                let shift_amount_ty = self.operand_source_ty_or_unit(r, body);
                match op {
                    BinOp::Add => format!(
                        "attempt to compute `{} + {}`, which would overflow",
                        self.render_operand(l, body, input_ty),
                        self.render_operand(r, body, input_ty),
                    ),
                    BinOp::Sub => format!(
                        "attempt to compute `{} - {}`, which would overflow",
                        self.render_operand(l, body, input_ty),
                        self.render_operand(r, body, input_ty),
                    ),
                    BinOp::Mul => format!(
                        "attempt to compute `{} * {}`, which would overflow",
                        self.render_operand(l, body, input_ty),
                        self.render_operand(r, body, input_ty),
                    ),
                    BinOp::Div => format!(
                        "attempt to compute `{} / {}`, which would overflow",
                        self.render_operand(l, body, input_ty),
                        self.render_operand(r, body, input_ty),
                    ),
                    BinOp::Rem => format!(
                        "attempt to compute the remainder of `{} % {}`, which would overflow",
                        self.render_operand(l, body, input_ty),
                        self.render_operand(r, body, input_ty),
                    ),
                    BinOp::Shl => format!(
                        "attempt to shift left by `{}`, which would overflow",
                        self.render_operand(r, body, shift_amount_ty),
                    ),
                    BinOp::Shr => format!(
                        "attempt to shift right by `{}`, which would overflow",
                        self.render_operand(r, body, shift_amount_ty),
                    ),
                    _ => {
                        // Per spec §7.9 only Add/Sub/Mul/Div/Rem/Shl/Shr
                        // produce Overflow asserts; other BinOps are not
                        // expected here. Fall back to a debug-style form.
                        format!(
                            "attempt to compute `{} {} {}`, which would overflow",
                            self.render_operand(l, body, input_ty),
                            bin_op_glyph(op),
                            self.render_operand(r, body, input_ty),
                        )
                    }
                }
            }
            AssertKind::OverflowNeg(operand) => {
                let ty = self.operand_source_ty_or_unit(operand, body);
                format!(
                    "attempt to negate `{}`, which would overflow",
                    self.render_operand(operand, body, ty),
                )
            }
            AssertKind::DivisionByZero(operand) => {
                let ty = self.operand_source_ty_or_unit(operand, body);
                format!(
                    "attempt to divide `{}` by zero",
                    self.render_operand(operand, body, ty),
                )
            }
            AssertKind::RemainderByZero(operand) => {
                let ty = self.operand_source_ty_or_unit(operand, body);
                format!(
                    "attempt to calculate the remainder of `{}` with a divisor of zero",
                    self.render_operand(operand, body, ty),
                )
            }
            AssertKind::MisalignedPointerDereference { required, found } => {
                format!(
                    "misaligned pointer dereference: address must be a multiple of {} but is {}",
                    self.render_operand(required, body, usize_ty),
                    self.render_operand(found, body, usize_ty),
                )
            }
            AssertKind::NullPointerDereference => "null pointer dereference occurred".to_owned(),
            AssertKind::InvalidEnumConstruction(found) => {
                let ty = self.operand_source_ty_or_unit(found, body);
                format!(
                    "invalid enum discriminant value: {}",
                    self.render_operand(found, body, ty),
                )
            }
        }
    }

    /// Renders a `BasicBlock` per `pretty-printer.md` §7.10.
    ///
    /// Composes the `bb<N>: { ... }` envelope: header line at the
    /// caller's envelope indent (4 spaces), each statement and the
    /// terminator at 8-space indent, and the trailing `;` after the
    /// terminator. The block label `bb<N>` resolves from the
    /// `block`'s positional identity inside `body.basic_blocks`;
    /// callers must pass a reference into the body's table
    /// (the function-body envelope in P1a-PP-05a does, by iterating
    /// `body.basic_blocks` with positional enumeration).
    pub fn render_basic_block(&self, block: &BasicBlock, body: &Body) -> String {
        let envelope_indent = "    ";
        let inner_indent = "        ";
        // Locate the block's BlockId by pointer identity within
        // body.basic_blocks. The §7.10 sigil requires the positional
        // index, and the signature does not carry it explicitly per
        // §8 ownership table.
        let block_index = body
            .basic_blocks
            .iter()
            .enumerate()
            .find_map(|(i, b)| {
                if std::ptr::eq(b, block) {
                    Some(i as u32)
                } else {
                    None
                }
            })
            .expect("render_basic_block: `block` must be a reference into `body.basic_blocks`");
        let mut out = format!("{envelope_indent}bb{block_index}: {{\n");
        for stmt in block.statements.iter() {
            out.push_str(inner_indent);
            out.push_str(&self.render_statement(stmt, body));
            out.push('\n');
        }
        out.push_str(inner_indent);
        out.push_str(&self.render_terminator(&block.terminator, body));
        out.push_str(";\n");
        out.push_str(envelope_indent);
        out.push('}');
        out
    }

    // ------------------------------------------------------------------
    // Internal helpers.
    // ------------------------------------------------------------------

    /// Renders an aggregate constructor per §7.3.2.
    fn render_aggregate(&self, kind: &AggregateKind, operands: &[Operand], body: &Body) -> String {
        match kind {
            AggregateKind::Array(elem_ty) => {
                let parts: Vec<String> = operands
                    .iter()
                    .map(|op| self.render_operand(op, body, *elem_ty))
                    .collect();
                format!("[{}]", parts.join(", "))
            }
            AggregateKind::Tuple(tuple_ty) => {
                let field_tys: Vec<TyId> = match self.model.types.resolve(*tuple_ty) {
                    Ty::Tuple { fields, .. } => fields.clone(),
                    _ => Vec::new(),
                };
                match operands.len() {
                    0 => "()".to_owned(),
                    1 => {
                        let ty = field_tys.first().copied().unwrap_or(*tuple_ty);
                        format!("({},)", self.render_operand(&operands[0], body, ty))
                    }
                    _ => {
                        let parts: Vec<String> = operands
                            .iter()
                            .enumerate()
                            .map(|(i, op)| {
                                let ty = field_tys.get(i).copied().unwrap_or(*tuple_ty);
                                self.render_operand(op, body, ty)
                            })
                            .collect();
                        format!("({})", parts.join(", "))
                    }
                }
            }
            AggregateKind::Struct(struct_ty) => {
                let (item, args, field_layouts) = match self.model.types.resolve(*struct_ty) {
                    Ty::Struct { item, args, layout } => match &layout.kind {
                        LayoutKind::Struct { fields } => (*item, args.clone(), fields.clone()),
                        _ => {
                            panic!("render_aggregate: AggregateKind::Struct expected Struct layout")
                        }
                    },
                    _ => panic!("render_aggregate: AggregateKind::Struct expected Ty::Struct"),
                };
                let path = self.render_def_path(item);
                let args_text = self.render_generic_args(&args, item);
                let is_tuple_variant = self.layout_fields_are_tuple_style(&field_layouts);
                let parts: Vec<String> = operands
                    .iter()
                    .enumerate()
                    .map(|(i, op)| {
                        let ty = field_layouts.get(i).map(|f| f.ty).unwrap_or(*struct_ty);
                        let value = self.render_operand(op, body, ty);
                        if is_tuple_variant {
                            value
                        } else {
                            let name_id = field_layouts[i].name;
                            let name = self.model.strings.resolve(name_id);
                            format!("{name}: {value}")
                        }
                    })
                    .collect();
                if is_tuple_variant {
                    format!("{path}{args_text}({})", parts.join(", "))
                } else {
                    format!("{path}{args_text} {{ {} }}", parts.join(", "))
                }
            }
            AggregateKind::Enum(enum_ty, variant_idx) => {
                let (item, args, variant_layout) = match self.model.types.resolve(*enum_ty) {
                    Ty::Enum { item, args, layout } => match &layout.kind {
                        LayoutKind::Enum { shape } => {
                            let variant = match shape {
                                EnumShape::Empty => panic!(
                                    "render_aggregate: AggregateKind::Enum on EnumShape::Empty"
                                ),
                                EnumShape::Single { variant, .. } => variant.clone(),
                                EnumShape::Multiple { variants, .. } => {
                                    variants[variant_idx.as_u32() as usize].clone()
                                }
                            };
                            (*item, args.clone(), variant)
                        }
                        _ => panic!("render_aggregate: AggregateKind::Enum expected Enum layout"),
                    },
                    _ => panic!("render_aggregate: AggregateKind::Enum expected Ty::Enum"),
                };
                let path = self.render_def_path(item);
                let args_text = self.render_generic_args(&args, item);
                let variant_name = self.model.strings.resolve(variant_layout.name).to_owned();
                let fields = &variant_layout.fields;
                if fields.is_empty() {
                    format!("{path}{args_text}::{variant_name}")
                } else {
                    let is_tuple_variant = self.layout_fields_are_tuple_style(fields);
                    let parts: Vec<String> = operands
                        .iter()
                        .enumerate()
                        .map(|(i, op)| {
                            let ty = fields.get(i).map(|f| f.ty).unwrap_or(*enum_ty);
                            let value = self.render_operand(op, body, ty);
                            if is_tuple_variant {
                                value
                            } else {
                                let name_id = fields[i].name;
                                let name = self.model.strings.resolve(name_id);
                                format!("{name}: {value}")
                            }
                        })
                        .collect();
                    if is_tuple_variant {
                        format!("{path}{args_text}::{variant_name}({})", parts.join(", "))
                    } else {
                        format!(
                            "{path}{args_text}::{variant_name} {{ {} }}",
                            parts.join(", ")
                        )
                    }
                }
            }
            AggregateKind::Union(union_ty, field_idx) => {
                let (item, args, field_layouts) = match self.model.types.resolve(*union_ty) {
                    Ty::Union { item, args, layout } => match &layout.kind {
                        LayoutKind::Union { fields } => (*item, args.clone(), fields.clone()),
                        _ => panic!("render_aggregate: AggregateKind::Union expected Union layout"),
                    },
                    _ => panic!("render_aggregate: AggregateKind::Union expected Ty::Union"),
                };
                let path = self.render_def_path(item);
                let args_text = self.render_generic_args(&args, item);
                let active = &field_layouts[field_idx.as_u32() as usize];
                let name = self.model.strings.resolve(active.name);
                let value = self.render_operand(&operands[0], body, active.ty);
                format!("{path}{args_text} {{ {name}: {value} }}")
            }
            AggregateKind::Closure(closure_ty) => {
                let head = self.render_ty(*closure_ty);
                let (captures_item, captures_args, field_layouts) =
                    match self.model.types.resolve(*closure_ty) {
                        Ty::Closure(item, args) => {
                            // Find the parallel ClosureCaptures struct
                            // per spec.md §13.1.
                            let captures_id = self
                                .model
                                .types
                                .find_nominal(NominalKind::Struct, *item, args)
                                .unwrap_or_else(|| {
                                    panic!(
                                        "render_aggregate: AggregateKind::Closure has no parallel \
                                     captures-struct nominal entry"
                                    )
                                });
                            match self.model.types.resolve(captures_id) {
                                Ty::Struct { item, args, layout } => match &layout.kind {
                                    LayoutKind::Struct { fields } => {
                                        (*item, args.clone(), fields.clone())
                                    }
                                    _ => panic!(
                                        "render_aggregate: closure captures struct has \
                                         non-Struct layout"
                                    ),
                                },
                                _ => panic!(
                                    "render_aggregate: closure captures resolved to non-Struct"
                                ),
                            }
                        }
                        _ => {
                            panic!("render_aggregate: AggregateKind::Closure expected Ty::Closure")
                        }
                    };
                let _ = (captures_item, captures_args);
                let parts: Vec<String> = operands
                    .iter()
                    .enumerate()
                    .map(|(i, op)| {
                        let ty = field_layouts.get(i).map(|f| f.ty).unwrap_or(*closure_ty);
                        let name_id = field_layouts[i].name;
                        let name = self.model.strings.resolve(name_id);
                        format!("{name}: {}", self.render_operand(op, body, ty))
                    })
                    .collect();
                format!("{head} {{ {} }}", parts.join(", "))
            }
            AggregateKind::RawPtr(pointee_ty, mutability) => {
                let addr_ty = self.find_or_zero_ty(&Ty::Ptr(*pointee_ty, *mutability));
                let meta_ty = self.raw_ptr_metadata_ty(*pointee_ty);
                let addr_text = self.render_operand(&operands[0], body, addr_ty);
                let meta_text = if operands.len() >= 2 {
                    self.render_operand(&operands[1], body, meta_ty)
                } else {
                    // Schema requires two operands; the metadata
                    // position is mandatory. If missing, omit it
                    // gracefully — validation rejects this elsewhere.
                    String::from("()")
                };
                let pointee_text = self.render_ty(*pointee_ty);
                let prefix = match mutability {
                    Mutability::Not => "*const",
                    Mutability::Mut => "*mut",
                };
                format!("({addr_text}, {meta_text}) as {prefix} {pointee_text}")
            }
        }
    }

    /// Narrows an `Array(elem, _)` / `Slice(elem)` predecessor to its
    /// element type per the §19.4 narrowing table for `Index` /
    /// `ConstantIndex` projections. Returns `None` if `ty` is neither
    /// `Array` nor `Slice` (a malformed projection at this point; the
    /// caller preserves the current type so downstream rendering does
    /// not panic).
    fn narrow_indexable_elem(&self, ty: TyId) -> Option<TyId> {
        match self.model.types.resolve(ty) {
            Ty::Array(elem, _) | Ty::Slice(elem) => Some(*elem),
            _ => None,
        }
    }

    /// Returns the metadata operand's expected type for the
    /// `AggregateKind::RawPtr` arm: unit `()` for thin pointers,
    /// `Ty::Uint(Usize)` for fat pointers to `[T]` / `str`, and the
    /// pointer-to-vtable type for `dyn` trait objects.
    fn raw_ptr_metadata_ty(&self, pointee_ty: TyId) -> TyId {
        let pointee = self.model.types.resolve(pointee_ty);
        match pointee {
            Ty::Slice(_) | Ty::Str => self.find_or_zero_ty(&Ty::Uint(UintKind::Usize)),
            Ty::Dynamic { vtable_ty, .. } => {
                let vtable_ty = *vtable_ty;
                self.find_or_zero_ty(&Ty::Ptr(vtable_ty, Mutability::Not))
            }
            _ => self.find_or_zero_ty(&Ty::Tuple {
                fields: Vec::new(),
                layout: crate::layout::UNIT_LAYOUT,
            }),
        }
    }

    /// Returns the per-arg expected `TyId` list for a Call's callee.
    ///
    /// `Direct(id)` resolves to `model.instances[id]`'s Function
    /// `fn_sig.inputs`. `Indirect(operand)` resolves the operand's
    /// `Ty::FnPtr(sig_id)` and reads the signature's `inputs` from the
    /// `FnSigInterner`, per §7.6 / §10.1.8.
    fn call_arg_tys(&self, callee: &CallCallee, body: &Body) -> Vec<TyId> {
        match callee {
            CallCallee::Direct(id) => {
                let instance = &self.model.instances[*id];
                match &instance.kind {
                    InstanceKind::Function(data) => {
                        let sig = self.model.sigs.resolve(data.fn_sig);
                        sig.inputs.clone()
                    }
                    _ => Vec::new(),
                }
            }
            CallCallee::Indirect(operand) => {
                let op_ty = match operand_ty(self.model, body, operand) {
                    TypeRecovery::Recovered(ty) => ty,
                    _ => return Vec::new(),
                };
                match self.model.types.resolve(op_ty) {
                    Ty::FnPtr(sig_id) => {
                        let sig = self.model.sigs.resolve(*sig_id);
                        sig.inputs.clone()
                    }
                    _ => Vec::new(),
                }
            }
        }
    }

    /// Resolves a field name from a layout-bearing predecessor type.
    ///
    /// Panics if the layout shape does not match the §3.4 rule (e.g.,
    /// missing field index). Spec contract: all field-name fields are
    /// non-optional per `spec.md` §11; missing-name is invalid IR.
    fn field_name(&self, ty: TyId, active_variant: Option<u32>, idx: u32) -> String {
        let resolved = self.model.types.resolve(ty);
        let field_name_id = match resolved {
            Ty::Struct { layout, .. } | Ty::Tuple { layout, .. } => match &layout.kind {
                LayoutKind::Struct { fields } => fields[idx as usize].name,
                _ => panic!("field_name: Struct/Tuple predecessor must carry LayoutKind::Struct"),
            },
            Ty::Union { layout, .. } => match &layout.kind {
                LayoutKind::Union { fields } => fields[idx as usize].name,
                _ => panic!("field_name: Union predecessor must carry LayoutKind::Union"),
            },
            Ty::Closure(item, args) => {
                let captures_id = self
                    .model
                    .types
                    .find_nominal(NominalKind::Struct, *item, args)
                    .unwrap_or_else(|| {
                        panic!("field_name: Closure predecessor has no parallel captures struct")
                    });
                match self.model.types.resolve(captures_id) {
                    Ty::Struct { layout, .. } => match &layout.kind {
                        LayoutKind::Struct { fields } => fields[idx as usize].name,
                        _ => panic!("field_name: closure captures must carry LayoutKind::Struct"),
                    },
                    _ => panic!("field_name: closure captures resolved to non-Struct"),
                }
            }
            Ty::Enum { layout, .. } => match &layout.kind {
                LayoutKind::Enum { shape } => match shape {
                    EnumShape::Empty => {
                        panic!("field_name: cannot Field-project into EnumShape::Empty")
                    }
                    EnumShape::Single { variant, .. } => variant.fields[idx as usize].name,
                    EnumShape::Multiple { variants, .. } => {
                        let v = active_variant.unwrap_or_else(|| {
                            panic!("field_name: Enum Field projection without active variant")
                        });
                        variants[v as usize].fields[idx as usize].name
                    }
                },
                _ => panic!("field_name: Enum predecessor must carry LayoutKind::Enum"),
            },
            other => panic!(
                "field_name: unexpected predecessor type {:?} for Field projection",
                other
            ),
        };
        self.model.strings.resolve(field_name_id).to_owned()
    }

    /// Resolves a variant name from a layout-bearing `Ty::Enum`
    /// predecessor type.
    fn variant_name(&self, ty: TyId, variant_idx: u32) -> String {
        let resolved = self.model.types.resolve(ty);
        let name_id = match resolved {
            Ty::Enum { layout, .. } => match &layout.kind {
                LayoutKind::Enum { shape } => match shape {
                    EnumShape::Empty => {
                        panic!("variant_name: cannot resolve variant on EnumShape::Empty")
                    }
                    EnumShape::Single { variant, .. } => variant.name,
                    EnumShape::Multiple { variants, .. } => variants[variant_idx as usize].name,
                },
                _ => panic!("variant_name: Enum predecessor must carry LayoutKind::Enum"),
            },
            other => panic!(
                "variant_name: unexpected predecessor type {:?} for Downcast / variant lookup",
                other
            ),
        };
        self.model.strings.resolve(name_id).to_owned()
    }

    /// Best-effort source-type recovery for an operand at a use site
    /// in `body`. Returns a fallback `TyId` (the same as the supplied
    /// `fallback`) when recovery is `NotApplicable` or `Malformed`;
    /// the type-context downstream consumer (e.g.,
    /// `render_const(id, expected_ty)`) accepts the supplied
    /// fallback per pair-form D-14-4.9.
    fn operand_source_ty(&self, op: &Operand, body: &Body, fallback: TyId) -> TyId {
        match operand_ty(self.model, body, op) {
            TypeRecovery::Recovered(ty) => ty,
            _ => fallback,
        }
    }

    /// Variant of `operand_source_ty` that falls back to a freshly
    /// resolved unit type when recovery fails.
    fn operand_source_ty_or_unit(&self, op: &Operand, body: &Body) -> TyId {
        let unit_ty = self.find_or_zero_ty(&Ty::Tuple {
            fields: Vec::new(),
            layout: crate::layout::UNIT_LAYOUT,
        });
        self.operand_source_ty(op, body, unit_ty)
    }

    /// Resolves a place's type via the body_ty walk; on
    /// `NotApplicable` / `Malformed`, returns the supplied fallback.
    fn place_ty_or(&self, place: &Place, body: &Body, fallback: TyId) -> TyId {
        match place_ty(self.model, body, place) {
            TypeRecovery::Recovered(ty) => ty,
            _ => fallback,
        }
    }

    /// Looks up an interned `Ty`; if absent, returns `TyId::ZERO` as
    /// a non-panicking fallback for type-context threading at sites
    /// where the consumer (render_const) ignores or tolerates the
    /// fallback. The actual rendered output never embeds the
    /// fallback type as text.
    fn find_or_zero_ty(&self, ty: &Ty) -> TyId {
        self.model.types.find(ty).unwrap_or(TyId::ZERO)
    }

    /// Returns `true` when every field name in the layout matches
    /// the rustc tuple-struct convention `"0"`, `"1"`, ..., `"n-1"`.
    /// This is the §7.3.2 tuple-variant / struct-variant dispatch
    /// test: tuple-variant structs (`struct Foo(u32, u32)`) carry
    /// positional digit-only field names; struct-variant structs
    /// carry source-level field names.
    fn layout_fields_are_tuple_style(&self, fields: &[crate::layout::FieldLayout]) -> bool {
        if fields.is_empty() {
            return false;
        }
        for (i, field) in fields.iter().enumerate() {
            let name = self.model.strings.resolve(field.name);
            if name != i.to_string() {
                return false;
            }
        }
        true
    }

    /// Emits a trailing `// <render_span(span)>` comment aligned to
    /// column `ALIGN` per §4.2 when `include_spans == true` and the
    /// span is not synthesized per §4.3.
    fn maybe_with_span_comment(&self, line: String, span: Span) -> String {
        if !self.options.include_spans {
            return line;
        }
        if span.file == SourceFileId::SYNTHESIZED {
            return line;
        }
        let span_text = self.render_span(span);
        format!("{line:ALIGN$} // {span_text}")
    }
}

/// Returns the CastKind annotation per §7.3.1, or `None` when the
/// source / target types alone are unambiguous.
fn cast_kind_annotation(kind: &CastKind) -> Option<String> {
    match kind {
        CastKind::PointerCoercion(PointerCoercion::ReifyFnPointer) => {
            Some("PointerCoercion(ReifyFnPointer)".to_owned())
        }
        CastKind::PointerCoercion(PointerCoercion::UnsafeFnPointer) => {
            Some("PointerCoercion(UnsafeFnPointer)".to_owned())
        }
        CastKind::PointerCoercion(PointerCoercion::ClosureFnPointer) => {
            Some("PointerCoercion(ClosureFnPointer)".to_owned())
        }
        CastKind::PointerCoercion(PointerCoercion::Unsize) => {
            Some("PointerCoercion(Unsize)".to_owned())
        }
        CastKind::Subtype => Some("Subtype".to_owned()),
        _ => None,
    }
}

/// Returns the rendering glyph for a `BinOp`. Source-level BinOps
/// use Rust infix operators; rustc-only checked / unchecked variants
/// render as their identifier form (`AddUnchecked`,
/// `AddWithOverflow`, `Cmp`, `Offset`, etc.).
fn bin_op_glyph(op: &BinOp) -> &'static str {
    match op {
        BinOp::Add => "+",
        BinOp::Sub => "-",
        BinOp::Mul => "*",
        BinOp::Div => "/",
        BinOp::Rem => "%",
        BinOp::AddUnchecked => "AddUnchecked",
        BinOp::SubUnchecked => "SubUnchecked",
        BinOp::MulUnchecked => "MulUnchecked",
        BinOp::AddWithOverflow => "AddWithOverflow",
        BinOp::SubWithOverflow => "SubWithOverflow",
        BinOp::MulWithOverflow => "MulWithOverflow",
        BinOp::BitXor => "^",
        BinOp::BitAnd => "&",
        BinOp::BitOr => "|",
        BinOp::Shl => "<<",
        BinOp::ShlUnchecked => "ShlUnchecked",
        BinOp::Shr => ">>",
        BinOp::ShrUnchecked => "ShrUnchecked",
        BinOp::Eq => "==",
        BinOp::Lt => "<",
        BinOp::Le => "<=",
        BinOp::Ne => "!=",
        BinOp::Ge => ">=",
        BinOp::Gt => ">",
        BinOp::Cmp => "Cmp",
        BinOp::Offset => "Offset",
    }
}

/// Returns the rendering glyph for a `UnOp`. `Not` and `Neg` use
/// the source-level prefix operators; `PtrMetadata` renders as the
/// identifier form (rustc has no source-level glyph for it).
fn un_op_glyph(op: &UnOp) -> &'static str {
    match op {
        UnOp::Not => "!",
        UnOp::Neg => "-",
        UnOp::PtrMetadata => "PtrMetadata",
    }
}

/// Renders a `Const::Bool` per §10.1.1 (no suffix).
fn render_const_bool(value: bool) -> String {
    if value { "true" } else { "false" }.to_owned()
}

/// Renders a `Const::Char` per §10.1.1: the `u32` payload is
/// reconstructed into a `char` (bridge has validated validity per
/// `spec.md` §9.7) and emitted via the Rust `char::Debug` form.
fn render_const_char(code_point: u32) -> String {
    let ch = char::from_u32(code_point).unwrap_or_else(|| {
        panic!(
            "render_const: Const::Char({code_point}) is not a valid Unicode scalar; \
             the bridge must reject invalid code points per spec.md §9.7"
        )
    });
    format!("{ch:?}")
}

/// Returns the type-name suffix used by the underscore-decimal form
/// of `Int` (e.g., `i32` in `42_i32`).
fn int_kind_name(kind: IntKind) -> &'static str {
    match kind {
        IntKind::I8 => "i8",
        IntKind::I16 => "i16",
        IntKind::I32 => "i32",
        IntKind::I64 => "i64",
        IntKind::I128 => "i128",
        IntKind::Isize => "isize",
    }
}

/// Returns the type-name suffix used by the underscore-decimal form
/// of `Uint` (e.g., `u8` in `255_u8`).
fn uint_kind_name(kind: UintKind) -> &'static str {
    match kind {
        UintKind::U8 => "u8",
        UintKind::U16 => "u16",
        UintKind::U32 => "u32",
        UintKind::U64 => "u64",
        UintKind::U128 => "u128",
        UintKind::Usize => "usize",
    }
}

/// Re-interprets the low `width` bits of `bits` as a signed two's-
/// complement integer per `pretty-printer.md:2764-2772`.
///
/// For `width < 128`, sign-extends from the width's MSB. For
/// `width == 128`, `bits as i128` is the identity decoding.
fn decode_int(width: u32, bits: u128) -> i128 {
    debug_assert!(width > 0 && width <= 128);
    if width == 128 {
        bits as i128
    } else {
        let shift = 128 - width;
        ((bits as i128) << shift) >> shift
    }
}

/// Masks `bits` to the low `width` bits per the unsigned decoding
/// rule at `pretty-printer.md:2773-2776`.
fn decode_uint(width: u32, bits: u128) -> u128 {
    debug_assert!(width > 0 && width <= 128);
    if width == 128 {
        bits
    } else {
        bits & ((1u128 << width) - 1)
    }
}

/// Computes the canonical `Uint` MAX value at `width` bits.
fn uint_max(width: u32) -> u128 {
    debug_assert!(width > 0 && width <= 128);
    if width == 128 {
        u128::MAX
    } else {
        (1u128 << width) - 1
    }
}

/// Computes the canonical `Int` MIN value at `width` bits.
fn int_min(width: u32) -> i128 {
    debug_assert!(width > 0 && width <= 128);
    if width == 128 {
        i128::MIN
    } else {
        -(1i128 << (width - 1))
    }
}

/// Computes the canonical `Int` MAX value at `width` bits.
fn int_max(width: u32) -> i128 {
    debug_assert!(width > 0 && width <= 128);
    if width == 128 {
        i128::MAX
    } else {
        (1i128 << (width - 1)) - 1
    }
}

/// D-14-4.10 sentinel-collapsing test for `Int`. Returns the
/// symbolic label (`i32::MIN`, `i32::MAX`, ...) when `decoded`
/// equals the `width`-bound MIN or MAX, else `None`.
fn int_sentinel_label(kind: IntKind, width: u32, decoded: i128) -> Option<&'static str> {
    if decoded == int_min(width) {
        Some(match kind {
            IntKind::I8 => "i8::MIN",
            IntKind::I16 => "i16::MIN",
            IntKind::I32 => "i32::MIN",
            IntKind::I64 => "i64::MIN",
            IntKind::I128 => "i128::MIN",
            IntKind::Isize => "isize::MIN",
        })
    } else if decoded == int_max(width) {
        Some(match kind {
            IntKind::I8 => "i8::MAX",
            IntKind::I16 => "i16::MAX",
            IntKind::I32 => "i32::MAX",
            IntKind::I64 => "i64::MAX",
            IntKind::I128 => "i128::MAX",
            IntKind::Isize => "isize::MAX",
        })
    } else {
        None
    }
}

/// Renders a `Const::Float` per §10.1.1 plus the D-14-4.12 non-finite
/// handling rule.
///
/// Reconstitutes the IEEE-754 value at the variant's `FloatKind`
/// width, classifies it (finite / +inf / -inf / NaN), and emits the
/// rustc-mirrored form. NaN bit-pattern variations collapse to the
/// single `NaN_f<width>` form.
fn render_const_float(kind: FloatKind, bits: u128) -> String {
    let label = float_kind_label(kind);
    match classify_float(kind, bits) {
        FloatClass::PositiveInfinity => format!("inf_{label}"),
        FloatClass::NegativeInfinity => format!("-inf_{label}"),
        FloatClass::NaN => format!("NaN_{label}"),
        FloatClass::Finite => render_finite_float(kind, bits, label),
    }
}

/// Float classification per D-14-4.12.
///
/// Inspects the IEEE-754 exponent / mantissa bit fields directly
/// (rather than relying on a native Rust type) so F16 and F128 are
/// classified faithfully even though the Rust language pin has no
/// stable native types for them.
enum FloatClass {
    Finite,
    PositiveInfinity,
    NegativeInfinity,
    NaN,
}

fn classify_float(kind: FloatKind, bits: u128) -> FloatClass {
    let (mantissa_bits, exponent_bits) = float_layout(kind);
    let total = mantissa_bits + exponent_bits + 1;
    let bits_mask = if total == 128 {
        u128::MAX
    } else {
        (1u128 << total) - 1
    };
    let bits = bits & bits_mask;
    let exp_mask: u128 = (1u128 << exponent_bits) - 1;
    let mantissa_mask: u128 = (1u128 << mantissa_bits) - 1;
    let sign = (bits >> (mantissa_bits + exponent_bits)) & 1;
    let exp = (bits >> mantissa_bits) & exp_mask;
    let mantissa = bits & mantissa_mask;
    if exp == exp_mask {
        if mantissa == 0 {
            if sign == 0 {
                FloatClass::PositiveInfinity
            } else {
                FloatClass::NegativeInfinity
            }
        } else {
            FloatClass::NaN
        }
    } else {
        FloatClass::Finite
    }
}

fn float_layout(kind: FloatKind) -> (u32, u32) {
    // (mantissa bits, exponent bits)
    match kind {
        FloatKind::F16 => (10, 5),
        FloatKind::F32 => (23, 8),
        FloatKind::F64 => (52, 11),
        FloatKind::F128 => (112, 15),
    }
}

fn float_kind_label(kind: FloatKind) -> &'static str {
    match kind {
        FloatKind::F16 => "f16",
        FloatKind::F32 => "f32",
        FloatKind::F64 => "f64",
        FloatKind::F128 => "f128",
    }
}

/// Renders a finite IEEE-754 value at `kind` width per
/// `pretty-printer.md` §10.1.1's "shortest round-trippable decimal"
/// rule.
///
/// F32 and F64 use native Rust types -- their `Display` already
/// produces the shortest round-trippable decimal. F16 and F128 use
/// `rustc_apfloat`'s `Half` and `Quad` types, mirroring rustc's MIR
/// pretty-printer at `rustc_middle/src/ty/print/pretty.rs:1788-1803`,
/// which is the canonical source for the shortest-round-trippable
/// rule the spec inherits.
///
/// Integer-valued finite floats render without a fractional part
/// (`1f32`, `0f64`, `-0f32`, `42f64`); non-integer finite floats
/// render with one (`1.5f32`, `3.14f64`). Negative zero is finite
/// and renders as `-0f<w>`. Both the native types and apfloat
/// observe this convention.
fn render_finite_float(kind: FloatKind, bits: u128, label: &str) -> String {
    use rustc_apfloat::Float as ApFloat;
    match kind {
        FloatKind::F32 => format!("{}{label}", f32::from_bits(bits as u32)),
        FloatKind::F64 => format!("{}{label}", f64::from_bits(bits as u64)),
        FloatKind::F16 => {
            let val = rustc_apfloat::ieee::Half::from_bits(bits);
            format!("{val}{label}")
        }
        FloatKind::F128 => {
            let val = rustc_apfloat::ieee::Quad::from_bits(bits);
            format!("{val}{label}")
        }
    }
}

#[cfg(test)]
#[path = "pretty_tests.rs"]
mod tests;
