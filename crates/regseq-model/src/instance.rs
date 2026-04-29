//! Monomorphized items: `Instance`, `InstanceKind`, generic args.
//!
//! Per Q2 the bridge bakes monomorphization. `Instance` is the
//! primary monomorphized unit in RegseqModel: every emittable
//! construct (functions, statics, consts, structs, enums, unions)
//! lives in `RegseqModel.instances` as a fully-resolved
//! monomorphization. Generic forms live in `items` only as the
//! alphabet for source-level traceback.
//!
//! This module also pins the two F4-2 rank tables:
//! `regseq_instance_kind_rank` over the six Regseq emission kinds
//! and `rustc_instance_kind_sort_rank` over rustc's 15-variant
//! callable taxonomy. Both are append-only.

use serde::{Deserialize, Serialize};

use crate::ids::{BodyId, ConstId, FnSigId, ItemId, StringId, TyId};
use crate::source::Span;

/// One generic argument substituted into an `Instance`.
///
/// Lifetimes are erased at the bridge (rustc's `erasing_regions`
/// step) and never appear here.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GenericArg {
    Type(TyId),
    Const(ConstId),
}

/// Vector of `GenericArg` substitutions for one `Instance`.
///
/// Order matches rustc's `GenericArgs` (parent-first, then own
/// parameters in declaration order). Empty for non-generic items.
pub type GenericArgs = Vec<GenericArg>;

/// Kind-specific data for an `Instance`.
///
/// Closed six-variant set covering every emittable C-level entity:
/// `Function`, `Static`, `Const`, `Struct`, `Enum`, `Union`. Stage 13
/// may extend with domain-specific subkinds (e.g., an `Isr` subkind
/// of `Function`); the stage-1 set is the closed baseline.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum InstanceKind {
    /// Function instantiation. `sig` is always present (foreign
    /// declarations have a sig but no body); `body` is `None` for
    /// foreign declarations whose bodies are not in our crate set.
    Function {
        sig: FnSigId,
        body: Option<BodyId>,
    },

    /// Static. The mutability flag (immutable `static` vs.
    /// `static mut`) lives on the source `Item` (stage 12); C
    /// codegen reads `instance.item` to decide on the `const`
    /// qualifier.
    Static { ty: TyId, init: ConstId },

    /// Const item.
    Const { ty: TyId, value: ConstId },

    /// Nominal struct. `layout` indexes the matching `Ty::Struct`
    /// in `RegseqModel.types`.
    Struct { layout: TyId },

    /// Nominal enum. `layout` indexes the matching `Ty::Enum`.
    Enum { layout: TyId },

    /// Nominal C-style union. `layout` indexes the matching
    /// `Ty::Union`.
    Union { layout: TyId },
}

/// One monomorphized item.
///
/// `item` back-references the source-level Item (always present;
/// even bridge-synthesized vtable instances have a synthesized Item
/// entry per the spec). `generic_args` is empty for non-generic
/// items. `symbol_name` is rustc's mangled name; uniqueness across
/// `instances` is guaranteed by the bridge (rustc's mangler factors
/// in the full instance identity, including shim discriminator).
/// `span` uses the synthesized sentinel
/// (`SourceFileId::SYNTHESIZED`, `lo: 0`, `hi: 0`) for
/// bridge-synthesized constructs that have no source location.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Instance {
    pub item: ItemId,
    pub generic_args: GenericArgs,
    pub symbol_name: StringId,
    pub span: Span,
    pub kind: InstanceKind,
}

/// Pinned stable rank for the Regseq `InstanceKind` variant.
///
/// Append-only: existing ranks are immutable. New variants added in
/// stage 13 (e.g., domain subkinds) get the next free rank.
///
/// Used as the leading discriminator in the F4-2 instance sort key
/// and in the bridge's instance interning key.
pub const fn regseq_instance_kind_rank(kind: &InstanceKind) -> u8 {
    match kind {
        InstanceKind::Function { .. } => 0,
        InstanceKind::Static { .. } => 1,
        InstanceKind::Const { .. } => 2,
        InstanceKind::Struct { .. } => 3,
        InstanceKind::Enum { .. } => 4,
        InstanceKind::Union { .. } => 5,
    }
}

/// Tag enum mirroring rustc's `ty::InstanceKind` discriminator.
///
/// `regseq-model` has zero rustc deps (Q1), so the rustc-side enum
/// cannot be referenced directly. The bridge converts a
/// `rustc_ty::InstanceKind<'_>` to one of these tags before calling
/// `rustc_instance_kind_sort_rank`. The taxonomy is pinned for the
/// toolchain `nightly-2026-02-01-aarch64-apple-darwin`.
///
/// Append-only: a toolchain bump that adds a rustc variant adds a
/// new tag at the end and a matching new rank.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RustcInstanceKindTag {
    Item,
    Intrinsic,
    VTableShim,
    ReifyShim,
    FnPtrShim,
    Virtual,
    ClosureOnceShim,
    ConstructCoroutineInClosureShim,
    ThreadLocalShim,
    FutureDropPollShim,
    DropGlue,
    CloneShim,
    FnPtrAddrShim,
    AsyncDropGlueCtorShim,
    AsyncDropGlue,
}

/// Pinned stable rank for rustc's `ty::InstanceKind` variant.
///
/// Used as `fn_shim_disambig` when the Regseq kind is `Function`,
/// per the F4-2 instance sort key. For non-`Function` Regseq kinds
/// the bridge passes `0` unconditionally (those kinds have no rustc
/// shim discrimination to encode).
///
/// Append-only: existing ranks are immutable. A toolchain bump that
/// adds a rustc variant appends a new tag and a new rank.
pub const fn rustc_instance_kind_sort_rank(tag: RustcInstanceKindTag) -> u8 {
    match tag {
        RustcInstanceKindTag::Item => 0,
        RustcInstanceKindTag::Intrinsic => 1,
        RustcInstanceKindTag::VTableShim => 2,
        RustcInstanceKindTag::ReifyShim => 3,
        RustcInstanceKindTag::FnPtrShim => 4,
        RustcInstanceKindTag::Virtual => 5,
        RustcInstanceKindTag::ClosureOnceShim => 6,
        RustcInstanceKindTag::ConstructCoroutineInClosureShim => 7,
        RustcInstanceKindTag::ThreadLocalShim => 8,
        RustcInstanceKindTag::FutureDropPollShim => 9,
        RustcInstanceKindTag::DropGlue => 10,
        RustcInstanceKindTag::CloneShim => 11,
        RustcInstanceKindTag::FnPtrAddrShim => 12,
        RustcInstanceKindTag::AsyncDropGlueCtorShim => 13,
        RustcInstanceKindTag::AsyncDropGlue => 14,
    }
}
