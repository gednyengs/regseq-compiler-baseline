//! Synchronization (locks) construct IR.
//!
//! The author declares a typed lock field on the `#[rseq::driver]` struct —
//! the field's *type is the kind*: `Mutex<T>` / `SpinLock<T>` /
//! `IrqSpinLock<T>` (sleeping / spinning / IRQ-safe-spinning mutual exclusion)
//! or `Completion` (one-shot signaling). Acquire returns an RAII `Guard<'_,T>`
//! that releases on drop (or explicit `guard_drop_early`); there is no
//! standalone `unlock`. This module is the owned/serializable model of that
//! surface: per-driver resource declarations (`kind`, protected type `T`,
//! cardinality) keyed by the driver-struct `ItemId`, plus per-acquire-site
//! facts the backends lower from. It is populated by the `regseq-driver`
//! bridge's `synchronization` recognition and consumed by the backends' lock
//! lowering + the generic-c oracle.
//!
//! **No new `IntrinsicKind`** — sync ops are wrapper methods, not intrinsics
//! (GRILL `C04` / design §6). Acquire-site bodies are referenced by
//! `InstanceRef` (content-addressed), NOT raw `InstanceId`, so they survive
//! the Phase-4 instance sort — the same discipline as `interrupt::IsrHandler`
//! and `device_lifecycle::LifecycleOps`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::ids::{BlockId, FieldIdx, ItemId, LocalId, StringId, TyId};
use crate::instance_ref::InstanceRef;

/// Sibling table on `RegseqModel` for recognized synchronization surfaces,
/// keyed by the `#[rseq::driver]` struct's final `ItemId` (assigned at Phase 4,
/// like `device_lifecycle`/`interrupt`). Sparse via map absence.
/// `#[serde(default)]` on the model field keeps pre-construct models
/// deserializable.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SynchronizationTables {
    /// Recognized per-driver synchronization surfaces, keyed by the
    /// driver-struct Item.
    #[serde(with = "ordered_synchronization_map")]
    pub drivers: BTreeMap<ItemId, DriverSynchronization>,
}

/// One driver's synchronization surface: its declared lock/completion
/// resources (declaration order) and the acquire sites recognized in its
/// bodies.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriverSynchronization {
    /// Declared lock/completion resource fields, declaration order. The index
    /// into this `Vec` is the resource's stable `lock_id` (referenced by
    /// [`AcquireSite::lock_id`] and threaded into the backend hook calls).
    pub resources: Vec<SyncResource>,
    /// Recognized lock acquire sites across this driver's bodies (`lock()`
    /// acquire + its paired guard release — drop or `guard_drop_early`). The
    /// backend lowers these to the per-OS primitive + the generic-c oracle
    /// hooks, keyed by the site's `(body, block)`. Empty until the Phase-5 op
    /// pass runs. `#[serde(default)]` keeps earlier serialized models
    /// deserializable.
    #[serde(default)]
    pub acquire_sites: Vec<AcquireSite>,
    /// Recognized `Completion` op sites (`wait`/`signal`/`reset`) across this
    /// driver's bodies. Point events (no guard), keyed by `(body, block)`.
    /// `#[serde(default)]` keeps earlier serialized models deserializable.
    #[serde(default)]
    pub completion_sites: Vec<CompletionSite>,
    /// The produced spinlock-held atomic-section markings (the thesis component,
    /// GRILL `C04` / PLAN T10): one [`SpinlockHeldRegion`] per `SpinLock`/
    /// `IrqSpinLock` acquire site, spanning the guard's (approximated) MIR live
    /// range. `Mutex` acquires do **not** produce a region — sleeping while
    /// holding a sleeping mutex is legal. Empty until the M3 context-marking
    /// pass (`super::super::synchronization::context`) runs. `#[serde(default)]`
    /// keeps M1/M2-era serialized models deserializable.
    #[serde(default)]
    pub held_regions: Vec<SpinlockHeldRegion>,
}

/// One recognized lock/completion resource field on the driver struct.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncResource {
    /// The struct field name.
    pub name: StringId,
    /// The driver-struct field index this lock is declared at. The Phase-5 op
    /// pass resolves a `lock()` receiver place to this index to recover the
    /// resource's `lock_id` (its position in `DriverSynchronization::resources`).
    pub field: FieldIdx,
    /// The lock kind, given by the field's wrapper type — `sync.lock_type` is
    /// `required` with no default (there is no separate "kind" attribute;
    /// GRILL `C01`/`D01`).
    pub kind: SyncKind,
    /// The protected payload type `T` (the `IrqSpinLock<T>`/… type argument).
    /// `None` for `Completion`, which carries no `T`. `#[serde(default)]` for
    /// append-only deserialization.
    #[serde(default)]
    pub protected_type: Option<TyId>,
    /// The decl-time signaling cardinality (`Completion` only; resolved at
    /// `init_primitives`, not at signal time — GRILL `I07`). `SingleWaiter` is
    /// the only first-slice cardinality. `#[serde(default)]` = `SingleWaiter`.
    #[serde(default)]
    pub cardinality: Cardinality,
}

/// The lock kind, selected by the field's wrapper type. Closed, append-only.
///
/// The first slice *recognizes/emits* `IrqSpinLock` (M1 — the corpus witness
/// kind that also discharges the Tier-2 pin); `Mutex`/`SpinLock`/`Completion`
/// land in M2. `RwLock`/`Semaphore` are deferred (designed-in, not present —
/// GRILL `C01`).
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SyncKind {
    /// Sleeping mutual exclusion (`may_sleep = Y`, `irq_safe = N`).
    Mutex = 0,
    /// Spinning, disables preemption (`may_sleep = N`, `irq_safe = N`;
    /// thread-only).
    SpinLock = 1,
    /// Spinning + IRQ save/restore (`may_sleep = N`, `irq_safe = Y`); the
    /// corpus witness kind (E1000 `stats_lock`, PL011 ISR↔mainline seam).
    IrqSpinLock = 2,
    /// One-shot signaling; carries no `T`.
    Completion = 3,
}

/// The decl-time signaling cardinality of a `Completion`. Closed, append-only.
///
/// `SingleWaiter` is the only first-slice cardinality; `AllWaiters` is deferred
/// (the Zephyr companion-condvar backing state is underspecified and there is
/// no corpus witness — GRILL `C02`).
#[repr(u8)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Cardinality {
    /// Exactly one waiter (Linux `complete`, Zephyr `k_sem_give`).
    #[default]
    SingleWaiter = 0,
}

/// One recognized synchronization op site (first slice: a `lock()` acquire and
/// its paired guard release). Recorded by the bridge's Phase-5 op pass and
/// lowered by the backends keyed by `(body, block)`. The resolved execution
/// context + the MIR live-range fields are appended in M3 (the context-marking
/// analysis); the struct is `#[serde(default)]`-friendly for that growth.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcquireSite {
    /// Index into [`DriverSynchronization::resources`] — the stable `lock_id`.
    pub lock_id: u32,
    /// The op recognized at this site.
    pub op: SyncOp,
    /// The enclosing function body, content-addressed so it survives the
    /// Phase-4 instance sort (like `interrupt::IsrHandler::body`).
    pub body: InstanceRef,
    /// The basic block whose terminator is the acquire `Call`.
    pub acquire_block: BlockId,
    /// The guard local the acquire binds (`let g = lock.lock()`), used to match
    /// the guard's drop-glue release `Call` (whose dropped place chases back to
    /// this local through the synthetic `RawPtr` drop prelude).
    pub guard_local: LocalId,
    /// The basic block(s) whose drop-glue `Call` releases the guard (scope end
    /// or `guard_drop_early`). More than one on a branchy CFG (each unwind /
    /// normal exit drops the guard).
    pub release_blocks: Vec<BlockId>,
}

/// The recognized synchronization op kind at an [`AcquireSite`]. Closed,
/// append-only. `LockAcquire` is the only acquire op; its release (the guard's
/// drop or an explicit `guard_drop_early`) is recorded in
/// [`AcquireSite::release_blocks`].
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SyncOp {
    /// `lock()` → `Guard` on a `Mutex`/`SpinLock`/`IrqSpinLock`. Release is the
    /// guard's drop (or `guard_drop_early`), recorded in
    /// [`AcquireSite::release_blocks`].
    LockAcquire = 0,
}

/// One recognized `Completion` op site (`wait`/`signal`/`reset`) — a point event
/// with no guard, keyed to its body + block for backend lowering to the per-OS
/// completion primitive + the generic-c oracle hook.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompletionSite {
    /// Index into [`DriverSynchronization::resources`] — the `Completion`'s
    /// `lock_id`.
    pub lock_id: u32,
    /// The op recognized at this site.
    pub op: CompletionOpKind,
    /// The enclosing function body, content-addressed (survives the Phase-4
    /// instance sort).
    pub body: InstanceRef,
    /// The basic block whose terminator is the op `Call`.
    pub block: BlockId,
}

/// The recognized `Completion` op kind. Closed, append-only.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CompletionOpKind {
    /// `wait(Duration) -> Result<(), RegseqError>` — sleeping, thread-only.
    Wait = 0,
    /// `signal()` — ISR-safe.
    Signal = 1,
    /// `reset()` — re-arm.
    Reset = 2,
}

/// The produced **spinlock-held atomic-section marking** (the thesis component;
/// GRILL `C04` / PLAN T10) over one `SpinLock`/`IrqSpinLock` acquire site's
/// guard. The first construct that is both a *consumer* of context markings
/// (device-interrupt's `IsrBinding`, timed-delay's may-sleep predicate) and a
/// *producer* of one.
///
/// [`Self::blocks`] is the set of basic blocks in [`Self::body`] where the
/// guard is **held at block entry** — an intraprocedural, **may-held,
/// path-insensitive** over-approximation of the guard's MIR live range
/// (computed by BFS from the acquire's CFG successors, bounded by the
/// acquire-site's `release_blocks`; the region ends at `guard_drop_early`, not
/// the syntactic scope). The may-sleep-while-held and recursive-acquire checks
/// (PLAN T12) consume this. Soundness for those consumers is *no false
/// negative* (never misses a block where the guard is genuinely held); the
/// residual false-positives are infeasible CFG paths / may-held joins (GRILL
/// `I04` — intraprocedural/lexical only).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpinlockHeldRegion {
    /// Index into [`DriverSynchronization::resources`] — the held lock's
    /// `lock_id` (matches the producing [`AcquireSite::lock_id`]).
    pub lock_id: u32,
    /// The held lock's kind — always a spinning kind (`SpinLock`/`IrqSpinLock`);
    /// `Mutex` produces no region. Carried so a consumer need not re-resolve it.
    pub kind: SyncKind,
    /// The enclosing function body, content-addressed (survives the Phase-4
    /// instance sort) — the same `InstanceRef` as the producing
    /// [`AcquireSite::body`].
    pub body: InstanceRef,
    /// The acquire block whose guard this region tracks (matches the producing
    /// [`AcquireSite::acquire_block`]). Excluded from [`Self::blocks`] — the
    /// guard is not yet held while the acquire `Call` itself executes.
    pub acquire_block: BlockId,
    /// The basic blocks where the guard is held at entry (block order). The
    /// release blocks are included (the guard is held through their statements,
    /// dropped at their terminator); their successors are not.
    pub blocks: Vec<BlockId>,
}

/// Serde adapter: serialize the `ItemId`-keyed driver map as a JSON array of
/// `(key, value)` pairs (mirrors `interrupt::ordered_interrupt_map` /
/// `device_lifecycle::ordered_driver_map`). `BTreeMap` iteration is key-sorted,
/// so the output is deterministic across runs regardless of insertion order.
mod ordered_synchronization_map {
    use super::*;
    use serde::{Deserializer, Serializer};

    pub fn serialize<S>(
        map: &BTreeMap<ItemId, DriverSynchronization>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let entries: Vec<(&ItemId, &DriverSynchronization)> = map.iter().collect();
        entries.serialize(serializer)
    }

    pub fn deserialize<'de, D>(
        deserializer: D,
    ) -> Result<BTreeMap<ItemId, DriverSynchronization>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let entries = Vec::<(ItemId, DriverSynchronization)>::deserialize(deserializer)?;
        Ok(entries.into_iter().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::args_key::ArgsKey;
    use crate::def_path::DefPathHash;
    use crate::StableCrateId;
    use serde::de::DeserializeOwned;

    fn sid(n: u32) -> StringId {
        StringId::from_u32(n)
    }

    fn iref(local_hash: u64) -> InstanceRef {
        InstanceRef {
            regseq_kind_rank: 0,
            def_path_hash: DefPathHash {
                stable_crate_id: StableCrateId(1),
                local_hash,
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

    fn sample_tables() -> SynchronizationTables {
        let mut tables = SynchronizationTables::default();
        tables.drivers.insert(
            ItemId::from_u32(3),
            DriverSynchronization {
                resources: vec![SyncResource {
                    name: sid(1),
                    field: FieldIdx::from_u32(2),
                    kind: SyncKind::IrqSpinLock,
                    protected_type: Some(TyId::from_u32(7)),
                    cardinality: Cardinality::SingleWaiter,
                }],
                acquire_sites: vec![AcquireSite {
                    lock_id: 0,
                    op: SyncOp::LockAcquire,
                    body: iref(42),
                    acquire_block: BlockId::from_u32(1),
                    guard_local: LocalId::from_u32(4),
                    release_blocks: vec![BlockId::from_u32(2)],
                }],
                completion_sites: vec![CompletionSite {
                    lock_id: 0,
                    op: CompletionOpKind::Signal,
                    body: iref(43),
                    block: BlockId::from_u32(3),
                }],
                held_regions: vec![SpinlockHeldRegion {
                    lock_id: 0,
                    kind: SyncKind::IrqSpinLock,
                    body: iref(42),
                    acquire_block: BlockId::from_u32(1),
                    blocks: vec![BlockId::from_u32(2)],
                }],
            },
        );
        tables
    }

    #[test]
    fn synchronization_tables_round_trip() {
        round_trip(&sample_tables());
    }

    #[test]
    fn synchronization_map_json_is_deterministic_across_insert_order() {
        let mut a = SynchronizationTables::default();
        let mut b = SynchronizationTables::default();
        let mk = |n: u32| DriverSynchronization {
            resources: vec![SyncResource {
                name: sid(n),
                field: FieldIdx::ZERO,
                kind: SyncKind::IrqSpinLock,
                protected_type: None,
                cardinality: Cardinality::SingleWaiter,
            }],
            acquire_sites: Vec::new(),
            completion_sites: Vec::new(),
            held_regions: Vec::new(),
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
    fn all_four_kinds_round_trip() {
        for kind in [
            SyncKind::Mutex,
            SyncKind::SpinLock,
            SyncKind::IrqSpinLock,
            SyncKind::Completion,
        ] {
            round_trip(&SyncResource {
                name: sid(1),
                field: FieldIdx::ZERO,
                kind,
                protected_type: None,
                cardinality: Cardinality::SingleWaiter,
            });
        }
    }

    #[test]
    fn sync_kind_discriminants_are_stable() {
        assert_eq!(SyncKind::Mutex as u8, 0);
        assert_eq!(SyncKind::SpinLock as u8, 1);
        assert_eq!(SyncKind::IrqSpinLock as u8, 2);
        assert_eq!(SyncKind::Completion as u8, 3);
    }

    #[test]
    fn cardinality_defaults_single_waiter() {
        assert_eq!(Cardinality::default(), Cardinality::SingleWaiter);
    }

    #[test]
    fn driver_without_held_regions_deserializes_to_empty() {
        // An M1/M2-era driver serialized before `held_regions` existed
        // deserializes with an empty region table (append-only discipline).
        let json = r#"{"resources":[],"acquire_sites":[],"completion_sites":[]}"#;
        let drv: DriverSynchronization = serde_json::from_str(json).expect("deserialize");
        assert!(drv.held_regions.is_empty());
    }

    #[test]
    fn spinlock_held_region_round_trips() {
        round_trip(&SpinlockHeldRegion {
            lock_id: 3,
            kind: SyncKind::SpinLock,
            body: iref(99),
            acquire_block: BlockId::from_u32(0),
            blocks: vec![BlockId::from_u32(1), BlockId::from_u32(2), BlockId::from_u32(4)],
        });
    }

    #[test]
    fn resource_without_optional_fields_deserializes_to_defaults() {
        // A resource serialized before `protected_type`/`cardinality` existed
        // (or by a degraded recognizer) deserializes with the conservative
        // defaults (`None` payload, `SingleWaiter`).
        let json = r#"{"name":1,"field":0,"kind":"IrqSpinLock"}"#;
        let res: SyncResource = serde_json::from_str(json).expect("deserialize");
        assert_eq!(res.protected_type, None);
        assert_eq!(res.cardinality, Cardinality::SingleWaiter);
    }
}
