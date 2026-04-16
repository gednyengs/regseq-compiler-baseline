//! Intrinsic and abstraction identifier enums and structured metadata
//! carried by `IntrinsicCall` terminators.
//!
//! [`RegseqIntrinsic`] identifies domain-specific operations (register
//! access, clock control, logging, etc.). [`RegseqAbstraction`] identifies
//! the abstraction kind a type belongs to. [`IntrinsicMetadata`] carries
//! structured data that backends need beyond the call arguments.

use serde::{Deserialize, Serialize};

use crate::identity::{RegisterBlockId, Symbol};

/// Identifies a domain-specific intrinsic operation.
///
/// Each variant maps to a specific `#[rseq::intrinsic("...")]` attribute
/// on a method. These operations flow through `IntrinsicCall` terminators,
/// not rvalues.
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data")]
pub enum RegseqIntrinsic {
    /// Read a full register value.
    RegisterRead,
    /// Read a single field from a register.
    RegisterReadField,
    /// Write a full register value.
    RegisterWrite,
    /// Write a single field of a register.
    RegisterWriteField,
    /// Enable a clock.
    ClockEnable,
    /// Disable a clock.
    ClockDisable,
    /// Get the rate of a clock.
    ClockGetRate,
    /// Assert a reset line.
    ResetAssert,
    /// Deassert a reset line.
    ResetDeassert,
    /// Toggle a reset line (assert then deassert).
    ResetToggle,
    /// Apply a pin control state.
    PinCtrlApply,
    /// Busy-wait (spin) for a duration.
    BusyWait,
    /// Sleep for a duration.
    Sleep,
    /// Log an error message.
    LogError,
    /// Log a warning message.
    LogWarn,
    /// Log an informational message.
    LogInfo,
    /// Log a debug message.
    LogDebug,
    /// Poll a condition until it becomes true or times out.
    PollUntil,
}

/// Identifies the abstraction kind a type belongs to.
///
/// Used during IR construction to classify framework types and during
/// analysis to reason about resource usage patterns.
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data")]
pub enum RegseqAbstraction {
    /// Clock resource.
    Clock,
    /// Reset line resource.
    Reset,
    /// Pin control resource.
    PinCtrl,
    /// Interrupt resource.
    Interrupt,
    /// Memory-mapped I/O register access.
    Mmio,
    /// Mutex synchronization primitive.
    Mutex,
    /// Spinlock synchronization primitive.
    SpinLock,
    /// IRQ-safe spinlock synchronization primitive.
    IrqSpinLock,
    /// Completion synchronization primitive.
    Completion,
}

/// Structured metadata carried by an `IntrinsicCall` terminator.
///
/// Provides backend-specific information beyond the call arguments
/// themselves. Not all intrinsic calls carry metadata.
#[derive(Clone, Eq, PartialEq, Debug)]
pub enum IntrinsicMetadata {
    /// Metadata for logging intrinsics.
    Log {
        /// The format template string (e.g., `"baud rate: {}"`).
        template: String,
        /// Placeholders in the template, in order.
        placeholders: Vec<FormatPlaceholder>,
    },
    /// Metadata for register access intrinsics.
    Register {
        /// The register block this access targets.
        register_block: RegisterBlockId,
        /// Byte offset of the register within the block.
        register_offset: u32,
        /// Access width in bits (8, 16, 32, or 64).
        access_width: u32,
        /// Field-level metadata, if this is a field access.
        field: Option<RegisterFieldMeta>,
    },
    /// Metadata for pin control intrinsics.
    PinCtrl {
        /// The pin control state identifier.
        state_id: Symbol,
    },
}

/// A single format placeholder in a log template.
#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct FormatPlaceholder {
    /// The formatting kind for this placeholder.
    pub format: FormatKind,
}

/// The formatting kind applied to a log placeholder.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data")]
pub enum FormatKind {
    /// Default display formatting (`{}`).
    Display,
    /// Lowercase hex (`{:x}`).
    LowerHex,
    /// Uppercase hex (`{:X}`).
    UpperHex,
    /// Lowercase hex with `0x` prefix (`{:#x}`).
    LowerHexPrefixed,
    /// Uppercase hex with `0x` prefix (`{:#X}`).
    UpperHexPrefixed,
    /// Padded hex with configurable prefix, case, and width.
    PaddedHex {
        /// Whether to include the `0x` prefix.
        prefix: bool,
        /// Whether to use uppercase hex digits.
        uppercase: bool,
        /// Minimum field width in characters.
        width: u8,
    },
    /// Pointer formatting (`{:p}`).
    Pointer,
}

/// Metadata for a register field access.
#[derive(Clone, Eq, PartialEq, Debug)]
pub struct RegisterFieldMeta {
    /// Bitmask for the field within the register.
    pub mask: u64,
    /// Bit position of the field's LSB.
    pub shift: u32,
    /// Field name.
    pub name: Symbol,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::{DeclId, SymbolInterner};

    // -- Construction: each of the 18 RegseqIntrinsic variants --

    #[test]
    fn construct_all_regseq_intrinsic_variants() {
        let variants = [
            RegseqIntrinsic::RegisterRead,
            RegseqIntrinsic::RegisterReadField,
            RegseqIntrinsic::RegisterWrite,
            RegseqIntrinsic::RegisterWriteField,
            RegseqIntrinsic::ClockEnable,
            RegseqIntrinsic::ClockDisable,
            RegseqIntrinsic::ClockGetRate,
            RegseqIntrinsic::ResetAssert,
            RegseqIntrinsic::ResetDeassert,
            RegseqIntrinsic::ResetToggle,
            RegseqIntrinsic::PinCtrlApply,
            RegseqIntrinsic::BusyWait,
            RegseqIntrinsic::Sleep,
            RegseqIntrinsic::LogError,
            RegseqIntrinsic::LogWarn,
            RegseqIntrinsic::LogInfo,
            RegseqIntrinsic::LogDebug,
            RegseqIntrinsic::PollUntil,
        ];
        assert_eq!(variants.len(), 18);
    }

    // -- Construction: each of the 9 RegseqAbstraction variants --

    #[test]
    fn construct_all_regseq_abstraction_variants() {
        let variants = [
            RegseqAbstraction::Clock,
            RegseqAbstraction::Reset,
            RegseqAbstraction::PinCtrl,
            RegseqAbstraction::Interrupt,
            RegseqAbstraction::Mmio,
            RegseqAbstraction::Mutex,
            RegseqAbstraction::SpinLock,
            RegseqAbstraction::IrqSpinLock,
            RegseqAbstraction::Completion,
        ];
        assert_eq!(variants.len(), 9);
    }

    // -- Construction: each IntrinsicMetadata variant --

    #[test]
    fn construct_intrinsic_metadata_log() {
        let _ = IntrinsicMetadata::Log {
            template: "baud rate: {}".to_string(),
            placeholders: vec![FormatPlaceholder {
                format: FormatKind::Display,
            }],
        };
    }

    #[test]
    fn construct_intrinsic_metadata_register() {
        let _ = IntrinsicMetadata::Register {
            register_block: RegisterBlockId::from(DeclId::new(0)),
            register_offset: 0x0C,
            access_width: 32,
            field: None,
        };
    }

    #[test]
    fn construct_intrinsic_metadata_register_with_field() {
        let mut interner = SymbolInterner::new();
        let name = interner.intern("baud_divint");

        let _ = IntrinsicMetadata::Register {
            register_block: RegisterBlockId::from(DeclId::new(1)),
            register_offset: 0x24,
            access_width: 32,
            field: Some(RegisterFieldMeta {
                mask: 0xFFFF,
                shift: 0,
                name,
            }),
        };
    }

    #[test]
    fn construct_intrinsic_metadata_pinctrl() {
        let mut interner = SymbolInterner::new();
        let state_id = interner.intern("default");

        let _ = IntrinsicMetadata::PinCtrl { state_id };
    }

    // -- Construction: each FormatKind variant --

    #[test]
    fn construct_all_format_kind_variants() {
        let _ = FormatKind::Display;
        let _ = FormatKind::LowerHex;
        let _ = FormatKind::UpperHex;
        let _ = FormatKind::LowerHexPrefixed;
        let _ = FormatKind::UpperHexPrefixed;
        let _ = FormatKind::PaddedHex {
            prefix: true,
            uppercase: false,
            width: 8,
        };
        let _ = FormatKind::Pointer;
    }

    // -- Construction: RegisterFieldMeta --

    #[test]
    fn construct_register_field_meta() {
        let mut interner = SymbolInterner::new();
        let name = interner.intern("enable");

        let meta = RegisterFieldMeta {
            mask: 0x01,
            shift: 0,
            name,
        };
        assert_eq!(meta.mask, 0x01);
        assert_eq!(meta.shift, 0);
    }
}
