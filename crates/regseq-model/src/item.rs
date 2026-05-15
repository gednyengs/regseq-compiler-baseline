//! Source-level item alphabet.
//!
//! Items are the pre-monomorphization source definitions in a
//! `RegseqModel`. Each `Item` corresponds to one rustc `DefId`;
//! per-monomorphization data lives on `Instance`.

use core::convert::TryFrom;

use serde::{Deserialize, Serialize};

use crate::def_path::DefPath;
use crate::ids::StringId;
use crate::source::Span;
use crate::ty::{ClosedEnumDiscriminantError, Mutability};

/// Source-level item record.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Item {
    /// Structured source-level definition path.
    pub def_path: DefPath,
    /// Closed source-level item kind.
    pub kind: ItemKind,
    /// Source span for the item definition.
    pub span: Span,

    /// Symbol-emission classifier.
    ///
    /// The bridge populates this from the per-DefId mapping of
    /// `is_foreign_item`, `NO_MANGLE`, `export_name`, weak linkage,
    /// and ordinary external definitions.
    pub linkage: Linkage,

    /// Link-section placement such as `.init_array` or
    /// `.driver_data`.
    ///
    /// The bridge populates this from rustc
    /// `CodegenFnAttrs.link_section`.
    pub link_section: Option<StringId>,

    /// Inline attribute classification for function items.
    ///
    /// This corresponds to `#[inline]`, `#[inline(always)]`, and
    /// `#[inline(never)]` through rustc `CodegenFnAttrs.inline`.
    /// Callers pass `InlineHint::None` for non-function items.
    pub inline_hint: InlineHint,

    /// Whether this item is an `unsafe fn`.
    ///
    /// Analyses consume this flag. C code generation does not lower
    /// it directly because C has no `unsafe` keyword.
    pub is_unsafe: bool,

    // No `is_naked` field. Stage-14 D-14-7 rejects `#[naked]` at
    // Item translation, so v1 has no representable IR shape for a
    // naked function.
    /// Whether this ADT is `PhantomData`.
    ///
    /// The bridge recognizes it with `adt_def.is_phantom_data()`,
    /// backed by rustc `AdtFlags::IS_PHANTOM_DATA`. `MaybeUninit`,
    /// `CVoid`, `Pin`, `OwnedBox`, `VaList`, `GlobalAlloc`, and
    /// `UnsafePinned` are not represented by Item flags in v1.
    pub is_phantom_data: bool,

    /// Whether this ADT is `UnsafeCell`.
    ///
    /// The bridge recognizes it with `adt_def.is_unsafe_cell()`,
    /// backed by rustc `AdtFlags::IS_UNSAFE_CELL`. `MaybeUninit`,
    /// `CVoid`, `Pin`, `OwnedBox`, `VaList`, `GlobalAlloc`, and
    /// `UnsafePinned` are not represented by Item flags in v1.
    pub is_unsafe_cell: bool,

    /// Whether this ADT is `ManuallyDrop`.
    ///
    /// The bridge recognizes it with `adt_def.is_manually_drop()`,
    /// backed by rustc `AdtFlags::IS_MANUALLY_DROP`. `MaybeUninit`,
    /// `CVoid`, `Pin`, `OwnedBox`, `VaList`, `GlobalAlloc`, and
    /// `UnsafePinned` are not represented by Item flags in v1.
    pub is_manually_drop: bool,

    /// Bridge-computed opaque-handle eligibility outcome.
    ///
    /// When true, all five locked conditions held at construction:
    /// the item is `ItemKind::Struct`; the source ADT had bridge-
    /// certified C representation; the item is non-generic; its
    /// unique monomorphic `Ty::Struct` layout is a ZST with
    /// `size: 0` and `align: 1`; and every reachable use of that
    /// monomorphic `TyId` is behind `Ref` or `Ptr`. Enforcement of
    /// the model-checkable subset belongs to the validator task.
    /// Generic opaque-handle candidates are rejected by the bridge.
    pub opaque_handle_eligible: bool,
}

/// Closed source-level item kind.
///
/// `impl` blocks are source scopes, not emittable units. Their
/// contents lift to top-level items as follows:
///
/// - Inherent impl methods lift to top-level function items.
/// - Trait impl methods lift to top-level function items. For
///   dynamic dispatch only, vtables are synthesised as static
///   instances.
/// - Associated constants lift to top-level const items.
/// - Associated types are erased; uses are substituted at
///   monomorphisation.
/// - Trait declarations are erased: static dispatch resolves to
///   the implementing function's instance, and dynamic dispatch
///   goes through a vtable.
/// - Default trait method bodies are owned by the trait and live
///   at the trait's definition path.
///
/// There are no `Impl`, `Trait`, or `AssocType` item kinds.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ItemKind {
    /// Function item.
    Function,
    /// Static item.
    Static {
        /// Source-level `static` mutability.
        mutability: Mutability,
    },
    /// Const item.
    Const,
    /// Struct item.
    Struct,
    /// Enum item.
    Enum,
    /// Union item.
    Union,
}

/// Symbol-emission classifier.
///
/// This is a closed four-variant set. Other rustc linkage variants
/// are LLVM-specific for this model and are rejected by the bridge as
/// unsupported linkage.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Linkage {
    /// Ordinary defined function or static with a mangled name.
    Default = 0,
    /// `#[no_mangle]` or `#[export_name]`; symbol name overrides.
    NoMangle = 1,
    /// `extern "C" { ... }` declaration; never defined locally.
    Foreign = 2,
    /// `#[linkage = "weak"]`; tentative definition.
    Weak = 3,
}

impl From<Linkage> for u8 {
    fn from(value: Linkage) -> Self {
        value as u8
    }
}

impl TryFrom<u8> for Linkage {
    type Error = ClosedEnumDiscriminantError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Default),
            1 => Ok(Self::NoMangle),
            2 => Ok(Self::Foreign),
            3 => Ok(Self::Weak),
            _ => Err(ClosedEnumDiscriminantError {
                enum_name: "Linkage",
                value,
            }),
        }
    }
}

impl Serialize for Linkage {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_u8((*self).into())
    }
}

impl<'de> Deserialize<'de> for Linkage {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = u8::deserialize(deserializer)?;
        Self::try_from(raw).map_err(|err| {
            serde::de::Error::custom(format!(
                "{} discriminant {} is not valid",
                err.enum_name, err.value
            ))
        })
    }
}

/// Inline hint.
///
/// This is a closed four-variant mirror of rustc `InlineAttr`.
/// Non-function items use `InlineHint::None`.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum InlineHint {
    /// No `#[inline]` annotation.
    None = 0,
    /// `#[inline]`.
    Hint = 1,
    /// `#[inline(always)]`.
    Always = 2,
    /// `#[inline(never)]`.
    Never = 3,
}

impl From<InlineHint> for u8 {
    fn from(value: InlineHint) -> Self {
        value as u8
    }
}

impl TryFrom<u8> for InlineHint {
    type Error = ClosedEnumDiscriminantError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::None),
            1 => Ok(Self::Hint),
            2 => Ok(Self::Always),
            3 => Ok(Self::Never),
            _ => Err(ClosedEnumDiscriminantError {
                enum_name: "InlineHint",
                value,
            }),
        }
    }
}

impl Serialize for InlineHint {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_u8((*self).into())
    }
}

impl<'de> Deserialize<'de> for InlineHint {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = u8::deserialize(deserializer)?;
        Self::try_from(raw).map_err(|err| {
            serde::de::Error::custom(format!(
                "{} discriminant {} is not valid",
                err.enum_name, err.value
            ))
        })
    }
}

#[cfg(test)]
mod tests {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    use super::*;
    use crate::def_path::{DefPath, DefPathData, DefPathSegment};
    use crate::ids::{CrateId, SourceFileId, StringId};
    use crate::model::RegseqModel;
    use crate::{BridgeInput, DefPathHash, StableCrateId, SyntheticItemKind};

    fn sid(value: u32) -> StringId {
        StringId::from_u32(value)
    }

    fn span() -> Span {
        Span {
            file: SourceFileId::SYNTHESIZED,
            lo: 0,
            hi: 0,
        }
    }

    fn def_path(rendered: StringId) -> DefPath {
        DefPath {
            crate_id: CrateId::ZERO,
            segments: vec![
                DefPathSegment {
                    data: DefPathData::CrateRoot,
                    disambiguator: 0,
                },
                DefPathSegment {
                    data: DefPathData::ValueNs(rendered),
                    disambiguator: 0,
                },
            ],
            rendered,
        }
    }

    fn item(kind: ItemKind) -> Item {
        Item {
            def_path: def_path(sid(1)),
            kind,
            span: span(),
            linkage: Linkage::Default,
            link_section: None,
            inline_hint: InlineHint::None,
            is_unsafe: false,
            is_phantom_data: false,
            is_unsafe_cell: false,
            is_manually_drop: false,
            opaque_handle_eligible: false,
        }
    }

    fn hash_of(value: &Item) -> u64 {
        let mut hasher = DefaultHasher::new();
        value.hash(&mut hasher);
        hasher.finish()
    }

    #[test]
    fn itemkind_function_constructs() {
        assert!(matches!(ItemKind::Function, ItemKind::Function));
    }

    #[test]
    fn itemkind_static_constructs_with_both_mutabilities() {
        assert!(matches!(
            ItemKind::Static {
                mutability: Mutability::Not
            },
            ItemKind::Static {
                mutability: Mutability::Not
            }
        ));
        assert!(matches!(
            ItemKind::Static {
                mutability: Mutability::Mut
            },
            ItemKind::Static {
                mutability: Mutability::Mut
            }
        ));
    }

    #[test]
    fn itemkind_const_constructs() {
        assert!(matches!(ItemKind::Const, ItemKind::Const));
    }

    #[test]
    fn itemkind_struct_constructs() {
        assert!(matches!(ItemKind::Struct, ItemKind::Struct));
    }

    #[test]
    fn itemkind_enum_constructs() {
        assert!(matches!(ItemKind::Enum, ItemKind::Enum));
    }

    #[test]
    fn itemkind_union_constructs() {
        assert!(matches!(ItemKind::Union, ItemKind::Union));
    }

    #[test]
    fn linkage_variants_construct_with_stable_discriminants_and_round_trip() {
        for (variant, raw) in [
            (Linkage::Default, 0),
            (Linkage::NoMangle, 1),
            (Linkage::Foreign, 2),
            (Linkage::Weak, 3),
        ] {
            assert_eq!(variant as u8, raw);
            assert_eq!(
                serde_json::from_str::<Linkage>(&raw.to_string()).unwrap(),
                variant
            );
            assert_eq!(serde_json::to_string(&variant).unwrap(), raw.to_string());
        }
    }

    #[test]
    fn inline_hint_variants_construct_with_stable_discriminants_and_round_trip() {
        for (variant, raw) in [
            (InlineHint::None, 0),
            (InlineHint::Hint, 1),
            (InlineHint::Always, 2),
            (InlineHint::Never, 3),
        ] {
            assert_eq!(variant as u8, raw);
            assert_eq!(
                serde_json::from_str::<InlineHint>(&raw.to_string()).unwrap(),
                variant
            );
            assert_eq!(serde_json::to_string(&variant).unwrap(), raw.to_string());
        }
    }

    #[test]
    fn non_function_item_uses_explicit_none_inline_hint() {
        let item = item(ItemKind::Static {
            mutability: Mutability::Not,
        });
        assert_eq!(item.inline_hint, InlineHint::None);
    }

    #[test]
    fn item_constructs_function_with_all_default_metadata() {
        let item = item(ItemKind::Function);
        assert_eq!(item.def_path, def_path(sid(1)));
        assert_eq!(item.kind, ItemKind::Function);
        assert_eq!(item.span, span());
        assert_eq!(item.linkage, Linkage::Default);
        assert_eq!(item.link_section, None);
        assert_eq!(item.inline_hint, InlineHint::None);
        assert!(!item.is_unsafe);
        assert!(!item.is_phantom_data);
        assert!(!item.is_unsafe_cell);
        assert!(!item.is_manually_drop);
        assert!(!item.opaque_handle_eligible);
    }

    #[test]
    fn item_constructs_unsafe_function() {
        let mut item = item(ItemKind::Function);
        item.is_unsafe = true;
        assert!(item.is_unsafe);
    }

    #[test]
    fn item_constructs_inline_always_function() {
        let mut item = item(ItemKind::Function);
        item.inline_hint = InlineHint::Always;
        assert_eq!(item.inline_hint, InlineHint::Always);
    }

    #[test]
    fn item_constructs_static_with_link_section() {
        let mut item = item(ItemKind::Static {
            mutability: Mutability::Not,
        });
        item.link_section = Some(sid(2));
        assert_eq!(item.link_section, Some(sid(2)));
    }

    #[test]
    fn item_constructs_no_mangle_function() {
        let mut item = item(ItemKind::Function);
        item.linkage = Linkage::NoMangle;
        assert_eq!(item.linkage, Linkage::NoMangle);
    }

    #[test]
    fn item_constructs_lang_item_flags_and_opaque_handle_flag() {
        let mut phantom = item(ItemKind::Struct);
        phantom.is_phantom_data = true;
        assert!(phantom.is_phantom_data);

        let mut unsafe_cell = item(ItemKind::Struct);
        unsafe_cell.is_unsafe_cell = true;
        assert!(unsafe_cell.is_unsafe_cell);

        let mut manually_drop = item(ItemKind::Struct);
        manually_drop.is_manually_drop = true;
        assert!(manually_drop.is_manually_drop);

        let mut opaque = item(ItemKind::Struct);
        opaque.opaque_handle_eligible = true;
        assert!(opaque.opaque_handle_eligible);
    }

    #[test]
    fn item_equality_and_hash_cover_all_metadata_fields() {
        let base = item(ItemKind::Function);
        let same = item(ItemKind::Function);
        assert_eq!(base, same);
        assert_eq!(hash_of(&base), hash_of(&same));

        let mut changed = base.clone();
        changed.linkage = Linkage::Weak;
        assert_ne!(base, changed);

        let mut changed = base.clone();
        changed.inline_hint = InlineHint::Never;
        assert_ne!(base, changed);

        let mut changed = base.clone();
        changed.is_unsafe = true;
        assert_ne!(base, changed);

        let mut changed = base.clone();
        changed.opaque_handle_eligible = true;
        assert_ne!(base, changed);

        let mut changed = base.clone();
        changed.link_section = Some(sid(3));
        assert_ne!(base, changed);
    }

    #[test]
    fn item_field_destructure_is_exhaustive() {
        fn _exhaustiveness_check(item: &Item) {
            let Item {
                def_path: _,
                kind: _,
                span: _,
                linkage: _,
                link_section: _,
                inline_hint: _,
                is_unsafe: _,
                is_phantom_data: _,
                is_unsafe_cell: _,
                is_manually_drop: _,
                opaque_handle_eligible: _,
            } = item;
        }
        _exhaustiveness_check(&item(ItemKind::Struct));
    }

    #[test]
    fn item_round_trips_through_serde_with_link_section_some_and_none() {
        let none = item(ItemKind::Function);
        let json = serde_json::to_string(&none).unwrap();
        let back: Item = serde_json::from_str(&json).unwrap();
        assert_eq!(none, back);

        let mut some = item(ItemKind::Static {
            mutability: Mutability::Mut,
        });
        some.link_section = Some(sid(9));
        let json = serde_json::to_string(&some).unwrap();
        let back: Item = serde_json::from_str(&json).unwrap();
        assert_eq!(some, back);
    }

    #[test]
    fn itemkind_exhaustiveness_scaffold_is_closed() {
        fn _itemkind_exhaustiveness(k: &ItemKind) {
            match k {
                ItemKind::Function
                | ItemKind::Static { .. }
                | ItemKind::Const
                | ItemKind::Struct
                | ItemKind::Enum
                | ItemKind::Union => (),
            }
        }
        _itemkind_exhaustiveness(&ItemKind::Function);
    }

    #[test]
    fn linkage_and_inline_hint_exhaustiveness_scaffolds_are_closed() {
        fn _linkage_exhaustiveness(l: &Linkage) {
            match l {
                Linkage::Default | Linkage::NoMangle | Linkage::Foreign | Linkage::Weak => (),
            }
        }

        fn _inline_hint_exhaustiveness(h: &InlineHint) {
            match h {
                InlineHint::None | InlineHint::Hint | InlineHint::Always | InlineHint::Never => (),
            }
        }

        _linkage_exhaustiveness(&Linkage::Default);
        _inline_hint_exhaustiveness(&InlineHint::None);
    }

    #[test]
    fn regseq_model_items_table_stores_items_in_index_order() {
        let mut model = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
        let first = model.items.push(item(ItemKind::Function));
        let second = model.items.push(item(ItemKind::Struct));
        let third = model.items.push(item(ItemKind::Union));

        assert_eq!(model.items[first].kind, ItemKind::Function);
        assert_eq!(model.items[second].kind, ItemKind::Struct);
        assert_eq!(model.items[third].kind, ItemKind::Union);
    }

    #[test]
    fn regseq_model_validate_accepts_matching_item_side_tables() {
        let mut model = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
        let name = model.strings.intern("<synthesized>");
        let rendered = model.strings.intern("pl011-spec::<synthesized>");
        let mut item = item(ItemKind::Function);
        item.def_path.segments[1].data = DefPathData::ValueNs(name);
        item.def_path.rendered = rendered;
        model.items.push(item);
        model.item_def_paths.push(DefPathHash {
            stable_crate_id: StableCrateId(1),
            local_hash: 1,
        });
        model.item_synthetic_kinds.push(SyntheticItemKind::None);

        assert_eq!(model.validate(&BridgeInput::matching(&model)), Ok(()));
    }

    #[test]
    fn opaque_handle_eligible_constructs_and_round_trips_without_validation() {
        let mut item = item(ItemKind::Struct);
        item.opaque_handle_eligible = true;

        let json = serde_json::to_string(&item).unwrap();
        let back: Item = serde_json::from_str(&json).unwrap();

        assert_eq!(back.kind, ItemKind::Struct);
        assert!(back.opaque_handle_eligible);
    }
}
