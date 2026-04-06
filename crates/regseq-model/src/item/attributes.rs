//! Item-level and field-level attribute metadata.
//!
//! Attributes are derived from HIR `#[rseq::*]` attributes during extraction,
//! not from pattern-matching type name strings. Each variant corresponds to a
//! specific attribute kind with typed payload.

use serde::{Deserialize, Serialize};

/// Struct-level attribute metadata.
///
/// Captures semantic classification from HIR attributes on struct items.
/// Extensible for future attribute kinds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum ItemAttribute {
    /// Struct is the driver struct (`#[rseq::driver]`).
    Driver,
    /// Struct is a register block (`#[rseq::register_block]`).
    RegisterBlock,
    /// Struct has `#[repr(C)]`.
    ReprC,
    /// Other attribute preserved for forward compatibility.
    Other { name: String },
}

/// Field-level attribute metadata.
///
/// Captures resource classification from HIR attributes on struct fields.
/// Mirrors the typed resource classification in
/// `declaration::raw_input::RawRseqFieldKindInput`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum FieldAttribute {
    /// Field carries `#[rseq::mmio(...)]`.
    Mmio { id: String },
    /// Field carries `#[rseq::clock(...)]`.
    Clock { id: String },
    /// Field carries `#[rseq::pinctrl(...)]`.
    PinCtrl { id: String },
    /// Field carries `#[rseq::interrupt(...)]`.
    Interrupt { id: String },
    /// Field carries `#[rseq::property(...)]`.
    Property { id: String },
    /// Register side-effects annotation.
    SideEffects { value: String },
    /// Register hardware-updates annotation.
    HwUpdates { value: String },
    /// Other attribute.
    Other { name: String },
}
