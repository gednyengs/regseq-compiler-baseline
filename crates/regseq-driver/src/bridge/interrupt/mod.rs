//! Device-interrupt construct bridge (construct #6).
//!
//! Recognizes the authored interrupt surface — `#[rseq::interrupt(id = "…")]`
//! fields of the typed `Interrupt` resource shell (all three declaration
//! forms) and the `#[rseq::isr]` bound handler returning `IrqResult` — and
//! interns it into `RegseqModel.interrupt` for the backends' interrupt
//! lifecycle glue to consume.
//!
//! Three passes:
//!
//!   * `materialise_interrupts` (Phase 4, after
//!     `device_lifecycle::materialise_drivers`) — field recognition + the
//!     mandatory-id check (R01/D08).
//!   * `resolve_isrs` (Phase 5, after `device_lifecycle::resolve_drivers`,
//!     before the body drain) — ISR discovery, C04 signature validation,
//!     handler↔resource binding (bare exactly-one / explicit `field`), D13
//!     handler-count discipline, and force-materialization of handler bodies.
//!   * `finalise_isrs` (post-drain, after the late-item barriers) — the
//!     `AttrData::IsrBinding` marker on final `ItemId`s + the `IrqResult`
//!     arm-reachability facts from the materialized bodies.
//!
//! The `Interrupt` shell stays methodless from the driver's perspective:
//! recognition introduces no driver-callable line-control surface (I02), and
//! the ISR body's device-source operations remain ordinary Register Access
//! MMIO traffic (I03).

mod recognition;
mod resolve;

pub(crate) use recognition::{is_interrupt_field, materialise_interrupts};
pub(crate) use resolve::{finalise_isrs, resolve_isrs};
