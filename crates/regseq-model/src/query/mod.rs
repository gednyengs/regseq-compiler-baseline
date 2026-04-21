//! Deterministic read-only traversal helpers over a [`RegseqModel`].
//!
//! The [`query`](self) module replaces the pre-computed family maps that
//! the legacy `AnalysisInput` exposed. Every helper here takes a
//! `&RegseqModel` and walks the module tree + CFG bodies on demand.
//!
//! All helpers are pure: no caching, no mutation, no interior
//! mutability. Collection-returning functions produce deterministic
//! iteration order by sorting on a canonical key before returning.
//!
//! The module is organized by the question each helper answers:
//!
//! - [`call_sites`] -- find every `IntrinsicCall` terminator belonging
//!   to a given intrinsic family (register access, polling, logging,
//!   timed delay, etc.).
//! - [`reachability`] -- transitive call-graph closures rooted at
//!   caller-chosen seed functions, used for reviewer-facing helper
//!   inventories.
//! - [`topology`] -- driver-module-level summaries (resource field
//!   names, callback families present, bus binding identity).
//!
//! Query functions assume the input [`RegseqModel`] has passed
//! `verify_phase_1` or `verify_phase_2`; malformed models are a
//! verifier concern. A helper that encounters a state that the
//! verifier should have prevented may panic with a diagnostic rather
//! than silently returning an empty result.
//!
//! [`RegseqModel`]: crate::model::RegseqModel

pub mod call_sites;
pub mod reachability;
pub mod topology;

pub use call_sites::{
    completion_call_sites, duration_ctor_call_sites, lock_call_sites, log_call_sites,
    pm_gate_call_sites, poll_call_sites, register_access_sites, timed_delay_call_sites,
    wrapper_call_sites, CallSite,
};
pub use reachability::{reachable_helper_names, transitively_reachable_from};
pub use topology::{
    bus_binding, bus_identity, callback_families_present, callback_impls, resource_field_names,
    CallbackImpl,
};

pub use crate::model::resources::ResourceKind;
