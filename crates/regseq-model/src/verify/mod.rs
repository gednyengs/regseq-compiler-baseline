//! Verifier framework for the [`RegseqModel`].
//!
//! Hosts the error-type vocabulary, level taxonomy, and verification
//! entry points used after phase transitions and (eventually) every
//! model mutation (spec.md Section 13).
//!
//! Task 55 implements:
//! - the public error types ([`VerifierError`], [`VerifierLevel`],
//!   [`VerifierErrorKind`], [`VerifierProvenance`]),
//! - the module-level and cross-item-level rule buckets
//!   ([`rules::module`], [`rules::cross_item`]),
//! - the post-Phase-1 entry function [`verify_phase_1`].
//!
//! Tasks 56/57/58/59 extend this surface additively: they will flesh
//! out [`phase_2`], [`rules::driver_struct`], [`rules::body`], and a
//! mutation-guard layer.
//!
//! # Provenance
//!
//! [`VerifierProvenance`] is a minimal, rustc-independent analogue of
//! `regseq-driver`'s `ErrorProvenance`. The driver's struct carries a
//! live rustc `Span`, which cannot cross into [`regseq_model`] (this
//! crate is rustc-independent -- see the crate-level rustdoc on
//! [`crate`]). Instead, verifier provenance carries an
//! [`SourceOrigin`] -- the same origin stored in
//! [`RegseqModel::source_locations`] -- and an optional best-effort
//! `item_path` string. The shape mirrors the driver's struct so future
//! diagnostics can render either one uniformly.
//!
//! [`RegseqModel`]: crate::model::RegseqModel
//! [`RegseqModel::source_locations`]: crate::model::RegseqModel::source_locations
//! [`SourceOrigin`]: crate::identity::SourceOrigin

// The task spec prescribes the exact `Result<(), VerifierError>`
// signature for `verify_phase_1` and its peers. `VerifierError` is a
// first-class value (not a `Box`) because downstream consumers match
// directly on `.kind` and `.level`. Box-wrapping the `Err` payload
// would force every match site to dereference first. The clippy lint
// is off here project-wide for the verify module so every rule
// bucket can keep the prescribed signature.
#![allow(clippy::result_large_err)]

use serde::Serialize;

use crate::identity::{
    DeclId, FunctionId, RegisterBlockId, RegisterDescriptorId, SourceOrigin, TypeDefId,
};

pub mod phase_1;
pub mod phase_2;
pub mod rules;

pub use phase_1::verify_phase_1;

/// The level of a verifier rule, per spec.md Section 13 lines 1424-1432.
///
/// Every [`VerifierError`] carries the level of the rule that produced
/// it so downstream consumers can filter or group diagnostics by
/// structural scope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", content = "data")]
pub enum VerifierLevel {
    /// Structural invariants on the module tree (one Driver module,
    /// unique item names within a module).
    Module,
    /// Resource-manifest invariants on the driver struct (completeness
    /// of taxonomic fields, no phase-boundary leakage).
    DriverStruct,
    /// Function body invariants (owned by task 56).
    Body,
    /// Cross-reference resolution across items and source-location
    /// coverage for every assigned [`DeclId`].
    CrossItem,
}

/// The specific violation detected by a verifier rule.
///
/// Variants are grouped by the rule bucket that produces them. New
/// variants land as their owning rule bucket is implemented (tasks
/// 56-59). Every variant is stable once introduced -- downstream
/// matchers rely on the tagged shape.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", content = "data")]
pub enum VerifierErrorKind {
    // ----- Module level (task 55) -------------------------------------
    /// The model's module tree contains no [`ModuleKind::Driver`] module.
    ///
    /// [`ModuleKind::Driver`]: crate::model::module::ModuleKind::Driver
    MissingDriverModule,
    /// The module tree contains more than one Driver module.
    MultipleDriverModules {
        /// Total count of Driver modules found.
        count: usize,
    },
    /// A module has two items in the same category with identical names.
    DuplicateItemName {
        /// Path-qualified name of the containing module.
        module: String,
        /// The duplicated item name.
        name: String,
    },

    // ----- Cross-item level (task 55) ---------------------------------
    /// A [`RegseqType::Defined`] (or other typed-id reference) targets a
    /// [`TypeDefId`] that is not declared in the model.
    ///
    /// [`RegseqType::Defined`]: crate::ir::RegseqType::Defined
    UnresolvedTypeDefId {
        /// The [`DeclId`] of the item that made the reference.
        referencing: DeclId,
        /// The unresolved target.
        target: TypeDefId,
    },
    /// A [`TerminatorKind::Call`] callee targets a [`FunctionId`] that
    /// is not declared in the model.
    ///
    /// [`TerminatorKind::Call`]: crate::ir::TerminatorKind::Call
    UnresolvedFunctionId {
        /// The [`DeclId`] of the item that made the reference.
        referencing: DeclId,
        /// The unresolved target.
        target: FunctionId,
    },
    /// An MMIO resource (or intrinsic metadata) references a
    /// [`RegisterBlockId`] that is not declared in the model.
    UnresolvedRegisterBlockId {
        /// The [`DeclId`] of the item that made the reference.
        referencing: DeclId,
        /// The unresolved target.
        target: RegisterBlockId,
    },
    /// A [`RegisterBlockField`] references a [`RegisterDescriptorId`]
    /// that is not declared in the model.
    ///
    /// [`RegisterBlockField`]: crate::model::register::RegisterBlockField
    UnresolvedRegisterDescriptorId {
        /// The [`DeclId`] of the item that made the reference.
        referencing: DeclId,
        /// The unresolved target.
        target: RegisterDescriptorId,
    },
    /// A [`DeclId`] assigned during Phase 5 Pass 1 has no entry in
    /// [`RegseqModel::source_locations`].
    ///
    /// [`RegseqModel::source_locations`]: crate::model::RegseqModel::source_locations
    MissingSourceLocation {
        /// The declaration whose source location is missing.
        decl_id: DeclId,
    },

    // ----- Phase-1 boundary leakage (task 55) -------------------------
    /// A resource binding slot is `Some` in a post-Phase-1 model. Any
    /// populated binding indicates data leaking across the Phase-1
    /// boundary (spec.md Section 13 line 1400).
    UnexpectedBindingPopulated {
        /// The resource field that carried the populated binding.
        resource: DeclId,
        /// Which binding slot was populated (`"clock"`, `"reset"`,
        /// `"pinctrl"`, `"interrupt"`, or `"mmio"`).
        slot: &'static str,
    },
    /// A property's `bound_value` is `Some` in a post-Phase-1 model.
    UnexpectedPropertyValuePopulated {
        /// The property field that carried a bound value.
        property: DeclId,
    },
    /// A declaration-side field required by Phase 1 is structurally
    /// empty / default / placeholder.
    DeclarationFieldUnpopulated {
        /// The declaration whose field is unpopulated.
        decl_id: DeclId,
        /// Which field is unpopulated (e.g., `"body"` for a function
        /// with a placeholder body).
        field: &'static str,
    },
}

/// Origin metadata attached to every [`VerifierError`].
///
/// Parallels the `ErrorProvenance` struct in `regseq-driver`, but is
/// rustc-independent: instead of a `rustc::Span`, it carries the
/// [`SourceOrigin`] recorded in [`RegseqModel::source_locations`]. The
/// shape (`item_path` + origin) is stable so future diagnostics can
/// render driver-side and model-side errors uniformly.
///
/// [`RegseqModel::source_locations`]: crate::model::RegseqModel::source_locations
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VerifierProvenance {
    /// Best-effort item path (e.g., `"crate::drv::MyDriver"`). Empty
    /// string for model-level errors with no specific item scope.
    pub item_path: String,
    /// [`SourceOrigin`] looked up from
    /// [`RegseqModel::source_locations`] when a
    /// specific [`DeclId`] is the subject of the error, or
    /// [`SourceOrigin::CompilerGenerated`] when no item is in scope.
    ///
    /// [`RegseqModel::source_locations`]: crate::model::RegseqModel::source_locations
    pub origin: SourceOrigin,
}

impl VerifierProvenance {
    /// Construct provenance with no item scope.
    ///
    /// Used for model-level invariants like "exactly one Driver
    /// module" where no specific [`DeclId`] is the subject.
    pub fn model_level() -> Self {
        Self {
            item_path: String::new(),
            origin: SourceOrigin::CompilerGenerated,
        }
    }

    /// Construct provenance scoped to a single item.
    pub fn for_item(item_path: impl Into<String>, origin: SourceOrigin) -> Self {
        Self {
            item_path: item_path.into(),
            origin,
        }
    }
}

/// A single verifier violation.
///
/// Returned by [`verify_phase_1`] (and, from task 56 onward, by the
/// other phase/transform entry points). Fail-fast rules return the
/// first violation; collect-all variants are not provided yet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VerifierError {
    /// Which verifier level produced the error.
    pub level: VerifierLevel,
    /// The specific violation.
    pub kind: VerifierErrorKind,
    /// Origin metadata for diagnostics.
    pub provenance: VerifierProvenance,
    /// Human-readable description of the specific violation.
    pub details: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::DeclId;

    #[test]
    fn verifier_provenance_model_level_is_empty_and_generated() {
        let p = VerifierProvenance::model_level();
        assert!(p.item_path.is_empty());
        assert_eq!(p.origin, SourceOrigin::CompilerGenerated);
    }

    #[test]
    fn verifier_provenance_for_item_captures_path_and_origin() {
        let p =
            VerifierProvenance::for_item("crate::drv::MyDriver", SourceOrigin::CompilerGenerated);
        assert_eq!(p.item_path, "crate::drv::MyDriver");
        assert_eq!(p.origin, SourceOrigin::CompilerGenerated);
    }

    #[test]
    fn verifier_error_serializes_with_tagged_enums() {
        let e = VerifierError {
            level: VerifierLevel::Module,
            kind: VerifierErrorKind::MissingDriverModule,
            provenance: VerifierProvenance::model_level(),
            details: "no Driver module found in model".into(),
        };
        let json = serde_json::to_string(&e).unwrap();
        assert!(json.contains("\"level\""));
        assert!(json.contains("\"kind\""));
        assert!(json.contains("MissingDriverModule"));
    }

    #[test]
    fn verifier_error_kinds_are_distinguishable_in_equality() {
        let a = VerifierErrorKind::MissingDriverModule;
        let b = VerifierErrorKind::MultipleDriverModules { count: 2 };
        assert_ne!(a, b);

        let c = VerifierErrorKind::UnresolvedTypeDefId {
            referencing: DeclId::new(1),
            target: TypeDefId::from(DeclId::new(99)),
        };
        let d = VerifierErrorKind::UnresolvedTypeDefId {
            referencing: DeclId::new(1),
            target: TypeDefId::from(DeclId::new(99)),
        };
        assert_eq!(c, d);
    }
}
