//! Deferred-work-item-scheduling construct bridge.
//!
//! Recognizes the authored deferred-work surface — the typed `Work` resource
//! field (recognized BY TYPE via the `#[rseq::abstraction("deferred_work")]`
//! marker on its ADT item, NOT by a field attribute, GRILL D04), the bound
//! `#[rseq::work_handler] fn(&mut self)` callback, and the `self.work.submit()`
//! call-sites — and interns it into `RegseqModel.deferred_work` for the
//! generic-c backend's work lifecycle glue (`init_work`/`cancel_work`), wrapper
//! synthesis, and `submit` lowering to consume. There is no L2 binding (0 L1 /
//! 0 L2 properties, GRILL D02).
//!
//! Passes (mirroring the clock + interrupt constructs):
//!   * `materialise_deferred_work` (Phase 4, after
//!     `device_lifecycle::materialise_drivers`) — type-driven `Work`-field
//!     recognition + the unsupported-form reject.
//!   * `validate_deferred_work_signatures` (Phase 4, after `materialise_intrinsics`)
//!     — the `submit`-callee spoofing check (the op exists only on `Work`).
//!   * `resolve_work_handlers` (Phase 5, before the body drain) — handler
//!     discovery, `fn(&mut self) -> ()` validation, handler↔`Work` binding, the
//!     duplicate/zero/no-handler discipline, and force-materialization of the
//!     handler body.
//!   * `finalise_work_handlers` (post-drain) — the `AttrData::WorkHandler` marker.
//!   * `recognise_submit_sites` (post-drain) — the `self.work.submit()`
//!     direct-field-path recovery + per-`Work` submit-site recording.

mod recognition;
mod resolve;
mod submit;

pub(crate) use recognition::{
    is_work_field, materialise_deferred_work, validate_deferred_work_signatures,
};
pub(crate) use resolve::{finalise_work_handlers, resolve_work_handlers};
pub(crate) use submit::recognise_submit_sites;
