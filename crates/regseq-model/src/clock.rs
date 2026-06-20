//! Peripheral-clock-management construct IR.
//!
//! The author declares a typed `Clock` field on the `#[rseq::driver]` struct via
//! `#[rseq::clock(id = "…")]` and calls `enable()` / `disable()` / `get_rate()`
//! on it. This module is the owned/serializable model of that surface:
//! per-resource facts (`id` join key, declaration form via `required`, field
//! name) and the L2-resolved `[clock.<id>]` binding (mode, default_frequency,
//! provider). It is populated by the `regseq-driver` bridge's `clock`
//! recognition, joined by the `regseq-l2` `clock` tenant, and consumed by the
//! backends' clock lifecycle glue + per-op lowering.
//!
//! It is the `interrupt.rs` mirror (a dedicated top-level `RegseqModel` table,
//! GRILL D03): a peripheral clock is a *capability layered on the device*, not
//! part of the `DriverHandle`'s address footprint, so it gets its own table
//! rather than living on `DriverHandle`. Lifecycle adjacency
//! (`acquire_clock` after `map_region`, before interrupt) is handled by
//! phase-ordering across tables (GRILL D16/C07), not by IR co-location.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::ids::{ItemId, StringId};

/// Sibling table on `RegseqModel` for recognized clock surfaces, keyed by the
/// `#[rseq::driver]` struct's final `ItemId` (assigned at Phase 4, like
/// `device_lifecycle` / `interrupt`). Sparse via map absence. `#[serde(default)]`
/// on the model field keeps pre-construct models deserializable (GRILL I06/D04).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClockTables {
    /// Recognized per-driver clock surfaces, keyed by the driver-struct Item.
    #[serde(with = "ordered_clock_map")]
    pub drivers: BTreeMap<ItemId, DriverClocks>,
}

/// One driver's clock surface: its resources in declaration order (the
/// `acquire_clock` preamble ops run in declaration order, GRILL D16). Unlike
/// `DriverInterrupts`, there are no handlers — clock ops are author-called
/// intrinsics, not callback bindings.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriverClocks {
    /// `#[rseq::clock(id = "…")]` resource fields, declaration order.
    pub resources: Vec<ClockResource>,
}

/// One recognized `Clock` resource field on the driver struct.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClockResource {
    /// The struct field name.
    pub name: StringId,
    /// The mandatory L1 resource id (`#[rseq::clock(id = "…")]`): the join key
    /// to the platform file's `[clock.<id>]` section and the generic-c trace
    /// identity. Always present — a missing/empty `id` is a frontend error
    /// (no field-name fallback, GRILL D13).
    pub id: StringId,
    /// The declaration form: `true` for the required `Clock` form (v1 always
    /// recognizes only this; `Option<Clock>` is recognized-and-rejected, GRILL
    /// D04/C06 N2). Reserved for the future optional form.
    pub required: bool,
    /// The L2-resolved `[clock.<id>]` binding. Empty (`ClockBinding::default`)
    /// until the platform-file join; the `clock` L2 tenant validates the mode
    /// token + mode↔provider invariant and fills it. `#[serde(default)]` keeps
    /// earlier serialized models deserializable.
    #[serde(default)]
    pub binding: ClockBinding,
}

/// The binding mode of a clock (GRILL C01/C02). Closed, append-only.
#[repr(u8)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ClockMode {
    /// `mode = "static"` — a fixed `default_frequency`; `enable`/`disable`
    /// lower to no-ops and `get_rate` const-folds to `default_frequency`. No
    /// `provider`. The default for an unjoined `ClockBinding`.
    #[default]
    Static = 0,
    /// `mode = "managed"` — backed by the OS clock framework; carries a
    /// `provider` (the backend-interpreted con-id / DT clock spec).
    Managed = 1,
}

/// The L2-resolved `[clock.<id>]` facts (contract C02). The tenant validates the
/// `mode` token to `ClockMode`, fills `default_frequency`/`provider`, and sets
/// `resolved`. The mode↔provider invariant is enforced by [`ClockBinding::is_resolved`].
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClockBinding {
    /// `managed` | `static`. Defaults to `Static` for an unjoined binding (which
    /// is still `resolved == false`, so it is never treated as valid).
    pub mode: ClockMode,
    /// `default_frequency` in Hz. Required (`Some`, `> 0`) on every resolved
    /// binding (GRILL D15); `None` until joined. Never a managed runtime
    /// fallback (GRILL D10/I04) — schema / static-portability only.
    pub default_frequency: Option<u32>,
    /// The backend-interpreted provider string (`devm_clk_get` con-id on Linux,
    /// DT clock spec on Zephyr). `Some` iff `Managed`, `None` iff `Static`
    /// (GRILL D14/I03). `None` until joined.
    pub provider: Option<StringId>,
    /// Whether the `[clock.<id>]` section was present and joined. A missing
    /// section on a required `Clock` is a binding error (`CLK-L2-*`, C06 N3).
    pub resolved: bool,
}

impl ClockBinding {
    /// Full binding validity (not merely "the section was joined"): the section
    /// must have been joined (`resolved`), a `default_frequency` must be present
    /// AND positive (GRILL D15 — a 0 Hz clock cannot compute a baud divisor), and
    /// the mode↔provider invariant must hold — a `Managed` binding MUST carry a
    /// `provider` (C06 N7) and a `Static` binding MUST NOT (C06 N8). This is the
    /// single source of truth the L2 tenant's `bind`/`validate` (regseq-l2) and
    /// the backend fold selection (regseq-backends) agree with (GRILL C02/I03):
    /// every shape the tenant rejects (incl. `default_frequency = 0`, CLK-L2-003)
    /// is `!is_resolved()`.
    pub fn is_resolved(&self) -> bool {
        self.resolved
            && self.default_frequency.is_some_and(|f| f > 0)
            && match self.mode {
                ClockMode::Managed => self.provider.is_some(),
                ClockMode::Static => self.provider.is_none(),
            }
    }
}

/// Serde adapter: serialize the `ItemId`-keyed driver map as a JSON array of
/// `(key, value)` pairs (mirrors `interrupt::ordered_interrupt_map` /
/// `device_lifecycle::ordered_driver_map`). `BTreeMap` iteration is key-sorted,
/// so the output is deterministic across runs regardless of insertion order.
mod ordered_clock_map {
    use super::*;
    use serde::{Deserializer, Serializer};

    pub fn serialize<S>(
        map: &BTreeMap<ItemId, DriverClocks>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let entries: Vec<(&ItemId, &DriverClocks)> = map.iter().collect();
        entries.serialize(serializer)
    }

    pub fn deserialize<'de, D>(
        deserializer: D,
    ) -> Result<BTreeMap<ItemId, DriverClocks>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let entries = Vec::<(ItemId, DriverClocks)>::deserialize(deserializer)?;
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

    fn sample_tables() -> ClockTables {
        let mut tables = ClockTables::default();
        tables.drivers.insert(
            ItemId::from_u32(3),
            DriverClocks {
                resources: vec![
                    ClockResource {
                        name: sid(1),
                        id: sid(2),
                        required: true,
                        binding: ClockBinding {
                            mode: ClockMode::Managed,
                            default_frequency: Some(24_000_000),
                            provider: Some(sid(4)),
                            resolved: true,
                        },
                    },
                    ClockResource {
                        name: sid(5),
                        id: sid(6),
                        required: true,
                        binding: ClockBinding {
                            mode: ClockMode::Static,
                            default_frequency: Some(48_000_000),
                            provider: None,
                            resolved: true,
                        },
                    },
                ],
            },
        );
        tables
    }

    #[test]
    fn clock_tables_round_trip() {
        round_trip(&sample_tables());
    }

    #[test]
    fn clock_map_json_is_deterministic_across_insert_order() {
        let mut a = ClockTables::default();
        let mut b = ClockTables::default();
        let mk = |n: u32| DriverClocks {
            resources: vec![ClockResource {
                name: sid(n),
                id: sid(n + 1),
                required: true,
                binding: ClockBinding::default(),
            }],
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
    fn both_modes_round_trip() {
        for mode in [ClockMode::Static, ClockMode::Managed] {
            round_trip(&mode);
        }
    }

    #[test]
    fn clock_mode_defaults_static() {
        assert_eq!(ClockMode::default(), ClockMode::Static);
    }

    #[test]
    fn clock_mode_discriminants_are_stable() {
        // PLAN T19 (S13/R13/I06): `ClockMode` is the IR-side append-only enum of
        // the peripheral-clock-management construct. Pin its serialized
        // discriminants so a reorder is caught here (the domain_attr.rs sweep
        // pins `AttrData::ClockResource = 6` / `IntrinsicKind::Clock = 7` /
        // `ClockOp = 0/1/2`; this completes the construct's discriminant cover).
        // `ClockMode` is fieldless, so `as u8` yields the pinned tag.
        assert_eq!(ClockMode::Static as u8, 0);
        assert_eq!(ClockMode::Managed as u8, 1);
    }

    #[test]
    fn binding_defaults_are_unresolved() {
        let b = ClockBinding::default();
        assert_eq!(b.mode, ClockMode::Static);
        assert!(b.default_frequency.is_none());
        assert!(b.provider.is_none());
        assert!(!b.resolved);
        // Default binding is not yet a valid, fully-resolved binding.
        assert!(!b.is_resolved());
    }

    #[test]
    fn is_resolved_truth_table() {
        // A binding that was never joined is never valid, regardless of mode.
        for mode in [ClockMode::Static, ClockMode::Managed] {
            let b = ClockBinding {
                mode,
                default_frequency: Some(24_000_000),
                provider: if mode == ClockMode::Managed {
                    Some(sid(1))
                } else {
                    None
                },
                resolved: false,
            };
            assert!(!b.is_resolved(), "unjoined {mode:?} must not be valid");
        }

        // Resolved but missing default_frequency → invalid (D15).
        assert!(!ClockBinding {
            mode: ClockMode::Static,
            default_frequency: None,
            provider: None,
            resolved: true,
        }
        .is_resolved());

        // Resolved with default_frequency = 0 → invalid (D15: a 0 Hz clock
        // cannot compute a baud divisor; the L2 tenant rejects it as CLK-L2-003,
        // so it must read `!is_resolved()`).
        assert!(!ClockBinding {
            mode: ClockMode::Static,
            default_frequency: Some(0),
            provider: None,
            resolved: true,
        }
        .is_resolved());

        // Managed: provider required (N7).
        assert!(ClockBinding {
            mode: ClockMode::Managed,
            default_frequency: Some(24_000_000),
            provider: Some(sid(1)),
            resolved: true,
        }
        .is_resolved());
        assert!(!ClockBinding {
            mode: ClockMode::Managed,
            default_frequency: Some(24_000_000),
            provider: None,
            resolved: true,
        }
        .is_resolved());

        // Static: provider forbidden (N8).
        assert!(ClockBinding {
            mode: ClockMode::Static,
            default_frequency: Some(24_000_000),
            provider: None,
            resolved: true,
        }
        .is_resolved());
        assert!(!ClockBinding {
            mode: ClockMode::Static,
            default_frequency: Some(24_000_000),
            provider: Some(sid(1)),
            resolved: true,
        }
        .is_resolved());
    }
}
