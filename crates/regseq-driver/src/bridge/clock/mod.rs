//! Peripheral-clock-management construct bridge.
//!
//! Recognizes the authored clock surface — `#[rseq::clock(id = "…")]` fields of
//! the typed `Clock` resource shell (the required form; `Option<Clock>` /
//! `[Clock; N]` are recognized-and-rejected, C06 N2/N14) and the author
//! `clock_*` call-sites (recognized generically by `bridge::intrinsic`) — and
//! interns it into `RegseqModel.clock` for the L2 `clock` tenant to join and the
//! backends' clock lifecycle glue + per-op lowering to consume.
//!
//! One pass:
//!
//!   * `materialise_clocks` (Phase 4, after
//!     `device_lifecycle::materialise_drivers`) — field recognition + the
//!     mandatory-id check (R01/D13), the unsupported-form reject (C06 N2/N14),
//!     and the duplicate-L1-id reject (C06 N15).
//!
//! The `Clock` shell's `enable`/`disable`/`get_rate` are author intrinsics
//! (tagged `#[rseq::intrinsic("clock_*")]`, recognized by the shared intrinsic
//! pass); the compiler-managed `acquire_clock`/`release` are synthesized from
//! the clock *field* in the device-lifecycle preamble, not author-called (D05).

mod recognition;

pub(crate) use recognition::{is_clock_field, materialise_clocks, validate_clock_signatures};
