//! Phase-6 Phase-2 platform join.
//!
//! [`populate_bindings`] is the Phase-2 entry point: it takes a
//! post-Phase-1 [`RegseqModel`] (every binding slot [`None`]) and a
//! parsed [`PlatformFile`], and populates every matched binding slot
//! in place. The mutation routes through the [`MutationGuard`] from
//! task 57, so `verify_phase_2` is auto-invoked on commit.
//!
//! # Error model
//!
//! The join produces a [`JoinError`] carrying a [`JoinErrorKind`] and
//! always-populated [`JoinProvenance`]. Three kinds of errors are
//! emitted directly by the section-level joiners:
//!
//! - [`JoinErrorKind::UnknownPlatformSection`] -- a platform section
//!   whose `id` does not match any resource declaration. Raised at
//!   the point of section iteration so the platform-section
//!   provenance is precise (spec.md Section 11 line 1330).
//! - [`JoinErrorKind::PropertyTypeMismatch`] -- a `[properties]`
//!   entry whose value kind does not match the declared
//!   [`ScalarType`] of the property (brief lines 104-105).
//! - [`JoinErrorKind::Phase2VerificationFailed`] -- the post-commit
//!   verifier rejected the joined model. Wraps the underlying
//!   [`VerifierError`].
//!
//! [`JoinErrorKind::RequiredResourceNotBound`] is an enum variant
//! that mirrors spec.md Section 11's "missing required binding"
//! rule, but the actual detection folds into `verify_phase_2`
//! (`RequiredBindingMissing`) at commit time. Keeping the variant
//! in the public enum documents the rule set without adding a
//! second detection path.
//!
//! # Determinism
//!
//! Phase 2 is deterministic: the same `(RegseqModel, PlatformFile)`
//! pair produces a byte-identical serialization after the join. The
//! [`determinism`] submodule centralizes the sort-before-serialize
//! rules: platform sections iterate by `(kind, id)` lexicographic
//! order, `[properties]` entries iterate by `property_id` order, and
//! every in-model collection introduced by this module is a
//! [`BTreeMap`]/[`BTreeSet`].
//!
//! [`BTreeMap`]: std::collections::BTreeMap
//! [`BTreeSet`]: std::collections::BTreeSet
//! [`MutationGuard`]: crate::model::MutationGuard
//! [`None`]: Option::None
//! [`PlatformFile`]: crate::platform::PlatformFile
//! [`RegseqModel`]: crate::model::RegseqModel
//! [`ScalarType`]: crate::ir::ScalarType
//! [`VerifierError`]: crate::verify::VerifierError

// `JoinError` wraps a `VerifierError` which is itself a first-class
// value. The task spec prescribes the exact `Result<(), JoinError>`
// return shape so downstream consumers can match on `.kind` and
// `.provenance` directly. Boxing the `Err` payload would force every
// caller to dereference before matching. Mirrors the matching allow
// on the `verify` and `normalize` modules.
#![allow(clippy::result_large_err)]

pub mod determinism;
pub mod sections;

use serde::Serialize;
use std::collections::BTreeMap;

use crate::identity::Symbol;
use crate::ir::ScalarType;
use crate::model::driver::DriverStruct;
use crate::model::module::{Module, ModuleKind};
use crate::model::mutation::MutationPhase;
use crate::model::regseq_model::RegseqModel;
use crate::platform::PlatformFile;
use crate::verify::VerifierError;

/// Post-Phase-2 join result.
pub type JoinResult = Result<(), JoinError>;

/// A single Phase-2 platform-join violation.
///
/// Every variant carries a [`JoinProvenance`] whose `platform_section`
/// is always non-empty -- either the TOML section header that caused
/// the error (e.g., `"[clock.bogus_clk]"`), the offending property
/// key (e.g., `"properties.clock-frequency"`), or the sentinel
/// `"<post-join verify>"` for the commit-time verifier wrapper.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct JoinError {
    /// The specific violation.
    pub kind: JoinErrorKind,
    /// Origin metadata for diagnostics.
    pub provenance: JoinProvenance,
}

/// The specific violation detected by a join step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", content = "data")]
pub enum JoinErrorKind {
    // ----- From spec.md Section 11 lines 1330-1335 -----
    /// A platform section references an `id` that is not declared by
    /// any resource of the given kind. Raised at the join step with
    /// full platform-section provenance (see module-level rustdoc).
    UnknownPlatformSection {
        /// The resource-kind namespace of the offending section
        /// (`"clock"`, `"reset"`, `"pinctrl"`, `"interrupt"`,
        /// `"mmio"`, or `"property"`).
        kind: String,
        /// The unresolved id from the platform file.
        id: String,
    },
    /// A required resource declaration has no matching platform
    /// section. Documented for completeness against spec.md Section
    /// 11's "missing required binding" rule; detection folds into
    /// [`JoinErrorKind::Phase2VerificationFailed`] at commit time
    /// (verifier's `RequiredBindingMissing`).
    RequiredResourceNotBound {
        /// The resource-kind namespace of the missing binding.
        kind: String,
        /// The declared resource id that has no platform section.
        id: String,
    },

    // ----- From brief line 104-105 -----
    /// A `[properties]` entry's value kind does not match the
    /// declared [`ScalarType`](crate::ir::ScalarType) of the
    /// property.
    PropertyTypeMismatch {
        /// The property id string.
        property_id: String,
        /// Declared scalar type as a textual tag (e.g., `"u32"`,
        /// `"i64"`).
        declared: String,
        /// Platform-supplied value kind tag (e.g., `"integer"`,
        /// `"string"`, `"boolean"`, `"list"`, `"table"`).
        provided: String,
    },

    // ----- From Phase-2 post-condition -----
    /// The post-commit verifier rejected the joined model. Wraps
    /// the underlying [`VerifierError`](crate::verify::VerifierError).
    Phase2VerificationFailed {
        /// The verifier violation that caused the rejection.
        inner: VerifierError,
    },
}

/// Origin metadata attached to every [`JoinError`].
///
/// `platform_section` is always non-empty -- the join step stamps
/// the responsible TOML section header (e.g., `"[clock.uart_clk]"`)
/// or the sentinel `"<post-join verify>"` on the commit-time
/// verifier wrapper. `resource_id` is the declared resource id
/// referenced by that section when applicable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct JoinProvenance {
    /// The platform-file section header that caused the error.
    pub platform_section: String,
    /// The declared resource id referenced, if applicable.
    pub resource_id: Option<String>,
}

/// Join a parsed platform file into a Phase-1 [`RegseqModel`].
///
/// The mutation happens in place on `model`: declared resources are
/// ID-matched to the corresponding platform sections, `bound_value`
/// slots are populated from `[properties]`, and platform/bus
/// metadata is recorded on the driver. On success, `verify_phase_2`
/// runs via [`MutationGuard::commit`](crate::model::MutationGuard::commit);
/// on failure the guard rolls the model back to its pre-join state.
///
/// # Error model
///
/// See [`JoinError`] for the error shape. Unknown platform sections
/// are rejected at join time with full section provenance; required
/// resources left unbound and required property values left unset
/// surface as [`JoinErrorKind::Phase2VerificationFailed`] wrapping
/// the verifier's `RequiredBindingMissing` /
/// `RequiredPropertyValueMissing`.
pub fn populate_bindings(model: &mut RegseqModel, platform: &PlatformFile) -> JoinResult {
    let index = DeclIndex::from_model(model);

    let mut guard = model.begin_mutation(MutationPhase::Phase2);

    sections::join_platform_bus(&mut guard, &platform.platform, platform.bus.as_ref())?;
    sections::join_construct_sections(&mut guard, &platform.construct_sections, &index)?;
    sections::join_properties(&mut guard, &platform.properties, &index)?;

    // `commit` now returns `Ok(VerifyReport)` on success, with any
    // Phase-2 lint findings attached. Platform join has no
    // diagnostic surface of its own, so the report is dropped here;
    // `regseq-analysis` consumes the verifier's lint channel directly
    // in task 6.
    guard.commit().map(|_report| ()).map_err(|inner| JoinError {
        kind: JoinErrorKind::Phase2VerificationFailed { inner },
        provenance: JoinProvenance {
            platform_section: String::from("<post-join verify>"),
            resource_id: None,
        },
    })
}

/// Pre-computed lookup of declared resource ids to their position
/// within the driver's resource vectors.
///
/// Built up front from the live [`RegseqModel`] (before the mutation
/// guard takes the exclusive borrow) so the section joiners can
/// resolve string ids without needing concurrent access to the
/// [`SymbolInterner`](crate::identity::SymbolInterner).
///
/// Every map is a [`BTreeMap`] so iteration order is deterministic.
#[derive(Debug, Clone, Default)]
pub(crate) struct DeclIndex {
    /// `clock_id` string -> index into `driver.clocks`.
    pub(crate) clocks: BTreeMap<String, usize>,
    /// `reset_id` string -> index into `driver.resets`.
    pub(crate) resets: BTreeMap<String, usize>,
    /// `pinctrl_id` string -> index into `driver.pinctrl`.
    pub(crate) pinctrl: BTreeMap<String, usize>,
    /// `interrupt_id` string -> index into `driver.interrupts`.
    pub(crate) interrupts: BTreeMap<String, usize>,
    /// `mmio_id` string -> index into `driver.mmio`.
    pub(crate) mmio: BTreeMap<String, usize>,
    /// `property_id` string -> (index into `driver.properties`,
    /// declared scalar type).
    pub(crate) properties: BTreeMap<String, PropertyEntry>,
}

/// Pre-resolved property declaration metadata.
#[derive(Debug, Clone, Copy)]
pub(crate) struct PropertyEntry {
    /// Index into `driver.properties`.
    pub(crate) index: usize,
    /// Declared scalar type -- used for [`PropertyTypeMismatch`]
    /// detection.
    ///
    /// [`PropertyTypeMismatch`]: JoinErrorKind::PropertyTypeMismatch
    pub(crate) field_type: ScalarType,
}

impl DeclIndex {
    /// Build the lookup from the driver struct currently in `model`.
    ///
    /// An empty index (every map empty) is returned if the model has
    /// no driver submodule; the subsequent section joiners will each
    /// early-return on an empty index (no platform section can match).
    pub(crate) fn from_model(model: &RegseqModel) -> Self {
        let Some(driver) = find_driver(&model.root_module) else {
            return Self::default();
        };
        let symbols = &model.symbols;

        let mut index = Self::default();
        for (i, r) in driver.clocks.iter().enumerate() {
            index
                .clocks
                .insert(resolve_string(symbols, r.decl.clock_id), i);
        }
        for (i, r) in driver.resets.iter().enumerate() {
            index
                .resets
                .insert(resolve_string(symbols, r.decl.reset_id), i);
        }
        for (i, r) in driver.pinctrl.iter().enumerate() {
            index
                .pinctrl
                .insert(resolve_string(symbols, r.decl.pinctrl_id), i);
        }
        for (i, r) in driver.interrupts.iter().enumerate() {
            index
                .interrupts
                .insert(resolve_string(symbols, r.decl.interrupt_id), i);
        }
        for (i, r) in driver.mmio.iter().enumerate() {
            index
                .mmio
                .insert(resolve_string(symbols, r.decl.mmio_id), i);
        }
        for (i, p) in driver.properties.iter().enumerate() {
            index.properties.insert(
                resolve_string(symbols, p.property_id),
                PropertyEntry {
                    index: i,
                    field_type: p.field_type,
                },
            );
        }
        index
    }
}

/// Walk the module tree and return the first [`DriverStruct`] found
/// in document order.
pub(crate) fn find_driver(root: &Module) -> Option<&DriverStruct> {
    if root.kind == ModuleKind::Driver {
        if let Some(d) = root.driver.as_ref() {
            return Some(d);
        }
    }
    for sub in &root.submodules {
        if let Some(d) = find_driver(sub) {
            return Some(d);
        }
    }
    None
}

fn resolve_string(symbols: &crate::identity::SymbolInterner, sym: Symbol) -> String {
    symbols.resolve(sym).to_string()
}
