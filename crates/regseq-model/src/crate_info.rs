//! Crate-level identity types.
//!
//! `StableCrateId` mirrors rustc's `rustc_span::def_id::StableCrateId`
//! as a transparent `u64` wrapper -- identity stable across builds.
//! `CrateInfo` is the per-crate entry stored in `RegseqModel.crates`,
//! holding the crate name (interned) plus its `StableCrateId`.
//!
//! Per Q1 (`architecture.md`) `regseq-model` has zero rustc deps;
//! the wrapper exists so the bridge can carry the value across the
//! `'tcx` boundary into the owned IR.

use serde::{Deserialize, Serialize};

use crate::ids::StringId;

/// Cross-build crate identity (rustc's `StableCrateId`).
///
/// Mirrored as a transparent `u64` wrapper so the JSON wire shape is
/// a bare integer. The bridge populates this from
/// `tcx.stable_crate_id(LOCAL_CRATE)` (and the same accessor for
/// upstream crates the model contains items from).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct StableCrateId(pub u64);

/// Per-crate entry in `RegseqModel.crates`.
///
/// `name` is the crate name interned through the model's
/// `StringInterner`. `stable_crate_id` is the cross-build identity.
/// Same name + different stable id means distinct crates; same
/// stable id means same crate (per construction invariant 5).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CrateInfo {
    pub name: StringId,
    pub stable_crate_id: StableCrateId,
}

#[cfg(test)]
#[path = "crate_info_tests.rs"]
mod tests;
