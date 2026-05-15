//! Top-level `RegseqModel` struct and construction-invariant registry.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::args_key::GenericArg;
use crate::bridge_input::BridgeInput;
use crate::build::BuildInfo;
use crate::cabi::CAbiLowering;
use crate::const_interner::ConstInterner;
use crate::crate_info::{CrateInfo, StableCrateId};
use crate::def_path::{DefPath, DefPathHash};
use crate::domain_attr::DomainAttrTables;
use crate::domain_attr::IntrinsicKind;
use crate::fn_sig::FnSig;
use crate::fn_sig_interner::FnSigInterner;
use crate::ids::{ConstId, CrateId, FnSigId, InstanceId, ItemId, SourceFileId, StringId, TyId};
use crate::index_vec::IndexVec;
use crate::instance::{Instance, InstanceKind};
use crate::layout::{EnumShape, LayoutKind};
use crate::placeholders::{Body, InlineHint, Item, Linkage, PrincipalTraitKey, SyntheticItemKind};
use crate::pretty::PrettyPrinter;
use crate::source::{SourceFile, Span};
use crate::string_interner::StringInterner;
use crate::target::{default_scalar_layouts, Endian, SizeAlign, TargetInfo};
use crate::ty::{FloatKind, IntKind, Ty, UintKind};
use crate::ty_interner::TyInterner;

/// The owned root struct returned by the bridge for one rustc
/// compilation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegseqModel {
    /// Crate inventory.
    pub crates: IndexVec<CrateId, CrateInfo>,
    /// Index of the spec crate in `crates`.
    pub primary_crate: CrateId,

    /// Target context.
    pub target: TargetInfo,

    /// Build provenance.
    pub build: BuildInfo,

    /// Content-unique string interner.
    pub strings: StringInterner,
    /// Source files; index 0 is the synthesized sentinel.
    pub source_files: IndexVec<SourceFileId, SourceFile>,

    /// Content-keyed type interner.
    pub types: TyInterner,
    /// Content-keyed const interner.
    pub consts: ConstInterner,
    /// Content-keyed function signature interner.
    pub sigs: FnSigInterner,

    /// Source-level item alphabet.
    pub items: IndexVec<ItemId, Item>,
    /// Stable item identity side table parallel to `items`.
    pub item_def_paths: IndexVec<ItemId, DefPathHash>,
    /// Synthetic item classification side table parallel to `items`.
    pub item_synthetic_kinds: IndexVec<ItemId, SyntheticItemKind>,

    /// C ABI lowering side table parallel to `sigs`.
    pub sig_c_abi_lowerings: IndexVec<FnSigId, Option<CAbiLowering>>,

    /// Primary monomorphized table.
    pub instances: IndexVec<InstanceId, Instance>,

    /// Bridge-side vtable dedup table rebuilt after deserialization.
    #[serde(skip)]
    pub vtables: HashMap<PrincipalTraitKey, ItemId>,

    /// Domain attribute sibling tables.
    pub domain_attrs: DomainAttrTables,
}

#[cfg(test)]
#[path = "model_tests.rs"]
mod tests;

const SOURCE_FILE_PATH: &str = "SourceFile.path (StringId)";
const CRATE_INFO_NAME: &str = "CrateInfo.name (StringId)";
const INSTANCE_ITEM: &str = "Instance.item (ItemId)";
const INSTANCE_SYMBOL_NAME: &str = "Instance.symbol_name (StringId)";
const INSTANCE_SPAN_FILE: &str = "Instance.span.file (SourceFileId)";
const INSTANCE_GENERIC_ARG_TY: &str = "Instance.generic_args[..](TyId)";
const INSTANCE_GENERIC_ARG_CONST: &str = "Instance.generic_args[..](ConstId)";
const INSTANCE_KIND_FUNCTION_SIG: &str = "InstanceKind::Function.sig (FnSigId)";
const INSTANCE_KIND_STATIC_TY: &str = "InstanceKind::Static.ty (TyId)";
const INSTANCE_KIND_STATIC_INIT: &str = "InstanceKind::Static.init (ConstId)";
const INSTANCE_KIND_CONST_TY: &str = "InstanceKind::Const.ty (TyId)";
const INSTANCE_KIND_CONST_VALUE: &str = "InstanceKind::Const.value (ConstId)";
const INSTANCE_KIND_STRUCT_LAYOUT: &str = "InstanceKind::Struct.layout (TyId)";
const INSTANCE_KIND_ENUM_LAYOUT: &str = "InstanceKind::Enum.layout (TyId)";
const INSTANCE_KIND_UNION_LAYOUT: &str = "InstanceKind::Union.layout (TyId)";

/// One construction-invariant violation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InvariantViolation {
    /// An ID does not resolve to a valid entry in its table.
    DanglingId {
        /// Field carrying the dangling reference.
        field: &'static str,
        /// Logical index value of the offending ID.
        value: u32,
        /// Length of the referenced table.
        table_len: usize,
    },

    /// `item_def_paths.len() != items.len()`.
    ItemDefPathsLengthMismatch {
        /// Length of `items`.
        items_len: usize,
        /// Length of `item_def_paths`.
        def_paths_len: usize,
    },

    /// `item_synthetic_kinds.len() != items.len()`.
    ItemSyntheticKindsLengthMismatch {
        /// Length of `items`.
        items_len: usize,
        /// Length of `item_synthetic_kinds`.
        synthetic_kinds_len: usize,
    },

    /// `primary_crate.index() >= crates.len()`.
    PrimaryCrateOutOfRange {
        /// Offending primary-crate index.
        value: u32,
        /// Length of `crates`.
        crates_len: usize,
    },

    /// Primary crate stable id does not match bridge input.
    PrimaryCrateStableIdMismatch {
        /// Stable id stored in the model primary crate.
        model: StableCrateId,
        /// Stable id supplied by the bridge input.
        bridge_input: StableCrateId,
    },

    /// Two `CrateInfo` entries share a `StableCrateId`.
    DuplicateStableCrateId {
        /// Duplicate stable crate id.
        stable_id: StableCrateId,
        /// First crate id carrying the stable id.
        first: CrateId,
        /// Second crate id carrying the stable id.
        second: CrateId,
    },

    /// `source_files[0]` is not the synthesized sentinel.
    SynthesizedSentinelInvalid {
        /// Brief reason.
        reason: &'static str,
    },
}

/// Construction-invariant identifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum InvariantId {
    /// 1. Tables are populated before they are referenced. Every model ID resolves to a valid index in its table.
    TablesPopulatedBeforeReference,
    /// 2. `item_def_paths` and `item_synthetic_kinds` mirror `items`.
    ItemSideTablesMirrorItems,
    /// 3. Items are sorted at construction by the F4 sort comparator.
    ItemsSortedAtConstruction,
    /// 4. `primary_crate` is valid and corresponds to the spec crate.
    PrimaryCrateValidAndMatchesSpec,
    /// 5. `crates[c].stable_crate_id` is unique across `c`.
    StableCrateIdsUnique,
    /// 6. Every `DefPath.crate_id` resolves to a valid entry.
    DefPathCrateIdsResolve,
    /// 7. No rustc lifetime references are stored in the model.
    NoTcxReferences,
    /// 8. `source_files[0]` is the synthesised source-file sentinel.
    SynthesizedSourceFileSentinel,
    /// 9. Instance closure-completeness holds for every `InstanceId` referenced anywhere in the model.
    InstanceClosureCompleteness,
    /// 10. Body presence matches the `InstanceKind::Function` shape.
    FunctionBodyPresenceMatchesKind,
    /// 11. Content-keyed interner uniqueness holds for `types`, `sigs`, `consts`, and strings.
    ContentKeyedInternerUniqueness,
    /// 12. Unsized marker types appear only as the pointee of a reference or pointer.
    UnsizedMarkerPlacement,
    /// 13. `Ty::Dynamic` values are well formed.
    DynamicWellFormedness,
    /// 14. `InstanceKind` layout references are consistent with the matching nominal type variant.
    InstanceKindLayoutConsistency,
    /// 15. `TargetInfo` is well formed.
    TargetInfoWellFormedness,
    /// 16. C ABI admissibility outcomes and lowering records are recorded for function signatures.
    CAbiLoweringRecordsMatchSignatures,
    /// 17. `BuildInfo` is well formed.
    BuildInfoWellFormedness,
    /// 18. `item_def_paths` are globally unique.
    ItemDefPathGlobalUniqueness,
    /// 19. `Item.opaque_handle_eligible` is consistent for non-generic items.
    OpaqueHandleEligibilityConsistency,
    /// 20. `StableCrateId(0)` is reserved as the synthetic-domain sentinel.
    StableCrateIdZeroReserved,
}

/// Returns the locked spec wording for a construction invariant.
pub fn invariant_descriptor(id: InvariantId) -> &'static str {
    match id {
        InvariantId::TablesPopulatedBeforeReference => {
            concat!(
                "Tables are populated before they are referenced. Every `CrateId`, `TyId`, ",
                "`ConstId`, `ItemId`, `InstanceId`, `FnSigId`, `SourceFileId`, `StringId`, ",
                "`VariantIdx` resolves to a valid index in its table. Debug-build assertion. ",
                "No `BodyId` per stage-12 D-12-1.4 / Option B; bodies live structurally on ",
                "`InstanceKind::Function`."
            )
        }
        InvariantId::ItemSideTablesMirrorItems => {
            concat!(
                "`item_def_paths` and `item_synthetic_kinds` mirror `items`. Both side tables ",
                "have `len() == items.len()`. Source-level items use rustc `DefPathHash` and ",
                "`SyntheticItemKind::None`; bridge-synthesised items use the synthetic hash and ",
                "structured synthetic-kind tag, except `ClosureCaptures` keeps the closure ",
                "rustc `DefPathHash`."
            )
        }
        InvariantId::ItemsSortedAtConstruction => {
            concat!(
                "Items are sorted at construction. Items are placed in `items` and ",
                "`item_def_paths` in F4 sort-comparator order. Builds satisfying the ",
                "build-equivalence quad produce identical `ItemId` assignments; other builds ",
                "are aligned by `DefPathHash`."
            )
        }
        InvariantId::PrimaryCrateValidAndMatchesSpec => {
            concat!(
                "`primary_crate` is valid and corresponds to the spec crate. ",
                "`primary_crate.index() < crates.len()` and ",
                "`crates[primary_crate].stable_crate_id` matches the bridge input's ",
                "`spec_crate_stable_id`."
            )
        }
        InvariantId::StableCrateIdsUnique => {
            concat!(
                "`crates[c].stable_crate_id` is unique across `c`. No two `CrateInfo` ",
                "entries share a `StableCrateId`. `crates` is sorted by ",
                "`(crate_name, stable_crate_id)` before `CrateId` assignment."
            )
        }
        InvariantId::DefPathCrateIdsResolve => {
            "Every `DefPath.crate_id` resolves to a valid entry. For every item, `item.def_path.crate_id.index() < crates.len()`."
        }
        InvariantId::NoTcxReferences => {
            "No `'tcx` references. The model is fully owned and `'static`-compatible; serialisation round-trips losslessly."
        }
        InvariantId::SynthesizedSourceFileSentinel => {
            "Synthesised source-file sentinel. `source_files[0]` has path `\"<synthesized>\"`, `content: None`, and `line_starts: vec![0]`; real source files start at `SourceFileId(1)`."
        }
        InvariantId::InstanceClosureCompleteness => {
            concat!(
                "Instance closure-completeness. The bridge computes the monomorphisation ",
                "closure from a settled root set. Every `InstanceId` referenced anywhere in ",
                "the model resolves to a valid entry."
            )
        }
        InvariantId::FunctionBodyPresenceMatchesKind => {
            concat!(
                "Body presence matches `InstanceKind::Function` shape. Bodies live ",
                "structurally on `InstanceKind::Function { body: Option<Body>, ... }`. ",
                "`body: Some(b)` stores the body on the Instance-side field; `body: None` ",
                "is reserved for foreign declarations. Non-Function kinds have no body field."
            )
        }
        InvariantId::ContentKeyedInternerUniqueness => {
            "Content-keyed interner uniqueness. `types`, `sigs`, and `consts` are content-keyed with no two distinct IDs mapping to canonically-equal values; strings follow the same rule."
        }
        InvariantId::UnsizedMarkerPlacement => {
            "`Ty::Str`, `Ty::Slice`, and `Ty::Dynamic` appear only as the pointee of a `Ty::Ref` or `Ty::Ptr`; bare unsized markers are rejected elsewhere."
        }
        InvariantId::DynamicWellFormedness => {
            "`Ty::Dynamic` well-formedness. Auto traits are sorted with no duplicates, a principal trait is required, and `vtable_ty` resolves to the synthesised vtable `Ty::Struct` for the full `PrincipalTraitKey`."
        }
        InvariantId::InstanceKindLayoutConsistency => {
            "`InstanceKind` layout consistency. For `Struct`, `Enum`, and `Union`, the referenced `TyId` resolves to the matching nominal `Ty` variant whose `(item, args)` equals `(Instance.item, Instance.generic_args)`."
        }
        InvariantId::TargetInfoWellFormedness => {
            "`TargetInfo` well-formedness. Target strings are non-empty, pointer width is one of 16, 32, or 64, endian and pointer width agree with the data layout, and scalar layouts match the target ABI."
        }
        InvariantId::CAbiLoweringRecordsMatchSignatures => {
            concat!(
                "C ABI admissibility outcome and lowering record. Every `FnSig` with ",
                "`abi: FnAbi::C` was checked for C ABI admissibility at bridge time, Rust ",
                "`TyId`s are preserved, and `sig_c_abi_lowerings` records `Some(CAbiLowering)` ",
                "for C ABI signatures and `None` for Rust ABI signatures."
            )
        }
        InvariantId::BuildInfoWellFormedness => {
            "`BuildInfo` well-formedness. `build.toolchain` is a non-empty interned string and `build.bridge_options_hash` is deterministic over normalised extraction options."
        }
        InvariantId::ItemDefPathGlobalUniqueness => {
            "`item_def_paths` global uniqueness. No two distinct `ItemId`s map to the same `DefPathHash`; source-derived and synthetic spaces are domain-separated by `stable_crate_id`."
        }
        InvariantId::OpaqueHandleEligibilityConsistency => {
            concat!(
                "`Item.opaque_handle_eligible` consistency (non-generic only). When true, ",
                "the item is a non-generic `ItemKind::Struct`, has bridge-certified C repr, ",
                "has a unique ZST monomorphic layout, and every reachable use is behind a ",
                "`Ref` or `Ptr`."
            )
        }
        InvariantId::StableCrateIdZeroReserved => {
            "`StableCrateId(0)` reserved as synthetic-domain sentinel. No real source crate carries `StableCrateId(0)`; synthetic-item `DefPathHash`es use `stable_crate_id == 0`."
        }
    }
}

impl RegseqModel {
    /// Empty model with the synthesized-source sentinel and primary
    /// crate pre-populated.
    pub fn with_primary(primary_name: &str, primary_stable_crate_id: StableCrateId) -> Self {
        let mut strings = StringInterner::new();

        let synth_path = strings.intern("<synthesized>");
        let target_triple = strings.intern("unknown-unknown-unknown");
        let data_layout = strings.intern("e-p:64:64");
        let toolchain = strings.intern("nightly-2026-02-01");

        let mut source_files: IndexVec<SourceFileId, SourceFile> = IndexVec::new();
        let pushed = source_files.push(SourceFile {
            path: synth_path,
            content: None,
            line_starts: vec![0],
        });
        debug_assert_eq!(pushed, SourceFileId::SYNTHESIZED);

        let primary_name_id = strings.intern(primary_name);
        let mut crates: IndexVec<CrateId, CrateInfo> = IndexVec::new();
        let primary_id = crates.push(CrateInfo {
            name: primary_name_id,
            stable_crate_id: primary_stable_crate_id,
        });

        Self {
            crates,
            primary_crate: primary_id,
            target: TargetInfo {
                triple: target_triple,
                pointer_width: 64,
                endian: Endian::Little,
                data_layout,
                scalar_layouts: default_scalar_layouts(64),
            },
            build: BuildInfo {
                toolchain,
                bridge_options_hash: 0,
            },
            strings,
            source_files,
            types: TyInterner::new(),
            consts: ConstInterner::new(),
            sigs: FnSigInterner::new(),
            items: IndexVec::new(),
            item_def_paths: IndexVec::new(),
            item_synthetic_kinds: IndexVec::new(),
            sig_c_abi_lowerings: IndexVec::new(),
            instances: IndexVec::new(),
            vtables: HashMap::new(),
            domain_attrs: DomainAttrTables::default(),
        }
    }

    /// Convenience accessor for `PrettyPrinter::new(self)`.
    pub fn pretty(&self) -> PrettyPrinter<'_> {
        PrettyPrinter::new(self)
    }

    /// Validates construction invariants enforceable at this phase.
    pub fn validate(&self, bridge_input: &BridgeInput) -> Result<(), InvariantViolation> {
        self.validate_synthesized_sentinel()?;

        if self.primary_crate.index() >= self.crates.len() {
            return Err(InvariantViolation::PrimaryCrateOutOfRange {
                value: self.primary_crate.as_u32(),
                crates_len: self.crates.len(),
            });
        }

        let primary_stable_id = self.crates[self.primary_crate].stable_crate_id;
        if primary_stable_id != bridge_input.spec_crate_stable_id {
            return Err(InvariantViolation::PrimaryCrateStableIdMismatch {
                model: primary_stable_id,
                bridge_input: bridge_input.spec_crate_stable_id,
            });
        }

        for info in self.crates.iter() {
            self.check_string_id(info.name, CRATE_INFO_NAME)?;
        }

        for file in self.source_files.iter() {
            self.check_string_id(file.path, SOURCE_FILE_PATH)?;
        }

        self.validate_unique_stable_crate_ids()?;

        if self.item_def_paths.len() != self.items.len() {
            return Err(InvariantViolation::ItemDefPathsLengthMismatch {
                items_len: self.items.len(),
                def_paths_len: self.item_def_paths.len(),
            });
        }

        if self.item_synthetic_kinds.len() != self.items.len() {
            return Err(InvariantViolation::ItemSyntheticKindsLengthMismatch {
                items_len: self.items.len(),
                synthetic_kinds_len: self.item_synthetic_kinds.len(),
            });
        }

        for instance in self.instances.iter() {
            self.validate_instance(instance)?;
        }

        Ok(())
    }

    /// Returns the source-level def path for an instance.
    pub fn instance_def_path(&self, id: InstanceId) -> &DefPath {
        &self.instance_item(id).def_path
    }

    /// Returns the source span for an instance's item.
    pub fn instance_span(&self, id: InstanceId) -> Span {
        self.instance_item(id).span
    }

    /// Returns the linkage for an instance's item.
    pub fn instance_linkage(&self, id: InstanceId) -> Linkage {
        self.instance_item(id).linkage
    }

    /// Returns the optional link section for an instance's item.
    pub fn instance_link_section(&self, id: InstanceId) -> Option<StringId> {
        self.instance_item(id).link_section
    }

    /// Returns the inline hint for an instance's item.
    pub fn instance_inline_hint(&self, id: InstanceId) -> InlineHint {
        self.instance_item(id).inline_hint
    }

    /// Returns whether an instance's item is unsafe.
    pub fn instance_is_unsafe(&self, id: InstanceId) -> bool {
        self.instance_item(id).is_unsafe
    }

    /// Returns whether an instance's item is `PhantomData`.
    pub fn instance_is_phantom_data(&self, id: InstanceId) -> bool {
        self.instance_item(id).is_phantom_data
    }

    /// Returns whether an instance's item is `UnsafeCell`.
    pub fn instance_is_unsafe_cell(&self, id: InstanceId) -> bool {
        self.instance_item(id).is_unsafe_cell
    }

    /// Returns true when `ty` structurally contains an `UnsafeCell`.
    ///
    /// Recurses through aggregate layouts and arrays, treats
    /// references, pointers, function pointers, and function
    /// definitions as boundaries, and uses `Item.is_unsafe_cell` for
    /// the nominal struct marker.
    pub fn contains_unsafe_cell(&self, ty: TyId) -> bool {
        self.contains_unsafe_cell_inner(ty, &mut HashSet::new())
    }

    /// Returns the ABI size and alignment for a sized type.
    ///
    /// Panics if `ty` resolves to a bare unsized marker (`Str`,
    /// `Slice`, or `Dynamic`), if a closure's parallel capture
    /// struct is absent, if array stride arithmetic overflows, or if
    /// an array size exceeds the target pointer-width `usize::MAX`.
    pub fn size_align_of(&self, ty: TyId) -> SizeAlign {
        match &self.types[ty] {
            Ty::Struct { layout, .. }
            | Ty::Enum { layout, .. }
            | Ty::Union { layout, .. }
            | Ty::Tuple { layout, .. } => SizeAlign {
                size: layout.size,
                align: layout.align,
            },
            Ty::Array(elem, len) => {
                let elem_layout = self.size_align_of(*elem);
                let stride = round_up_to_multiple(elem_layout.size, elem_layout.align);
                let size = stride
                    .checked_mul(*len)
                    .expect("array layout size overflows u64");
                let max_usize = target_usize_max(self.target.pointer_width);
                assert!(
                    size <= max_usize,
                    "array layout size exceeds target usize::MAX"
                );
                SizeAlign {
                    size,
                    align: elem_layout.align,
                }
            }
            Ty::Ref(pointee, _) | Ty::Ptr(pointee, _) => {
                if self.is_unsized_marker(*pointee) {
                    SizeAlign {
                        size: self
                            .target
                            .scalar_layouts
                            .data_ptr
                            .size
                            .checked_mul(2)
                            .expect("fat pointer size overflows u64"),
                        align: self.target.scalar_layouts.data_ptr.align,
                    }
                } else {
                    self.target.scalar_layouts.data_ptr
                }
            }
            Ty::FnPtr(_) => self.target.scalar_layouts.fn_ptr,
            Ty::FnDef(_, _) | Ty::Never => SizeAlign { size: 0, align: 1 },
            Ty::Closure(item, args) => self
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
                .expect("closure capture struct layout not found"),
            Ty::Bool => self.target.scalar_layouts.bool_,
            Ty::Char => self.target.scalar_layouts.char_,
            Ty::Int(kind) => self.target.scalar_layouts.int[int_kind_index(*kind)],
            Ty::Uint(kind) => self.target.scalar_layouts.uint[uint_kind_index(*kind)],
            Ty::Float(kind) => self.target.scalar_layouts.float[float_kind_index(*kind)],
            Ty::Str | Ty::Slice(_) | Ty::Dynamic { .. } => {
                panic!("unsized marker queried directly")
            }
        }
    }

    /// Returns whether an instance's item is `ManuallyDrop`.
    pub fn instance_is_manually_drop(&self, id: InstanceId) -> bool {
        self.instance_item(id).is_manually_drop
    }

    /// Returns the bridge-computed opaque-handle eligibility flag.
    pub fn instance_opaque_handle_eligible(&self, id: InstanceId) -> bool {
        self.instance_item(id).opaque_handle_eligible
    }

    /// Returns the function signature for function instances.
    pub fn instance_fn_sig(&self, id: InstanceId) -> Option<&FnSig> {
        match self.instances[id].kind {
            InstanceKind::Function { sig, .. } => Some(self.sigs.resolve(sig)),
            _ => None,
        }
    }

    /// Returns the inline body for function instances that have one.
    pub fn instance_body(&self, id: InstanceId) -> Option<&Body> {
        match &self.instances[id].kind {
            InstanceKind::Function { body, .. } => body.as_ref(),
            _ => None,
        }
    }

    /// Returns the domain intrinsic marker for an instance's item.
    pub fn instance_intrinsic(&self, _id: InstanceId) -> Option<&IntrinsicKind> {
        todo!("P1a-DS-09 adds DomainAttrTables contents")
    }

    fn instance_item(&self, id: InstanceId) -> &Item {
        &self.items[self.instances[id].item]
    }

    fn contains_unsafe_cell_inner(&self, ty: TyId, visited: &mut HashSet<TyId>) -> bool {
        if !visited.insert(ty) {
            return false;
        }

        match &self.types[ty] {
            Ty::Struct { item, layout, .. } => {
                self.items[*item].is_unsafe_cell
                    || self.layout_contains_unsafe_cell(layout, visited)
            }
            Ty::Enum { layout, .. } | Ty::Union { layout, .. } => {
                self.layout_contains_unsafe_cell(layout, visited)
            }
            Ty::Tuple { fields, .. } => fields
                .iter()
                .any(|field_ty| self.contains_unsafe_cell_inner(*field_ty, visited)),
            Ty::Array(elem, _) => self.contains_unsafe_cell_inner(*elem, visited),
            Ty::Closure(item, args) => {
                let captures = self
                    .types
                    .find_nominal(crate::NominalKind::Struct, *item, args)
                    .expect("closure capture struct type not found");
                self.contains_unsafe_cell_inner(captures, visited)
            }
            Ty::Ref(_, _)
            | Ty::Ptr(_, _)
            | Ty::FnPtr(_)
            | Ty::FnDef(_, _)
            | Ty::Bool
            | Ty::Char
            | Ty::Int(_)
            | Ty::Uint(_)
            | Ty::Float(_)
            | Ty::Str
            | Ty::Slice(_)
            | Ty::Dynamic { .. }
            | Ty::Never => false,
        }
    }

    fn layout_contains_unsafe_cell(
        &self,
        layout: &crate::Layout,
        visited: &mut HashSet<TyId>,
    ) -> bool {
        match &layout.kind {
            LayoutKind::Struct { fields } | LayoutKind::Union { fields } => fields
                .iter()
                .any(|field| self.contains_unsafe_cell_inner(field.ty, visited)),
            LayoutKind::Enum { shape } => match shape {
                EnumShape::Empty => false,
                EnumShape::Single { variant, .. } => variant
                    .fields
                    .iter()
                    .any(|field| self.contains_unsafe_cell_inner(field.ty, visited)),
                EnumShape::Multiple { variants, .. } => variants.iter().any(|variant| {
                    variant
                        .fields
                        .iter()
                        .any(|field| self.contains_unsafe_cell_inner(field.ty, visited))
                }),
            },
        }
    }

    fn validate_synthesized_sentinel(&self) -> Result<(), InvariantViolation> {
        let sf = self.source_files.iter().next().ok_or({
            InvariantViolation::SynthesizedSentinelInvalid {
                reason: "source_files is empty",
            }
        })?;

        if sf.path.index() >= self.strings.len() {
            return Err(InvariantViolation::SynthesizedSentinelInvalid {
                reason: "sentinel path StringId is dangling",
            });
        }
        if self.strings.resolve(sf.path) != "<synthesized>" {
            return Err(InvariantViolation::SynthesizedSentinelInvalid {
                reason: "sentinel path is not \"<synthesized>\"",
            });
        }
        if sf.content.is_some() {
            return Err(InvariantViolation::SynthesizedSentinelInvalid {
                reason: "sentinel content must be None",
            });
        }
        if sf.line_starts != [0] {
            return Err(InvariantViolation::SynthesizedSentinelInvalid {
                reason: "sentinel line_starts must be vec![0]",
            });
        }
        Ok(())
    }

    fn validate_unique_stable_crate_ids(&self) -> Result<(), InvariantViolation> {
        for i in 0..self.crates.len() {
            for j in (i + 1)..self.crates.len() {
                let a = CrateId::from_u32(i as u32);
                let b = CrateId::from_u32(j as u32);
                if self.crates[a].stable_crate_id == self.crates[b].stable_crate_id {
                    return Err(InvariantViolation::DuplicateStableCrateId {
                        stable_id: self.crates[a].stable_crate_id,
                        first: a,
                        second: b,
                    });
                }
            }
        }
        Ok(())
    }

    fn validate_instance(&self, instance: &Instance) -> Result<(), InvariantViolation> {
        self.check_item_id(instance.item, INSTANCE_ITEM)?;
        self.check_string_id(instance.symbol_name, INSTANCE_SYMBOL_NAME)?;
        self.check_source_file_id(instance.span.file, INSTANCE_SPAN_FILE)?;

        for arg in &instance.generic_args {
            match *arg {
                GenericArg::Type(ty) => self.check_ty_id(ty, INSTANCE_GENERIC_ARG_TY)?,
                GenericArg::Const(c) => self.check_const_id(c, INSTANCE_GENERIC_ARG_CONST)?,
            }
        }

        match instance.kind {
            InstanceKind::Function { sig, .. } => {
                self.check_fn_sig_id(sig, INSTANCE_KIND_FUNCTION_SIG)?;
            }
            InstanceKind::Static { ty, init } => {
                self.check_ty_id(ty, INSTANCE_KIND_STATIC_TY)?;
                self.check_const_id(init, INSTANCE_KIND_STATIC_INIT)?;
            }
            InstanceKind::Const { ty, value } => {
                self.check_ty_id(ty, INSTANCE_KIND_CONST_TY)?;
                self.check_const_id(value, INSTANCE_KIND_CONST_VALUE)?;
            }
            InstanceKind::Struct { layout } => {
                self.check_ty_id(layout, INSTANCE_KIND_STRUCT_LAYOUT)?;
            }
            InstanceKind::Enum { layout } => {
                self.check_ty_id(layout, INSTANCE_KIND_ENUM_LAYOUT)?;
            }
            InstanceKind::Union { layout } => {
                self.check_ty_id(layout, INSTANCE_KIND_UNION_LAYOUT)?;
            }
        }

        Ok(())
    }

    fn check_string_id(&self, id: StringId, field: &'static str) -> Result<(), InvariantViolation> {
        if id.index() >= self.strings.len() {
            return Err(InvariantViolation::DanglingId {
                field,
                value: id.as_u32(),
                table_len: self.strings.len(),
            });
        }
        Ok(())
    }

    fn check_source_file_id(
        &self,
        id: SourceFileId,
        field: &'static str,
    ) -> Result<(), InvariantViolation> {
        if id.index() >= self.source_files.len() {
            return Err(InvariantViolation::DanglingId {
                field,
                value: id.as_u32(),
                table_len: self.source_files.len(),
            });
        }
        Ok(())
    }

    fn check_item_id(&self, id: ItemId, field: &'static str) -> Result<(), InvariantViolation> {
        if id.index() >= self.items.len() {
            return Err(InvariantViolation::DanglingId {
                field,
                value: id.as_u32(),
                table_len: self.items.len(),
            });
        }
        Ok(())
    }

    fn check_ty_id(&self, id: TyId, field: &'static str) -> Result<(), InvariantViolation> {
        if id.index() >= self.types.len() {
            return Err(InvariantViolation::DanglingId {
                field,
                value: id.as_u32(),
                table_len: self.types.len(),
            });
        }
        Ok(())
    }

    fn check_const_id(&self, id: ConstId, field: &'static str) -> Result<(), InvariantViolation> {
        if id.index() >= self.consts.len() {
            return Err(InvariantViolation::DanglingId {
                field,
                value: id.as_u32(),
                table_len: self.consts.len(),
            });
        }
        Ok(())
    }

    fn check_fn_sig_id(&self, id: FnSigId, field: &'static str) -> Result<(), InvariantViolation> {
        if id.index() >= self.sigs.len() {
            return Err(InvariantViolation::DanglingId {
                field,
                value: id.as_u32(),
                table_len: self.sigs.len(),
            });
        }
        Ok(())
    }

    fn is_unsized_marker(&self, ty: TyId) -> bool {
        matches!(self.types[ty], Ty::Str | Ty::Slice(_) | Ty::Dynamic { .. })
    }
}

fn int_kind_index(kind: IntKind) -> usize {
    kind as usize
}

fn uint_kind_index(kind: UintKind) -> usize {
    kind as usize
}

fn float_kind_index(kind: FloatKind) -> usize {
    kind as usize
}

fn round_up_to_multiple(value: u64, boundary: u64) -> u64 {
    assert!(boundary != 0, "array element alignment must be nonzero");
    let remainder = value % boundary;
    if remainder == 0 {
        value
    } else {
        value
            .checked_add(boundary - remainder)
            .expect("array stride overflows u64")
    }
}

fn target_usize_max(pointer_width: u8) -> u64 {
    assert!(pointer_width <= 64, "target pointer width exceeds 64 bits");
    if pointer_width == 64 {
        u64::MAX
    } else {
        (1u64 << pointer_width) - 1
    }
}
