//! Structured per-item path identity.
//!
//! Mirrors rustc's `DefPathData` / `DisambiguatedDefPathData` /
//! `DefPath` / `DefPathHash` quartet through the model's interner
//! and ID conventions.
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

use crate::crate_info::{CrateInfo, StableCrateId};
use crate::ids::{CrateId, StringId};
use crate::index_vec::IndexVec;
use crate::string_interner::StringInterner;

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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
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
/// Renders a `DefPath` to its canonical verbose form.
///
/// Single source of truth for the `DefPath.rendered` byte-identical-
/// across-builds invariant. Both the bridge (when populating
/// `DefPath.rendered` during `Item` construction) and the pretty-
/// printer call this helper, so the rendering rule lives in exactly
/// one place.
///
/// The form mirrors rustc's
/// `DisambiguatedDefPathData::as_sym(verbose=true)` applied to the
/// resolved crate name plus the segment vector, against the pinned
/// `nightly-2026-02-01-aarch64-apple-darwin` toolchain. Anonymous
/// segments always render as `{namespace#disambig}` (including
/// disambig 0); plain name-bearing segments render as `name` or
/// `name#N` (the `#N` is omitted when disambig is 0). `AnonAssocTy`
/// is a hybrid: the payload name appears, then `::`, then
/// `{anon_assoc#disambig}`. `DesugaredAnonymousLifetime` is mapped
/// by rustc to `Named(kw::UnderscoreLifetime)`, so it renders as
/// the literal `'_` with the named-segment disambig rule.
///
/// # Construction precondition
///
/// `CrateRoot` may appear only at index 0 (where it is skipped in
/// favor of the resolved crate prefix). A non-leading `CrateRoot`
/// is a bridge bug; rustc never emits one in `DefPath.data`. Per
/// methodology ("no defensive fallbacks") the helper panics rather
/// than emit a placeholder.
///
/// # Panics
///
/// Panics with diagnostic messages on:
///
/// - `path.crate_id` out of range for `crates`.
/// - The crate's `name` `StringId` out of range for `strings`.
/// - `CrateRoot` appearing at any segment index other than 0.
/// - A name-bearing variant carrying an out-of-range `StringId`
///   (propagates from `StringInterner::resolve`'s underlying `Vec`
///   panic).
pub fn render_def_path(
    path: &DefPath,
    crates: &IndexVec<CrateId, CrateInfo>,
    strings: &StringInterner,
) -> String {
    if path.crate_id.index() >= crates.len() {
        panic!(
            "render_def_path: crate_id {} out of range (crates.len() = {})",
            path.crate_id.as_u32(),
            crates.len()
        );
    }
    let crate_name_id = crates[path.crate_id].name;
    if crate_name_id.index() >= strings.len() {
        panic!(
            "render_def_path: crate name StringId {} out of range (strings.len() = {})",
            crate_name_id.as_u32(),
            strings.len()
        );
    }

    let mut out = String::with_capacity(32);
    out.push_str(strings.resolve(crate_name_id));

    for (idx, seg) in path.segments.iter().enumerate() {
        match seg.data {
            DefPathData::CrateRoot => {
                if idx != 0 {
                    panic!("render_def_path: CrateRoot at non-leading position {}", idx);
                }
                // Index 0 CrateRoot: skip; the crate prefix already covers it.
            }

            // Plain name-bearing variants: render `name`; append
            // `#N` only when disambiguator != 0.
            DefPathData::TypeNs(s)
            | DefPathData::ValueNs(s)
            | DefPathData::MacroNs(s)
            | DefPathData::LifetimeNs(s)
            | DefPathData::OpaqueLifetime(s) => {
                out.push_str("::");
                out.push_str(strings.resolve(s));
                if seg.disambiguator != 0 {
                    out.push('#');
                    out.push_str(&seg.disambiguator.to_string());
                }
            }

            // DesugaredAnonymousLifetime is rustc's
            // `Named(kw::UnderscoreLifetime)`; the named-segment
            // verbose rule applies to the literal `'_` name.
            DefPathData::DesugaredAnonymousLifetime => {
                out.push_str("::'_");
                if seg.disambiguator != 0 {
                    out.push('#');
                    out.push_str(&seg.disambiguator.to_string());
                }
            }

            // AnonAssocTy is the rustc hybrid: emits the payload
            // name, then `::`, then `{anon_assoc#disambig}`.
            DefPathData::AnonAssocTy(s) => {
                out.push_str("::");
                out.push_str(strings.resolve(s));
                out.push_str("::{anon_assoc#");
                out.push_str(&seg.disambiguator.to_string());
                out.push('}');
            }

            // Pure anonymous segments: always `{namespace#N}`,
            // including N == 0.
            DefPathData::Impl
            | DefPathData::ForeignMod
            | DefPathData::Use
            | DefPathData::GlobalAsm
            | DefPathData::Closure
            | DefPathData::Ctor
            | DefPathData::AnonConst
            | DefPathData::LateAnonConst
            | DefPathData::OpaqueTy
            | DefPathData::SyntheticCoroutineBody
            | DefPathData::NestedStatic => {
                out.push_str("::{");
                out.push_str(anon_namespace_name(&seg.data));
                out.push('#');
                out.push_str(&seg.disambiguator.to_string());
                out.push('}');
            }
        }
    }

    out
}

/// Lowercase namespace string for each pure anonymous variant.
///
/// Pinned against `rustc_hir/src/definitions.rs:509-520` for the
/// `nightly-2026-02-01-aarch64-apple-darwin` toolchain. Append-only;
/// adding a variant during a toolchain bump is a methodology event
/// (per the spec's "Closed-vocabulary scope" note) that updates
/// this table together with `def_path_data_sort_rank`.
///
/// # Panics
///
/// Panics if called with a variant that is not a pure anonymous
/// one. `CrateRoot` is filtered at the call site (skipped at index
/// 0; panics earlier at non-leading positions). Name-bearing
/// variants, `DesugaredAnonymousLifetime`, and `AnonAssocTy` are
/// rendered by their own arms in `render_def_path` and never reach
/// this function.
fn anon_namespace_name(data: &DefPathData) -> &'static str {
    match data {
        DefPathData::Impl => "impl",
        DefPathData::ForeignMod => "extern",
        DefPathData::Use => "use",
        DefPathData::GlobalAsm => "global_asm",
        DefPathData::Closure => "closure",
        DefPathData::Ctor => "constructor",
        DefPathData::AnonConst => "constant",
        DefPathData::LateAnonConst => "constant",
        DefPathData::OpaqueTy => "opaque",
        DefPathData::SyntheticCoroutineBody => "synthetic",
        DefPathData::NestedStatic => "nested",

        DefPathData::CrateRoot => {
            panic!("anon_namespace_name called on CrateRoot")
        }
        DefPathData::DesugaredAnonymousLifetime => {
            panic!("anon_namespace_name called on DesugaredAnonymousLifetime")
        }
        DefPathData::AnonAssocTy(_) => {
            panic!("anon_namespace_name called on AnonAssocTy")
        }
        DefPathData::TypeNs(_)
        | DefPathData::ValueNs(_)
        | DefPathData::MacroNs(_)
        | DefPathData::LifetimeNs(_)
        | DefPathData::OpaqueLifetime(_) => {
            panic!("anon_namespace_name called on name-bearing variant")
        }
    }
}

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

#[cfg(test)]
#[path = "def_path_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "render_def_path_tests.rs"]
mod render_tests;
