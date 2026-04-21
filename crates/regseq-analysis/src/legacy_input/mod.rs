//! Transitional backward-compat shim for the legacy `AnalysisInput`
//! surface.
//!
//! This module preserves the legacy [`AnalysisInput`] type and its
//! reviewer-facing companion functions (`check_readiness`,
//! `check_body_readiness`) only to keep the workspace compiling
//! between this task (`regseq-analysis-06`, which removes the old
//! `input.rs` and `body_readiness.rs` modules and rewrites
//! [`crate::readiness`] over `&RegseqModel` directly) and
//! `regseq-lowering-10`, which rewrites the lowering planner to take
//! `&RegseqModel` + a `VerifyReport` directly.
//!
//! **Retirement target: `regseq-lowering-10`.** Delete this entire
//! module (and remove all `pub use legacy_input::*` lines from
//! [`crate`]'s `lib.rs`) as part of that task. No production code
//! should link against `legacy_input` after that.
//!
//! Design notes:
//!
//! - The shim is a **standalone** storage type: it owns
//!   [`regseq_model::compat::DeclarationModel`],
//!   [`regseq_model::compat::JoinedArtifact`], and
//!   `Option<regseq_model::compat::BusBinding>` directly as fields.
//!   It does not depend on the new unified [`RegseqModel`](regseq_model::RegseqModel)
//!   machinery, so the two migration paths (shim consumers vs.
//!   `&RegseqModel` consumers) are independent.
//! - Every public item in this module is marked `#[deprecated]`. The
//!   shim silences its own internal deprecation warnings with
//!   `#![allow(deprecated)]`; downstream consumers will see
//!   deprecation notices until they migrate to
//!   [`crate::readiness::check_readiness`] against `&RegseqModel`.
//! - The shim scope is intentionally narrow: only the accessors and
//!   reviewer-facing helpers that the surviving callers
//!   (`regseq-backends` lowering planner / body interpreter,
//!   `regseq-driver::emit_driver`, a handful of E2E and backend
//!   tests) actually read. Everything else is gone.

#![allow(deprecated)]

mod body_shim;
mod input_shim;
mod readiness_shim;

pub use body_shim::{check_body_readiness, BodyFinding, BodyReadinessReport, MirFormSummary};
pub use input_shim::{AnalysisInput, ResourceJoinSummary};
pub use readiness_shim::{check_readiness, FindingSeverity, ReadinessFinding, ReadinessReport};
