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
    /// `None` for the tag view; `Some(variant_idx)` for a variant payload
    /// view. The body-side navigator looks views up by this so it reads the
    /// `member` / field names `build_views` produced rather than re-deriving
    /// them.
    variant: Option<usize>,
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

    let type_operand = format!("struct {enum_name}");
    if views.is_empty() {
        // Zero views: a size-faithful struct (an `Empty` enum, or a
        // `Single { scalar: None }` whose sole variant is fieldless). A
        // size-0 uninhabited enum collapses to an empty `struct` (a
        // gnu11 extension).
        if layout.size > 0 {
            out.push_str(&format!("    unsigned char _pad0[{}];\n", layout.size));
        }
    } else {
        // One view collapses inline (no `union` wrapper, empty member
        // prefix), so a fieldless `Direct` enum becomes a tag-only struct
        // (section 9.8) and a niche `Option<&T>` a single-field struct
        // (section 9.9); two or more views share a `union { ... } u;` with
        // the nested `u.<member>.<field>` designator. The inline-vs-`union`
        // collapse + member-prefix spelling is [`member_prefix`] -- the one
        // source of truth shared with the body-side navigator (task d2.1),
        // so the access path cannot drift from this definition.
        let union_wrap = views.len() >= 2;
        if union_wrap {
            out.push_str("    union {\n");
        }
        for view in &views {
            if union_wrap {
                out.push_str("    struct {\n");
            }
            let prefix = member_prefix(&views, &view.member);
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
            if union_wrap {
                out.push_str(&format!("    }} {};\n", view.member));
            }
        }
        if union_wrap {
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
            variant: None,
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
                variant: Some(idx),
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

/// The C member-designator prefix for a view `member` given the full view
/// list: empty when the enum collapses to <= 1 view (emitted inline, no
/// `union` wrapper), else `u.<member>.` (the shared `union { ... } u;`).
///
/// This is the single source of truth for the inline-vs-`union` collapse,
/// consumed by both [`emit_enum`] (the definition + its `offsetof` guards)
/// and the body-side navigator below (task d2.1's `Downcast`/`Field`,
/// `SetDiscriminant`, `Aggregate::Enum`, `Const::EnumValue` access), so the
/// access path can never disagree with the emitted member path.
fn member_prefix(views: &[View], member: &str) -> String {
    if views.len() <= 1 {
        String::new()
    } else {
        format!("u.{member}.")
    }
}

// -- task d2.1 body-side navigator (the shared source of truth) -----------

/// The tag cell of an enum, as the body side accesses it (task d2.1). A thin
/// read of [`build_views`] / [`tag_view_scalar`] / [`TagScalar`]; recomputes
/// no offset, tag value, or name.
pub(super) enum EnumTag {
    /// A `_tag` view exists (`Single { scalar: Some }` / `Multiple { Direct }`):
    /// the cell is the member at `member_path` (`tag` inline / `u._tag.tag`
    /// multi-view) of logical C type `scalar_ty`.
    View {
        /// The member designator suffix appended to the enum lvalue.
        member_path: String,
        /// The tag scalar's logical type.
        scalar_ty: TyId,
    },
    /// A niche cell (`Multiple { Niche }`): no view -- a byte region at
    /// `offset` of logical type `scalar_ty`, accessed via the section 14.7
    /// byte-storage (memcpy) technique.
    Niche {
        /// Byte offset of the niche cell within the enum object.
        offset: u64,
        /// The niche scalar's logical type.
        scalar_ty: TyId,
    },
    /// No tag bytes (`Single { scalar: None }` / `Empty`).
    None,
}

/// Resolve the [`EnumShape`] of an enum `ty`, or `None` if `ty` is not an
/// enum. The body-side entry point into the shared enum machinery.
pub(super) fn enum_shape(model: &RegseqModel, ty: TyId) -> Option<&EnumShape> {
    match &model.types[ty] {
        Ty::Enum { layout, .. } => match &layout.kind {
            LayoutKind::Enum { shape } => Some(shape),
            _ => None,
        },
        _ => None,
    }
}

/// Whether `shape` collapses to its single view inline (no `union` wrapper,
/// empty member prefix): true for 0 or 1 views. The body side consults this
/// for the `Const::EnumValue` designator (task d2.1).
pub(super) fn enum_is_inline(model: &RegseqModel, shape: &EnumShape) -> bool {
    build_views(model, shape).len() <= 1
}

/// The C member-designator suffix + post-narrow type for variant `variant_idx`'s
/// payload field `field_idx` of enum `enum_ty`: `u._v<variant>.<name>`
/// (multi-view) or `<name>` (inline). The `_v<variant>` member name and the
/// `<name>` payload field name are read straight from the [`View`] /
/// [`ViewField`] [`build_views`] produced (not re-derived), so they match the
/// emitted definition exactly. `None` when the variant contributes no view
/// (all fields ZST), the field is ZST (omitted from the view), or out of range
/// -- a caller-handled signal, not a silent default.
pub(super) fn enum_variant_field(
    model: &RegseqModel,
    enum_ty: TyId,
    variant_idx: usize,
    field_idx: usize,
) -> Option<(String, TyId)> {
    let shape = enum_shape(model, enum_ty)?;
    let views = build_views(model, shape);
    let view = views.iter().find(|v| v.variant == Some(variant_idx))?;
    let field = view.fields.get(field_idx)?;
    if is_zst(model, field.ty) {
        return None;
    }
    let prefix = member_prefix(&views, &view.member);
    Some((format!("{prefix}{}", field.name), field.ty))
}

/// The tag cell descriptor for `shape` (task d2.1). The `_tag` view member /
/// field names are read straight from the [`View`] [`build_views`] produced
/// (not re-derived); the niche cell reads [`TagScalar`] directly -- never
/// re-deriving the niche formula.
pub(super) fn enum_tag(model: &RegseqModel, shape: &EnumShape) -> EnumTag {
    if let Some(tag) = tag_view_scalar(shape) {
        let views = build_views(model, shape);
        // The tag view is the one with `variant == None`; its single field is
        // the `tag` cell. Read both names from `build_views`.
        let view = views
            .iter()
            .find(|v| v.variant.is_none())
            .expect("a tag scalar implies a tag view");
        let prefix = member_prefix(&views, &view.member);
        return EnumTag::View {
            member_path: format!("{prefix}{}", view.fields[0].name),
            scalar_ty: tag.scalar_ty,
        };
    }
    match shape {
        EnumShape::Multiple {
            tag,
            encoding: TagEncoding::Niche { .. },
            ..
        } => EnumTag::Niche {
            offset: tag.offset,
            scalar_ty: tag.scalar_ty,
        },
        _ => EnumTag::None,
    }
}
