//! Pin-controller-and-GPIO-configuration construct bridge (pinctrl
//! state-selection).
//!
//! Recognizes the authored pinctrl surface and interns it into
//! `RegseqModel.pinctrl` for the L2 `pinctrl` tenant to join and the backends'
//! pinctrl lifecycle glue + per-op lowering to consume. Two item-driven passes:
//!
//!   * `materialise_pinctrls` (Phase 4, after `device_lifecycle::materialise_drivers`)
//!     — `#[rseq::pinctrl(id = "…")]` field recognition of BOTH the required
//!     `PinCtrl` and optional `Option<PinCtrl>` forms (build-wide, D02/D03), the
//!     mandatory-id check (R01/D07), the unsupported-form reject (`[PinCtrl; N]`/
//!     `&PinCtrl`/`Vec<PinCtrl>`, C06), and the duplicate-L1-id reject (C06).
//!
//!   * `materialise_pin_state_markers` (Phase 4) — walks every struct Item and
//!     records `AttrData::PinStateMarker{id}` for each `#[rseq::pin_state(id = "…")]`
//!     author-declared marker (D04). Mirrors `framework_type::materialise_framework_types`
//!     (item-driven, the attribute is the authority).
//!
//! The `PinCtrl`/`OptionalPinCtrl` `apply::<S>()` op is an author intrinsic
//! (tagged `#[rseq::intrinsic("pinctrl_apply")]`, recognized by the shared
//! intrinsic pass); its resolved state name is read at lowering from the call
//! site's generic arg `S` (→ the marker's `PinStateMarker.id`). The
//! compiler-managed `acquire_pinctrl`/`release_pinctrl` are synthesized from the
//! pinctrl *field* in the backend preamble, not author-called (D05/D08).
//!
//! M1 builds the EASY receiver shapes only (the `OptionalPinCtrl` direct-call on
//! `Option<PinCtrl>` + the required-form `&PinCtrl` call, D13); the harder plain
//! `if let Some(p) = &self.pinctrl` local-tracing is M3 (T15). The full C06
//! recognition negative set (non-ZST marker, invalid/duplicate state markers,
//! unresolved generic `S`, spoofed `apply` receiver) is T09/T10.

mod recognition;

pub(crate) use recognition::{
    is_pinctrl_field, materialise_pin_state_markers, materialise_pinctrls,
    validate_pinctrl_signatures,
};
