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

use crate::args_key::{ArgsKey, GenericArgs};
use crate::ids::{ConstId, FnSigId, ItemId, StringId, TyId};
use crate::placeholders::Body;
use crate::source::Span;

/// Data carried by a function instance.
///
/// The payload is stored structurally on `InstanceKind::Function`
/// so the post-monomorphization signature and body stay with the
/// exact monomorphized instance that owns them.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FunctionInstanceData {
    /// Post-monomorphization function signature.
    pub fn_sig: FnSigId,
    /// Post-monomorphization body, or `None` for foreign functions.
    pub body: Option<Body>,
    /// Rustc instance-kind flavor translated by the bridge.
    pub flavor: FunctionInstanceFlavor,
    /// Vtable slot index for virtual function instances.
    pub virtual_idx: Option<u32>,
}

impl FunctionInstanceData {
    /// Returns the post-monomorphization function signature ID.
    pub fn fn_sig(&self) -> FnSigId {
        self.fn_sig
    }

    /// Returns the optional post-monomorphization body.
    pub fn body(&self) -> Option<&Body> {
        self.body.as_ref()
    }

    /// Returns the optional mutable post-monomorphization body.
    pub fn body_mut(&mut self) -> Option<&mut Body> {
        self.body.as_mut()
    }

    /// Returns the translated rustc instance-kind flavor.
    pub fn flavor(&self) -> FunctionInstanceFlavor {
        self.flavor
    }

    /// Returns the optional virtual dispatch slot.
    pub fn virtual_idx(&self) -> Option<u32> {
        self.virtual_idx
    }
}

/// Kind-specific data for an `Instance`.
///
/// Closed six-variant set covering every emittable C-level entity:
/// `Function`, `Static`, `Const`, `Struct`, `Enum`, `Union`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum InstanceKind {
    /// Function instantiation.
    Function(FunctionInstanceData),

    /// Static instance.
    Static {
        /// Static type.
        ty: TyId,
        /// Initializer value, or `None` for foreign statics.
        init: Option<ConstId>,
    },

    /// Const item.
    Const {
        /// Const value type.
        ty: TyId,
        /// Const value.
        value: ConstId,
    },

    /// Nominal struct.
    Struct {
        /// Matching `Ty::Struct` layout.
        layout: TyId,
    },

    /// Nominal enum.
    Enum {
        /// Matching `Ty::Enum` layout.
        layout: TyId,
    },

    /// Nominal C-style union.
    Union {
        /// Matching `Ty::Union` layout.
        layout: TyId,
    },
}

impl InstanceKind {
    /// Returns the function payload for function instances.
    pub fn as_function(&self) -> Option<&FunctionInstanceData> {
        match self {
            InstanceKind::Function(data) => Some(data),
            _ => None,
        }
    }

    /// Returns the mutable function payload for function instances.
    pub fn as_function_mut(&mut self) -> Option<&mut FunctionInstanceData> {
        match self {
            InstanceKind::Function(data) => Some(data),
            _ => None,
        }
    }
}

/// One monomorphized item.
///
/// `item` back-references the source-level Item (always present;
/// even bridge-synthesized vtable instances have a synthesized Item
/// entry per the spec).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Instance {
    /// Originating source-level item.
    pub item: ItemId,
    /// Resolved generic arguments, empty for non-generic items.
    pub generic_args: GenericArgs,
    /// C-side identifier emitted for this instance.
    pub emission_name: StringId,
    /// Source span, synthesized for bridge-synthesized items.
    pub span: Span,
    /// Kind-specific instance payload.
    pub kind: InstanceKind,
}

/// Bridge-side instance interning key.
///
/// Internal to instance construction; the bridge uses this shape to
/// distinguish Regseq emission kinds and function shim flavors.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct RegseqInstanceKey {
    /// Stable rank for the Regseq `InstanceKind` variant.
    pub regseq_kind_rank: u8,
    /// Originating source-level item.
    pub item: ItemId,
    /// Stable key for the resolved generic arguments.
    pub generic_args_key: ArgsKey,
    /// Function shim discriminator, or `0` for non-functions.
    pub fn_shim_disambig: u8,
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
        InstanceKind::Function(_) => 0,
        InstanceKind::Static { .. } => 1,
        InstanceKind::Const { .. } => 2,
        InstanceKind::Struct { .. } => 3,
        InstanceKind::Enum { .. } => 4,
        InstanceKind::Union { .. } => 5,
    }
}

/// Flavor enum mirroring rustc's `ty::InstanceKind` discriminator.
///
/// `regseq-model` has zero rustc deps (Q1), so the rustc-side enum
/// cannot be referenced directly. The bridge converts a
/// `rustc_ty::InstanceKind<'_>` to one of these tags before calling
/// `rustc_instance_kind_sort_rank`. The taxonomy is pinned for the
/// toolchain `nightly-2026-02-01-aarch64-apple-darwin`.
///
/// Append-only: a toolchain bump that adds a rustc variant adds a
/// new tag at the end and a matching new rank.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum FunctionInstanceFlavor {
    /// Plain item instance.
    Item,
    /// Intrinsic instance.
    Intrinsic,
    /// Vtable shim instance.
    VTableShim,
    /// Reify shim instance.
    ReifyShim,
    /// Function pointer shim instance.
    FnPtrShim,
    /// Virtual dispatch instance.
    Virtual,
    /// Closure `FnOnce` shim instance.
    ClosureOnceShim,
    /// Coroutine-in-closure constructor shim instance.
    ConstructCoroutineInClosureShim,
    /// Thread-local shim instance.
    ThreadLocalShim,
    /// Future drop poll shim instance.
    FutureDropPollShim,
    /// Drop glue instance.
    DropGlue,
    /// Clone shim instance.
    CloneShim,
    /// Function pointer address shim instance.
    FnPtrAddrShim,
    /// Async drop-glue constructor shim instance.
    AsyncDropGlueCtorShim,
    /// Async drop-glue instance.
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
pub const fn rustc_instance_kind_sort_rank(tag: FunctionInstanceFlavor) -> u8 {
    match tag {
        FunctionInstanceFlavor::Item => 0,
        FunctionInstanceFlavor::Intrinsic => 1,
        FunctionInstanceFlavor::VTableShim => 2,
        FunctionInstanceFlavor::ReifyShim => 3,
        FunctionInstanceFlavor::FnPtrShim => 4,
        FunctionInstanceFlavor::Virtual => 5,
        FunctionInstanceFlavor::ClosureOnceShim => 6,
        FunctionInstanceFlavor::ConstructCoroutineInClosureShim => 7,
        FunctionInstanceFlavor::ThreadLocalShim => 8,
        FunctionInstanceFlavor::FutureDropPollShim => 9,
        FunctionInstanceFlavor::DropGlue => 10,
        FunctionInstanceFlavor::CloneShim => 11,
        FunctionInstanceFlavor::FnPtrAddrShim => 12,
        FunctionInstanceFlavor::AsyncDropGlueCtorShim => 13,
        FunctionInstanceFlavor::AsyncDropGlue => 14,
    }
}

#[cfg(test)]
#[path = "instance_tests.rs"]
mod tests;
