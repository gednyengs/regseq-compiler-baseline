//! Layout-consistency and Phase B (continued) validators.
//!
//! Implements the Phase B (§3.3) checks owned by task
//! `regseq-model-85` (P1a-VAL-03): target / build well-formedness
//! (invariants 15 / 17), `FnSig` / `CAbiLowering` structural checks
//! (§3.3 step 5 + Phase B continued), layout sanity (§11.5 / §12.5),
//! tag-scalar width-fit (§11.2), invariant 14 (`InstanceKind` layout
//! consistency), const-graph acyclicity (§3.3 step 9), domain-attr
//! validation (§3.6.5), invariant 19 (`Item.opaque_handle_eligible`
//! model-checkable subset), invariant 20 (`StableCrateId(0)`
//! synthetic-domain enforcement), the canonical `UNIT_LAYOUT` check
//! on every empty tuple type, and source-file line-table /
//! duplicate-path validation.

use std::collections::HashMap;

use crate::args_key::GenericArg;
use crate::cabi::{CAbiShape, SubstReason};
use crate::const_value::Const;
use crate::crate_info::StableCrateId;
use crate::domain_attr::{AttrData, FieldPath};
use crate::ids::{ConstId, CrateId, FnSigId, InstanceId, ItemId, SourceFileId, TyId};
use crate::instance::InstanceKind;
use crate::item::ItemKind;
use crate::layout::{
    EnumShape, FieldLayout, Layout, LayoutKind, TagScalar, VariantLayout, UNIT_LAYOUT,
};
use crate::model::{InvariantViolation, RegseqModel};
use crate::synthetic::SyntheticItemKind;
use crate::target::Endian;
use crate::ty::{FnAbi, IntKind, Ty, UintKind};

/// Validates `RegseqModel.target` against invariant 15.
pub(crate) fn validate_target(model: &RegseqModel) -> Result<(), InvariantViolation> {
    let target = &model.target;

    if target.triple.index() >= model.strings.len() {
        return Err(InvariantViolation::TargetMalformed {
            reason: "target.triple StringId is out of range",
        });
    }
    if target.data_layout.index() >= model.strings.len() {
        return Err(InvariantViolation::TargetMalformed {
            reason: "target.data_layout StringId is out of range",
        });
    }
    if model.strings.resolve(target.triple).is_empty() {
        return Err(InvariantViolation::TargetMalformed {
            reason: "target.triple is empty",
        });
    }
    let data_layout_text = model.strings.resolve(target.data_layout);
    if data_layout_text.is_empty() {
        return Err(InvariantViolation::TargetMalformed {
            reason: "target.data_layout is empty",
        });
    }

    if !matches!(target.pointer_width, 16 | 32 | 64) {
        return Err(InvariantViolation::TargetMalformed {
            reason: "target.pointer_width must be 16, 32, or 64",
        });
    }

    let parsed = parse_data_layout(data_layout_text)?;
    if let Some(endian) = parsed.endian {
        if endian != target.endian {
            return Err(InvariantViolation::TargetMalformed {
                reason: "target.endian disagrees with target.data_layout",
            });
        }
    }
    if let Some(width) = parsed.pointer_width {
        if width != target.pointer_width {
            return Err(InvariantViolation::TargetMalformed {
                reason: "target.pointer_width disagrees with target.data_layout",
            });
        }
    }

    let scalars = &target.scalar_layouts;
    if !is_populated(scalars.bool_) {
        return Err(InvariantViolation::TargetMalformed {
            reason: "target.scalar_layouts.bool_ is unpopulated",
        });
    }
    if !is_populated(scalars.char_) {
        return Err(InvariantViolation::TargetMalformed {
            reason: "target.scalar_layouts.char_ is unpopulated",
        });
    }
    if !is_populated(scalars.fn_ptr) {
        return Err(InvariantViolation::TargetMalformed {
            reason: "target.scalar_layouts.fn_ptr is unpopulated",
        });
    }
    if !is_populated(scalars.data_ptr) {
        return Err(InvariantViolation::TargetMalformed {
            reason: "target.scalar_layouts.data_ptr is unpopulated",
        });
    }
    for entry in scalars.int.iter() {
        if !is_populated(*entry) {
            return Err(InvariantViolation::TargetMalformed {
                reason: "target.scalar_layouts.int has an unpopulated entry",
            });
        }
    }
    for entry in scalars.uint.iter() {
        if !is_populated(*entry) {
            return Err(InvariantViolation::TargetMalformed {
                reason: "target.scalar_layouts.uint has an unpopulated entry",
            });
        }
    }
    for entry in scalars.float.iter() {
        if !is_populated(*entry) {
            return Err(InvariantViolation::TargetMalformed {
                reason: "target.scalar_layouts.float has an unpopulated entry",
            });
        }
    }

    let alignments = [
        scalars.bool_.align,
        scalars.char_.align,
        scalars.fn_ptr.align,
        scalars.data_ptr.align,
    ]
    .into_iter()
    .chain(scalars.int.iter().map(|s| s.align))
    .chain(scalars.uint.iter().map(|s| s.align))
    .chain(scalars.float.iter().map(|s| s.align));
    for align in alignments {
        if !align.is_power_of_two() {
            return Err(InvariantViolation::TargetMalformed {
                reason: "target.scalar_layouts alignment is not a power of two",
            });
        }
    }

    let pointer_width_bytes = u64::from(target.pointer_width / 8);
    if scalars.bool_.size != 1 {
        return Err(InvariantViolation::TargetMalformed {
            reason: "target.scalar_layouts.bool_ size is not 1 byte",
        });
    }
    if scalars.char_.size != 4 {
        return Err(InvariantViolation::TargetMalformed {
            reason: "target.scalar_layouts.char_ size is not 4 bytes",
        });
    }
    for (idx, expected_bits) in [(0usize, 8), (1, 16), (2, 32), (3, 64), (4, 128)]
        .iter()
        .copied()
    {
        let expected = expected_bits / 8;
        if scalars.int[idx].size != expected {
            return Err(InvariantViolation::TargetMalformed {
                reason: "target.scalar_layouts.int width does not match IntKind",
            });
        }
        if scalars.uint[idx].size != expected {
            return Err(InvariantViolation::TargetMalformed {
                reason: "target.scalar_layouts.uint width does not match UintKind",
            });
        }
    }
    if scalars.int[5].size != pointer_width_bytes {
        return Err(InvariantViolation::TargetMalformed {
            reason: "target.scalar_layouts.int[Isize] size does not match target.pointer_width",
        });
    }
    if scalars.uint[5].size != pointer_width_bytes {
        return Err(InvariantViolation::TargetMalformed {
            reason: "target.scalar_layouts.uint[Usize] size does not match target.pointer_width",
        });
    }
    let float_widths = [16u64, 32, 64, 128];
    for (idx, bits) in float_widths.iter().copied().enumerate() {
        let expected = bits / 8;
        if scalars.float[idx].size != expected {
            return Err(InvariantViolation::TargetMalformed {
                reason: "target.scalar_layouts.float width does not match FloatKind",
            });
        }
    }
    if scalars.data_ptr.size != pointer_width_bytes {
        return Err(InvariantViolation::TargetMalformed {
            reason: "target.scalar_layouts.data_ptr size does not match target.pointer_width",
        });
    }
    if scalars.fn_ptr.size != pointer_width_bytes {
        return Err(InvariantViolation::TargetMalformed {
            reason: "target.scalar_layouts.fn_ptr size does not match target.pointer_width",
        });
    }

    let all_entries = [
        ("bool_", scalars.bool_),
        ("char_", scalars.char_),
        ("data_ptr", scalars.data_ptr),
        ("fn_ptr", scalars.fn_ptr),
    ]
    .into_iter()
    .chain(scalars.int.iter().copied().enumerate().map(|(i, s)| {
        (
            match i {
                0 => "int[I8]",
                1 => "int[I16]",
                2 => "int[I32]",
                3 => "int[I64]",
                4 => "int[I128]",
                _ => "int[Isize]",
            },
            s,
        )
    }))
    .chain(scalars.uint.iter().copied().enumerate().map(|(i, s)| {
        (
            match i {
                0 => "uint[U8]",
                1 => "uint[U16]",
                2 => "uint[U32]",
                3 => "uint[U64]",
                4 => "uint[U128]",
                _ => "uint[Usize]",
            },
            s,
        )
    }))
    .chain(scalars.float.iter().copied().enumerate().map(|(i, s)| {
        (
            match i {
                0 => "float[F16]",
                1 => "float[F32]",
                2 => "float[F64]",
                _ => "float[F128]",
            },
            s,
        )
    }));
    for (_name, entry) in all_entries {
        if entry.size > 0 && entry.align > entry.size {
            return Err(InvariantViolation::TargetMalformed {
                reason: "target.scalar_layouts entry alignment exceeds its size",
            });
        }
    }

    Ok(())
}

/// Validates `RegseqModel.build` against invariant 17.
pub(crate) fn validate_build(model: &RegseqModel) -> Result<(), InvariantViolation> {
    if model.build.toolchain.index() >= model.strings.len() {
        return Err(InvariantViolation::BuildMalformed {
            reason: "build.toolchain StringId is out of range",
        });
    }
    if model.strings.resolve(model.build.toolchain).is_empty() {
        return Err(InvariantViolation::BuildMalformed {
            reason: "build.toolchain is empty",
        });
    }
    Ok(())
}

/// Validates `FnSig` entries against §3.3 step 5 + Phase B continued.
pub(crate) fn validate_fn_sigs(model: &RegseqModel) -> Result<(), InvariantViolation> {
    for (sig_id, sig) in model.sigs.iter() {
        if sig.c_variadic {
            if sig.abi != FnAbi::C {
                return Err(InvariantViolation::FnSigMalformed {
                    sig: sig_id,
                    reason: "c_variadic requires abi == FnAbi::C",
                });
            }
            if sig.inputs.is_empty() {
                return Err(InvariantViolation::FnSigMalformed {
                    sig: sig_id,
                    reason: "c_variadic requires at least one fixed input",
                });
            }
        }
        for input in &sig.inputs {
            if matches!(&model.types[*input], Ty::Never) {
                return Err(InvariantViolation::FnSigMalformed {
                    sig: sig_id,
                    reason: "FnSig.inputs may not contain Ty::Never",
                });
            }
        }
    }
    Ok(())
}

/// Validates `sig_c_abi_lowerings` against §3.3 step 5 + §14.5.
pub(crate) fn validate_c_abi_lowerings(model: &RegseqModel) -> Result<(), InvariantViolation> {
    if model.sig_c_abi_lowerings.len() != model.sigs.len() {
        return Err(InvariantViolation::CAbiLoweringMalformed {
            sig: FnSigId::from_u32(0),
            reason: "sig_c_abi_lowerings.len() != sigs.len()",
        });
    }

    for (sig_id, sig) in model.sigs.iter() {
        let lowering = &model.sig_c_abi_lowerings[sig_id];
        match (sig.abi, lowering) {
            (FnAbi::C, None) => {
                return Err(InvariantViolation::CAbiLoweringMalformed {
                    sig: sig_id,
                    reason: "FnAbi::C signature requires Some(CAbiLowering)",
                });
            }
            (FnAbi::Rust, Some(_)) => {
                return Err(InvariantViolation::CAbiLoweringMalformed {
                    sig: sig_id,
                    reason: "FnAbi::Rust signature requires None lowering",
                });
            }
            (FnAbi::C, Some(lowering)) => {
                if lowering.inputs.len() != sig.inputs.len() {
                    return Err(InvariantViolation::CAbiLoweringMalformed {
                        sig: sig_id,
                        reason: "lowering.inputs.len() != FnSig.inputs.len()",
                    });
                }
                for shape in &lowering.inputs {
                    validate_c_abi_shape(model, sig_id, shape)?;
                }
                validate_c_abi_shape(model, sig_id, &lowering.output)?;
            }
            (FnAbi::Rust, None) => {}
        }
    }

    Ok(())
}

fn validate_c_abi_shape(
    model: &RegseqModel,
    sig_id: FnSigId,
    shape: &CAbiShape,
) -> Result<(), InvariantViolation> {
    if let CAbiShape::Substituted { emit_as, reason } = shape {
        if emit_as.index() >= model.types.len() {
            return Err(InvariantViolation::CAbiLoweringMalformed {
                sig: sig_id,
                reason: "Substituted.emit_as is out of range",
            });
        }
        if matches!(
            reason,
            SubstReason::TagScalar | SubstReason::TransparentChainAndTagScalar
        ) {
            match &model.types[*emit_as] {
                Ty::Int(_) | Ty::Uint(_) => {}
                _ => {
                    return Err(InvariantViolation::CAbiLoweringMalformed {
                        sig: sig_id,
                        reason: "TagScalar emit_as must resolve to Ty::Int(_) or Ty::Uint(_)",
                    });
                }
            }
        }
    }
    Ok(())
}

/// Validates layout sanity (§11.5 / §12.5) for every `Ty` that
/// carries an inline `Layout` plus array overflow rules per
/// `spec.md:4089-4090`.
pub(crate) fn validate_layouts(model: &RegseqModel) -> Result<(), InvariantViolation> {
    for (id, ty) in model.types.iter() {
        match ty {
            Ty::Tuple { fields, layout } => {
                validate_tuple_layout(model, id, fields, layout)?;
            }
            Ty::Struct { layout, .. } => {
                validate_struct_kind_layout(model, id, layout)?;
            }
            Ty::Union { layout, .. } => {
                validate_union_layout(model, id, layout)?;
            }
            Ty::Enum { layout, .. } => {
                validate_enum_layout(model, id, layout)?;
            }
            Ty::Array(elem, len) => {
                validate_array_layout(model, id, *elem, *len)?;
            }
            _ => {}
        }
    }
    Ok(())
}

fn validate_tuple_layout(
    model: &RegseqModel,
    ty: TyId,
    fields: &[TyId],
    layout: &Layout,
) -> Result<(), InvariantViolation> {
    let field_layouts = match &layout.kind {
        LayoutKind::Struct { fields } => fields,
        LayoutKind::Union { .. } | LayoutKind::Enum { .. } => {
            return Err(InvariantViolation::LayoutMalformed {
                ty,
                reason: "Tuple layout must use LayoutKind::Struct",
            });
        }
    };
    if field_layouts.len() != fields.len() {
        return Err(InvariantViolation::LayoutMalformed {
            ty,
            reason: "Tuple field layout count does not match Ty::Tuple.fields",
        });
    }
    for (idx, field) in field_layouts.iter().enumerate() {
        if field.name.index() >= model.strings.len() {
            return Err(InvariantViolation::LayoutMalformed {
                ty,
                reason: "Tuple FieldLayout.name is out of range",
            });
        }
        let expected = format!("_{idx}");
        if model.strings.resolve(field.name) != expected {
            return Err(InvariantViolation::LayoutMalformed {
                ty,
                reason: "Tuple FieldLayout.name does not follow _N convention",
            });
        }
    }
    validate_field_layouts(model, ty, layout, field_layouts, false)
}

fn validate_array_layout(
    model: &RegseqModel,
    ty: TyId,
    elem: TyId,
    len: u64,
) -> Result<(), InvariantViolation> {
    if elem.index() >= model.types.len() {
        return Err(InvariantViolation::LayoutMalformed {
            ty,
            reason: "Array element TyId is out of range",
        });
    }
    if is_unsized_marker(model, elem) {
        return Err(InvariantViolation::LayoutMalformed {
            ty,
            reason: "Array element resolves to an unsized marker",
        });
    }
    let elem_layout = compute_size_align(model, elem)?;
    let stride = round_up_to_multiple(elem_layout.size, elem_layout.align.max(1));
    let size = stride
        .checked_mul(len)
        .ok_or(InvariantViolation::LayoutMalformed {
            ty,
            reason: "Array stride * length overflows u64",
        })?;
    let max_usize = target_usize_max(model.target.pointer_width);
    if size > max_usize {
        return Err(InvariantViolation::LayoutMalformed {
            ty,
            reason: "Array layout size exceeds target usize::MAX",
        });
    }
    Ok(())
}

fn target_usize_max(pointer_width: u8) -> u64 {
    if pointer_width >= 64 {
        u64::MAX
    } else {
        (1u64 << pointer_width) - 1
    }
}

fn validate_struct_kind_layout(
    model: &RegseqModel,
    ty: TyId,
    layout: &Layout,
) -> Result<(), InvariantViolation> {
    let fields = match &layout.kind {
        LayoutKind::Struct { fields } => fields,
        LayoutKind::Union { .. } | LayoutKind::Enum { .. } => {
            return Err(InvariantViolation::LayoutMalformed {
                ty,
                reason: "Struct/Tuple layout must use LayoutKind::Struct",
            });
        }
    };
    validate_field_layouts(model, ty, layout, fields, false)
}

fn validate_union_layout(
    model: &RegseqModel,
    ty: TyId,
    layout: &Layout,
) -> Result<(), InvariantViolation> {
    let fields = match &layout.kind {
        LayoutKind::Union { fields } => fields,
        LayoutKind::Struct { .. } | LayoutKind::Enum { .. } => {
            return Err(InvariantViolation::LayoutMalformed {
                ty,
                reason: "Union layout must use LayoutKind::Union",
            });
        }
    };
    for field in fields {
        if field.offset != 0 {
            return Err(InvariantViolation::LayoutMalformed {
                ty,
                reason: "Union FieldLayout.offset must be 0",
            });
        }
    }
    validate_field_layouts(model, ty, layout, fields, true)
}

fn validate_enum_layout(
    model: &RegseqModel,
    ty: TyId,
    layout: &Layout,
) -> Result<(), InvariantViolation> {
    let shape = match &layout.kind {
        LayoutKind::Enum { shape } => shape,
        LayoutKind::Struct { .. } | LayoutKind::Union { .. } => {
            return Err(InvariantViolation::LayoutMalformed {
                ty,
                reason: "Enum layout must use LayoutKind::Enum",
            });
        }
    };

    match shape {
        EnumShape::Empty => Ok(()),
        EnumShape::Single { variant, scalar } => {
            validate_variant_layout(model, ty, layout, variant)?;
            if let Some(scalar) = scalar {
                validate_tag_scalar_width_fit(
                    model,
                    ty,
                    scalar,
                    std::iter::once(variant)
                        .filter_map(|v| v.tag_value)
                        .collect::<Vec<_>>(),
                )?;
            }
            Ok(())
        }
        EnumShape::Multiple {
            tag,
            encoding,
            variants,
        } => {
            for variant in variants {
                validate_variant_layout(model, ty, layout, variant)?;
            }
            let mut tag_values: Vec<u128> = variants.iter().filter_map(|v| v.tag_value).collect();
            if let crate::layout::TagEncoding::Niche { niche_start, .. } = encoding {
                tag_values.push(*niche_start);
            }
            validate_tag_scalar_width_fit(model, ty, tag, tag_values)?;
            Ok(())
        }
    }
}

fn validate_variant_layout(
    model: &RegseqModel,
    ty: TyId,
    layout: &Layout,
    variant: &VariantLayout,
) -> Result<(), InvariantViolation> {
    validate_field_layouts(model, ty, layout, &variant.fields, false)
}

fn validate_field_layouts(
    model: &RegseqModel,
    ty: TyId,
    layout: &Layout,
    fields: &[FieldLayout],
    is_union: bool,
) -> Result<(), InvariantViolation> {
    let mut non_zst_intervals: Vec<(u64, u64)> = Vec::new();
    for field in fields {
        if field.ty.index() >= model.types.len() {
            return Err(InvariantViolation::LayoutMalformed {
                ty,
                reason: "FieldLayout.ty is out of range",
            });
        }
        if is_unsized_marker(model, field.ty) {
            return Err(InvariantViolation::LayoutMalformed {
                ty,
                reason: "FieldLayout.ty resolves to an unsized marker",
            });
        }
        let field_size_align = compute_size_align(model, field.ty)?;
        let end = field.offset.checked_add(field_size_align.size).ok_or(
            InvariantViolation::LayoutMalformed {
                ty,
                reason: "FieldLayout offset + size overflows u64",
            },
        )?;
        if end > layout.size {
            return Err(InvariantViolation::LayoutMalformed {
                ty,
                reason: "FieldLayout offset + size exceeds layout.size",
            });
        }
        let required_align = match layout.pack {
            None => field_size_align.align,
            Some(n) => core::cmp::min(field_size_align.align, u64::from(n)),
        };
        if required_align == 0 {
            return Err(InvariantViolation::LayoutMalformed {
                ty,
                reason: "FieldLayout required alignment is zero",
            });
        }
        if field.offset % required_align != 0 {
            return Err(InvariantViolation::LayoutMalformed {
                ty,
                reason: "FieldLayout offset is not aligned",
            });
        }
        if !is_union && field_size_align.size > 0 {
            for (lo, hi) in &non_zst_intervals {
                if field.offset < *hi && *lo < end {
                    return Err(InvariantViolation::LayoutMalformed {
                        ty,
                        reason: "Non-ZST fields overlap in layout",
                    });
                }
            }
            non_zst_intervals.push((field.offset, end));
        }
    }
    Ok(())
}

fn validate_tag_scalar_width_fit(
    model: &RegseqModel,
    ty: TyId,
    scalar: &TagScalar,
    extra_values: Vec<u128>,
) -> Result<(), InvariantViolation> {
    if scalar.scalar_ty.index() >= model.types.len() {
        return Err(InvariantViolation::LayoutMalformed {
            ty,
            reason: "TagScalar.scalar_ty is out of range",
        });
    }
    let width_bits = tag_scalar_width_bits(model, scalar.scalar_ty).ok_or(
        InvariantViolation::LayoutMalformed {
            ty,
            reason: "TagScalar.scalar_ty is not an admitted scalar type",
        },
    )?;
    let mut values = vec![scalar.valid_range.start, scalar.valid_range.end];
    values.extend(extra_values);
    for value in values {
        if !value_fits_in_bits(value, width_bits) {
            return Err(InvariantViolation::TagScalarOutOfRange {
                ty,
                value,
                scalar_width_bits: width_bits,
            });
        }
    }
    Ok(())
}

fn tag_scalar_width_bits(model: &RegseqModel, ty: TyId) -> Option<u32> {
    let pointer_width = u32::from(model.target.pointer_width);
    match &model.types[ty] {
        Ty::Bool => Some(8),
        Ty::Char => Some(32),
        Ty::Int(kind) => Some(int_kind_bits(*kind, pointer_width)),
        Ty::Uint(kind) => Some(uint_kind_bits(*kind, pointer_width)),
        Ty::Ref(_, _) | Ty::Ptr(_, _) => Some(pointer_width),
        _ => None,
    }
}

fn int_kind_bits(kind: IntKind, pointer_width: u32) -> u32 {
    match kind {
        IntKind::I8 => 8,
        IntKind::I16 => 16,
        IntKind::I32 => 32,
        IntKind::I64 => 64,
        IntKind::I128 => 128,
        IntKind::Isize => pointer_width,
    }
}

fn uint_kind_bits(kind: UintKind, pointer_width: u32) -> u32 {
    match kind {
        UintKind::U8 => 8,
        UintKind::U16 => 16,
        UintKind::U32 => 32,
        UintKind::U64 => 64,
        UintKind::U128 => 128,
        UintKind::Usize => pointer_width,
    }
}

fn value_fits_in_bits(value: u128, bits: u32) -> bool {
    if bits >= 128 {
        true
    } else {
        let max = (1u128 << bits) - 1;
        value <= max
    }
}

/// Validates invariant 14 (`InstanceKind` layout consistency).
pub(crate) fn validate_instance_layouts(model: &RegseqModel) -> Result<(), InvariantViolation> {
    for (id, instance) in model.instances.iter().enumerate() {
        let instance_id = InstanceId::from_u32(id as u32);
        let (layout_ty, expected) = match &instance.kind {
            InstanceKind::Struct { layout } => (*layout, "Ty::Struct"),
            InstanceKind::Enum { layout } => (*layout, "Ty::Enum"),
            InstanceKind::Union { layout } => (*layout, "Ty::Union"),
            _ => continue,
        };
        let ty = &model.types[layout_ty];
        let (kind_match, item, args) = match (expected, ty) {
            ("Ty::Struct", Ty::Struct { item, args, .. }) => (true, *item, args),
            ("Ty::Enum", Ty::Enum { item, args, .. }) => (true, *item, args),
            ("Ty::Union", Ty::Union { item, args, .. }) => (true, *item, args),
            _ => (false, ItemId::from_u32(0), &Vec::<GenericArg>::new()),
        };
        if !kind_match {
            return Err(InvariantViolation::InstanceLayoutMismatch {
                instance: instance_id,
                expected_kind: expected,
                found_kind: ty_variant_name(ty),
            });
        }
        if item != instance.item || args != &instance.generic_args {
            return Err(InvariantViolation::InstanceLayoutItemArgsMismatch {
                instance: instance_id,
            });
        }
    }
    Ok(())
}

fn ty_variant_name(ty: &Ty) -> &'static str {
    match ty {
        Ty::Bool => "Ty::Bool",
        Ty::Char => "Ty::Char",
        Ty::Int(_) => "Ty::Int",
        Ty::Uint(_) => "Ty::Uint",
        Ty::Float(_) => "Ty::Float",
        Ty::Str => "Ty::Str",
        Ty::Never => "Ty::Never",
        Ty::Array(_, _) => "Ty::Array",
        Ty::Slice(_) => "Ty::Slice",
        Ty::Ref(_, _) => "Ty::Ref",
        Ty::Ptr(_, _) => "Ty::Ptr",
        Ty::FnDef(_, _) => "Ty::FnDef",
        Ty::FnPtr(_) => "Ty::FnPtr",
        Ty::Dynamic { .. } => "Ty::Dynamic",
        Ty::Closure(_, _) => "Ty::Closure",
        Ty::Tuple { .. } => "Ty::Tuple",
        Ty::Struct { .. } => "Ty::Struct",
        Ty::Enum { .. } => "Ty::Enum",
        Ty::Union { .. } => "Ty::Union",
    }
}

/// Validates const-graph acyclicity per §3.3 step 9 with three-state
/// DFS marks.
pub(crate) fn validate_const_graph_acyclic(model: &RegseqModel) -> Result<(), InvariantViolation> {
    let len = model.consts.len();
    let mut state = vec![DfsMark::Unseen; len];
    let mut stack_path: Vec<ConstId> = Vec::new();
    for (id, _) in model.consts.iter() {
        if state[id.index()] == DfsMark::Unseen {
            dfs_visit(model, id, &mut state, &mut stack_path, id)?;
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DfsMark {
    Unseen,
    Visiting,
    Done,
}

fn dfs_visit(
    model: &RegseqModel,
    node: ConstId,
    state: &mut [DfsMark],
    stack_path: &mut Vec<ConstId>,
    entry_point: ConstId,
) -> Result<(), InvariantViolation> {
    state[node.index()] = DfsMark::Visiting;
    stack_path.push(node);
    let children = const_recursion_children(&model.consts[node]);
    for child in children {
        if child.index() >= model.consts.len() {
            continue;
        }
        match state[child.index()] {
            DfsMark::Unseen => {
                dfs_visit(model, child, state, stack_path, entry_point)?;
            }
            DfsMark::Visiting => {
                let cycle_start = stack_path
                    .iter()
                    .position(|node| *node == child)
                    .unwrap_or(0);
                let mut cycle = stack_path[cycle_start..].to_vec();
                cycle.push(child);
                return Err(InvariantViolation::ConstGraphCycle { entry_point, cycle });
            }
            DfsMark::Done => {}
        }
    }
    stack_path.pop();
    state[node.index()] = DfsMark::Done;
    Ok(())
}

fn const_recursion_children(value: &Const) -> Vec<ConstId> {
    match value {
        Const::Aggregate(children) => children.clone(),
        Const::EnumValue { payload, .. } => payload.clone(),
        Const::Bool(_)
        | Const::Char(_)
        | Const::Int(_, _)
        | Const::Uint(_, _)
        | Const::Float(_, _)
        | Const::ZST
        | Const::Bytes(_)
        | Const::NullPtr(_)
        | Const::Reloc(_) => Vec::new(),
    }
}

/// Validates `domain_attrs` per §3.6.5.
pub(crate) fn validate_domain_attrs(model: &RegseqModel) -> Result<(), InvariantViolation> {
    if model.domain_attrs.item_attrs.len() != model.items.len() {
        return Err(InvariantViolation::DomainAttrMalformed {
            reason: "domain_attrs.item_attrs.len() != model.items.len()",
        });
    }

    for (item_id, entries) in model.domain_attrs.item_attrs.iter().enumerate() {
        let item_id = ItemId::from_u32(item_id as u32);
        let mut intrinsic_count = 0usize;
        for entry in entries {
            validate_attr_data_payload(model, entry)?;
            if matches!(entry, AttrData::Intrinsic(_)) {
                intrinsic_count += 1;
            }
        }
        if intrinsic_count > 1 {
            let _ = item_id;
            return Err(InvariantViolation::DomainAttrMalformed {
                reason: "I-DomainAttr-Intrinsic-Singleton: at most one Intrinsic AttrData per Item",
            });
        }
    }

    for (path, entries) in model.domain_attrs.field_attrs.iter() {
        validate_field_path(model, path)?;
        for entry in entries {
            validate_attr_data_payload(model, entry)?;
        }
    }

    for (span, entries) in model.domain_attrs.span_attrs.iter() {
        if span.file.index() >= model.source_files.len() {
            return Err(InvariantViolation::DomainAttrMalformed {
                reason: "span_attrs key file id is out of range",
            });
        }
        if span.lo > span.hi {
            return Err(InvariantViolation::DomainAttrMalformed {
                reason: "span_attrs key lo > hi",
            });
        }
        for entry in entries {
            validate_attr_data_payload(model, entry)?;
        }
    }

    Ok(())
}

fn validate_attr_data_payload(
    model: &RegseqModel,
    entry: &AttrData,
) -> Result<(), InvariantViolation> {
    match entry {
        AttrData::Intrinsic(_) => Ok(()),
        // The framework-type marker carries a closed enum with no nested
        // model IDs, so there is nothing to range-check here.
        AttrData::FrameworkType(_) => Ok(()),
        // The atomic field marker carries a closed `AtomicElement` enum and an
        // optional `i128` literal — no nested model IDs to range-check.
        AttrData::Atomic { .. } => Ok(()),
        // The interrupt-resource marker carries the interned resource id;
        // range-check it against the string table.
        AttrData::InterruptResource { id, .. } => {
            if id.index() >= model.strings.len() {
                return Err(InvariantViolation::DomainAttrMalformed {
                    reason: "InterruptResource id StringId is out of range",
                });
            }
            Ok(())
        }
        // The ISR-binding marker's optional explicit field target is interned;
        // range-check it when present.
        AttrData::IsrBinding { field, .. } => {
            if let Some(field) = field {
                if field.index() >= model.strings.len() {
                    return Err(InvariantViolation::DomainAttrMalformed {
                        reason: "IsrBinding field StringId is out of range",
                    });
                }
            }
            Ok(())
        }
        // The sync-resource marker carries a closed `SyncKind` enum with no
        // nested model IDs, so there is nothing to range-check here.
        AttrData::SyncResource { .. } => Ok(()),
    }
}

fn validate_field_path(model: &RegseqModel, path: &FieldPath) -> Result<(), InvariantViolation> {
    if path.item.index() >= model.items.len() {
        return Err(InvariantViolation::DomainAttrMalformed {
            reason: "field_attrs key item is out of range",
        });
    }
    let item = &model.items[path.item];
    match item.kind {
        ItemKind::Struct | ItemKind::Union => {
            if path.variant.as_u32() != 0 {
                return Err(InvariantViolation::DomainAttrMalformed {
                    reason: "Struct/Union FieldPath.variant must be 0",
                });
            }
            let layout = nominal_layout_for_item(model, path.item).ok_or(
                InvariantViolation::DomainAttrMalformed {
                    reason: "field_attrs key item has no monomorphic layout",
                },
            )?;
            let fields = match &layout.kind {
                LayoutKind::Struct { fields } | LayoutKind::Union { fields } => fields,
                LayoutKind::Enum { .. } => {
                    return Err(InvariantViolation::DomainAttrMalformed {
                        reason: "Struct/Union FieldPath references enum layout",
                    });
                }
            };
            if (path.field.index()) >= fields.len() {
                return Err(InvariantViolation::DomainAttrMalformed {
                    reason: "field_attrs FieldIdx is out of range for Struct/Union",
                });
            }
        }
        ItemKind::Enum => {
            let layout = nominal_layout_for_item(model, path.item).ok_or(
                InvariantViolation::DomainAttrMalformed {
                    reason: "field_attrs key item has no monomorphic enum layout",
                },
            )?;
            let shape = match &layout.kind {
                LayoutKind::Enum { shape } => shape,
                LayoutKind::Struct { .. } | LayoutKind::Union { .. } => {
                    return Err(InvariantViolation::DomainAttrMalformed {
                        reason: "Enum FieldPath references non-enum layout",
                    });
                }
            };
            let variants: &[VariantLayout] = match shape {
                EnumShape::Empty => &[],
                EnumShape::Single { variant, .. } => core::slice::from_ref(variant),
                EnumShape::Multiple { variants, .. } => variants.as_slice(),
            };
            if path.variant.index() >= variants.len() {
                return Err(InvariantViolation::DomainAttrMalformed {
                    reason: "Enum FieldPath.variant is out of range",
                });
            }
            let variant = &variants[path.variant.index()];
            if path.field.index() >= variant.fields.len() {
                return Err(InvariantViolation::DomainAttrMalformed {
                    reason: "Enum FieldPath.field is out of range",
                });
            }
        }
        ItemKind::Function | ItemKind::Static { .. } | ItemKind::Const => {
            return Err(InvariantViolation::DomainAttrMalformed {
                reason: "field_attrs key item is not an ADT",
            });
        }
    }
    Ok(())
}

fn nominal_layout_for_item(model: &RegseqModel, item: ItemId) -> Option<&Layout> {
    for (_, ty) in model.types.iter() {
        match ty {
            Ty::Struct {
                item: ty_item,
                layout,
                ..
            }
            | Ty::Enum {
                item: ty_item,
                layout,
                ..
            }
            | Ty::Union {
                item: ty_item,
                layout,
                ..
            } if *ty_item == item => return Some(layout),
            _ => {}
        }
    }
    None
}

/// Validates the canonical `UNIT_LAYOUT` rule on every empty tuple.
pub(crate) fn validate_unit_layout(model: &RegseqModel) -> Result<(), InvariantViolation> {
    for (id, ty) in model.types.iter() {
        if let Ty::Tuple { fields, layout } = ty {
            if fields.is_empty() && layout != &UNIT_LAYOUT {
                return Err(InvariantViolation::UnitLayoutMismatch { ty: id });
            }
        }
    }
    Ok(())
}

/// Validates invariant 19 (model-checkable subset) for every Item
/// flagged `opaque_handle_eligible`.
pub(crate) fn validate_opaque_handle_eligibility(
    model: &RegseqModel,
) -> Result<(), InvariantViolation> {
    for (item_idx, item) in model.items.iter().enumerate() {
        if !item.opaque_handle_eligible {
            continue;
        }
        let item_id = ItemId::from_u32(item_idx as u32);

        if item.kind != ItemKind::Struct {
            return Err(InvariantViolation::OpaqueHandleInconsistent {
                item: item_id,
                reason: "opaque_handle_eligible requires ItemKind::Struct",
            });
        }

        let mut matching: Vec<(TyId, &Vec<GenericArg>, &Layout)> = Vec::new();
        for (ty_id, ty) in model.types.iter() {
            if let Ty::Struct {
                item: struct_item,
                args,
                layout,
            } = ty
            {
                if *struct_item == item_id {
                    matching.push((ty_id, args, layout));
                }
            }
        }

        if matching.len() != 1 {
            return Err(InvariantViolation::OpaqueHandleInconsistent {
                item: item_id,
                reason: "opaque_handle_eligible requires exactly one Ty::Struct entry",
            });
        }
        let (mono_ty, args, layout) = matching[0];
        if !args.is_empty() {
            return Err(InvariantViolation::OpaqueHandleInconsistent {
                item: item_id,
                reason: "opaque_handle_eligible requires non-generic args",
            });
        }
        if layout.size != 0 || layout.align != 1 {
            return Err(InvariantViolation::OpaqueHandleInconsistent {
                item: item_id,
                reason: "opaque_handle_eligible requires ZST monomorphic layout",
            });
        }

        if has_by_value_use(model, mono_ty) {
            return Err(InvariantViolation::OpaqueHandleInconsistent {
                item: item_id,
                reason: "opaque_handle_eligible requires every reachable use behind Ref/Ptr",
            });
        }
    }
    Ok(())
}

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

fn shape_uses_by_value(shape: &CAbiShape, target: TyId) -> bool {
    match shape {
        CAbiShape::Direct => false,
        CAbiShape::Substituted { emit_as, .. } => *emit_as == target,
    }
}

fn const_uses_by_value(value: &Const, target: TyId) -> bool {
    matches!(value, Const::NullPtr(t) if *t == target)
}

/// Validates invariant 20 (`StableCrateId(0)` synthetic-domain).
pub(crate) fn validate_stable_crate_id_zero(model: &RegseqModel) -> Result<(), InvariantViolation> {
    for (idx, info) in model.crates.iter().enumerate() {
        if info.stable_crate_id == StableCrateId(0) {
            return Err(InvariantViolation::StableCrateIdZeroCollision {
                crate_id: CrateId::from_u32(idx as u32),
            });
        }
    }

    for (idx, hash) in model.item_def_paths.iter().enumerate() {
        let item_id = ItemId::from_u32(idx as u32);
        let kind = &model.item_synthetic_kinds[item_id];
        let is_source_derived = matches!(
            kind,
            SyntheticItemKind::None | SyntheticItemKind::ClosureCaptures { .. }
        );
        if is_source_derived {
            if hash.stable_crate_id == StableCrateId(0) {
                return Err(InvariantViolation::SyntheticDomainBoundary {
                    item: item_id,
                    reason: "source-derived item carries DefPathHash.stable_crate_id == 0",
                });
            }
        } else if hash.stable_crate_id != StableCrateId(0) {
            return Err(InvariantViolation::SyntheticDomainBoundary {
                item: item_id,
                reason: "synthetic item carries DefPathHash.stable_crate_id != 0",
            });
        }
    }

    Ok(())
}

/// Validates source-file shape: synthesised sentinel preserved by the
/// extended walk, line-table consistency for non-sentinel entries,
/// and no two source files share the same `path` `StringId`.
pub(crate) fn validate_source_files(model: &RegseqModel) -> Result<(), InvariantViolation> {
    for (idx, file) in model.source_files.iter().enumerate() {
        let file_id = SourceFileId::from_u32(idx as u32);
        if file_id == SourceFileId::SYNTHESIZED {
            continue;
        }
        if file.line_starts.first().copied() != Some(0) {
            return Err(InvariantViolation::SourceFileMalformed {
                file: file_id,
                reason: "line_starts does not start at 0",
            });
        }
        if file.line_starts.windows(2).any(|pair| pair[0] > pair[1]) {
            return Err(InvariantViolation::SourceFileMalformed {
                file: file_id,
                reason: "line_starts is not monotonic",
            });
        }
        if let Some(content) = file.content.as_ref() {
            let limit = content.len() as u64;
            if file
                .line_starts
                .iter()
                .any(|start| u64::from(*start) > limit)
            {
                return Err(InvariantViolation::SourceFileMalformed {
                    file: file_id,
                    reason: "line_starts entry exceeds content length",
                });
            }
        }
    }

    let mut seen: HashMap<crate::ids::StringId, SourceFileId> = HashMap::new();
    for (idx, file) in model.source_files.iter().enumerate() {
        let file_id = SourceFileId::from_u32(idx as u32);
        if let Some(prev) = seen.insert(file.path, file_id) {
            return Err(InvariantViolation::DuplicateSourceFilePath {
                first: prev,
                second: file_id,
                path: file.path,
            });
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Debug)]
struct SizeAlign {
    size: u64,
    align: u64,
}

fn compute_size_align(model: &RegseqModel, ty: TyId) -> Result<SizeAlign, InvariantViolation> {
    match &model.types[ty] {
        Ty::Bool => Ok(scalar_size_align(model.target.scalar_layouts.bool_)),
        Ty::Char => Ok(scalar_size_align(model.target.scalar_layouts.char_)),
        Ty::Int(kind) => Ok(scalar_size_align(
            model.target.scalar_layouts.int[*kind as usize],
        )),
        Ty::Uint(kind) => Ok(scalar_size_align(
            model.target.scalar_layouts.uint[*kind as usize],
        )),
        Ty::Float(kind) => Ok(scalar_size_align(
            model.target.scalar_layouts.float[*kind as usize],
        )),
        Ty::FnPtr(_) => Ok(scalar_size_align(model.target.scalar_layouts.fn_ptr)),
        Ty::FnDef(_, _) | Ty::Never => Ok(SizeAlign { size: 0, align: 1 }),
        Ty::Ref(pointee, _) | Ty::Ptr(pointee, _) => {
            let data_ptr = model.target.scalar_layouts.data_ptr;
            if is_unsized_marker(model, *pointee) {
                Ok(SizeAlign {
                    size: data_ptr.size.saturating_mul(2),
                    align: data_ptr.align,
                })
            } else {
                Ok(scalar_size_align(data_ptr))
            }
        }
        Ty::Array(elem, len) => {
            let inner = compute_size_align(model, *elem)?;
            let stride = round_up_to_multiple(inner.size, inner.align);
            let size = stride
                .checked_mul(*len)
                .ok_or(InvariantViolation::LayoutMalformed {
                    ty,
                    reason: "Array stride * length overflows u64",
                })?;
            Ok(SizeAlign {
                size,
                align: inner.align,
            })
        }
        Ty::Struct { layout, .. }
        | Ty::Enum { layout, .. }
        | Ty::Union { layout, .. }
        | Ty::Tuple { layout, .. } => Ok(SizeAlign {
            size: layout.size,
            align: layout.align,
        }),
        Ty::Closure(item, args) => model
            .types
            .iter()
            .find_map(|(_, candidate)| match candidate {
                Ty::Struct {
                    item: struct_item,
                    args: struct_args,
                    layout,
                } if struct_item == item && struct_args == args => Some(SizeAlign {
                    size: layout.size,
                    align: layout.align,
                }),
                _ => None,
            })
            .ok_or(InvariantViolation::LayoutMalformed {
                ty,
                reason: "Closure capture struct layout not found",
            }),
        Ty::Str | Ty::Slice(_) | Ty::Dynamic { .. } => Err(InvariantViolation::LayoutMalformed {
            ty,
            reason: "unsized marker has no inherent size",
        }),
    }
}

fn scalar_size_align(entry: crate::target::SizeAlign) -> SizeAlign {
    SizeAlign {
        size: entry.size,
        align: entry.align,
    }
}

fn round_up_to_multiple(value: u64, boundary: u64) -> u64 {
    if boundary == 0 {
        return value;
    }
    let rem = value % boundary;
    if rem == 0 {
        value
    } else {
        value.saturating_add(boundary - rem)
    }
}

fn is_unsized_marker(model: &RegseqModel, ty: TyId) -> bool {
    matches!(
        &model.types[ty],
        Ty::Str | Ty::Slice(_) | Ty::Dynamic { .. }
    )
}

fn is_populated(value: crate::target::SizeAlign) -> bool {
    value.size > 0 && value.align > 0
}

struct ParsedDataLayout {
    endian: Option<Endian>,
    pointer_width: Option<u8>,
}

fn parse_data_layout(text: &str) -> Result<ParsedDataLayout, InvariantViolation> {
    let mut endian = None;
    let mut pointer_width = None;
    for token in text.split('-') {
        if token.is_empty() {
            continue;
        }
        match token {
            "e" => endian = Some(Endian::Little),
            "E" => endian = Some(Endian::Big),
            _ => {
                if let Some(rest) = token.strip_prefix('p') {
                    let body = rest
                        .split_once(':')
                        .map(|(_, body)| body)
                        .unwrap_or_else(|| rest.trim_start_matches(|c: char| c.is_ascii_digit()));
                    let size_part = body.split(':').next().unwrap_or("").trim();
                    if let Ok(size_bits) = size_part.parse::<u32>() {
                        pointer_width = u8::try_from(size_bits).ok();
                    }
                }
            }
        }
    }
    Ok(ParsedDataLayout {
        endian,
        pointer_width,
    })
}
