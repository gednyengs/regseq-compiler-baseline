//! Resource, property, and user-data field model types.
//!
//! Models the three categories of fields on a driver struct as defined
//! in spec.md Sections 4.2 (Resources), 4.3 (Properties), and 4.4
//! (User Data). Each resource kind has a paired declaration + optional
//! binding representation; bindings start as `None` after extraction
//! normalization and are populated at platform join time (Phase 6).

use serde::{Deserialize, Serialize};

use crate::identity::{FunctionId, PropertyId, RegisterBlockId, ResourceId, Symbol, UserDataId};
use crate::ir::{LiteralValue, RegseqType, ScalarType};

/// Shape of a resource field: `Single`, `Optional`, or `Array`.
///
/// Corresponds to `T`, `Option<T>`, and `[T; N]` in the spec source.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data")]
pub enum Shape {
    /// `T`
    Single,
    /// `Option<T>`
    Optional,
    /// `[T; N]`
    Array(usize),
}

/// Resource family discriminant.
///
/// The canonical home for the resource-family tag shared across the
/// model, query helpers, and backends. Consumers that previously
/// imported `regseq_model::semantic::ResourceKind` should migrate to
/// this path; the `semantic` module re-exports this type for
/// transitional compatibility until `semantic/` is deleted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum ResourceKind {
    /// MMIO mapping.
    Mmio,
    /// Interrupt line.
    Interrupt,
    /// Clock.
    Clock,
    /// Reset line.
    Reset,
    /// Pin controller state.
    PinCtrl,
    /// Firmware property read.
    Property,
}

// --- Clock ------------------------------------------------------------

/// Clock resource field on the driver struct.
#[derive(Clone, Eq, PartialEq, Debug, Serialize)]
pub struct ClockResource {
    /// The resource's declaration identifier.
    pub id: ResourceId,
    /// The struct field's identifier.
    pub field_name: Symbol,
    /// `T` / `Option<T>` / `[T; N]` shape.
    pub shape: Shape,
    /// Declaration-side properties from `#[rseq::clock(..)]`.
    pub decl: ClockDecl,
    /// Binding-side data. `None` until platform join.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub binding: Option<ClockBinding>,
}

/// Declaration-side Clock properties.
#[derive(Clone, Eq, PartialEq, Debug, Serialize)]
pub struct ClockDecl {
    /// Clock identifier (required).
    pub clock_id: Symbol,
}

/// Binding-side Clock data from `[clock.<id>]` in the platform file.
#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct ClockBinding {
    /// Default clock frequency in Hz.
    pub default_frequency: u64,
}

// --- Reset ------------------------------------------------------------

/// Reset-line resource field on the driver struct.
#[derive(Clone, Eq, PartialEq, Debug, Serialize)]
pub struct ResetResource {
    /// The resource's declaration identifier.
    pub id: ResourceId,
    /// The struct field's identifier.
    pub field_name: Symbol,
    /// `T` / `Option<T>` / `[T; N]` shape.
    pub shape: Shape,
    /// Declaration-side properties from `#[rseq::reset(..)]`.
    pub decl: ResetDecl,
    /// Binding-side data. `None` until platform join.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub binding: Option<ResetBinding>,
}

/// Declaration-side Reset properties.
#[derive(Clone, Eq, PartialEq, Debug, Serialize)]
pub struct ResetDecl {
    /// Reset line identifier (required).
    pub reset_id: Symbol,
}

/// Binding-side Reset data from `[reset.<id>]` in the platform file.
#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct ResetBinding {
    /// Reset line identifier string.
    pub reset_line_id: String,
    /// Whether the reset line is shared.
    pub reset_shared: bool,
}

// --- PinCtrl ---------------------------------------------------------

/// PinCtrl resource field on the driver struct.
#[derive(Clone, Eq, PartialEq, Debug, Serialize)]
pub struct PinCtrlResource {
    /// The resource's declaration identifier.
    pub id: ResourceId,
    /// The struct field's identifier.
    pub field_name: Symbol,
    /// `T` / `Option<T>` / `[T; N]` shape.
    pub shape: Shape,
    /// Declaration-side properties from `#[rseq::pinctrl(..)]`.
    pub decl: PinCtrlDecl,
    /// Binding-side data. `None` until platform join.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub binding: Option<PinCtrlBinding>,
}

/// Declaration-side PinCtrl properties.
#[derive(Clone, Eq, PartialEq, Debug, Serialize)]
pub struct PinCtrlDecl {
    /// Pinctrl identifier (required).
    pub pinctrl_id: Symbol,
}

/// Binding-side PinCtrl data from `[pinctrl.<id>]` in the platform file.
#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct PinCtrlBinding {
    /// List of pin state names (e.g., `["default", "sleep"]`).
    pub pin_states: Vec<String>,
}

// --- Interrupt --------------------------------------------------------

/// Interrupt resource field on the driver struct.
#[derive(Clone, Eq, PartialEq, Debug, Serialize)]
pub struct InterruptResource {
    /// The resource's declaration identifier.
    pub id: ResourceId,
    /// The struct field's identifier.
    pub field_name: Symbol,
    /// `T` / `Option<T>` / `[T; N]` shape.
    pub shape: Shape,
    /// Declaration-side properties from `#[rseq::interrupt(..)]`.
    pub decl: InterruptDecl,
    /// Binding-side data. `None` until platform join.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub binding: Option<InterruptBinding>,
}

/// Declaration-side Interrupt properties.
#[derive(Clone, Eq, PartialEq, Debug, Serialize)]
pub struct InterruptDecl {
    /// Interrupt identifier (required).
    pub interrupt_id: Symbol,
}

/// Binding-side Interrupt data from `[interrupt.<id>]` in the platform file.
///
/// `isr_target` carries the [`FunctionId`] of the ISR handler routed to
/// this interrupt resource. The slot is `Option` because not every
/// interrupt declaration has an `#[rseq::isr]`-annotated handler bound
/// to it -- e.g., a polled interrupt has no handler and the slot stays
/// `None`. Phase 2 (`populate_bindings`, task 59) is the populator;
/// the post-Phase-2 verifier (task 56) checks that any populated
/// `FunctionId` resolves to a declared, ISR-classified function (per
/// spec.md Section 13 line 1432).
#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct InterruptBinding {
    /// Platform-specific interrupt number (required).
    pub irq_number: u32,
    /// Trigger type (e.g., `level`, `edge`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trigger_type: Option<String>,
    /// Trigger polarity (e.g., `high`, `low`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trigger_polarity: Option<String>,
    /// Whether the interrupt is shared.
    pub shared: bool,
    /// ISR handler routed to this interrupt resource, if any.
    /// `None` for interrupts with no `#[rseq::isr]` handler bound.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub isr_target: Option<FunctionId>,
}

// --- Mmio -------------------------------------------------------------

/// Mmio resource field on the driver struct.
#[derive(Clone, Eq, PartialEq, Debug, Serialize)]
pub struct MmioResource {
    /// The resource's declaration identifier.
    pub id: ResourceId,
    /// The struct field's identifier.
    pub field_name: Symbol,
    /// `T` / `Option<T>` / `[T; N]` shape.
    pub shape: Shape,
    /// Declaration-side properties from `#[rseq::mmio(..)]`.
    pub decl: MmioDecl,
    /// Binding-side data. `None` until platform join.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub binding: Option<MmioBinding>,
}

/// Declaration-side Mmio properties.
#[derive(Clone, Eq, PartialEq, Debug, Serialize)]
pub struct MmioDecl {
    /// MMIO identifier (required).
    pub mmio_id: Symbol,
    /// Reference to the register block definition (from `Mmio<T>`
    /// type parameter).
    pub register_block: RegisterBlockId,
}

/// Binding-side Mmio data from `[mmio.<id>]` in the platform file.
#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct MmioBinding {
    /// Base physical address.
    pub base: u64,
    /// Region size in bytes.
    pub size: u64,
}

// --- Synchronization --------------------------------------------------

/// The kind of synchronization primitive.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data")]
pub enum SyncKind {
    /// `Mutex<T>` in the spec source.
    Mutex,
    /// `SpinLock<T>` in the spec source.
    SpinLock,
    /// `IrqSpinLock<T>` in the spec source.
    IrqSpinLock,
    /// `Completion` in the spec source (no payload).
    Completion,
}

/// Synchronization primitive field on the driver struct.
///
/// Synchronization primitives are purely spec-side entities with no
/// platform-specific configuration, so there is no binding data and no
/// shape field (they are always `Single`).
#[derive(Clone, Eq, PartialEq, Debug, Serialize)]
pub struct SyncResource {
    /// The resource's declaration identifier.
    pub id: ResourceId,
    /// The struct field's identifier.
    pub field_name: Symbol,
    /// The kind of synchronization primitive.
    pub sync_kind: SyncKind,
    /// The type of the protected data. `None` for `Completion`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payload_type: Option<RegseqType>,
}

// --- Property ---------------------------------------------------------

/// A firmware property field on the driver struct.
///
/// `required` is `true` if the spec-side field type is `T`, and `false`
/// if the field type is `Option<T>`. `bound_value` is `None` until
/// platform join populates it from the platform file's `[properties]`
/// section.
#[derive(Clone, Eq, PartialEq, Debug, Serialize)]
pub struct PropertyField {
    /// The property's declaration identifier.
    pub id: PropertyId,
    /// The struct field's identifier.
    pub field_name: Symbol,
    /// The property's type, constrained to [`ScalarType`].
    pub field_type: ScalarType,
    /// The firmware property key (required).
    pub property_id: Symbol,
    /// Optional default literal.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_value: Option<LiteralValue>,
    /// Optional vendor prefix string.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vendor_prefix: Option<Symbol>,
    /// `true` if the field type is `T`, `false` if `Option<T>`.
    pub required: bool,
    /// Concrete value from the platform file. `None` until join.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bound_value: Option<LiteralValue>,
}

// --- User Data --------------------------------------------------------

/// An author-defined state field not managed by the framework.
///
/// Two acceptance rules are supported:
/// - Rule 1 ([`UserDataKind::ZeroInit`]): a zero-init-friendly type
///   admitted by a compiler-controlled whitelist (scalars, arrays,
///   and `Copy` structs recursively composed of accepted fields).
/// - Rule 2 ([`UserDataKind::Option`]): `Option<T>` where `T: Copy`.
#[derive(Clone, Eq, PartialEq, Debug, Serialize)]
pub struct UserDataField {
    /// The field's declaration identifier.
    pub id: UserDataId,
    /// The struct field's identifier.
    pub field_name: Symbol,
    /// Discriminator naming which acceptance rule admitted the field
    /// plus the accepted inner type.
    pub kind: UserDataKind,
}

/// Two-rule discriminator for [`UserDataField`].
///
/// Each variant carries `inner_type`, the `RegseqType` of the accepted
/// value (for `Option`, it is the type `T` inside `Option<T>`; for
/// `ZeroInit`, it is the type of the bare value).
#[derive(Clone, Eq, PartialEq, Debug, Serialize)]
#[serde(tag = "kind", content = "data")]
pub enum UserDataKind {
    /// Rule 2: `Option<T: Copy>`. `inner_type` is the `RegseqType` of
    /// `T` inside the `Option`.
    Option {
        /// The type `T` of `Option<T>`. Must be `Copy`.
        inner_type: RegseqType,
    },
    /// Rule 1: zero-init-accepted type. `inner_type` is the
    /// `RegseqType` of the accepted bare value (no `Option` wrapping).
    ZeroInit {
        /// The bare value's type.
        inner_type: RegseqType,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::{DeclId, SymbolInterner};

    fn sym(interner: &mut SymbolInterner, s: &str) -> Symbol {
        interner.intern(s)
    }

    // -- Shape ----------------------------------------------------------

    #[test]
    fn all_three_shape_variants_including_array() {
        let _s = Shape::Single;
        let _o = Shape::Optional;
        let a = Shape::Array(4);
        match a {
            Shape::Array(n) => assert_eq!(n, 4),
            _ => panic!("expected Array"),
        }
    }

    #[test]
    fn shape_round_trips_through_json_for_every_variant() {
        for variant in [Shape::Single, Shape::Optional, Shape::Array(4)] {
            let json = serde_json::to_string(&variant).expect("serialize");
            let back: Shape = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(variant, back, "round-trip mismatch for {variant:?}");
        }
    }

    // -- Clock ----------------------------------------------------------

    #[test]
    fn constructs_clock_resource_without_and_with_binding() {
        let mut interner = SymbolInterner::new();
        let field = sym(&mut interner, "clk");
        let id = sym(&mut interner, "apb_clk");

        let mut r = ClockResource {
            id: ResourceId::from(DeclId::new(1)),
            field_name: field,
            shape: Shape::Single,
            decl: ClockDecl { clock_id: id },
            binding: None,
        };
        assert!(r.binding.is_none());

        r.binding = Some(ClockBinding {
            default_frequency: 24_000_000,
        });
        assert_eq!(r.binding.as_ref().unwrap().default_frequency, 24_000_000);

        let json = serde_json::to_string(r.binding.as_ref().unwrap()).unwrap();
        let back: ClockBinding = serde_json::from_str(&json).unwrap();
        assert_eq!(
            back,
            ClockBinding {
                default_frequency: 24_000_000,
            }
        );
    }

    // -- Reset ----------------------------------------------------------

    #[test]
    fn constructs_reset_resource_without_and_with_binding() {
        let mut interner = SymbolInterner::new();
        let field = sym(&mut interner, "rst");
        let id = sym(&mut interner, "apb_rst");

        let mut r = ResetResource {
            id: ResourceId::from(DeclId::new(2)),
            field_name: field,
            shape: Shape::Optional,
            decl: ResetDecl { reset_id: id },
            binding: None,
        };
        assert!(r.binding.is_none());

        let b = ResetBinding {
            reset_line_id: "apb-reset".into(),
            reset_shared: true,
        };
        r.binding = Some(b.clone());

        let json = serde_json::to_string(&b).unwrap();
        let back: ResetBinding = serde_json::from_str(&json).unwrap();
        assert_eq!(back, b);
    }

    // -- PinCtrl --------------------------------------------------------

    #[test]
    fn constructs_pinctrl_resource_without_and_with_binding() {
        let mut interner = SymbolInterner::new();
        let field = sym(&mut interner, "pctl");
        let id = sym(&mut interner, "serial_pins");

        let mut r = PinCtrlResource {
            id: ResourceId::from(DeclId::new(3)),
            field_name: field,
            shape: Shape::Array(2),
            decl: PinCtrlDecl { pinctrl_id: id },
            binding: None,
        };
        assert!(r.binding.is_none());

        let b = PinCtrlBinding {
            pin_states: vec!["default".into(), "sleep".into()],
        };
        r.binding = Some(b.clone());

        let json = serde_json::to_string(&b).unwrap();
        let back: PinCtrlBinding = serde_json::from_str(&json).unwrap();
        assert_eq!(back, b);
    }

    // -- Interrupt ------------------------------------------------------

    #[test]
    fn constructs_interrupt_resource_without_and_with_binding() {
        let mut interner = SymbolInterner::new();
        let field = sym(&mut interner, "irq");
        let id = sym(&mut interner, "uart_irq");

        let mut r = InterruptResource {
            id: ResourceId::from(DeclId::new(4)),
            field_name: field,
            shape: Shape::Single,
            decl: InterruptDecl { interrupt_id: id },
            binding: None,
        };
        assert!(r.binding.is_none());

        let b = InterruptBinding {
            irq_number: 37,
            trigger_type: Some("level".into()),
            trigger_polarity: Some("high".into()),
            shared: false,
            isr_target: None,
        };
        r.binding = Some(b.clone());

        let json = serde_json::to_string(&b).unwrap();
        let back: InterruptBinding = serde_json::from_str(&json).unwrap();
        assert_eq!(back, b);
    }

    // -- Mmio -----------------------------------------------------------

    #[test]
    fn constructs_mmio_resource_without_and_with_binding() {
        let mut interner = SymbolInterner::new();
        let field = sym(&mut interner, "regs");
        let id = sym(&mut interner, "uart_regs");
        let block = RegisterBlockId::from(DeclId::new(100));

        let mut r = MmioResource {
            id: ResourceId::from(DeclId::new(5)),
            field_name: field,
            shape: Shape::Single,
            decl: MmioDecl {
                mmio_id: id,
                register_block: block,
            },
            binding: None,
        };
        assert!(r.binding.is_none());

        let b = MmioBinding {
            base: 0x0900_0000,
            size: 0x1000,
        };
        r.binding = Some(b.clone());

        let json = serde_json::to_string(&b).unwrap();
        let back: MmioBinding = serde_json::from_str(&json).unwrap();
        assert_eq!(back, b);
    }

    // -- Synchronization ------------------------------------------------

    #[test]
    fn constructs_all_four_sync_kinds() {
        let mut interner = SymbolInterner::new();
        let field = sym(&mut interner, "lock");
        let payload = RegseqType::Scalar(ScalarType::U32);

        for kind in [
            SyncKind::Mutex,
            SyncKind::SpinLock,
            SyncKind::IrqSpinLock,
            SyncKind::Completion,
        ] {
            let payload_type = if kind == SyncKind::Completion {
                None
            } else {
                Some(payload.clone())
            };
            let r = SyncResource {
                id: ResourceId::from(DeclId::new(10)),
                field_name: field,
                sync_kind: kind,
                payload_type: payload_type.clone(),
            };
            if kind == SyncKind::Completion {
                assert!(r.payload_type.is_none());
            } else {
                assert_eq!(r.payload_type, Some(payload.clone()));
            }
        }
    }

    #[test]
    fn sync_kind_round_trips_through_json_for_every_variant() {
        for variant in [
            SyncKind::Mutex,
            SyncKind::SpinLock,
            SyncKind::IrqSpinLock,
            SyncKind::Completion,
        ] {
            let json = serde_json::to_string(&variant).expect("serialize");
            let back: SyncKind = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(variant, back, "round-trip mismatch for {variant:?}");
        }
    }

    // -- Property -------------------------------------------------------

    #[test]
    fn constructs_property_field_with_and_without_default_value() {
        let mut interner = SymbolInterner::new();
        let field = sym(&mut interner, "clock_frequency");
        let property_id = sym(&mut interner, "clock-frequency");

        let with_default = PropertyField {
            id: PropertyId::from(DeclId::new(20)),
            field_name: field,
            field_type: ScalarType::U32,
            property_id,
            default_value: Some(LiteralValue::U32(24_000_000)),
            vendor_prefix: None,
            required: false,
            bound_value: None,
        };
        assert_eq!(
            with_default.default_value,
            Some(LiteralValue::U32(24_000_000))
        );
        assert!(!with_default.required);
        assert!(with_default.bound_value.is_none());

        let without_default = PropertyField {
            id: PropertyId::from(DeclId::new(21)),
            field_name: field,
            field_type: ScalarType::U32,
            property_id,
            default_value: None,
            vendor_prefix: None,
            required: true,
            bound_value: None,
        };
        assert!(without_default.default_value.is_none());
        assert!(without_default.required);
    }

    // -- User Data ------------------------------------------------------

    #[test]
    fn constructs_user_data_field() {
        let mut interner = SymbolInterner::new();
        let field = sym(&mut interner, "frame_count");

        let u = UserDataField {
            id: UserDataId::from(DeclId::new(30)),
            field_name: field,
            kind: UserDataKind::ZeroInit {
                inner_type: RegseqType::Scalar(ScalarType::U32),
            },
        };
        match &u.kind {
            UserDataKind::ZeroInit { inner_type } => {
                assert_eq!(*inner_type, RegseqType::Scalar(ScalarType::U32));
            }
            other => panic!("expected ZeroInit, got {other:?}"),
        }
    }
}
