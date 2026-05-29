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

use super::layout_emit::{
    align_attr, emit_offset_fields, is_zst, reserved_member_name, GuardDesignator, ViewField,
};
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
    let const_views = build_const_views(model, shape);

    let mut out = String::new();
    let mut guards = String::new();
    out.push_str(&format!("struct {enum_name} {{\n"));

    let type_operand = format!("struct {enum_name}");
    if views.is_empty() {
        // Zero views: a size-faithful struct (an `Empty` enum, or a
        // `Single { scalar: None }` whose sole variant is fieldless). A
        // size-0 uninhabited enum collapses to an empty `struct` (a
        // gnu11 extension). No payload / niche const exists, so no `_raw`.
        if layout.size > 0 {
            out.push_str(&format!("    unsigned char _pad0[{}];\n", layout.size));
        }
    } else if views.len() == 1 {
        // Exactly one view (the section 9.8 tag-only / 9.9 single-field
        // collapse). Change A (d2.3b): wrap the view in an ANONYMOUS
        // `union { struct { ... }; unsigned char <raw>[size]; }` so the inline
        // enum gains a byte-covering `<raw>` for the uniform const path while
        // gnu11 member promotion keeps the runtime access path IDENTICAL
        // (`e.<field>`) -- `member_prefix` stays `""`, the d2.1 navigator is
        // untouched. The `<raw>` name is collision-proof against the promoted
        // view field names (a Rust variant field named `_raw` would otherwise
        // clash with the array member) and spelled solely through
        // [`enum_raw_member`] / [`raw_member_for`].
        out.push_str("    union {\n");
        out.push_str("    struct {\n");
        let prefix = member_prefix(&views, &views[0].member); // "" for one view
        let guard = GuardDesignator {
            type_operand: &type_operand,
            label: enum_name,
            member_prefix: &prefix,
        };
        emit_offset_fields(
            model,
            names,
            &views[0].fields,
            layout.size,
            &guard,
            &mut out,
            &mut guards,
        );
        out.push_str("    };\n");
        let raw = raw_member_for(&views, &const_views);
        out.push_str(&format!("    unsigned char {raw}[{}];\n", layout.size));
        out.push_str("    };\n");
        // The promoted `<raw>` member is `offsetof`-addressable and starts at 0.
        guards.push_str(&format!(
            "_Static_assert(offsetof(struct {enum_name}, {raw}) == 0, \"{enum_name}.{raw} offset\");\n",
        ));
    } else {
        // Two or more views share a `union { ... } u;` with the nested
        // `u.<member>.<field>` designator. The inline-vs-`union` collapse +
        // member-prefix spelling is [`member_prefix`] -- the one source of
        // truth shared with the body-side navigator (task d2.1), so the
        // access path cannot drift from this definition.
        out.push_str("    union {\n");
        for view in &views {
            out.push_str("    struct {\n");
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
            out.push_str(&format!("    }} {};\n", view.member));
        }
        // Change C (d2.3b / M1): a CONST-INIT-ONLY typed full-object view
        // `_c<idx>` per payload-bearing variant of a multi-view tag-view
        // (`Direct`) enum -- `struct { <tag>; <payload fields...>; }` at
        // ABSOLUTE offsets -- so a tagged enum const whose payload carries a
        // relocation can route the tag + the reloc through ONE union
        // designator (`_raw`, a `char[]`, cannot hold an address constant).
        // The runtime `_tag` / `_v<idx>` views + navigator are unchanged
        // (`_c<idx>` is written only by the const path). Layout-neutral:
        // another full-object-sized union member.
        for view in &const_views {
            out.push_str("    struct {\n");
            let prefix = member_prefix(&views, &view.member); // "u._c<idx>."
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
        // Change A (d2.2): a full-object byte-image member so a value-position
        // enum const can initialize the union through one expression
        // (`.u._raw = { ... }`). A C designated initializer that names a second
        // union member overrides the first, so the tag and payload cannot be
        // set separately in a const; `_raw` carries the whole serialized value.
        // Layout-neutral: the views are already full-object size, `_raw` is
        // alignment 1, and the outer `aligned` + the size/align
        // `_Static_assert`s still hold. The name is collision-proof against the
        // generated `u` member names via [`raw_member_for`] (for uniformity --
        // a fixed `_raw` is already safe among generated siblings).
        let raw = raw_member_for(&views, &const_views);
        out.push_str(&format!("    unsigned char {raw}[{}];\n", layout.size));
        out.push_str("    } u;\n");
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

/// Build the `_c<idx>` const-init-only typed full-object views (Change C /
/// M1): one per payload-bearing variant of a multi-view tag-view (`Direct` /
/// `Single { scalar: Some }`) enum. Each view holds the tag field (its name
/// collision-proofed against the payload field names -- typically `tag`,
/// falling back to `tag0`/... on a clash) followed by the variant's payload
/// fields (the SAME names / types / absolute offsets `_v<idx>` uses; ZST fields
/// are kept so the body side can index it in parallel with the payload, and the
/// section 14 emitter filters them). Empty for an enum with no tag view (a
/// niche / `Single { scalar: None }` / `Empty` shape) -- a niche enum's reloc
/// const is M1-lite (untagged, d2.3a) or the niche-tagged-reloc defensive guard.
fn build_const_views(model: &RegseqModel, shape: &EnumShape) -> Vec<View> {
    let Some(tag) = tag_view_scalar(shape) else {
        return Vec::new();
    };
    let mut const_views = Vec::new();
    for (idx, variant) in shape_variants(shape).iter().enumerate() {
        let payload = variant_view_fields(model, &variant.fields);
        if !payload.iter().any(|f| !is_zst(model, f.ty)) {
            // A fieldless variant carries no payload, so no `_c<idx>` view (its
            // const is the single tag-member init).
            continue;
        }
        let tag_name = reserved_member_name("tag", payload.iter().map(|f| f.name.clone()));
        let mut fields = vec![ViewField {
            name: tag_name,
            ty: tag.scalar_ty,
            offset: tag.offset,
        }];
        fields.extend(payload);
        const_views.push(View {
            member: format!("_c{idx}"),
            variant: Some(idx),
            fields,
        });
    }
    const_views
}

/// The bare `<raw>` byte-image member name for an enum whose `views` /
/// `const_views` were produced by [`build_views`] / [`build_const_views`]
/// (Change A). For a one-view inline enum the `<raw>` member is PROMOTED next
/// to the view's field names, so the name is collision-proofed against those
/// (a Rust variant field named `_raw` would otherwise clash); for a multi-view
/// enum the `<raw>` sits among the generated `_tag` / `_v<idx>` / `_c<idx>`
/// siblings (a fixed `_raw` is already safe, but the same reservation is used
/// for uniformity + future-proofing). The single source of truth both
/// [`emit_enum`] (the definition) and [`enum_raw_member`] (the body designator)
/// spell the member through, so the two cannot drift.
fn raw_member_for(views: &[View], const_views: &[View]) -> String {
    if views.len() <= 1 {
        reserved_member_name("_raw", views[0].fields.iter().map(|f| f.name.clone()))
    } else {
        let members = views
            .iter()
            .chain(const_views.iter())
            .map(|v| v.member.clone());
        reserved_member_name("_raw", members)
    }
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

/// The byte-image `<raw>` member designator for a value-position enum const,
/// the NAME-COMPUTING single source of truth (Change A). `Some(u.<raw>)` for a
/// multi-view enum (whose `union u` carries `unsigned char <raw>[size]`),
/// `Some(<raw>)` for a one-view inline enum (whose ANONYMOUS union PROMOTES a
/// byte-covering `<raw>` sibling -- access path unchanged), `None` for a
/// zero-view (`Empty` / fieldless `Single { scalar: None }`) enum (no payload /
/// niche const exists). The `<raw>` name is computed through the SAME
/// [`raw_member_for`] call [`emit_enum`] spells the definition through (the same
/// collision-proof seed), so the const designator and the emitted member cannot
/// drift; the body side spells `.<raw>` only through this lookup.
pub(super) fn enum_raw_member(model: &RegseqModel, shape: &EnumShape) -> Option<String> {
    let views = build_views(model, shape);
    if views.is_empty() {
        return None;
    }
    let const_views = build_const_views(model, shape);
    let raw = raw_member_for(&views, &const_views);
    if views.len() == 1 {
        Some(raw)
    } else {
        Some(format!("u.{raw}"))
    }
}

/// One emitted `_c<idx>` const-init-only typed full-object view (Change C /
/// M1), as the body side accesses it. Read straight from [`build_const_views`]
/// (the `tag` sub-field name, the payload field names / types / offsets are
/// never re-derived).
pub(super) struct TypedView {
    /// The union member name, e.g. `_c1` (the const designator is
    /// `.u._c1 = { ... }`).
    pub(super) member: String,
    /// The synthetic tag sub-field name (collision-proofed against the payload
    /// field names).
    pub(super) tag_name: String,
    /// The tag scalar's logical type.
    pub(super) tag_ty: TyId,
    /// The variant's payload fields (the same names / types / absolute offsets
    /// `_v<idx>` uses). ZST fields are kept so the body side can index them in
    /// parallel with the const payload (it filters them when emitting
    /// designators, matching the section 14 view emitter).
    pub(super) fields: Vec<ViewField>,
}

/// The `_c<idx>` typed full-object view for variant `variant_idx` of enum
/// `enum_ty` (Change C / M1), or `None` when the variant has no such view (not
/// a tag-view enum, a fieldless variant, or out of range). The body side spells
/// the const designator `.u._c<idx> = { .<tag> = .., .<field> = .. }` through
/// this, never re-deriving the `_c<idx>` member / `tag` sub-field / payload
/// field names.
pub(super) fn enum_variant_typed_view(
    model: &RegseqModel,
    enum_ty: TyId,
    variant_idx: usize,
) -> Option<TypedView> {
    let shape = enum_shape(model, enum_ty)?;
    let cv = build_const_views(model, shape)
        .into_iter()
        .find(|v| v.variant == Some(variant_idx))?;
    let mut fields = cv.fields;
    // `build_const_views` puts the tag first, then the payload fields.
    let tag = fields.remove(0);
    Some(TypedView {
        member: cv.member,
        tag_name: tag.name,
        tag_ty: tag.ty,
        fields,
    })
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
