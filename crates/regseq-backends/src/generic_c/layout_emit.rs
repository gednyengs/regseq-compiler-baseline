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

use std::collections::HashSet;

use regseq_model::{Layout, LayoutKind, RegseqModel, Ty, TyId};

use super::naming::sanitize;
use super::types::declare;
use super::NameTable;

/// Generates synthetic padding-member names that never collide with the
/// aggregate's real (sanitized) field names (Fix #1). A field literally named
/// `_pad0` is a legal sanitized name, so the namer seeds its reserved set with
/// the real member names and skips any `_pad<n>` already taken, reserving each
/// name it yields (so successive fillers are also mutually distinct).
struct PadNamer {
    used: HashSet<String>,
    next: usize,
}

impl PadNamer {
    /// Seed the namer with the already-emitted real member names.
    fn new(used: HashSet<String>) -> PadNamer {
        PadNamer { used, next: 0 }
    }

    /// The next `_pad<n>` filler name not colliding with a real member or a
    /// previously yielded filler. Reserves the returned name.
    fn fresh(&mut self) -> String {
        loop {
            let candidate = format!("_pad{}", self.next);
            self.next += 1;
            if self.used.insert(candidate.clone()) {
                return candidate;
            }
        }
    }
}

/// One emittable (non-ZST accounted) field, normalized across the
/// struct / tuple / union / closure shapes (and, for task c2, an enum
/// tag / variant-payload view).
pub(super) struct ViewField {
    /// C field name (sanitized source name, or `_N` for a tuple).
    pub(super) name: String,
    /// Field type.
    pub(super) ty: TyId,
    /// Byte offset within the aggregate (`0` for every union field).
    pub(super) offset: u64,
}

/// How a field body's per-field `offsetof` guard is spelled, so the same
/// field-layout machinery ([`emit_offset_fields`]) serves both a plain
/// aggregate (`offsetof(struct T, f)`) and an enum union-of-views member
/// (`offsetof(struct E, u._v0.f)`).
pub(super) struct GuardDesignator<'a> {
    /// The `offsetof` type operand, e.g. `"struct T"` / `"union U"` /
    /// `"struct E"`.
    pub(super) type_operand: &'a str,
    /// The aggregate label used in the assert message, e.g. `"T"` / `"E"`.
    pub(super) label: &'a str,
    /// The member-path prefix prepended to each field name in the
    /// designator + message: `""` for a plain field, `"u._v0."` for an
    /// enum union-of-views member.
    pub(super) member_prefix: &'a str,
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
pub(super) fn is_zst(model: &RegseqModel, ty: TyId) -> bool {
    model.size_align_of(ty).size == 0
}

/// The C field name + field type the aggregate definition emits for field
/// `idx` of a struct / tuple / union / closure `ty`: tuple fields are
/// `_<idx>`, struct / union fields are `sanitize(FieldLayout.name)`, and a
/// closure resolves through its parallel capture struct -- the same names
/// [`emit_aggregate`] / [`emit_offset_fields`] emit. Task d2's field access
/// (`.fname`) and aggregate construction (designated initializers) spell
/// these names so they agree with the definition (and its `offsetof`
/// guards). `idx` is the source / MIR field index. Returns `None` when `ty`
/// is not a non-enum aggregate or `idx` is out of range.
pub(super) fn field_c_name_ty(model: &RegseqModel, ty: TyId, idx: usize) -> Option<(String, TyId)> {
    let view = resolve_view(model, ty)?;
    view.fields.get(idx).map(|f| (f.name.clone(), f.ty))
}

/// The GNU type-attribute clause forcing the aggregate's C alignment to
/// `layout.align` (section 14 step 4): `aligned(<align>)`, plus `packed`
/// when `pack.is_some()`. The alignment is always `layout.align`, never
/// `pack`'s `n` -- under `repr(packed(n))` the type's true alignment is
/// `min(max_field_align, n) = layout.align`, which the `_Alignof` guard
/// asserts.
pub(super) fn align_attr(layout: &Layout) -> String {
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
        let type_operand = format!("struct {agg_name}");
        let guard = GuardDesignator {
            type_operand: &type_operand,
            label: agg_name,
            member_prefix: "",
        };
        emit_offset_fields(
            model,
            names,
            &view.fields,
            view.layout.size,
            &guard,
            &mut out,
            &mut guards,
        );
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

/// Emit the section 14 field-layout body for `fields` within an aggregate
/// of total byte `size`: filter ZST fields, sort by `offset`, insert
/// explicit `_padN` fillers + a trailing pad to `size`, and append a
/// per-field `offsetof` guard (spelled via `guard`). The 4-space-indented
/// member lines go to `out`; the `_Static_assert` lines to `guards`.
///
/// Shared by [`emit_aggregate`] (a plain struct / tuple / closure body)
/// and the enum emitter ([`super::enum_emit`], each tag / variant view).
/// Field offsets are always absolute to the aggregate object -- the enum
/// views start at the enum object's offset 0, so a view-relative offset
/// *is* the absolute offset and no base-offset shifting is needed; the
/// only enum-specific input is the member-path prefix in `guard`.
pub(super) fn emit_offset_fields(
    model: &RegseqModel,
    names: &NameTable,
    fields: &[ViewField],
    size: u64,
    guard: &GuardDesignator<'_>,
    out: &mut String,
    guards: &mut String,
) {
    let mut emittable: Vec<&ViewField> = fields.iter().filter(|f| !is_zst(model, f.ty)).collect();
    emittable.sort_by_key(|f| f.offset);

    let mut padder = PadNamer::new(emittable.iter().map(|f| f.name.clone()).collect());
    let mut cursor: u64 = 0;
    for f in &emittable {
        if f.offset > cursor {
            out.push_str(&format!(
                "    unsigned char {}[{}];\n",
                padder.fresh(),
                f.offset - cursor
            ));
        }
        out.push_str(&format!("    {};\n", declare(model, names, f.ty, &f.name)));
        guards.push_str(&format!(
            "_Static_assert(offsetof({type_operand}, {prefix}{fname}) == {off}, \"{label}.{prefix}{fname} offset\");\n",
            type_operand = guard.type_operand,
            prefix = guard.member_prefix,
            label = guard.label,
            fname = f.name,
            off = f.offset,
        ));
        cursor = f.offset + model.size_align_of(f.ty).size;
    }
    if cursor < size {
        out.push_str(&format!(
            "    unsigned char {}[{}];\n",
            padder.fresh(),
            size - cursor
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
    let emittable: Vec<&ViewField> = view
        .fields
        .iter()
        .filter(|f| !is_zst(model, f.ty))
        .collect();
    let mut largest: u64 = 0;
    for f in &emittable {
        out.push_str(&format!("    {};\n", declare(model, names, f.ty, &f.name)));
        guards.push_str(&format!(
            "_Static_assert(offsetof(union {agg_name}, {fname}) == 0, \"{agg_name}.{fname} offset\");\n",
            fname = f.name,
        ));
        largest = largest.max(model.size_align_of(f.ty).size);
    }
    if largest < view.layout.size {
        // Collision-proof the trailing filler against a real union member
        // literally named `_pad` (Fix #1).
        let mut padder = PadNamer::new(emittable.iter().map(|f| f.name.clone()).collect());
        out.push_str(&format!(
            "    unsigned char {}[{}];\n",
            padder.fresh(),
            view.layout.size
        ));
    }
}
