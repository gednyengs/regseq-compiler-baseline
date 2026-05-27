//! The section 13 / D9 enum C representation -- the offset-faithful
//! "union-of-views" form, reusing the section 14 field-offset primitive
//! ([`super::layout_emit`]) for the tag view and each variant payload view.
//!
//! Rationale (the locked D9 decision). A naive `{ tag; union; }` shape is
//! unsound: it assumes the tag sits at offset 0 and the payload is a
//! single contiguous region relocatable to one `union_start`. Neither
//! holds. rustc lays the discriminant out as a prefix and places each
//! variant's payload fields at offsets that are *absolute to the whole
//! enum object*, and those offsets are not relocatable to a common base --
//! a shifted-alignment `Direct` enum such as `enum E { A(u8), B(u32) }`
//! (tag `u8` @0, `A.0` @1, `B.0` @4) cannot place its `uint32_t` payload at
//! the first payload byte.
//!
//! Instead each view -- the tag cell and every variant's payload -- is
//! emitted as a full-object section 14 `struct` whose fields sit at their
//! *absolute* offsets from the enum object's start (so a field's
//! view-relative offset *is* its absolute offset; rustc already placed it
//! at a naturally-aligned position, so no view needs packing). All views
//! share one `union`, wrapped in the outer enum `struct` forced to
//! `__attribute__((aligned(layout.align)))`; the union aliasing reproduces
//! the exact byte layout. The `_Static_assert(sizeof / _Alignof /
//! offsetof)` guards make the emitted TU self-checking: compiling it is
//! the proof that the lowering reproduces rustc's enum layout.
//!
//! Scope (task c2): the type *representation* only. The tag *operations*
//! (`Discriminant` / `SetDiscriminant` / `Downcast` / `Aggregate::Enum` /
//! `Const::EnumValue`) and the per-variant tag-value constants are
//! use-site / body lowering -- task d.

use regseq_model::{EnumShape, LayoutKind, RegseqModel, TagEncoding, TagScalar, Ty, TyId};

use super::layout_emit::{align_attr, emit_offset_fields, is_zst, GuardDesignator, ViewField};
use super::naming::sanitize;
use super::NameTable;

/// One emitted view: its `union` member name (`_tag` / `_vN`) and the
/// fields it holds, each at its absolute enum-object offset.
struct View {
    /// The `union` member name (`_tag` for the tag cell, `_v<idx>` for a
    /// variant payload).
    member: String,
    /// The view's fields, at their absolute offsets.
    fields: Vec<ViewField>,
}

/// Emit the section 13 / D9 offset-faithful C `struct` for an enum `ty`
/// (the union-of-views form) plus its `_Static_assert` layout guards.
///
/// `enum_name` is the resolved C type name (already spelled `struct
/// <enum_name>` by the section 2 `Ty`->C path). Returns the empty string
/// for a non-enum `ty` (the caller dispatches enums here, so this is
/// defensive only).
pub(crate) fn emit_enum(
    model: &RegseqModel,
    names: &NameTable,
    ty: TyId,
    enum_name: &str,
) -> String {
    let layout = match &model.types[ty] {
        Ty::Enum { layout, .. } => layout,
        _ => return String::new(),
    };
    let shape = match &layout.kind {
        LayoutKind::Enum { shape } => shape,
        _ => return String::new(),
    };

    let views = build_views(model, shape);

    let mut out = String::new();
    let mut guards = String::new();
    out.push_str(&format!("struct {enum_name} {{\n"));

    match views.as_slice() {
        // Zero views: a size-faithful struct (an `Empty` enum, or a
        // `Single { scalar: None }` whose sole variant is fieldless). A
        // size-0 uninhabited enum collapses to an empty `struct` (a
        // gnu11 extension).
        [] => {
            if layout.size > 0 {
                out.push_str(&format!("    unsigned char _pad0[{}];\n", layout.size));
            }
        }
        // Exactly one view: emit it inline (no `union` wrapper), so a
        // fieldless `Direct` enum collapses to a tag-only struct (section
        // 9.8) and a niche `Option<&T>` to a single-field struct (section
        // 9.9). The designator has no member prefix.
        [view] => {
            let type_operand = format!("struct {enum_name}");
            let guard = GuardDesignator {
                type_operand: &type_operand,
                label: enum_name,
                member_prefix: "",
            };
            emit_offset_fields(
                model,
                names,
                &view.fields,
                layout.size,
                &guard,
                &mut out,
                &mut guards,
            );
        }
        // Two or more views: the `union { ... } u;` wrapper, each view a
        // section 14 struct padded to the enum size; guards use the nested
        // `u.<member>.<field>` member designator.
        _ => {
            let type_operand = format!("struct {enum_name}");
            out.push_str("    union {\n");
            for view in &views {
                out.push_str("    struct {\n");
                let prefix = format!("u.{}.", view.member);
                let guard = GuardDesignator {
                    type_operand: &type_operand,
                    label: enum_name,
                    member_prefix: &prefix,
                };
                emit_offset_fields(
                    model,
                    names,
                    &view.fields,
                    layout.size,
                    &guard,
                    &mut out,
                    &mut guards,
                );
                out.push_str(&format!("    }} {};\n", view.member));
            }
            out.push_str("    } u;\n");
        }
    }

    out.push_str(&format!("}} {};\n", align_attr(layout)));
    out.push_str(&format!(
        "_Static_assert(sizeof(struct {enum_name}) == {}, \"{enum_name} size\");\n",
        layout.size
    ));
    out.push_str(&format!(
        "_Static_assert(_Alignof(struct {enum_name}) == {}, \"{enum_name} align\");\n",
        layout.align
    ));
    out.push_str(&guards);
    out
}

/// Build the ordered view list for `shape`: the tag view (when present)
/// followed by one view per variant with at least one non-ZST field.
fn build_views(model: &RegseqModel, shape: &EnumShape) -> Vec<View> {
    let mut views = Vec::new();
    if let Some(tag) = tag_view_scalar(shape) {
        views.push(View {
            member: "_tag".to_string(),
            fields: vec![ViewField {
                name: "tag".to_string(),
                ty: tag.scalar_ty,
                offset: tag.offset,
            }],
        });
    }
    for (idx, variant) in shape_variants(shape).iter().enumerate() {
        let fields = variant_view_fields(model, &variant.fields);
        if fields.iter().any(|f| !is_zst(model, f.ty)) {
            views.push(View {
                member: format!("_v{idx}"),
                fields,
            });
        }
    }
    views
}

/// The tag scalar whose cell becomes a `_tag` view: present iff
/// `Single { scalar: Some }` or `Multiple { encoding: Direct }`. `Niche`
/// carries no tag view (the niche cell is a payload field whose bit
/// pattern is the tag), nor do `Single { scalar: None }` / `Empty`.
fn tag_view_scalar(shape: &EnumShape) -> Option<&TagScalar> {
    match shape {
        EnumShape::Single {
            scalar: Some(tag), ..
        } => Some(tag),
        EnumShape::Multiple {
            tag,
            encoding: TagEncoding::Direct,
            ..
        } => Some(tag),
        _ => None,
    }
}

/// The variants of `shape` in `VariantIdx` order, for the `_v<idx>`
/// view naming. `Empty` has none; `Single` has its one variant.
fn shape_variants(shape: &EnumShape) -> Vec<&regseq_model::VariantLayout> {
    match shape {
        EnumShape::Empty => Vec::new(),
        EnumShape::Single { variant, .. } => vec![variant],
        EnumShape::Multiple { variants, .. } => variants.iter().collect(),
    }
}

/// Build the [`ViewField`] list for a variant's payload fields, at their
/// absolute enum-object offsets. ZST fields are kept here (the section 14
/// builder filters them) so the caller can decide whether the variant
/// contributes a view at all.
fn variant_view_fields(
    model: &RegseqModel,
    fields: &[regseq_model::FieldLayout],
) -> Vec<ViewField> {
    fields
        .iter()
        .enumerate()
        .map(|(idx, f)| ViewField {
            name: view_field_name(model.strings.resolve(f.name), idx),
            ty: f.ty,
            offset: f.offset,
        })
        .collect()
}

/// The C member name for an enum variant payload field. Struct-style
/// variant fields (`B { x, y }`) sanitize to `x` / `y`; tuple-style
/// variant fields are named `0` / `1` / ... by rustc, and a leading digit
/// is not a valid C identifier, so they fall back to a positional `_<idx>`
/// (the same `_<idx>` convention task c1 uses for tuple fields). Total and
/// deterministic.
fn view_field_name(raw: &str, idx: usize) -> String {
    let s = sanitize(raw);
    if s.chars().next().is_none_or(|c| c.is_ascii_digit()) {
        format!("_{idx}")
    } else {
        s
    }
}
