//! Synchronization (locks) construct bridge recognition.
//!
//! Two passes, mirroring `bridge::interrupt`:
//!
//! - **Phase-4 field recognition** ([`recognition::materialise_synchronization`]):
//!   classify driver-struct lock fields into [`regseq_model::SyncResource`]
//!   entries + [`regseq_model::AttrData::SyncResource`] markers. Runs in
//!   `bridge::assign`, after `interrupt::materialise_interrupts`.
//!
//! - **Phase-5 op / acquire-site recognition** ([`acquire::recognise_acquire_sites`]):
//!   walk the materialized bodies, recognize each `lock()` acquire (anchored on
//!   the receiver field's `SyncResource` marker) and its paired guard release
//!   (the guard's drop-glue `Call`), and record the per-site facts into the
//!   `SynchronizationTables` sibling table. Runs in
//!   `construct_instances_and_resolve_cross_references`, after
//!   `interrupt::finalise_isrs` and the item sort — the same point as
//!   `timed_delay::validate_isr_no_sleep`.
//!
//! **No new `IntrinsicKind`** — sync ops are wrapper methods, not intrinsics
//! (GRILL `C04` / design §6).

pub(crate) mod acquire;
pub(crate) mod context;
pub(crate) mod recognition;

pub(crate) use acquire::recognise_acquire_sites;
pub(crate) use context::analyse_sync_contexts;
pub(crate) use recognition::{is_sync_field, materialise_synchronization};
