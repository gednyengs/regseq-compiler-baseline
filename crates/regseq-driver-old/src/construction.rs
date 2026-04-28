//! Two-pass rustc-MIR-to-Regseq-IR construction.
//!
//! This module implements Pass 1 (declaration skeleton) of the
//! RegseqModel construction algorithm described in
//! `docs/discussions/models/regseq-model/ir-reference.md` Section 8.
//!
//! Construction is invoked in-process with extraction, inside the same
//! rustc `after_analysis` hook. It consumes a live [`TyCtxt`] plus the
//! in-memory attribute registries produced by Phase 4 extraction and
//! returns a [`RegseqModel`] with every declaration-level identifier
//! assigned. Pass 2 (function body translation) is not in scope here.

use std::collections::BTreeMap;

use rustc_hir::def_id::DefId;
use rustc_middle::ty::TyCtxt;
use rustc_span::Span;

use regseq_model_old::identity::{
    DeclId, ModuleId, SourceLocation, SourceOrigin, Symbol, SymbolInterner,
};
use regseq_model_old::model::{Module, ModuleKind, RegseqModel};

use crate::extract::abstraction_types::AbstractionTypeRegistry;
use crate::extract::framework_types::FrameworkTypeRegistry;
use crate::extract::intrinsic_registry::IntrinsicRegistry;

pub mod attrs;
pub mod driver;
pub mod id_allocator;
pub mod impls;
pub mod pin_state_registry;
pub mod registers;
pub mod resources;
pub mod walk;

pub mod cross_crate;
pub mod cross_crate_types;
pub mod deref_collapse;
pub mod intrinsics;
pub mod log_metadata;
pub(crate) mod mir_mapping;
pub mod module_deps;
pub(crate) mod ordered_region;
pub(crate) mod pass2;
pub(crate) mod pinctrl_metadata;
pub mod register_metadata;
pub(crate) mod switch_and_loops;
pub(crate) mod try_folding;

// Re-export the Pass 2 mapping helpers so integration tests and Pass
// 2 task code can reach them without the sub-module itself being part
// of the public `construction::*` surface.
pub use deref_collapse::{DerefCollapseLookup, DerefCollapseRewrites, TRANSPORT_ABSTRACTIONS};
pub use intrinsics::IntrinsicLookup;
pub use log_metadata::LogArgUnwrap;
pub use mir_mapping::{
    map_const, map_operand, map_place, map_rvalue, map_ty, ConstantIndex, FunctionIndex,
    LocalIdMap, MapOperandResult, TypeDefIndex,
};
pub use module_deps::derive_module_dependencies;
pub use ordered_region::OrderedMarkers;
pub use pass2::build_body;
pub use pinctrl_metadata::PinStateLookup;
pub use register_metadata::{RegisterEntry, RegisterIndex};
pub use try_folding::TryMarkers;

use id_allocator::DeclIdAllocator;

pub use pin_state_registry::PinStateRegistry;

// ---------------------------------------------------------------------------
// Extraction registry bundle
// ---------------------------------------------------------------------------

/// Borrow-only bundle of the Phase 4 attribute registries consumed by
/// Pass 1 construction.
pub struct ExtractionRegistries<'a> {
    /// `#[rseq::intrinsic("canonical_name")]` map.
    pub intrinsic: &'a IntrinsicRegistry,
    /// `#[rseq::framework_type("canonical_name")]` map.
    pub framework_types: &'a FrameworkTypeRegistry,
    /// `#[rseq::abstraction("canonical_name")]` map.
    pub abstraction_types: &'a AbstractionTypeRegistry,
    /// `#[rseq::pin_state(id = "...")]` table.
    pub pin_states: &'a PinStateRegistry,
}

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// A span paired with a human-readable item path, used in error payloads
/// so diagnostics can point back to a specific source location even after
/// the `TyCtxt` is gone.
#[derive(Clone, Debug)]
pub struct ErrorProvenance {
    /// Best-effort item path rendered via `tcx.def_path_str` when
    /// available. Empty string for errors without a specific item.
    pub item_path: String,
    /// The rustc span, preserved so diagnostics can resolve it through
    /// the session source map if the caller still has access to one.
    pub span: Span,
}

impl ErrorProvenance {
    /// Build provenance from a [`DefId`] and its containing rustc
    /// context.
    pub fn for_def_id(tcx: TyCtxt<'_>, def_id: DefId) -> Self {
        Self {
            item_path: tcx.def_path_str(def_id),
            span: tcx.def_span(def_id),
        }
    }

    /// Provenance carrying only a span (no item path).
    pub fn for_span(span: Span) -> Self {
        Self {
            item_path: String::new(),
            span,
        }
    }
}

/// Construction-time errors. Each variant carries [`ErrorProvenance`] so
/// downstream diagnostic code can render a useful message.
#[derive(Clone, Debug)]
pub enum ConstructionError {
    /// No `#[rseq::driver]`-annotated struct was found in the crate.
    NoDriverStruct {
        /// Provenance anchored at the crate root so diagnostics can
        /// point to where the driver declaration was expected.
        crate_root: ErrorProvenance,
    },
    /// More than one `#[rseq::driver]` struct was found.
    MultipleDriverStructs {
        /// Provenance for each discovered driver struct.
        drivers: Vec<ErrorProvenance>,
    },
    /// Two items in the same module share a name.
    DuplicateItemName {
        /// The module that contains the duplicates.
        module: ErrorProvenance,
        /// The clashing name.
        name: String,
    },
    /// A user-data field's shape is not accepted by either rule:
    /// Rule 1 (zero-init whitelist) or Rule 2 (`Option<T>` where
    /// `T: Copy`). Raised when the outer `Option<_>` wrapper is
    /// absent AND the bare field type fails the Rule-1 outer `Copy`
    /// check (so Rule 2 does not apply and Rule 1 cannot proceed).
    InvalidUserDataShape {
        /// Provenance of the offending struct field.
        field: ErrorProvenance,
        /// Rendered type that failed both rules.
        rendered_type: String,
    },
    /// A user-data field's `T` in `Option<T>` is not `Copy`, or a
    /// Rule-1 candidate field type is not `Copy` (the outer `Copy`
    /// check that both rules require).
    UserDataNotCopy {
        /// Provenance of the offending struct field.
        field: ErrorProvenance,
        /// Rendered inner (or outer, for Rule-1) type.
        inner_type: String,
    },
    /// A Rule-1 candidate user-data field is `Copy` but its type
    /// falls outside the Stage-1 zero-init whitelist. Admitted by
    /// Rule 1: `u8`, `u16`, `u32`, `u64`, `bool`, arrays of those,
    /// and `Copy` structs recursively composed of accepted fields.
    /// Every other `Copy` type (including `i8`..`i64`, `usize`,
    /// `f32`, `char`, `u128`, tuples, enums, unions, `NonZeroU*`,
    /// references, function pointers, raw pointers, and
    /// unresolved generic parameters / projections) is rejected
    /// through this variant.
    UserDataNotZeroInitWhitelisted {
        /// Provenance of the offending struct field.
        field: ErrorProvenance,
        /// Rendered type that failed the whitelist predicate.
        rejected_type: String,
    },
    /// A resource field lacks its required `id = "..."` attribute.
    MissingResourceId {
        /// Provenance of the offending struct field.
        field: ErrorProvenance,
        /// Resource attribute kind (`"clock"`, `"reset"`, etc.).
        kind: &'static str,
    },
    /// Pass 1's direct extraction-side resource population asked an
    /// extraction surface to validate the crate and the surface
    /// rejected it.
    SurfaceExtractionFailed {
        /// Provenance anchored at the crate root.
        provenance: ErrorProvenance,
        /// Surface name (`"clock"`, `"interrupts"`, etc.).
        surface: &'static str,
        /// Human-readable extractor error.
        detail: String,
    },
    /// An `#[rseq::<...>]` attribute payload could not be parsed.
    ///
    /// Reserved for a stricter validation pass: Pass 1 is intentionally
    /// permissive about unknown `#[rseq::...]` attribute names so the DSL
    /// can add new attributes without breaking existing drivers. A later
    /// verifier may elect to raise this variant once the attribute set is
    /// frozen.
    UnknownRseqAttribute {
        /// Provenance for the offending attribute.
        provenance: ErrorProvenance,
        /// Rendered attribute name.
        attr: String,
    },
    /// A registry entry maps a DefId to a string that does not
    /// correspond to a known canonical name.
    RegistryLookupMismatch {
        /// Provenance of the mis-mapped item.
        provenance: ErrorProvenance,
        /// The canonical name string that failed the lookup.
        canonical: String,
    },
    /// The Phase 4 [`IntrinsicRegistry`] carries a canonical-name string
    /// that does not match any [`RegseqIntrinsic`] variant. Raised at
    /// Pass 2 setup when building the [`IntrinsicLookup`] (see
    /// [`construction::intrinsics`](self::intrinsics)), so bad entries
    /// surface once up-front instead of masquerading as silent
    /// fall-through at every unrelated call site.
    UnknownIntrinsicCanonicalName {
        /// Provenance of the registry-mapped item.
        provenance: ErrorProvenance,
        /// The canonical-name string that failed to parse.
        canonical: String,
    },
    /// An ADT encountered during MIR-to-IR type mapping resolves to a
    /// `std::*` path. Driver specs must not depend on `std`.
    StdTypeNotPermitted {
        /// Provenance of the offending type reference.
        provenance: ErrorProvenance,
        /// Rendered `std::*` path for diagnostics.
        path: String,
    },
    /// A rustc `ty::TyKind` encountered during MIR-to-IR type mapping
    /// has no `RegseqType` representation (closures, coroutines, raw
    /// pointers, function pointers, trait objects, etc.).
    UnsupportedRustType {
        /// Provenance of the offending type reference.
        provenance: ErrorProvenance,
        /// Rendered rustc type for diagnostics.
        rendered: String,
    },
    /// A MIR constant operand cannot be evaluated to a scalar or named
    /// constant reference (e.g., aggregate constants, slice constants).
    UnsupportedConstShape {
        /// Provenance of the offending constant operand.
        provenance: ErrorProvenance,
        /// Short description of the unsupported constant shape.
        rendered: String,
    },
    /// A MIR place projection has no Regseq IR representation
    /// (`OpaqueCast`, `UnwrapUnsafeBinder`).
    UnsupportedProjection {
        /// Provenance of the offending projection.
        provenance: ErrorProvenance,
        /// Rendered projection kind for diagnostics.
        rendered: String,
    },
    /// A MIR `Rvalue` has no Regseq IR representation
    /// (`ThreadLocalRef`, `RawPtr`, `ShallowInitBox`, `WrapUnsafeBinder`,
    /// and similar pointer-centric constructs).
    UnsupportedRvalue {
        /// Provenance of the offending rvalue.
        provenance: ErrorProvenance,
        /// Rendered rvalue kind for diagnostics.
        rendered: String,
    },
    /// A MIR terminator has no Regseq IR representation in Pass 2's
    /// basic-terminator subset. Covers `TailCall`, `Yield`,
    /// `CoroutineDrop`, `InlineAsm`, `FalseEdge`, `FalseUnwind`,
    /// `Assert`, `UnwindResume`, `UnwindTerminate`.
    UnsupportedTerminator {
        /// Provenance of the offending terminator.
        provenance: ErrorProvenance,
        /// Rendered terminator kind for diagnostics.
        rendered: String,
    },
    /// A `SwitchInt` terminator's original target indexed a cleanup
    /// block that Pass 2 dropped. Non-cleanup control flow must not
    /// route into the cleanup subtree.
    SwitchTargetIntoCleanup {
        /// Provenance of the offending SwitchInt.
        provenance: ErrorProvenance,
        /// The original MIR basic-block index of the dropped target.
        block_index: u32,
    },
    /// A MIR `Call` terminator's `func` operand does not resolve to a
    /// known [`FunctionId`]: the operand is not an `FnDef` constant, or
    /// its [`DefId`] is missing from the Pass 1 `FunctionIndex`.
    UnresolvedCallTarget {
        /// Provenance of the offending call site.
        provenance: ErrorProvenance,
        /// Rendered callee for diagnostics.
        rendered: String,
    },
    /// Pass 2 setup could not resolve `core::ops::Deref::deref` via
    /// [`TyCtxt::lang_items`] on the current crate. The pinned
    /// toolchain always exposes `Deref` on every `#![no_std]` crate
    /// that depends on `core`; a failure here indicates the
    /// toolchain is broken or the crate omits `core`. Rule 4 deref
    /// collapse cannot run without a resolved `Deref::deref` DefId,
    /// so construction aborts rather than silently disabling the
    /// rule.
    NoDerefLangItem {
        /// Provenance anchored at the crate root (or at the Deref
        /// trait, when that resolved but the `deref` assoc fn did
        /// not).
        crate_root: ErrorProvenance,
    },
    /// The `regseq` facade crate is in the dependency graph but at
    /// least one of the `regseq::internal::ordered_region_*` marker
    /// functions could not be resolved. Raised at Pass 2 setup when
    /// [`construction::ordered_region`](self::ordered_region) builds
    /// its marker cache -- defends against facade renames that would
    /// otherwise silently disable `ordered!` region detection.
    OrderedMarkerLookupFailed {
        /// Provenance anchored at the best-effort facade item (the
        /// crate root, or the `internal` submodule if it was found).
        provenance: ErrorProvenance,
        /// Diagnostic rendering naming the missing marker(s).
        rendered: String,
    },
    /// An `ordered!` region has an unmatched begin/end marker: a
    /// `regseq::internal::ordered_region_begin()` with no matching
    /// `ordered_region_end()` in the same body (or vice versa). Raised
    /// at Pass 2 when [`construction::ordered_region`](self::ordered_region)
    /// folds the captured marker sites into regions.
    UnpairedOrderedMarker {
        /// Provenance of the enclosing function body.
        provenance: ErrorProvenance,
        /// Diagnostic rendering naming the unmatched marker.
        rendered: String,
    },
    /// A `Call` terminator in an already-registered function body
    /// references a function that belongs to a crate outside the
    /// Rule 3 cross-crate allowlist (currently `core` and `regseq`).
    /// Driver specs are `#![no_std]`, so `std::*` references must
    /// be rejected; other non-allowlisted crates (e.g., `alloc`)
    /// follow the same rule. Raised by
    /// [`construction::cross_crate`](self::cross_crate) during the
    /// Pass 1 call-graph walk.
    StdFunctionNotPermitted {
        /// Provenance of the offending cross-crate callee.
        provenance: ErrorProvenance,
        /// Rendered `def_path_str` of the callee (e.g.,
        /// `"std::vec::Vec::<u8>::push"`).
        path: String,
        /// Rendered `def_path_str` of the authored function whose
        /// body reached the offending callee.
        caller: String,
    },
    /// Rule 3's cross-crate call-graph walk exceeded its depth
    /// bound (16 levels). Defensive guard against unbounded
    /// recursion through cyclic or deeply-nested `core` functions.
    /// Raised by
    /// [`construction::cross_crate`](self::cross_crate) when the
    /// walk hits the depth cap.
    CrossCrateRecursionLimit {
        /// Provenance of the deepest reached callee.
        provenance: ErrorProvenance,
        /// The depth at which the limit tripped.
        depth: u32,
    },
    /// Rule 2 `?`-pattern recognition fired (the originating Call's
    /// return target contains a `Try::branch` Call) but a subsequent
    /// structural check failed. Raised by
    /// [`construction::try_folding`](self::try_folding) when the
    /// Try-desugaring shape deviates from the algorithm documented
    /// in `ir-reference.md` Section 5 (SwitchInt arity/values,
    /// `otherwise: Unreachable` slot, Continue / Break extraction
    /// shape, `FromResidual::from_residual` arg shape). Carries the
    /// originating Call's MIR basic-block index and a short tag
    /// naming the offending sub-step so diagnostics can identify
    /// the first failing check.
    MalformedTryPattern {
        /// Provenance of the originating Call's call site.
        provenance: ErrorProvenance,
        /// MIR basic-block index of the originating `Call` whose
        /// `?` fold failed.
        call_mir_block: u32,
        /// Short tag naming the failing sub-step (e.g.,
        /// `"switch_targets_not_continue_break_unreachable"`,
        /// `"otherwise_not_unreachable"`). See the
        /// [`recognize`](self::try_folding) function for the full
        /// set of tags.
        step: &'static str,
    },
    /// A logging intrinsic call carries a template-string placeholder
    /// outside the restricted Regseq subset. Raised by
    /// [`construction::log_metadata`](self::log_metadata) when the
    /// template parser encounters a format specifier other than the
    /// supported `Display`, `LowerHex`, `UpperHex`,
    /// `LowerHexPrefixed`, `UpperHexPrefixed`, `PaddedHex`, or
    /// `Pointer` variants (for example `{:?}` or named placeholders).
    UnsupportedLogFormat {
        /// Provenance of the originating logging call site.
        provenance: ErrorProvenance,
        /// The full template string carrying the offending placeholder.
        template: String,
        /// Rendered form of the offending placeholder (e.g. `{:?}`).
        placeholder: String,
    },
    /// A logging intrinsic call's parsed placeholder count does not
    /// match the number of supplied arguments after `LogArg` carrier
    /// stripping. Raised by
    /// [`construction::log_metadata`](self::log_metadata) when arity
    /// validation fails (template `{} {}` with one argument, etc.).
    LogPlaceholderArityMismatch {
        /// Provenance of the originating logging call site.
        provenance: ErrorProvenance,
        /// The full template string carrying the placeholders.
        template: String,
        /// Number of placeholders parsed from the template.
        placeholders: usize,
        /// Number of supplied arguments after `LogArg` stripping.
        args: usize,
    },
    /// A register intrinsic call names a register marker type that
    /// belongs to a different register block than the call's
    /// `&Mmio<Block>` receiver. Raised by Rule 7's metadata
    /// resolution as a defensive guard against extraction surprises
    /// -- well-formed source never triggers it because type-checking
    /// already rejects cross-block field usage via the `RegisterOf<Block>`
    /// trait bound.
    CrossBlockFieldUsage {
        /// Provenance of the offending intrinsic call site.
        provenance: ErrorProvenance,
        /// Register block identified by the receiver's `Mmio<Block>`
        /// type.
        receiver_block: regseq_model_old::identity::RegisterBlockId,
        /// Register block recorded on the marker type's
        /// [`RegisterEntry`](self::register_metadata::RegisterEntry).
        register_block: regseq_model_old::identity::RegisterBlockId,
        /// Rendered def-path of the offending register marker.
        marker: String,
    },
    /// A register intrinsic call's access kind (read vs write) does
    /// not match the marker type's registered
    /// [`AccessKind`](regseq_model_old::model::AccessKind). Raised by
    /// Rule 7's metadata resolution as a defensive guard --
    /// well-formed source never triggers it because the
    /// `R::Access: ReadAccess | WriteAccess` trait bound on each
    /// intrinsic already rejects mismatched access kinds at
    /// type-check time.
    WrongRegisterAccessKind {
        /// Provenance of the offending intrinsic call site.
        provenance: ErrorProvenance,
        /// Rendered def-path of the offending register marker.
        marker: String,
        /// Access kind recorded on the marker's
        /// [`RegisterEntry`](self::register_metadata::RegisterEntry).
        access_kind: regseq_model_old::model::AccessKind,
        /// Intrinsic variant whose required access kind was not met.
        intrinsic: regseq_model_old::ir::RegseqIntrinsic,
    },
    /// A `PinCtrlApply` intrinsic call's monomorphized `S` type
    /// parameter resolves to a marker type whose `TypeDef.attrs` bag
    /// carries no well-formed `#[rseq::pin_state(id = "...")]` entry.
    /// Phase 4.2 attribute extraction already rejects malformed
    /// pin-state attributes earlier, so this error only fires when
    /// an otherwise-valid marker was never annotated at all -- in
    /// that case the driver spec is using a type that does not
    /// identify a pin-state. Raised by
    /// [`construction::pinctrl_metadata`](self::pinctrl_metadata) as
    /// a defensive guard during Pass 2 metadata resolution.
    MissingPinStateAttribute {
        /// Provenance of the offending marker type (falls back to
        /// the call site when the marker `DefId` cannot be
        /// resolved).
        provenance: ErrorProvenance,
        /// Rendered def-path of the marker type whose attrs bag
        /// lacks a `pin_state` entry.
        marker: String,
    },
}

impl std::fmt::Display for ConstructionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoDriverStruct { .. } => write!(f, "no #[rseq::driver] struct found in crate"),
            Self::MultipleDriverStructs { drivers } => {
                write!(
                    f,
                    "multiple #[rseq::driver] structs found ({} total)",
                    drivers.len(),
                )
            }
            Self::DuplicateItemName { module, name } => write!(
                f,
                "duplicate item name `{}` in module `{}`",
                name, module.item_path
            ),
            Self::InvalidUserDataShape {
                field,
                rendered_type,
            } => write!(
                f,
                "user-data field `{}` has type `{}`; must be either a zero-init-whitelisted type (Rule 1) or `Option<T: Copy>` (Rule 2)",
                field.item_path, rendered_type,
            ),
            Self::UserDataNotCopy { field, inner_type } => write!(
                f,
                "user-data field `{}`: type `{}` is not Copy",
                field.item_path, inner_type,
            ),
            Self::UserDataNotZeroInitWhitelisted {
                field,
                rejected_type,
            } => write!(
                f,
                "user-data field `{}`: type `{}` is not accepted by the Rule-1 zero-init whitelist (admitted: u8, u16, u32, u64, bool; arrays of those; Copy structs recursively composed of accepted fields)",
                field.item_path, rejected_type,
            ),
            Self::MissingResourceId { field, kind } => write!(
                f,
                "resource field `{}` is missing required id on #[rseq::{kind}]",
                field.item_path,
            ),
            Self::SurfaceExtractionFailed {
                provenance,
                surface,
                detail,
            } => write!(
                f,
                "extraction surface `{surface}` failed while building Pass 1 (at `{}`): {detail}",
                provenance.item_path,
            ),
            Self::UnknownRseqAttribute { provenance, attr } => write!(
                f,
                "malformed or unknown attribute `#[rseq::{}]` on `{}`",
                attr, provenance.item_path,
            ),
            Self::RegistryLookupMismatch {
                provenance,
                canonical,
            } => write!(
                f,
                "registry-mapped canonical name `{}` is not recognized (item `{}`)",
                canonical, provenance.item_path,
            ),
            Self::UnknownIntrinsicCanonicalName {
                provenance,
                canonical,
            } => write!(
                f,
                "unknown #[rseq::intrinsic] canonical name `{}` on `{}`",
                canonical, provenance.item_path,
            ),
            Self::StdTypeNotPermitted { provenance, path } => write!(
                f,
                "driver specs must not use `std`: `{}` at `{}`",
                path, provenance.item_path,
            ),
            Self::UnsupportedRustType {
                provenance,
                rendered,
            } => write!(
                f,
                "rustc type `{}` has no Regseq IR representation (at `{}`)",
                rendered, provenance.item_path,
            ),
            Self::UnsupportedConstShape {
                provenance,
                rendered,
            } => write!(
                f,
                "unsupported constant shape `{}` (at `{}`)",
                rendered, provenance.item_path,
            ),
            Self::UnsupportedProjection {
                provenance,
                rendered,
            } => write!(
                f,
                "unsupported MIR projection `{}` (at `{}`)",
                rendered, provenance.item_path,
            ),
            Self::UnsupportedRvalue {
                provenance,
                rendered,
            } => write!(
                f,
                "unsupported MIR rvalue `{}` (at `{}`)",
                rendered, provenance.item_path,
            ),
            Self::UnsupportedTerminator {
                provenance,
                rendered,
            } => write!(
                f,
                "unsupported MIR terminator `{}` (at `{}`)",
                rendered, provenance.item_path,
            ),
            Self::UnresolvedCallTarget {
                provenance,
                rendered,
            } => write!(
                f,
                "MIR call target `{}` cannot be resolved to a FunctionId (at `{}`)",
                rendered, provenance.item_path,
            ),
            Self::NoDerefLangItem { crate_root } => write!(
                f,
                "core::ops::Deref lang item is not available for Pass 2 setup (at `{}`)",
                crate_root.item_path,
            ),
            Self::SwitchTargetIntoCleanup {
                provenance,
                block_index,
            } => write!(
                f,
                "SwitchInt target bb{block_index} is a cleanup block (at `{}`)",
                provenance.item_path,
            ),
            Self::OrderedMarkerLookupFailed {
                provenance,
                rendered,
            } => write!(
                f,
                "ordered-region marker lookup failed: `{}` (at `{}`)",
                rendered, provenance.item_path,
            ),
            Self::UnpairedOrderedMarker {
                provenance,
                rendered,
            } => write!(
                f,
                "unpaired ordered-region marker: `{}` (at `{}`)",
                rendered, provenance.item_path,
            ),
            Self::StdFunctionNotPermitted {
                provenance,
                path,
                caller,
            } => write!(
                f,
                "driver specs must not call `std` or non-allowlisted crates: `{path}` reached from `{caller}` (at `{}`)",
                provenance.item_path,
            ),
            Self::CrossCrateRecursionLimit { provenance, depth } => write!(
                f,
                "cross-crate call-graph walk exceeded depth {depth} (at `{}`)",
                provenance.item_path,
            ),
            Self::MalformedTryPattern {
                provenance,
                call_mir_block,
                step,
            } => write!(
                f,
                "malformed `?`-pattern at bb{call_mir_block} (step: {step}) (at `{}`)",
                provenance.item_path,
            ),
            Self::UnsupportedLogFormat {
                provenance,
                template,
                placeholder,
            } => write!(
                f,
                "unsupported log format placeholder `{placeholder}` in template `{template}` (at `{}`)",
                provenance.item_path,
            ),
            Self::LogPlaceholderArityMismatch {
                provenance,
                template,
                placeholders,
                args,
            } => write!(
                f,
                "log template `{template}` has {placeholders} placeholder(s) but {args} argument(s) (at `{}`)",
                provenance.item_path,
            ),
            Self::CrossBlockFieldUsage {
                provenance,
                receiver_block,
                register_block,
                marker,
            } => write!(
                f,
                "register marker `{marker}` belongs to block `{register_block:?}` but the receiver targets block `{receiver_block:?}` (at `{}`)",
                provenance.item_path,
            ),
            Self::WrongRegisterAccessKind {
                provenance,
                marker,
                access_kind,
                intrinsic,
            } => write!(
                f,
                "register marker `{marker}` has access kind `{access_kind:?}` but intrinsic `{}` requires a different access (at `{}`)",
                intrinsic.canonical_name(),
                provenance.item_path,
            ),
            Self::MissingPinStateAttribute {
                provenance,
                marker,
            } => write!(
                f,
                "pin-state marker `{marker}` has no `#[rseq::pin_state(id = \"...\")]` attribute (at `{}`)",
                provenance.item_path,
            ),
        }
    }
}

impl std::error::Error for ConstructionError {}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Cross-reference maps produced by Pass 1. Downstream passes (task 43+
/// MIR lowering) consume these to resolve rustc [`DefId`]s to the
/// model's typed IDs without re-walking the HIR.
#[derive(Clone, Default)]
pub struct CrossRefMaps {
    /// Local ADTs keyed by their rustc `DefId`.
    pub type_defs: std::collections::HashMap<DefId, regseq_model_old::identity::TypeDefId>,
    /// `#[rseq::register_block]` structs.
    pub register_blocks: std::collections::HashMap<DefId, regseq_model_old::identity::RegisterBlockId>,
    /// Free functions and methods, used by Pass 2 to resolve call targets
    /// against the model's function surface.
    pub functions: std::collections::HashMap<DefId, regseq_model_old::identity::FunctionId>,
    /// `register_bitfields!`-style descriptor modules.
    pub register_descriptors:
        std::collections::HashMap<DefId, regseq_model_old::identity::RegisterDescriptorId>,
    /// Named constants and statics (free or associated), used by Pass 2
    /// to resolve `Operand::Constant` references from unevaluated MIR
    /// constants.
    pub constants: std::collections::HashMap<DefId, regseq_model_old::identity::ConstantId>,
    /// Register marker type index built by Rule 7. Pass 2 consults
    /// this at every register-intrinsic `Call` site to attach
    /// [`IntrinsicMetadata::Register`](regseq_model_old::ir::IntrinsicMetadata::Register).
    pub register_index: register_metadata::RegisterIndex,
    /// Pin-state marker index (task regseq-model-52). Maps each
    /// `#[rseq::pin_state(id = "...")]`-annotated TypeDef's `DefId`
    /// to its interned `state_id` [`Symbol`]. Pass 2 consults this
    /// at every `PinCtrlApply` call site to attach
    /// [`IntrinsicMetadata::PinCtrl`](regseq_model_old::ir::IntrinsicMetadata::PinCtrl).
    pub pin_state_lookup: pinctrl_metadata::PinStateLookup,
}

/// Full Pass 1 output: the declaration-level [`RegseqModel`] plus the
/// cross-reference maps needed by Pass 2 to resolve rustc [`DefId`]s to
/// model IDs.
pub struct Pass1Output {
    pub model: RegseqModel,
    pub cross_refs: CrossRefMaps,
}

/// Run Pass 1 of Regseq-Model construction and return a
/// [`RegseqModel`] with every declaration-level identifier assigned.
///
/// Function bodies are attached as empty `Body` placeholders -- Pass 2
/// (out of scope for this function) replaces them with translated MIR.
/// Every binding-side slot on resources is set to `None`; platform join
/// (Phase 6) populates them later.
pub fn build_declaration_skeleton<'tcx>(
    tcx: TyCtxt<'tcx>,
    registries: &ExtractionRegistries<'_>,
) -> Result<RegseqModel, ConstructionError> {
    build_declaration_skeleton_inner(tcx, registries, cross_crate::MAX_CROSS_CRATE_DEPTH)
        .map(|out| out.model)
}

/// Test-only variant of [`build_pass1_output`] that lets callers
/// override Rule 3's cross-crate depth bound. Production code
/// always uses [`cross_crate::MAX_CROSS_CRATE_DEPTH`]. Exposed at
/// `#[doc(hidden)]` visibility so integration tests can exercise
/// the [`ConstructionError::CrossCrateRecursionLimit`] path
/// without synthesizing a 17-deep call chain through real `core`
/// MIR (infeasible in a stand-alone specimen). Not part of the
/// stable public API.
#[doc(hidden)]
pub fn build_pass1_output_with_cross_crate_depth<'tcx>(
    tcx: TyCtxt<'tcx>,
    registries: &ExtractionRegistries<'_>,
    max_cross_crate_depth: u32,
) -> Result<Pass1Output, ConstructionError> {
    build_declaration_skeleton_inner(tcx, registries, max_cross_crate_depth)
}

fn build_declaration_skeleton_inner<'tcx>(
    tcx: TyCtxt<'tcx>,
    registries: &ExtractionRegistries<'_>,
    max_cross_crate_depth: u32,
) -> Result<Pass1Output, ConstructionError> {
    let mut symbols = SymbolInterner::new();
    let mut source_locations: BTreeMap<DeclId, SourceOrigin> = BTreeMap::new();
    let mut allocator = DeclIdAllocator::new();

    let crate_name = tcx.crate_name(rustc_hir::def_id::LOCAL_CRATE);
    let root_name: Symbol = symbols.intern(crate_name.as_str());
    let root_id: ModuleId = ModuleId::from(allocator.alloc());
    // The crate root has its own DefId (`CRATE_DEF_ID`) with a real span
    // covering the whole crate file; use it rather than borrowing a span
    // from the first free item (which can be synthetic or missing).
    let root_def_id = rustc_hir::def_id::CRATE_DEF_ID.to_def_id();
    record_span(
        tcx,
        &mut symbols,
        &mut source_locations,
        root_id.decl_id(),
        tcx.def_span(root_def_id),
    );

    let mut cx = walk::BuildContext {
        tcx,
        registries,
        symbols: &mut symbols,
        source_locations: &mut source_locations,
        allocator: &mut allocator,
        driver_modules: Vec::new(),
        type_def_map: std::collections::HashMap::new(),
        register_block_map: std::collections::HashMap::new(),
        function_map: std::collections::HashMap::new(),
        descriptor_map: std::collections::HashMap::new(),
        constant_map: std::collections::HashMap::new(),
    };

    let mut root_module = Module {
        id: root_id,
        name: root_name,
        kind: ModuleKind::Normal,
        submodules: Vec::new(),
        constants: Vec::new(),
        free_functions: Vec::new(),
        type_defs: Vec::new(),
        register_descriptors: Vec::new(),
        register_blocks: Vec::new(),
        impl_blocks: Vec::new(),
        driver: None,
        trait_impls: Vec::new(),
        driver_impl_blocks: Vec::new(),
        abstractions_used: std::collections::BTreeSet::new(),
        intrinsics_used: std::collections::BTreeSet::new(),
    };

    // Crate-wide pre-pass: pre-allocate `RegisterBlockId`s for every
    // `#[rseq::register_block]` struct anywhere in the module tree so
    // `Mmio<T>` fields on driver structs can resolve their block id
    // regardless of the source-order relationship between the driver
    // module and the registers module. Without this pre-pass a driver
    // struct in a module walked before the registers submodule would
    // fail with `MissingResourceId { kind: "mmio-register-block" }`.
    walk::prealloc_register_block_ids_crate_wide(
        &mut cx,
        rustc_hir::def_id::CRATE_DEF_ID.to_def_id(),
    );

    walk::populate_module_items_and_submodules(
        &mut cx,
        &mut root_module,
        rustc_hir::def_id::CRATE_DEF_ID.to_def_id(),
    )?;

    match cx.driver_modules.len() {
        0 => {
            return Err(ConstructionError::NoDriverStruct {
                crate_root: ErrorProvenance::for_def_id(tcx, root_def_id),
            });
        }
        1 => {}
        _ => {
            return Err(ConstructionError::MultipleDriverStructs {
                drivers: cx.driver_modules,
            });
        }
    }

    // Rule 3 cross-crate function extraction (task regseq-model-47).
    // Walk every `Call` terminator reachable from an authored body
    // and register the referenced `core` / `regseq` callees as
    // regular `Function`s under synthetic module chains.
    cross_crate::extract_cross_crate_functions_with_depth_bound(
        &mut cx,
        &mut root_module,
        max_cross_crate_depth,
    )?;

    // Cross-crate type extraction: register every non-lang-item ADT
    // referenced by a body (authored or cross-crate) so Pass 2's
    // `map_ty` can resolve each reference to `RegseqType::Defined`
    // instead of surfacing `UnsupportedRustType`. This integration
    // pass is additive and does not introduce a new recognition
    // rule: body-level IR construction rules 1-7 are unchanged.
    cross_crate_types::extract_cross_crate_types(&mut cx, &mut root_module)?;

    let interrupt_surface = crate::extract::interrupts::extract_interrupt_surface(tcx)
        .map_err(|err| extraction_surface_failed(tcx, root_def_id, "interrupts", err))?;
    let isr_handler_names: std::collections::HashSet<String> = interrupt_surface
        .isr_bindings
        .iter()
        .map(|binding| binding.handler_name.clone())
        .collect();
    let sync_surface =
        crate::extract::synchronization::extract_synchronization_surface(tcx, &isr_handler_names)
            .map_err(|err| extraction_surface_failed(tcx, root_def_id, "synchronization", err))?;
    let trait_surface = crate::extract::traits::extract_trait_surface(tcx)
        .map_err(|err| extraction_surface_failed(tcx, root_def_id, "traits", err))?;
    let property_surface = crate::extract::property::extract_property_surface(tcx)
        .map_err(|err| extraction_surface_failed(tcx, root_def_id, "property", err))?;
    let clock_surface = match crate::extract::clock::extract_clock_surface(tcx) {
        Ok(surface) => surface,
        Err(crate::extract::clock::ClockError::MissingClockId(field_name)) => {
            return Err(missing_resource_id_for_driver_field(
                tcx,
                root_def_id,
                &field_name,
                "clock",
            ));
        }
        Err(err) => return Err(extraction_surface_failed(tcx, root_def_id, "clock", err)),
    };
    let reset_surface = crate::extract::reset::extract_reset_surface(tcx)
        .map_err(|err| extraction_surface_failed(tcx, root_def_id, "reset", err))?;
    let pinctrl_surface = crate::extract::pinctrl::extract_pinctrl_surface(tcx)
        .map_err(|err| extraction_surface_failed(tcx, root_def_id, "pinctrl", err))?;
    let pm_impl_types: std::collections::HashSet<String> = trait_surface
        .power_management_impls
        .iter()
        .map(|pm| pm.driver_type.clone())
        .collect();
    let pm_surface = crate::extract::pm::extract_pm_surface(tcx, &pm_impl_types)
        .map_err(|err| extraction_surface_failed(tcx, root_def_id, "pm", err))?;

    resources::populate_clocks(&mut cx, &mut root_module, &clock_surface)?;
    resources::populate_resets(&mut cx, &mut root_module, &reset_surface)?;
    resources::populate_pinctrl(&mut cx, &mut root_module, &pinctrl_surface)?;
    resources::populate_interrupts(&mut cx, &mut root_module, &interrupt_surface)?;
    resources::populate_sync(&mut cx, &mut root_module, &sync_surface)?;
    resources::populate_properties(&mut cx, &mut root_module, &property_surface)?;
    resources::populate_user_data(&mut cx, &mut root_module)?;
    resources::populate_pm_info(&mut root_module, &pm_surface)?;

    // Extract the map + register-block list from `cx` before the
    // Rule 7 builder below (which borrows `symbols` mutably) so the
    // borrow checker can see `cx`'s `&mut symbols` is gone.
    let register_block_refs = collect_register_block_refs(&root_module, &cx.register_block_map);
    let walk::BuildContext {
        type_def_map,
        register_block_map,
        function_map,
        descriptor_map,
        constant_map,
        ..
    } = cx;

    // Rule 7 register marker -> metadata index (task regseq-model-51).
    // Walk the built register blocks and collect per-register entries
    // keyed by the marker type's `DefId`. Running here, after the HIR
    // walk completes, keeps the register-index build independent of
    // the in-progress Pass 1 state -- it only consumes the stable
    // `register_block_map` plus the model's block list + the built
    // descriptor list (which is the single source of truth for
    // bit-field mask/shift/name metadata).
    let descriptor_lookup = collect_descriptor_lookup(&root_module);
    let register_index = register_metadata::RegisterIndex::build(
        tcx,
        &register_block_map,
        &register_block_refs,
        &descriptor_lookup,
        &mut symbols,
    );

    // Rule 8 pin-state marker -> state_id index (task regseq-model-52).
    // Walk the populated module tree and intern each `#[rseq::pin_state(
    // id = "...")]`-annotated TypeDef's id string into the Pass 1 symbol
    // interner so Pass 2 can attach
    // `IntrinsicMetadata::PinCtrl { state_id }` to every `PinCtrlApply`
    // call without re-reading attrs from HIR.
    let pin_state_lookup =
        pinctrl_metadata::PinStateLookup::build(&root_module, &type_def_map, &mut symbols);

    let cross_refs = CrossRefMaps {
        type_defs: type_def_map,
        register_blocks: register_block_map,
        functions: function_map,
        register_descriptors: descriptor_map,
        constants: constant_map,
        register_index,
        pin_state_lookup,
    };

    Ok(Pass1Output {
        model: RegseqModel {
            symbols,
            source_locations,
            root_module,
            format_version: 1,
        },
        cross_refs,
    })
}

fn extraction_surface_failed(
    tcx: TyCtxt<'_>,
    root_def_id: DefId,
    surface: &'static str,
    err: impl std::fmt::Debug,
) -> ConstructionError {
    ConstructionError::SurfaceExtractionFailed {
        provenance: ErrorProvenance::for_def_id(tcx, root_def_id),
        surface,
        detail: format!("{err:?}"),
    }
}

fn missing_resource_id_for_driver_field(
    tcx: TyCtxt<'_>,
    root_def_id: DefId,
    field_name: &str,
    kind: &'static str,
) -> ConstructionError {
    fn find_field(tcx: TyCtxt<'_>, module_def_id: DefId, field_name: &str) -> Option<DefId> {
        let local = module_def_id
            .as_local()
            .unwrap_or(rustc_hir::def_id::CRATE_DEF_ID);
        let mod_id = rustc_hir::def_id::LocalModDefId::new_unchecked(local);
        for item_id in tcx.hir_module_items(mod_id).free_items() {
            let item = tcx.hir_item(item_id);
            let def_id = item.owner_id.def_id.to_def_id();
            match &item.kind {
                rustc_hir::ItemKind::Struct(_, _, variant_data) => {
                    let is_driver = tcx.get_all_attrs(def_id).iter().any(|attr| {
                        let rustc_hir::Attribute::Unparsed(item) = attr else {
                            return false;
                        };
                        let segs = &item.path.segments;
                        segs.len() == 2
                            && segs[0].as_str() == "rseq"
                            && segs[1].as_str() == "driver"
                    });
                    if !is_driver {
                        continue;
                    }
                    for field in variant_data.fields() {
                        if field.ident.name.as_str() == field_name {
                            return Some(field.def_id.to_def_id());
                        }
                    }
                }
                rustc_hir::ItemKind::Mod(..) => {
                    if let Some(def_id) = find_field(tcx, def_id, field_name) {
                        return Some(def_id);
                    }
                }
                _ => {}
            }
        }
        None
    }

    let field_def_id = find_field(tcx, rustc_hir::def_id::CRATE_DEF_ID.to_def_id(), field_name)
        .unwrap_or(root_def_id);

    ConstructionError::MissingResourceId {
        field: ErrorProvenance::for_def_id(tcx, field_def_id),
        kind,
    }
}

/// Walk the populated module tree and return every built
/// [`RegisterBlock`] paired with its `DefId`. Used by
/// [`build_declaration_skeleton_inner`] to seed Rule 7's
/// [`RegisterIndex`] without re-walking the HIR.
fn collect_register_block_refs(
    module: &regseq_model_old::model::Module,
    register_block_map: &std::collections::HashMap<DefId, regseq_model_old::identity::RegisterBlockId>,
) -> Vec<(DefId, regseq_model_old::model::RegisterBlock)> {
    // Reverse the `DefId -> RegisterBlockId` map so we can look up
    // each model-side `RegisterBlockId` back to its authored `DefId`.
    // The map is small (one entry per block) so the linear scan is
    // acceptable.
    let mut out: Vec<(DefId, regseq_model_old::model::RegisterBlock)> = Vec::new();
    collect_register_block_refs_inner(module, register_block_map, &mut out);
    out
}

/// Walk the populated module tree and build a
/// [`RegisterDescriptorId`] -> [`RegisterDescriptor`] lookup used by
/// Rule 7's register-index build to source authoritative bit-field
/// mask/shift/name metadata.
fn collect_descriptor_lookup(
    module: &regseq_model_old::model::Module,
) -> std::collections::HashMap<
    regseq_model_old::identity::RegisterDescriptorId,
    regseq_model_old::model::RegisterDescriptor,
> {
    let mut out = std::collections::HashMap::new();
    collect_descriptor_lookup_inner(module, &mut out);
    out
}

fn collect_descriptor_lookup_inner(
    module: &regseq_model_old::model::Module,
    out: &mut std::collections::HashMap<
        regseq_model_old::identity::RegisterDescriptorId,
        regseq_model_old::model::RegisterDescriptor,
    >,
) {
    for d in &module.register_descriptors {
        out.insert(d.id, d.clone());
    }
    for sub in &module.submodules {
        collect_descriptor_lookup_inner(sub, out);
    }
}

fn collect_register_block_refs_inner(
    module: &regseq_model_old::model::Module,
    register_block_map: &std::collections::HashMap<DefId, regseq_model_old::identity::RegisterBlockId>,
    out: &mut Vec<(DefId, regseq_model_old::model::RegisterBlock)>,
) {
    for block in &module.register_blocks {
        if let Some((&def_id, _)) = register_block_map.iter().find(|(_, id)| **id == block.id) {
            out.push((def_id, block.clone()));
        }
    }
    for sub in &module.submodules {
        collect_register_block_refs_inner(sub, register_block_map, out);
    }
}

/// Run Pass 1 and return both the [`RegseqModel`] and the cross-reference
/// maps downstream passes need to resolve rustc [`DefId`]s to model IDs.
/// The plain [`build_declaration_skeleton`] discards these maps; when
/// wiring Pass 2 (task 43+), use this entry point instead so call targets
/// and type references can be resolved without re-walking the HIR.
pub fn build_pass1_output<'tcx>(
    tcx: TyCtxt<'tcx>,
    registries: &ExtractionRegistries<'_>,
) -> Result<Pass1Output, ConstructionError> {
    build_declaration_skeleton_inner(tcx, registries, cross_crate::MAX_CROSS_CRATE_DEPTH)
}

// ---------------------------------------------------------------------------
// Span helpers (shared with walk submodule)
// ---------------------------------------------------------------------------

/// Resolve a [`Span`] into a [`SourceOrigin`] and record it under
/// `decl_id`. Returns early for dummy spans, writing
/// [`SourceOrigin::CompilerGenerated`] instead (should not occur in Pass
/// 1).
pub(crate) fn record_span(
    tcx: TyCtxt<'_>,
    symbols: &mut SymbolInterner,
    out: &mut BTreeMap<DeclId, SourceOrigin>,
    decl_id: DeclId,
    span: Span,
) {
    let origin = resolve_span(tcx, symbols, span).unwrap_or(SourceOrigin::CompilerGenerated);
    out.insert(decl_id, origin);
}

/// Resolve a span into a [`SourceOrigin`], returning
/// [`SourceOrigin::CompilerGenerated`] when the span cannot be mapped
/// to a real file (dummy span, cross-file macro, etc.).
pub(crate) fn resolve_origin(
    tcx: TyCtxt<'_>,
    symbols: &mut SymbolInterner,
    span: Span,
) -> SourceOrigin {
    resolve_span(tcx, symbols, span).unwrap_or(SourceOrigin::CompilerGenerated)
}

fn resolve_span(tcx: TyCtxt<'_>, symbols: &mut SymbolInterner, span: Span) -> Option<SourceOrigin> {
    if span.is_dummy() {
        return None;
    }
    // Drill through macro expansions to the originating user call
    // site. Pass 2's spec says `Source(...)` whenever a rustc `Span`
    // is available and non-dummy, and reserves `CompilerGenerated`
    // for genuinely synthesized nodes (see ir-reference.md Section 7
    // design rationale: "use the originating user span"). A macro
    // body's span points at a synthetic expansion file; walking to
    // `source_callsite()` recovers the author-visible location.
    let span = span.source_callsite();
    let source_map = tcx.sess.source_map();
    let lo = source_map.lookup_char_pos(span.lo());
    let hi = source_map.lookup_char_pos(span.hi());

    let (lo_real, hi_real) = match (&lo.file.name, &hi.file.name) {
        (rustc_span::FileName::Real(l), rustc_span::FileName::Real(h)) => (l, h),
        _ => return None,
    };
    if lo_real != hi_real {
        return None;
    }
    let file_path = lo_real.local_path()?.to_string_lossy().into_owned();
    let file = symbols.intern_file(&file_path);

    Some(SourceOrigin::Source(SourceLocation {
        file,
        start_line: lo.line as u32,
        start_column: (lo.col.0 as u32) + 1,
        end_line: hi.line as u32,
        end_column: (hi.col.0 as u32) + 1,
    }))
}
