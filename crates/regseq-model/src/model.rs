//! Top-level `RegseqModel` struct and construction-invariant registry.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::args_key::{args_key_for_generic_args, GenericArg};
use crate::body::Body;
use crate::body_ty::{
    operand_ty, place_enum_variant_count, place_ty, rvalue_ty, TypeRecovery, VariantRecovery,
};
use crate::bridge_input::BridgeInput;
use crate::build::BuildInfo;
use crate::cabi::{CAbiLowering, CAbiShape};
use crate::const_interner::{ConstIdentityKey, ConstInterner};
use crate::const_value::{Byte, Const};
use crate::crate_info::{CrateInfo, StableCrateId};
use crate::def_path::{
    def_path_data_sort_rank, render_def_path, DefPath, DefPathData, DefPathHash,
};
use crate::domain_attr::AttrData;
use crate::domain_attr::DomainAttrTables;
use crate::domain_attr::IntrinsicKind;
use crate::fn_sig::FnSig;
use crate::fn_sig_interner::FnSigInterner;
use crate::ids::{
    BlockId, ConstId, CrateId, FnSigId, InstanceId, ItemId, SourceFileId, StringId, TyId,
};
use crate::index_vec::IndexVec;
use crate::instance::{
    regseq_instance_kind_rank, rustc_instance_kind_sort_rank, FunctionInstanceFlavor, Instance,
    InstanceKind, RegseqInstanceKey,
};
use crate::instance_ref::InstanceRef;
use crate::item::{InlineHint, Item, ItemKind, Linkage};
use crate::layout::{EnumShape, Layout, LayoutKind, TagEncoding, TagScalar, VariantLayout};
use crate::operand::Operand;
use crate::place::PlaceElem;
use crate::place_validator::{validate_body_places, PlaceValidationError};
use crate::pretty::PrettyPrinter;
use crate::rvalue::{AggregateKind, CastKind, Rvalue};
use crate::source::{SourceFile, Span};
use crate::statement::{CopyNonOverlapping, NonDivergingIntrinsic, StatementKind};
use crate::string_interner::StringInterner;
use crate::synthetic::{PrincipalTraitKey, SyntheticItemKind};
use crate::target::{default_scalar_layouts, Endian, SizeAlign, TargetInfo};
use crate::terminator::{AssertKind, CallCallee, TerminatorKind};
use crate::ty::{FloatKind, IntKind, Mutability, Ty, UintKind};
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

    /// Bridge-side mapping from a `Ty::Closure` TyId to the parallel
    /// `Ty::Struct` captures-view TyId per spec.md section 13.1. Both
    /// `Ty` entries share the same `ItemId`; this side table records
    /// the dual view so consumers can recover the layout-bearing
    /// captures struct from a closure type-identity reference.
    /// Rebuilt after deserialization (the `Ty` entries themselves
    /// suffice for round-tripping, so the map is recomputed lazily by
    /// consumers).
    #[serde(skip)]
    pub closure_captures_struct_ty: HashMap<TyId, TyId>,

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

    /// `StringInterner` sentinel violated -- the spec mandates
    /// `StringId(0) == ""` and a non-empty table (spec section 4 /
    /// section 12 Phase B step 1).
    StringInternerSentinelInvalid {
        /// Brief reason describing which sentinel rule was violated.
        reason: &'static str,
    },

    /// `StringInterner` rebuild found two entries with the same
    /// string content (spec section 12 Phase B step 1 "content
    /// uniqueness").
    DuplicateString {
        /// First `StringId` carrying the duplicate content.
        first: StringId,
        /// Second `StringId` carrying the duplicate content.
        second: StringId,
    },

    /// A function body's place walk found a malformed place.
    PlaceWalk {
        /// Function instance whose body contains the invalid place.
        instance: InstanceId,
        /// First place-validation failure in that body.
        error: PlaceValidationError,
    },

    /// A `SyntheticItemKind` payload does not match the shape its
    /// owning `ItemKind` advertises (spec section 13: e.g.
    /// `VTableType` must own a `Struct` Item; `VTableInstance` must
    /// own a `Static { mutability: Not }` Item).
    SyntheticKindItemKindMismatch {
        /// Offending item.
        item: ItemId,
        /// Synthetic kind that drove the consistency check.
        synthetic_kind: SyntheticItemKind,
        /// Actual item kind that violated the consistency rule.
        item_kind: ItemKind,
    },

    /// A `CallCallee::Direct` references an `InstanceId` outside
    /// `RegseqModel.instances` (terminator invariant B-9, spec
    /// section 23.6).
    DirectCalleeOutOfRange {
        /// Function instance whose body contains the unresolved
        /// direct call.
        owner: InstanceId,
        /// Block containing the unresolved direct call.
        block: BlockId,
        /// Unresolved callee identifier.
        callee: InstanceId,
        /// Length of `RegseqModel.instances` at validation time.
        instances_len: usize,
    },

    /// `TargetInfo` violates invariant 15 well-formedness.
    TargetMalformed {
        /// Brief reason describing the violation.
        reason: &'static str,
    },

    /// `BuildInfo` violates invariant 17 well-formedness.
    BuildMalformed {
        /// Brief reason describing the violation.
        reason: &'static str,
    },

    /// A `FnSig` violates `c_variadic` or input-shape invariants.
    FnSigMalformed {
        /// Offending signature.
        sig: FnSigId,
        /// Brief reason describing the violation.
        reason: &'static str,
    },

    /// A `CAbiLowering` entry violates a structural rule.
    CAbiLoweringMalformed {
        /// Signature whose lowering entry is malformed.
        sig: FnSigId,
        /// Brief reason describing the violation.
        reason: &'static str,
    },

    /// A `Layout` violates §11.5 / §12.5 sanity rules.
    LayoutMalformed {
        /// Type whose layout is malformed.
        ty: TyId,
        /// Brief reason describing the violation.
        reason: &'static str,
    },

    /// A tag scalar carries a value outside the scalar bit width.
    TagScalarOutOfRange {
        /// Type whose tag-bearing layout is offending.
        ty: TyId,
        /// Out-of-range tag value.
        value: u128,
        /// Bit width of the tag scalar type.
        scalar_width_bits: u32,
    },

    /// An `InstanceKind` layout TyId does not resolve to the matching
    /// nominal variant.
    InstanceLayoutMismatch {
        /// Offending instance.
        instance: InstanceId,
        /// Expected nominal kind name.
        expected_kind: &'static str,
        /// Resolved kind name.
        found_kind: &'static str,
    },

    /// An `InstanceKind` layout's `(item, args)` does not match the
    /// instance's `(item, generic_args)`.
    InstanceLayoutItemArgsMismatch {
        /// Offending instance.
        instance: InstanceId,
    },

    /// A back-edge was found while walking value-level const
    /// recursion edges.
    ConstGraphCycle {
        /// Entry point used to discover the cycle.
        entry_point: ConstId,
        /// Cycle path containing the back edge.
        cycle: Vec<ConstId>,
    },

    /// A domain-attribute sibling table is malformed.
    DomainAttrMalformed {
        /// Brief reason describing the violation.
        reason: &'static str,
    },

    /// An opaque-handle eligibility flag is inconsistent with the
    /// model.
    OpaqueHandleInconsistent {
        /// Offending item.
        item: ItemId,
        /// Brief reason describing the violation.
        reason: &'static str,
    },

    /// A real source crate carries `StableCrateId(0)`.
    StableCrateIdZeroCollision {
        /// Offending crate id.
        crate_id: CrateId,
    },

    /// A synthetic-domain boundary is violated.
    SyntheticDomainBoundary {
        /// Offending item.
        item: ItemId,
        /// Brief reason describing the violation.
        reason: &'static str,
    },

    /// An empty `Ty::Tuple { fields: [] }` does not carry
    /// `UNIT_LAYOUT`.
    UnitLayoutMismatch {
        /// Offending tuple type.
        ty: TyId,
    },

    /// A `SourceFile` is malformed.
    SourceFileMalformed {
        /// Offending source file.
        file: SourceFileId,
        /// Brief reason describing the violation.
        reason: &'static str,
    },

    /// Two `SourceFile` entries share the same `path` string id.
    DuplicateSourceFilePath {
        /// First source file carrying the duplicate path.
        first: SourceFileId,
        /// Second source file carrying the duplicate path.
        second: SourceFileId,
        /// Shared path string id.
        path: StringId,
    },

    /// A `LocalDecl.ty`, projection-carried `TyId`, or other
    /// body-embedded `TyId` references an entry outside
    /// `model.types`. Spec section 25.6.3 invariant 1 fold-in.
    BodyTyIdOutOfRange {
        /// Function instance whose body carries the dangling TyId.
        owner: InstanceId,
        /// Diagnostic-only label describing the offending slot.
        site: &'static str,
        /// Offending TyId.
        ty: TyId,
        /// Length of `model.types` at validation time.
        types_len: usize,
    },

    /// `Assign(p, rv)` has `place_ty(p) != rvalue_ty(rv)`. Spec
    /// section 22.5.2 B-1.
    StatementAssignTypeMismatch {
        /// Function instance whose body contains the bad assignment.
        owner: InstanceId,
        /// Block containing the bad assignment.
        block: BlockId,
        /// Statement index within the block.
        stmt: usize,
        /// Recovered place type.
        place_ty: TyId,
        /// Recovered rvalue type.
        rvalue_ty: TyId,
    },

    /// `Assign(p, rv)` could not validate B-1 because the body's
    /// type recovery failed at the place or rvalue side. Spec
    /// section 22.5.2 B-1 with audit-mandated malformed-body
    /// distinction.
    StatementAssignMalformed {
        /// Function instance whose body contains the assignment.
        owner: InstanceId,
        /// Block containing the assignment.
        block: BlockId,
        /// Statement index within the block.
        stmt: usize,
        /// Which side failed recovery: `"place"` or `"rvalue"`.
        side: &'static str,
    },

    /// `SetDiscriminant(p, vi)` has `vi >= variant_count(place_ty(p))`.
    /// Spec section 22.5.2 B-2.
    SetDiscriminantVariantIdxOutOfRange {
        /// Function instance whose body contains the violation.
        owner: InstanceId,
        /// Block containing the violation.
        block: BlockId,
        /// Statement index within the block.
        stmt: usize,
        /// Offending variant index.
        variant: crate::ids::VariantIdx,
        /// Exclusive upper bound for valid variant indices.
        max: u32,
    },

    /// `SetDiscriminant(p, _)` was applied to a non-enum
    /// `place_ty(p)`. Spec section 22.5.2 B-3.
    SetDiscriminantNonEnumTarget {
        /// Function instance whose body contains the violation.
        owner: InstanceId,
        /// Block containing the violation.
        block: BlockId,
        /// Statement index within the block.
        stmt: usize,
        /// Recovered place type.
        place_ty: TyId,
    },

    /// `Intrinsic(Assume(op))` has `operand_ty(op) != Ty::Bool`. Spec
    /// section 22.5.2 B-4.
    AssumeOperandNotBool {
        /// Function instance whose body contains the violation.
        owner: InstanceId,
        /// Block containing the violation.
        block: BlockId,
        /// Statement index within the block.
        stmt: usize,
        /// Recovered operand type.
        operand_ty: TyId,
    },

    /// A `CopyNonOverlapping` operand has a type outside the spec
    /// section 22.5.2 B-5 admitted set. `which` identifies the
    /// offending position: `"src"`, `"dst"`, `"count"`, or
    /// `"pointee"` for `src` / `dst` pointee disagreement.
    CopyNonOverlappingOperandType {
        /// Function instance whose body contains the violation.
        owner: InstanceId,
        /// Block containing the violation.
        block: BlockId,
        /// Statement index within the block.
        stmt: usize,
        /// Offending operand position.
        which: &'static str,
        /// Actual operand type.
        actual_ty: TyId,
    },

    /// `SwitchInt.discr` has a type outside
    /// `{Ty::Bool, Ty::Char, Ty::Int(_), Ty::Uint(_)}`. Spec
    /// section 23.6.2 B-1.
    SwitchIntDiscrTypeMismatch {
        /// Function instance whose body contains the violation.
        owner: InstanceId,
        /// Block containing the violation.
        block: BlockId,
        /// Recovered discriminant type.
        operand_ty: TyId,
    },

    /// `Assert.cond` has a type other than `Ty::Bool`. Spec
    /// section 23.6.2 B-2.
    AssertCondNotBool {
        /// Function instance whose body contains the violation.
        owner: InstanceId,
        /// Block containing the violation.
        block: BlockId,
        /// Recovered condition type.
        cond_ty: TyId,
    },

    /// `Call.destination` type disagrees with the callee's return
    /// type when `target: Some(_)`. Spec section 23.6.2 B-3.
    CallDestinationTypeMismatch {
        /// Function instance whose body contains the violation.
        owner: InstanceId,
        /// Block containing the violation.
        block: BlockId,
        /// Callee return type.
        expected_ty: TyId,
        /// Recovered destination type.
        actual_ty: TyId,
    },

    /// `Call.args.len()` disagrees with the callee signature's input
    /// count. Spec section 23.6.2 B-4.
    CallArgsArityMismatch {
        /// Function instance whose body contains the violation.
        owner: InstanceId,
        /// Block containing the violation.
        block: BlockId,
        /// Expected signature arity.
        expected: usize,
        /// Actual `Call.args` length.
        actual: usize,
    },

    /// `Call.callee = Direct(_)` resolves to a non-Function instance.
    /// Spec section 23.6.2 B-5 Direct branch.
    CallDirectCalleeNotFunction {
        /// Function instance whose body contains the violation.
        owner: InstanceId,
        /// Block containing the violation.
        block: BlockId,
        /// The non-Function callee id.
        callee: InstanceId,
    },

    /// `Call.callee = Indirect(op)` whose `operand_ty(op)` is not
    /// `Ty::FnPtr(_)`. Spec section 23.6.2 B-5 Indirect branch.
    CallIndirectCalleeNotFnPtr {
        /// Function instance whose body contains the violation.
        owner: InstanceId,
        /// Block containing the violation.
        block: BlockId,
        /// Recovered callee operand type.
        operand_ty: TyId,
    },

    /// `Call.args[i]` has a type that disagrees with the callee
    /// signature's `inputs[i]`. Spec section 23.6.2 B-6.
    CallArgTypeMismatch {
        /// Function instance whose body contains the violation.
        owner: InstanceId,
        /// Block containing the violation.
        block: BlockId,
        /// Argument index.
        idx: usize,
        /// Callee signature input type.
        expected_ty: TyId,
        /// Recovered argument type.
        actual_ty: TyId,
    },

    /// `Call.callee = Direct(<virtual instance>)` with
    /// `virtual_idx == None`, or a non-Virtual flavor with
    /// `virtual_idx == Some(_)`. Spec section 23.6.2 B-8 +
    /// section 9.4 biconditional.
    CallVirtualIdxInconsistent {
        /// Function instance whose body contains the violation.
        owner: InstanceId,
        /// Block containing the violation.
        block: BlockId,
        /// The callee whose flavor / virtual_idx are inconsistent.
        callee: InstanceId,
        /// Brief reason describing the inconsistency.
        reason: &'static str,
    },

    /// `Call.target.is_none()` does not match
    /// `callee_output == Ty::Never`. Spec section 23.6.2 B-9.
    CallTargetDivergenceMismatch {
        /// Function instance whose body contains the violation.
        owner: InstanceId,
        /// Block containing the violation.
        block: BlockId,
        /// Brief reason describing the inconsistency.
        reason: &'static str,
    },

    /// Log-intrinsic `Call.args[0]` is not `Ty::Ref(Ty::Str, _)` per
    /// the spec.md section 26.6 post-DiagLogPeel flat-arg shape
    /// (`spec.md:12705+`); the format-template slot must carry the
    /// `&'static str` literal operand. Surfaced through the spec.md
    /// section 23.6.2 B-4 / B-6 I-DiagLog-CallArity exception
    /// branch (`spec.md:9649-9778`).
    DiagLogCallArg0NotStrRef {
        /// Function instance whose body contains the violation.
        owner: InstanceId,
        /// Block containing the violation.
        block: BlockId,
        /// Recovered `args[0]` type, or `TyId::ZERO` when the
        /// operand's type recovery was `Malformed`.
        actual_ty: TyId,
    },

    /// Log-intrinsic `Call.args` is empty; the post-DiagLogPeel
    /// flat-arg shape per spec.md section 26.6 must guarantee at
    /// least the format-template slot at `args[0]`.
    DiagLogCallArityBelowMinimum {
        /// Function instance whose body contains the violation.
        owner: InstanceId,
        /// Block containing the violation.
        block: BlockId,
    },

    /// Log-intrinsic `Call.args[idx]` (`idx >= 1`) does not resolve
    /// to a value-typed `Ty` in the spec.md section 26.4 admissible
    /// LogArg-source set (`Ty::Uint(U8..U64)`, `Ty::Int(I8..I64)`,
    /// `Ty::Bool`, or `Ty::Ref(Ty::Str, _)`).
    DiagLogCallArgNotAdmissible {
        /// Function instance whose body contains the violation.
        owner: InstanceId,
        /// Block containing the violation.
        block: BlockId,
        /// Offending argument index (always `>= 1`).
        idx: usize,
        /// Recovered argument type, or `TyId::ZERO` when the
        /// operand's type recovery was `Malformed`.
        actual_ty: TyId,
    },

    /// A source-derived `Item` that requires a covering `Instance`
    /// has none.
    ///
    /// Per spec.md section 8.1 `ItemKind` mirrors `InstanceKind`
    /// and spec.md section 3.4 Phase 5 constructs one Instance per
    /// `(Item, GenericArgs, rustc shim flavor)` tuple; this extends
    /// the section 3.1 invariant 14 `InstanceKind`-consistency
    /// family with the Item-side coverage direction. Scoped to
    /// source-derived (`SyntheticItemKind::None`) Items; foreign
    /// source declarations (`linkage == Linkage::Foreign`) are
    /// carved out (their `body` / `init` is legitimately `None` and
    /// whether they need an Instance row is SPEC-AMBIGUOUS, out of
    /// scope per the regseq-driver-55 Non-Goals). Synthetic Items'
    /// Instance lifecycle is owned by the synth passes + residue
    /// DCE (Non-Goals 5 + 6) and is not policed here. See
    /// `validate_bridge_completeness` for the full rationale.
    ItemMissingInstance {
        /// Item lacking a covering `Instance`.
        item: ItemId,
        /// The uncovered Item's kind.
        item_kind: ItemKind,
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
    /// 21. Log-intrinsic `Call.args[0]` is `Ty::Ref(Ty::Str, _)` per spec.md section 23.6.2 B-4 / B-6 I-DiagLog-CallArity exception (`spec.md:9649-9778`) over the section 26.6 post-DiagLogPeel flat-arg shape.
    DiagLogCallArg0IsStrRef,
    /// 22. Log-intrinsic `Call.args` carries at least one operand (the format-template slot at `args[0]`) per spec.md section 26.6 post-DiagLogPeel flat-arg shape.
    DiagLogCallArityFloor,
    /// 23. Log-intrinsic `Call.args[idx]` for `idx >= 1` resolves to a value-typed entry in the spec.md section 26.4 admissible LogArg-source set.
    DiagLogCallArgAdmissibleLogArgSource,
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
        InvariantId::DiagLogCallArg0IsStrRef => {
            concat!(
                "Log-intrinsic Call args[0] is `Ty::Ref(Ty::Str, _)`. The post-DiagLogPeel ",
                "flat-arg shape per spec.md section 26.6 pins the format-template slot at ",
                "args[0] as a `&'static str` operand; the section 23.6.2 B-4 / B-6 ",
                "I-DiagLog-CallArity exception checks this shape instead of the declared ",
                "callee FnSig's first input."
            )
        }
        InvariantId::DiagLogCallArityFloor => {
            concat!(
                "Log-intrinsic Call args has at least one operand. The post-DiagLogPeel ",
                "flat-arg shape per spec.md section 26.6 guarantees `args.len() >= 1` ",
                "(the format-template slot at args[0]); the validator enforces this as a ",
                "defensive precondition before any args[0] access."
            )
        }
        InvariantId::DiagLogCallArgAdmissibleLogArgSource => {
            concat!(
                "Log-intrinsic Call args[idx] for `idx >= 1` is in the admissible ",
                "LogArg-source set per spec.md section 26.4: `Ty::Uint(U8..U64)`, ",
                "`Ty::Int(I8..I64)`, `Ty::Bool`, or `Ty::Ref(Ty::Str, _)`. The post-",
                "DiagLogPeel flat-arg shape replaces the declared callee FnSig's ",
                "`&[LogArg<'_>]` input with these peeled value operands."
            )
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
            closure_captures_struct_ty: HashMap::new(),
            domain_attrs: DomainAttrTables::default(),
        }
    }

    /// Convenience accessor for `PrettyPrinter::new(self)`.
    pub fn pretty(&self) -> PrettyPrinter<'_> {
        PrettyPrinter::new(self)
    }

    /// Compares two item payloads by the F4 item sort contract.
    pub fn compare_items_f4(&self, first: &Item, second: &Item) -> core::cmp::Ordering {
        let first_path = &first.def_path;
        let second_path = &second.def_path;
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

    /// Sort `model.instances` by the F4-2 instance sort key per
    /// spec.md section 16.3, remapping every `CallCallee::Direct`
    /// `InstanceId` reference in materialised function bodies so the
    /// terminator graph survives the permutation. Idempotent: a model
    /// already in F4-2 order stays unchanged.
    ///
    /// The sort key is
    /// `(regseq_kind_rank, item_id, generic_args_key, fn_shim_disambig)`
    /// per `RegseqInstanceKey`. Function instances (rank 0) precede
    /// Static instances (rank 1), satisfying the F4-2 constraint that
    /// vtable Static initialisers land after every Function instance
    /// even when the bridge's per-phase push order does not.
    /// Audit-finding HIGH-40-2.
    pub fn sort_instances_by_f4_2(&mut self) {
        use crate::args_key::args_key_for_generic_args;
        use crate::basic_block::BasicBlock;
        use crate::ids::InstanceId;
        use crate::instance::{
            regseq_instance_kind_rank, rustc_instance_kind_sort_rank, InstanceKind,
        };
        use crate::terminator::{CallCallee, TerminatorKind};

        if self.instances.len() <= 1 {
            return;
        }

        // 1. Compute the F4-2 key for every existing instance.
        type F42Key = (u8, u32, u128, u8);
        let keys: Vec<F42Key> = self
            .instances
            .iter()
            .map(|inst| {
                let kind_rank = regseq_instance_kind_rank(&inst.kind);
                let item_id = inst.item.as_u32();
                let args_key = args_key_for_generic_args(self, &inst.generic_args).0;
                let fn_shim_disambig = match &inst.kind {
                    InstanceKind::Function(data) => rustc_instance_kind_sort_rank(data.flavor),
                    _ => 0,
                };
                (kind_rank, item_id, args_key, fn_shim_disambig)
            })
            .collect();

        // 2. Sort indices by key to derive the permutation.
        let mut order: Vec<usize> = (0..keys.len()).collect();
        order.sort_by_key(|&idx| keys[idx]);

        // Bail out early if already sorted -- avoids quadratic body
        // walks on repeat invocations.
        if order.iter().enumerate().all(|(new, old)| new == *old) {
            return;
        }

        // 3. Build the old->new remap and apply it to the storage.
        let mut remap = vec![InstanceId::ZERO; self.instances.len()];
        for (new_idx, &old_idx) in order.iter().enumerate() {
            remap[old_idx] = InstanceId::from_u32(new_idx as u32);
        }
        let mut old_vec = std::mem::take(&mut self.instances).into_inner();
        let mut taken: Vec<Option<_>> = old_vec.drain(..).map(Some).collect();
        let mut sorted: Vec<_> = Vec::with_capacity(taken.len());
        for &old_idx in &order {
            sorted.push(
                taken[old_idx]
                    .take()
                    .expect("permutation visits each old index once"),
            );
        }
        self.instances.replace_inner(sorted);

        // 4. Remap every `CallCallee::Direct(InstanceId)` reference
        // inside materialised function bodies. `InstanceRef`
        // (used by `Const::Reloc`) is content-addressed and stable
        // across permutation, so const-side cross-references need no
        // patching. Per spec.md section 23.2 the only InstanceId-bearing
        // shape on the IR side is `CallCallee::Direct`.
        let mut bodies_to_patch: Vec<(InstanceId, crate::body::Body)> = Vec::new();
        for (idx, inst) in self.instances.iter().enumerate() {
            if let Some(function) = inst.kind.as_function() {
                if let Some(body) = function.body.as_ref() {
                    bodies_to_patch.push((InstanceId::from_u32(idx as u32), body.clone()));
                }
            }
        }
        for (instance_id, mut body) in bodies_to_patch {
            remap_body_call_callees(&mut body, &remap);
            let function = self
                .instances
                .iter_mut()
                .nth(instance_id.as_u32() as usize)
                .and_then(|inst| inst.kind.as_function_mut())
                .expect("function instance must remain after permutation");
            function.body = Some(body);
        }

        fn remap_body_call_callees(body: &mut crate::body::Body, remap: &[crate::ids::InstanceId]) {
            for block in body.basic_blocks.iter_mut() {
                remap_block_callees(block, remap);
            }
        }

        fn remap_block_callees(block: &mut BasicBlock, remap: &[crate::ids::InstanceId]) {
            if let TerminatorKind::Call {
                callee: CallCallee::Direct(id),
                ..
            } = &mut block.terminator.kind
            {
                let old = id.as_u32() as usize;
                if old < remap.len() {
                    *id = remap[old];
                }
            }
        }
    }

    /// Apply `f` to every `ItemId` reference in the model. Visits:
    /// - Type-interner nominal entries: `Ty::Enum`, `Ty::Struct`,
    ///   `Ty::Union`, `Ty::FnDef`, and `Ty::Closure`. The
    ///   `TyInterner` rebuilds its content-keyed lookup map after
    ///   the walk so subsequent intern lookups resolve through the
    ///   remapped identity keys.
    /// - `Instance.item` for every instance.
    /// - Side-table values: `vtables`'s
    ///   `HashMap<PrincipalTraitKey, ItemId>` values.
    /// - Side-table keys: `domain_attrs.field_attrs`'s
    ///   `FieldPath.item` field. The map is rebuilt with
    ///   remapped keys so subsequent lookups find the entries
    ///   under their new `FieldPath`.
    ///
    /// Does NOT visit `IndexVec<ItemId, _>` side tables -- those
    /// are positional and must be permuted (not remapped) by
    /// callers via the shared permutation table.
    ///
    /// Does NOT visit bridge-side `HandleTable` -- `HandleTable`
    /// lives on the driver side
    /// (`crates/regseq-driver/src/bridge/handles.rs`,
    /// re-exported from `crates/regseq-driver/src/bridge/mod.rs`)
    /// and is not part of `RegseqModel`. Bridge-side callers apply
    /// the same remap separately via
    /// `HandleTable::remap_item_ids`.
    pub(crate) fn for_each_item_id_mut<F>(&mut self, mut f: F)
    where
        F: FnMut(&mut ItemId),
    {
        self.types.for_each_nominal_item_id_mut(&mut f);

        for instance in self.instances.iter_mut() {
            f(&mut instance.item);
        }

        for item in self.vtables.values_mut() {
            f(item);
        }

        let drained: Vec<_> = std::mem::take(&mut self.domain_attrs.field_attrs)
            .into_iter()
            .collect();
        for (mut key, value) in drained {
            f(&mut key.item);
            let prev = self.domain_attrs.field_attrs.insert(key, value);
            debug_assert!(
                prev.is_none(),
                "domain_attrs.field_attrs collision after ItemId remap",
            );
        }
    }

    /// Sort `model.items` by `compare_items_f4` per spec.md section
    /// 16.1, remapping every `ItemId` reference everywhere it
    /// appears in the model so the type / instance / side-table
    /// graph survives the permutation. Idempotent: a model already
    /// in F4 order stays unchanged.
    ///
    /// Mirrors `sort_instances_by_f4_2` (spec.md section 16.2)
    /// for the items-side ordering. The instance-side method exists
    /// because Phase-5 sub-passes push Function and Static instances
    /// in distinct orders; this items-side method exists because
    /// `assign_late_pending_items`
    /// (`crates/regseq-driver/src/bridge/assign.rs`) appends late
    /// synthesis items past the original F4-sorted batch, which
    /// can interleave them out of order with the existing items
    /// per spec.md section 16.1 ranking.
    ///
    /// Returns the old-to-new `ItemId` remap table
    /// (`Vec<ItemId>` indexed by the old `ItemId`'s `index()`).
    /// Callers MUST apply the same remap to any bridge-side state
    /// that holds `ItemId`s outside the model (e.g.,
    /// `HandleTable.handle_to_item_id` via
    /// `HandleTable::remap_item_ids`).
    pub fn sort_items_by_f4(&mut self) -> Vec<ItemId> {
        let len = self.items.len();
        let identity: Vec<ItemId> = (0..len).map(|idx| ItemId::from_u32(idx as u32)).collect();
        if len <= 1 {
            return identity;
        }

        let mut order: Vec<usize> = (0..len).collect();
        order.sort_by(|&a, &b| {
            self.compare_items_f4(
                &self.items[ItemId::from_u32(a as u32)],
                &self.items[ItemId::from_u32(b as u32)],
            )
        });

        if order.iter().enumerate().all(|(new, &old)| new == old) {
            return identity;
        }

        permute_index_vec(&mut self.items, &order);
        permute_index_vec(&mut self.item_def_paths, &order);
        permute_index_vec(&mut self.item_synthetic_kinds, &order);
        permute_index_vec(&mut self.domain_attrs.item_attrs, &order);

        let mut remap = vec![ItemId::ZERO; len];
        for (new_idx, &old_idx) in order.iter().enumerate() {
            remap[old_idx] = ItemId::from_u32(new_idx as u32);
        }

        self.for_each_item_id_mut(|item| {
            let new = remap[item.index()];
            *item = new;
        });

        #[cfg(debug_assertions)]
        for idx in 0..self.items.len().saturating_sub(1) {
            let first = ItemId::from_u32(idx as u32);
            let second = ItemId::from_u32((idx + 1) as u32);
            debug_assert!(
                !self.compare_item_f4(first, second).is_gt(),
                "F4 monotonicity broken after sort_items_by_f4 between {:?} and {:?}",
                first,
                second,
            );
        }

        remap
    }

    /// Apply `f` to every `TyId` reference in the model. Visits the
    /// surfaces enumerated in `regseq-driver-48` Change D:
    ///
    /// - Type-interner entries: every `Ty` variant that embeds a
    ///   `TyId` (`Array`, `Slice`, `Ref`, `Ptr`, `Dynamic.vtable_ty`,
    ///   `Tuple.fields`, `Enum/Struct/Union.args` + nested
    ///   `GenericArg::Type`, `FnDef.args`, `Closure.args`), plus
    ///   every inline `Layout` TyId (`FieldLayout.ty`,
    ///   `TagScalar.scalar_ty`).
    /// - `FnSig` entries: every `inputs[*]` and `output`.
    /// - `sig_c_abi_lowerings[*]`: every `Some(CAbiLowering)`'s
    ///   per-position `CAbiShape::Substituted { emit_as, .. }`.
    /// - Body local_decls: `LocalDecl.ty`.
    /// - Body statements + terminators: every `Rvalue::Cast` TyId,
    ///   every `Rvalue::Aggregate(AggregateKind, _)` TyId,
    ///   `PlaceElem::Field(_, TyId)` reachable through any Place
    ///   projection (in Assign LHS, Rvalue, Operand, or
    ///   Terminator).
    /// - Const interner: `Const::NullPtr(TyId)`.
    /// - Instances: `InstanceKind::Static.ty`,
    ///   `InstanceKind::Const.ty`, `InstanceKind::Struct.layout`,
    ///   `InstanceKind::Enum.layout`, `InstanceKind::Union.layout`,
    ///   plus every `Instance.generic_args[*]::Type(TyId)`.
    ///
    /// Does NOT visit `closure_captures_struct_ty` (its HashMap
    /// keys require re-insertion; callers performing TyId remap
    /// rebuild that map separately, e.g.,
    /// `remove_tys_and_remap`).
    ///
    /// Rebuilds the content-keyed lookup maps on the TyInterner,
    /// FnSigInterner, and ConstInterner after the walk because
    /// every interner's identity key embeds at least one of the
    /// visited surfaces.
    pub(crate) fn for_each_ty_id_mut<F: FnMut(&mut TyId)>(&mut self, mut f: F) {
        self.types
            .for_each_ty_mut(|ty| visit_ty_ids_in_ty(ty, &mut f));
        self.sigs.for_each_fn_sig_mut(|sig| {
            for input in sig.inputs.iter_mut() {
                f(input);
            }
            f(&mut sig.output);
        });
        for lowering in self.sig_c_abi_lowerings.iter_mut().flatten() {
            for shape in lowering.inputs.iter_mut() {
                if let CAbiShape::Substituted { emit_as, .. } = shape {
                    f(emit_as);
                }
            }
            if let CAbiShape::Substituted { emit_as, .. } = &mut lowering.output {
                f(emit_as);
            }
        }
        for instance in self.instances.iter_mut() {
            for arg in instance.generic_args.iter_mut() {
                if let GenericArg::Type(ty) = arg {
                    f(ty);
                }
            }
            match &mut instance.kind {
                InstanceKind::Function(data) => {
                    if let Some(body) = &mut data.body {
                        visit_ty_ids_in_body(body, &mut f);
                    }
                }
                InstanceKind::Static { ty, .. } => f(ty),
                InstanceKind::Const { ty, .. } => f(ty),
                InstanceKind::Struct { layout } => f(layout),
                InstanceKind::Enum { layout } => f(layout),
                InstanceKind::Union { layout } => f(layout),
            }
        }
        self.consts.for_each_const_mut(|value| {
            if let Const::NullPtr(ty) = value {
                f(ty);
            }
        });
        self.types.rebuild_lookup();
        self.sigs.rebuild_lookup();
        self.consts.rebuild_lookup();
    }

    /// Apply `f` to every `FnSigId` reference in the model. Visits
    /// the surfaces enumerated in `regseq-driver-48` Change D:
    ///
    /// - Type-interner: `Ty::FnPtr(FnSigId)`.
    /// - Every `InstanceKind::Function.fn_sig`.
    ///
    /// Does NOT visit the `sig_c_abi_lowerings` `IndexVec` indexer
    /// (`sig_c_abi_lowerings` is positional and is permuted in
    /// lock-step with FnSig removal by `remove_fn_sigs_and_remap`).
    ///
    /// Rebuilds the content-keyed lookup map on the TyInterner
    /// after the walk because `Ty::FnPtr` identity embeds the
    /// visited `FnSigId`.
    pub(crate) fn for_each_fn_sig_id_mut<F: FnMut(&mut FnSigId)>(&mut self, mut f: F) {
        self.types.for_each_ty_mut(|ty| {
            if let Ty::FnPtr(sig) = ty {
                f(sig);
            }
        });
        for instance in self.instances.iter_mut() {
            if let InstanceKind::Function(data) = &mut instance.kind {
                f(&mut data.fn_sig);
            }
        }
        self.types.rebuild_lookup();
    }

    /// Remove `Item` entries at every index where
    /// `item_keep[idx] == false`, compacting `model.items` and
    /// every `IndexVec<ItemId, _>` side table (`item_def_paths`,
    /// `item_synthetic_kinds`, `domain_attrs.item_attrs`) in
    /// lock-step, and remap every surviving `ItemId` reference
    /// through the shared `for_each_item_id_mut` walker.
    ///
    /// Returns the old-to-new `ItemId` remap as
    /// `Vec<Option<ItemId>>`. `None` marks a removed item;
    /// `Some(new_id)` marks a surviving item's new index.
    ///
    /// Removed items must have NO surviving references in the
    /// model -- this is a caller responsibility. The method does
    /// NOT verify the absence of references; callers performing a
    /// residue-driven DCE (`regseq-driver-48`) compute the keep
    /// set so this contract holds.
    ///
    /// Panics if `item_keep.len() != self.items.len()`.
    pub fn remove_items_and_remap(&mut self, item_keep: &[bool]) -> Vec<Option<ItemId>> {
        assert_eq!(
            item_keep.len(),
            self.items.len(),
            "remove_items_and_remap keep slice length must match items length",
        );
        let old_len = self.items.len();
        let mut remap: Vec<Option<ItemId>> = vec![None; old_len];
        let mut next: u32 = 0;
        for (idx, keep) in item_keep.iter().enumerate() {
            if *keep {
                remap[idx] = Some(ItemId::from_u32(next));
                next += 1;
            }
        }
        permute_index_vec_with_keep(&mut self.items, item_keep);
        permute_index_vec_with_keep(&mut self.item_def_paths, item_keep);
        permute_index_vec_with_keep(&mut self.item_synthetic_kinds, item_keep);
        permute_index_vec_with_keep(&mut self.domain_attrs.item_attrs, item_keep);
        self.for_each_item_id_mut(|item| {
            let new = remap[item.index()].expect(
                "remove_items_and_remap: surviving ItemId reference points to a removed item",
            );
            *item = new;
        });
        remap
    }

    /// Remove `Instance` entries at every index where
    /// `instance_keep[idx] == false`, compacting `model.instances`
    /// and remapping every surviving `CallCallee::Direct`
    /// `InstanceId` reference inside materialised function bodies.
    ///
    /// Returns the old-to-new `InstanceId` remap as
    /// `Vec<Option<InstanceId>>`. `None` marks a removed instance;
    /// `Some(new_id)` marks a surviving instance's new index.
    ///
    /// Removed instances must have NO surviving references; the
    /// method does NOT verify.
    ///
    /// Panics if `instance_keep.len() != self.instances.len()`.
    pub fn remove_instances_and_remap(
        &mut self,
        instance_keep: &[bool],
    ) -> Vec<Option<InstanceId>> {
        assert_eq!(
            instance_keep.len(),
            self.instances.len(),
            "remove_instances_and_remap keep slice length must match instances length",
        );
        let old_len = self.instances.len();
        let mut remap: Vec<Option<InstanceId>> = vec![None; old_len];
        let mut next: u32 = 0;
        for (idx, keep) in instance_keep.iter().enumerate() {
            if *keep {
                remap[idx] = Some(InstanceId::from_u32(next));
                next += 1;
            }
        }
        permute_index_vec_with_keep(&mut self.instances, instance_keep);
        for instance in self.instances.iter_mut() {
            if let InstanceKind::Function(data) = &mut instance.kind {
                if let Some(body) = &mut data.body {
                    remap_body_call_callees_optional(body, &remap);
                }
            }
        }
        remap
    }

    /// Remove `Ty` entries at every `TyId` where
    /// `ty_keep[idx] == false`, compacting the `TyInterner`'s
    /// internal slab, and remap every surviving `TyId` reference
    /// through the shared `for_each_ty_id_mut` walker.
    ///
    /// In addition to walking the `for_each_ty_id_mut` surface,
    /// this method REBUILDS `model.closure_captures_struct_ty`
    /// with remapped keys + values. `HashMap` key mutation
    /// requires re-insertion; TyInterner compaction shifts every
    /// surviving `TyId`, so the keys (and values) must be
    /// rewritten even when no LogArg `TyId` is itself a key.
    /// Pairs whose key OR value resolves to a REMOVED `TyId`
    /// panic in debug builds rather than silently drop (the
    /// residue-driven contract requires the caller to compute a
    /// keep set that preserves every closure-capture `TyId`).
    ///
    /// Returns the old-to-new `TyId` remap as `Vec<Option<TyId>>`.
    /// `None` marks a removed entry; `Some(new_id)` marks a
    /// surviving entry's new index.
    ///
    /// Removed `Ty` entries must have NO surviving references; the
    /// method does NOT verify.
    ///
    /// Panics if `ty_keep.len() != self.types.len()`.
    pub fn remove_tys_and_remap(&mut self, ty_keep: &[bool]) -> Vec<Option<TyId>> {
        assert_eq!(
            ty_keep.len(),
            self.types.len(),
            "remove_tys_and_remap keep slice length must match TyInterner storage length",
        );
        let remap = self.types.compact_by_keep(ty_keep);
        self.for_each_ty_id_mut(|ty_id| {
            let new = remap[ty_id.index()]
                .expect("remove_tys_and_remap: surviving TyId reference points to a removed entry");
            *ty_id = new;
        });
        let drained: Vec<(TyId, TyId)> = self.closure_captures_struct_ty.drain().collect();
        for (key, value) in drained {
            let new_key = remap[key.index()].expect(
                "remove_tys_and_remap: closure_captures_struct_ty key points to a removed Ty",
            );
            let new_value = remap[value.index()].expect(
                "remove_tys_and_remap: closure_captures_struct_ty value points to a removed Ty",
            );
            let prev = self.closure_captures_struct_ty.insert(new_key, new_value);
            debug_assert!(
                prev.is_none(),
                "closure_captures_struct_ty key collision after TyId remap",
            );
        }
        remap
    }

    /// Remove `FnSig` entries at every `FnSigId` where
    /// `fn_sig_keep[idx] == false`, compacting both the
    /// `FnSigInterner`'s internal slab AND
    /// `model.sig_c_abi_lowerings` in lock-step (the latter is a
    /// `IndexVec<FnSigId, Option<CAbiLowering>>` parallel side
    /// table; validation requires
    /// `sig_c_abi_lowerings.len() == sigs.len()`). Remap every
    /// surviving `FnSigId` reference via the shared
    /// `for_each_fn_sig_id_mut` walker.
    ///
    /// Returns the old-to-new `FnSigId` remap as
    /// `Vec<Option<FnSigId>>`. `None` marks a removed entry;
    /// `Some(new_id)` marks a surviving entry's new index.
    ///
    /// Removed `FnSig` entries must have NO surviving references;
    /// the method does NOT verify.
    ///
    /// Panics if `fn_sig_keep.len() != self.sigs.len()`.
    pub fn remove_fn_sigs_and_remap(&mut self, fn_sig_keep: &[bool]) -> Vec<Option<FnSigId>> {
        assert_eq!(
            fn_sig_keep.len(),
            self.sigs.len(),
            "remove_fn_sigs_and_remap keep slice length must match FnSigInterner storage length",
        );
        let remap = self.sigs.compact_by_keep(fn_sig_keep);
        permute_index_vec_with_keep(&mut self.sig_c_abi_lowerings, fn_sig_keep);
        self.for_each_fn_sig_id_mut(|sig_id| {
            let new = remap[sig_id.index()].expect(
                "remove_fn_sigs_and_remap: surviving FnSigId reference points to a removed entry",
            );
            *sig_id = new;
        });
        self.sigs.rebuild_lookup();
        remap
    }

    /// Validates construction invariants enforceable at this phase.
    pub fn validate(&mut self, bridge_input: &BridgeInput) -> Result<(), InvariantViolation> {
        // Phase B step 1 (spec section 12, line 676-680 ordering):
        // string interner sentinel + content uniqueness precede the
        // synthesized-source-file sentinel check, because the latter
        // resolves `source_files[0].path` through `model.strings`.
        self.validate_string_interner()?;
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

        for file in self.source_files.iter() {
            self.check_string_id(file.path, SOURCE_FILE_PATH)?;
        }
        crate::layout_validator::validate_source_files(self)?;

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

        crate::layout_validator::validate_target(self)?;
        crate::layout_validator::validate_build(self)?;

        self.validate_const_interner_uniqueness()?;
        self.validate_fn_sig_interner()?;
        crate::layout_validator::validate_fn_sigs(self)?;
        crate::layout_validator::validate_c_abi_lowerings(self)?;
        self.validate_ty_interner()?;
        crate::layout_validator::validate_layouts(self)?;
        self.validate_synthetic_kinds()?;

        for (idx, instance) in self.instances.iter().enumerate() {
            self.validate_instance(instance)?;
            self.validate_span(instance.span, INSTANCE_SPAN_FILE)?;
            if let InstanceKind::Function(data) = &instance.kind {
                if let Some(body) = data.body.as_ref() {
                    let owner = InstanceId::from_u32(idx as u32);
                    // TyId closure walk (spec section 25.6.3
                    // invariant 1) runs first so a dangling
                    // `LocalDecl.ty` does not cause the §19.7 place
                    // walk to panic when it indexes
                    // `model.types[..]`.
                    self.validate_body_ty_closure(owner, body)?;
                    validate_body_places(self, body).map_err(|error| {
                        InvariantViolation::PlaceWalk {
                            instance: owner,
                            error,
                        }
                    })?;
                    self.validate_body_direct_callees(owner, body)?;
                    self.validate_body_statement_invariants(owner, body)?;
                    self.validate_body_terminator_invariants(owner, body)?;
                }
            }
        }

        crate::layout_validator::validate_instance_layouts(self)?;
        // Per regseq-driver-55 Change G (spec v2): Item-vs-Instance
        // bridge-completeness is DELIBERATELY NOT folded into the
        // model-correctness `validate` entry point. It is the
        // separate `validate_bridge_completeness` sibling method
        // invoked from `bridge::run_bridge`; see that method's doc
        // comment for the partial-model-corpus rationale.

        self.validate_value_consts()?;
        crate::layout_validator::validate_const_graph_acyclic(self)?;
        crate::layout_validator::validate_domain_attrs(self)?;
        self.validate_domain_attr_spans()?;
        self.validate_item_def_path_hash_uniqueness()?;
        crate::layout_validator::validate_stable_crate_id_zero(self)?;
        crate::layout_validator::validate_opaque_handle_eligibility(self)?;
        crate::layout_validator::validate_unit_layout(self)?;
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

    /// Returns the closed-vocabulary intrinsic marker for an
    /// instance's item per spec.md section 3.5.2. Walks
    /// `domain_attrs.item_attrs[item]` for the first
    /// `AttrData::Intrinsic(_)` entry. The bridge populates this
    /// during Phase-2 translation per spec.md section 3.6.4 (see
    /// `regseq-driver-44`'s Phase-2 recognition codepath).
    ///
    /// Returns `None` when:
    /// - `id` is out of range (defensive bound; surfaces as a skip
    ///   rather than a panic, mirroring the bridge-side
    ///   `call_is_log_intrinsic` discipline at
    ///   `crates/regseq-driver/src/bridge/diag_log/peel.rs:556-580`).
    /// - The Instance's item has no `IntrinsicKind` entry in
    ///   `domain_attrs.item_attrs`.
    /// - Bridge ordering: the Instance's `item.index() >=
    ///   domain_attrs.item_attrs.len()` (Phase-4 side-table sizing;
    ///   should not occur post-Phase-4 but guarded defensively).
    ///
    /// `item_attrs[item]` is a `Vec<AttrData>` that may carry multiple
    /// per-domain payloads per item; this helper returns the first
    /// `AttrData::Intrinsic(_)` discriminant. At most one `Intrinsic`
    /// entry per item is expected per spec.md section 3.6.4 (the
    /// Phase-2 recognition codepath pushes exactly one), but if
    /// multiple are present (a bridge bug), the helper returns the
    /// first -- downstream validators read the discriminant only, so
    /// first-wins is well-defined.
    pub fn instance_intrinsic(&self, id: InstanceId) -> Option<&IntrinsicKind> {
        if id.index() >= self.instances.len() {
            return None;
        }
        let item = self.instances[id].item;
        if item.index() >= self.domain_attrs.item_attrs.len() {
            return None;
        }
        // Explicit `for` + `if let` rather than `find_map` so a
        // future non-Intrinsic `AttrData` variant is naturally
        // skipped (the equivalent `find_map(|attr| match attr {
        // AttrData::Intrinsic(k) => Some(k) })` collapses to
        // `map(..).next()` under clippy because the single arm
        // always returns Some, defeating the future-proof intent
        // documented above). The `if let` is currently irrefutable
        // because `AttrData::Intrinsic` is the only variant, hence
        // the per-block allow.
        #[allow(irrefutable_let_patterns)]
        for attr in self.domain_attrs.item_attrs[item].iter() {
            if let AttrData::Intrinsic(kind) = attr {
                return Some(kind);
            }
        }
        None
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
        self.compare_items_f4(&self.items[first], &self.items[second])
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

    /// Spec section 12 Phase B step 1: the `StringInterner` table is
    /// non-empty, `StringId(0)` holds the empty string sentinel, and
    /// every string is content-unique. Construction-time interning
    /// already guarantees uniqueness, but the spec requires the
    /// validator to re-verify the rebuild against the serialised
    /// table.
    fn validate_string_interner(&self) -> Result<(), InvariantViolation> {
        if self.strings.is_empty() {
            return Err(InvariantViolation::StringInternerSentinelInvalid {
                reason: "StringInterner table is empty; spec mandates StringId(0) sentinel",
            });
        }
        if !self.strings.resolve(StringId::from_u32(0)).is_empty() {
            return Err(InvariantViolation::StringInternerSentinelInvalid {
                reason: "StringId(0) does not resolve to the empty string",
            });
        }
        let mut seen: HashMap<&str, StringId> = HashMap::with_capacity(self.strings.len());
        for (id, value) in self.strings.iter() {
            if let Some(first) = seen.insert(value, id) {
                return Err(InvariantViolation::DuplicateString { first, second: id });
            }
        }
        Ok(())
    }

    fn validate_synthetic_kinds(&self) -> Result<(), InvariantViolation> {
        for (idx, kind) in self.item_synthetic_kinds.iter().enumerate() {
            let item_id = ItemId::from_u32(idx as u32);
            let item_kind = &self.items[item_id].kind;
            let consistent = match kind {
                // Source-derived items are unconstrained at this
                // check; their ItemKind reflects the source ADT or
                // callable shape.
                SyntheticItemKind::None => true,
                // Closure captures are a synthesised struct.
                SyntheticItemKind::ClosureCaptures { .. } => {
                    matches!(item_kind, ItemKind::Struct)
                }
                // Closure call / drop-glue / drop-glue-for-concrete
                // / vtable method shim / vtable drop shim are all
                // synthesised functions.
                SyntheticItemKind::ClosureCall { .. }
                | SyntheticItemKind::ClosureDropGlue { .. }
                | SyntheticItemKind::DropGlue { .. }
                | SyntheticItemKind::VTableMethodShim { .. }
                | SyntheticItemKind::VTableDropShim { .. } => {
                    matches!(item_kind, ItemKind::Function)
                }
                // VTable type is a synthesised struct.
                SyntheticItemKind::VTableType { .. } => {
                    matches!(item_kind, ItemKind::Struct)
                }
                // VTable instance and anonymous const-memory both
                // back immutable statics.
                SyntheticItemKind::VTableInstance { .. }
                | SyntheticItemKind::ConstMemoryStatic { .. } => matches!(
                    item_kind,
                    ItemKind::Static {
                        mutability: Mutability::Not,
                    }
                ),
            };
            if !consistent {
                return Err(InvariantViolation::SyntheticKindItemKindMismatch {
                    item: item_id,
                    synthetic_kind: kind.clone(),
                    item_kind: item_kind.clone(),
                });
            }
        }
        Ok(())
    }

    /// Terminator invariant B-9 (spec section 23.6): every
    /// `CallCallee::Direct(InstanceId)` resolves in
    /// `RegseqModel.instances`. Bounds-only at this phase; type
    /// invariants B-3 / B-4 / B-6 are validated separately when
    /// statement and terminator type invariants land.
    fn validate_body_direct_callees(
        &self,
        owner: InstanceId,
        body: &Body,
    ) -> Result<(), InvariantViolation> {
        for (block_idx, block) in body.basic_blocks.iter().enumerate() {
            if let TerminatorKind::Call {
                callee: CallCallee::Direct(callee),
                ..
            } = &block.terminator.kind
            {
                if callee.index() >= self.instances.len() {
                    return Err(InvariantViolation::DirectCalleeOutOfRange {
                        owner,
                        block: BlockId::from_u32(block_idx as u32),
                        callee: *callee,
                        instances_len: self.instances.len(),
                    });
                }
            }
        }
        Ok(())
    }

    /// Closes construction invariant 1 over body content (spec
    /// section 25.6.3): every `LocalDecl.ty`, every projection-
    /// carried field type, every `Rvalue::Cast` destination, every
    /// `AggregateKind` carrier, and every Operand-context type
    /// resolves within `model.types`.
    fn validate_body_ty_closure(
        &self,
        owner: InstanceId,
        body: &Body,
    ) -> Result<(), InvariantViolation> {
        let check = |ty: TyId, site: &'static str| -> Result<(), InvariantViolation> {
            if ty.index() >= self.types.len() {
                return Err(InvariantViolation::BodyTyIdOutOfRange {
                    owner,
                    site,
                    ty,
                    types_len: self.types.len(),
                });
            }
            Ok(())
        };

        for decl in body.local_decls.iter() {
            check(decl.ty, "LocalDecl.ty")?;
        }

        for block in body.basic_blocks.iter() {
            for stmt in block.statements.iter() {
                match &stmt.kind {
                    StatementKind::Assign(place, rvalue) => {
                        self.body_ty_closure_place(owner, place)?;
                        self.body_ty_closure_rvalue(owner, rvalue)?;
                    }
                    StatementKind::SetDiscriminant(place, _) => {
                        self.body_ty_closure_place(owner, place)?;
                    }
                    StatementKind::Intrinsic(intrinsic) => match intrinsic {
                        NonDivergingIntrinsic::Assume(operand) => {
                            self.body_ty_closure_operand(owner, operand)?;
                        }
                        NonDivergingIntrinsic::CopyNonOverlapping(payload) => {
                            let CopyNonOverlapping { src, dst, count } = payload;
                            self.body_ty_closure_operand(owner, src)?;
                            self.body_ty_closure_operand(owner, dst)?;
                            self.body_ty_closure_operand(owner, count)?;
                        }
                    },
                }
            }
            self.body_ty_closure_terminator(owner, &block.terminator.kind)?;
        }
        Ok(())
    }

    fn body_ty_closure_place(
        &self,
        owner: InstanceId,
        place: &crate::place::Place,
    ) -> Result<(), InvariantViolation> {
        for elem in place.projection.iter() {
            if let PlaceElem::Field(_, ty) = elem {
                if ty.index() >= self.types.len() {
                    return Err(InvariantViolation::BodyTyIdOutOfRange {
                        owner,
                        site: "PlaceElem::Field.ty",
                        ty: *ty,
                        types_len: self.types.len(),
                    });
                }
            }
        }
        Ok(())
    }

    fn body_ty_closure_operand(
        &self,
        owner: InstanceId,
        operand: &Operand,
    ) -> Result<(), InvariantViolation> {
        match operand {
            Operand::Copy(place) | Operand::Move(place) => self.body_ty_closure_place(owner, place),
            Operand::Constant(const_id) => {
                if const_id.index() >= self.consts.len() {
                    return Err(InvariantViolation::DanglingId {
                        field: "Operand::Constant.ConstId",
                        value: const_id.as_u32(),
                        table_len: self.consts.len(),
                    });
                }
                Ok(())
            }
        }
    }

    fn body_ty_closure_rvalue(
        &self,
        owner: InstanceId,
        rvalue: &Rvalue,
    ) -> Result<(), InvariantViolation> {
        match rvalue {
            Rvalue::Use(op) | Rvalue::Repeat(op, _) | Rvalue::UnaryOp(_, op) => {
                self.body_ty_closure_operand(owner, op)
            }
            Rvalue::Ref(_, place) | Rvalue::RawPtr(_, place) | Rvalue::Discriminant(place) => {
                self.body_ty_closure_place(owner, place)
            }
            Rvalue::Cast(_kind, op, ty) => {
                self.body_ty_closure_operand(owner, op)?;
                if ty.index() >= self.types.len() {
                    return Err(InvariantViolation::BodyTyIdOutOfRange {
                        owner,
                        site: "Rvalue::Cast.dest",
                        ty: *ty,
                        types_len: self.types.len(),
                    });
                }
                // CastKind has no nested TyId surface; the variants
                // are pinned by spec section 21.2.5.
                let _ = CastKind::IntToInt;
                Ok(())
            }
            Rvalue::BinaryOp(_op, left, right) => {
                self.body_ty_closure_operand(owner, left)?;
                self.body_ty_closure_operand(owner, right)
            }
            Rvalue::Aggregate(kind, operands) => {
                let ty = match kind {
                    AggregateKind::Array(ty)
                    | AggregateKind::Tuple(ty)
                    | AggregateKind::Struct(ty)
                    | AggregateKind::Enum(ty, _)
                    | AggregateKind::Union(ty, _)
                    | AggregateKind::Closure(ty)
                    | AggregateKind::RawPtr(ty, _) => *ty,
                };
                if ty.index() >= self.types.len() {
                    return Err(InvariantViolation::BodyTyIdOutOfRange {
                        owner,
                        site: "AggregateKind carrier",
                        ty,
                        types_len: self.types.len(),
                    });
                }
                for op in operands.iter() {
                    self.body_ty_closure_operand(owner, op)?;
                }
                Ok(())
            }
        }
    }

    fn body_ty_closure_terminator(
        &self,
        owner: InstanceId,
        kind: &TerminatorKind,
    ) -> Result<(), InvariantViolation> {
        match kind {
            TerminatorKind::Goto(_) | TerminatorKind::Return | TerminatorKind::Unreachable => {
                Ok(())
            }
            TerminatorKind::SwitchInt { discr, .. } => self.body_ty_closure_operand(owner, discr),
            TerminatorKind::Call {
                callee,
                args,
                destination,
                target: _,
            } => {
                match callee {
                    CallCallee::Direct(_) => {}
                    CallCallee::Indirect(operand) => {
                        self.body_ty_closure_operand(owner, operand)?
                    }
                }
                for arg in args.iter() {
                    self.body_ty_closure_operand(owner, arg)?;
                }
                self.body_ty_closure_place(owner, destination)
            }
            TerminatorKind::Assert { cond, msg, .. } => {
                self.body_ty_closure_operand(owner, cond)?;
                match msg {
                    AssertKind::BoundsCheck { len, index } => {
                        self.body_ty_closure_operand(owner, len)?;
                        self.body_ty_closure_operand(owner, index)
                    }
                    AssertKind::Overflow(_, left, right) => {
                        self.body_ty_closure_operand(owner, left)?;
                        self.body_ty_closure_operand(owner, right)
                    }
                    AssertKind::OverflowNeg(op)
                    | AssertKind::DivisionByZero(op)
                    | AssertKind::RemainderByZero(op)
                    | AssertKind::InvalidEnumConstruction(op) => {
                        self.body_ty_closure_operand(owner, op)
                    }
                    AssertKind::MisalignedPointerDereference { required, found } => {
                        self.body_ty_closure_operand(owner, required)?;
                        self.body_ty_closure_operand(owner, found)
                    }
                    AssertKind::NullPointerDereference => Ok(()),
                }
            }
        }
    }

    /// Per-statement invariants B-1..B-5 (spec section 22.5.2).
    /// Place / Operand structural integrity is handled separately by
    /// `validate_body_places`; this pass focuses on type coherence
    /// rules.
    fn validate_body_statement_invariants(
        &self,
        owner: InstanceId,
        body: &Body,
    ) -> Result<(), InvariantViolation> {
        for (block_idx, block) in body.basic_blocks.iter().enumerate() {
            let block_id = BlockId::from_u32(block_idx as u32);
            for (stmt_idx, stmt) in block.statements.iter().enumerate() {
                match &stmt.kind {
                    StatementKind::Assign(place, rvalue) => {
                        let place_recovery = place_ty(self, body, place);
                        let rvalue_recovery = rvalue_ty(self, body, rvalue);
                        match (place_recovery, rvalue_recovery) {
                            (TypeRecovery::Malformed, _) => {
                                return Err(InvariantViolation::StatementAssignMalformed {
                                    owner,
                                    block: block_id,
                                    stmt: stmt_idx,
                                    side: "place",
                                });
                            }
                            (_, TypeRecovery::Malformed) => {
                                return Err(InvariantViolation::StatementAssignMalformed {
                                    owner,
                                    block: block_id,
                                    stmt: stmt_idx,
                                    side: "rvalue",
                                });
                            }
                            (TypeRecovery::Recovered(p), TypeRecovery::Recovered(r)) => {
                                if p != r {
                                    return Err(InvariantViolation::StatementAssignTypeMismatch {
                                        owner,
                                        block: block_id,
                                        stmt: stmt_idx,
                                        place_ty: p,
                                        rvalue_ty: r,
                                    });
                                }
                            }
                            // Either side NotApplicable means we do
                            // not have a derivable Ty for that arm;
                            // skip per spec section 22.5.2 narrowing.
                            _ => {}
                        }
                    }
                    StatementKind::SetDiscriminant(place, variant) => {
                        match place_enum_variant_count(self, body, place) {
                            VariantRecovery::Recovered(max) => {
                                if variant.as_u32() >= max {
                                    return Err(
                                        InvariantViolation::SetDiscriminantVariantIdxOutOfRange {
                                            owner,
                                            block: block_id,
                                            stmt: stmt_idx,
                                            variant: *variant,
                                            max,
                                        },
                                    );
                                }
                            }
                            VariantRecovery::NotEnum(place_ty) => {
                                return Err(InvariantViolation::SetDiscriminantNonEnumTarget {
                                    owner,
                                    block: block_id,
                                    stmt: stmt_idx,
                                    place_ty,
                                });
                            }
                            VariantRecovery::Malformed => {
                                return Err(InvariantViolation::StatementAssignMalformed {
                                    owner,
                                    block: block_id,
                                    stmt: stmt_idx,
                                    side: "place",
                                });
                            }
                            VariantRecovery::NotApplicable => {}
                        }
                    }
                    StatementKind::Intrinsic(intrinsic) => match intrinsic {
                        NonDivergingIntrinsic::Assume(op) => match operand_ty(self, body, op) {
                            TypeRecovery::Recovered(ty) => {
                                if !matches!(self.types.resolve(ty), Ty::Bool) {
                                    return Err(InvariantViolation::AssumeOperandNotBool {
                                        owner,
                                        block: block_id,
                                        stmt: stmt_idx,
                                        operand_ty: ty,
                                    });
                                }
                            }
                            TypeRecovery::Malformed => {
                                return Err(InvariantViolation::StatementAssignMalformed {
                                    owner,
                                    block: block_id,
                                    stmt: stmt_idx,
                                    side: "rvalue",
                                });
                            }
                            TypeRecovery::NotApplicable => {}
                        },
                        NonDivergingIntrinsic::CopyNonOverlapping(payload) => {
                            self.validate_copy_non_overlapping(
                                owner, block_id, stmt_idx, body, payload,
                            )?;
                        }
                    },
                }
            }
        }
        Ok(())
    }

    fn validate_copy_non_overlapping(
        &self,
        owner: InstanceId,
        block: BlockId,
        stmt: usize,
        body: &Body,
        payload: &CopyNonOverlapping,
    ) -> Result<(), InvariantViolation> {
        let CopyNonOverlapping { src, dst, count } = payload;
        let src_ty = self.expect_operand_ty(owner, block, stmt, body, src, "src")?;
        let dst_ty = self.expect_operand_ty(owner, block, stmt, body, dst, "dst")?;
        let count_ty = self.expect_operand_ty(owner, block, stmt, body, count, "count")?;

        let src_pointee = match (src_ty, self.types.resolve(src_ty)) {
            (_, Ty::Ref(p, _) | Ty::Ptr(p, _)) => *p,
            _ => {
                return Err(InvariantViolation::CopyNonOverlappingOperandType {
                    owner,
                    block,
                    stmt,
                    which: "src",
                    actual_ty: src_ty,
                });
            }
        };
        let dst_pointee = match (dst_ty, self.types.resolve(dst_ty)) {
            (_, Ty::Ref(p, _) | Ty::Ptr(p, _)) => *p,
            _ => {
                return Err(InvariantViolation::CopyNonOverlappingOperandType {
                    owner,
                    block,
                    stmt,
                    which: "dst",
                    actual_ty: dst_ty,
                });
            }
        };
        if src_pointee != dst_pointee {
            return Err(InvariantViolation::CopyNonOverlappingOperandType {
                owner,
                block,
                stmt,
                which: "pointee",
                actual_ty: src_pointee,
            });
        }
        if !matches!(self.types.resolve(count_ty), Ty::Uint(UintKind::Usize)) {
            return Err(InvariantViolation::CopyNonOverlappingOperandType {
                owner,
                block,
                stmt,
                which: "count",
                actual_ty: count_ty,
            });
        }
        Ok(())
    }

    fn expect_operand_ty(
        &self,
        owner: InstanceId,
        block: BlockId,
        stmt: usize,
        body: &Body,
        op: &Operand,
        which: &'static str,
    ) -> Result<TyId, InvariantViolation> {
        match operand_ty(self, body, op) {
            TypeRecovery::Recovered(ty) => Ok(ty),
            TypeRecovery::Malformed => Err(InvariantViolation::StatementAssignMalformed {
                owner,
                block,
                stmt,
                side: which,
            }),
            // For CopyNonOverlapping the type rule applies; an
            // unrecoverable operand is treated as malformed so the
            // invariant is not silently skipped.
            TypeRecovery::NotApplicable => Err(InvariantViolation::StatementAssignMalformed {
                owner,
                block,
                stmt,
                side: which,
            }),
        }
    }

    /// Per-terminator invariants B-1..B-9 (spec section 23.6.2).
    /// Place / Operand structural integrity is handled separately by
    /// `validate_body_places`; this pass focuses on type coherence and
    /// callee-shape rules.
    fn validate_body_terminator_invariants(
        &self,
        owner: InstanceId,
        body: &Body,
    ) -> Result<(), InvariantViolation> {
        for (block_idx, block) in body.basic_blocks.iter().enumerate() {
            let block_id = BlockId::from_u32(block_idx as u32);
            self.validate_terminator(owner, block_id, body, &block.terminator.kind)?;
        }
        Ok(())
    }

    fn validate_terminator(
        &self,
        owner: InstanceId,
        block: BlockId,
        body: &Body,
        kind: &TerminatorKind,
    ) -> Result<(), InvariantViolation> {
        match kind {
            TerminatorKind::Goto(_) | TerminatorKind::Return | TerminatorKind::Unreachable => {
                Ok(())
            }
            TerminatorKind::SwitchInt { discr, .. } => {
                self.validate_switch_int_discr(owner, block, body, discr)
            }
            TerminatorKind::Assert { cond, .. } => {
                self.validate_assert_cond(owner, block, body, cond)
            }
            TerminatorKind::Call {
                callee,
                args,
                destination,
                target,
            } => self.validate_call(owner, block, body, callee, args, destination, *target),
        }
    }

    fn validate_switch_int_discr(
        &self,
        owner: InstanceId,
        block: BlockId,
        body: &Body,
        discr: &Operand,
    ) -> Result<(), InvariantViolation> {
        match operand_ty(self, body, discr) {
            TypeRecovery::Recovered(ty) => {
                if !matches!(
                    self.types.resolve(ty),
                    Ty::Bool | Ty::Char | Ty::Int(_) | Ty::Uint(_)
                ) {
                    return Err(InvariantViolation::SwitchIntDiscrTypeMismatch {
                        owner,
                        block,
                        operand_ty: ty,
                    });
                }
                Ok(())
            }
            TypeRecovery::Malformed => Err(InvariantViolation::SwitchIntDiscrTypeMismatch {
                owner,
                block,
                operand_ty: TyId::ZERO,
            }),
            TypeRecovery::NotApplicable => Ok(()),
        }
    }

    fn validate_assert_cond(
        &self,
        owner: InstanceId,
        block: BlockId,
        body: &Body,
        cond: &Operand,
    ) -> Result<(), InvariantViolation> {
        match operand_ty(self, body, cond) {
            TypeRecovery::Recovered(ty) => {
                if !matches!(self.types.resolve(ty), Ty::Bool) {
                    return Err(InvariantViolation::AssertCondNotBool {
                        owner,
                        block,
                        cond_ty: ty,
                    });
                }
                Ok(())
            }
            TypeRecovery::Malformed => Err(InvariantViolation::AssertCondNotBool {
                owner,
                block,
                cond_ty: TyId::ZERO,
            }),
            TypeRecovery::NotApplicable => Ok(()),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn validate_call(
        &self,
        owner: InstanceId,
        block: BlockId,
        body: &Body,
        callee: &CallCallee,
        args: &[Operand],
        destination: &crate::place::Place,
        target: Option<BlockId>,
    ) -> Result<(), InvariantViolation> {
        let resolved_sig: Option<(Vec<TyId>, TyId)> = match callee {
            CallCallee::Direct(id) => {
                if id.index() >= self.instances.len() {
                    // Bounds owned by `validate_body_direct_callees`;
                    // emit `DirectCalleeOutOfRange` from there. Skip
                    // here.
                    return Ok(());
                }
                let inst = &self.instances[*id];
                match &inst.kind {
                    InstanceKind::Function(data) => {
                        // B-8: flavor / virtual_idx biconditional at
                        // the per-call site.
                        let is_virtual = data.flavor == FunctionInstanceFlavor::Virtual;
                        match (is_virtual, data.virtual_idx.is_some()) {
                            (true, false) => {
                                return Err(InvariantViolation::CallVirtualIdxInconsistent {
                                    owner,
                                    block,
                                    callee: *id,
                                    reason: "Virtual flavor requires virtual_idx",
                                });
                            }
                            (false, true) => {
                                return Err(InvariantViolation::CallVirtualIdxInconsistent {
                                    owner,
                                    block,
                                    callee: *id,
                                    reason: "non-Virtual flavor must not carry virtual_idx",
                                });
                            }
                            _ => {}
                        }
                        if data.fn_sig.index() >= self.sigs.len() {
                            return Ok(());
                        }
                        let sig = self.sigs.resolve(data.fn_sig);
                        Some((sig.inputs.clone(), sig.output))
                    }
                    _ => {
                        return Err(InvariantViolation::CallDirectCalleeNotFunction {
                            owner,
                            block,
                            callee: *id,
                        });
                    }
                }
            }
            CallCallee::Indirect(op) => match operand_ty(self, body, op) {
                TypeRecovery::Recovered(ty) => match self.types.resolve(ty) {
                    Ty::FnPtr(sig_id) => {
                        if sig_id.index() >= self.sigs.len() {
                            return Ok(());
                        }
                        let sig = self.sigs.resolve(*sig_id);
                        Some((sig.inputs.clone(), sig.output))
                    }
                    _ => {
                        return Err(InvariantViolation::CallIndirectCalleeNotFnPtr {
                            owner,
                            block,
                            operand_ty: ty,
                        });
                    }
                },
                // Per audit-finding 5: a Malformed indirect-callee
                // operand must surface as a typed error rather than
                // silently skipping B-5. The constructed `TyId::ZERO`
                // is a diagnostic placeholder.
                TypeRecovery::Malformed => {
                    return Err(InvariantViolation::CallIndirectCalleeNotFnPtr {
                        owner,
                        block,
                        operand_ty: TyId::ZERO,
                    });
                }
                // NotApplicable arises when the callee operand is a
                // constant whose `Ty` is not recoverable from inline
                // data (spec section 20.5 places that responsibility
                // upstream). Skipping here is the spec-inapplicable
                // case, not a silent malformed skip.
                TypeRecovery::NotApplicable => None,
            },
        };

        let Some((inputs, output)) = resolved_sig else {
            return Ok(());
        };

        // I-DiagLog-CallArity exception per spec.md section 23.6.2
        // B-4 / B-6 (`spec.md:9649-9778`): Log-intrinsic Direct
        // callees carry the post-DiagLogPeel flat-arg shape
        // `[fmt_literal, expr_1, ..., expr_N]` (spec.md section 26.6),
        // which does NOT match the declared callee FnSig's
        // `(fmt, &[LogArg<'_>])` inputs nor the post-regseq-driver-48
        // rewritten `(fmt,) -> ()` inputs. Route Log-intrinsic Direct
        // callees through the dedicated validator that honours the
        // peeled shape's invariants for B-4 + B-6 only; B-3
        // (destination type) + B-9 (target / divergence coherence)
        // below still apply uniformly because the post-rewrite callee
        // FnSig output is `unit` (matching the post-peel destination
        // place) and `target = Some(_)` (the Call is not divergent).
        // Indirect callees do NOT receive this exception per spec.md
        // section 23.6.2 B-5: `IntrinsicKind` is keyed by Instance
        // identity, and indirect callees do not pin an InstanceId.
        let is_log_intrinsic = match callee {
            CallCallee::Direct(id) => {
                matches!(self.instance_intrinsic(*id), Some(IntrinsicKind::Log(_)))
            }
            CallCallee::Indirect(_) => false,
        };

        if is_log_intrinsic {
            self.validate_log_intrinsic_call(owner, block, body, args)?;
        } else {
            // B-4: arity match.
            if args.len() != inputs.len() {
                return Err(InvariantViolation::CallArgsArityMismatch {
                    owner,
                    block,
                    expected: inputs.len(),
                    actual: args.len(),
                });
            }

            // B-6: per-arg type match. NotApplicable arms are skipped
            // per the spec narrowing (operand-context typing for
            // constants is upstream's responsibility).
            for (i, arg) in args.iter().enumerate() {
                match operand_ty(self, body, arg) {
                    TypeRecovery::Recovered(ty) => {
                        if ty != inputs[i] {
                            return Err(InvariantViolation::CallArgTypeMismatch {
                                owner,
                                block,
                                idx: i,
                                expected_ty: inputs[i],
                                actual_ty: ty,
                            });
                        }
                    }
                    TypeRecovery::NotApplicable => {}
                    TypeRecovery::Malformed => {
                        return Err(InvariantViolation::CallArgTypeMismatch {
                            owner,
                            block,
                            idx: i,
                            expected_ty: inputs[i],
                            actual_ty: TyId::ZERO,
                        });
                    }
                }
            }
        }

        let output_is_never = matches!(self.types.resolve(output), Ty::Never);

        // B-9: target / divergence coherence.
        match (target, output_is_never) {
            (None, true) | (Some(_), false) => {}
            (None, false) => {
                return Err(InvariantViolation::CallTargetDivergenceMismatch {
                    owner,
                    block,
                    reason: "target: None requires callee output == Ty::Never",
                });
            }
            (Some(_), true) => {
                return Err(InvariantViolation::CallTargetDivergenceMismatch {
                    owner,
                    block,
                    reason: "target: Some(_) requires callee output != Ty::Never",
                });
            }
        }

        // B-3: destination type matches callee output (when
        // target: Some(_)).
        if target.is_some() {
            match place_ty(self, body, destination) {
                TypeRecovery::Recovered(actual) => {
                    if actual != output {
                        return Err(InvariantViolation::CallDestinationTypeMismatch {
                            owner,
                            block,
                            expected_ty: output,
                            actual_ty: actual,
                        });
                    }
                }
                TypeRecovery::Malformed => {
                    return Err(InvariantViolation::CallDestinationTypeMismatch {
                        owner,
                        block,
                        expected_ty: output,
                        actual_ty: TyId::ZERO,
                    });
                }
                TypeRecovery::NotApplicable => {}
            }
        }
        Ok(())
    }

    /// Validate a Log-intrinsic `Call` per the spec.md section 23.6.2
    /// B-4 / B-6 I-DiagLog-CallArity exception (`spec.md:9649-9778`).
    ///
    /// Honours the post-DiagLogPeel flat-arg shape per spec.md
    /// section 26.6:
    /// - `args[0]` is `Ty::Ref(Ty::Str, _)` (the `&'static str`
    ///   format-template operand; literal-ness is validated at
    ///   DiagLogPeel synthesis time per spec.md section 25.7.3
    ///   step 1, not here).
    /// - `args[i]` for `i in 1..args.len()` is a value-typed `Ty` in
    ///   the admissible LogArg-source set per spec.md section 26.4:
    ///   `Ty::Uint(UintKind::U8..U64)`, `Ty::Int(IntKind::I8..I64)`,
    ///   `Ty::Bool`, or `Ty::Ref(Ty::Str, _)`.
    /// - Arity: `args.len() >= 1` (at least the fmt slot). The
    ///   stronger arity-vs-`{}`-placeholder-count check (`args.len()
    ///   == 1 + N`) is performed at DiagLogPeel synthesis time per
    ///   `crates/regseq-driver/src/bridge/diag_log/peel.rs::validate_log_call_fingerprints`
    ///   (D1 per spec.md section 26.8); the validator does NOT
    ///   re-derive the placeholder count from the literal because
    ///   the model does not pin the literal's content (the bytes
    ///   live in a `ConstMemoryStatic` per spec.md section 9.7 and
    ///   are not accessible without bridge-side
    ///   `const_memory_static_cache` state). This validator runs
    ///   structural shape checks only.
    fn validate_log_intrinsic_call(
        &self,
        owner: InstanceId,
        block: BlockId,
        body: &Body,
        args: &[Operand],
    ) -> Result<(), InvariantViolation> {
        // Spec.md section 26.6: the post-DiagLogPeel flat-arg shape
        // requires at least the format-template slot at `args[0]`.
        // Validate-don't-assume discipline: an empty-args
        // `args[0]` access would panic.
        if args.is_empty() {
            return Err(InvariantViolation::DiagLogCallArityBelowMinimum { owner, block });
        }

        // Spec.md section 26.6: `args[0]` must resolve to
        // `Ty::Ref(Ty::Str, _)`.
        match operand_ty(self, body, &args[0]) {
            TypeRecovery::Recovered(ty) => {
                if !self.is_str_ref(ty) {
                    return Err(InvariantViolation::DiagLogCallArg0NotStrRef {
                        owner,
                        block,
                        actual_ty: ty,
                    });
                }
            }
            TypeRecovery::Malformed => {
                return Err(InvariantViolation::DiagLogCallArg0NotStrRef {
                    owner,
                    block,
                    actual_ty: TyId::ZERO,
                });
            }
            // NotApplicable arises when `args[0]` is a constant whose
            // `Ty` is not recoverable from inline data per spec.md
            // section 20.5; skip per the existing TypeRecovery
            // discipline.
            TypeRecovery::NotApplicable => {}
        }

        // Spec.md section 26.4: `args[i]` for `i >= 1` must resolve
        // to a value-typed `Ty` in the admissible LogArg-source set.
        for (idx, arg) in args.iter().enumerate().skip(1) {
            match operand_ty(self, body, arg) {
                TypeRecovery::Recovered(ty) => {
                    if !self.is_admissible_log_arg(ty) {
                        return Err(InvariantViolation::DiagLogCallArgNotAdmissible {
                            owner,
                            block,
                            idx,
                            actual_ty: ty,
                        });
                    }
                }
                TypeRecovery::Malformed => {
                    return Err(InvariantViolation::DiagLogCallArgNotAdmissible {
                        owner,
                        block,
                        idx,
                        actual_ty: TyId::ZERO,
                    });
                }
                TypeRecovery::NotApplicable => {}
            }
        }

        Ok(())
    }

    /// Returns whether `ty` resolves to `Ty::Ref(pointee, _)` whose
    /// `pointee` resolves to `Ty::Str`. Spec.md section 26.6
    /// args[0] shape predicate.
    fn is_str_ref(&self, ty: TyId) -> bool {
        if ty.index() >= self.types.len() {
            return false;
        }
        match self.types.resolve(ty) {
            Ty::Ref(pointee, _) => {
                pointee.index() < self.types.len()
                    && matches!(self.types.resolve(*pointee), Ty::Str)
            }
            _ => false,
        }
    }

    /// Returns whether `ty` is in the spec.md section 26.4 admissible
    /// LogArg-source set: `Ty::Uint(U8..U64)`, `Ty::Int(I8..I64)`,
    /// `Ty::Bool`, or `Ty::Ref(Ty::Str, _)`.
    fn is_admissible_log_arg(&self, ty: TyId) -> bool {
        if ty.index() >= self.types.len() {
            return false;
        }
        match self.types.resolve(ty) {
            Ty::Bool => true,
            Ty::Uint(UintKind::U8 | UintKind::U16 | UintKind::U32 | UintKind::U64) => true,
            Ty::Int(IntKind::I8 | IntKind::I16 | IntKind::I32 | IntKind::I64) => true,
            Ty::Ref(_, _) => self.is_str_ref(ty),
            _ => false,
        }
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

    /// Bridge-completeness check per regseq-driver-55 Change G
    /// (spec.md section 3.4 Phase 5 + section 8.1 `ItemKind`-mirrors-
    /// `InstanceKind`; extends the section 3.1 invariant 14
    /// `InstanceKind`-consistency family with the Item-side
    /// direction).
    ///
    /// Enforces: for every `Item` whose `item_synthetic_kinds[i] ==
    /// SyntheticItemKind::None` AND whose `linkage !=
    /// Linkage::Foreign`, there is at least one `Instance` whose
    /// `item` field references it. `ItemKind` is the closed
    /// 6-variant set per spec.md section 8.0 (`Function` / `Static`
    /// / `Const` / `Struct` / `Enum` / `Union`), so every
    /// non-synthetic non-foreign Item is in scope without an
    /// explicit per-kind filter.
    ///
    /// This is a SEPARATE public entry point, DELIBERATELY NOT
    /// folded into `RegseqModel::validate(&mut self, &BridgeInput)`.
    /// `validate` is the model-correctness entry point for
    /// arbitrary models (hand-constructed partial test fixtures,
    /// deserialised models, mid-construction snapshots); the
    /// partial-model validator corpus at
    /// `crates/regseq-model/tests/{id_referential_integrity_validator,
    /// layout_validator}.rs` intentionally builds partial models
    /// with zero Instances to negative-test other invariants
    /// (`F4OutOfOrderInstance`, `InstanceLayoutMismatch`,
    /// `InstanceLayoutItemArgsMismatch`) and positive layout tests
    /// (`layout_pass_struct`) that rely on `validate` returning
    /// `Ok(())` for a one-Struct zero-Instance model. Folding the
    /// coverage check into `validate` would either mask those
    /// intentional negative tests or force a per-test corpus
    /// migration this task did not scope. The bridge-completeness
    /// check is therefore invoked from `bridge::run_bridge` (after
    /// the final `sort_instances_by_f4_2`), not from `validate`.
    ///
    /// Foreign-source-Item carve-out (`linkage == Linkage::Foreign`):
    /// foreign `fn` / `static` declarations legitimately carry
    /// `body: None` / `init: None` (spec.md section 3.1 invariant 10
    /// Function-body biconditional; spec.md section 9.4 Static-init
    /// biconditional) and whether they require an Instance row is
    /// SPEC-AMBIGUOUS and out of scope per the regseq-driver-55
    /// Non-Goals 1-2.
    ///
    /// Synthetic-Item exemption (`item_synthetic_kinds[i] != None`):
    /// the out-of-scope DiagLogPeel residue-driven DCE
    /// (`regseq-driver-48/49`, Non-Goals 5 + 6) legitimately strips
    /// the `Instance` of a synth Item while leaving the Item entry;
    /// synth-Item Instance lifecycle is owned by the synth passes +
    /// the residue DCE, not policed here.
    pub fn validate_bridge_completeness(&self) -> Result<(), InvariantViolation> {
        let mut covered = vec![false; self.items.len()];
        for instance in self.instances.iter() {
            let idx = instance.item.index();
            if idx < covered.len() {
                covered[idx] = true;
            }
        }
        for (idx, item) in self.items.iter().enumerate() {
            if covered[idx] {
                continue;
            }
            let item_id = ItemId::from_u32(idx as u32);
            // Synthetic Items' Instance lifecycle is owned by the
            // synth passes + the residue DCE (out of scope per the
            // regseq-driver-55 Non-Goals 5 + 6); only source-derived
            // Items are checked here.
            if !matches!(self.item_synthetic_kinds[item_id], SyntheticItemKind::None) {
                continue;
            }
            // Source foreign declarations are the carved-out shape.
            if item.linkage == Linkage::Foreign {
                continue;
            }
            return Err(InvariantViolation::ItemMissingInstance {
                item: item_id,
                item_kind: item.kind.clone(),
            });
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

fn permute_index_vec<I: crate::idx::Idx, T>(table: &mut IndexVec<I, T>, order: &[usize]) {
    debug_assert_eq!(table.len(), order.len());
    let taken_vec = std::mem::take(table).into_inner();
    let mut taken: Vec<Option<T>> = taken_vec.into_iter().map(Some).collect();
    let mut sorted: Vec<T> = Vec::with_capacity(order.len());
    for &old_idx in order {
        sorted.push(
            taken[old_idx]
                .take()
                .expect("permutation visits each old index exactly once"),
        );
    }
    table.replace_inner(sorted);
}

/// Compact an `IndexVec<I, T>` in place, retaining only entries
/// where `keep[idx] == true`. Used by the model-side removal
/// helpers to compact `IndexVec<ItemId, _>` /
/// `IndexVec<InstanceId, _>` / `IndexVec<FnSigId, _>` side tables
/// in lock-step with the corresponding primary table.
fn permute_index_vec_with_keep<I: crate::idx::Idx, T>(table: &mut IndexVec<I, T>, keep: &[bool]) {
    debug_assert_eq!(table.len(), keep.len());
    let taken = std::mem::take(table).into_inner();
    let mut retained: Vec<T> = Vec::with_capacity(taken.len());
    for (value, keep) in taken.into_iter().zip(keep.iter()) {
        if *keep {
            retained.push(value);
        }
    }
    table.replace_inner(retained);
}

/// Apply a `Vec<Option<InstanceId>>` remap to every
/// `CallCallee::Direct(InstanceId)` reference inside a `Body`.
/// Mirrors `remap_body_call_callees` in `sort_instances_by_f4_2`
/// but accepts the `Option<InstanceId>` shape produced by the
/// removal-style remap (`None` is a panic in debug builds; a
/// surviving Direct callee MUST not point to a removed Instance
/// per the residue-driven contract).
fn remap_body_call_callees_optional(body: &mut crate::body::Body, remap: &[Option<InstanceId>]) {
    for block in body.basic_blocks.iter_mut() {
        if let TerminatorKind::Call {
            callee: CallCallee::Direct(id),
            ..
        } = &mut block.terminator.kind
        {
            let old = id.as_u32() as usize;
            if old < remap.len() {
                if let Some(new) = remap[old] {
                    *id = new;
                } else {
                    debug_assert!(
                        false,
                        "remap_body_call_callees_optional: surviving CallCallee::Direct({}) points to a removed Instance",
                        old,
                    );
                }
            }
        }
    }
}

/// Visit every `TyId` reference inside a stored `Ty` value via
/// the shared `for_each_ty_id_mut` walker. Mirrors the surface
/// enumerated in `regseq-driver-48` Change D.
fn visit_ty_ids_in_ty<F: FnMut(&mut TyId)>(ty: &mut Ty, f: &mut F) {
    match ty {
        Ty::Bool | Ty::Char | Ty::Int(_) | Ty::Uint(_) | Ty::Float(_) | Ty::Str | Ty::Never => {}
        Ty::Array(elem, _) | Ty::Slice(elem) | Ty::Ref(elem, _) | Ty::Ptr(elem, _) => f(elem),
        Ty::FnPtr(_) => {}
        Ty::Dynamic { vtable_ty, .. } => f(vtable_ty),
        Ty::FnDef(_, args) | Ty::Closure(_, args) => visit_ty_ids_in_generic_args(args, f),
        Ty::Tuple { fields, layout } => {
            for field in fields.iter_mut() {
                f(field);
            }
            visit_ty_ids_in_layout(layout, f);
        }
        Ty::Struct { args, layout, .. }
        | Ty::Enum { args, layout, .. }
        | Ty::Union { args, layout, .. } => {
            visit_ty_ids_in_generic_args(args, f);
            visit_ty_ids_in_layout(layout, f);
        }
    }
}

/// Visit every `TyId` inside a `GenericArgs` vector.
fn visit_ty_ids_in_generic_args<F: FnMut(&mut TyId)>(args: &mut [GenericArg], f: &mut F) {
    for arg in args.iter_mut() {
        if let GenericArg::Type(ty) = arg {
            f(ty);
        }
    }
}

/// Visit every inline `Layout` `TyId` field reachable from a
/// nominal or tuple type's resolved layout: `FieldLayout.ty` per
/// `crates/regseq-model/src/layout.rs:58-62` and
/// `TagScalar.scalar_ty` per
/// `crates/regseq-model/src/layout.rs:146-154`.
fn visit_ty_ids_in_layout<F: FnMut(&mut TyId)>(layout: &mut Layout, f: &mut F) {
    match &mut layout.kind {
        LayoutKind::Struct { fields } | LayoutKind::Union { fields } => {
            for field in fields.iter_mut() {
                f(&mut field.ty);
            }
        }
        LayoutKind::Enum { shape } => match shape {
            EnumShape::Empty => {}
            EnumShape::Single { variant, scalar } => {
                for field in variant.fields.iter_mut() {
                    f(&mut field.ty);
                }
                if let Some(scalar) = scalar {
                    f(&mut scalar.scalar_ty);
                }
            }
            EnumShape::Multiple { tag, variants, .. } => {
                f(&mut tag.scalar_ty);
                for variant in variants.iter_mut() {
                    for field in variant.fields.iter_mut() {
                        f(&mut field.ty);
                    }
                }
            }
        },
    }
}

/// Visit every `TyId` inside a function body: `LocalDecl.ty` for
/// every local plus every statement's and terminator's
/// `TyId`-bearing payload (Rvalue::Cast, Rvalue::Aggregate,
/// `PlaceElem::Field`).
fn visit_ty_ids_in_body<F: FnMut(&mut TyId)>(body: &mut crate::body::Body, f: &mut F) {
    for decl in body.local_decls.iter_mut() {
        f(&mut decl.ty);
    }
    for block in body.basic_blocks.iter_mut() {
        for stmt in block.statements.iter_mut() {
            visit_ty_ids_in_statement_kind(&mut stmt.kind, f);
        }
        visit_ty_ids_in_terminator_kind(&mut block.terminator.kind, f);
    }
}

fn visit_ty_ids_in_statement_kind<F: FnMut(&mut TyId)>(kind: &mut StatementKind, f: &mut F) {
    match kind {
        StatementKind::Assign(place, rvalue) => {
            visit_ty_ids_in_place(place, f);
            visit_ty_ids_in_rvalue(rvalue, f);
        }
        StatementKind::SetDiscriminant(place, _) => visit_ty_ids_in_place(place, f),
        StatementKind::Intrinsic(intrinsic) => match intrinsic {
            NonDivergingIntrinsic::Assume(operand) => visit_ty_ids_in_operand(operand, f),
            NonDivergingIntrinsic::CopyNonOverlapping(payload) => {
                visit_ty_ids_in_operand(&mut payload.src, f);
                visit_ty_ids_in_operand(&mut payload.dst, f);
                visit_ty_ids_in_operand(&mut payload.count, f);
            }
        },
    }
}

fn visit_ty_ids_in_terminator_kind<F: FnMut(&mut TyId)>(kind: &mut TerminatorKind, f: &mut F) {
    match kind {
        TerminatorKind::Goto(_) | TerminatorKind::Return | TerminatorKind::Unreachable => {}
        TerminatorKind::SwitchInt { discr, .. } => visit_ty_ids_in_operand(discr, f),
        TerminatorKind::Call {
            callee,
            args,
            destination,
            ..
        } => {
            if let CallCallee::Indirect(operand) = callee {
                visit_ty_ids_in_operand(operand, f);
            }
            for arg in args.iter_mut() {
                visit_ty_ids_in_operand(arg, f);
            }
            visit_ty_ids_in_place(destination, f);
        }
        TerminatorKind::Assert { cond, msg, .. } => {
            visit_ty_ids_in_operand(cond, f);
            visit_ty_ids_in_assert_kind(msg, f);
        }
    }
}

fn visit_ty_ids_in_assert_kind<F: FnMut(&mut TyId)>(kind: &mut AssertKind, f: &mut F) {
    match kind {
        AssertKind::BoundsCheck { len, index } => {
            visit_ty_ids_in_operand(len, f);
            visit_ty_ids_in_operand(index, f);
        }
        AssertKind::Overflow(_, left, right) => {
            visit_ty_ids_in_operand(left, f);
            visit_ty_ids_in_operand(right, f);
        }
        AssertKind::OverflowNeg(operand)
        | AssertKind::DivisionByZero(operand)
        | AssertKind::RemainderByZero(operand)
        | AssertKind::InvalidEnumConstruction(operand) => visit_ty_ids_in_operand(operand, f),
        AssertKind::MisalignedPointerDereference { required, found } => {
            visit_ty_ids_in_operand(required, f);
            visit_ty_ids_in_operand(found, f);
        }
        AssertKind::NullPointerDereference => {}
    }
}

fn visit_ty_ids_in_rvalue<F: FnMut(&mut TyId)>(rvalue: &mut Rvalue, f: &mut F) {
    match rvalue {
        Rvalue::Use(operand) | Rvalue::Repeat(operand, _) | Rvalue::UnaryOp(_, operand) => {
            visit_ty_ids_in_operand(operand, f);
        }
        Rvalue::Ref(_, place) | Rvalue::RawPtr(_, place) => visit_ty_ids_in_place(place, f),
        Rvalue::Cast(_, operand, ty) => {
            visit_ty_ids_in_operand(operand, f);
            f(ty);
        }
        Rvalue::BinaryOp(_, left, right) => {
            visit_ty_ids_in_operand(left, f);
            visit_ty_ids_in_operand(right, f);
        }
        Rvalue::Discriminant(place) => visit_ty_ids_in_place(place, f),
        Rvalue::Aggregate(kind, operands) => {
            visit_ty_ids_in_aggregate_kind(kind, f);
            for operand in operands.iter_mut() {
                visit_ty_ids_in_operand(operand, f);
            }
        }
    }
}

fn visit_ty_ids_in_aggregate_kind<F: FnMut(&mut TyId)>(kind: &mut AggregateKind, f: &mut F) {
    match kind {
        AggregateKind::Array(ty)
        | AggregateKind::Tuple(ty)
        | AggregateKind::Struct(ty)
        | AggregateKind::Enum(ty, _)
        | AggregateKind::Union(ty, _)
        | AggregateKind::Closure(ty)
        | AggregateKind::RawPtr(ty, _) => f(ty),
    }
}

fn visit_ty_ids_in_operand<F: FnMut(&mut TyId)>(operand: &mut Operand, f: &mut F) {
    match operand {
        Operand::Copy(place) | Operand::Move(place) => visit_ty_ids_in_place(place, f),
        Operand::Constant(_) => {}
    }
}

fn visit_ty_ids_in_place<F: FnMut(&mut TyId)>(place: &mut crate::place::Place, f: &mut F) {
    for elem in place.projection.iter_mut() {
        if let PlaceElem::Field(_, ty) = elem {
            f(ty);
        }
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
