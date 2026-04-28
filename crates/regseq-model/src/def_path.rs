//! Structured per-item path identity.
//!
//! Mirrors rustc's `DefPathData` / `DisambiguatedDefPathData` /
//! `DefPath` / `DefPathHash` quartet through the model's interner
//! and ID conventions. See `docs/discussions/regseq-model/
//! stage-1-vocabulary.md` section "DefPath form" for the full spec.
//!
//! - `DefPathData` is a closed 19-variant enum mirroring
//!   `rustc_hir::definitions::DefPathData` for the pinned toolchain.
//! - `DefPathSegment` pairs a `DefPathData` with rustc's
//!   `disambiguator: u32`.
//! - `DefPath` carries the originating crate, the segment vector,
//!   and the canonical verbose-rendered form (an interned string).
//! - `DefPathHash` mirrors rustc's
//!   `(StableCrateId, Hash64)` cross-build identity.
//! - `def_path_data_sort_rank` is the pinned, append-only stable
//!   rank used by the F4 sort comparator.

use serde::{Deserialize, Serialize};

use crate::crate_info::StableCrateId;
use crate::ids::{CrateId, StringId};

/// Closed enum mirroring `rustc_hir::definitions::DefPathData`.
///
/// 19 variants pinned to the toolchain
/// `nightly-2026-02-01-aarch64-apple-darwin`. A toolchain bump that
/// adds a variant is a methodology event: the new variant is
/// appended explicitly, the corresponding entry in
/// `def_path_data_sort_rank` is added at the next free rank
/// (append-only), and the bridge surface gets reviewed.
///
/// Variants without a name (`Impl`, `Closure`, `AnonConst`, etc.)
/// take their identity from the parent `DefPath` and the segment
/// `disambiguator`. Name-bearing variants carry a `StringId` into
/// the model's interner.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DefPathData {
    /// Root marker for a crate.
    CrateRoot,

    /// Inherent or trait `impl` block (anonymous).
    Impl,
    /// `extern "C" {}` block.
    ForeignMod,
    /// `use` item.
    Use,
    /// `global_asm!` invocation at item scope.
    GlobalAsm,

    /// Type-namespace entry (struct, enum, union, module, ...).
    TypeNs(StringId),
    /// Value-namespace entry (function, const, static, ...).
    ValueNs(StringId),
    /// Macro-namespace entry.
    MacroNs(StringId),
    /// Lifetime-namespace entry (named lifetime parameters).
    LifetimeNs(StringId),

    /// Closure expression (anonymous).
    Closure,

    /// Implicit ctor for a tuple-struct or unit-variant.
    Ctor,
    /// Anonymous const expression in source (e.g., array length).
    AnonConst,
    /// Anonymous const created during AST -> HIR lowering.
    LateAnonConst,
    /// Elided-lifetime placeholder produced by desugaring.
    DesugaredAnonymousLifetime,
    /// Existential `impl Trait` node.
    OpaqueTy,
    /// Lifetime captured by a return-position `impl Trait` in trait.
    OpaqueLifetime(StringId),
    /// Anonymous associated type for RPITIT.
    AnonAssocTy(StringId),
    /// Coroutine by-move body synthesized for an async/coroutine.
    SyntheticCoroutineBody,
    /// Additional static data referred to by another static.
    NestedStatic,
}

/// One segment of a `DefPath`, mirroring rustc's
/// `DisambiguatedDefPathData`.
///
/// `disambiguator` matches rustc's same-named field. It is `0` for
/// the common case and disambiguates sibling anonymous segments
/// (`Impl#0`, `Impl#1`, `AnonConst#0`, ...).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DefPathSegment {
    pub data: DefPathData,
    pub disambiguator: u32,
}

/// Structured per-item path within a single `RegseqModel`.
///
/// `crate_id` records which crate the path lives in (Q6 places items
/// from multiple crates into one model). `segments` is the bridge-
/// produced structured form, used by the F4 sort comparator and any
/// path-shape pattern matching. `rendered` is the eagerly-computed
/// canonical *verbose* `crate::module::Item` form (every
/// `disambiguator != 0` segment carries a `#N` suffix; every
/// anonymous-namespace segment renders as `{namespace#N}`).
///
/// Construction invariant: `rendered` is the canonical verbose
/// rendering of `(crates[crate_id].name, segments)`. Two RegseqModels
/// for the same source must produce byte-identical `rendered`
/// strings for corresponding items.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DefPath {
    pub crate_id: CrateId,
    pub segments: Vec<DefPathSegment>,
    pub rendered: StringId,
}

/// Cross-build path identity, mirroring rustc's
/// `DefPathHash = (StableCrateId, Hash64)`.
///
/// Stable across builds; computed by the bridge from
/// `tcx.def_path_hash(def_id)` (the `StableCrateId` half plus the
/// 64-bit local hash). Stored in the side table
/// `RegseqModel.item_def_paths` per F4.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DefPathHash {
    pub stable_crate_id: StableCrateId,
    pub local_hash: u64,
}

/// Pinned stable rank for the F4 sort comparator.
///
/// The comparator's `data_kind: u8` is *not* `DefPathData as u8`
/// (which can shift when variants are reordered) but a stable rank
/// fixed by this function. Adding a variant during a toolchain bump
/// means appending a new rank at the end (e.g., 19); existing ranks
/// are immutable.
///
/// See the spec section "Determinism (F4 in detail) -> Stable rank
/// table" for the authoritative ordering.
pub const fn def_path_data_sort_rank(data: &DefPathData) -> u8 {
    match data {
        DefPathData::CrateRoot => 0,
        DefPathData::Impl => 1,
        DefPathData::ForeignMod => 2,
        DefPathData::Use => 3,
        DefPathData::GlobalAsm => 4,
        DefPathData::TypeNs(_) => 5,
        DefPathData::ValueNs(_) => 6,
        DefPathData::MacroNs(_) => 7,
        DefPathData::LifetimeNs(_) => 8,
        DefPathData::Closure => 9,
        DefPathData::Ctor => 10,
        DefPathData::AnonConst => 11,
        DefPathData::LateAnonConst => 12,
        DefPathData::DesugaredAnonymousLifetime => 13,
        DefPathData::OpaqueTy => 14,
        DefPathData::OpaqueLifetime(_) => 15,
        DefPathData::AnonAssocTy(_) => 16,
        DefPathData::SyntheticCoroutineBody => 17,
        DefPathData::NestedStatic => 18,
    }
}
