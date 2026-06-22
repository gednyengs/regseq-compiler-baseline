//! Domain attribute sibling-table data shapes.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::ids::{FieldIdx, ItemId, StringId, Symbol, VariantIdx};
use crate::index_vec::IndexVec;
use crate::interrupt::InterruptForm;
use crate::source::Span;
use crate::synchronization::SyncKind;

/// Sibling tables on `RegseqModel` for structured DSL property
/// attributes derived from the `#[rseq::*]` tool namespace.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DomainAttrTables {
    /// Attributes attached to source-level Items.
    ///
    /// The table is indexed by `ItemId` and is sparse via empty
    /// vectors. Stage 14's `AttrData::Intrinsic` entries live here.
    pub item_attrs: IndexVec<ItemId, Vec<AttrData>>,

    /// Attributes attached to fields of ADT Items.
    ///
    /// The table is indexed by `FieldPath` and sparse via absence in
    /// the map. Phase-2 families append future occupants.
    #[serde(with = "ordered_map_vec")]
    pub field_attrs: BTreeMap<FieldPath, Vec<AttrData>>,

    /// Attributes attached to source-code spans.
    ///
    /// This table is reserved for future diagnostic annotations and
    /// lint-style markers. It is sparse via absence in the map.
    #[serde(with = "ordered_map_vec")]
    pub span_attrs: BTreeMap<Span, Vec<AttrData>>,
}

/// Composite key for the field-attribute sibling table.
///
/// The derived `Ord` performs field-order lex comparison over
/// `(item, variant, field)`. All three component types use the
/// `newtype_index!`-generated hand-written `Ord` impl over
/// `as_u32()` (see `idx.rs`), so the ordering matches the
/// canonical walk: items first (F4-sorted post-Phase-4), then
/// variants within an item, then fields within a variant.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct FieldPath {
    /// Parent ADT Item.
    pub item: ItemId,
    /// Variant index, with `0` used for structs and unions.
    pub variant: VariantIdx,
    /// Field index within the variant.
    pub field: FieldIdx,
}

/// Closed, append-only discriminated union of per-domain attribute
/// payloads.
///
/// Variant ordering is normative for serialized RegseqModels:
/// `Intrinsic` is fixed at discriminant 0 and future variants append
/// at the end.
#[repr(u8)]
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AttrData {
    /// Diagnostic intrinsic marker stored in `DomainAttrTables.item_attrs`.
    Intrinsic(IntrinsicKind) = 0,
    /// Framework-type marker identifying an Item as a recognized Regseq
    /// framework type (such as the RegseqError error type). The bridge
    /// records this variant; the backend reads it. Fixed at discriminant 1.
    FrameworkType(FrameworkType) = 1,
    /// Interrupt-resource field marker (`#[rseq::interrupt(id = "…")]`,
    /// device-interrupt construct #6), stored in
    /// `DomainAttrTables.field_attrs`. Carries the mandatory resource id
    /// (the `[interrupt.<id>]` join key, GRILL D08) and the recognized
    /// declaration form. The joined per-driver facts live in the
    /// `InterruptTables` sibling table. Appended at discriminant 2.
    InterruptResource {
        /// The interned resource id.
        id: StringId,
        /// The declaration form (`Interrupt`/`Option<…>`/`[…; N]`).
        form: InterruptForm,
    } = 2,
    /// ISR-binding method marker (`#[rseq::isr]` /
    /// `#[rseq::isr(field = "…")]`, device-interrupt construct #6), stored in
    /// `DomainAttrTables.item_attrs`. Appended at discriminant 3.
    IsrBinding {
        /// The interned explicit `field = "…"` target, `None` for the bare
        /// form (which resolves against exactly one resource).
        field: Option<StringId>,
        /// Whether this is an array-bind handler (`index: usize` parameter).
        array_bind: bool,
    } = 3,
    /// `Atomic<T>` driver-struct field marker
    /// (atomic-memory-access-and-ordering construct), stored in
    /// `DomainAttrTables.field_attrs`. Carries the recognized element type (for
    /// backend storage-width/signedness selection) and the optional
    /// `#[rseq::atomic(initial = N)]` initial value emitted at field-init
    /// (R10/S10). The presence of this marker at a `FieldPath` is also what the
    /// generic-c backend enumerates (in `FieldPath` order) to assign a
    /// deterministic per-`Atomic`-field cell id (C07/OR-1). Appended at
    /// discriminant 4.
    Atomic {
        /// The recognized element type of `Atomic<T>`.
        element: AtomicElement,
        /// The `#[rseq::atomic(initial = N)]` literal value, or `None` for
        /// default-init (`0` / null pointer).
        initial_value: Option<i128>,
    } = 4,
    /// Synchronization lock/completion field marker (`Mutex<T>`/`SpinLock<T>`/
    /// `IrqSpinLock<T>`/`Completion`, synchronization construct), stored in
    /// `DomainAttrTables.field_attrs`. Carries the recognized lock kind — the
    /// field's wrapper type *is* the kind (GRILL `C01`). The joined per-driver
    /// facts (protected type, cardinality, acquire sites) live in the
    /// `SynchronizationTables` sibling table.
    ///
    /// Appended at discriminant **5**: discriminant **4 is reserved** for the
    /// atomicity construct's `AttrData::Atomic` (the sibling Concurrency-umbrella
    /// construct, developed in parallel) so the two append-only enums do not
    /// collide on a discriminant when their branches merge.
    SyncResource {
        /// The lock kind, given by the field's wrapper type.
        kind: SyncKind,
    } = 5,
    /// Peripheral-clock-management resource field marker
    /// (`#[rseq::clock(id = "…")]`, peripheral-clock-management construct),
    /// stored in `DomainAttrTables.field_attrs`. Carries the mandatory resource
    /// id (the `[clock.<id>]` join key, GRILL D13) and whether the declaration
    /// is the required `Clock` form (`required = true`; the future
    /// `Option<Clock>` form would set it `false`). The joined per-driver facts
    /// (mode / default_frequency / provider) live in the `ClockTables` sibling
    /// table. Appended at discriminant 6 (next-free after `SyncResource = 5`),
    /// append-only (GRILL I06/D04).
    ClockResource {
        /// The interned resource id (`#[rseq::clock(id = "…")]`).
        id: StringId,
        /// Whether this is the required `Clock` form (v1 always `true`;
        /// `Option<Clock>` is recognized-and-rejected, C06 N2).
        required: bool,
    } = 6,
    /// Pin-controller-and-GPIO-configuration resource field marker
    /// (`#[rseq::pinctrl(id = "…")]`, pinctrl state-selection construct),
    /// stored in `DomainAttrTables.field_attrs`. Carries the mandatory resource
    /// id (the `[pinctrl.<id>]` join key, GRILL D07) and whether the
    /// declaration is the required `PinCtrl` form (`required = true`) or the
    /// optional `Option<PinCtrl>` form (`required = false`, GRILL D02/D03). The
    /// joined per-driver facts (the `pin_states` inventory) live in the
    /// `PinctrlTables` sibling table. Appended at discriminant 7 (next-free
    /// after `ClockResource = 6`), append-only (GRILL I06/D09).
    PinctrlResource {
        /// The interned resource id (`#[rseq::pinctrl(id = "…")]`).
        pinctrl_id: StringId,
        /// Whether this is the required `PinCtrl` form (`true`) versus the
        /// optional `Option<PinCtrl>` form (`false`).
        required: bool,
    } = 7,
    /// Pin-state marker-type marker (`#[rseq::pin_state(id = "…")]`, pinctrl
    /// state-selection construct), stored in `DomainAttrTables.item_attrs` on
    /// the author-declared unit-struct marker item (NOT in `field_attrs`).
    /// Carries the interned state-name `id`; recognition resolves an
    /// `apply::<S>()` op's generic arg `S` to its marker `DefId` and reads this
    /// `id` to stamp the resolved state name on the op (GRILL D04/C03).
    /// Appended at discriminant 8 (next-free after `PinctrlResource = 7`),
    /// append-only.
    PinStateMarker {
        /// The interned pin-state name (`#[rseq::pin_state(id = "…")]`).
        id: StringId,
    } = 8,
}

/// Closed, append-only framework-type taxonomy.
///
/// `RegseqError` is fixed at discriminant 0. Future framework types append
/// variants at the end, preserving existing serialized discriminants.
#[repr(u8)]
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FrameworkType {
    /// The RegseqError framework error type.
    RegseqError = 0,
    /// The IrqResult ISR dispatch-result type (`#[rseq::framework_type
    /// ("irq-result")]`, device-interrupt construct #6). A dispatch result,
    /// not an error type. Appended at discriminant 1.
    IrqResult = 1,
    /// The Duration value type (`#[rseq::framework_type("duration")]`,
    /// timed-cpu-delay-and-sleep construct). An opaque u64-nanosecond amount
    /// consumed by the `busy_wait`/`sleep` intrinsics (and the existing
    /// `poll_until`/`reset.toggle`/`sync.wait` Duration consumers). The bridge
    /// records this marker; backends read the inner u64 ns off the operand.
    /// Appended at discriminant 2.
    Duration = 2,
}

/// Closed, append-only intrinsic-family taxonomy.
///
/// `Log` is fixed at discriminant 0. Future intrinsic families append
/// variants at the end, preserving existing serialized discriminants.
#[repr(u8)]
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum IntrinsicKind {
    /// Diagnostic-logging intrinsic family.
    Log(LogLevel) = 0,
    /// Register-access MMIO operation family (register-access construct).
    /// Appended at discriminant 1, preserving `Log` at 0.
    Mmio(MmioOp) = 1,
    /// Busy-poll intrinsic (`regseq::poll::until`). Appended at discriminant 2.
    PollUntil = 2,
    /// Bus config-space access family (`regseq::bus::read_config/write_config`,
    /// bus-resource-management construct #3, M5). Appended at discriminant 3,
    /// preserving the discriminants above. PCI-only (BRM-001, the non-PCI
    /// rejection is the construct-#3 M6 diagnostic).
    Config(ConfigOp) = 3,
    /// Busy-wait CPU spin (`regseq::delay::busy_wait`,
    /// timed-cpu-delay-and-sleep construct). Spins for ≥ a `Duration` without
    /// yielding; legal in ANY execution context (including IRQ/atomic).
    /// Appended at discriminant 4, preserving the discriminants above.
    BusyWait = 4,
    /// Scheduler sleep (`regseq::delay::sleep`, timed-cpu-delay-and-sleep
    /// construct). Yields the calling task for ≥ a `Duration`; legal only in
    /// sleepable context (sleeping in a marked IRQ region is rejected by
    /// TCD-ERR-01). Appended at discriminant 5.
    Sleep = 5,
    /// Atomic memory-access-and-ordering operation family
    /// (atomic-memory-access-and-ordering construct). The lock-free ops on a
    /// typed driver-local `Atomic<T>` field (`load`/`store`/`fetch_*`/`swap`/
    /// `compare_exchange`/`bit_test_*`) plus the standalone CPU memory
    /// `regseq::atomic::fence`. Each op carries an explicit, compile-time-
    /// constant memory `Ordering` whose discriminant rides in the `Call.args`
    /// const operand (the model `Call` terminator is unchanged — C06/I08).
    /// Appended at discriminant 6, preserving the discriminants above.
    Atomic(AtomicOp) = 6,
    /// Peripheral-clock-management operation family
    /// (peripheral-clock-management construct). The author ops on a typed
    /// driver-local `Clock` field (`enable`/`disable`/`get_rate`), tagged via
    /// the facade `#[rseq::intrinsic("clock_*")]` markers. The compiler-managed
    /// `acquire_clock`/`release` are NOT intrinsics — they are synthesized from
    /// the clock *field* (the `model.clock` resource), so they have no
    /// `IntrinsicKind`. `ClockOp::SetRate` is reserved (not added) until
    /// `set_rate` is built. Appended at discriminant 7, preserving the
    /// discriminants above (GRILL D04/C03).
    Clock(ClockOp) = 7,
    /// Pin-controller-and-GPIO-configuration operation family (pinctrl
    /// state-selection construct). The author op on a typed driver-local
    /// `PinCtrl` resource (`apply::<S>()`), tagged via the facade
    /// `#[rseq::intrinsic("pinctrl_apply")]` marker. The compiler-managed
    /// `acquire_pinctrl`/`release_pinctrl` are NOT intrinsics — they are
    /// synthesized from the pinctrl *field* (the `model.pinctrl` resource), so
    /// they have no `IntrinsicKind`. Appended at discriminant 8, preserving the
    /// discriminants above (GRILL D04/D09/C03).
    Pinctrl(PinctrlOp) = 8,
}

/// Config-space operation carried by [`IntrinsicKind::Config`]
/// (bus-resource-management construct #3, M5). These correspond to the
/// `#[rseq::intrinsic("read_config"/"write_config")]`-tagged
/// `regseq::bus::{read_config,write_config}` free functions. Closed,
/// append-only.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ConfigOp {
    /// `read_config::<T>(offset)`: read `size_of::<T>()` bytes of PCI config
    /// space (the PCI_COMMAND witness + the PCI-ID-caching rule).
    Read = 0,
    /// `write_config::<T>(offset, val)`: write `size_of::<T>()` bytes of PCI
    /// config space (recognized + lowerable; no behavioral witness, GRILL O3).
    Write = 1,
}

/// Atomic operation carried by [`IntrinsicKind::Atomic`]
/// (atomic-memory-access-and-ordering construct). These correspond to the
/// `#[rseq::intrinsic("atomic_*")]`-tagged wrapper methods on `Atomic<T>` and
/// the `regseq::atomic::fence` free function. Closed, append-only — variants
/// append at the end preserving existing serialized discriminants.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AtomicOp {
    /// `atomic_load`: read the atomic value (Relaxed/Acquire/SeqCst).
    Load = 0,
    /// `atomic_store`: write the atomic value (Relaxed/Release/SeqCst).
    Store = 1,
    /// `atomic_fetch_*`/`atomic_swap`: read-modify-write returning the prior
    /// value (all five orderings legal). The sub-operation is the nested
    /// [`FetchOp`], mirroring the fieldful `IntrinsicKind::Mmio(MmioOp)` shape.
    FetchModify(FetchOp) = 2,
    /// `atomic_compare_exchange`: strong CAS with separate success/failure
    /// orderings, returning `Result<T, T>` (I07/C04).
    CompareExchange = 3,
    /// `atomic_bit_test_set`: set bit `n`, return the prior bit (unsigned `T`).
    BitTestSet = 4,
    /// `atomic_bit_test_clear`: clear bit `n`, return the prior bit (unsigned `T`).
    BitTestClear = 5,
    /// `atomic_fence`: standalone CPU memory fence — CPU-ordering ONLY, never
    /// MMIO (D02/I03). Not a method; the `regseq::atomic::fence` free function.
    Fence = 6,
}

/// The fetch-modify sub-operation carried by [`AtomicOp::FetchModify`]
/// (atomic-memory-access-and-ordering construct). Closed, append-only.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FetchOp {
    /// `atomic_fetch_add`: two's-complement wrapping add.
    Add = 0,
    /// `atomic_fetch_sub`: two's-complement wrapping sub.
    Sub = 1,
    /// `atomic_fetch_or`: bitwise or.
    Or = 2,
    /// `atomic_fetch_and`: bitwise and.
    And = 3,
    /// `atomic_fetch_xor`: bitwise xor.
    Xor = 4,
    /// `atomic_swap`: unconditional exchange (no arithmetic).
    Swap = 5,
}

/// The element type of an `Atomic<T>` field
/// (atomic-memory-access-and-ordering construct), recorded by the bridge so
/// backends select the storage width + signedness without re-deriving `T`. The
/// admitted set is `{u32,i32,u64,i64}` plus a permissive pointer-width integer
/// atomic (C07/D08; `T ∉` this set ⇒ `ATOMIC-ERR-ELEMENT-TYPE`). Closed,
/// append-only.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AtomicElement {
    /// `u32` — 32-bit unsigned.
    U32 = 0,
    /// `i32` — 32-bit signed.
    I32 = 1,
    /// `u64` — 64-bit unsigned.
    U64 = 2,
    /// `i64` — 64-bit signed.
    I64 = 3,
    /// A pointer-width integer atomic (`*mut T`/`*const T`; LP64 ⇒ 64-bit).
    Ptr = 4,
}

impl AtomicElement {
    /// The storage/access width in bits (32 or 64; pointer atomics are LP64).
    pub fn width_bits(self) -> u32 {
        match self {
            AtomicElement::U32 | AtomicElement::I32 => 32,
            AtomicElement::U64 | AtomicElement::I64 | AtomicElement::Ptr => 64,
        }
    }

    /// Whether the element is a signed integer (governs two's-complement
    /// wrapping + sign-extension on value ops).
    pub fn is_signed(self) -> bool {
        matches!(self, AtomicElement::I32 | AtomicElement::I64)
    }

    /// Whether `bit_test_*` is legal on this element (unsigned integer only —
    /// D08; signed and pointer elements ⇒ `ATOMIC-ERR-BIT-TEST-TYPE`).
    pub fn allows_bit_test(self) -> bool {
        matches!(self, AtomicElement::U32 | AtomicElement::U64)
    }
}

/// MMIO register-access operation carried by `IntrinsicKind::Mmio`.
///
/// These correspond to the `#[rseq::intrinsic("mmio_*")]`-tagged wrapper
/// methods on the `Mmio<T>` resource. Closed, append-only.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MmioOp {
    /// `mmio_read`: read a full register identified by a `RegisterId`.
    Read = 0,
    /// `mmio_read_field`: read a masked/shifted field identified by a `Field`.
    ReadField = 1,
    /// `mmio_write`: write a full register value.
    Write = 2,
    /// `mmio_write_field`: write masked field(s) via a `FieldValue`.
    WriteField = 3,
    /// `mmio_modify`: read-modify-write a register via a `RegisterId` + `FieldValue`.
    Modify = 4,
}

/// Diagnostic-logging severity carried by `IntrinsicKind::Log`.
#[repr(u8)]
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LogLevel {
    /// Debug severity.
    Debug = 0,
    /// Informational severity.
    Info = 1,
    /// Warning severity.
    Warn = 2,
    /// Error severity.
    Error = 3,
}

/// Peripheral-clock-management author operation carried by
/// [`IntrinsicKind::Clock`] (peripheral-clock-management construct). These
/// correspond to the `#[rseq::intrinsic("clock_*")]`-tagged wrapper methods on
/// the `Clock` resource. Closed, append-only — `SetRate` is reserved (not
/// added) until `set_rate` is built (GRILL D04, Out of Scope).
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ClockOp {
    /// `clock_enable`: gate the clock on (managed → `clk_prepare_enable`;
    /// static → no-op). Author-called, fallible (`-> Result`).
    Enable = 0,
    /// `clock_disable`: gate the clock off (managed → `clk_disable_unprepare`;
    /// static → no-op). Author-called, infallible.
    Disable = 1,
    /// `clock_get_rate`: read the clock rate in Hz (managed → raw live query,
    /// no fallback, D10; static → const-fold `default_frequency`).
    GetRate = 2,
}

/// Pin-controller-and-GPIO-configuration author operation carried by
/// [`IntrinsicKind::Pinctrl`] (pinctrl state-selection construct). This
/// corresponds to the `#[rseq::intrinsic("pinctrl_apply")]`-tagged `apply::<S>()`
/// method on the `PinCtrl` resource (and the `OptionalPinCtrl` extension on
/// `Option<PinCtrl>`). Closed, append-only — `acquire`/`release` are
/// compiler-managed (synthesized from the field, not author intrinsics).
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PinctrlOp {
    /// `pinctrl_apply`: select a named pin state (`apply::<S>()`). The resolved
    /// state name (read off the `S` marker's `PinStateMarker.id`) is stamped on
    /// the op by recognition; backends do not re-resolve. Author-called,
    /// fallible (`-> Result`).
    ApplyState = 0,
}

impl IntrinsicKind {
    /// Closed-vocabulary lookup from dash-form attribute NAME to an
    /// intrinsic kind.
    ///
    /// The NAME strings use dash form, such as `log-debug`. This is
    /// intentionally distinct from Rust function names such as
    /// `log_debug`. Unknown names return `None`.
    pub fn from_name(name: Symbol) -> Option<Self> {
        match name.as_str() {
            "log-debug" => Some(IntrinsicKind::Log(LogLevel::Debug)),
            "log-info" => Some(IntrinsicKind::Log(LogLevel::Info)),
            "log-warn" => Some(IntrinsicKind::Log(LogLevel::Warn)),
            "log-error" => Some(IntrinsicKind::Log(LogLevel::Error)),
            // Register-access MMIO intrinsics (underscore form, matching the
            // facade `#[rseq::intrinsic("mmio_*")]` / `"poll_until"` tags).
            "mmio_read" => Some(IntrinsicKind::Mmio(MmioOp::Read)),
            "mmio_read_field" => Some(IntrinsicKind::Mmio(MmioOp::ReadField)),
            "mmio_write" => Some(IntrinsicKind::Mmio(MmioOp::Write)),
            "mmio_write_field" => Some(IntrinsicKind::Mmio(MmioOp::WriteField)),
            "mmio_modify" => Some(IntrinsicKind::Mmio(MmioOp::Modify)),
            "poll_until" => Some(IntrinsicKind::PollUntil),
            // Bus config-space intrinsics (construct #3 M5; underscore form,
            // matching the `#[rseq::intrinsic("read_config"/"write_config")]` tags).
            "read_config" => Some(IntrinsicKind::Config(ConfigOp::Read)),
            "write_config" => Some(IntrinsicKind::Config(ConfigOp::Write)),
            // Timed-CPU-delay-and-sleep intrinsics (underscore form, matching
            // the facade `#[rseq::intrinsic("busy_wait"/"sleep")]` tags).
            // `sleep_ex`/`sleep_until` reserve NAMEs only -- no discriminants
            // until their later fold.
            "busy_wait" => Some(IntrinsicKind::BusyWait),
            "sleep" => Some(IntrinsicKind::Sleep),
            // Atomic-memory-access-and-ordering intrinsics (underscore form,
            // matching the facade `#[rseq::intrinsic("atomic_*")]` tags; the
            // `atomic_fence` is the `regseq::atomic::fence` free function).
            "atomic_load" => Some(IntrinsicKind::Atomic(AtomicOp::Load)),
            "atomic_store" => Some(IntrinsicKind::Atomic(AtomicOp::Store)),
            "atomic_fetch_add" => Some(IntrinsicKind::Atomic(AtomicOp::FetchModify(FetchOp::Add))),
            "atomic_fetch_sub" => Some(IntrinsicKind::Atomic(AtomicOp::FetchModify(FetchOp::Sub))),
            "atomic_fetch_or" => Some(IntrinsicKind::Atomic(AtomicOp::FetchModify(FetchOp::Or))),
            "atomic_fetch_and" => Some(IntrinsicKind::Atomic(AtomicOp::FetchModify(FetchOp::And))),
            "atomic_fetch_xor" => Some(IntrinsicKind::Atomic(AtomicOp::FetchModify(FetchOp::Xor))),
            "atomic_swap" => Some(IntrinsicKind::Atomic(AtomicOp::FetchModify(FetchOp::Swap))),
            "atomic_compare_exchange" => Some(IntrinsicKind::Atomic(AtomicOp::CompareExchange)),
            "atomic_bit_test_set" => Some(IntrinsicKind::Atomic(AtomicOp::BitTestSet)),
            "atomic_bit_test_clear" => Some(IntrinsicKind::Atomic(AtomicOp::BitTestClear)),
            "atomic_fence" => Some(IntrinsicKind::Atomic(AtomicOp::Fence)),
            // Peripheral-clock-management intrinsics (underscore form, matching
            // the facade `#[rseq::intrinsic("clock_*")]` tags). `clock_set_rate`
            // is NOT in the vocabulary (reserved, Out of Scope) and `acquire`/
            // `release` are compiler-managed, not author-called intrinsics.
            "clock_enable" => Some(IntrinsicKind::Clock(ClockOp::Enable)),
            "clock_disable" => Some(IntrinsicKind::Clock(ClockOp::Disable)),
            "clock_get_rate" => Some(IntrinsicKind::Clock(ClockOp::GetRate)),
            // Pin-controller-and-GPIO-configuration intrinsic (underscore form,
            // matching the facade `#[rseq::intrinsic("pinctrl_apply")]` tag).
            // `acquire_pinctrl`/`release_pinctrl` are compiler-managed (not
            // author-called intrinsics).
            "pinctrl_apply" => Some(IntrinsicKind::Pinctrl(PinctrlOp::ApplyState)),
            _ => None,
        }
    }
}

/// Serde adapter that serializes a `BTreeMap` as a JSON array of
/// `(key, value)` pairs.
///
/// The array-of-pairs JSON shape is required because `FieldPath`
/// and `Span` are struct keys (not strings), and JSON object keys
/// must be strings. `BTreeMap` iteration is naturally sorted by
/// key, so the emitted `Vec` is deterministic across runs
/// regardless of insertion order.
mod ordered_map_vec {
    use super::*;
    use serde::{Deserializer, Serializer};
    use std::collections::BTreeMap;

    pub fn serialize<K, V, S>(map: &BTreeMap<K, V>, serializer: S) -> Result<S::Ok, S::Error>
    where
        K: Serialize + Ord,
        V: Serialize,
        S: Serializer,
    {
        let entries: Vec<(&K, &V)> = map.iter().collect();
        entries.serialize(serializer)
    }

    pub fn deserialize<'de, K, V, D>(deserializer: D) -> Result<BTreeMap<K, V>, D::Error>
    where
        K: Deserialize<'de> + Ord,
        V: Deserialize<'de>,
        D: Deserializer<'de>,
    {
        let entries = Vec::<(K, V)>::deserialize(deserializer)?;
        Ok(entries.into_iter().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{HashMap, HashSet};

    use serde::de::DeserializeOwned;

    use crate::ids::SourceFileId;

    fn symbol(name: &'static str) -> Symbol {
        Symbol::new(name)
    }

    fn levels() -> [LogLevel; 4] {
        [
            LogLevel::Debug,
            LogLevel::Info,
            LogLevel::Warn,
            LogLevel::Error,
        ]
    }

    fn log(level: LogLevel) -> IntrinsicKind {
        IntrinsicKind::Log(level)
    }

    fn attr(level: LogLevel) -> AttrData {
        AttrData::Intrinsic(log(level))
    }

    fn path(item: u32, variant: u32, field: u32) -> FieldPath {
        FieldPath {
            item: ItemId::from_u32(item),
            variant: VariantIdx::from_u32(variant),
            field: FieldIdx::from_u32(field),
        }
    }

    fn span(lo: u32, hi: u32) -> Span {
        Span {
            file: SourceFileId::SYNTHESIZED,
            lo,
            hi,
        }
    }

    fn assert_json_round_trip<T>(value: T)
    where
        T: Clone + core::fmt::Debug + PartialEq + Serialize + DeserializeOwned,
    {
        let encoded = serde_json::to_string(&value).expect("serialize");
        let decoded: T = serde_json::from_str(&encoded).expect("deserialize");
        assert_eq!(decoded, value);
    }

    fn attr_data_tag(value: &AttrData) -> u8 {
        // SAFETY: `AttrData` is `repr(u8)` with explicit discriminants.
        // Primitive-repr enums with fields store a `u8` tag; reading the
        // tag byte lets this test assert the spec-pinned discriminant.
        unsafe { *(value as *const AttrData as *const u8) }
    }

    fn intrinsic_kind_tag(value: &IntrinsicKind) -> u8 {
        // SAFETY: `IntrinsicKind` is `repr(u8)` with explicit discriminants;
        // the same tag-byte read as `attr_data_tag`. (`as u8` is unavailable
        // because some variants carry fields.)
        unsafe { *(value as *const IntrinsicKind as *const u8) }
    }

    #[test]
    fn log_level_variants_construct_and_compare() {
        let [debug, info, warn, error] = levels();
        assert_ne!(debug, info);
        assert_ne!(debug, warn);
        assert_ne!(debug, error);
        assert_ne!(info, warn);
        assert_ne!(info, error);
        assert_ne!(warn, error);
    }

    #[test]
    fn log_level_hash_and_clone_round_trip() {
        let mut set = HashSet::new();
        for level in levels() {
            set.insert(level.clone());
        }
        for level in levels() {
            assert!(set.contains(&level.clone()));
        }
    }

    #[test]
    fn intrinsic_log_variants_construct_and_compare() {
        let values = levels().map(log);
        for i in 0..values.len() {
            for j in (i + 1)..values.len() {
                assert_ne!(values[i], values[j]);
            }
        }
    }

    #[test]
    fn from_name_maps_dash_form_names() {
        let cases = [
            ("log-debug", LogLevel::Debug),
            ("log-info", LogLevel::Info),
            ("log-warn", LogLevel::Warn),
            ("log-error", LogLevel::Error),
        ];

        for (name, level) in cases {
            assert_eq!(IntrinsicKind::from_name(symbol(name)), Some(log(level)));
        }
    }

    #[test]
    fn from_name_rejects_non_vocabulary_names() {
        for name in ["log_debug", "mmio-read32", "", "lambda-\u{03bb}"] {
            assert_eq!(IntrinsicKind::from_name(symbol(name)), None);
        }
    }

    #[test]
    fn attr_data_intrinsic_tag_is_zero() {
        // `AttrData::Intrinsic` must stay at serialized discriminant 0 regardless
        // of which intrinsic family it carries.
        assert_eq!(
            attr_data_tag(&AttrData::Intrinsic(IntrinsicKind::PollUntil)),
            0
        );
    }

    #[test]
    fn attr_data_intrinsic_variants_construct_and_compare() {
        let values = levels().map(attr);
        for i in 0..values.len() {
            for j in (i + 1)..values.len() {
                assert_ne!(values[i], values[j]);
            }
        }
    }

    #[test]
    fn attr_data_intrinsic_discriminant_is_stable() {
        assert_eq!(attr_data_tag(&attr(LogLevel::Debug)), 0);
        assert_eq!(attr_data_tag(&attr(LogLevel::Error)), 0);
    }

    #[test]
    fn field_path_constructs_and_clones() {
        let path = path(0, 0, 2);
        let cloned = path.clone();
        assert_eq!(path, cloned);
        assert_eq!(path.item, ItemId::from_u32(0));
        assert_eq!(path.variant, VariantIdx::from_u32(0));
        assert_eq!(path.field, FieldIdx::from_u32(2));
    }

    #[test]
    fn field_path_hash_map_key_behavior() {
        let key = path(0, 0, 2);
        let mut map = HashMap::new();
        map.insert(key.clone(), ());
        assert_eq!(map.get(&path(0, 0, 2)), Some(&()));
        assert_eq!(map.get(&path(1, 0, 2)), None);
        assert_eq!(map.get(&path(0, 1, 2)), None);
        assert_eq!(map.get(&path(0, 0, 3)), None);
    }

    #[test]
    fn domain_attr_tables_construct() {
        let tables = DomainAttrTables::default();
        assert!(tables.item_attrs.is_empty());
        assert!(tables.field_attrs.is_empty());
        assert!(tables.span_attrs.is_empty());
    }

    #[test]
    fn item_attrs_mutation_round_trips() {
        let mut tables = DomainAttrTables {
            item_attrs: IndexVec::from_elem(Vec::new(), 1),
            ..DomainAttrTables::default()
        };
        tables.item_attrs[ItemId::from_u32(0)].push(attr(LogLevel::Info));
        assert_eq!(
            tables.item_attrs[ItemId::from_u32(0)],
            vec![attr(LogLevel::Info)]
        );
    }

    #[test]
    fn field_attrs_mutation_round_trips() {
        let mut tables = DomainAttrTables::default();
        let key = path(0, 0, 2);
        let value = vec![attr(LogLevel::Warn)];
        tables.field_attrs.insert(key.clone(), value.clone());
        assert_eq!(tables.field_attrs.get(&key), Some(&value));
    }

    #[test]
    fn span_attrs_mutation_round_trips() {
        let mut tables = DomainAttrTables::default();
        let key = span(1, 3);
        let value = vec![attr(LogLevel::Error)];
        tables.span_attrs.insert(key, value.clone());
        assert_eq!(tables.span_attrs.get(&key), Some(&value));
    }

    #[test]
    fn multiple_attr_data_entries_keep_insertion_order() {
        let mut tables = DomainAttrTables {
            item_attrs: IndexVec::from_elem(Vec::new(), 1),
            ..DomainAttrTables::default()
        };
        tables.item_attrs[ItemId::from_u32(0)].push(attr(LogLevel::Debug));
        tables.item_attrs[ItemId::from_u32(0)].push(attr(LogLevel::Error));
        assert_eq!(
            tables.item_attrs[ItemId::from_u32(0)],
            vec![attr(LogLevel::Debug), attr(LogLevel::Error)]
        );
    }

    #[test]
    fn log_level_json_round_trips() {
        for level in levels() {
            assert_json_round_trip(level);
        }
    }

    #[test]
    fn intrinsic_kind_json_round_trips() {
        for level in levels() {
            assert_json_round_trip(log(level));
        }
    }

    #[test]
    fn config_intrinsic_from_name_round_trip_and_discriminant() {
        // Bus-resource-management construct #3 M5: the config-space intrinsics
        // map from their underscore-form NAMEs, round-trip through JSON, and the
        // `Config` family appends at discriminant 3 (Log/Mmio/PollUntil unmoved).
        assert_eq!(
            IntrinsicKind::from_name(symbol("read_config")),
            Some(IntrinsicKind::Config(ConfigOp::Read))
        );
        assert_eq!(
            IntrinsicKind::from_name(symbol("write_config")),
            Some(IntrinsicKind::Config(ConfigOp::Write))
        );
        assert_json_round_trip(IntrinsicKind::Config(ConfigOp::Read));
        assert_json_round_trip(IntrinsicKind::Config(ConfigOp::Write));
        // The two config ops are distinct, and `AttrData::Intrinsic` stays at 0.
        assert_ne!(
            IntrinsicKind::Config(ConfigOp::Read),
            IntrinsicKind::Config(ConfigOp::Write)
        );
        assert_eq!(
            attr_data_tag(&AttrData::Intrinsic(IntrinsicKind::Config(ConfigOp::Write))),
            0
        );
    }

    #[test]
    fn timed_delay_from_name_round_trip_and_discriminants() {
        // Timed-cpu-delay-and-sleep: busy_wait/sleep map from their
        // underscore-form NAMEs, round-trip through JSON, are distinct, and
        // append at discriminants 4/5 with every prior family unmoved
        // (Log=0 / Mmio=1 / PollUntil=2 / Config=3). `AttrData::Intrinsic`
        // stays at 0 regardless of which family it carries.
        assert_eq!(
            IntrinsicKind::from_name(symbol("busy_wait")),
            Some(IntrinsicKind::BusyWait)
        );
        assert_eq!(
            IntrinsicKind::from_name(symbol("sleep")),
            Some(IntrinsicKind::Sleep)
        );
        assert_json_round_trip(IntrinsicKind::BusyWait);
        assert_json_round_trip(IntrinsicKind::Sleep);
        assert_ne!(IntrinsicKind::BusyWait, IntrinsicKind::Sleep);
        // Append-only discriminant stability (pinned numeric tags).
        assert_eq!(intrinsic_kind_tag(&IntrinsicKind::Log(LogLevel::Debug)), 0);
        assert_eq!(intrinsic_kind_tag(&IntrinsicKind::Mmio(MmioOp::Read)), 1);
        assert_eq!(intrinsic_kind_tag(&IntrinsicKind::PollUntil), 2);
        assert_eq!(intrinsic_kind_tag(&IntrinsicKind::Config(ConfigOp::Read)), 3);
        assert_eq!(intrinsic_kind_tag(&IntrinsicKind::BusyWait), 4);
        assert_eq!(intrinsic_kind_tag(&IntrinsicKind::Sleep), 5);
        assert_eq!(attr_data_tag(&AttrData::Intrinsic(IntrinsicKind::Sleep)), 0);
        // `sleep_ex`/`sleep_until` reserve NAMEs only — no discriminants yet.
        assert_eq!(IntrinsicKind::from_name(symbol("sleep_ex")), None);
        assert_eq!(IntrinsicKind::from_name(symbol("sleep_until")), None);
    }

    #[test]
    fn atomic_from_name_round_trip_and_discriminants() {
        // Atomic-memory-access-and-ordering: every `atomic_*` tag maps from its
        // underscore-form NAME, round-trips through JSON, and the `Atomic`
        // family appends at IntrinsicKind discriminant 6 with every prior
        // family unmoved (Log=0 / Mmio=1 / PollUntil=2 / Config=3 / BusyWait=4 /
        // Sleep=5).
        let cases = [
            ("atomic_load", AtomicOp::Load),
            ("atomic_store", AtomicOp::Store),
            ("atomic_fetch_add", AtomicOp::FetchModify(FetchOp::Add)),
            ("atomic_fetch_sub", AtomicOp::FetchModify(FetchOp::Sub)),
            ("atomic_fetch_or", AtomicOp::FetchModify(FetchOp::Or)),
            ("atomic_fetch_and", AtomicOp::FetchModify(FetchOp::And)),
            ("atomic_fetch_xor", AtomicOp::FetchModify(FetchOp::Xor)),
            ("atomic_swap", AtomicOp::FetchModify(FetchOp::Swap)),
            ("atomic_compare_exchange", AtomicOp::CompareExchange),
            ("atomic_bit_test_set", AtomicOp::BitTestSet),
            ("atomic_bit_test_clear", AtomicOp::BitTestClear),
            ("atomic_fence", AtomicOp::Fence),
        ];
        for (name, op) in cases {
            assert_eq!(
                IntrinsicKind::from_name(symbol(name)),
                Some(IntrinsicKind::Atomic(op)),
                "from_name({name})"
            );
            assert_json_round_trip(IntrinsicKind::Atomic(op));
        }
        // The `Atomic` family appends at IntrinsicKind discriminant 6; every
        // prior family discriminant is unmoved.
        assert_eq!(intrinsic_kind_tag(&IntrinsicKind::Log(LogLevel::Debug)), 0);
        assert_eq!(intrinsic_kind_tag(&IntrinsicKind::Mmio(MmioOp::Read)), 1);
        assert_eq!(intrinsic_kind_tag(&IntrinsicKind::PollUntil), 2);
        assert_eq!(intrinsic_kind_tag(&IntrinsicKind::Config(ConfigOp::Read)), 3);
        assert_eq!(intrinsic_kind_tag(&IntrinsicKind::BusyWait), 4);
        assert_eq!(intrinsic_kind_tag(&IntrinsicKind::Sleep), 5);
        assert_eq!(
            intrinsic_kind_tag(&IntrinsicKind::Atomic(AtomicOp::Load)),
            6
        );
        // `AttrData::Intrinsic` stays at 0 regardless of which family it carries.
        assert_eq!(
            attr_data_tag(&AttrData::Intrinsic(IntrinsicKind::Atomic(AtomicOp::Fence))),
            0
        );
        // A non-vocabulary atomic-ish name stays outside the closed set.
        assert_eq!(IntrinsicKind::from_name(symbol("atomic_fetch_nand")), None);
        assert_eq!(IntrinsicKind::from_name(symbol("atomic")), None);
    }

    #[test]
    fn atomic_op_inner_discriminants_are_stable() {
        // SAFETY: `AtomicOp` is `repr(u8)`; read the tag byte. `as u8` is
        // unavailable because `FetchModify` carries a field.
        fn atomic_op_tag(value: &AtomicOp) -> u8 {
            unsafe { *(value as *const AtomicOp as *const u8) }
        }
        assert_eq!(atomic_op_tag(&AtomicOp::Load), 0);
        assert_eq!(atomic_op_tag(&AtomicOp::Store), 1);
        assert_eq!(atomic_op_tag(&AtomicOp::FetchModify(FetchOp::Add)), 2);
        assert_eq!(atomic_op_tag(&AtomicOp::CompareExchange), 3);
        assert_eq!(atomic_op_tag(&AtomicOp::BitTestSet), 4);
        assert_eq!(atomic_op_tag(&AtomicOp::BitTestClear), 5);
        assert_eq!(atomic_op_tag(&AtomicOp::Fence), 6);
        // FetchOp is fieldless, so `as u8` yields its pinned discriminant.
        assert_eq!(FetchOp::Add as u8, 0);
        assert_eq!(FetchOp::Sub as u8, 1);
        assert_eq!(FetchOp::Or as u8, 2);
        assert_eq!(FetchOp::And as u8, 3);
        assert_eq!(FetchOp::Xor as u8, 4);
        assert_eq!(FetchOp::Swap as u8, 5);
        // AtomicElement is fieldless and pinned.
        assert_eq!(AtomicElement::U32 as u8, 0);
        assert_eq!(AtomicElement::I32 as u8, 1);
        assert_eq!(AtomicElement::U64 as u8, 2);
        assert_eq!(AtomicElement::I64 as u8, 3);
        assert_eq!(AtomicElement::Ptr as u8, 4);
        // Width/signedness/bit-test helpers.
        assert_eq!(AtomicElement::U32.width_bits(), 32);
        assert_eq!(AtomicElement::U64.width_bits(), 64);
        assert_eq!(AtomicElement::Ptr.width_bits(), 64);
        assert!(AtomicElement::I32.is_signed());
        assert!(!AtomicElement::U32.is_signed());
        assert!(AtomicElement::U32.allows_bit_test());
        assert!(!AtomicElement::I32.allows_bit_test());
        assert!(!AtomicElement::Ptr.allows_bit_test());
    }

    #[test]
    fn attr_data_atomic_discriminant_and_round_trip() {
        // The `Atomic` field-attr variant appends at AttrData discriminant 4
        // without shifting the prior variants, and round-trips through JSON
        // (both `Some`/`None` initial value).
        assert_eq!(
            attr_data_tag(&AttrData::Atomic {
                element: AtomicElement::U32,
                initial_value: Some(0),
            }),
            4
        );
        assert_eq!(attr_data_tag(&attr(LogLevel::Debug)), 0);
        assert_eq!(
            attr_data_tag(&AttrData::FrameworkType(FrameworkType::RegseqError)),
            1
        );
        for element in [
            AtomicElement::U32,
            AtomicElement::I32,
            AtomicElement::U64,
            AtomicElement::I64,
            AtomicElement::Ptr,
        ] {
            assert_json_round_trip(AttrData::Atomic {
                element,
                initial_value: Some(-1),
            });
            assert_json_round_trip(AttrData::Atomic {
                element,
                initial_value: None,
            });
        }
        // A large unsigned initial (u64 high bit) round-trips through the i128
        // carrier.
        assert_json_round_trip(AttrData::Atomic {
            element: AtomicElement::U64,
            initial_value: Some(i128::from(u64::MAX)),
        });
    }

    #[test]
    fn attr_data_json_round_trips() {
        for level in levels() {
            assert_json_round_trip(attr(level));
        }
    }

    #[test]
    fn field_path_json_round_trips() {
        assert_json_round_trip(path(0, 1, 2));
    }

    #[test]
    fn domain_attr_tables_json_round_trips() {
        let mut tables = DomainAttrTables {
            item_attrs: IndexVec::from_elem(Vec::new(), 2),
            ..DomainAttrTables::default()
        };
        tables.item_attrs[ItemId::from_u32(0)].push(attr(LogLevel::Debug));
        tables
            .field_attrs
            .insert(path(3, 0, 1), vec![attr(LogLevel::Info)]);
        tables
            .span_attrs
            .insert(span(5, 8), vec![attr(LogLevel::Warn)]);

        assert_json_round_trip(tables);
    }

    #[test]
    fn domain_attrs_json_is_deterministic_across_insert_order() {
        let entries = [
            (
                FieldPath {
                    item: ItemId::from_u32(5),
                    variant: VariantIdx::ZERO,
                    field: FieldIdx::from_u32(2),
                },
                vec![AttrData::Intrinsic(IntrinsicKind::Log(LogLevel::Debug))],
            ),
            (
                FieldPath {
                    item: ItemId::from_u32(2),
                    variant: VariantIdx::ZERO,
                    field: FieldIdx::from_u32(7),
                },
                vec![AttrData::Intrinsic(IntrinsicKind::Log(LogLevel::Info))],
            ),
            (
                FieldPath {
                    item: ItemId::from_u32(9),
                    variant: VariantIdx::ZERO,
                    field: FieldIdx::from_u32(0),
                },
                vec![AttrData::Intrinsic(IntrinsicKind::Log(LogLevel::Warn))],
            ),
        ];

        let mut a = DomainAttrTables::default();
        for (k, v) in entries.iter() {
            a.field_attrs.insert(k.clone(), v.clone());
        }

        let mut b = DomainAttrTables::default();
        for (k, v) in entries.iter().rev() {
            b.field_attrs.insert(k.clone(), v.clone());
        }

        let json_a = serde_json::to_string(&a).unwrap();
        let json_b = serde_json::to_string(&b).unwrap();
        assert_eq!(
            json_a, json_b,
            "JSON output must be byte-identical regardless of insertion order"
        );
    }

    #[test]
    fn span_attrs_json_is_deterministic_across_insert_order() {
        let entries = [
            (
                Span {
                    file: SourceFileId::SYNTHESIZED,
                    lo: 30,
                    hi: 40,
                },
                vec![AttrData::Intrinsic(IntrinsicKind::Log(LogLevel::Debug))],
            ),
            (
                Span {
                    file: SourceFileId::SYNTHESIZED,
                    lo: 5,
                    hi: 12,
                },
                vec![AttrData::Intrinsic(IntrinsicKind::Log(LogLevel::Info))],
            ),
            (
                Span {
                    file: SourceFileId::SYNTHESIZED,
                    lo: 100,
                    hi: 110,
                },
                vec![AttrData::Intrinsic(IntrinsicKind::Log(LogLevel::Warn))],
            ),
        ];

        let mut a = DomainAttrTables::default();
        for (k, v) in entries.iter() {
            a.span_attrs.insert(*k, v.clone());
        }

        let mut b = DomainAttrTables::default();
        for (k, v) in entries.iter().rev() {
            b.span_attrs.insert(*k, v.clone());
        }

        let json_a = serde_json::to_string(&a).unwrap();
        let json_b = serde_json::to_string(&b).unwrap();
        assert_eq!(
            json_a, json_b,
            "JSON output must be byte-identical regardless of insertion order"
        );
    }

    #[test]
    fn framework_type_json_round_trips() {
        assert_json_round_trip(FrameworkType::RegseqError);
    }

    #[test]
    fn attr_data_framework_type_json_round_trips() {
        assert_json_round_trip(AttrData::FrameworkType(FrameworkType::RegseqError));
    }

    #[test]
    fn attr_data_framework_type_discriminant_is_stable() {
        // The FrameworkType variant is pinned at discriminant 1...
        assert_eq!(
            attr_data_tag(&AttrData::FrameworkType(FrameworkType::RegseqError)),
            1
        );
        // ...and adding it must not shift Intrinsic off discriminant 0.
        assert_eq!(attr_data_tag(&attr(LogLevel::Debug)), 0);
    }

    #[test]
    fn framework_type_inner_discriminant_is_stable() {
        // SAFETY-free cast: `FrameworkType` is a fieldless `repr(u8)` enum,
        // so the `as u8` cast yields its pinned discriminant.
        assert_eq!(FrameworkType::RegseqError as u8, 0);
        // IrqResult appends at 1 (device-interrupt construct #6) without
        // shifting RegseqError off 0.
        assert_eq!(FrameworkType::IrqResult as u8, 1);
        // Duration appends at 2 (timed-cpu-delay-and-sleep construct) without
        // shifting the prior two.
        assert_eq!(FrameworkType::Duration as u8, 2);
    }

    #[test]
    fn duration_framework_type_json_round_trips() {
        // The Duration framework type (timed-cpu-delay-and-sleep construct)
        // serializes by its pinned discriminant and round-trips, both bare and
        // wrapped in an `AttrData::FrameworkType` marker.
        assert_json_round_trip(FrameworkType::Duration);
        assert_json_round_trip(AttrData::FrameworkType(FrameworkType::Duration));
        // Wrapping it leaves the `FrameworkType` AttrData variant at 1...
        assert_eq!(
            attr_data_tag(&AttrData::FrameworkType(FrameworkType::Duration)),
            1
        );
        // ...and does not shift Intrinsic off discriminant 0.
        assert_eq!(attr_data_tag(&attr(LogLevel::Debug)), 0);
    }

    #[test]
    fn attr_data_interrupt_discriminants_are_stable() {
        // The construct-#6 variants append at 2 and 3 (I06)...
        assert_eq!(
            attr_data_tag(&AttrData::InterruptResource {
                id: StringId::from_u32(1),
                form: InterruptForm::Single,
            }),
            2
        );
        assert_eq!(
            attr_data_tag(&AttrData::IsrBinding {
                field: None,
                array_bind: false,
            }),
            3
        );
        // ...and adding them must not shift the prior variants.
        assert_eq!(attr_data_tag(&attr(LogLevel::Debug)), 0);
        assert_eq!(
            attr_data_tag(&AttrData::FrameworkType(FrameworkType::RegseqError)),
            1
        );
    }

    #[test]
    fn attr_data_interrupt_variants_json_round_trip() {
        for form in [
            InterruptForm::Single,
            InterruptForm::Optional,
            InterruptForm::Array { len: 2 },
        ] {
            assert_json_round_trip(AttrData::InterruptResource {
                id: StringId::from_u32(4),
                form,
            });
        }
        assert_json_round_trip(AttrData::IsrBinding {
            field: None,
            array_bind: false,
        });
        assert_json_round_trip(AttrData::IsrBinding {
            field: Some(StringId::from_u32(9)),
            array_bind: true,
        });
        assert_json_round_trip(AttrData::FrameworkType(FrameworkType::IrqResult));
    }

    #[test]
    fn attr_data_sync_resource_discriminant_is_stable() {
        // The synchronization field marker appends at discriminant **5**
        // (discriminant 4 is reserved for the atomicity construct's
        // `AttrData::Atomic`)...
        assert_eq!(
            attr_data_tag(&AttrData::SyncResource {
                kind: SyncKind::IrqSpinLock,
            }),
            5
        );
        // ...and adding it must not shift any prior variant.
        assert_eq!(attr_data_tag(&attr(LogLevel::Debug)), 0);
        assert_eq!(
            attr_data_tag(&AttrData::FrameworkType(FrameworkType::RegseqError)),
            1
        );
        assert_eq!(
            attr_data_tag(&AttrData::InterruptResource {
                id: StringId::from_u32(1),
                form: InterruptForm::Single,
            }),
            2
        );
        assert_eq!(
            attr_data_tag(&AttrData::IsrBinding {
                field: None,
                array_bind: false,
            }),
            3
        );
    }

    #[test]
    fn attr_data_sync_resource_variants_json_round_trip() {
        for kind in [
            SyncKind::Mutex,
            SyncKind::SpinLock,
            SyncKind::IrqSpinLock,
            SyncKind::Completion,
        ] {
            assert_json_round_trip(AttrData::SyncResource { kind });
        }
    }

    #[test]
    fn clock_from_name_round_trip_and_discriminants() {
        // Peripheral-clock-management: every `clock_*` author op maps from its
        // underscore-form NAME, round-trips through JSON, and the `Clock` family
        // appends at IntrinsicKind discriminant 7 with every prior family
        // unmoved (Log=0 / Mmio=1 / PollUntil=2 / Config=3 / BusyWait=4 /
        // Sleep=5 / Atomic=6).
        let cases = [
            ("clock_enable", ClockOp::Enable),
            ("clock_disable", ClockOp::Disable),
            ("clock_get_rate", ClockOp::GetRate),
        ];
        for (name, op) in cases {
            assert_eq!(
                IntrinsicKind::from_name(symbol(name)),
                Some(IntrinsicKind::Clock(op)),
                "from_name({name})"
            );
            assert_json_round_trip(IntrinsicKind::Clock(op));
        }
        assert_eq!(intrinsic_kind_tag(&IntrinsicKind::Log(LogLevel::Debug)), 0);
        assert_eq!(intrinsic_kind_tag(&IntrinsicKind::Mmio(MmioOp::Read)), 1);
        assert_eq!(intrinsic_kind_tag(&IntrinsicKind::PollUntil), 2);
        assert_eq!(intrinsic_kind_tag(&IntrinsicKind::Config(ConfigOp::Read)), 3);
        assert_eq!(intrinsic_kind_tag(&IntrinsicKind::BusyWait), 4);
        assert_eq!(intrinsic_kind_tag(&IntrinsicKind::Sleep), 5);
        assert_eq!(
            intrinsic_kind_tag(&IntrinsicKind::Atomic(AtomicOp::Load)),
            6
        );
        assert_eq!(
            intrinsic_kind_tag(&IntrinsicKind::Clock(ClockOp::Enable)),
            7
        );
        // `AttrData::Intrinsic` stays at 0 regardless of which family it carries.
        assert_eq!(
            attr_data_tag(&AttrData::Intrinsic(IntrinsicKind::Clock(ClockOp::GetRate))),
            0
        );
        // `clock_set_rate` is reserved (Out of Scope); `acquire`/`release` are
        // compiler-managed (not intrinsics); dash-forms are rejected (clock uses
        // underscore form, matching the facade tags).
        assert_eq!(IntrinsicKind::from_name(symbol("clock_set_rate")), None);
        assert_eq!(IntrinsicKind::from_name(symbol("clock_acquire")), None);
        assert_eq!(IntrinsicKind::from_name(symbol("clock_release")), None);
        assert_eq!(IntrinsicKind::from_name(symbol("clock-enable")), None);
        assert_eq!(IntrinsicKind::from_name(symbol("clock")), None);
    }

    #[test]
    fn clock_op_inner_discriminants_are_stable() {
        // `ClockOp` is fieldless, so `as u8` yields its pinned discriminant.
        assert_eq!(ClockOp::Enable as u8, 0);
        assert_eq!(ClockOp::Disable as u8, 1);
        assert_eq!(ClockOp::GetRate as u8, 2);
    }

    #[test]
    fn attr_data_clock_resource_discriminant_and_round_trip() {
        // The `ClockResource` field-attr variant appends at AttrData
        // discriminant 6 (next-free after `SyncResource = 5`) without shifting
        // any prior variant, and round-trips through JSON for both `required`
        // values.
        assert_eq!(
            attr_data_tag(&AttrData::ClockResource {
                id: StringId::from_u32(1),
                required: true,
            }),
            6
        );
        assert_eq!(attr_data_tag(&attr(LogLevel::Debug)), 0);
        assert_eq!(
            attr_data_tag(&AttrData::FrameworkType(FrameworkType::RegseqError)),
            1
        );
        assert_eq!(
            attr_data_tag(&AttrData::InterruptResource {
                id: StringId::from_u32(1),
                form: InterruptForm::Single,
            }),
            2
        );
        assert_eq!(
            attr_data_tag(&AttrData::IsrBinding {
                field: None,
                array_bind: false,
            }),
            3
        );
        assert_eq!(
            attr_data_tag(&AttrData::Atomic {
                element: AtomicElement::U32,
                initial_value: None,
            }),
            4
        );
        assert_eq!(
            attr_data_tag(&AttrData::SyncResource {
                kind: SyncKind::Mutex,
            }),
            5
        );
        for required in [true, false] {
            assert_json_round_trip(AttrData::ClockResource {
                id: StringId::from_u32(7),
                required,
            });
        }
    }

    #[test]
    fn pinctrl_from_name_round_trip_and_discriminants() {
        // Pin-controller-and-GPIO-configuration: the `pinctrl_apply` author op
        // maps from its underscore-form NAME, round-trips through JSON, and the
        // `Pinctrl` family appends at IntrinsicKind discriminant 8 with every
        // prior family unmoved (Log=0 / Mmio=1 / PollUntil=2 / Config=3 /
        // BusyWait=4 / Sleep=5 / Atomic=6 / Clock=7).
        assert_eq!(
            IntrinsicKind::from_name(symbol("pinctrl_apply")),
            Some(IntrinsicKind::Pinctrl(PinctrlOp::ApplyState))
        );
        assert_json_round_trip(IntrinsicKind::Pinctrl(PinctrlOp::ApplyState));
        assert_eq!(intrinsic_kind_tag(&IntrinsicKind::Log(LogLevel::Debug)), 0);
        assert_eq!(intrinsic_kind_tag(&IntrinsicKind::Mmio(MmioOp::Read)), 1);
        assert_eq!(intrinsic_kind_tag(&IntrinsicKind::PollUntil), 2);
        assert_eq!(intrinsic_kind_tag(&IntrinsicKind::Config(ConfigOp::Read)), 3);
        assert_eq!(intrinsic_kind_tag(&IntrinsicKind::BusyWait), 4);
        assert_eq!(intrinsic_kind_tag(&IntrinsicKind::Sleep), 5);
        assert_eq!(
            intrinsic_kind_tag(&IntrinsicKind::Atomic(AtomicOp::Load)),
            6
        );
        assert_eq!(
            intrinsic_kind_tag(&IntrinsicKind::Clock(ClockOp::Enable)),
            7
        );
        assert_eq!(
            intrinsic_kind_tag(&IntrinsicKind::Pinctrl(PinctrlOp::ApplyState)),
            8
        );
        // `AttrData::Intrinsic` stays at 0 regardless of which family it carries.
        assert_eq!(
            attr_data_tag(&AttrData::Intrinsic(IntrinsicKind::Pinctrl(
                PinctrlOp::ApplyState
            ))),
            0
        );
        // `acquire`/`release` are compiler-managed (not intrinsics); dash-form
        // is rejected (pinctrl uses underscore form, matching the facade tag).
        assert_eq!(IntrinsicKind::from_name(symbol("pinctrl_acquire")), None);
        assert_eq!(IntrinsicKind::from_name(symbol("pinctrl_release")), None);
        assert_eq!(IntrinsicKind::from_name(symbol("pinctrl-apply")), None);
        assert_eq!(IntrinsicKind::from_name(symbol("pinctrl")), None);
    }

    #[test]
    fn pinctrl_op_inner_discriminants_are_stable() {
        // `PinctrlOp` is fieldless, so `as u8` yields its pinned discriminant.
        assert_eq!(PinctrlOp::ApplyState as u8, 0);
    }

    #[test]
    fn attr_data_pinctrl_resource_discriminant_and_round_trip() {
        // The `PinctrlResource` field-attr variant appends at AttrData
        // discriminant 7 (next-free after `ClockResource = 6`) without shifting
        // any prior variant, and round-trips through JSON for both `required`
        // values.
        assert_eq!(
            attr_data_tag(&AttrData::PinctrlResource {
                pinctrl_id: StringId::from_u32(1),
                required: true,
            }),
            7
        );
        // Prior variants unmoved.
        assert_eq!(attr_data_tag(&attr(LogLevel::Debug)), 0);
        assert_eq!(
            attr_data_tag(&AttrData::ClockResource {
                id: StringId::from_u32(1),
                required: true,
            }),
            6
        );
        for required in [true, false] {
            assert_json_round_trip(AttrData::PinctrlResource {
                pinctrl_id: StringId::from_u32(7),
                required,
            });
        }
    }

    #[test]
    fn attr_data_pin_state_marker_discriminant_and_round_trip() {
        // The `PinStateMarker` item-attr variant appends at AttrData
        // discriminant 8 (next-free after `PinctrlResource = 7`) without
        // shifting any prior variant, and round-trips through JSON.
        assert_eq!(
            attr_data_tag(&AttrData::PinStateMarker {
                id: StringId::from_u32(3),
            }),
            8
        );
        assert_eq!(
            attr_data_tag(&AttrData::PinctrlResource {
                pinctrl_id: StringId::from_u32(1),
                required: false,
            }),
            7
        );
        assert_json_round_trip(AttrData::PinStateMarker {
            id: StringId::from_u32(3),
        });
    }

    #[test]
    fn attr_data_and_intrinsic_kind_discriminants_have_no_collision() {
        // Explicit no-collision check (PLAN T01/T12/T25): the clock AND pinctrl
        // additions (AttrData::{PinctrlResource, PinStateMarker} +
        // IntrinsicKind::Pinctrl) must not alias any existing occupant. One
        // instance per variant; their tag bytes must all be distinct.
        let attr_variants = [
            AttrData::Intrinsic(IntrinsicKind::PollUntil),
            AttrData::FrameworkType(FrameworkType::RegseqError),
            AttrData::InterruptResource {
                id: StringId::from_u32(1),
                form: InterruptForm::Single,
            },
            AttrData::IsrBinding {
                field: None,
                array_bind: false,
            },
            AttrData::Atomic {
                element: AtomicElement::U32,
                initial_value: None,
            },
            AttrData::SyncResource {
                kind: SyncKind::Mutex,
            },
            AttrData::ClockResource {
                id: StringId::from_u32(1),
                required: true,
            },
            AttrData::PinctrlResource {
                pinctrl_id: StringId::from_u32(1),
                required: true,
            },
            AttrData::PinStateMarker {
                id: StringId::from_u32(1),
            },
        ];
        let attr_tags: std::collections::BTreeSet<u8> =
            attr_variants.iter().map(attr_data_tag).collect();
        assert_eq!(
            attr_tags.len(),
            attr_variants.len(),
            "AttrData discriminant collision"
        );

        let intrinsic_variants = [
            IntrinsicKind::Log(LogLevel::Debug),
            IntrinsicKind::Mmio(MmioOp::Read),
            IntrinsicKind::PollUntil,
            IntrinsicKind::Config(ConfigOp::Read),
            IntrinsicKind::BusyWait,
            IntrinsicKind::Sleep,
            IntrinsicKind::Atomic(AtomicOp::Load),
            IntrinsicKind::Clock(ClockOp::Enable),
            IntrinsicKind::Pinctrl(PinctrlOp::ApplyState),
        ];
        let intrinsic_tags: std::collections::BTreeSet<u8> =
            intrinsic_variants.iter().map(intrinsic_kind_tag).collect();
        assert_eq!(
            intrinsic_tags.len(),
            intrinsic_variants.len(),
            "IntrinsicKind discriminant collision"
        );
    }

    #[test]
    fn framework_type_attrs_json_is_deterministic_across_insert_order() {
        let entries = [
            (
                FieldPath {
                    item: ItemId::from_u32(5),
                    variant: VariantIdx::ZERO,
                    field: FieldIdx::from_u32(2),
                },
                vec![AttrData::FrameworkType(FrameworkType::RegseqError)],
            ),
            (
                FieldPath {
                    item: ItemId::from_u32(2),
                    variant: VariantIdx::ZERO,
                    field: FieldIdx::from_u32(7),
                },
                vec![AttrData::FrameworkType(FrameworkType::RegseqError)],
            ),
            (
                FieldPath {
                    item: ItemId::from_u32(9),
                    variant: VariantIdx::ZERO,
                    field: FieldIdx::from_u32(0),
                },
                vec![AttrData::FrameworkType(FrameworkType::RegseqError)],
            ),
        ];

        let mut a = DomainAttrTables::default();
        for (k, v) in entries.iter() {
            a.field_attrs.insert(k.clone(), v.clone());
        }

        let mut b = DomainAttrTables::default();
        for (k, v) in entries.iter().rev() {
            b.field_attrs.insert(k.clone(), v.clone());
        }

        let json_a = serde_json::to_string(&a).unwrap();
        let json_b = serde_json::to_string(&b).unwrap();
        assert_eq!(
            json_a, json_b,
            "JSON output must be byte-identical regardless of insertion order"
        );
    }

    #[test]
    fn domain_attr_tables_json_round_trip_preserves_all_entries() {
        let mut tables = DomainAttrTables::default();

        let field_entries = [
            (path(7, 0, 3), vec![attr(LogLevel::Debug)]),
            (path(2, 0, 5), vec![attr(LogLevel::Info)]),
            (path(11, 0, 1), vec![attr(LogLevel::Warn)]),
        ];
        for (k, v) in field_entries.iter() {
            tables.field_attrs.insert(k.clone(), v.clone());
        }

        let span_entries = [
            (span(50, 60), vec![attr(LogLevel::Debug)]),
            (span(10, 20), vec![attr(LogLevel::Info)]),
            (span(200, 210), vec![attr(LogLevel::Warn)]),
        ];
        for (k, v) in span_entries.iter() {
            tables.span_attrs.insert(*k, v.clone());
        }

        let encoded = serde_json::to_string(&tables).expect("serialize");
        let decoded: DomainAttrTables = serde_json::from_str(&encoded).expect("deserialize");

        assert_eq!(decoded.field_attrs.len(), field_entries.len());
        for (k, v) in field_entries.iter() {
            assert_eq!(decoded.field_attrs.get(k), Some(v));
        }

        assert_eq!(decoded.span_attrs.len(), span_entries.len());
        for (k, v) in span_entries.iter() {
            assert_eq!(decoded.span_attrs.get(k), Some(v));
        }
    }
}
