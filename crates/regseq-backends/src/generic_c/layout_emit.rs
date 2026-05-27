//! The section 14 / D10 field-offset aggregate emitter -- the shared
//! primitive (task c2 reuses it for enum payloads).
//!
//! `FieldLayout` vectors are in source / MIR order, not memory order
//! (`repr(Rust)` reorders), so verbatim emission would not reproduce
//! `offset`. This module emits a C `struct` / `union` by the section 14
//! seven-step technique: sort emittable fields by `offset`, omit ZST
//! fields (accounting for them via explicit padding), insert
//! `unsigned char _padN[k]` fillers + a trailing pad to `layout.size`,
//! force the C type alignment to `layout.align` via the GNU
//! `aligned` attribute (plus `packed` when `pack.is_some()`), and emit
//! the three `_Static_assert` guards. The static-asserts make the emitted
//! TU self-checking: a wrong offset / size / alignment fails the compile.

use regseq_model::{Layout, LayoutKind, RegseqModel, Ty, TyId};

use super::naming::sanitize;
use super::types::declare;
use super::NameTable;

/// One emittable (non-ZST accounted) field, normalized across the
/// struct / tuple / union / closure shapes.
struct ViewField {
    /// C field name (sanitized source name, or `_N` for a tuple).
    name: String,
    /// Field type.
    ty: TyId,
    /// Byte offset within the aggregate (`0` for every union field).
    offset: u64,
}

/// A normalized aggregate view: the resolved `Layout` plus the field
/// list with C-level names. Closures resolve through their parallel
/// capture struct (the same lookup `size_align_of` uses).
struct AggView<'a> {
    layout: &'a Layout,
    fields: Vec<ViewField>,
    is_union: bool,
}

/// Resolve the layout + field list for an aggregate `ty`. Returns `None`
/// for an enum (its body is the task-c2 hook) or a non-aggregate.
fn resolve_view<'a>(model: &'a RegseqModel, ty: TyId) -> Option<AggView<'a>> {
    let (layout, is_tuple) = match &model.types[ty] {
        Ty::Struct { layout, .. } | Ty::Union { layout, .. } => (layout, false),
        Ty::Tuple { layout, .. } => (layout, true),
        Ty::Closure(item, args) => {
            // The closure's layout is its parallel capture `Ty::Struct`.
            let cap = model
                .types
                .iter()
                .find_map(|(_, candidate)| match candidate {
                    Ty::Struct {
                        item: s_item,
                        args: s_args,
                        layout,
                    } if s_item == item && s_args == args => Some(layout),
                    _ => None,
                })?;
            (cap, false)
        }
        _ => return None,
    };
    let (raw, is_union) = match &layout.kind {
        LayoutKind::Struct { fields } => (fields, false),
        LayoutKind::Union { fields } => (fields, true),
        LayoutKind::Enum { .. } => return None,
    };
    let fields = raw
        .iter()
        .enumerate()
        .map(|(idx, f)| ViewField {
            name: if is_tuple {
                format!("_{idx}")
            } else {
                sanitize(model.strings.resolve(f.name))
            },
            ty: f.ty,
            offset: f.offset,
        })
        .collect();
    Some(AggView {
        layout,
        fields,
        is_union,
    })
}

/// Whether `ty` is a zero-sized type (omitted from emission per section
/// 14 step 2; PhantomData / unit / ZST all collapse to size 0).
fn is_zst(model: &RegseqModel, ty: TyId) -> bool {
    model.size_align_of(ty).size == 0
}

/// The GNU type-attribute clause forcing the aggregate's C alignment to
/// `layout.align` (section 14 step 4): `aligned(<align>)`, plus `packed`
/// when `pack.is_some()`. The alignment is always `layout.align`, never
/// `pack`'s `n` -- under `repr(packed(n))` the type's true alignment is
/// `min(max_field_align, n) = layout.align`, which the `_Alignof` guard
/// asserts.
fn align_attr(layout: &Layout) -> String {
    if layout.pack.is_some() {
        format!("__attribute__((packed, aligned({})))", layout.align)
    } else {
        format!("__attribute__((aligned({})))", layout.align)
    }
}

/// Emit a `struct` / `union` definition + the section 14 `_Static_assert`
/// layout guards for the aggregate `ty`, or `None` if `ty` is an enum
/// (task-c2 hook) or a non-aggregate.
pub(crate) fn emit_aggregate(
    model: &RegseqModel,
    names: &NameTable,
    ty: TyId,
    agg_name: &str,
) -> Option<String> {
    let view = resolve_view(model, ty)?;
    let tag = if view.is_union { "union" } else { "struct" };
    let mut out = String::new();
    out.push_str(&format!("{tag} {agg_name} {{\n"));

    // Field bodies. Static-assert lines are collected as we go so the
    // per-field offsetof guards land after the definition.
    let mut guards = String::new();
    if view.is_union {
        emit_union_body(model, names, &view, agg_name, &mut out, &mut guards);
    } else {
        emit_struct_body(model, names, &view, agg_name, &mut out, &mut guards);
    }

    out.push_str(&format!("}} {};\n", align_attr(view.layout)));
    out.push_str(&format!(
        "_Static_assert(sizeof({tag} {agg_name}) == {}, \"{agg_name} size\");\n",
        view.layout.size
    ));
    out.push_str(&format!(
        "_Static_assert(_Alignof({tag} {agg_name}) == {}, \"{agg_name} align\");\n",
        view.layout.align
    ));
    out.push_str(&guards);
    Some(out)
}

/// Emit a struct / tuple / closure body: fields sorted by offset with
/// explicit `_padN` fillers + a trailing pad, and per-field offsetof
/// guards.
fn emit_struct_body(
    model: &RegseqModel,
    names: &NameTable,
    view: &AggView<'_>,
    agg_name: &str,
    out: &mut String,
    guards: &mut String,
) {
    let mut emittable: Vec<&ViewField> = view
        .fields
        .iter()
        .filter(|f| !is_zst(model, f.ty))
        .collect();
    emittable.sort_by_key(|f| f.offset);

    let mut cursor: u64 = 0;
    let mut pad_n: usize = 0;
    for f in &emittable {
        if f.offset > cursor {
            out.push_str(&format!(
                "    unsigned char _pad{pad_n}[{}];\n",
                f.offset - cursor
            ));
            pad_n += 1;
        }
        out.push_str(&format!("    {};\n", declare(model, names, f.ty, &f.name)));
        guards.push_str(&format!(
            "_Static_assert(offsetof({struct_kw} {agg_name}, {fname}) == {off}, \"{agg_name}.{fname} offset\");\n",
            struct_kw = "struct",
            fname = f.name,
            off = f.offset,
        ));
        cursor = f.offset + model.size_align_of(f.ty).size;
    }
    if cursor < view.layout.size {
        out.push_str(&format!(
            "    unsigned char _pad{pad_n}[{}];\n",
            view.layout.size - cursor
        ));
    }
}

/// Emit a union body: every non-ZST field at offset 0 (offsetof guards
/// all assert `0`), plus a trailing `unsigned char _pad[size]` member
/// when no real member already spans `layout.size` (pins `sizeof`).
fn emit_union_body(
    model: &RegseqModel,
    names: &NameTable,
    view: &AggView<'_>,
    agg_name: &str,
    out: &mut String,
    guards: &mut String,
) {
    let mut largest: u64 = 0;
    for f in view.fields.iter().filter(|f| !is_zst(model, f.ty)) {
        out.push_str(&format!("    {};\n", declare(model, names, f.ty, &f.name)));
        guards.push_str(&format!(
            "_Static_assert(offsetof(union {agg_name}, {fname}) == 0, \"{agg_name}.{fname} offset\");\n",
            fname = f.name,
        ));
        largest = largest.max(model.size_align_of(f.ty).size);
    }
    if largest < view.layout.size {
        out.push_str(&format!("    unsigned char _pad[{}];\n", view.layout.size));
    }
}
