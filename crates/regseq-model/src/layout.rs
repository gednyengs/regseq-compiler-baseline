//! Resolved layout records preserved from rustc.
//!
//! The bridge records rustc's resolved size, alignment, field
//! offsets, and enum tag shape here. RegseqModel preserves these
//! answers; it does not recompute layout.

use serde::{Deserialize, Serialize};

use crate::ids::{StringId, TyId, VariantIdx};

/// Resolved aggregate layout.
///
/// SERDE: derived construction shape pending the later
/// resolution-time refinement.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Layout {
    /// ABI size in bytes.
    pub size: u64,
    /// ABI alignment in bytes.
    pub align: u64,
    /// `None` for ordinary layout; `Some(n)` for `repr(packed(n))`.
    pub pack: Option<u32>,
    /// True for uninhabited layouts.
    pub uninhabited: bool,
    /// Layout-specific aggregate shape.
    pub kind: LayoutKind,
}

/// Aggregate layout shape.
///
/// SERDE: derived construction shape pending the later
/// resolution-time refinement.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LayoutKind {
    /// Struct fields, also used for tuples. Field vector order is
    /// source or MIR field index order, not memory order.
    Struct {
        /// Source or MIR field-indexed fields.
        fields: Vec<FieldLayout>,
    },
    /// Union fields. Field offsets are all zero by construction.
    Union {
        /// Source or MIR field-indexed fields.
        fields: Vec<FieldLayout>,
    },
    /// Enum layout shape.
    Enum {
        /// Resolved enum shape.
        shape: EnumShape,
    },
}

/// Resolved field layout.
///
/// SERDE: derived construction shape pending the later
/// resolution-time refinement.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FieldLayout {
    /// Interned source or synthetic field name.
    pub name: StringId,
    /// Resolved field type.
    pub ty: TyId,
    /// Byte offset from the start of the containing aggregate.
    ///
    /// For enum variant payload fields, the containing aggregate is
    /// the whole enum object, not the variant payload start.
    pub offset: u64,
}

/// Canonical layout for Rust unit `()`.
///
/// Validation later enforces that `Ty::Tuple { fields: [] }` carries
/// exactly this layout.
pub const UNIT_LAYOUT: Layout = Layout {
    size: 0,
    align: 1,
    pack: None,
    uninhabited: false,
    kind: LayoutKind::Struct { fields: Vec::new() },
};

/// Resolved enum shape.
///
/// SERDE: derived construction shape pending the later
/// resolution-time refinement.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EnumShape {
    /// Zero-variant uninhabited enum.
    Empty,
    /// Single-variant enum. `scalar` is `Some` for fieldless
    /// `repr(int)` enums and `None` for ordinary single-variant
    /// enums with no tag bytes.
    Single {
        /// The only variant.
        variant: VariantLayout,
        /// Optional tag scalar for fieldless `repr(int)` enums.
        scalar: Option<TagScalar>,
    },
    /// Multiple-variant enum.
    Multiple {
        /// Tag scalar storage.
        tag: TagScalar,
        /// Direct or niche tag encoding.
        encoding: TagEncoding,
        /// Variants indexed by `VariantIdx`.
        variants: Vec<VariantLayout>,
    },
}

/// Resolved layout for one enum variant.
///
/// SERDE: derived construction shape pending the later
/// resolution-time refinement.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct VariantLayout {
    /// Interned variant name.
    pub name: StringId,
    /// Payload fields indexed by source or MIR field index.
    pub fields: Vec<FieldLayout>,
    /// True if this variant is uninhabited.
    pub uninhabited: bool,
    /// Rust-level semantic discriminant bit pattern.
    ///
    /// Signed repr values are stored as the unsigned reinterpretation
    /// of their two's-complement bits at the tag scalar width.
    pub discriminant: u128,
    /// Layout-level tag bytes written by `SetDiscriminant`.
    ///
    /// Semantics: ordinary single-variant enums without a scalar use
    /// `None`; single-variant fieldless `repr(int)` enums use
    /// `Some(discriminant)`; direct-encoded multiple variants use
    /// `Some(discriminant)`; niche-encoded niched variants use
    /// `Some(value)` from the niche formula; the niche-encoded
    /// untagged variant uses `None` because its payload is the tag.
    pub tag_value: Option<u128>,
}

/// Scalar cell used for enum tags.
///
/// `valid_range`, `niche_start`, and every variant `tag_value` must
/// fit the bit width of `scalar_ty`. Validation enforces this later.
///
/// SERDE: derived construction shape pending the later
/// resolution-time refinement.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TagScalar {
    /// Byte offset of the tag cell in the containing enum object.
    pub offset: u64,
    /// Logical scalar type for the tag.
    ///
    /// Admitted resolved variants are `Bool`, `Char`, `Int(_)`,
    /// `Uint(_)`, `Ptr(_, _)`, and `Ref(_, _)`. `Float` is not
    /// admitted.
    pub scalar_ty: TyId,
    /// Inclusive valid range. Always present.
    pub valid_range: WrappingRange,
}

/// Enum tag encoding.
///
/// SERDE: derived construction shape pending the later
/// resolution-time refinement.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TagEncoding {
    /// Direct encoding: each variant writes its discriminant as the
    /// tag value.
    Direct,
    /// Niche encoding.
    ///
    /// For each variant index in `niche_variants`, compute
    /// `raw = niche_start + (variant.index - niche_variants.start)`,
    /// then store `raw mod 2^scalar_width` as that variant's
    /// `tag_value`. The `untagged_variant` has no tag write because
    /// the payload is the tag.
    Niche {
        /// Variant whose payload carries the tag.
        untagged_variant: VariantIdx,
        /// Inclusive range of niched variants.
        niche_variants: VariantRange,
        /// First niche tag value.
        niche_start: u128,
    },
}

/// Inclusive range of enum variants.
///
/// SERDE: derived construction shape pending the later
/// resolution-time refinement.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct VariantRange {
    /// First variant in the range.
    pub start: VariantIdx,
    /// Last variant in the range, inclusive.
    pub end: VariantIdx,
}

/// Inclusive wrapping scalar range.
///
/// If `start > end`, the range wraps around the scalar's bit width.
/// Both endpoints must fit the tag scalar width; validation enforces
/// this later.
///
/// SERDE: derived construction shape pending the later
/// resolution-time refinement.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WrappingRange {
    /// First value in the range.
    pub start: u128,
    /// Last value in the range, inclusive.
    pub end: u128,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::{FnSigId, ItemId};
    use crate::model::RegseqModel;
    use crate::target::{Endian, ScalarLayouts, SizeAlign, TargetInfo};
    use crate::ty::{AutoTrait, Ty};

    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    fn sid(value: u32) -> StringId {
        StringId::from_u32(value)
    }

    fn tid(value: u32) -> TyId {
        TyId::from_u32(value)
    }

    fn iid(value: u32) -> ItemId {
        ItemId::from_u32(value)
    }

    fn fid(value: u32) -> FnSigId {
        FnSigId::from_u32(value)
    }

    fn vid(value: u32) -> VariantIdx {
        VariantIdx::from_u32(value)
    }

    fn field(offset: u64) -> FieldLayout {
        FieldLayout {
            name: sid(1),
            ty: tid(2),
            offset,
        }
    }

    fn wrapping() -> WrappingRange {
        WrappingRange { start: 0, end: 255 }
    }

    fn tag_scalar() -> TagScalar {
        TagScalar {
            offset: 0,
            scalar_ty: tid(3),
            valid_range: wrapping(),
        }
    }

    fn variant(tag_value: Option<u128>) -> VariantLayout {
        VariantLayout {
            name: sid(4),
            fields: vec![field(8)],
            uninhabited: false,
            discriminant: 5,
            tag_value,
        }
    }

    fn layout(size: u64, pack: Option<u32>) -> Layout {
        Layout {
            size,
            align: 4,
            pack,
            uninhabited: false,
            kind: LayoutKind::Struct {
                fields: vec![field(0)],
            },
        }
    }

    fn sized_layout(size: u64, align: u64) -> Layout {
        Layout {
            size,
            align,
            pack: None,
            uninhabited: false,
            kind: LayoutKind::Struct { fields: vec![] },
        }
    }

    fn model() -> RegseqModel {
        let mut model = RegseqModel::with_primary("primary", crate::StableCrateId(1));
        model.target = TargetInfo {
            triple: sid(1),
            pointer_width: 64,
            endian: Endian::Little,
            data_layout: sid(2),
            scalar_layouts: ScalarLayouts {
                bool_: SizeAlign { size: 1, align: 1 },
                char_: SizeAlign { size: 4, align: 4 },
                int: [
                    SizeAlign { size: 11, align: 1 },
                    SizeAlign { size: 12, align: 2 },
                    SizeAlign { size: 13, align: 4 },
                    SizeAlign { size: 14, align: 8 },
                    SizeAlign {
                        size: 15,
                        align: 16,
                    },
                    SizeAlign { size: 16, align: 8 },
                ],
                uint: [
                    SizeAlign { size: 21, align: 1 },
                    SizeAlign { size: 22, align: 2 },
                    SizeAlign { size: 23, align: 4 },
                    SizeAlign { size: 24, align: 8 },
                    SizeAlign {
                        size: 25,
                        align: 16,
                    },
                    SizeAlign { size: 26, align: 8 },
                ],
                float: [
                    SizeAlign { size: 31, align: 2 },
                    SizeAlign { size: 32, align: 4 },
                    SizeAlign { size: 33, align: 8 },
                    SizeAlign {
                        size: 34,
                        align: 16,
                    },
                ],
                fn_ptr: SizeAlign { size: 7, align: 7 },
                data_ptr: SizeAlign { size: 8, align: 8 },
            },
        };
        model
    }

    fn hash<T: Hash>(value: &T) -> u64 {
        let mut hasher = DefaultHasher::new();
        value.hash(&mut hasher);
        hasher.finish()
    }

    fn round_trip<T>(value: T)
    where
        T: serde::Serialize + serde::de::DeserializeOwned + PartialEq + std::fmt::Debug,
    {
        let json = serde_json::to_string(&value).unwrap();
        assert_eq!(serde_json::from_str::<T>(&json).unwrap(), value);
    }

    #[test]
    fn layout_constructs() {
        let value = Layout {
            size: 4,
            align: 4,
            pack: None,
            uninhabited: false,
            kind: LayoutKind::Struct { fields: vec![] },
        };
        assert_eq!(value.size, 4);
        assert_eq!(value.align, 4);
    }

    #[test]
    fn layout_kind_variants_construct() {
        assert!(matches!(
            LayoutKind::Struct {
                fields: vec![field(0)]
            },
            LayoutKind::Struct { .. }
        ));
        assert!(matches!(
            LayoutKind::Union {
                fields: vec![field(0)]
            },
            LayoutKind::Union { .. }
        ));
        assert!(matches!(
            LayoutKind::Enum {
                shape: EnumShape::Empty
            },
            LayoutKind::Enum { .. }
        ));
    }

    #[test]
    fn field_layout_constructs_and_round_trips() {
        let value = field(12);
        assert_eq!(value.offset, 12);
        round_trip(value);
    }

    #[test]
    fn enum_shape_variants_construct() {
        assert!(matches!(EnumShape::Empty, EnumShape::Empty));
        assert!(matches!(
            EnumShape::Single {
                variant: variant(Some(5)),
                scalar: Some(tag_scalar())
            },
            EnumShape::Single {
                scalar: Some(_),
                ..
            }
        ));
        assert!(matches!(
            EnumShape::Single {
                variant: variant(None),
                scalar: None
            },
            EnumShape::Single { scalar: None, .. }
        ));
        assert!(matches!(
            EnumShape::Multiple {
                tag: tag_scalar(),
                encoding: TagEncoding::Direct,
                variants: vec![variant(Some(0))]
            },
            EnumShape::Multiple {
                encoding: TagEncoding::Direct,
                ..
            }
        ));
        assert!(matches!(
            EnumShape::Multiple {
                tag: tag_scalar(),
                encoding: TagEncoding::Niche {
                    untagged_variant: vid(0),
                    niche_variants: VariantRange {
                        start: vid(1),
                        end: vid(2)
                    },
                    niche_start: 3,
                },
                variants: vec![variant(None), variant(Some(3))]
            },
            EnumShape::Multiple {
                encoding: TagEncoding::Niche { .. },
                ..
            }
        ));
    }

    #[test]
    fn variant_layout_constructs_with_optional_tag_value() {
        assert_eq!(variant(Some(7)).tag_value, Some(7));
        assert_eq!(variant(None).tag_value, None);
    }

    #[test]
    fn tag_scalar_constructs() {
        let tag = tag_scalar();
        assert_eq!(tag.offset, 0);
        assert_eq!(tag.scalar_ty, tid(3));
    }

    #[test]
    fn variant_range_constructs_with_inclusive_end() {
        let range = VariantRange {
            start: vid(2),
            end: vid(5),
        };
        assert_eq!(range.end, vid(5));
    }

    #[test]
    fn wrapping_range_constructs_wrapping_case() {
        let range = WrappingRange { start: 5, end: 2 };
        assert!(range.start > range.end);
    }

    #[test]
    fn unit_layout_matches_canonical_fields() {
        assert_eq!(UNIT_LAYOUT.size, 0);
        assert_eq!(UNIT_LAYOUT.align, 1);
        assert_eq!(UNIT_LAYOUT.pack, None);
        assert!(!UNIT_LAYOUT.uninhabited);
        assert!(matches!(
            &UNIT_LAYOUT.kind,
            LayoutKind::Struct { fields } if fields.is_empty()
        ));
        const CONST_UNIT: Layout = UNIT_LAYOUT;
        const CONST_UNIT_TY: Ty = Ty::Tuple {
            fields: Vec::new(),
            layout: UNIT_LAYOUT,
        };
        assert_eq!(CONST_UNIT, UNIT_LAYOUT);
        assert!(matches!(CONST_UNIT_TY, Ty::Tuple { .. }));
        assert_eq!(UNIT_LAYOUT, UNIT_LAYOUT);
    }

    #[test]
    fn equality_and_hash_use_full_values() {
        let field_a = field(4);
        let field_b = field(4);
        assert_eq!(field_a, field_b);
        assert_eq!(hash(&field_a), hash(&field_b));

        let layout_a = layout(4, None);
        let layout_b = layout(4, None);
        assert_eq!(layout_a, layout_b);
        assert_eq!(hash(&layout_a), hash(&layout_b));
        assert_ne!(layout(4, None), layout(8, None));
        assert_ne!(layout(4, None), layout(4, Some(2)));
    }

    #[test]
    fn layout_types_round_trip_through_serde() {
        round_trip(layout(4, None));
        round_trip(LayoutKind::Struct {
            fields: vec![field(0)],
        });
        round_trip(LayoutKind::Union {
            fields: vec![field(0)],
        });
        round_trip(LayoutKind::Enum {
            shape: EnumShape::Empty,
        });
        round_trip(field(0));
        round_trip(EnumShape::Empty);
        round_trip(EnumShape::Single {
            variant: variant(Some(1)),
            scalar: Some(tag_scalar()),
        });
        round_trip(EnumShape::Multiple {
            tag: tag_scalar(),
            encoding: TagEncoding::Direct,
            variants: vec![variant(Some(0))],
        });
        round_trip(EnumShape::Multiple {
            tag: tag_scalar(),
            encoding: TagEncoding::Niche {
                untagged_variant: vid(0),
                niche_variants: VariantRange {
                    start: vid(1),
                    end: vid(2),
                },
                niche_start: 3,
            },
            variants: vec![variant(None), variant(Some(3))],
        });
        round_trip(variant(None));
        round_trip(tag_scalar());
        round_trip(TagEncoding::Direct);
        round_trip(TagEncoding::Niche {
            untagged_variant: vid(0),
            niche_variants: VariantRange {
                start: vid(1),
                end: vid(2),
            },
            niche_start: 3,
        });
        round_trip(VariantRange {
            start: vid(1),
            end: vid(2),
        });
        round_trip(WrappingRange { start: 5, end: 2 });
    }

    #[test]
    fn layout_can_appear_inside_ty_serde() {
        let value = Ty::Tuple {
            fields: vec![tid(0)],
            layout: sized_layout(4, 4),
        };
        round_trip(value);
    }

    #[test]
    fn size_align_of_primitive_scalars_uses_target_tables() {
        let mut model = model();

        for (kind, expected) in [
            (crate::IntKind::I8, SizeAlign { size: 11, align: 1 }),
            (crate::IntKind::I16, SizeAlign { size: 12, align: 2 }),
            (crate::IntKind::I32, SizeAlign { size: 13, align: 4 }),
            (crate::IntKind::I64, SizeAlign { size: 14, align: 8 }),
            (
                crate::IntKind::I128,
                SizeAlign {
                    size: 15,
                    align: 16,
                },
            ),
            (crate::IntKind::Isize, SizeAlign { size: 16, align: 8 }),
        ] {
            let id = model.types.push(Ty::Int(kind));
            assert_eq!(model.size_align_of(id), expected);
        }

        for (kind, expected) in [
            (crate::UintKind::U8, SizeAlign { size: 21, align: 1 }),
            (crate::UintKind::U16, SizeAlign { size: 22, align: 2 }),
            (crate::UintKind::U32, SizeAlign { size: 23, align: 4 }),
            (crate::UintKind::U64, SizeAlign { size: 24, align: 8 }),
            (
                crate::UintKind::U128,
                SizeAlign {
                    size: 25,
                    align: 16,
                },
            ),
            (crate::UintKind::Usize, SizeAlign { size: 26, align: 8 }),
        ] {
            let id = model.types.push(Ty::Uint(kind));
            assert_eq!(model.size_align_of(id), expected);
        }

        for (kind, expected) in [
            (crate::FloatKind::F16, SizeAlign { size: 31, align: 2 }),
            (crate::FloatKind::F32, SizeAlign { size: 32, align: 4 }),
            (crate::FloatKind::F64, SizeAlign { size: 33, align: 8 }),
            (
                crate::FloatKind::F128,
                SizeAlign {
                    size: 34,
                    align: 16,
                },
            ),
        ] {
            let id = model.types.push(Ty::Float(kind));
            assert_eq!(model.size_align_of(id), expected);
        }
    }

    #[test]
    fn size_align_of_simple_builtin_variants() {
        let mut model = model();
        let bool_ty = model.types.push(Ty::Bool);
        let char_ty = model.types.push(Ty::Char);
        let fn_ptr_ty = model.types.push(Ty::FnPtr(fid(0)));
        let fn_def_ty = model.types.push(Ty::FnDef(iid(0), vec![]));
        let never_ty = model.types.push(Ty::Never);

        assert_eq!(
            model.size_align_of(bool_ty),
            SizeAlign { size: 1, align: 1 }
        );
        assert_eq!(
            model.size_align_of(char_ty),
            SizeAlign { size: 4, align: 4 }
        );
        assert_eq!(
            model.size_align_of(fn_ptr_ty),
            SizeAlign { size: 7, align: 7 }
        );
        assert_eq!(
            model.size_align_of(fn_def_ty),
            SizeAlign { size: 0, align: 1 }
        );
        assert_eq!(
            model.size_align_of(never_ty),
            SizeAlign { size: 0, align: 1 }
        );
    }

    #[test]
    fn size_align_of_ref_and_ptr_distinguishes_sized_and_unsized_pointees() {
        let mut model = model();
        let sized = model.types.push(Ty::Bool);
        let str_ty = model.types.push(Ty::Str);
        let slice_ty = model.types.push(Ty::Slice(sized));
        let dyn_ty = model.types.push(Ty::Dynamic {
            auto_traits: vec![AutoTrait::Send],
            vtable_ty: sized,
        });

        for ty in [
            Ty::Ref(sized, crate::Provenance::SharedBorrow),
            Ty::Ref(sized, crate::Provenance::UniqueBorrow),
            Ty::Ptr(sized, crate::Mutability::Not),
            Ty::Ptr(sized, crate::Mutability::Mut),
        ] {
            let id = model.types.push(ty);
            assert_eq!(model.size_align_of(id), SizeAlign { size: 8, align: 8 });
        }

        for ty in [
            Ty::Ref(str_ty, crate::Provenance::SharedBorrow),
            Ty::Ptr(str_ty, crate::Mutability::Not),
            Ty::Ref(slice_ty, crate::Provenance::SharedBorrow),
            Ty::Ptr(slice_ty, crate::Mutability::Mut),
            Ty::Ref(dyn_ty, crate::Provenance::UniqueBorrow),
            Ty::Ptr(dyn_ty, crate::Mutability::Not),
        ] {
            let id = model.types.push(ty);
            assert_eq!(model.size_align_of(id), SizeAlign { size: 16, align: 8 });
        }
    }

    #[test]
    fn size_align_of_arrays_uses_stride_and_length() {
        let mut model = model();
        let elem_4 = model.types.push(Ty::Struct {
            item: iid(40),
            args: vec![],
            layout: sized_layout(4, 4),
        });
        let array_4 = model.types.push(Ty::Array(elem_4, 10));
        assert_eq!(
            model.size_align_of(array_4),
            SizeAlign { size: 40, align: 4 }
        );

        let elem_1 = model.types.push(Ty::Struct {
            item: iid(41),
            args: vec![],
            layout: sized_layout(1, 1),
        });
        let array_1 = model.types.push(Ty::Array(elem_1, 5));
        assert_eq!(
            model.size_align_of(array_1),
            SizeAlign { size: 5, align: 1 }
        );

        let elem_padded = model.types.push(Ty::Struct {
            item: iid(42),
            args: vec![],
            layout: sized_layout(5, 4),
        });
        let array_padded = model.types.push(Ty::Array(elem_padded, 2));
        assert_eq!(
            model.size_align_of(array_padded),
            SizeAlign { size: 16, align: 4 }
        );
    }

    #[test]
    #[should_panic(expected = "array layout size overflows u64")]
    fn size_align_of_array_panics_on_u64_overflow() {
        let mut model = model();
        let elem = model.types.push(Ty::Tuple {
            fields: vec![],
            layout: sized_layout(2, 2),
        });
        let array = model.types.push(Ty::Array(elem, u64::MAX));
        let _ = model.size_align_of(array);
    }

    #[test]
    #[should_panic(expected = "array layout size exceeds target usize::MAX")]
    fn size_align_of_array_panics_when_size_exceeds_target_usize() {
        let mut model = model();
        model.target.pointer_width = 16;
        let elem = model.types.push(Ty::Tuple {
            fields: vec![],
            layout: sized_layout(256, 1),
        });
        let array = model.types.push(Ty::Array(elem, 256));
        let _ = model.size_align_of(array);
    }

    #[test]
    fn size_align_of_inline_layout_variants_reads_layout_fields() {
        let mut model = model();
        for ty in [
            Ty::Struct {
                item: iid(0),
                args: vec![],
                layout: sized_layout(10, 2),
            },
            Ty::Enum {
                item: iid(0),
                args: vec![],
                layout: sized_layout(11, 4),
            },
            Ty::Union {
                item: iid(0),
                args: vec![],
                layout: sized_layout(12, 8),
            },
            Ty::Tuple {
                fields: vec![tid(0)],
                layout: sized_layout(13, 16),
            },
        ] {
            let expected = match &ty {
                Ty::Struct { layout, .. }
                | Ty::Enum { layout, .. }
                | Ty::Union { layout, .. }
                | Ty::Tuple { layout, .. } => SizeAlign {
                    size: layout.size,
                    align: layout.align,
                },
                _ => unreachable!(),
            };
            let id = model.types.push(ty);
            assert_eq!(model.size_align_of(id), expected);
        }
    }

    #[test]
    fn size_align_of_closure_uses_parallel_capture_struct() {
        let mut model = model();
        let item = iid(7);
        let args = vec![crate::GenericArg::Type(tid(1))];
        model.types.push(Ty::Struct {
            item,
            args: args.clone(),
            layout: sized_layout(24, 8),
        });
        let closure = model.types.push(Ty::Closure(item, args));

        assert_eq!(
            model.size_align_of(closure),
            SizeAlign { size: 24, align: 8 }
        );
    }

    #[test]
    #[should_panic(expected = "closure capture struct layout not found")]
    fn size_align_of_closure_panics_without_parallel_capture_struct() {
        let mut model = model();
        let closure = model.types.push(Ty::Closure(iid(7), vec![]));
        let _ = model.size_align_of(closure);
    }

    #[test]
    #[should_panic(expected = "unsized marker queried directly")]
    fn size_align_of_direct_str_panics() {
        let mut model = model();
        let str_ty = model.types.push(Ty::Str);
        let _ = model.size_align_of(str_ty);
    }

    #[test]
    #[should_panic(expected = "unsized marker queried directly")]
    fn size_align_of_direct_slice_panics() {
        let mut model = model();
        let elem = model.types.push(Ty::Bool);
        let slice = model.types.push(Ty::Slice(elem));
        let _ = model.size_align_of(slice);
    }

    #[test]
    #[should_panic(expected = "unsized marker queried directly")]
    fn size_align_of_direct_dynamic_panics() {
        let mut model = model();
        let vtable_ty = model.types.push(Ty::Bool);
        let dynamic = model.types.push(Ty::Dynamic {
            auto_traits: vec![],
            vtable_ty,
        });
        let _ = model.size_align_of(dynamic);
    }

    #[allow(dead_code)]
    fn _layout_kind_exhaustiveness_check(value: &LayoutKind) {
        match value {
            LayoutKind::Struct { .. } | LayoutKind::Union { .. } | LayoutKind::Enum { .. } => (),
        }
    }

    #[allow(dead_code)]
    fn _enum_shape_exhaustiveness_check(value: &EnumShape) {
        match value {
            EnumShape::Empty | EnumShape::Single { .. } | EnumShape::Multiple { .. } => (),
        }
    }

    #[allow(dead_code)]
    fn _tag_encoding_exhaustiveness_check(value: &TagEncoding) {
        match value {
            TagEncoding::Direct | TagEncoding::Niche { .. } => (),
        }
    }
}
