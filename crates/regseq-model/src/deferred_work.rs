//! Deferred-work-item-scheduling construct IR.
//!
//! The author declares a typed opaque `Work` field on the `#[rseq::driver]`
//! struct (recognized BY TYPE — `#[rseq::abstraction("deferred_work")]` on the
//! `Work` ADT item, NOT by a field attribute, GRILL D04), binds a handler method
//! via `#[rseq::work_handler] fn(&mut self)`, and schedules it with
//! `self.work.submit()`. This module is the owned/serializable model of that
//! surface: per-resource facts (field name, declaration form via `required`,
//! the design-in field `offset`), the force-materialized handler `InstanceRef`,
//! and the recorded submit sites. It is populated by the `regseq-driver`
//! bridge's `deferred_work` recognition + handler binding + submit pass, and
//! consumed by the backends' work lifecycle glue (`init_work`/`cancel_work`),
//! wrapper synthesis, and `submit` lowering.
//!
//! It is the `clock.rs` / `interrupt.rs` mirror (a dedicated top-level
//! `RegseqModel` table, GRILL D03): deferred work is a *capability layered on
//! the device* (a sleepable-context callback the driver schedules), not part of
//! the `DriverHandle`'s address footprint, so it gets its own table rather than
//! living on `DriverHandle`. Unlike `clock`, there is NO L2 binding — the
//! construct has 0 L1 / 0 L2 properties (GRILL D02), so there is no
//! `[deferred_work.<id>]` platform section and no join.
//!
//! Handler bodies are stored as `InstanceRef` (content-addressed), NOT raw
//! `InstanceId`, so they survive the Phase-4 (`F4-2`) instance sort — the same
//! discipline as `interrupt::IsrHandler::body` / `device_lifecycle::LifecycleOps`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::ids::{FieldIdx, ItemId, StringId};
use crate::instance_ref::InstanceRef;

/// Sibling table on `RegseqModel` for recognized deferred-work surfaces, keyed
/// by the `#[rseq::driver]` struct's final `ItemId` (assigned at Phase 4, like
/// `device_lifecycle` / `interrupt` / `clock`). Sparse via map absence.
/// `#[serde(default)]` on the model field keeps pre-construct models
/// deserializable (GRILL I06/D04).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeferredWorkTables {
    /// Recognized per-driver deferred-work surfaces, keyed by the driver-struct
    /// Item.
    #[serde(with = "ordered_deferred_work_map")]
    pub drivers: BTreeMap<ItemId, DriverDeferredWork>,
}

/// One driver's deferred-work surface: its `Work` resources in declaration
/// order (the `init_work` preamble ops run in declaration order). Unlike
/// `DriverInterrupts`, the bound handler lives ON the resource (a `Work` has
/// exactly one handler, GRILL D10) rather than in a separate handler vector.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriverDeferredWork {
    /// Recognized `Work` resource fields, declaration order. The declaration
    /// order is the deterministic per-resource slot index the generic-c backend
    /// uses for `&__regseq_work_slots[idx]` (consistent across
    /// `init_work`/`submit`/`cancel_work`, GRILL T07 storage contract).
    pub resources: Vec<WorkResource>,
}

/// One recognized `Work` resource field on the driver struct.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkResource {
    /// The struct field name (wrapper symbols, diagnostics, trace identity).
    pub name: StringId,
    /// The field's index within the driver struct variant. Lets the post-body
    /// submit pass + the generic-c backend map a recovered `self.work` receiver
    /// field path back to this resource (mirrors `SyncResource::field`), which
    /// determines the deterministic per-resource slot index (GRILL T06/T07).
    pub field: FieldIdx,
    /// The declaration form: `true` for the required `Work` form (v1 always
    /// recognizes only this; `Option<Work>` is recognized-and-rejected, GRILL
    /// D01/T04). Reserved for the future optional form.
    pub required: bool,
    /// The field's byte offset within the driver struct, recorded for the
    /// design-in `container_of`-style recovery (GRILL S14). Unused in v1 — the
    /// generic-c backend keys on the per-resource slot index, not the offset
    /// (the `Work` ADT is a ZST, so the field has no real storage).
    pub offset: u64,
    /// The force-materialized authored handler body (Phase-5 resolution, like
    /// `interrupt::IsrHandler::body` / `device_lifecycle` callbacks). `None`
    /// until the `#[rseq::work_handler]` binding resolves; a required `Work`
    /// that stays `None` is a frontend error
    /// (`DWS-ERR-REQUIRED-WORK-NO-HANDLER`, GRILL G5).
    pub handler: Option<InstanceRef>,
    /// The recorded `self.work.submit()` call sites bound to this resource,
    /// recovered by the bridge's post-body-materialization direct-field-path
    /// pass (GRILL T06). Empty when the author declares a `Work` but never
    /// submits it (the resource-gated `init_work`/`cancel_work` are still
    /// emitted, GRILL D07/D13). `#[serde(default)]` keeps earlier serialized
    /// models deserializable.
    #[serde(default)]
    pub submit_sites: Vec<SubmitSite>,
}

/// One recognized `submit`-family call site bound to a [`WorkResource`]
/// (GRILL T06). v1 records only the `Submit` variant; `SubmitUnique` is
/// design-in (GRILL G4, Out of Scope).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubmitSite {
    /// Which submit-family op this call site is.
    pub variant: SubmitVariant,
}

/// The submit-family operation of a [`SubmitSite`]. Closed, append-only —
/// `SubmitUnique` is reserved (not added) until `submit_unique` is built
/// (GRILL G4, Out of Scope).
#[repr(u8)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SubmitVariant {
    /// `self.work.submit()` — unconditional schedule (always queues a fresh
    /// dispatch). The only built variant.
    #[default]
    Submit = 0,
}

/// Serde adapter: serialize the `ItemId`-keyed driver map as a JSON array of
/// `(key, value)` pairs (mirrors `clock::ordered_clock_map` /
/// `interrupt::ordered_interrupt_map`). `BTreeMap` iteration is key-sorted, so
/// the output is deterministic across runs regardless of insertion order.
mod ordered_deferred_work_map {
    use super::*;
    use serde::{Deserializer, Serializer};

    pub fn serialize<S>(
        map: &BTreeMap<ItemId, DriverDeferredWork>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let entries: Vec<(&ItemId, &DriverDeferredWork)> = map.iter().collect();
        entries.serialize(serializer)
    }

    pub fn deserialize<'de, D>(
        deserializer: D,
    ) -> Result<BTreeMap<ItemId, DriverDeferredWork>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let entries = Vec::<(ItemId, DriverDeferredWork)>::deserialize(deserializer)?;
        Ok(entries.into_iter().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::de::DeserializeOwned;

    use crate::args_key::ArgsKey;
    use crate::crate_info::StableCrateId;
    use crate::def_path::DefPathHash;

    fn sid(n: u32) -> StringId {
        StringId::from_u32(n)
    }

    fn iref(n: u64) -> InstanceRef {
        InstanceRef {
            regseq_kind_rank: 0,
            def_path_hash: DefPathHash {
                stable_crate_id: StableCrateId(0xAABB),
                local_hash: n,
            },
            args_key: ArgsKey(0),
            fn_shim_disambig: 0,
        }
    }

    fn round_trip<T: Serialize + DeserializeOwned + PartialEq + std::fmt::Debug>(value: &T) {
        let json = serde_json::to_string(value).expect("serialize");
        let back: T = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(&back, value);
    }

    fn sample_tables() -> DeferredWorkTables {
        let mut tables = DeferredWorkTables::default();
        tables.drivers.insert(
            ItemId::from_u32(3),
            DriverDeferredWork {
                resources: vec![WorkResource {
                    name: sid(1),
                    field: FieldIdx::from_u32(0),
                    required: true,
                    offset: 0,
                    handler: Some(iref(42)),
                    submit_sites: vec![SubmitSite {
                        variant: SubmitVariant::Submit,
                    }],
                }],
            },
        );
        tables
    }

    #[test]
    fn deferred_work_tables_round_trip() {
        round_trip(&sample_tables());
    }

    #[test]
    fn deferred_work_map_json_is_deterministic_across_insert_order() {
        let mut a = DeferredWorkTables::default();
        let mut b = DeferredWorkTables::default();
        let mk = |n: u32| DriverDeferredWork {
            resources: vec![WorkResource {
                name: sid(n),
                field: FieldIdx::from_u32(0),
                required: true,
                offset: 0,
                handler: None,
                submit_sites: Vec::new(),
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
    fn submit_variant_defaults_submit_and_discriminant_is_stable() {
        // `SubmitVariant` is the IR-side append-only enum; `Submit` is fixed at
        // discriminant 0 and is the default. `SubmitUnique` is reserved (Out of
        // Scope) and not added.
        assert_eq!(SubmitVariant::default(), SubmitVariant::Submit);
        assert_eq!(SubmitVariant::Submit as u8, 0);
    }

    #[test]
    fn work_resource_no_submit_round_trips() {
        // A declared `Work` with a bound handler but NO submit site round-trips
        // (the resource-gated `init_work`/`cancel_work` still emit, D07/D13).
        round_trip(&WorkResource {
            name: sid(5),
            field: FieldIdx::from_u32(2),
            required: true,
            offset: 8,
            handler: Some(iref(7)),
            submit_sites: Vec::new(),
        });
    }

    #[test]
    fn work_resource_unbound_handler_round_trips() {
        // A pre-binding `Work` (handler not yet resolved) round-trips with the
        // `None` handler and empty submit sites.
        round_trip(&WorkResource {
            name: sid(9),
            field: FieldIdx::from_u32(0),
            required: true,
            offset: 0,
            handler: None,
            submit_sites: Vec::new(),
        });
    }
}
