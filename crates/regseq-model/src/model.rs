//! Top-level `RegseqModel` struct and construction-invariant registry.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::args_key::{args_key_for_generic_args, GenericArg};
use crate::body::Body;
use crate::bridge_input::BridgeInput;
use crate::build::BuildInfo;
use crate::cabi::{CAbiLowering, CAbiShape};
use crate::const_interner::{ConstIdentityKey, ConstInterner};
use crate::const_value::{Byte, Const};
use crate::crate_info::{CrateInfo, StableCrateId};
use crate::def_path::{
    def_path_data_sort_rank, render_def_path, DefPath, DefPathData, DefPathHash,
};
use crate::domain_attr::DomainAttrTables;
use crate::domain_attr::IntrinsicKind;
use crate::fn_sig::FnSig;
use crate::fn_sig_interner::FnSigInterner;
use crate::ids::{ConstId, CrateId, FnSigId, InstanceId, ItemId, SourceFileId, StringId, TyId};
use crate::index_vec::IndexVec;
use crate::instance::{
    regseq_instance_kind_rank, rustc_instance_kind_sort_rank, Instance, InstanceKind,
    RegseqInstanceKey,
};
use crate::instance_ref::InstanceRef;
use crate::item::{InlineHint, Item, Linkage};
use crate::layout::{EnumShape, Layout, LayoutKind, TagEncoding, TagScalar, VariantLayout};
use crate::place_validator::{validate_body_places, PlaceValidationError};
use crate::pretty::PrettyPrinter;
use crate::source::{SourceFile, Span};
use crate::string_interner::StringInterner;
use crate::synthetic::{PrincipalTraitKey, SyntheticItemKind};
use crate::target::{default_scalar_layouts, Endian, SizeAlign, TargetInfo};
use crate::ty::{FloatKind, IntKind, Ty, UintKind};
use crate::ty_interner::{TyIdentityKey, TyInterner};

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
const ITEM_DEF_PATH_CRATE: &str = "Item.def_path.crate_id (CrateId)";
const ITEM_DEF_PATH_RENDERED: &str = "Item.def_path.rendered (StringId)";
const ITEM_DEF_PATH_SEGMENT_NAME: &str = "Item.def_path.segments[..].data (StringId)";
const ITEM_SPAN_FILE: &str = "Item.span.file (SourceFileId)";
const ITEM_LINK_SECTION: &str = "Item.link_section (StringId)";
const TY_ID: &str = "Ty nested TyId";
const TY_ITEM: &str = "Ty nominal ItemId";
const TY_FN_SIG: &str = "Ty::FnPtr (FnSigId)";
const TY_GENERIC_ARG_TY: &str = "Ty generic_args[..](TyId)";
const TY_GENERIC_ARG_CONST: &str = "Ty generic_args[..](ConstId)";
const TY_LAYOUT_FIELD_NAME: &str = "Layout.field.name (StringId)";
const TY_LAYOUT_FIELD_TY: &str = "Layout.field.ty (TyId)";
const TY_LAYOUT_VARIANT_NAME: &str = "Layout.variant.name (StringId)";
const TY_LAYOUT_TAG_SCALAR_TY: &str = "Layout.tag.scalar_ty (TyId)";
const CONST_TY: &str = "Const nested TyId";
const CONST_CHILD: &str = "Const nested ConstId";
const CONST_RELOC_INSTANCE: &str = "Const relocation InstanceRef";
const FN_SIG_INPUT: &str = "FnSig.inputs[..] (TyId)";
const FN_SIG_OUTPUT: &str = "FnSig.output (TyId)";
const C_ABI_LOWERING_EMIT_AS: &str = "CAbiLowering.emit_as (TyId)";
const DOMAIN_ATTR_SPAN_FILE: &str = "DomainAttrTables.span_attrs key file (SourceFileId)";
const INSTANCE_ITEM: &str = "Instance.item (ItemId)";
const INSTANCE_EMISSION_NAME: &str = "Instance.emission_name (StringId)";
const INSTANCE_SPAN_FILE: &str = "Instance.span.file (SourceFileId)";
const INSTANCE_GENERIC_ARG_TY: &str = "Instance.generic_args[..](TyId)";
const INSTANCE_GENERIC_ARG_CONST: &str = "Instance.generic_args[..](ConstId)";
const INSTANCE_KIND_FUNCTION_SIG: &str = "InstanceKind::Function.fn_sig (FnSigId)";
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

    /// `Item.def_path.crate_id` does not resolve to `crates`.
    DefPathCrateIdOutOfBounds {
        /// Item carrying the invalid def-path crate.
        item: ItemId,
        /// Invalid crate id.
        crate_id: CrateId,
        /// Length of `crates`.
        crates_len: usize,
    },

    /// Two entries in a content-keyed interner have the same identity key.
    DuplicateInternerEntry {
        /// Interner name.
        interner: &'static str,
        /// First ID as a raw index.
        first: u32,
        /// Second ID as a raw index.
        second: u32,
    },

    /// Two items share the same `DefPathHash`.
    DuplicateItemDefPathHash {
        /// Duplicate hash.
        hash: DefPathHash,
        /// First item carrying the hash.
        first: ItemId,
        /// Second item carrying the hash.
        second: ItemId,
    },

    /// Adjacent `items` entries are not in F4 order.
    F4OutOfOrder {
        /// First adjacent item.
        first: ItemId,
        /// Second adjacent item.
        second: ItemId,
    },

    /// Adjacent `instances` entries are not in F4-2 order.
    F4OutOfOrderInstance {
        /// First adjacent instance.
        first: InstanceId,
        /// Second adjacent instance.
        second: InstanceId,
    },

    /// Stored `DefPath.rendered` does not match canonical rendering.
    DefPathRenderMismatch {
        /// Item carrying the mismatched rendered path.
        item: ItemId,
        /// Canonical rendered path.
        expected: String,
        /// Stored rendered path.
        actual: String,
    },

    /// Source span byte range exceeds file content bounds.
    SpanOutOfBounds {
        /// Source file referenced by the span.
        file: SourceFileId,
        /// Inclusive start byte.
        lo: u32,
        /// Exclusive end byte.
        hi: u32,
        /// Source content length in bytes.
        content_len: usize,
    },

    /// Source span endpoint is not on a UTF-8 boundary.
    SpanNotOnUtf8Boundary {
        /// Source file referenced by the span.
        file: SourceFileId,
        /// Inclusive start byte.
        lo: u32,
        /// Exclusive end byte.
        hi: u32,
        /// Endpoint that failed the boundary check.
        end: &'static str,
    },

    /// Two vtable type items share one principal trait key.
    DuplicateVTablePrincipal {
        /// Duplicate principal key.
        principal: PrincipalTraitKey,
        /// First item carrying the principal.
        first: ItemId,
        /// Second item carrying the principal.
        second: ItemId,
    },

    /// `source_files[0]` is not the synthesized sentinel.
    SynthesizedSentinelInvalid {
        /// Brief reason.
        reason: &'static str,
    },

    /// A function body's place walk found a malformed place.
    PlaceWalk {
        /// Function instance whose body contains the invalid place.
        instance: InstanceId,
        /// First place-validation failure in that body.
        error: PlaceValidationError,
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
                "structurally on `InstanceKind::Function(FunctionInstanceData { body: Option<Body>, ... })`. ",
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
    pub fn validate(&mut self, bridge_input: &BridgeInput) -> Result<(), InvariantViolation> {
        self.validate_synthesized_sentinel()?;

        if self.primary_crate.index() >= self.crates.len() {
            return Err(InvariantViolation::PrimaryCrateOutOfRange {
                value: self.primary_crate.as_u32(),
                crates_len: self.crates.len(),
            });
        }

        for info in self.crates.iter() {
            self.check_string_id(info.name, CRATE_INFO_NAME)?;
        }

        for (idx, file) in self.source_files.iter().enumerate() {
            self.check_string_id(file.path, SOURCE_FILE_PATH)?;
            self.validate_line_starts(SourceFileId::from_u32(idx as u32), file)?;
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

        self.validate_items()?;
        self.validate_f4_item_order()?;

        let primary_stable_id = self.crates[self.primary_crate].stable_crate_id;
        if primary_stable_id != bridge_input.spec_crate_stable_id {
            return Err(InvariantViolation::PrimaryCrateStableIdMismatch {
                model: primary_stable_id,
                bridge_input: bridge_input.spec_crate_stable_id,
            });
        }

        self.validate_const_interner_uniqueness()?;
        self.validate_fn_sig_interner()?;
        self.validate_ty_interner()?;
        self.validate_c_abi_lowerings()?;
        self.validate_synthetic_kinds()?;

        for (idx, instance) in self.instances.iter().enumerate() {
            self.validate_instance(instance)?;
            self.validate_span(instance.span, INSTANCE_SPAN_FILE)?;
            if let InstanceKind::Function(data) = &instance.kind {
                if let Some(body) = data.body.as_ref() {
                    validate_body_places(self, body).map_err(|error| {
                        InvariantViolation::PlaceWalk {
                            instance: InstanceId::from_u32(idx as u32),
                            error,
                        }
                    })?;
                }
            }
        }

        self.validate_value_consts()?;
        self.validate_item_def_path_hash_uniqueness()?;
        self.validate_domain_attr_spans()?;
        self.validate_f4_instance_order()?;
        self.rebuild_vtables()?;

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
        match &self.instances[id].kind {
            InstanceKind::Function(data) => Some(self.sigs.resolve(data.fn_sig)),
            _ => None,
        }
    }

    /// Returns the inline body for function instances that have one.
    pub fn instance_body(&self, id: InstanceId) -> Option<&Body> {
        match &self.instances[id].kind {
            InstanceKind::Function(data) => data.body.as_ref(),
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

    fn validate_items(&self) -> Result<(), InvariantViolation> {
        for (idx, item) in self.items.iter().enumerate() {
            let item_id = ItemId::from_u32(idx as u32);
            if item.def_path.crate_id.index() >= self.crates.len() {
                return Err(InvariantViolation::DefPathCrateIdOutOfBounds {
                    item: item_id,
                    crate_id: item.def_path.crate_id,
                    crates_len: self.crates.len(),
                });
            }
            self.check_crate_id(item.def_path.crate_id, ITEM_DEF_PATH_CRATE)?;
            self.check_string_id(item.def_path.rendered, ITEM_DEF_PATH_RENDERED)?;
            self.validate_def_path_segment_strings(&item.def_path)?;
            self.validate_span(item.span, ITEM_SPAN_FILE)?;
            if let Some(link_section) = item.link_section {
                self.check_string_id(link_section, ITEM_LINK_SECTION)?;
            }

            let expected = render_def_path(&item.def_path, &self.crates, &self.strings);
            let actual = self.strings.resolve(item.def_path.rendered).to_owned();
            if expected != actual {
                return Err(InvariantViolation::DefPathRenderMismatch {
                    item: item_id,
                    expected,
                    actual,
                });
            }
        }
        Ok(())
    }

    fn validate_def_path_segment_strings(&self, path: &DefPath) -> Result<(), InvariantViolation> {
        for segment in &path.segments {
            if let Some(string) = def_path_data_string_id(&segment.data) {
                self.check_string_id(string, ITEM_DEF_PATH_SEGMENT_NAME)?;
            }
        }
        Ok(())
    }

    fn validate_f4_item_order(&self) -> Result<(), InvariantViolation> {
        for idx in 0..self.items.len().saturating_sub(1) {
            let first = ItemId::from_u32(idx as u32);
            let second = ItemId::from_u32((idx + 1) as u32);
            if self.compare_item_f4(first, second).is_gt() {
                return Err(InvariantViolation::F4OutOfOrder { first, second });
            }
        }
        Ok(())
    }

    fn compare_item_f4(&self, first: ItemId, second: ItemId) -> core::cmp::Ordering {
        let first_path = &self.items[first].def_path;
        let second_path = &self.items[second].def_path;
        let first_crate = &self.crates[first_path.crate_id];
        let second_crate = &self.crates[second_path.crate_id];

        self.strings
            .resolve(first_crate.name)
            .cmp(self.strings.resolve(second_crate.name))
            .then_with(|| {
                first_crate
                    .stable_crate_id
                    .cmp(&second_crate.stable_crate_id)
            })
            .then_with(|| {
                self.compare_def_path_segments(&first_path.segments, &second_path.segments)
            })
    }

    fn compare_def_path_segments(
        &self,
        first: &[crate::def_path::DefPathSegment],
        second: &[crate::def_path::DefPathSegment],
    ) -> core::cmp::Ordering {
        let first_keys = first
            .iter()
            .map(|segment| self.def_path_segment_sort_key(segment))
            .collect::<Vec<_>>();
        let second_keys = second
            .iter()
            .map(|segment| self.def_path_segment_sort_key(segment))
            .collect::<Vec<_>>();
        first_keys.cmp(&second_keys)
    }

    fn def_path_segment_sort_key<'a>(
        &'a self,
        segment: &'a crate::def_path::DefPathSegment,
    ) -> (u8, &'a str, u32) {
        (
            def_path_data_sort_rank(&segment.data),
            def_path_data_string_id(&segment.data)
                .map(|id| self.strings.resolve(id))
                .unwrap_or(""),
            segment.disambiguator,
        )
    }

    fn validate_const_interner_uniqueness(&self) -> Result<(), InvariantViolation> {
        let mut seen = HashMap::new();
        for (id, value) in self.consts.iter() {
            let key = ConstIdentityKey::from_const(value);
            if let Some(first) = seen.insert(key, id) {
                return Err(InvariantViolation::DuplicateInternerEntry {
                    interner: "ConstInterner",
                    first: first.as_u32(),
                    second: id.as_u32(),
                });
            }
        }
        Ok(())
    }

    fn validate_fn_sig_interner(&self) -> Result<(), InvariantViolation> {
        let mut seen = HashMap::new();
        for (id, sig) in self.sigs.iter() {
            if let Some(first) = seen.insert(sig.clone(), id) {
                return Err(InvariantViolation::DuplicateInternerEntry {
                    interner: "FnSigInterner",
                    first: first.as_u32(),
                    second: id.as_u32(),
                });
            }
            for input in &sig.inputs {
                self.check_ty_id(*input, FN_SIG_INPUT)?;
            }
            self.check_ty_id(sig.output, FN_SIG_OUTPUT)?;
        }
        Ok(())
    }

    fn validate_ty_interner(&self) -> Result<(), InvariantViolation> {
        let mut seen = HashMap::new();
        for (id, ty) in self.types.iter() {
            let key = TyIdentityKey::from_ty(ty);
            if let Some(first) = seen.insert(key, id) {
                return Err(InvariantViolation::DuplicateInternerEntry {
                    interner: "TyInterner",
                    first: first.as_u32(),
                    second: id.as_u32(),
                });
            }
            self.validate_ty_references(ty)?;
        }
        Ok(())
    }

    fn validate_ty_references(&self, ty: &Ty) -> Result<(), InvariantViolation> {
        match ty {
            Ty::Array(elem, _) | Ty::Slice(elem) | Ty::Ref(elem, _) | Ty::Ptr(elem, _) => {
                self.check_ty_id(*elem, TY_ID)?;
            }
            Ty::FnDef(item, args) | Ty::Closure(item, args) => {
                self.check_item_id(*item, TY_ITEM)?;
                self.validate_generic_args(args, TY_GENERIC_ARG_TY, TY_GENERIC_ARG_CONST)?;
            }
            Ty::FnPtr(sig) => self.check_fn_sig_id(*sig, TY_FN_SIG)?,
            Ty::Dynamic { vtable_ty, .. } => self.check_ty_id(*vtable_ty, TY_ID)?,
            Ty::Tuple { fields, layout } => {
                for field in fields {
                    self.check_ty_id(*field, TY_ID)?;
                }
                self.validate_layout_references(layout)?;
            }
            Ty::Struct { item, args, layout }
            | Ty::Enum { item, args, layout }
            | Ty::Union { item, args, layout } => {
                self.check_item_id(*item, TY_ITEM)?;
                self.validate_generic_args(args, TY_GENERIC_ARG_TY, TY_GENERIC_ARG_CONST)?;
                self.validate_layout_references(layout)?;
            }
            Ty::Bool | Ty::Char | Ty::Int(_) | Ty::Uint(_) | Ty::Float(_) | Ty::Str | Ty::Never => {
            }
        }
        Ok(())
    }

    fn validate_layout_references(&self, layout: &Layout) -> Result<(), InvariantViolation> {
        match &layout.kind {
            LayoutKind::Struct { fields } | LayoutKind::Union { fields } => {
                self.validate_field_layouts(fields)?;
            }
            LayoutKind::Enum { shape } => match shape {
                EnumShape::Empty => {}
                EnumShape::Single { variant, scalar } => {
                    self.validate_variant_layout(variant)?;
                    if let Some(scalar) = scalar {
                        self.validate_tag_scalar(scalar)?;
                    }
                }
                EnumShape::Multiple {
                    tag,
                    encoding,
                    variants,
                } => {
                    self.validate_tag_scalar(tag)?;
                    if let TagEncoding::Niche {
                        untagged_variant,
                        niche_variants,
                        ..
                    } = encoding
                    {
                        self.check_variant_idx(*untagged_variant, variants.len())?;
                        self.check_variant_idx(niche_variants.start, variants.len())?;
                        self.check_variant_idx(niche_variants.end, variants.len())?;
                    }
                    for variant in variants {
                        self.validate_variant_layout(variant)?;
                    }
                }
            },
        }
        Ok(())
    }

    fn validate_field_layouts(
        &self,
        fields: &[crate::layout::FieldLayout],
    ) -> Result<(), InvariantViolation> {
        for field in fields {
            self.check_string_id(field.name, TY_LAYOUT_FIELD_NAME)?;
            self.check_ty_id(field.ty, TY_LAYOUT_FIELD_TY)?;
        }
        Ok(())
    }

    fn validate_variant_layout(&self, variant: &VariantLayout) -> Result<(), InvariantViolation> {
        self.check_string_id(variant.name, TY_LAYOUT_VARIANT_NAME)?;
        self.validate_field_layouts(&variant.fields)
    }

    fn validate_tag_scalar(&self, scalar: &TagScalar) -> Result<(), InvariantViolation> {
        self.check_ty_id(scalar.scalar_ty, TY_LAYOUT_TAG_SCALAR_TY)
    }

    fn validate_c_abi_lowerings(&self) -> Result<(), InvariantViolation> {
        for lowering in self.sig_c_abi_lowerings.iter().flatten() {
            for shape in &lowering.inputs {
                self.validate_c_abi_shape(shape)?;
            }
            self.validate_c_abi_shape(&lowering.output)?;
        }
        Ok(())
    }

    fn validate_c_abi_shape(&self, shape: &CAbiShape) -> Result<(), InvariantViolation> {
        match shape {
            CAbiShape::Direct => Ok(()),
            CAbiShape::Substituted { emit_as, .. } => {
                self.check_ty_id(*emit_as, C_ABI_LOWERING_EMIT_AS)
            }
        }
    }

    fn validate_synthetic_kinds(&self) -> Result<(), InvariantViolation> {
        for kind in self.item_synthetic_kinds.iter() {
            match kind {
                SyntheticItemKind::None
                | SyntheticItemKind::ClosureCaptures { .. }
                | SyntheticItemKind::ClosureCall { .. }
                | SyntheticItemKind::ClosureDropGlue { .. }
                | SyntheticItemKind::VTableType { .. }
                | SyntheticItemKind::VTableInstance { .. }
                | SyntheticItemKind::VTableMethodShim { .. }
                | SyntheticItemKind::VTableDropShim { .. }
                | SyntheticItemKind::ConstMemoryStatic { .. } => {}
            }
        }
        Ok(())
    }

    fn validate_value_consts(&self) -> Result<(), InvariantViolation> {
        let instance_refs = self.instance_refs()?;
        for value in self.consts.iter().map(|(_, value)| value) {
            self.validate_const_references(value, &instance_refs)?;
        }
        Ok(())
    }

    fn validate_const_references(
        &self,
        value: &Const,
        instance_refs: &HashSet<InstanceRef>,
    ) -> Result<(), InvariantViolation> {
        match value {
            Const::Aggregate(children) => {
                for child in children {
                    self.check_const_id(*child, CONST_CHILD)?;
                }
            }
            Const::EnumValue { payload, .. } => {
                for child in payload {
                    self.check_const_id(*child, CONST_CHILD)?;
                }
            }
            Const::NullPtr(ty) => self.check_ty_id(*ty, CONST_TY)?,
            Const::Reloc(instance) => {
                self.check_instance_ref(*instance, instance_refs, CONST_RELOC_INSTANCE)?
            }
            Const::Bytes(bytes) => {
                for byte in bytes {
                    if let Byte::Reloc(instance) = byte {
                        self.check_instance_ref(*instance, instance_refs, CONST_RELOC_INSTANCE)?;
                    }
                }
            }
            Const::Bool(_)
            | Const::Char(_)
            | Const::Int(_, _)
            | Const::Uint(_, _)
            | Const::Float(_, _)
            | Const::ZST => {}
        }
        Ok(())
    }

    fn validate_item_def_path_hash_uniqueness(&self) -> Result<(), InvariantViolation> {
        let mut seen = HashMap::new();
        for (idx, hash) in self.item_def_paths.iter().copied().enumerate() {
            let item = ItemId::from_u32(idx as u32);
            if let Some(first) = seen.insert(hash, item) {
                return Err(InvariantViolation::DuplicateItemDefPathHash {
                    hash,
                    first,
                    second: item,
                });
            }
        }
        Ok(())
    }

    fn validate_domain_attr_spans(&self) -> Result<(), InvariantViolation> {
        for span in self.domain_attrs.span_attrs.keys() {
            self.check_source_file_id(span.file, DOMAIN_ATTR_SPAN_FILE)?;
            self.validate_span(*span, DOMAIN_ATTR_SPAN_FILE)?;
        }
        Ok(())
    }

    fn validate_f4_instance_order(&self) -> Result<(), InvariantViolation> {
        for idx in 0..self.instances.len().saturating_sub(1) {
            let first = InstanceId::from_u32(idx as u32);
            let second = InstanceId::from_u32((idx + 1) as u32);
            let first_key = self.regseq_instance_key(&self.instances[first]);
            let second_key = self.regseq_instance_key(&self.instances[second]);
            if first_key > second_key {
                return Err(InvariantViolation::F4OutOfOrderInstance { first, second });
            }
        }
        Ok(())
    }

    fn rebuild_vtables(&mut self) -> Result<(), InvariantViolation> {
        let mut rebuilt = HashMap::new();
        for (idx, kind) in self.item_synthetic_kinds.iter().enumerate() {
            if let SyntheticItemKind::VTableType { principal } = kind {
                let item = ItemId::from_u32(idx as u32);
                if let Some(first) = rebuilt.insert(principal.clone(), item) {
                    return Err(InvariantViolation::DuplicateVTablePrincipal {
                        principal: principal.clone(),
                        first,
                        second: item,
                    });
                }
            }
        }
        self.vtables = rebuilt;
        Ok(())
    }

    fn validate_line_starts(
        &self,
        file_id: SourceFileId,
        file: &SourceFile,
    ) -> Result<(), InvariantViolation> {
        if file.line_starts.first().copied() != Some(0) {
            return Err(InvariantViolation::SpanOutOfBounds {
                file: file_id,
                lo: file.line_starts.first().copied().unwrap_or(1),
                hi: file.line_starts.first().copied().unwrap_or(1),
                content_len: file.content.as_ref().map_or(0, |text| text.len()),
            });
        }
        if file.line_starts.windows(2).any(|pair| pair[0] > pair[1]) {
            return Err(InvariantViolation::SpanOutOfBounds {
                file: file_id,
                lo: 0,
                hi: 0,
                content_len: file.content.as_ref().map_or(0, |text| text.len()),
            });
        }
        Ok(())
    }

    fn validate_span(&self, span: Span, field: &'static str) -> Result<(), InvariantViolation> {
        self.check_source_file_id(span.file, field)?;
        let file = &self.source_files[span.file];
        let Some(content) = file.content.as_ref() else {
            return Ok(());
        };
        let lo = span.lo as usize;
        let hi = span.hi as usize;
        if lo > hi || hi > content.len() {
            return Err(InvariantViolation::SpanOutOfBounds {
                file: span.file,
                lo: span.lo,
                hi: span.hi,
                content_len: content.len(),
            });
        }
        if !content.is_char_boundary(lo) {
            return Err(InvariantViolation::SpanNotOnUtf8Boundary {
                file: span.file,
                lo: span.lo,
                hi: span.hi,
                end: "lo",
            });
        }
        if !content.is_char_boundary(hi) {
            return Err(InvariantViolation::SpanNotOnUtf8Boundary {
                file: span.file,
                lo: span.lo,
                hi: span.hi,
                end: "hi",
            });
        }
        Ok(())
    }

    fn instance_refs(&self) -> Result<HashSet<InstanceRef>, InvariantViolation> {
        let mut refs = HashSet::new();
        for instance in self.instances.iter() {
            refs.insert(self.instance_ref(instance)?);
        }
        Ok(refs)
    }

    fn check_instance_ref(
        &self,
        reference: InstanceRef,
        instance_refs: &HashSet<InstanceRef>,
        field: &'static str,
    ) -> Result<(), InvariantViolation> {
        if !instance_refs.contains(&reference) {
            return Err(InvariantViolation::DanglingId {
                field,
                value: 0,
                table_len: self.instances.len(),
            });
        }
        Ok(())
    }

    fn instance_ref(&self, instance: &Instance) -> Result<InstanceRef, InvariantViolation> {
        self.check_item_id(instance.item, INSTANCE_ITEM)?;
        Ok(InstanceRef {
            regseq_kind_rank: regseq_instance_kind_rank(&instance.kind),
            def_path_hash: self.item_def_paths[instance.item],
            args_key: args_key_for_generic_args(self, &instance.generic_args),
            fn_shim_disambig: function_instance_disambig(&instance.kind),
        })
    }

    fn regseq_instance_key(&self, instance: &Instance) -> RegseqInstanceKey {
        RegseqInstanceKey {
            regseq_kind_rank: regseq_instance_kind_rank(&instance.kind),
            item: instance.item,
            generic_args_key: args_key_for_generic_args(self, &instance.generic_args),
            fn_shim_disambig: function_instance_disambig(&instance.kind),
        }
    }

    fn validate_generic_args(
        &self,
        args: &[GenericArg],
        ty_field: &'static str,
        const_field: &'static str,
    ) -> Result<(), InvariantViolation> {
        for arg in args {
            match *arg {
                GenericArg::Type(ty) => self.check_ty_id(ty, ty_field)?,
                GenericArg::Const(c) => self.check_const_id(c, const_field)?,
            }
        }
        Ok(())
    }

    fn validate_instance(&self, instance: &Instance) -> Result<(), InvariantViolation> {
        self.check_item_id(instance.item, INSTANCE_ITEM)?;
        self.check_string_id(instance.emission_name, INSTANCE_EMISSION_NAME)?;
        self.check_source_file_id(instance.span.file, INSTANCE_SPAN_FILE)?;

        for arg in &instance.generic_args {
            match *arg {
                GenericArg::Type(ty) => self.check_ty_id(ty, INSTANCE_GENERIC_ARG_TY)?,
                GenericArg::Const(c) => self.check_const_id(c, INSTANCE_GENERIC_ARG_CONST)?,
            }
        }

        match &instance.kind {
            InstanceKind::Function(data) => {
                self.check_fn_sig_id(data.fn_sig, INSTANCE_KIND_FUNCTION_SIG)?;
            }
            InstanceKind::Static { ty, init } => {
                self.check_ty_id(*ty, INSTANCE_KIND_STATIC_TY)?;
                if let Some(init) = *init {
                    self.check_const_id(init, INSTANCE_KIND_STATIC_INIT)?;
                }
            }
            InstanceKind::Const { ty, value } => {
                self.check_ty_id(*ty, INSTANCE_KIND_CONST_TY)?;
                self.check_const_id(*value, INSTANCE_KIND_CONST_VALUE)?;
            }
            InstanceKind::Struct { layout } => {
                self.check_ty_id(*layout, INSTANCE_KIND_STRUCT_LAYOUT)?;
            }
            InstanceKind::Enum { layout } => {
                self.check_ty_id(*layout, INSTANCE_KIND_ENUM_LAYOUT)?;
            }
            InstanceKind::Union { layout } => {
                self.check_ty_id(*layout, INSTANCE_KIND_UNION_LAYOUT)?;
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

    fn check_crate_id(&self, id: CrateId, field: &'static str) -> Result<(), InvariantViolation> {
        if id.index() >= self.crates.len() {
            return Err(InvariantViolation::DanglingId {
                field,
                value: id.as_u32(),
                table_len: self.crates.len(),
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

    fn check_variant_idx(
        &self,
        id: crate::VariantIdx,
        variants_len: usize,
    ) -> Result<(), InvariantViolation> {
        if id.index() >= variants_len {
            return Err(InvariantViolation::DanglingId {
                field: "Layout variant index (VariantIdx)",
                value: id.as_u32(),
                table_len: variants_len,
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

fn def_path_data_string_id(data: &DefPathData) -> Option<StringId> {
    match data {
        DefPathData::TypeNs(id)
        | DefPathData::ValueNs(id)
        | DefPathData::MacroNs(id)
        | DefPathData::LifetimeNs(id)
        | DefPathData::OpaqueLifetime(id)
        | DefPathData::AnonAssocTy(id) => Some(*id),
        DefPathData::CrateRoot
        | DefPathData::Impl
        | DefPathData::ForeignMod
        | DefPathData::Use
        | DefPathData::GlobalAsm
        | DefPathData::Closure
        | DefPathData::Ctor
        | DefPathData::AnonConst
        | DefPathData::LateAnonConst
        | DefPathData::DesugaredAnonymousLifetime
        | DefPathData::OpaqueTy
        | DefPathData::SyntheticCoroutineBody
        | DefPathData::NestedStatic => None,
    }
}

fn function_instance_disambig(kind: &InstanceKind) -> u8 {
    match kind {
        InstanceKind::Function(data) => rustc_instance_kind_sort_rank(data.flavor),
        InstanceKind::Static { .. }
        | InstanceKind::Const { .. }
        | InstanceKind::Struct { .. }
        | InstanceKind::Enum { .. }
        | InstanceKind::Union { .. } => 0,
    }
}
