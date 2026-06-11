//! Device-interrupt construct IR (construct #6).
//!
//! The author declares a typed `Interrupt` field on the `#[rseq::driver]`
//! struct via `#[rseq::interrupt(id = "…")]` and binds a handler method via
//! `#[rseq::isr]` returning `IrqResult`. This module is the owned/serializable
//! model of that surface: per-resource facts (`id` join key, declaration form,
//! field name), the L2-resolved `[interrupt.<id>]` binding, and per-handler
//! facts (bound resource, array-bind marker, `IrqResult` arm reachability).
//! It is populated by the `regseq-driver` bridge's `interrupt` recognition,
//! joined by the `regseq-l2` `interrupt` tenant, and consumed by the backends'
//! interrupt lifecycle glue.
//!
//! Handler bodies are stored as `InstanceRef` (content-addressed), NOT raw
//! `InstanceId`, so they survive the Phase-4 (`F4-2`) instance sort — the same
//! discipline as `device_lifecycle::LifecycleOps`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::ids::{ItemId, StringId};
use crate::instance_ref::InstanceRef;

/// Sibling table on `RegseqModel` for recognized interrupt surfaces, keyed by
/// the `#[rseq::driver]` struct's final `ItemId` (assigned at Phase 4, like
/// `device_lifecycle`). Sparse via map absence. `#[serde(default)]` on the
/// model field keeps pre-construct-#6 models deserializable.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterruptTables {
    /// Recognized per-driver interrupt surfaces, keyed by the driver-struct Item.
    #[serde(with = "ordered_interrupt_map")]
    pub drivers: BTreeMap<ItemId, DriverInterrupts>,
}

/// One driver's interrupt surface: its resources (declaration order) and the
/// handlers bound to them.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriverInterrupts {
    /// `#[rseq::interrupt(id = "…")]` resource fields, declaration order
    /// (the interrupt preamble runs after all other resource preambles; order
    /// among multiple interrupt resources is declaration order).
    pub resources: Vec<InterruptResource>,
    /// `#[rseq::isr]` handlers, declaration order.
    pub handlers: Vec<IsrHandler>,
}

/// One recognized `Interrupt` resource field on the driver struct.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterruptResource {
    /// The struct field name.
    pub name: StringId,
    /// The mandatory L1 resource id (`#[rseq::interrupt(id = "…")]`): the join
    /// key to the platform file's `[interrupt.<id>]` section. Always present —
    /// a missing `id` is a frontend error (no field-name fallback; intended
    /// taxonomy tightening, GRILL D08).
    pub id: StringId,
    /// The declaration form (`Interrupt` / `Option<Interrupt>` /
    /// `[Interrupt; N]`). All three are recognized whole; non-`Single` forms
    /// are emit-stage rejected this slice (GRILL D06).
    pub form: InterruptForm,
    /// The L2-resolved `[interrupt.<id>]` binding. Empty
    /// (`InterruptBinding::default`) until the platform-file join; the
    /// `interrupt` L2 tenant validates canonical spellings and fills it.
    /// `#[serde(default)]` keeps earlier serialized models deserializable.
    #[serde(default)]
    pub binding: InterruptBinding,
}

/// The declaration form of an `Interrupt` resource field. Closed, append-only.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum InterruptForm {
    /// `Interrupt` — exactly one line, handler mandatory (GRILL D13).
    Single = 0,
    /// `Option<Interrupt>` — line may be absent on the platform (GRILL D06).
    Optional = 1,
    /// `[Interrupt; N]` — N lines, array-bound handler (GRILL D06).
    Array { len: u32 } = 2,
}

/// The L2-resolved `[interrupt.<id>]` facts (contract C03). The tenant parses
/// canonical spellings only (`level|edge`; `active_high|active_low|rising|
/// falling` — GRILL D10/R12) into the typed enums below; trigger facts are
/// validation/trace inputs, never flag emission (D10).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterruptBinding {
    /// The platform line number. Required for generic-c/static targets,
    /// advisory for zephyr (`DT_INST_IRQN` is authoritative), unused for the
    /// linux runtime resolution (R10). `None` until joined.
    pub irq_number: Option<u32>,
    /// `trigger_type` (`level` | `edge`); `None` if unspecified.
    pub trigger_type: Option<TriggerType>,
    /// `trigger_polarity` (`active_high` | `active_low` | `rising` |
    /// `falling`); `None` if unspecified.
    pub trigger_polarity: Option<TriggerPolarity>,
    /// Scheduling priority (zephyr `IRQ_CONNECT` argument only, R12).
    pub priority: Option<u32>,
    /// The physical board fact: the line is electrically shared (GRILL D05,
    /// I08 single source). Defaults `false`.
    pub shared: bool,
    /// Whether the `[interrupt.<id>]` section was present and joined. A
    /// missing section on a non-`Optional` form is a frontend error (C03);
    /// `Optional` absence leaves this `false`.
    pub resolved: bool,
}

/// Canonical `trigger_type` vocabulary (C03). Closed, append-only.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TriggerType {
    /// Level-triggered line.
    Level = 0,
    /// Edge-triggered line.
    Edge = 1,
}

/// Canonical `trigger_polarity` vocabulary (C03). Closed, append-only.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TriggerPolarity {
    /// Active-high level.
    ActiveHigh = 0,
    /// Active-low level.
    ActiveLow = 1,
    /// Rising edge.
    Rising = 2,
    /// Falling edge.
    Falling = 3,
}

/// One recognized `#[rseq::isr]` handler.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct IsrHandler {
    /// The method name (wrapper symbols, diagnostics).
    pub method_name: StringId,
    /// The bound resource: an index into [`DriverInterrupts::resources`].
    /// Bare `#[rseq::isr]` resolves against exactly one resource (frontend
    /// error when ambiguous); `#[rseq::isr(field = "…")]` names it explicitly.
    pub resource: u32,
    /// Whether this is an array-bind handler (`fn(&mut self, index: usize)`)
    /// for an `[Interrupt; N]` resource.
    pub array_bind: bool,
    /// The force-materialized authored body (Phase-5 resolution, like
    /// `LifecycleOps` callbacks). `None` until resolved.
    pub body: Option<InstanceRef>,
    /// `IrqResult` arm reachability, computed by the bridge from the handler
    /// body. Drives the R11 shared-discipline warning (warn unless
    /// `NotHandled` is PROVEN reachable — warn-on-unknown) and the R09
    /// `DeferWork` emit-stage rejection (reject unless PROVEN unreachable —
    /// fail-loud-on-unknown). `#[serde(default)]` (all-`Unknown`) keeps the
    /// conservative direction for models serialized without the facts.
    #[serde(default)]
    pub arms: IrqArmReachability,
}

/// Per-arm reachability facts for a handler's `IrqResult` returns.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct IrqArmReachability {
    /// `IrqResult::Handled` reachability.
    pub handled: ArmReachability,
    /// `IrqResult::NotHandled` reachability.
    pub not_handled: ArmReachability,
    /// `IrqResult::DeferWork` reachability.
    pub defer_work: ArmReachability,
}

/// Tri-state reachability of one `IrqResult` arm. The default is `Unknown`,
/// which every consumer must treat conservatively: the R11 warning fires
/// (over-warn, never under-warn) and the R09 `DeferWork` guard rejects
/// (fail-loud, never silent). Closed, append-only.
#[repr(u8)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ArmReachability {
    /// The analysis could not decide. Consumers treat as reachable for
    /// rejection purposes and as not-proven-reachable for the R11 warning.
    #[default]
    Unknown = 0,
    /// A return of this arm is proven reachable.
    ProvenReachable = 1,
    /// No return of this arm exists or all are proven unreachable.
    ProvenUnreachable = 2,
}

/// Serde adapter: serialize the `ItemId`-keyed driver map as a JSON array of
/// `(key, value)` pairs (mirrors `device_lifecycle::ordered_driver_map`).
/// `BTreeMap` iteration is key-sorted, so the output is deterministic across
/// runs regardless of insertion order.
mod ordered_interrupt_map {
    use super::*;
    use serde::{Deserializer, Serializer};

    pub fn serialize<S>(
        map: &BTreeMap<ItemId, DriverInterrupts>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let entries: Vec<(&ItemId, &DriverInterrupts)> = map.iter().collect();
        entries.serialize(serializer)
    }

    pub fn deserialize<'de, D>(
        deserializer: D,
    ) -> Result<BTreeMap<ItemId, DriverInterrupts>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let entries = Vec::<(ItemId, DriverInterrupts)>::deserialize(deserializer)?;
        Ok(entries.into_iter().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::de::DeserializeOwned;

    fn sid(n: u32) -> StringId {
        StringId::from_u32(n)
    }

    fn round_trip<T: Serialize + DeserializeOwned + PartialEq + std::fmt::Debug>(value: &T) {
        let json = serde_json::to_string(value).expect("serialize");
        let back: T = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(&back, value);
    }

    fn sample_tables() -> InterruptTables {
        let mut tables = InterruptTables::default();
        tables.drivers.insert(
            ItemId::from_u32(3),
            DriverInterrupts {
                resources: vec![InterruptResource {
                    name: sid(1),
                    id: sid(2),
                    form: InterruptForm::Single,
                    binding: InterruptBinding {
                        irq_number: Some(37),
                        trigger_type: Some(TriggerType::Level),
                        trigger_polarity: Some(TriggerPolarity::ActiveHigh),
                        priority: None,
                        shared: false,
                        resolved: true,
                    },
                }],
                handlers: vec![IsrHandler {
                    method_name: sid(3),
                    resource: 0,
                    array_bind: false,
                    body: None,
                    arms: IrqArmReachability {
                        handled: ArmReachability::ProvenReachable,
                        not_handled: ArmReachability::ProvenReachable,
                        defer_work: ArmReachability::ProvenUnreachable,
                    },
                }],
            },
        );
        tables
    }

    #[test]
    fn interrupt_tables_round_trip() {
        round_trip(&sample_tables());
    }

    #[test]
    fn interrupt_map_json_is_deterministic_across_insert_order() {
        let mut a = InterruptTables::default();
        let mut b = InterruptTables::default();
        let mk = |n: u32| DriverInterrupts {
            resources: vec![InterruptResource {
                name: sid(n),
                id: sid(n + 1),
                form: InterruptForm::Single,
                binding: InterruptBinding::default(),
            }],
            handlers: Vec::new(),
        };
        a.drivers.insert(ItemId::from_u32(1), mk(1));
        a.drivers.insert(ItemId::from_u32(2), mk(2));
        b.drivers.insert(ItemId::from_u32(2), mk(2));
        b.drivers.insert(ItemId::from_u32(1), mk(1));
        assert_eq!(
            serde_json::to_string(&a).unwrap(),
            serde_json::to_string(&b).unwrap()
        );
    }

    #[test]
    fn all_three_forms_round_trip() {
        for form in [
            InterruptForm::Single,
            InterruptForm::Optional,
            InterruptForm::Array { len: 4 },
        ] {
            round_trip(&form);
        }
    }

    #[test]
    fn arm_reachability_defaults_unknown() {
        let arms = IrqArmReachability::default();
        assert_eq!(arms.handled, ArmReachability::Unknown);
        assert_eq!(arms.not_handled, ArmReachability::Unknown);
        assert_eq!(arms.defer_work, ArmReachability::Unknown);
    }

    #[test]
    fn handler_without_arms_field_deserializes_to_unknown() {
        // A handler serialized before the reachability facts existed (or by a
        // degraded analysis) must deserialize with the conservative default.
        let json = r#"{"method_name":7,"resource":0,"array_bind":false,"body":null}"#;
        let handler: IsrHandler = serde_json::from_str(json).expect("deserialize");
        assert_eq!(handler.arms, IrqArmReachability::default());
    }

    #[test]
    fn binding_defaults_are_unresolved() {
        let b = InterruptBinding::default();
        assert!(b.irq_number.is_none());
        assert!(!b.shared);
        assert!(!b.resolved);
    }
}
