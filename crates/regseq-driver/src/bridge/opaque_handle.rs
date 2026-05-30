//! Opaque-handle eligibility classification (regseq-backends-24, IT-14).
//!
//! `Item.opaque_handle_eligible` drives the generic-C opaque-handle pattern:
//! an aggregate used only behind a pointer is emitted as a forward declaration
//! only (`struct X;` + `struct X *` at use sites), with no full definition and
//! no layout `_Static_assert`s. The flag is constructed `false` at every
//! ADT-item construction site (the bridge interns an Item the first time it
//! sees an ADT, long before the whole-model use graph exists), so a final
//! post-pass over the assembled model is the only place the criterion can be
//! decided.
//!
//! The eligibility criteria are the ones model invariant 19 enforces
//! (`regseq_model::layout_validator::validate_opaque_handle_eligibility`):
//!
//! 1. `ItemKind::Struct` (only the nominal struct form is an opaque handle).
//! 2. Exactly one `Ty::Struct` entry names the item (one monomorphization).
//! 3. Non-generic (`args` empty).
//! 4. ZST monomorphic layout (`size == 0`, `align == 1`).
//! 5. Every reachable use is behind a `Ref` / `Ptr` -- no by-value use
//!    anywhere in the model.
//!
//! This classifier mirrors that predicate exactly: the validator runs after
//! `run_bridge` returns, so any divergence (flagging an ineligible item) would
//! fail validation loudly. Mirroring keeps the classifier and the validator in
//! lock-step.

use regseq_model::{
    CAbiShape, Const, EnumShape, GenericArg, InstanceKind, ItemId, ItemKind, Layout, LayoutKind,
    RegseqModel, Ty, TyId,
};

/// Set `Item.opaque_handle_eligible = true` on every Item that qualifies for
/// the opaque-handle pattern (the five criteria above). Idempotent: a second
/// pass over an already-classified model is a no-op.
pub(crate) fn classify_opaque_handle_eligibility(model: &mut RegseqModel) {
    let eligible: Vec<ItemId> = (0..model.items.len())
        .map(|idx| ItemId::from_u32(idx as u32))
        .filter(|item_id| is_eligible(model, *item_id))
        .collect();
    for item_id in eligible {
        model.items[item_id].opaque_handle_eligible = true;
    }
}

/// Whether `item_id` satisfies all five opaque-handle criteria.
fn is_eligible(model: &RegseqModel, item_id: ItemId) -> bool {
    // Criterion 1: nominal struct.
    if model.items[item_id].kind != ItemKind::Struct {
        return false;
    }

    // Criterion 2: exactly one `Ty::Struct` entry names the item.
    let mut matching: Option<(TyId, &Vec<GenericArg>, &Layout)> = None;
    for (ty_id, ty) in model.types.iter() {
        if let Ty::Struct {
            item: struct_item,
            args,
            layout,
        } = ty
        {
            if *struct_item == item_id {
                if matching.is_some() {
                    // More than one monomorphization -> not eligible.
                    return false;
                }
                matching = Some((ty_id, args, layout));
            }
        }
    }
    let Some((mono_ty, args, layout)) = matching else {
        return false;
    };

    // Criterion 3: non-generic.
    if !args.is_empty() {
        return false;
    }

    // Criterion 4: ZST monomorphic layout.
    if layout.size != 0 || layout.align != 1 {
        return false;
    }

    // Criterion 5: no by-value use anywhere in the model.
    !has_by_value_use(model, mono_ty)
}

/// Whether `target` appears by value (i.e. not solely behind a `Ref` / `Ptr`)
/// anywhere in the model. Mirrors
/// `regseq_model::layout_validator::has_by_value_use`.
fn has_by_value_use(model: &RegseqModel, target: TyId) -> bool {
    for (ty_id, ty) in model.types.iter() {
        if ty_id == target {
            continue;
        }
        if ty_uses_by_value(ty, target) {
            return true;
        }
    }
    for (_, sig) in model.sigs.iter() {
        if sig.inputs.contains(&target) || sig.output == target {
            return true;
        }
    }
    for lowering in model.sig_c_abi_lowerings.iter().flatten() {
        if shape_uses_by_value(&lowering.output, target) {
            return true;
        }
        for input in &lowering.inputs {
            if shape_uses_by_value(input, target) {
                return true;
            }
        }
    }
    for instance in model.instances.iter() {
        for arg in &instance.generic_args {
            if let GenericArg::Type(t) = arg {
                if *t == target {
                    return true;
                }
            }
        }
        match &instance.kind {
            InstanceKind::Static { ty, .. } | InstanceKind::Const { ty, .. } => {
                if *ty == target {
                    return true;
                }
            }
            InstanceKind::Function(_)
            | InstanceKind::Struct { .. }
            | InstanceKind::Enum { .. }
            | InstanceKind::Union { .. } => {}
        }
    }
    for (_, value) in model.consts.iter() {
        if const_uses_by_value(value, target) {
            return true;
        }
    }
    false
}

/// Whether `ty` contains `target` by value. A `Ref` / `Ptr` to `target` is a
/// pointer use (not by-value); array / slice elements, tuple / aggregate
/// fields, generic args, vtable types, and `FnDef` / `Closure` args are
/// by-value. Mirrors `layout_validator::ty_uses_by_value`.
fn ty_uses_by_value(ty: &Ty, target: TyId) -> bool {
    match ty {
        Ty::Ref(_, _) | Ty::Ptr(_, _) => false,
        Ty::Array(elem, _) | Ty::Slice(elem) => *elem == target,
        Ty::Tuple { fields, layout } => {
            fields.contains(&target) || layout_uses_by_value(layout, target)
        }
        Ty::Struct { args, layout, .. }
        | Ty::Enum { args, layout, .. }
        | Ty::Union { args, layout, .. } => {
            args.iter().any(|a| match a {
                GenericArg::Type(t) => *t == target,
                GenericArg::Const(_) => false,
            }) || layout_uses_by_value(layout, target)
        }
        Ty::FnDef(_, args) | Ty::Closure(_, args) => args.iter().any(|a| match a {
            GenericArg::Type(t) => *t == target,
            GenericArg::Const(_) => false,
        }),
        Ty::Dynamic { vtable_ty, .. } => *vtable_ty == target,
        Ty::FnPtr(_)
        | Ty::Bool
        | Ty::Char
        | Ty::Int(_)
        | Ty::Uint(_)
        | Ty::Float(_)
        | Ty::Str
        | Ty::Never => false,
    }
}

/// Whether any field of `layout` holds `target` by value. Mirrors
/// `layout_validator::layout_uses_by_value`.
fn layout_uses_by_value(layout: &Layout, target: TyId) -> bool {
    match &layout.kind {
        LayoutKind::Struct { fields } | LayoutKind::Union { fields } => {
            fields.iter().any(|f| f.ty == target)
        }
        LayoutKind::Enum { shape } => match shape {
            EnumShape::Empty => false,
            EnumShape::Single { variant, .. } => variant.fields.iter().any(|f| f.ty == target),
            EnumShape::Multiple { variants, .. } => variants
                .iter()
                .any(|v| v.fields.iter().any(|f| f.ty == target)),
        },
    }
}

/// Whether a C-ABI shape substitutes to `target` by value. Mirrors
/// `layout_validator::shape_uses_by_value`.
fn shape_uses_by_value(shape: &CAbiShape, target: TyId) -> bool {
    match shape {
        CAbiShape::Direct => false,
        CAbiShape::Substituted { emit_as, .. } => *emit_as == target,
    }
}

/// Whether a const value embeds `target` by value. A `NullPtr(target)` is a
/// pointer-typed const (pointer use, not by-value). Mirrors
/// `layout_validator::const_uses_by_value`.
fn const_uses_by_value(value: &Const, target: TyId) -> bool {
    matches!(value, Const::NullPtr(t) if *t == target)
}

#[cfg(test)]
mod tests {
    use super::*;
    use regseq_model::{
        CrateId, DefPath, FieldLayout, GenericArgs, Item, Linkage, Mutability, Provenance,
        SourceFileId, Span, StableCrateId, UintKind,
    };

    fn model() -> RegseqModel {
        RegseqModel::with_primary("oh", StableCrateId(1))
    }

    fn span() -> Span {
        Span {
            file: SourceFileId::SYNTHESIZED,
            lo: 0,
            hi: 0,
        }
    }

    fn push_item(model: &mut RegseqModel, rendered: &str) -> ItemId {
        let rid = model.strings.intern(rendered);
        model.items.push(Item {
            def_path: DefPath {
                crate_id: CrateId::ZERO,
                segments: Vec::new(),
                rendered: rid,
            },
            kind: ItemKind::Struct,
            span: span(),
            linkage: Linkage::Default,
            link_section: None,
            inline_hint: regseq_model::InlineHint::None,
            is_unsafe: false,
            is_phantom_data: false,
            is_unsafe_cell: false,
            is_manually_drop: false,
            opaque_handle_eligible: false,
        })
    }

    fn zst_layout() -> Layout {
        Layout {
            size: 0,
            align: 1,
            pack: None,
            uninhabited: false,
            kind: LayoutKind::Struct { fields: Vec::new() },
        }
    }

    /// Intern a ZST struct `Handle` whose only use is behind a `*const Handle`
    /// field of a sized `Holder` struct: the classifier must flag it.
    #[test]
    fn flags_zst_struct_used_only_behind_pointer() {
        let mut m = model();
        let handle_item = push_item(&mut m, "Handle");
        let handle_ty = m.types.intern(Ty::Struct {
            item: handle_item,
            args: GenericArgs::new(),
            layout: zst_layout(),
        });
        let handle_ptr = m.types.intern(Ty::Ptr(handle_ty, Mutability::Not));
        // Holder { p: *const Handle } -- a by-value field of pointer type.
        let holder_item = push_item(&mut m, "Holder");
        let p = FieldLayout {
            name: m.strings.intern("p"),
            ty: handle_ptr,
            offset: 0,
        };
        m.types.intern(Ty::Struct {
            item: holder_item,
            args: GenericArgs::new(),
            layout: Layout {
                size: 8,
                align: 8,
                pack: None,
                uninhabited: false,
                kind: LayoutKind::Struct { fields: vec![p] },
            },
        });

        classify_opaque_handle_eligibility(&mut m);
        assert!(
            m.items[handle_item].opaque_handle_eligible,
            "ZST struct behind a pointer must be flagged"
        );
        assert!(
            !m.items[holder_item].opaque_handle_eligible,
            "the sized holder must not be flagged"
        );
    }

    /// A ZST struct used BY VALUE (a non-pointer field) must not be flagged.
    #[test]
    fn does_not_flag_by_value_used_struct() {
        let mut m = model();
        let handle_item = push_item(&mut m, "Marker");
        let handle_ty = m.types.intern(Ty::Struct {
            item: handle_item,
            args: GenericArgs::new(),
            layout: zst_layout(),
        });
        // Holder { m: Marker } -- a by-value field.
        let holder_item = push_item(&mut m, "Holder");
        let f = FieldLayout {
            name: m.strings.intern("m"),
            ty: handle_ty,
            offset: 0,
        };
        m.types.intern(Ty::Struct {
            item: holder_item,
            args: GenericArgs::new(),
            layout: Layout {
                size: 0,
                align: 1,
                pack: None,
                uninhabited: false,
                kind: LayoutKind::Struct { fields: vec![f] },
            },
        });

        classify_opaque_handle_eligibility(&mut m);
        assert!(
            !m.items[handle_item].opaque_handle_eligible,
            "a by-value-used ZST must not be flagged"
        );
    }

    /// A non-ZST struct behind a pointer is not an opaque handle (criterion 4).
    #[test]
    fn does_not_flag_non_zst_struct() {
        let mut m = model();
        let item_id = push_item(&mut m, "Sized");
        let u32t = m.types.intern(Ty::Uint(UintKind::U32));
        let f = FieldLayout {
            name: m.strings.intern("v"),
            ty: u32t,
            offset: 0,
        };
        let sized_ty = m.types.intern(Ty::Struct {
            item: item_id,
            args: GenericArgs::new(),
            layout: Layout {
                size: 4,
                align: 4,
                pack: None,
                uninhabited: false,
                kind: LayoutKind::Struct { fields: vec![f] },
            },
        });
        // Used only behind a reference, but it is not a ZST.
        m.types.intern(Ty::Ref(sized_ty, Provenance::SharedBorrow));

        classify_opaque_handle_eligibility(&mut m);
        assert!(
            !m.items[item_id].opaque_handle_eligible,
            "a non-ZST struct is not an opaque handle"
        );
    }

    /// The classifier is idempotent.
    #[test]
    fn classification_is_idempotent() {
        let mut m = model();
        let handle_item = push_item(&mut m, "Handle");
        let handle_ty = m.types.intern(Ty::Struct {
            item: handle_item,
            args: GenericArgs::new(),
            layout: zst_layout(),
        });
        m.types.intern(Ty::Ptr(handle_ty, Mutability::Not));

        classify_opaque_handle_eligibility(&mut m);
        let first = m.items[handle_item].opaque_handle_eligible;
        classify_opaque_handle_eligibility(&mut m);
        assert_eq!(first, m.items[handle_item].opaque_handle_eligible);
        assert!(first);
    }
}
